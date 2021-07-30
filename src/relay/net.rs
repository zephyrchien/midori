use std::io;
use tokio::task::JoinHandle;

use super::common;
use super::transport;
use crate::config::{EpHalfConfig, NetConfig};
use crate::transport::plain::{self, PlainListener};
use crate::transport::udp;

pub fn new_plain_conn(addr: &str, net: &NetConfig) -> plain::Connector {
    #[cfg(unix)]
    use std::path::PathBuf;
    #[cfg(unix)]
    use crate::utils::CommonAddr;
    match net {
        NetConfig::TCP => {
            let sockaddr = common::parse_socket_addr(addr, true).unwrap();
            plain::Connector::new(sockaddr)
        }
        #[cfg(unix)]
        NetConfig::UDS => {
            let path = CommonAddr::UnixSocketPath(PathBuf::from(addr));
            plain::Connector::new(path)
        }
        _ => unreachable!(),
    }
}

pub fn new_plain_lis(addr: &str, net: &NetConfig) -> plain::Acceptor {
    #[cfg(unix)]
    use std::path::PathBuf;
    #[cfg(unix)]
    use crate::utils::CommonAddr;
    match net {
        NetConfig::TCP => {
            let sockaddr = common::parse_socket_addr(addr, false).unwrap();
            let lis = PlainListener::bind(&sockaddr).unwrap();
            plain::Acceptor::new(lis, sockaddr)
        }
        #[cfg(unix)]
        NetConfig::UDS => {
            let path = CommonAddr::UnixSocketPath(PathBuf::from(addr));
            let lis = PlainListener::bind(&path).unwrap();
            plain::Acceptor::new(lis, path)
        }
        _ => unreachable!(),
    }
}

// ===== UDP =====
pub fn new_udp_conn(addr: &str, net: &NetConfig) -> udp::Connector {
    match net {
        NetConfig::UDP => {
            let sockaddr = common::parse_socket_addr(addr, true).unwrap();
            udp::Connector::new(sockaddr)
        }
        _ => unreachable!(),
    }
}

pub fn new_udp_lis(addr: &str, net: &NetConfig) -> udp::Acceptor {
    match net {
        NetConfig::UDP => {
            let sockaddr = common::parse_socket_addr(addr, false).unwrap();
            udp::Acceptor::new(sockaddr)
        }
        _ => unreachable!(),
    }
}

pub fn spawn_with_net(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    listen: &EpHalfConfig,
    remote: &EpHalfConfig,
) {
    match listen.net {
        NetConfig::TCP | NetConfig::UDS => {
            let lis = new_plain_lis(&listen.addr, &listen.net);
            match remote.net {
                NetConfig::TCP | NetConfig::UDS => {
                    let conn = new_plain_conn(&remote.addr, &remote.net);
                    transport::spawn_with_trans(
                        workers, listen, remote, lis, conn,
                    )
                }
                NetConfig::UDP => {
                    let conn = new_udp_conn(&remote.addr, &remote.net);
                    transport::spawn_with_trans(
                        workers, listen, remote, lis, conn,
                    )
                }
            }
        }
        NetConfig::UDP => {
            let lis = new_udp_lis(&listen.addr, &listen.net);
            match remote.net {
                NetConfig::TCP | NetConfig::UDS => {
                    let conn = new_plain_conn(&remote.addr, &remote.net);
                    transport::spawn_with_trans(
                        workers, listen, remote, lis, conn,
                    )
                }
                NetConfig::UDP => {
                    let conn = new_udp_conn(&remote.addr, &remote.net);
                    transport::spawn_with_trans(
                        workers, listen, remote, lis, conn,
                    )
                }
            }
        }
    }
}
