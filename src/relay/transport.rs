use std::io;
use std::sync::Arc;

use tokio::task::JoinHandle;

use crate::io::proxy;
use crate::config::{
    EpHalfConfig, HTTP2Config, TLSConfig, TransportConfig, WebSocketConfig,
    WithTransport,
};
use crate::transport::{AsyncConnect, AsyncAccept};

fn spawn_lis_half_with_trans<L, C>(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    lis_trans: &TransportConfig,
    lis: L,
    conn: C,
) where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    match lis_trans {
        TransportConfig::Plain => {
            workers.push(tokio::spawn(proxy(Arc::new(lis), Arc::new(conn))));
        }
        TransportConfig::WS(lisc) => {
            let lis = <WebSocketConfig as WithTransport<L, C>>::apply_to_lis(
                lisc, lis,
            );
            workers.push(tokio::spawn(proxy(Arc::new(lis), Arc::new(conn))));
        }
        TransportConfig::H2(lisc) => {
            let conn = Arc::new(conn);
            let lis =
                <HTTP2Config as WithTransport<L, C>>::apply_to_lis_with_conn(
                    lisc,
                    conn.clone(),
                    lis,
                );
            workers.push(tokio::spawn(proxy(Arc::new(lis), conn)));
        }
    }
}

fn spawn_conn_half_with_trans<L, C>(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    lis_trans: &TransportConfig,
    conn_trans: &TransportConfig,
    lis: L,
    conn: C,
) where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    match conn_trans {
        TransportConfig::Plain => {
            spawn_lis_half_with_trans(workers, lis_trans, lis, conn);
        }
        TransportConfig::WS(connc) => {
            let conn = <WebSocketConfig as WithTransport<L, C>>::apply_to_conn(
                connc, conn,
            );
            spawn_lis_half_with_trans(workers, lis_trans, lis, conn);
        }
        TransportConfig::H2(connc) => {
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
    lis: L,
    conn: C,
) where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    match &listen.tls {
        TLSConfig::Server(lisc) => match &remote.tls {
            TLSConfig::Client(connc) => spawn_conn_half_with_trans(
                workers,
                &listen.trans,
                &remote.trans,
                lisc.apply_to_lis(lis),
                connc.apply_to_conn(conn),
            ),
            _ => spawn_conn_half_with_trans(
                workers,
                &listen.trans,
                &remote.trans,
                lisc.apply_to_lis(lis),
                conn,
            ),
        },
        _ => match &remote.tls {
            TLSConfig::Client(connc) => spawn_conn_half_with_trans(
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
    lis: L,
    conn: C,
) where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    spawn_with_tls(workers, listen, remote, lis, conn)
}
