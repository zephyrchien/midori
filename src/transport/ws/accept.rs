use std::io::{Error, ErrorKind, Result};
use std::net::SocketAddr;

use log::debug;
use http::StatusCode;
use async_trait::async_trait;

use tokio_tungstenite::tungstenite;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::handshake::server::{Callback, Request, Response, ErrorResponse};

use super::WebSocketStream;
use crate::transport::{AsyncAccept, Transport};
use crate::utils::CommonAddr;

pub struct Acceptor<T: AsyncAccept> {
    lis: T,
    path: String,
    config: Option<WebSocketConfig>,
}

impl<T: AsyncAccept> Acceptor<T> {
    pub fn new(lis: T, path: String) -> Self {
        Acceptor {
            lis,
            path,
            config: None,
        }
    }
}

struct RequestHook {
    path: String,
}

impl Callback for RequestHook {
    fn on_request(
        self,
        request: &Request,
        response: Response,
    ) -> std::result::Result<Response, ErrorResponse> {
        if request.uri().path() == self.path {
            debug!("check request path -- ok");
            Ok(response)
        } else {
            debug!("check request path -- not found");
            let mut response = ErrorResponse::new(None);
            *response.status_mut() = StatusCode::NOT_FOUND;
            Err(response)
        }
    }
}

#[async_trait]
impl<T: AsyncAccept> AsyncAccept for Acceptor<T> {
    const TRANS: Transport = Transport::WS;

    const SCHEME: &'static str = match T::TRANS {
        Transport::TLS => "wss",
        _ => "ws",
    };

    type IO = WebSocketStream<T::IO>;

    type Base = T::Base;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.lis.addr() }

    #[inline]
    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        self.lis.accept_base().await
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> {
        let stream = self.lis.accept(base).await?;
        debug!("ws accept <-");

        let hook = RequestHook {
            path: self.path.clone(),
        };
        tokio_tungstenite::accept_hdr_async_with_config(
            stream,
            hook,
            self.config,
        )
        .await
        .map_or_else(
            |e| Err(Error::new(ErrorKind::ConnectionAborted, e)),
            |ws| Ok(WebSocketStream::new(ws)),
        )
    }
}
