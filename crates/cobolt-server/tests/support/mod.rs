// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Test support for the loopback end-to-end tests: a certificate authority of
//! one (a self-signed certificate made with RustCrypto's `p256` — no `ring`, no
//! `aws-lc-rs`), an identity provider of one (an issuer key, its JWKS and a
//! token mint), and a raw WebSocket client that can say anything, including
//! what a well-behaved client never would.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use p256::ecdsa::SigningKey;
use p256::pkcs8::EncodePrivateKey;
use rand_core::OsRng;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use cobolt_server::backend::Backend;
use cobolt_server::config::{ServerConfig, RunMode};
use cobolt_server::jwks::JwksFetcher;
use cobolt_server::log::MemorySink;
use cobolt_server::serve::{Options, ServerHandle};

// ── a certificate ─────────────────────────────────────────────────────────────
//
// An RSA key, not an EC one: macOS's Security framework (which `native-tls` uses
// there) imports a PKCS#8 PEM private key only when it is RSA, and trusts none
// under 2048 bits. The key is made once per machine and kept in the temp folder.

struct TestKey {
    signing: rsa::pkcs1v15::SigningKey<sha2::Sha256>,
    public_der: Vec<u8>,
}

impl rcgen::PublicKeyData for TestKey {
    fn der_bytes(&self) -> &[u8] {
        &self.public_der
    }
    fn algorithm(&self) -> &'static rcgen::SignatureAlgorithm {
        &rcgen::PKCS_RSA_SHA256
    }
}

impl rcgen::SigningKey for TestKey {
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, rcgen::Error> {
        use signature::{SignatureEncoding, Signer};
        let sig = self.signing.sign(msg);
        Ok(sig.to_vec())
    }
}

/// A self-signed certificate for `localhost` / 127.0.0.1 and its PKCS#8 key.
#[derive(Clone)]
pub struct TestCert {
    pub cert_pem: String,
    pub key_pem: String,
}

pub fn test_cert() -> TestCert {
    static CERT: std::sync::OnceLock<TestCert> = std::sync::OnceLock::new();
    CERT.get_or_init(|| {
        // A 2048-bit key takes many seconds to generate in a debug build, so the
        // pair is kept in the temp folder and reused by later runs on this machine
        // (it is a localhost test certificate; it protects nothing).
        let dir = std::env::temp_dir();
        let (cert_path, key_path) = (dir.join("prc-test-localhost.cert.pem"), dir.join("prc-test-localhost.key.pem"));
        if let (Ok(cert_pem), Ok(key_pem)) = (std::fs::read_to_string(&cert_path), std::fs::read_to_string(&key_path)) {
            return TestCert { cert_pem, key_pem };
        }
        let made = make_cert();
        let tmp = dir.join(format!("prc-test-localhost.{}.tmp", std::process::id()));
        if std::fs::write(&tmp, &made.key_pem).is_ok() && std::fs::rename(&tmp, &key_path).is_ok() {
            let _ = std::fs::write(&cert_path, &made.cert_pem);
        }
        made
    })
    .clone()
}

fn make_cert() -> TestCert {
    use rsa::pkcs1::EncodeRsaPublicKey;
    use rsa::pkcs8::EncodePrivateKey as _;
    let private = rsa::RsaPrivateKey::new(&mut OsRng, 2048).expect("rsa key");
    let key_pem = private.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).expect("pkcs8").to_string();
    let public_der = rsa::RsaPublicKey::from(&private).to_pkcs1_der().expect("pkcs1").as_bytes().to_vec();
    let key = TestKey { signing: rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private), public_der };
    let mut params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).expect("params");
    params.subject_alt_names.push(rcgen::SanType::IpAddress(IpAddr::from([127, 0, 0, 1])));
    params.distinguished_name.push(rcgen::DnType::CommonName, "prc-test-backend");
    params.serial_number = Some(rcgen::SerialNumber::from(1u64));
    let cert = params.self_signed(&key).expect("self-signed");
    TestCert { cert_pem: cert.pem(), key_pem }
}

// ── an identity provider ──────────────────────────────────────────────────────

/// One issuer key, its published JWKS, and a mint for tokens.
pub struct Issuer {
    pub issuer: String,
    pub kid: String,
    signing: SigningKey,
    pkcs8_der: Vec<u8>,
}

impl Issuer {
    pub fn new(issuer: &str, kid: &str) -> Self {
        let signing = SigningKey::random(&mut OsRng);
        let pkcs8_der = signing.to_pkcs8_der().expect("der").as_bytes().to_vec();
        Issuer { issuer: issuer.to_string(), kid: kid.to_string(), signing, pkcs8_der }
    }

