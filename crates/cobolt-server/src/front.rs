// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The backend's HTTP front on Leptos (WASM-003 R2, Q3; T10a).
//!
//! The platform's routes are a **Leptos route table** ([`PlatformRoutes`], a
//! `leptos_router` `<Routes>`), which `leptos_axum::generate_route_list` reads
//! and `LeptosRoutes::leptos_routes_with_handler` mounts on the router, and the
//! health probe is a **Leptos server function** ([`health`]), answered by
//! `leptos_axum::handle_server_fns_with_context`. A path that is in neither is
//! not served (404). The table is closed (D6): a developer cannot add a route;
//! the identity routes of WASM-009 are added here when they exist.
//!
//! What a route does once it is reached — the Origin check, the WebSocket
//! upgrade and session, the download — is `serve.rs`'s, unchanged (D1). There
//! is no page, no hydration and no client crate: the developer sees only COBOL.

use std::sync::{Arc, OnceLock};

use leptos::prelude::*;
use leptos::server_fn::codec::GetUrl;
use leptos_axum::AxumRouteListing;
use leptos::config::LeptosOptions;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

/// The WebSocket upgrade on `/` and on `/ws`.
pub const UPGRADE_PATHS: [&str; 2] = ["/", "/ws"];
/// A file offered for download (WASM-014 R15), as axum spells it.
pub const DOWNLOAD_PATH: &str = "/dl/{token}";
/// The health probe: the server function's address.
pub const HEALTH_PATH: &str = "/health";

/// The platform's routes. They render nothing: Leptos owns the table, the
/// handlers of `serve.rs` answer them.
#[component]
pub fn PlatformRoutes() -> impl IntoView {
    view! {
        <Router>
            <Routes fallback=|| ()>
                <Route path=path!("/") view=|| () />
                <Route path=path!("/ws") view=|| () />
                <Route path=path!("/dl/:token") view=|| () />
            </Routes>
        </Router>
    }
}

/// The routes of [`PlatformRoutes`] as Leptos lists them for axum; read once.
pub fn route_list() -> &'static [AxumRouteListing] {
    static LIST: OnceLock<Vec<AxumRouteListing>> = OnceLock::new();
    LIST.get_or_init(|| leptos_axum::generate_route_list(PlatformRoutes))
}

/// The listings of `paths`, for mounting each with its own handler.
pub fn listings(paths: &[&str]) -> Vec<AxumRouteListing> {
    route_list().iter().filter(|l| paths.contains(&l.path())).cloned().collect()
}

/// What `leptos_axum`'s router extension asks of the router's state. The
/// front serves no site, so these are the defaults under a fixed name.
pub fn options() -> LeptosOptions {
    static OPTIONS: OnceLock<LeptosOptions> = OnceLock::new();
    OPTIONS.get_or_init(|| LeptosOptions::builder().output_name("backend").build()).clone()
}

/// `GET /health`: the probe's JSON (status, contract, protocol, counters).
/// The server's state is provided as context by the route that runs it.
#[server(prefix = "", endpoint = "health", input = GetUrl)]
pub async fn health() -> Result<serde_json::Value, ServerFnError> {
    let shared = use_context::<Arc<crate::serve::Shared>>().ok_or_else(|| ServerFnError::new("the server's state is not provided"))?;
    Ok(crate::serve::health_value(&shared))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is Leptos's: what it lists, and only that, is mounted; the
    /// health probe is a registered server function.
    #[test]
    fn the_platform_routes_are_leptos_s_table_and_the_probe_a_server_function() {
        let mut paths: Vec<&str> = route_list().iter().map(|l| l.path()).collect();
        paths.sort();
        println!("leptos route table: {paths:?}");
        assert_eq!(paths, ["/", "/dl/{token}", "/ws"]);
        assert_eq!(listings(&UPGRADE_PATHS).len(), 2);
        assert_eq!(listings(&[DOWNLOAD_PATH]).len(), 1);
        assert!(listings(&["/admin"]).is_empty());
        let fns: Vec<(&str, String)> = leptos::server_fn::axum::server_fn_paths().map(|(p, m)| (p, m.to_string())).collect();
        println!("server functions: {fns:?}");
        assert!(fns.contains(&(HEALTH_PATH, "GET".to_string())), "{fns:?}");
    }
}
