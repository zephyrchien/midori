use std::io::{Result, Error, ErrorKind};
use std::net::IpAddr;

use crate::config::dns::DnsServerConfig;

use futures::executor;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts, LookupIpStrategy};
use lazy_static::lazy_static;

static mut RESOLVE_STRATEGY: LookupIpStrategy = LookupIpStrategy::Ipv4thenIpv6;
static mut DNS_SERVERS: Vec<DnsServerConfig> = vec![];

lazy_static! {
    static ref DNS: TokioAsyncResolver = {
        let mut temp_resolver_config : ResolverConfig = ResolverConfig::new();
        unsafe { 
            for dns_server in &DNS_SERVERS {
                temp_resolver_config.add_name_server(dns_server.convert());
            }
        }
        TokioAsyncResolver::tokio(
        temp_resolver_config,
        ResolverOpts {
            ip_strategy: unsafe { RESOLVE_STRATEGY },
            ..Default::default()
        }
    )
    .unwrap()};
}

pub fn init_resolver(strategy: LookupIpStrategy,dns_server_configs: Vec<DnsServerConfig>) {
    unsafe { RESOLVE_STRATEGY = strategy };
    
    unsafe { DNS_SERVERS = dns_server_configs};
    lazy_static::initialize(&DNS);
}

pub fn resolve_sync(addr: &str) -> Result<IpAddr> {
    executor::block_on(resolve_async(addr))
}

pub async fn resolve_async(addr: &str) -> Result<IpAddr> {
    let res = DNS
        .lookup_ip(addr)
        .await
        .map_err(|e| Error::new(ErrorKind::Other, e))?
        .into_iter()
        .next()
        .unwrap();
    Ok(res)
}
