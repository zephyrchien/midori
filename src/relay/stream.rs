use std::io;
use std::sync::Arc;
use futures::try_join;

use tokio::io::{AsyncRead, AsyncWrite};

use super::copy;
use crate::transport::plain::{self, PlainStream};
use crate::transport::{AsyncConnect, AsyncAccept};

async fn bidi_copy<S, C>(sin: S, conn: Arc<C>) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
    C: AsyncConnect,
{
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
    let conn = Arc::new(conn);
    loop {
        if let Ok((sin, _)) = lis.accept().await {
            tokio::spawn(bidi_copy(sin, conn.clone()));
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
    loop {
        if let Ok((sin, _)) = lis.accept().await {
            tokio::spawn(bidi_zero_copy(sin, conn.clone()));
        }
    }
}
