use std::io;
use std::net::SocketAddr;
use futures::try_join;

use super::copy;
use crate::transport::plain::{PlainStream, PlainListener};
use crate::transport::{AsyncConnect, AsyncAccept};

async fn bidi_copy<S, C>(
    res: (PlainStream, SocketAddr),
    lis: S,
    conn: C,
) -> io::Result<()>
where
    S: AsyncAccept,
    C: AsyncConnect,
{
    let ((sin, _), sout) = try_join!(lis.accept(res), conn.connect())?;
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
    let plain_lis = PlainListener::bind(lis.addr()).unwrap();
    loop {
        if let Ok(res) = plain_lis.accept_plain().await {
            tokio::spawn(bidi_copy(res, lis.clone(), conn.clone()));
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
    use super::zero_copy;
    let mut sout = conn.connect().await?;
    let (ri, wi) = plain::linux_ext::split(&mut sin);
    let (ro, wo) = plain::linux_ext::split(&mut sout);

    let _ = try_join!(zero_copy::copy(ri, wo), zero_copy::copy(ro, wi));

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
