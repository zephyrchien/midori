use std::io;
use std::sync::Arc;

use log::debug;
use tokio::task::JoinHandle;

use crate::io::proxy;
use crate::utils::MaybeQuic;
use crate::config::{
    EpHalfConfig, HTTP2Config, TLSConfig, TransportConfig, WebSocketConfig,
    WithTransport,
};
use crate::transport::{AsyncConnect, AsyncAccept};

fn spawn_lis_half_with_trans<L, C>(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    lis_trans: &TransportConfig,
    lis: MaybeQuic<L>,
    conn: C,
) where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    use TransportConfig::*;
    debug!("load listen transport[{}]", lis_trans);
    match lis_trans {
        // quic does not need extra configuration
        Plain => {
            let lis = lis.take_other().unwrap();
            workers.push(tokio::spawn(proxy(Arc::new(lis), Arc::new(conn))));
        }
        WS(lisc) => {
            let lis = <WebSocketConfig as WithTransport<L, C>>::apply_to_lis(
                lisc,
                lis.take_other().unwrap(),
            );
            workers.push(tokio::spawn(proxy(Arc::new(lis), Arc::new(conn))));
        }
        H2(lisc) => {
            let conn = Arc::new(conn);
            let lis =
                <HTTP2Config as WithTransport<L, C>>::apply_to_lis_with_conn(
                    lisc,
                    conn.clone(),
                    lis.take_other().unwrap(),
                );
            workers.push(tokio::spawn(proxy(Arc::new(lis), conn)));
        }
        QUIC(_) => {
            let conn = Arc::new(conn);
            let lis = lis.take_quic().unwrap().set_connector(conn.clone());
            workers.push(tokio::spawn(proxy(Arc::new(lis), conn)));
        }
    }
}

fn spawn_conn_half_with_trans<L, C>(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    lis_trans: &TransportConfig,
    conn_trans: &TransportConfig,
    lis: MaybeQuic<L>,
    conn: C,
) where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    use TransportConfig::*;
    debug!("load remote transport[{}]", conn_trans);
    match conn_trans {
        // quic does not need extra configuration
        Plain | QUIC(_) => {
            spawn_lis_half_with_trans(workers, lis_trans, lis, conn);
        }
        WS(connc) => {
            let conn = <WebSocketConfig as WithTransport<L, C>>::apply_to_conn(
                connc, conn,
            );
            spawn_lis_half_with_trans(workers, lis_trans, lis, conn);
        }
        H2(connc) => {
            let conn = <HTTP2Config as WithTransport<L, C>>::apply_to_conn(
                connc, conn,
            );
            spawn_lis_half_with_trans(workers, lis_trans, lis, conn);
        }
    }
}

fn spawn_with_tls<L, C>(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    listen: &EpHalfConfig,
    remote: &EpHalfConfig,
    lis: MaybeQuic<L>,
    conn: C,
) where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    use TLSConfig::*;
    use TransportConfig::QUIC;

    debug!("load listen tls[{}]", &listen.tls);
    debug!("load remote tls[{}]", &remote.tls);

    match &listen.tls {
        Server(lisc) if !matches!(listen.trans, QUIC(_)) => match &remote.tls {
            Client(connc) => spawn_conn_half_with_trans(
                workers,
                &listen.trans,
                &remote.trans,
                lisc.apply_to_lis_ext(lis),
                connc.apply_to_conn(conn),
            ),
            _ => spawn_conn_half_with_trans(
                workers,
                &listen.trans,
                &remote.trans,
                lisc.apply_to_lis_ext(lis),
                conn,
            ),
        },
        _ => match &remote.tls {
            Client(connc) if !matches!(remote.trans, QUIC(_)) => {
                spawn_conn_half_with_trans(
                    workers,
                    &listen.trans,
                    &remote.trans,
                    lis,
                    connc.apply_to_conn(conn),
                )
            }

            _ => spawn_conn_half_with_trans(
                workers,
                &listen.trans,
                &remote.trans,
                lis,
                conn,
            ),
        },
    }
}

pub fn spawn_with_trans<L, C>(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    listen: &EpHalfConfig,
    remote: &EpHalfConfig,
    lis: MaybeQuic<L>,
    conn: C,
) where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    spawn_with_tls(workers, listen, remote, lis, conn)
}
