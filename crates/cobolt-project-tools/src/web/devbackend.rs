// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Where the Backend of a development run listens, and what the page is
//! built to call** (WASM-008 R6, WASM-003 R20).
//!
//! A Web page connects to the Backend address baked into its bundle by the
//! Build (`config.json` `backend.origins` and the `connect-src` of its
//! Content-Security-Policy). So a Preview that starts the project's Backend
//! must know the address the Backend **actually** listens on before it
//! builds. The rules, decided here without a process or a socket of their own
//! (the caller says which ports are free):
//!
//! * `[web] backend_url` **set** in the manifest is honoured as it is: the
//!   page is built for it. When it names this machine, the Backend is started
//!   on its port; when that port is already taken, it is not started (the
//!   page calls whatever listens there, and the developer is told); when it
//!   names another machine, no local Backend is started.
//! * `[web] backend_url` **empty**: the Backend is started on the port of
//!   `backend/backend.toml` `[server] listen`, on loopback; when that port is
//!   taken — or none is set — on a free loopback port. Either way the page is
//!   built for the address the Backend reports once it listens, so a port
//!   fallback can never leave the page calling the wrong place.

use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::Path;

/// The address a development run listens on when it is told nothing: a free
/// loopback port (the same default as `rcrun backend run`).
pub const ANY_LOOPBACK_PORT: &str = "127.0.0.1:0";

/// What a Preview does about the project's Backend before it builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevBackendPlan {
    /// Start the Backend with `--listen <listen>` and wait for its address.
    Start {
        /// `host:port` for `rcrun backend run --listen`.
        listen: String,
        /// Build the page for the address the Backend reports (the manifest
        /// names none); `false` keeps the manifest's.
        bake_reported: bool,
        /// The configured port that was in use, when the Backend falls back to a free one.
        busy_port: Option<u16>,
    },
    /// The manifest's loopback address is already in use: no Backend is
    /// started; the page is built for that address.
    Taken { url: String },
    /// The manifest names another machine: no local Backend is started.
    Remote { url: String },
}

/// `[server] listen` of `backend/backend.toml` in `project_dir`, if it says one.
pub fn configured_listen(project_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(project_dir.join("backend").join("backend.toml")).ok()?;
    let table = text.parse::<toml::Table>().ok()?;
    table.get("server")?.get("listen")?.as_str().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// Can a socket be bound on `127.0.0.1:port` now? (A port in use answers `false`.)
pub fn loopback_port_is_free(port: u16) -> bool {
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).is_ok()
}

/// Is `host` a name or address of this machine?
fn is_loopback_host(host: &str) -> bool {
    let h = host.trim().trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    h == "localhost" || h == "127.0.0.1" || h == "::1" || h.ends_with(".localhost")
}

/// The host and the port of a `ws://` / `wss://` address (the scheme's default port when none is written).
fn host_and_port(url: &str) -> Option<(String, u16)> {
    let (scheme, rest) = url.trim().split_once("://")?;
    let default = match scheme.to_ascii_lowercase().as_str() {
        "ws" => 80,
        "wss" => 443,
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    if let Some(v6) = authority.strip_prefix('[') {
        let (host, after) = v6.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(p) => p.parse().ok()?,
            None => default,
        };
        return Some((host.to_owned(), port));
    }
    match authority.rsplit_once(':') {
        Some((h, p)) => Some((h.to_owned(), p.parse().ok()?)),
        None => Some((authority.to_owned(), default)),
    }
    .filter(|(h, _)| !h.is_empty())
}

/// The port of a `host:port` listen address; `None` when it is not one.
fn listen_port(listen: &str) -> Option<u16> {
    let (_, port) = listen.trim().rsplit_once(':')?;
    port.parse().ok()
}

/// Decide what a Preview does about the Backend (see the module comment).
/// `manifest_url` is `[web] backend_url`; `configured` is
/// [`configured_listen`]; `port_is_free` answers for a loopback port.
pub fn plan(manifest_url: &str, configured: Option<&str>, port_is_free: impl Fn(u16) -> bool) -> DevBackendPlan {
    let manifest_url = manifest_url.trim();
    if !manifest_url.is_empty() {
        return match host_and_port(manifest_url) {
            Some((host, port)) if is_loopback_host(&host) => {
                if port_is_free(port) {
                    DevBackendPlan::Start { listen: format!("127.0.0.1:{port}"), bake_reported: false, busy_port: None }
                } else {
                    DevBackendPlan::Taken { url: manifest_url.to_owned() }
                }
            }
            _ => DevBackendPlan::Remote { url: manifest_url.to_owned() },
        };
    }
    // A development run listens on this machine only: the configured port, on loopback.
    match configured.and_then(listen_port).filter(|p| *p != 0) {
        Some(port) if port_is_free(port) => {
            DevBackendPlan::Start { listen: format!("127.0.0.1:{port}"), bake_reported: true, busy_port: None }
        }
        Some(port) => DevBackendPlan::Start { listen: ANY_LOOPBACK_PORT.to_owned(), bake_reported: true, busy_port: Some(port) },
        None => DevBackendPlan::Start { listen: ANY_LOOPBACK_PORT.to_owned(), bake_reported: true, busy_port: None },
    }
}

