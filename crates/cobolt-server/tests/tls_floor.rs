// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The TLS floor is proven, and the HTTPS headers are present** (WASM-013 T13; R4, O5).
//!
//! A loopback HTTPS server (the test certificate of the other loopback tests) is contacted by a client
//! that offers nothing above TLS 1.1 and must be refused; a client that offers TLS 1.2 or better is served,
//! and its response carries every line of `SECURITY_HEADERS`. Some platforms' TLS stacks no longer let a
//! client offer 1.1 at all; the test then says so and asserts the server's configured minimum instead, in
//! the one place it is set (`tlsutil::acceptor`).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use cobolt_server::testkit::*;
use native_tls::{Certificate, Protocol, TlsConnector};

fn fresh_rig() -> Rig {
    rig(backend_dir("tlsfloor", "[contract]\nversion = \"1.0.0\"\n", &[]), "")
}

/// What an old client meets: `Ok(true)` it was refused, `Ok(false)` it was served, `Err` it cannot offer 1.1.
fn old_client_refused(rig: &Rig) -> Result<bool, String> {
    let mut b = TlsConnector::builder();
    b.add_root_certificate(Certificate::from_pem(rig.cert.cert_pem.as_bytes()).unwrap());
    b.min_protocol_version(Some(Protocol::Tlsv10));
    b.max_protocol_version(Some(Protocol::Tlsv11));
    let c = b.build().map_err(|e| format!("the client cannot be built capped at TLS 1.1: {e}"))?;
    let tcp = TcpStream::connect(rig.server.addr).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    tcp.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
    match c.connect("localhost", tcp) {
        Err(e) => {
            println!("  TLS 1.1 client: refused during the handshake ({e})");
            Ok(true)
        }
        Ok(mut s) => {
            let _ = s.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
            let mut out = String::new();
            let _ = s.read_to_string(&mut out);
            Ok(out.starts_with("HTTP/1.1 200"))
        }
    }
}

#[test]
fn a_client_offering_nothing_above_tls_11_is_refused_and_a_modern_one_gets_every_security_header() {
    println!("TLS floor ({})", std::env::consts::OS);
    let rig = fresh_rig();

    match old_client_refused(&rig) {
        Ok(refused) => assert!(refused, "a client capped at TLS 1.1 was served"),
        Err(why) => {
            println!("  TLS 1.1 client: not possible on this platform ({why}); asserting the server's configured minimum");
            let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tlsutil.rs")).unwrap();
            let acceptor = &src[src.find("pub fn acceptor").unwrap()..src.find("pub fn connector").unwrap()];
            assert!(acceptor.contains("min_protocol_version(Some(Protocol::Tlsv12))"), "the server's minimum is not TLS 1.2");
        }
    }

    // a client at TLS 1.2 or better
    let mut b = TlsConnector::builder();
    b.add_root_certificate(Certificate::from_pem(rig.cert.cert_pem.as_bytes()).unwrap());
    b.min_protocol_version(Some(Protocol::Tlsv12));
    let c = b.build().unwrap();
    let tcp = TcpStream::connect(rig.server.addr).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut s = c.connect("localhost", tcp).expect("a TLS 1.2+ client is served");
    s.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    let head = out.split("\r\n\r\n").next().unwrap().to_ascii_lowercase();
    let mut present = 0;
    for (name, value) in cobolt_server::http::SECURITY_HEADERS {
        let line = format!("{name}: {value}").to_ascii_lowercase();
        assert!(head.contains(&line), "the response lacks `{line}`:\n{head}");
        present += 1;
    }
    println!("  TLS 1.2+ client: served; {present} of {} security headers present", cobolt_server::http::SECURITY_HEADERS.len());
    rig.server.stop();
}
