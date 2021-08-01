use std::io;
use std::net::SocketAddr;

use crate::dns;
use crate::utils::{self, CommonAddr};

#[allow(dead_code)]
pub fn parse_domain_name(s: &str) -> Option<((String, u16), bool)> {
    let mut iter = s.splitn(2, ':');
    let addr = iter.next()?.to_string();
    let port = iter.next()?.parse::<u16>().ok()?;
    // check addr
    if let Ok(ip) = dns::resolve_sync(&addr) {
        return Some(((addr, port), ip.is_ipv6()));
    }
    None
}

#[allow(dead_code)]
pub fn parse_socket_addr(
    addr: &str,
    allow_domain_name: bool,
) -> io::Result<(CommonAddr, bool)> {
    if let Ok(sockaddr) = addr.parse::<SocketAddr>() {
        return Ok((CommonAddr::SocketAddr(sockaddr), sockaddr.is_ipv6()));
    };
    if allow_domain_name {
        if let Some(((addr, port), is_ipv6)) = parse_domain_name(addr) {
            return Ok((CommonAddr::DomainName(addr, port), is_ipv6));
        }
    };
    Err(utils::new_io_err("unable to parse socket addr"))
}
