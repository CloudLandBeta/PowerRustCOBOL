// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The small HTTPS surface beside the WebSocket (WASM-003 Q3, WASM-013 R4).
//!
//! Requests are parsed by hyper and routed by the closed route table in
//! `serve.rs` (`front_router`); what stays here is what every answer shares.

/// The largest request head accepted; past it hyper answers 431.
pub const MAX_HEAD: usize = 16 * 1024;

/// The security headers every response of the HTTPS surface carries
/// (WASM-013 R4).
pub const SECURITY_HEADERS: &[(&str, &str)] = &[
    ("strict-transport-security", "max-age=63072000; includeSubDomains"),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    ("permissions-policy", "geolocation=(), camera=(), microphone=()"),
    ("cache-control", "no-store"),
];

/// Percent-decode a query component (enough for the signed-link and callback parameters).
pub fn query_param(query: &str, name: &str) -> Option<String> {
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == name {
            return Some(percent_decode(v));
        }
    }
    None
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let h = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|x| u8::from_str_radix(x, 16).ok());
                match h {
                    Some(v) => {
                        out.push(v);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
