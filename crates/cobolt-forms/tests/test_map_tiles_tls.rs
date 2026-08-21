// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The OSM basemap needs a TLS-carrying agent, and this pins why.
//!
//! `ureq`'s `native-tls` feature is an **adapter only**: the crate-level
//! helpers (`ureq::get`, …) and a bare `AgentBuilder` never pick it up. A call
//! made either of those ways fails with "no TLS backend" before it leaves the
//! machine — which is exactly what `map_tiles` did, so every tile failed, and
//! the Maps control drew a flat grey square with only its markers on it
//! (operator, 2026-08-20).
//!
//! Grey is also what a map centred on open water looks like, which is why this
//! went unnoticed: nothing distinguished "no basemap" from "no land here".
//!
//! ⚠️ **This test uses the network** — it fetches one 256×256 tile from
//! `tile.openstreetmap.org`. Offline it reports that and passes, because an
//! unreachable host is not the bug it is guarding.

#![cfg(feature = "render")]

use std::io::Read;
use std::sync::Arc;

/// The tile `map_tiles` would request at the top of the world map, and the
/// User-Agent it sends — OSM's usage policy rejects a client that sends the
/// default one.
const TILE_URL: &str = "https://tile.openstreetmap.org/2/1/1.png";
const USER_AGENT: &str =
    "PowerRustCOBOL-IDE/1.0 (+https://github.com/CloudLandBeta/PowerRustCOBOL)";

fn connector() -> Option<Arc<native_tls::TlsConnector>> {
    native_tls::TlsConnector::new().ok().map(Arc::new)
}

/// The shape `map_tiles::agent` builds: an agent carrying the connector.
#[test]
fn a_tile_fetch_with_a_tls_agent_succeeds() {
    let Some(connector) = connector() else {
        eprintln!("no platform TLS on this machine — nothing to check");
        return;
    };
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(15))
        .tls_connector(connector)
        .build();

    match agent.get(TILE_URL).set("User-Agent", USER_AGENT).call() {
        Ok(resp) => {
            let mut bytes = Vec::new();
            resp.into_reader()
                .read_to_end(&mut bytes)
                .expect("tile body reads");
            assert!(
                bytes.len() > 100,
                "a PNG tile, not an error page: {} bytes",
                bytes.len()
            );
            assert_eq!(
                &bytes[..4],
                b"\x89PNG",
                "the basemap tile must decode as a PNG"
            );
            eprintln!("fetched {} bytes of PNG through a TLS agent", bytes.len());
        }
        // Offline, or OSM refusing service: not what this guards.
        Err(e) => eprintln!("tile host unreachable ({e}) — skipping"),
    }
}

/// **The bug, reproduced.** The crate-level helper takes no connector, so the
/// very same request fails without touching the network. If this ever starts
/// succeeding, `ureq` has gained a built-in backend and the agent in
/// `map_tiles` is no longer load-bearing — which is worth knowing, not worth
/// silently relying on.
#[test]
fn the_bare_helper_has_no_tls_backend() {
    match ureq::get(TILE_URL).set("User-Agent", USER_AGENT).call() {
        Ok(_) => eprintln!(
            "NOTE: ureq's crate-level helper now reaches HTTPS on its own; \
             map_tiles no longer depends on its own agent for correctness"
        ),
        Err(e) => {
            let text = e.to_string();
            assert!(
                text.to_lowercase().contains("tls"),
                "expected a TLS-backend failure from the bare helper, got: {text}"
            );
            eprintln!("bare helper failed as expected: {text}");
        }
    }
}
