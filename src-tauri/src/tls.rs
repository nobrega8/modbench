//! Modbus/TCP Security: Modbus TCP inside TLS 1.2 or later, usually on port
//! 802, with a certificate on each side (the Modbus Organization's
//! "Modbus/TCP Security Protocol Specification"). The client checks the
//! device's certificate against a CA, or trusts any when no CA is given, and
//! shows its own certificate to devices that ask for one. The simulator can
//! answer over it too.
//!
//! Certificates and keys are read from PEM files; keys are never logged.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, ring, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::server::WebPkiClientVerifier;
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, ServerConfig, SignatureScheme};
use serde::{Deserialize, Serialize};
use tokio_rustls::{TlsAcceptor, TlsConnector};

use crate::modbus::ModbusError;

/// The port Modbus/TCP Security uses.
pub const PORT: u16 = 802;

/// How a client talks TLS to a device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlsConfig {
    /// CA certificate (PEM) the device's certificate must be signed by; None
    /// trusts any certificate the device shows.
    #[serde(default)]
    pub ca: Option<String>,
    /// This client's certificate and private key (PEM), for devices that ask
    /// for one, as the spec's mutual authentication does.
    #[serde(default)]
    pub cert: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(ring::default_provider())
}

fn invalid(text: String) -> ModbusError {
    ModbusError::Invalid(text)
}

fn certs(path: &str) -> Result<Vec<CertificateDer<'static>>, ModbusError> {
    let list = CertificateDer::pem_file_iter(path)
        .map_err(|e| invalid(format!("{path}: {e}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| invalid(format!("{path}: {e}")))?;
    if list.is_empty() {
        return Err(invalid(format!("{path}: no certificate in the file")));
    }
    Ok(list)
}

fn key(path: &str) -> Result<PrivateKeyDer<'static>, ModbusError> {
    PrivateKeyDer::from_pem_file(path).map_err(|e| invalid(format!("{path}: no private key ({e})")))
}

fn roots(path: &str) -> Result<RootCertStore, ModbusError> {
    let mut store = RootCertStore::empty();
    for cert in certs(path)? {
        store
            .add(cert)
            .map_err(|e| invalid(format!("{path}: {e}")))?;
    }
    Ok(store)
}

/// What opens TLS to a device, from `config`.
pub fn connector(config: &TlsConfig) -> Result<TlsConnector, ModbusError> {
    let tls = |e: rustls::Error| invalid(format!("TLS: {e}"));
    let builder = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(tls)?;
    let builder = match &config.ca {
        Some(ca) => builder.with_root_certificates(roots(ca)?),
        None => builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AnyServer(provider()))),
    };
    let client = match (&config.cert, &config.key) {
        (Some(cert), Some(k)) => builder
            .with_client_auth_cert(certs(cert)?, key(k)?)
            .map_err(tls)?,
        (None, None) => builder.with_no_client_auth(),
        _ => {
            return Err(invalid(
                "a client certificate needs its private key, and a key its certificate".into(),
            ));
        }
    };
    Ok(TlsConnector::from(Arc::new(client)))
}

/// The name the device's certificate must carry: a host name or an address.
pub fn server_name(host: &str) -> Result<ServerName<'static>, ModbusError> {
    ServerName::try_from(host.to_string()).map_err(|e| invalid(format!("{host}: {e}")))
}

/// Why a TLS handshake failed, in words: rustls says what it did not like
/// about the certificates.
pub fn handshake_error(e: std::io::Error) -> ModbusError {
    invalid(format!("TLS handshake failed: {e}"))
}

/// What answers TLS as a device (the simulator): its certificate and key,
/// and the CA a client's certificate must be signed by, if clients must show
/// one.
pub fn acceptor(
    cert: &str,
    key_path: &str,
    client_ca: Option<&str>,
) -> Result<TlsAcceptor, ModbusError> {
    let tls = |e: rustls::Error| invalid(format!("TLS: {e}"));
    let builder = ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(tls)?;
    let builder = match client_ca {
        Some(ca) => {
            let verifier =
                WebPkiClientVerifier::builder_with_provider(Arc::new(roots(ca)?), provider())
                    .build()
                    .map_err(|e| invalid(format!("{ca}: {e}")))?;
            builder.with_client_cert_verifier(verifier)
        }
        None => builder.with_no_client_auth(),
    };
    let server = builder
        .with_single_cert(certs(cert)?, key(key_path)?)
        .map_err(tls)?;
    Ok(TlsAcceptor::from(Arc::new(server)))
}

