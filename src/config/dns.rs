use serde::{Serialize, Deserialize};
use trust_dns_resolver::config::LookupIpStrategy;
use trust_dns_resolver::config::{NameServerConfig,Protocol};
use std::net::ToSocketAddrs;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsMode {
    /// Only query for A (Ipv4) records
    Ipv4Only,
    /// Only query for AAAA (Ipv6) records
    Ipv6Only,
    /// Query for A and AAAA in parallel
    Ipv4AndIpv6,
    /// Query for Ipv4 if that fails, query for Ipv6 (default)
    Ipv4ThenIpv6,
    /// Query for Ipv6 if that fails, query for Ipv4
    Ipv6ThenIpv4,
}

impl Default for DnsMode {
    fn default() -> Self { Self::Ipv4ThenIpv6 }
}

impl From<DnsMode> for LookupIpStrategy {
    fn from(mode: DnsMode) -> Self {
        match mode {
            DnsMode::Ipv4Only => LookupIpStrategy::Ipv4Only,
            DnsMode::Ipv6Only => LookupIpStrategy::Ipv6Only,
            DnsMode::Ipv4AndIpv6 => LookupIpStrategy::Ipv4AndIpv6,
            DnsMode::Ipv4ThenIpv6 => LookupIpStrategy::Ipv4thenIpv6,
            DnsMode::Ipv6ThenIpv4 => LookupIpStrategy::Ipv6thenIpv4,
        }
    }
}

// default values
const fn def_true() -> bool { true }
fn default_protocol() -> String {String::from("udp")}
    
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DnsServerNode {
    
    addr: String,
    
    #[serde(default = "default_protocol")]
    protocol: String,
    
    #[serde(default = "def_true")]
    trust_nx_responses: bool
}

impl From<DnsServerNode> for NameServerConfig {
    fn from(dns_server_node: DnsServerNode) -> Self {
        let dns_server_socket = dns_server_node.addr.to_socket_addrs().unwrap().next().unwrap();

        let str_protocol = dns_server_node.protocol.as_str();
        let dest_protocol = match str_protocol {"tcp" => Protocol::Tcp , _ => Protocol::Udp,};
        let dest_trust_nx_responses = dns_server_node.trust_nx_responses;
        NameServerConfig {
            socket_addr: dns_server_socket,
            protocol: dest_protocol,
            tls_dns_name: None,
            trust_nx_responses: dest_trust_nx_responses,
        }
    }
}
