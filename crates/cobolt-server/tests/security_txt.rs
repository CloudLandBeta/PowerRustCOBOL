// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-013 T14 (Q4): the backend's HTTPS surface answers `GET /.well-known/security.txt` from the `[security]`
//! table of `backend.toml`, with the same text the Web Build writes into the bundle; without the table the path is a
//! 404, and a table that is incomplete or out of date stops the start, naming the key.

use cobolt_server::config::{RunMode, ServerConfig};
use cobolt_server::testkit::*;
use cobolt_wire::security_txt::SecurityTxt;

const EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n";
const SECURITY: &str = "[security]\ncontact = \"mailto:security@example.com\"\nexpires = \"2999-01-31\"\npolicy = \"https://example.com/policy\"\npreferred_languages = \"en, pt\"\n";

#[test]
fn the_backend_serves_the_text_the_build_writes_and_a_404_without_the_table() {
    let rig = rig(backend_dir("sectxt", EXPORTS, &[]), SECURITY);
    let (status, body) = https_get(rig.server.addr, &rig.cert.cert_pem, "/.well-known/security.txt", None).expect("the request is answered");
    let body = String::from_utf8(body).unwrap();
    println!("GET /.well-known/security.txt -> {status}\n{body}");
    let same = SecurityTxt {
        contact: "mailto:security@example.com".into(),
        policy: "https://example.com/policy".into(),
        expires: "2999-01-31".into(),
        preferred_languages: "en, pt".into(),
    }
    .render(0)
    .unwrap();
    assert_eq!(status, 200);
    assert_eq!(body, same, "the backend and the bundle write one text");
    assert!(body.contains("Contact:") && body.contains("Expires:"));
    rig.server.stop();

    let rig = rig_without_security();
    let (status, _) = https_get(rig.server.addr, &rig.cert.cert_pem, "/.well-known/security.txt", None).expect("answered");
    assert_eq!(status, 404, "without [security] the path does not exist");
    rig.server.stop();
}

fn rig_without_security() -> Rig {
    rig(backend_dir("sectxt-none", EXPORTS, &[]), "")
}

#[test]
fn an_incomplete_or_stale_table_stops_the_start_and_names_the_key() {
    let base = "[server]\nlisten = \"127.0.0.1:8443\"\nallowed_origins = [\"https://app.example.com\"]\n[auth]\nissuer = [\"https://idp.example.com\"]\naudiences = [\"app\"]\n";
    let cases = [
        ("no expires", "[security]\ncontact = \"mailto:a@example.com\"\n", "[security] expires"),
        ("no contact", "[security]\nexpires = \"2999-01-31\"\n", "[security] contact"),
        ("expired", "[security]\ncontact = \"mailto:a@example.com\"\nexpires = \"2020-01-01\"\n", "has passed"),
        ("a misspelt key", "[security]\ncontact = \"mailto:a@example.com\"\nexpires = \"2999-01-31\"\ncontakt = \"x\"\n", "contakt"),
    ];
    for (name, table, want) in cases {
        let text = format!("{base}{table}");
        let why = match ServerConfig::from_toml(&text) {
            Err(e) => e,
            Ok(c) => c.resolve(RunMode::Development, std::path::Path::new(".")).expect_err(name).join("\n"),
        };
        println!("{name}: {why}");
        assert!(why.contains(want), "{name}: {why}");
    }
    let ok = ServerConfig::from_toml(&format!("{base}{SECURITY}")).unwrap().resolve(RunMode::Development, std::path::Path::new(".")).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(ok.security_txt.unwrap().starts_with("Contact: mailto:security@example.com\n"));
}
