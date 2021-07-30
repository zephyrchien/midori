use std::io;
use std::cmp::min;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use futures::ready;

use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncRead, AsyncWrite};
use quinn::{
    ClientConfigBuilder, Endpoint, NewConnection, CertificateChain, Connecting,
    ServerConfigBuilder,
};

use super::{AsyncConnect, AsyncAccept, IOStream, Transport};
use crate::utils::{self, CommonAddr, QUIC_BUF_SIZE};

