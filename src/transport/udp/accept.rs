use std::io::{Result, Error, ErrorKind};
use std::time::Duration;
use std::net::SocketAddr;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use log::{trace, debug};
use bytes::Bytes;
use async_trait::async_trait;

use tokio::net::UdpSocket;
use tokio::time::sleep;
use tokio::sync::mpsc::{channel, Sender};

use super::UdpServerStream;
use crate::utils::{CommonAddr, UDP_BUF_SIZE, UDP_TIMEOUT};
use crate::transport::{AsyncAccept, Transport};

type Map = Arc<RwLock<HashMap<SocketAddr, Sender<Bytes>>>>;

pub struct Acceptor {
    io: Arc<UdpSocket>,
    addr: CommonAddr,
    records: Map,
}

impl Acceptor {
    pub fn new(io: UdpSocket, addr: CommonAddr) -> Self {
        Acceptor {
            io: Arc::new(io),
            addr,
            records: Arc::new(RwLock::new(HashMap::with_capacity(16))),
        }
    }
}

#[async_trait]
impl AsyncAccept for Acceptor {
    const TRANS: Transport = Transport::UDP;

    const SCHEME: &'static str = "udp";

    type IO = UdpServerStream;

    type Base = UdpServerStream;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        // this is safe cuz there is only one listener
        let mut buffer = vec![0u8; UDP_BUF_SIZE];
        let (n, peer_addr) = self.io.recv_from(&mut buffer).await?;
        let mut buffer = Bytes::from(buffer);
        buffer.truncate(n);

        // fake accept; send data to channel
        let send = self.records.read().unwrap().get(&peer_addr).cloned();
        if let Some(send) = send {
            trace!("udp send to channel");
            let _ = send.send(buffer).await;
            return Err(Error::new(ErrorKind::Other, "send data ok"));
        }

        // accept new stream
        debug!("udp accept {} <- {}", &self.addr, &peer_addr);
        let (send, recv) = channel::<Bytes>(4);
        let _ = send.send(buffer).await;
        self.records.write().unwrap().insert(peer_addr, send);
        tokio::spawn(udp_timeout(self.records.clone(), peer_addr));
        Ok((
            UdpServerStream::new(self.io.clone(), recv, peer_addr),
            peer_addr,
        ))
    }

    #[inline]
    async fn accept(&self, base: Self::Base) -> Result<Self::IO> { Ok(base) }
}

async fn udp_timeout(records: Map, peer_addr: SocketAddr) {
    sleep(Duration::from_secs(UDP_TIMEOUT)).await;
    // sender dropped
    debug!("udp timeout <- {}", &peer_addr);
    records.write().unwrap().remove(&peer_addr);
}