/// The origin (`scheme://host:port`) of a `ws://` / `wss://` address, to compare two of them.
pub fn origin(url: &str) -> Option<String> {
    let (scheme, _) = url.trim().split_once("://")?;
    let (host, port) = host_and_port(url)?;
    let host = if host.contains(':') { format!("[{host}]") } else { host };
    Some(format!("{}://{}:{port}", scheme.to_ascii_lowercase(), host.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FREE: fn(u16) -> bool = |_| true;
    const TAKEN: fn(u16) -> bool = |_| false;

    #[test]
    fn an_empty_manifest_address_starts_the_backend_on_its_configured_port_and_bakes_what_it_reports() {
        assert_eq!(
            plan("", Some("127.0.0.1:8443"), FREE),
            DevBackendPlan::Start { listen: "127.0.0.1:8443".into(), bake_reported: true, busy_port: None }
        );
        // A network address in backend.toml: a development run still listens on loopback.
        assert_eq!(
            plan("  ", Some("0.0.0.0:9000"), FREE),
            DevBackendPlan::Start { listen: "127.0.0.1:9000".into(), bake_reported: true, busy_port: None }
        );
    }

    #[test]
    fn a_configured_port_in_use_falls_back_to_a_free_one_before_the_build() {
        assert_eq!(
            plan("", Some("127.0.0.1:8443"), TAKEN),
            DevBackendPlan::Start { listen: ANY_LOOPBACK_PORT.into(), bake_reported: true, busy_port: Some(8443) }
        );
        // Only the configured port is asked about.
        assert_eq!(
            plan("", Some("127.0.0.1:8443"), |p| p != 8443),
            DevBackendPlan::Start { listen: ANY_LOOPBACK_PORT.into(), bake_reported: true, busy_port: Some(8443) }
        );
    }

    #[test]
    fn no_configured_port_takes_a_free_one() {
        for configured in [None, Some("127.0.0.1:0"), Some("not an address")] {
            assert_eq!(
                plan("", configured, TAKEN),
                DevBackendPlan::Start { listen: ANY_LOOPBACK_PORT.into(), bake_reported: true, busy_port: None },
                "{configured:?}"
            );
        }
    }

    #[test]
    fn a_manifest_address_on_this_machine_is_honoured_and_the_backend_started_there() {
        for url in ["ws://127.0.0.1:8750/ws", "ws://localhost:8750", "ws://[::1]:8750/ws", "ws://app.localhost:8750"] {
            assert_eq!(
                plan(url, Some("127.0.0.1:8443"), FREE),
                DevBackendPlan::Start { listen: "127.0.0.1:8750".into(), bake_reported: false, busy_port: None },
                "{url}"
            );
        }
        assert_eq!(
            plan("ws://127.0.0.1", None, FREE),
            DevBackendPlan::Start { listen: "127.0.0.1:80".into(), bake_reported: false, busy_port: None },
            "the scheme's port"
        );
    }

    #[test]
    fn a_manifest_address_in_use_is_not_started_and_never_moved() {
        assert_eq!(plan("ws://127.0.0.1:8750/ws", None, TAKEN), DevBackendPlan::Taken { url: "ws://127.0.0.1:8750/ws".into() });
    }

    #[test]
    fn a_manifest_address_elsewhere_starts_nothing() {
        for url in ["wss://api.example.com/ws", "wss://api.example.com:8443", "ws://10.0.0.5:8443"] {
            assert_eq!(plan(url, Some("127.0.0.1:8443"), FREE), DevBackendPlan::Remote { url: url.into() }, "{url}");
        }
    }

    #[test]
    fn the_configured_listen_is_read_from_backend_toml() {
        let dir = std::env::temp_dir().join(format!("prc-devbackend-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("backend")).unwrap();
        assert_eq!(configured_listen(&dir), None, "no backend.toml");
        std::fs::write(dir.join("backend/backend.toml"), "# c\n[server]\nlisten = \"127.0.0.1:8443\"\n").unwrap();
        assert_eq!(configured_listen(&dir).as_deref(), Some("127.0.0.1:8443"));
        std::fs::write(dir.join("backend/backend.toml"), "[auth]\nissuer = []\n").unwrap();
        assert_eq!(configured_listen(&dir), None, "no [server] listen");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_port_in_use_is_seen() {
        let held = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let port = held.local_addr().unwrap().port();
        assert!(!loopback_port_is_free(port));
        drop(held);
        assert!(loopback_port_is_free(port));
    }

    #[test]
    fn origins_compare_addresses_not_spellings() {
        assert_eq!(origin("ws://127.0.0.1:8443/ws").as_deref(), Some("ws://127.0.0.1:8443"));
        assert_eq!(origin("WS://127.0.0.1:8443").as_deref(), Some("ws://127.0.0.1:8443"));
        assert_eq!(origin("wss://Api.Example.com/ws").as_deref(), Some("wss://api.example.com:443"));
        assert_eq!(origin("ws://[::1]:9/ws").as_deref(), Some("ws://[::1]:9"));
        assert_ne!(origin("ws://127.0.0.1:8443/ws"), origin("ws://127.0.0.1:8444/ws"));
    }
}
