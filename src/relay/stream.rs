use std::io;
use std::sync::Arc;
use std::net::SocketAddr;
use futures::try_join;

use super::copy;
use crate::transport::plain::{self, PlainStream, PlainListener};
use crate::transport::{AsyncConnect, AsyncAccept};

async fn bidi_copy<S, C>(
    res: (PlainStream, SocketAddr),
    lis: Arc<S>,
    conn: Arc<C>,
) -> io::Result<()>
where
    S: AsyncAccept,
    C: AsyncConnect,
{
    let (sin, _) = lis.accept(res).await?;
    let sout = conn.connect().await?;
    let (ri, wi) = tokio::io::split(sin);
    let (ro, wo) = tokio::io::split(sout);
    let _ = try_join!(copy::copy(ri, wo), copy::copy(ro, wi));
    Ok(())
}

pub async fn proxy<L, C>(lis: L, conn: C) -> io::Result<()>
where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    let lis = Arc::new(lis);
    let conn = Arc::new(conn);
    let plain_lis = PlainListener::bind(lis.addr()).unwrap();
    loop {
        if let Ok(res) = plain_lis.accept_plain().await {
            tokio::spawn(bidi_copy(res, lis.clone(), conn.clone()));
        }
    }
}

// zero copy
#[cfg(target_os = "linux")]
use super::zero_copy;

#[cfg(target_os = "linux")]
async fn bidi_zero_copy(
    mut sin: PlainStream,
    conn: plain::Connector,
) -> io::Result<()> {
    let mut sout = conn.connect().await?;
    let (mut ri, mut wi) = sin.split();
    let (mut ro, mut wo) = sout.split();

    let _ = try_join!(
        zero_copy::copy(&mut ri, &mut wo),
        zero_copy::copy(&mut ro, &mut wi)
    );

    Ok(())
}

#[cfg(target_os = "linux")]
pub async fn splice(
    lis: plain::Acceptor,
    conn: plain::Connector,
) -> io::Result<()> {
    let plain_lis = PlainListener::bind(lis.addr()).unwrap();
    loop {
        if let Ok((sin, _)) = plain_lis.accept_plain().await {
            tokio::spawn(bidi_zero_copy(sin, conn.clone()));
        }
    }
}