    pub fn jwks(&self) -> String {
        let p = self.signing.verifying_key().to_encoded_point(false);
        let (x, y) = (p.x().unwrap(), p.y().unwrap());
        json!({"keys": [{"kty": "EC", "crv": "P-256", "alg": "ES256", "use": "sig", "kid": self.kid,
                         "x": URL_SAFE_NO_PAD.encode(x), "y": URL_SAFE_NO_PAD.encode(y)}]})
        .to_string()
    }

    /// A token with these claims (the caller supplies `exp` etc.), signed ES256.
    pub fn mint(&self, claims: &Value) -> String {
        let mut h = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256);
        h.kid = Some(self.kid.clone());
        jsonwebtoken::encode(&h, claims, &jsonwebtoken::EncodingKey::from_ec_der(&self.pkcs8_der)).expect("sign")
    }

    /// Standard claims for an access token valid for `ttl` seconds.
    pub fn access(&self, sub: &str, aud: &str, ttl: i64, extra: Value) -> String {
        let now = cobolt_server::jwt::now();
        let mut c = json!({"iss": self.issuer, "sub": sub, "aud": aud, "iat": now, "exp": now + ttl});
        if let (Some(o), Some(e)) = (c.as_object_mut(), extra.as_object()) {
            for (k, v) in e {
                o.insert(k.clone(), v.clone());
            }
        }
        self.mint(&c)
    }
}

/// Serves a fixed JWKS and counts how often it is asked.
pub struct StaticJwks {
    pub doc: Mutex<String>,
    pub hits: Mutex<u64>,
}

impl StaticJwks {
    pub fn new(doc: String) -> Arc<Self> {
        Arc::new(StaticJwks { doc: Mutex::new(doc), hits: Mutex::new(0) })
    }
}

impl JwksFetcher for StaticJwks {
    fn fetch(&self, _uri: &str) -> Result<String, String> {
        *self.hits.lock().unwrap() += 1;
        Ok(self.doc.lock().unwrap().clone())
    }
}

// ── a backend on disk ─────────────────────────────────────────────────────────

/// Write a backend folder (`exports.toml`, `src/*.cbl`) under a fresh temp dir.
pub fn backend_dir(name: &str, exports: &str, sources: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-server-{name}-{}-{}", std::process::id(), rand_suffix()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("exports.toml"), exports).unwrap();
    for (file, text) in sources {
        std::fs::write(dir.join("src").join(file), text).unwrap();
    }
    dir
}

fn rand_suffix() -> u64 {
    use rand_core::RngCore;
    OsRng.next_u64()
}

// ── a running server ──────────────────────────────────────────────────────────

pub struct Rig {
    pub server: ServerHandle,
    pub cert: TestCert,
    pub issuer: Arc<Issuer>,
    pub jwks: Arc<StaticJwks>,
    pub sink: MemorySink,
    pub dir: std::path::PathBuf,
}

pub const AUDIENCE: &str = "prc-api";
pub const ORIGIN: &str = "https://app.example.test";

/// Start a TLS server with the backend in `dir`, trusting `issuer`.
pub fn rig(dir: std::path::PathBuf, extra_toml: &str) -> Rig {
    let cert = test_cert();
    let issuer = Arc::new(Issuer::new("https://idp.example.test/pool", "k1"));
    let jwks = StaticJwks::new(issuer.jwks());
    std::fs::write(dir.join("server-cert.pem"), &cert.cert_pem).unwrap();
    std::fs::write(dir.join("server-key.pem"), &cert.key_pem).unwrap();
    let config_text = format!(
        r#"
[server]
listen = "127.0.0.1:0"
allowed_origins = ["{ORIGIN}"]
name = "test-backend"
[tls]
cert_chain = "server-cert.pem"
private_key = "server-key.pem"
[auth]
issuer = ["{}"]
audiences = ["{AUDIENCE}"]
jwks_uri = "https://idp.example.test/pool/.well-known/jwks.json"
{extra_toml}
"#,
        issuer.issuer
    );
    let cfg = ServerConfig::from_toml(&config_text).expect("config");
    let resolved = cfg.resolve(RunMode::Release, &dir).expect("resolve");
    let backend = Backend::load(&dir).expect("backend");
    let sink = MemorySink::new();
    let server = cobolt_server::start(resolved, backend, Options { sink: Some(Arc::new(sink.clone())), fetcher: Some(jwks.clone()) }).expect("start");
    Rig { server, cert, issuer, jwks, sink, dir }
}

// ── a raw client ──────────────────────────────────────────────────────────────

pub type Ws = tokio_tungstenite::WebSocketStream<tokio_native_tls::TlsStream<tokio::net::TcpStream>>;

pub struct Raw {
    pub ws: Ws,
}

impl Raw {
    /// Connect with an Origin header (None: send none).
    pub async fn connect(rig: &Rig, origin: Option<&str>) -> Result<Raw, String> {
        Self::connect_to(rig.server.addr, &rig.cert.cert_pem, origin).await
    }

