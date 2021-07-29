use std::io;
use std::sync::Arc;
use futures::try_join;

use crate::io::copy;
use crate::transport::plain::PlainStream;
use crate::transport::{AsyncConnect, AsyncAccept};

async fn bidi_copy<L, C>(
    base: L::Base,
    lis: Arc<L>,
    conn: Arc<C>,
) -> io::Result<()>
where
    L: AsyncAccept,
    C: AsyncConnect,
{
    let (sin, sout) = try_join!(lis.accept(base), conn.connect())?;
    let (ri, wi) = tokio::io::split(sin);
    let (ro, wo) = tokio::io::split(sout);
    let _ = try_join!(copy(ri, wo), copy(ro, wi));
    Ok(())
}

pub async fn proxy<L, C>(lis: Arc<L>, conn: Arc<C>) -> io::Result<()>
where
    L: AsyncAccept + 'static,
    C: AsyncConnect + 'static,
{
    loop {
        if let Ok((base, _)) = lis.accept_base().await {
            tokio::spawn(bidi_copy(base, lis.clone(), conn.clone()));
        }
    }
}

// zero copy
#[cfg(target_os = "linux")]
use crate::transport::plain;

#[cfg(target_os = "linux")]
async fn bidi_zero_copy(
    mut sin: PlainStream,
    conn: plain::Connector,
) -> io::Result<()> {
    use crate::io::zero_copy;
    let mut sout = conn.connect().await?;
    let (ri, wi) = plain::linux_ext::split(&mut sin);
    let (ro, wo) = plain::linux_ext::split(&mut sout);

    let _ = try_join!(zero_copy(ri, wo), zero_copy(ro, wi));

    Ok(())
}

#[cfg(target_os = "linux")]
pub async fn splice(
    lis: plain::Acceptor,
    conn: plain::Connector,
) -> io::Result<()> {
    let plain_lis = lis.inner();
    loop {
        if let Ok((sin, _)) = plain_lis.accept_plain().await {
            tokio::spawn(bidi_zero_copy(sin, conn.clone()));
        }
    }
}