/// Trusts whatever certificate a device shows (no CA given), while still
/// checking that the device holds its key.
#[derive(Debug)]
struct AnyServer(Arc<CryptoProvider>);

impl ServerCertVerifier for AnyServer {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modbus::{ConnectionConfig, RawData, Session, Transport};
    use crate::planner::Area;
    use crate::simulator::{self, Device, Memory};

    fn file(name: &str) -> String {
        format!("{}/tests/tls/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    /// A simulated device over TLS that asks clients for a certificate from
    /// the test CA; holding register 0 is 42.
    async fn device(client_ca: bool) -> simulator::Running {
        let mut memory = Memory::default();
        memory
            .set(Area::Holding, 0, &RawData::Registers(vec![42]))
            .unwrap();
        let devices = Arc::new(std::sync::Mutex::new(vec![Device::new(Some(1), memory)]));
        let ca = file("ca.pem");
        let acceptor = acceptor(
            &file("server.pem"),
            &file("server.key"),
            client_ca.then_some(ca.as_str()),
        )
        .unwrap();
        let listen = Transport::Tcp {
            host: "127.0.0.1".into(),
            port: 0,
        };
        simulator::start_tls(listen, Some(acceptor), devices, None)
            .await
            .unwrap()
    }

    fn config(port: u16, tls: TlsConfig) -> ConnectionConfig {
        ConnectionConfig {
            transport: Transport::Tcp {
                host: "127.0.0.1".into(),
                port,
            },
            unit_id: 1,
            timeout_ms: 2000,
            local_address: None,
            tls: Some(tls),
        }
    }

    fn mutual() -> TlsConfig {
        TlsConfig {
            ca: Some(file("ca.pem")),
            cert: Some(file("client.pem")),
            key: Some(file("client.key")),
        }
    }

    #[tokio::test]
    async fn reads_and_writes_with_mutual_authentication() {
        let sim = device(true).await;
        let mut s = Session::connect(&config(sim.port().unwrap(), mutual()), None)
            .await
            .unwrap();
        assert_eq!(
            s.read(Area::Holding, 0, 1).await.unwrap(),
            RawData::Registers(vec![42])
        );
        s.write_registers(1, &[7]).await.unwrap();
        assert_eq!(
            s.read(Area::Holding, 1, 1).await.unwrap(),
            RawData::Registers(vec![7])
        );
    }

    #[tokio::test]
    async fn a_client_without_a_certificate_is_turned_away() {
        let sim = device(true).await;
        let tls = TlsConfig {
            ca: Some(file("ca.pem")),
            ..TlsConfig::default()
        };
        // TLS 1.3 settles client certificates after the handshake: the
        // refusal shows on the first request at the latest.
        let refused = match Session::connect(&config(sim.port().unwrap(), tls), None).await {
            Err(_) => true,
            Ok(mut s) => s.read(Area::Holding, 0, 1).await.is_err(),
        };
        assert!(refused);
    }

    #[tokio::test]
    async fn a_device_certificate_from_another_ca_fails_clearly() {
        let sim = device(false).await;
        let tls = TlsConfig {
            ca: Some(file("other-ca.pem")),
            ..TlsConfig::default()
        };
        let err = Session::connect(&config(sim.port().unwrap(), tls), None)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(err.contains("TLS handshake failed"), "{err}");
        assert!(err.to_lowercase().contains("certificate"), "{err}");
    }

    #[tokio::test]
    async fn without_a_ca_any_device_certificate_is_trusted() {
        let sim = device(false).await;
        let mut s = Session::connect(&config(sim.port().unwrap(), TlsConfig::default()), None)
            .await
            .unwrap();
        assert_eq!(
            s.read(Area::Holding, 0, 1).await.unwrap(),
            RawData::Registers(vec![42])
        );
    }

    #[test]
    fn missing_files_and_half_pairs_are_explained() {
        let missing = TlsConfig {
            ca: Some(file("nope.pem")),
            ..TlsConfig::default()
        };
        assert!(connector(&missing).is_err());
        let half = TlsConfig {
            cert: Some(file("client.pem")),
            ..TlsConfig::default()
        };
        let err = connector(&half).err().unwrap().to_string();
        assert!(err.contains("private key"), "{err}");
    }

    #[tokio::test]
    async fn tls_goes_with_modbus_tcp_only() {
        let mut c = config(1, mutual());
        c.transport = Transport::Udp {
            host: "127.0.0.1".into(),
            port: 1,
        };
        let err = Session::connect(&c, None).await.err().unwrap().to_string();
        assert!(err.contains("Modbus TCP only"), "{err}");
    }
}
