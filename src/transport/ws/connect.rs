use std::io::{Error, ErrorKind, Result};

use log::debug;
use http::Uri;
use async_trait::async_trait;

use tokio_tungstenite::tungstenite;
use tungstenite::protocol::WebSocketConfig;

use super::WebSocketStream;
use crate::transport::{AsyncConnect, Transport};

use crate::utils::CommonAddr;

pub struct Connector<T: AsyncConnect> {
    cc: T,
    uri: Uri,
    config: Option<WebSocketConfig>,
}

impl<T: AsyncConnect> Connector<T> {
    pub fn new(cc: T, path: String) -> Self {
        let authority = cc.addr().to_string();
        Connector {
            cc,
            uri: Uri::builder()
                .scheme(Self::SCHEME)
                .authority(authority.as_str())
                .path_and_query(path)
                .build()
                .unwrap(),
            config: None,
        }
    }
}

#[async_trait]
impl<T: AsyncConnect> AsyncConnect for Connector<T> {
    const TRANS: Transport = Transport::WS;

    const SCHEME: &'static str = match T::TRANS {
        Transport::TLS => "wss",
        _ => "ws",
    };

    type IO = WebSocketStream<T::IO>;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.cc.addr() }

    fn clear_reuse(&self) {}

    async fn connect(&self) -> Result<Self::IO> {
        let stream = self.cc.connect().await?;
        debug!("ws connect ->");
        tokio_tungstenite::client_async_with_config(
            &self.uri,
            stream,
            self.config,
        )
        .await
        .map_or_else(
            |e| Err(Error::new(ErrorKind::ConnectionRefused, e)),
            |(ws, _)| Ok(WebSocketStream::new(ws)),
        )
    }
}
