use std::io;
use std::net::SocketAddr;
use futures::future::join_all;

use tokio::task::JoinHandle;

use crate::dns;
use crate::utils::{self, CommonAddr};
use crate::config::{
    EndpointConfig, EpHalfConfig, HTTP2Config, NetConfig, TLSConfig,
    TransportConfig, WebSocketConfig, WithTransport,
};
use crate::transport::plain;
use crate::transport::{AsyncConnect, AsyncAccept};

mod copy;
mod stream;

#[cfg(target_os = "linux")]
mod zero_copy;

fn parse_domain_name(s: &str) -> Option<(String, u16)> {
    let mut iter = s.splitn(2, ':');
    let addr = iter.next()?.to_string();
    let port = iter.next()?.parse::<u16>().ok()?;
    // check addr
    if dns::resolve_sync(&addr).is_ok() {
        Some((addr, port))
    } else {
        None
    }
}

fn parse_socket_addr(
    addr: &str,
    allow_domain_name: bool,
) -> io::Result<CommonAddr> {
    if let Ok(sockaddr) = addr.parse::<SocketAddr>() {
        return Ok(CommonAddr::SocketAddr(sockaddr));
    };
    if allow_domain_name {
        if let Some((addr, port)) = parse_domain_name(addr) {
            return Ok(CommonAddr::DomainName(addr, port));
        }
    };
    Err(utils::new_io_err("unable to parse socket addr"))
}

fn new_plain_conn(addr: &str, net: &NetConfig) -> plain::Connector {
    #[cfg(unix)]
    use std::path::PathBuf;
    match net {
        NetConfig::TCP => {
            let sockaddr = parse_socket_addr(addr, true).unwrap();
            plain::Connector::new(sockaddr)
        }
        #[cfg(unix)]
        NetConfig::UDS => {
            let path = CommonAddr::UnixSocketPath(PathBuf::from(addr));
            plain::Connector::new(path)
        }
        NetConfig::UDP => unreachable!(),
    }
}

fn new_plain_lis(addr: &str, net: &NetConfig) -> plain::Acceptor {
    #[cfg(unix)]
    use std::path::PathBuf;
    match net {
        NetConfig::TCP => {
            let sockaddr = parse_socket_addr(addr, false).unwrap();
            plain::Acceptor::new(sockaddr)
        }
        #[cfg(unix)]
        NetConfig::UDS => {
            let path = CommonAddr::UnixSocketPath(PathBuf::from(addr));
            plain::Acceptor::new(path)
        }
        NetConfig::UDP => unreachable!(),
    }
}

#[cfg(target_os = "linux")]
fn meet_zero_copy(
    lis_trans: &TransportConfig,
    conn_trans: &TransportConfig,
) -> bool {
    matches!(
        (lis_trans, conn_trans),
        (TransportConfig::Plain, TransportConfig::Plain)
    )
}

fn spawn_conn_half_with_trans<L, C>(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    conn_trans: &TransportConfig,
    lis: L,
    conn: C,
) where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    match conn_trans {
        TransportConfig::Plain => {
            workers.push(tokio::spawn(stream::proxy(lis, conn)));
        }
        TransportConfig::WS(connc) => {
            let conn = <WebSocketConfig as WithTransport<L, C>>::apply_to_conn(
                connc, conn,
            );
            workers.push(tokio::spawn(stream::proxy(lis, conn)));
        }
        TransportConfig::H2(connc) => {
            let conn = <HTTP2Config as WithTransport<L, C>>::apply_to_conn(
                connc, conn,
            );
            workers.push(tokio::spawn(stream::proxy(lis, conn)));
        }
    }
}

fn spawn_lis_half_with_trans<L, C>(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    lis_trans: &TransportConfig,
    conn_trans: &TransportConfig,
    lis: L,
    conn: C,
) where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    match lis_trans {
        TransportConfig::Plain => {
            spawn_conn_half_with_trans(workers, conn_trans, lis, conn);
        }
        TransportConfig::WS(lisc) => {
            let lis = <WebSocketConfig as WithTransport<L, C>>::apply_to_lis(
                lisc, lis,
            );
            spawn_conn_half_with_trans(workers, conn_trans, lis, conn);
        }
        TransportConfig::H2(lisc) => {
            let lis =
                <HTTP2Config as WithTransport<L, C>>::apply_to_lis(lisc, lis);
            spawn_conn_half_with_trans(workers, conn_trans, lis, conn);
        }
    }
}

fn spawn_with_tls(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    listen: &EpHalfConfig,
    remote: &EpHalfConfig,
    lis: plain::Acceptor,
    conn: plain::Connector,
) {
    match &listen.tls {
        TLSConfig::Server(lisc) => match &remote.tls {
            TLSConfig::Client(connc) => spawn_lis_half_with_trans(
                workers,
                &listen.trans,
                &remote.trans,
                lisc.apply_to_lis(lis),
                connc.apply_to_conn(conn),
            ),
            _ => spawn_lis_half_with_trans(
                workers,
                &listen.trans,
                &remote.trans,
                lisc.apply_to_lis(lis),
                conn,
            ),
        },
        _ => match &remote.tls {
            TLSConfig::Client(connc) => spawn_lis_half_with_trans(
                workers,
                &listen.trans,
                &remote.trans,
                lis,
                connc.apply_to_conn(conn),
            ),

            _ => spawn_lis_half_with_trans(
                workers,
                &listen.trans,
                &remote.trans,
                lis,
                conn,
            ),
        },
    }
}

pub async fn run(eps: Vec<EndpointConfig>) {
    let mut workers: Vec<JoinHandle<io::Result<()>>> =
        Vec::with_capacity(eps.len());
    for ep in eps.into_iter() {
        // convert to full config
        let EndpointConfig { listen, remote } = ep;
        let listen: EpHalfConfig = listen.into();
        let remote: EpHalfConfig = remote.into();
        // init listener and connector
        let plain_lis = new_plain_lis(&listen.addr, &listen.net);
        let plain_conn = new_plain_conn(&remote.addr, &remote.net);
        // create zero-copy task
        #[cfg(target_os = "linux")]
        if meet_zero_copy(&listen.trans, &remote.trans) {
            workers.push(tokio::spawn(stream::splice(plain_lis, plain_conn)));
            continue;
        }
        // load transport config and create task
        spawn_with_tls(&mut workers, &listen, &remote, plain_lis, plain_conn);
    }
    join_all(workers).await;
}
