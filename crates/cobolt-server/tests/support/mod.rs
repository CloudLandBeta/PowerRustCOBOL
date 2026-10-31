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

/// One rig at a time per test binary.
///
/// Measured, not assumed: started together, the thirteen rigs of `client_contract`
/// (each its own TLS server, runtime and interpreter, in a debug build) overrun the
/// client's 10 s connect budget - ten of them failed with "the TLS handshake took
/// too long" - while the same tests one after another pass every time. The cause
/// was not isolated; the rigs are therefore taken in turn. A test that starts two
/// rigs on its own thread is let through (the lock is held per thread).
static SERIAL: Mutex<()> = Mutex::new(());

thread_local! {
    static HELD: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

pub struct Serial {
    _guard: Option<std::sync::MutexGuard<'static, ()>>,
}

impl Serial {
    fn take() -> Serial {
        HELD.with(|held| {
            if held.get() > 0 {
                held.set(held.get() + 1);
                Serial { _guard: None }
            } else {
                let guard = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                held.set(1);
                Serial { _guard: Some(guard) }
            }
        })
    }
}

impl Drop for Serial {
    fn drop(&mut self) {
        HELD.with(|held| held.set(held.get().saturating_sub(1)));
    }
}

pub struct Rig {
    pub server: ServerHandle,
    pub cert: TestCert,
    pub issuer: Arc<Issuer>,
    pub jwks: Arc<StaticJwks>,
    pub sink: MemorySink,
    pub dir: std::path::PathBuf,
    iface: cobolt_wire::contract::InterfaceDescription,
    /// Declared last: it is released after the server has shut down.
    _serial: Serial,
}

impl Rig {
    /// The interface description of the backend this rig serves (what `interface.json` would hold).
    pub fn interface(&self) -> cobolt_wire::contract::InterfaceDescription {
        self.iface.clone()
    }
}

pub const AUDIENCE: &str = "prc-api";
pub const ORIGIN: &str = "https://app.example.test";

/// Start a TLS server with the backend in `dir`, trusting `issuer`.
pub fn rig(dir: std::path::PathBuf, extra_toml: &str) -> Rig {
    rig_with_state(dir, extra_toml, cobolt_server::backend::StateMode::PerCall)
}

/// [`rig`], with the project's `[backend] state` (WASM-003 Q2).
pub fn rig_with_state(dir: std::path::PathBuf, extra_toml: &str, state: cobolt_server::backend::StateMode) -> Rig {
    let serial = Serial::take();
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
    let mut backend = Backend::load(&dir).expect("backend");
    backend.state = state;
    let iface = backend.interface.clone();
    let sink = MemorySink::new();
    let server = cobolt_server::start(resolved, backend, Options { sink: Some(Arc::new(sink.clone())), fetcher: Some(jwks.clone()) }).expect("start");
    Rig { server, cert, issuer, jwks, sink, dir, iface, _serial: serial }
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

// ── a TCP proxy that can lose the connection on command ──────────────────────
//
// TLS runs end to end through it; it only moves bytes, so a test can cut a
// connection at the moment it chooses, or swallow what the server says so that
// a call that ran never reports back.

use std::sync::atomic::{AtomicU8, Ordering};

pub const PROXY_FORWARD: u8 = 0;
/// Bytes from the server are discarded (the client never hears the answer).
pub const PROXY_SWALLOW_REPLIES: u8 = 1;
/// New connections are closed at once.
pub const PROXY_REFUSE: u8 = 2;

pub struct Proxy {
    pub addr: std::net::SocketAddr,
    mode: Arc<AtomicU8>,
    kill: Arc<tokio::sync::watch::Sender<u64>>,
    /// When each connection reached the proxy, refused ones included (WASM-003 R25: the attempts).
    accepted: Arc<Mutex<Vec<std::time::Instant>>>,
    _thread: std::thread::JoinHandle<()>,
}

impl Proxy {
    pub fn start(target: std::net::SocketAddr) -> Proxy {
        let mode = Arc::new(AtomicU8::new(PROXY_FORWARD));
        let (kill_tx, kill_rx) = tokio::sync::watch::channel(0u64);
        let kill = Arc::new(kill_tx);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (m2, k2) = (mode.clone(), kill_rx);
        let accepted = Arc::new(Mutex::new(Vec::new()));
        let a2 = accepted.clone();
        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
            rt.block_on(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let _ = ready_tx.send(listener.local_addr().unwrap());
                loop {
                    let Ok((client, _)) = listener.accept().await else { continue };
                    a2.lock().unwrap().push(std::time::Instant::now());
                    if m2.load(Ordering::SeqCst) == PROXY_REFUSE {
                        drop(client);
                        continue;
                    }
                    let Ok(server) = tokio::net::TcpStream::connect(target).await else { continue };
                    let (cr, cw) = client.into_split();
                    let (sr, sw) = server.into_split();
                    // A connection made after a kill must not feel it: mark the current version as seen.
                    let mut up = k2.clone();
                    up.borrow_and_update();
                    let mut down = k2.clone();
                    down.borrow_and_update();
                    tokio::spawn(pump(cr, sw, m2.clone(), false, up));
                    tokio::spawn(pump(sr, cw, m2.clone(), true, down));
                }
            });
        });
        let addr = ready_rx.recv().unwrap();
        Proxy { addr, mode, kill, accepted, _thread: thread }
    }

    /// When each connection reached the proxy so far.
    pub fn accepted(&self) -> Vec<std::time::Instant> {
        self.accepted.lock().unwrap().clone()
    }

    pub fn set(&self, mode: u8) {
        self.mode.store(mode, Ordering::SeqCst);
    }

    /// Close every connection that goes through the proxy now.
    pub fn kill_all(&self) {
        self.kill.send_modify(|v| *v += 1);
    }
}

async fn pump(
    mut from: tokio::net::tcp::OwnedReadHalf,
    mut to: tokio::net::tcp::OwnedWriteHalf,
    mode: Arc<AtomicU8>,
    server_to_client: bool,
    mut kill: tokio::sync::watch::Receiver<u64>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        tokio::select! {
            _ = kill.changed() => break,
            n = from.read(&mut buf) => {
                let n = n.unwrap_or(0);
                if n == 0 { break; }
                if server_to_client && mode.load(Ordering::SeqCst) == PROXY_SWALLOW_REPLIES { continue; }
                if to.write_all(&buf[..n]).await.is_err() { break; }
            }
        }
    }
    let _ = to.shutdown().await;
}

/// A plain HTTPS GET over TLS (the download address), returning status and body.
pub fn https_get(addr: std::net::SocketAddr, cert_pem: &str, path: &str, range_from: Option<u64>) -> Option<(u16, Vec<u8>)> {
    use std::io::{Read, Write};
    let mut b = native_tls::TlsConnector::builder();
    b.add_root_certificate(native_tls::Certificate::from_pem(cert_pem.as_bytes()).ok()?);
    let c = b.build().ok()?;
    let tcp = std::net::TcpStream::connect(addr).ok()?;
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(10))).ok()?;
    let mut s = c.connect("localhost", tcp).ok()?;
    let range = range_from.map(|n| format!("Range: bytes={n}-\r\n")).unwrap_or_default();
    write!(s, "GET {path} HTTP/1.1\r\nHost: localhost\r\n{range}Connection: close\r\n\r\n").ok()?;
    let mut raw = Vec::new();
    let _ = s.read_to_end(&mut raw);
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let status = head.split(' ').nth(1)?.parse().ok()?;
    Some((status, raw[split + 4..].to_vec()))
}
