use std::io;
use std::sync::Arc;

use log::debug;
use tokio::task::JoinHandle;

use crate::io::proxy;
use crate::utils::MaybeQuic;
use crate::config::{EpHalfConfig, TransportConfig, WithTransport, TLSConfig};
use crate::transport::{AsyncConnect, AsyncAccept};

// #[cfg(feature = "tls")]
// use crate::config::tls::{TLSClientConfig, TLSServerConfig};

#[cfg(feature = "ws")]
use crate::config::trans::WebSocketConfig;

#[cfg(feature = "h2c")]
use crate::config::trans::HTTP2Config;

// #[cfg(feature = "quic")]
// use crate::config::trans::QuicConfig;

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
        Plain => {
            let lis = lis.take_other().unwrap();
            workers.push(tokio::spawn(proxy(Arc::new(lis), Arc::new(conn))));
        }
        #[cfg(feature = "ws")]
        WS(lisc) => {
            let lis = <WebSocketConfig as WithTransport<L, C>>::apply_to_lis(
                lisc,
                lis.take_other().unwrap(),
            );
            workers.push(tokio::spawn(proxy(Arc::new(lis), Arc::new(conn))));
        }
        #[cfg(feature = "h2c")]
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
        #[cfg(feature = "quic")]
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
        Plain => {
            spawn_lis_half_with_trans(workers, lis_trans, lis, conn);
        }
        #[cfg(feature = "ws")]
        WS(connc) => {
            let conn = <WebSocketConfig as WithTransport<L, C>>::apply_to_conn(
                connc, conn,
            );
            spawn_lis_half_with_trans(workers, lis_trans, lis, conn);
        }
        #[cfg(feature = "h2c")]
        H2(connc) => {
            let conn = <HTTP2Config as WithTransport<L, C>>::apply_to_conn(
                connc, conn,
            );
            spawn_lis_half_with_trans(workers, lis_trans, lis, conn);
        }
        // quic does not need extra configuration
        #[cfg(feature = "quic")]
        QUIC(_) => {
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
    #[cfg(feature = "quic")]
    use TransportConfig::QUIC;

    #[cfg(not(feature = "quic"))]
    let (is_quic_lis, is_quic_conn) = (false, false);
    #[cfg(feature = "quic")]
    let (is_quic_lis, is_quic_conn) = (
        matches!(&listen.trans, QUIC(_)),
        matches!(&remote.trans, QUIC(_)),
    );

    debug!("load listen tls[{}]", &listen.tls);
    debug!("load remote tls[{}]", &remote.tls);

    match &listen.tls {
        #[cfg(feature = "tls")]
        Server(lisc) if !is_quic_lis => match &remote.tls {
            Client(connc) if !is_quic_conn => spawn_conn_half_with_trans(
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
            #[cfg(feature = "tls")]
            Client(connc) if !is_quic_conn => spawn_conn_half_with_trans(
                workers,
                &listen.trans,
                &remote.trans,
                lis,
                connc.apply_to_conn(conn),
            ),

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
