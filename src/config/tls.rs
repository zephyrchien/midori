use std::fs;
use std::io::{BufReader, Read};
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use webpki::DNSNameRef;
use rustls::{ClientConfig, ServerConfig, NoClientAuth};
use rustls::internal::msgs::enums::ProtocolVersion;

use crate::utils::{self, NATIVE_CERTS, NOT_A_DNS_NAME};
use crate::transport::tls;
use crate::transport::{AsyncConnect, AsyncAccept};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TLSConfig {
    None,
    Client(TLSClientConfig),
    Server(TLSServerConfig),
}

// create default values
fn def_true() -> bool { true }

fn def_empty_str() -> String { String::new() }

fn def_empty_vec() -> Vec<String> { Vec::new() }

fn def_roots_str() -> String { "firefox".to_string() }

// TLS Client
#[derive(Debug, Serialize, Deserialize)]
pub struct TLSClientConfig {
    pub skip_verify: bool,
    #[serde(default = "def_true")]
    pub enable_sni: bool,
    #[serde(default)]
    pub enable_early_data: bool,
    #[serde(default = "def_empty_str")]
    pub sni: String,
    #[serde(default = "def_empty_vec")]
    pub alpns: Vec<String>,
    // tlsv1.2, tlsv1.3
    #[serde(default = "def_empty_vec")]
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
                .add_pem_file(&mut BufReader::new(
                    fs::File::open(file_path).expect("invalid cert file"),
                ))
                .unwrap();
        }
    };
    tlsc
}

impl TLSClientConfig {
    pub fn apply_to_conn<C: AsyncConnect>(&self, conn: C) -> impl AsyncConnect {
        let mut config = make_client_config(self);
        let sni = DNSNameRef::try_from_ascii_str(&{
            if !self.sni.is_empty() {
                self.sni.clone()
            } else {
                let s = conn.addr().to_dns_name();
                if s.is_empty() {
                    config.enable_sni = false;
                    String::from(NOT_A_DNS_NAME)
                } else {
                    s
                }
            }
        })
        .unwrap()
        .to_owned();
        tls::Connector::new(conn, sni, config)
    }
}

// TLS Server
#[derive(Debug, Serialize, Deserialize)]
pub struct TLSServerConfig {
    #[serde(default = "def_empty_vec")]
    pub alpns: Vec<String>,
    #[serde(default = "def_empty_vec")]
    pub versions: Vec<String>,
    pub cert: String,
    pub key: String,
    #[serde(default)]
    pub ocsp: String,
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
        utils::generate_cert_key(&config.cert).unwrap()
    } else {
        let certs = utils::load_certs(&config.cert).unwrap();
        let mut keys = utils::load_keys(&config.key).unwrap();
        (certs, keys.remove(0))
    };
    let mut ocsp = vec![0u8];
    if !config.ocsp.is_empty() {
        ocsp.reserve(utils::OCSP_BUF_SIZE);
        let mut r = BufReader::new(
            fs::File::open(&config.ocsp).expect("invalid ocsp file"),
        );
        r.read_to_end(&mut ocsp).unwrap();
    }
    tlsc.set_single_cert_with_ocsp_and_sct(certs, key, ocsp, Vec::new())
        .unwrap();
    tlsc
}

impl TLSServerConfig {
    pub fn apply_to_lis<L: AsyncAccept>(&self, lis: L) -> impl AsyncAccept {
        let config = make_server_config(self);
        tls::Acceptor::new(lis, config)
    }
}
