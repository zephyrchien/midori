use std::fs;
use std::io::{BufReader, Read};
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use webpki::DNSNameRef;
use rustls::{ClientConfig, ServerConfig, NoClientAuth};
use rustls::internal::msgs::enums::ProtocolVersion;

use crate::utils::{self, must, CommonAddr, NATIVE_CERTS, NOT_A_DNS_NAME};
use crate::transport::tls;
use crate::transport::{AsyncConnect, AsyncAccept};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TLSConfig {
    None,
    Client(TLSClientConfig),
    Server(TLSServerConfig),
}

impl Default for TLSConfig {
    fn default() -> Self { Self::None }
}

// create default values
fn def_true() -> bool { true }

fn def_roots_str() -> String { "firefox".to_string() }

// TLS Client
#[derive(Debug, Serialize, Deserialize)]
pub struct TLSClientConfig {
    pub skip_verify: bool,

    #[serde(default = "def_true")]
    pub enable_sni: bool,

    #[serde(default)]
    pub enable_early_data: bool,

    #[serde(default)]
    pub sni: String,

    #[serde(default)]
    pub alpns: Vec<String>,

    // tlsv1.2, tlsv1.3
    #[serde(default)]
    pub versions: Vec<String>,

    // native, firefox, or provide a file
    #[serde(default = "def_roots_str")]
    pub roots: String,
}

struct ClientSkipVerify;
impl rustls::ServerCertVerifier for ClientSkipVerify {
    fn verify_server_cert(
        &self,
        _: &rustls::RootCertStore,
        _: &[rustls::Certificate],
        _: webpki::DNSNameRef<'_>,
        _: &[u8],
    ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
        Ok(rustls::ServerCertVerified::assertion())
    }
}

impl TLSClientConfig {
    pub fn to_tls(&self) -> ClientConfig { make_client_config(self) }

    pub fn set_sni(
        &self,
        tlsc: &mut ClientConfig,
        addr: &CommonAddr,
    ) -> String {
        if !self.sni.is_empty() {
            return self.sni.clone();
        };
        let sni = addr.to_dns_name();
        if !sni.is_empty() {
            return sni;
        };
        tlsc.enable_sni = false;
        String::from(NOT_A_DNS_NAME)
    }

    pub fn apply_to_conn<C: AsyncConnect>(&self, conn: C) -> impl AsyncConnect {
        let mut tlsc = make_client_config(self);
        let sni = self.set_sni(&mut tlsc, conn.addr());
        let sni = DNSNameRef::try_from_ascii_str(&sni).unwrap().to_owned();
        tls::Connector::new(conn, sni, tlsc)
    }
}

fn make_client_config(config: &TLSClientConfig) -> ClientConfig {
    let mut tlsc = ClientConfig::new();
    tlsc.enable_sni = config.enable_sni;
    tlsc.enable_early_data = config.enable_early_data;
    // if not specified, use the constructor's default value
    if !config.alpns.is_empty() {
        tlsc.alpn_protocols =
            config.alpns.iter().map(|x| x.as_bytes().to_vec()).collect();
    };
    // the same as alpns
    if !config.versions.is_empty() {
        tlsc.versions = config
            .versions
            .iter()
            .map(|x| match x.as_str() {
                "tlsv1.2" => ProtocolVersion::TLSv1_2,
                "tlsv1.3" => ProtocolVersion::TLSv1_3,
                _ => panic!("unknown ssl version"),
            })
            .collect();
    };
    // skip verify
    if config.skip_verify {
        tlsc.dangerous()
            .set_certificate_verifier(Arc::new(ClientSkipVerify));
        return tlsc;
    };
    // configure the validator
    match config.roots.as_str() {
        "native" => tlsc.root_store = NATIVE_CERTS.clone(),

        "firefox" => tlsc
            .root_store
            .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS),

        file_path => {
            tlsc.root_store
                .add_pem_file(&mut BufReader::new(must!(
                    fs::File::open(file_path),
                    "open {}",
                    file_path
                )))
                .unwrap();
        }
    };
    tlsc
}

// TLS Server
#[derive(Debug, Serialize, Deserialize)]
pub struct TLSServerConfig {
    pub cert: String,

    pub key: String,

    #[serde(default)]
    pub alpns: Vec<String>,

    #[serde(default)]
    pub versions: Vec<String>,

    #[serde(default)]
    pub ocsp: String,
}

use crate::utils::MaybeQuic;

impl TLSServerConfig {
    pub fn to_tls(&self) -> ServerConfig { make_server_config(self) }

    pub fn apply_to_lis<L: AsyncAccept>(&self, lis: L) -> impl AsyncAccept {
        let config = make_server_config(self);
        tls::Acceptor::new(lis, config)
    }

    pub fn apply_to_lis_ext<L: AsyncAccept>(
        &self,
        lis: MaybeQuic<L>,
    ) -> MaybeQuic<impl AsyncAccept> {
        match lis {
            MaybeQuic::Quic(x) => MaybeQuic::Quic(x),
            MaybeQuic::Other(x) => MaybeQuic::Other(self.apply_to_lis(x)),
        }
    }
}

fn make_server_config(config: &TLSServerConfig) -> ServerConfig {
    let mut tlsc = ServerConfig::new(NoClientAuth::new());
    // if not specified, use the constructor's default value
    if !config.alpns.is_empty() {
        tlsc.alpn_protocols =
            config.alpns.iter().map(|x| x.as_bytes().to_vec()).collect();
    };
    // the same as alpns
    if !config.versions.is_empty() {
        tlsc.versions = config
            .versions
            .iter()
            .map(|x| match x.as_str() {
                "tlsv1.2" => ProtocolVersion::TLSv1_2,
                "tlsv1.3" => ProtocolVersion::TLSv1_3,
                _ => panic!("unknown ssl version"),
            })
            .collect();
    };
    let (certs, key) = if config.cert == config.key {
        must!(utils::generate_cert_key(&config.cert))
    } else {
        let certs =
            must!(utils::load_certs(&config.cert), "load {}", &config.cert);
        let mut keys =
            must!(utils::load_keys(&config.key), "load {}", &config.key);
        (certs, keys.remove(0))
    };
    let mut ocsp = vec![0u8];
    if !config.ocsp.is_empty() {
        ocsp.reserve(utils::OCSP_BUF_SIZE);
        let mut r = BufReader::new(must!(
            fs::File::open(&config.ocsp),
            "open {}",
            &config.ocsp
        ));
        must!(r.read_to_end(&mut ocsp), "load {}", &config.ocsp);
    }
    must!(tlsc.set_single_cert_with_ocsp_and_sct(certs, key, ocsp, Vec::new()));
    tlsc
}