    pub async fn connect_to(addr: std::net::SocketAddr, cert_pem: &str, origin: Option<&str>) -> Result<Raw, String> {
        let tcp = tokio::net::TcpStream::connect(addr).await.map_err(|e| e.to_string())?;
        let connector = cobolt_server::tlsutil::connector(Some(cert_pem.as_bytes()))?;
        let tls = connector.connect("localhost", tcp).await.map_err(|e| format!("tls: {e}"))?;
        let mut req = format!("wss://localhost:{}/ws", addr.port()).into_client_request().map_err(|e| e.to_string())?;
        if let Some(o) = origin {
            req.headers_mut().insert("Origin", o.parse().unwrap());
        }
        let (ws, _) = tokio_tungstenite::client_async(req, tls).await.map_err(|e| format!("ws: {e}"))?;
        Ok(Raw { ws })
    }

    pub async fn send(&mut self, v: Value) {
        self.ws.send(Message::text(v.to_string())).await.expect("send");
    }

    pub async fn send_text(&mut self, text: &str) {
        self.ws.send(Message::text(text.to_string())).await.expect("send");
    }

    /// The next text frame as JSON (or None if the connection ended / timed out).
    pub async fn next(&mut self) -> Option<Value> {
        loop {
            match tokio::time::timeout(std::time::Duration::from_secs(10), self.ws.next()).await {
                Ok(Some(Ok(Message::Text(t)))) => return serde_json::from_str(t.as_str()).ok(),
                Ok(Some(Ok(Message::Close(_)))) | Ok(None) | Ok(Some(Err(_))) | Err(_) => return None,
                Ok(Some(Ok(_))) => continue,
            }
        }
    }

    pub async fn hello(&mut self, contract: &str) -> Value {
        self.send(json!({"t": "hello", "protocol": 1, "contract": contract, "client": "test"})).await;
        self.next().await.expect("hello reply")
    }

    pub async fn auth(&mut self, token: &str) -> Value {
        self.send(json!({"t": "auth", "token": token})).await;
        self.next().await.expect("auth reply")
    }

    /// Call with raw JSON texts (the argument payloads are embedded as given).
    pub async fn call(&mut self, id: u64, program: &str, args: &[(&str, &str)], returning: bool) -> Value {
        let args_json: Vec<String> = args.iter().map(|(m, j)| format!("{{\"mode\":\"{m}\",\"json\":{j}}}")).collect();
        let text = format!("{{\"t\":\"call\",\"id\":{id},\"program\":{},\"args\":[{}],\"returning\":{returning}}}", serde_json::to_string(program).unwrap(), args_json.join(","));
        self.send_text(&text).await;
        self.next().await.expect("call reply")
    }
}

pub fn class(v: &Value) -> String {
    v.get("class").and_then(|c| c.as_str()).unwrap_or("").to_string()
}

pub fn code(v: &Value) -> u64 {
    v.get("code").and_then(|c| c.as_u64()).unwrap_or(0)
}

pub fn kinds(v: &BTreeMap<String, usize>) -> String {
    format!("{v:?}")
}

// ── an RSA identity provider ──────────────────────────────────────────────────
//
// Cognito, Keycloak and Entra sign with RS256. Signing with a 2048-bit key in a
// debug build takes a good part of a second, so the tests that need it are few.

pub struct RsaIssuer {
    pub issuer: String,
    pub kid: String,
    key_pem: String,
    public: rsa::RsaPublicKey,
}

impl RsaIssuer {
    pub fn new(issuer: &str, kid: &str) -> Self {
        use rsa::pkcs8::DecodePrivateKey;
        let key_pem = test_cert().key_pem;
        let private = rsa::RsaPrivateKey::from_pkcs8_pem(&key_pem).expect("the test key");
        RsaIssuer { issuer: issuer.to_string(), kid: kid.to_string(), key_pem, public: rsa::RsaPublicKey::from(&private) }
    }

    pub fn jwks(&self) -> String {
        use rsa::traits::PublicKeyParts;
        json!({"keys": [{"kty": "RSA", "alg": "RS256", "use": "sig", "kid": self.kid,
                         "n": URL_SAFE_NO_PAD.encode(self.public.n().to_bytes_be()),
                         "e": URL_SAFE_NO_PAD.encode(self.public.e().to_bytes_be())}]})
        .to_string()
    }

    pub fn mint(&self, claims: &Value) -> String {
        let mut h = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        h.kid = Some(self.kid.clone());
        jsonwebtoken::encode(&h, claims, &jsonwebtoken::EncodingKey::from_rsa_pem(self.key_pem.as_bytes()).expect("rsa key")).expect("sign")
    }
}
