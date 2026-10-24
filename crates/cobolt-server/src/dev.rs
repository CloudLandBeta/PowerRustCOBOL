// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The development loop (WASM-003 R20): start the backend from its project
//! folder, see its log, stop it. It is the same server as the built binary —
//! the same code path, the same checks — with these differences only, all of
//! them named in the log:
//!
//! * the backend's programs are read from `backend/src` (no build);
//! * with no `backend.toml`, it listens on **loopback** with plain `ws://`
//!   (allowed for loopback in a development run, WASM-003 R5) on a free port;
//! * a **development token** (HS256, signed with a secret made fresh for the
//!   run) is accepted — `unsafe_dev_token_secret`, which a release start
//!   refuses (WASM-003 Q5, WASM-013 R11);
//! * any `Origin` is accepted.
//!
//! The IDE calls [`DevServer::start`], shows [`DevServer::log_lines`] in the
//! Output panel, hands [`DevServer::dev_token`] to the Run Form it launches,
//! and calls [`DevServer::stop`]. `rcrun backend run` is the same thing for a
//! terminal.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::backend::Backend;
use crate::config::{RunMode, ServerConfig};
use crate::log::{LogRecord, LogSink, MemorySink};
use crate::serve::{Options, ServerHandle};

/// The memory sink, and each record also printed to standard error as it
/// happens — `rcrun backend run`'s live log, which the IDE's Output panel
/// reads from the child process (WASM-003 R20).
struct EchoSink(MemorySink);

impl LogSink for EchoSink {
    fn emit(&self, record: &LogRecord) {
        self.0.emit(record);
        if let Ok(line) = serde_json::to_string(record) {
            eprintln!("{line}");
        }
    }
}

/// What a development run is told.
#[derive(Debug, Clone, Default)]
pub struct DevOptions {
    /// The project's `backend/` folder.
    pub backend_dir: PathBuf,
    /// `host:port` to listen on; default `127.0.0.1:0` (a free port).
    pub listen: Option<String>,
    /// The project's `[backend] state`.
    pub state: crate::backend::StateMode,
    /// Also print each log record to standard error as it is written.
    pub echo: bool,
}

pub struct DevServer {
    handle: ServerHandle,
    sink: MemorySink,
    secret: String,
    /// `ws://127.0.0.1:PORT/ws` (or `wss://` when the folder's `backend.toml` has TLS).
    pub url: String,
}

fn fresh_secret() -> String {
    let mut b = [0u8; 24];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A development token: HS256, `sub`, `roles`, valid `ttl_secs`.
pub fn mint_dev_token(secret: &str, sub: &str, roles: &[&str], ttl_secs: i64) -> String {
    let now = crate::jwt::now();
    let claims = serde_json::json!({"sub": sub, "roles": roles, "iat": now, "exp": now + ttl_secs, "iss": "prc-dev"});
    jsonwebtoken::encode(&jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256), &claims, &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes())).unwrap_or_default()
}

impl DevServer {
    pub fn start(opts: &DevOptions) -> Result<DevServer, Vec<String>> {
        let mut backend = Backend::load(&opts.backend_dir)?;
        backend.state = opts.state;
        let secret = fresh_secret();
        let path = opts.backend_dir.join("backend.toml");
        let mut cfg = if path.exists() { ServerConfig::load(&path).map_err(|e| vec![e])? } else { ServerConfig::default() };
        if cfg.server.listen.is_none() || opts.listen.is_some() {
            cfg.server.listen = Some(opts.listen.clone().unwrap_or_else(|| "127.0.0.1:0".to_string()));
        }
        cfg.unsafe_dev_token_secret = Some(crate::config::Secret::new(secret.clone()));
        cfg.unsafe_allow_any_origin = true;
        let resolved = cfg.resolve(RunMode::Development, &opts.backend_dir)?;
        let tls = resolved.tls.is_some();
        let sink = MemorySink::new();
        let to: Arc<dyn LogSink> = if opts.echo { Arc::new(EchoSink(sink.clone())) } else { Arc::new(sink.clone()) };
        let handle = crate::serve::start(resolved, backend, Options { sink: Some(to), fetcher: None }).map_err(|e| vec![e])?;
        let url = format!("{}://{}/ws", if tls { "wss" } else { "ws" }, handle.addr);
        Ok(DevServer { handle, sink, secret, url })
    }

    pub fn addr(&self) -> std::net::SocketAddr {
        self.handle.addr
    }

    /// A token this run accepts.
    pub fn dev_token(&self, sub: &str, roles: &[&str]) -> String {
        mint_dev_token(&self.secret, sub, roles, 3600)
    }

    /// The log so far, one JSON line per record (never a token).
    pub fn log_lines(&self) -> Vec<String> {
        self.sink.lines()
    }

    pub fn stop(self) {
        self.handle.stop();
    }
}

/// Write the interface description of the backend in `backend_dir` to
/// `contract/interface.json` (what Build does, and the frontend's Check reads).
pub fn write_interface(backend_dir: &Path) -> Result<PathBuf, Vec<String>> {
    let backend = Backend::load(backend_dir)?;
    let out = backend_dir.join("contract").join("interface.json");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| vec![format!("{}: {e}", parent.display())])?;
    }
    std::fs::write(&out, backend.interface.to_json()).map_err(|e| vec![format!("{}: {e}", out.display())])?;
    Ok(out)
}

/// Record the backend's current interface as **released**: copy it to
/// `contract/released/<contract>.json`. Later builds compare against it and a
/// release build fails on a breaking change (WASM-014 R5). A contract version
/// that was already released is never overwritten with a different interface —
/// that is the point of a released version — so the call fails instead.
pub fn release_interface(backend_dir: &Path) -> Result<PathBuf, Vec<String>> {
    let backend = Backend::load(backend_dir)?;
    let dir = backend_dir.join("contract").join("released");
    std::fs::create_dir_all(&dir).map_err(|e| vec![format!("{}: {e}", dir.display())])?;
    let out = dir.join(format!("{}.json", backend.interface.contract));
    let text = backend.interface.to_json();
    if let Ok(existing) = std::fs::read_to_string(&out) {
        if existing.trim() != text.trim() {
            return Err(vec![format!(
                "contract {} was released already with a different interface ({}): raise the version in exports.toml to release this one",
                backend.interface.contract,
                out.display()
            )]);
        }
        return Ok(out);
    }
    std::fs::write(&out, text).map_err(|e| vec![format!("{}: {e}", out.display())])?;
    Ok(out)
}
