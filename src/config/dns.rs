use serde::{Serialize, Deserialize};
use trust_dns_resolver::config::{ResolverConfig,NameServerConfig,LookupIpStrategy,Protocol};
use std::net::IpAddr;
use crate::error::addr::{Result, AddrError};
use crate::utils::{must};
use std::net::SocketAddr;

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

fn default_dns_address() -> String {
    String::from("8.8.8.8")
}

fn default_dns_port() -> u16 {
    53
}

fn default_dns_protocol() -> String {
    String::from("udp")
}

fn default_trust_nx_responses() -> bool {
    true
}

fn default_dns_tls_servername() -> Option<String> {
    None
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DnsServerConfig {
    #[serde(default ="default_dns_address")]
    pub address: String,

    #[serde(default = "default_dns_port")]
    pub port: u16,

    #[serde(default = "default_dns_protocol")]
    pub protocol: String,

    #[serde(default = "default_trust_nx_responses")]
    pub trust_nx_responses: bool,

    #[serde(default = "default_dns_tls_servername")]
    pub tls_servername: Option<String>,
}

pub fn parse_ip_addr(
    addr: &str
) -> Result<(IpAddr, bool)> {
    if let Ok(sockaddr) = addr.parse::<IpAddr>() {
        return Ok((sockaddr, sockaddr.is_ipv6()));
    };
    Err(AddrError::Invalid(addr.to_string()))
}

impl DnsServerConfig {
    pub fn convert(&self) -> NameServerConfig {
        let (sockaddr, _) = must!(parse_ip_addr(&(self.address)));
        let protocol = match &(self.protocol)[..] {
            "tcp" => Protocol::Tcp,
            #[cfg(feature = "dns-over-tls")]
            "tls" => Protocol::Tls,
            #[cfg(feature = "dns-over-https")]
            "https" => Protocol::Https,
            #[cfg(feature = "mdns")]
            "mdns" => Protocol::Mdns,
            _ => Protocol::Udp,
            };
            NameServerConfig {
            socket_addr: SocketAddr::new(sockaddr, self.port),
            protocol: protocol,
            tls_dns_name: self.tls_servername.clone(),
            trust_nx_responses: self.trust_nx_responses,
            #[cfg(feature = "dns-over-rustls")]
            tls_config: None,
        }
    }
}

impl From<DnsServerConfig> for ResolverConfig {
    fn from(dns_server: DnsServerConfig) -> Self {
        let mut resolver_config = ResolverConfig::new();
        resolver_config.add_name_server(dns_server.convert());
        resolver_config
    }
}