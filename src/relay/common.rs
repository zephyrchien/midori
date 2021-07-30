use std::io;
use std::net::SocketAddr;

use crate::dns;
use crate::utils::{self, CommonAddr};

#[allow(dead_code)]
pub fn parse_domain_name(s: &str) -> Option<(String, u16)> {
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

#[allow(dead_code)]
pub fn parse_socket_addr(
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
