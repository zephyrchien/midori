use std::io;
use std::sync::Arc;
use futures::try_join;

use log::{warn, info};
use tokio::io::{AsyncRead, AsyncWrite};
use crate::transport::{AsyncConnect, AsyncAccept};

mod copy;
pub use copy::copy;

pub async fn bidi_copy<L, C>(base: L::Base, lis: Arc<L>, conn: Arc<C>)
where
    L: AsyncAccept,
    C: AsyncConnect,
{
    let (sin, sout) = match try_join!(lis.accept(base), conn.connect()) {
        Ok((sin, sout)) => (sin, sout),
        Err(e) => {
            warn!("protocol level handshake error: {}", e);
            return;
        }
    };
    let (ri, wi) = tokio::io::split(sin);
    let (ro, wo) = tokio::io::split(sout);
    let res = try_join!(copy(ri, wo), copy(ro, wi));
    if let Err(e) = res {
        warn!("forwarding finished, {}", e);
    }
}

// this is only used by protocols that impl multiplex
#[cfg(any(feature = "h2c", feature = "quic"))]
pub async fn bidi_copy_with_stream<C, S>(cc: Arc<C>, sin: S)
where
    C: AsyncConnect + 'static,
    S: AsyncRead + AsyncWrite,
{
    let sout = match cc.connect().await {
        Ok(sout) => sout,
        Err(e) => {
            warn!("protocol level handshake error: {}", e);
            return;
        }
    };
    let (ri, wi) = tokio::io::split(sin);
    let (ro, wo) = tokio::io::split(sout);
    let res = try_join!(copy(ri, wo), copy(ro, wi));
    if let Err(e) = res {
        warn!("forwarding finished, {}", e);
    }
}

pub async fn proxy<L, C>(lis: Arc<L>, conn: Arc<C>) -> io::Result<()>
where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    loop {
        match lis.accept_base().await {
            Ok((base, addr)) => {
                info!(
                    "{}[{}] <-> {}[{}]",
                    &addr,
                    L::SCHEME,
                    conn.addr(),
                    C::SCHEME
                );
                tokio::spawn(bidi_copy(base, lis.clone(), conn.clone()));
            }
            Err(e) => warn!("failed to accept[{}]: {}", L::SCHEME, e),
        }
    }
}

// zero copy
#[cfg(target_os = "linux")]
mod zero_copy;

#[cfg(target_os = "linux")]
pub mod linux_ext {
    use super::*;
    use zero_copy::zero_copy;
    use crate::transport::plain;
    pub async fn bidi_zero_copy(
        mut sin: plain::PlainStream,
        conn: plain::Connector,
    ) {
        let mut sout = match conn.connect_plain().await {
            Ok(sout) => sout,
            Err(e) => {
                warn!("protocol level handshake error: {}", e);
                return;
            }
        };
        let (ri, wi) = plain::linux_ext::split(&mut sin);
        let (ro, wo) = plain::linux_ext::split(&mut sout);

        let res = try_join!(zero_copy(ri, wo), zero_copy(ro, wi));
        if let Err(e) = res {
            warn!("forwarding finished, {}", e);
        }
    }

    pub async fn splice(
        lis: plain::Acceptor,
        conn: plain::Connector,
    ) -> io::Result<()> {
        let plain_lis = lis.inner();
        loop {
            match plain_lis.accept_plain().await {
                Ok((sin, addr)) => {
                    info!("{}[raw] <-> {}[raw]", &addr, conn.addr());
                    tokio::spawn(bidi_zero_copy(sin, conn.clone()));
                }
                Err(e) => warn!("failed to accept[raw]: {}", e),
            }
        }
    }
}
