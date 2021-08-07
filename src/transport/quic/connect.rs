use std::io::{Result, Error, ErrorKind};
use std::net::SocketAddr;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use log::{debug, trace};
use async_trait::async_trait;
use quinn::crypto::rustls::TlsSession;
use quinn::generic::Connection;
use quinn::{Endpoint, NewConnection};

use super::QuicStream;
use crate::dns;
use crate::utils::CommonAddr;
use crate::transport::{AsyncConnect, Transport};

pub struct Connector {
    cc: Endpoint,
    addr: CommonAddr,
    sni: String,
    max_concurrent: usize,
    count: AtomicUsize,
    channel: RwLock<Option<Connection<TlsSession>>>,
}

impl Connector {
    pub fn new(
        cc: Endpoint,
        addr: CommonAddr,
        sni: String,
        max_concurrent: usize,
    ) -> Self {
        let max_concurrent = if max_concurrent == 0 || max_concurrent > 100 {
            100
        } else {
            max_concurrent
        };
        Connector {
            cc,
            addr,
            sni,
            max_concurrent,
            count: AtomicUsize::new(1),
            channel: RwLock::new(None),
        }
    }
}

#[async_trait]
impl AsyncConnect for Connector {
    const TRANS: Transport = Transport::QUIC;

    const SCHEME: &'static str = "quic";

    type IO = QuicStream;

    #[inline]
    fn addr(&self) -> &CommonAddr { &self.addr }

    #[inline]
    fn clear_reuse(&self) { *self.channel.write().unwrap() = None; }

    async fn connect(&self) -> Result<Self::IO> {
        let client = new_client(self).await?;
        let (send, recv) = client.open_bi().await?;
        Ok(QuicStream::new(send, recv))
    }
}

async fn new_client(cc: &Connector) -> Result<Connection<TlsSession>> {
    // reuse existed connection
    trace!("quic init new client");
    let channel = (*cc.channel.read().unwrap()).clone();
    if let Some(client) = channel {
        let count = cc.count.load(Ordering::Relaxed);
        trace!("quic reusable, current mux = {}", count);
        if count < cc.max_concurrent {
            debug!("quic connect[reuse {}] -> {}", count, &cc.addr);
            cc.count.fetch_add(1, Ordering::Relaxed);
            return Ok(client);
        };
    };

    // establish a new connection
    let connect_addr = match &cc.addr {
        CommonAddr::SocketAddr(sockaddr) => *sockaddr,
        CommonAddr::DomainName(addr, port) => {
            let ip = dns::resolve_async(addr).await?;
            SocketAddr::new(ip, *port)
        }
        #[cfg(all(unix, feature = "uds"))]
        CommonAddr::UnixSocketPath(_) => unreachable!(),
    };

    debug!("quic connect[new] -> {}", &cc.addr);
    let connecting = cc
        .cc
        .connect(&connect_addr, &cc.sni)
        .map_err(|e| Error::new(ErrorKind::ConnectionRefused, e))?;

    // early data
    let new_conn = match connecting.into_0rtt() {
        Ok((new_conn, zero_rtt)) => {
            zero_rtt.await;
            new_conn
        }
        Err(connecting) => connecting.await?,
    };

    let NewConnection {
        connection: client, ..
    } = new_conn;

    // store connection
    // may have conflicts
    cc.count.store(1, Ordering::Relaxed);
    *cc.channel.write().unwrap() = Some(client.clone());
    Ok(client)
}
