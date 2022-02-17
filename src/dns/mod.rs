use std::io::{Result, Error, ErrorKind};
use std::net::{IpAddr};

use futures::executor;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts, LookupIpStrategy,NameServerConfig};
use lazy_static::lazy_static;
use log::{debug};

use super::config::dns::{DnsServerNode};


static mut RESOLVE_STRATEGY: LookupIpStrategy = LookupIpStrategy::Ipv4thenIpv6;
static mut RESOLVER_CONFIGS: Vec<ResolverConfig> = vec![];

lazy_static! {
    static ref DNS: TokioAsyncResolver = {
        let resolver_config = {
            unsafe{
                RESOLVER_CONFIGS.pop().unwrap()
            }
        };
        TokioAsyncResolver::tokio(
            resolver_config,
            ResolverOpts {
                ip_strategy: unsafe { RESOLVE_STRATEGY },
                ..Default::default()
            }
        )
    }.unwrap();
}

pub fn init_resolver(strategy: LookupIpStrategy,dns_servers: Vec<DnsServerNode>) {
    
    let mut resolver_config = ResolverConfig::new();
    if dns_servers.is_empty() {
        resolver_config = ResolverConfig::cloudflare();
    }else{
        for dns_server in dns_servers.into_iter() {
            debug!("load next dns_server");
            resolver_config.add_name_server(NameServerConfig::from(dns_server));
        }
    }
    unsafe { 
        RESOLVE_STRATEGY = strategy;
        RESOLVER_CONFIGS.push(resolver_config);
    };
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
