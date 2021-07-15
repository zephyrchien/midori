use std::io;
use std::path::PathBuf;
use std::net::SocketAddr;
use futures::future::join_all;

use tokio::task::JoinHandle;

use crate::dns;
use crate::utils::{self, CommonAddr};
use crate::config::{
    EndpointConfig, NetConfig, WithTransport, TransportConfig, WebSocketConfig,
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

fn meet_zero_copy(
    lis_trans: &TransportConfig,
    conn_trans: &TransportConfig,
) -> bool {
    if let TransportConfig::Plain = lis_trans {
        if let TransportConfig::Plain = conn_trans {
            return true;
        }
    }
    false
}

async fn spawn_lis_haf_with_trans<L, C>(
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
            spawn_conn_haf_with_trans(workers, conn_trans, lis, conn).await;
        }
        TransportConfig::WS(lisc) => {
            let lis = <WebSocketConfig as WithTransport<L, C>>::apply_to_lis(
                lisc, lis,
            );
            spawn_conn_haf_with_trans(workers, conn_trans, lis, conn).await;
        }
    }
}

async fn spawn_conn_haf_with_trans<L, C>(
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
    }
}

pub async fn run(eps: Vec<EndpointConfig>) {
    let mut workers: Vec<JoinHandle<io::Result<()>>> =
        Vec::with_capacity(eps.len());
    for ep in eps.into_iter() {
        let plain_lis = new_plain_lis(&ep.listen.addr, &ep.listen.net);
        let plain_conn = new_plain_conn(&ep.remote.addr, &ep.remote.net);
        #[cfg(target_os = "linux")]
        if meet_zero_copy(&ep.listen.trans, &ep.remote.trans) {
            workers.push(tokio::spawn(stream::splice(plain_lis, plain_conn)));
            continue;
        }
        spawn_lis_haf_with_trans(
            &mut workers,
            &ep.listen.trans,
            &ep.remote.trans,
            plain_lis,
            plain_conn,
        )
        .await;
    }
    join_all(workers).await;
}
