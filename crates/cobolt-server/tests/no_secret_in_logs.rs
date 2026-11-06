// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! No token, password or secret in the server's log (WASM-013 R13, T18; AC10). A **development** run signs a
//! user in with a development token (`unsafe_dev_token_secret`, HS256) and calls a program with a planted
//! password field; a second client presents a token that is refused; a call to a program that is not exported
//! fails. Every line the `MemorySink` holds is then searched for the token (whole, and each of its three
//! parts), the refused token, the password and the development secret itself. The log still says who did what.

use cobolt_server::dev::mint_dev_token;
use cobolt_server::testkit::*;

const EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"GET-CUSTOMER\"\n";
const GET_CUSTOMER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. GET-CUSTOMER.
DATA DIVISION.
LINKAGE SECTION.
01 LK-REQ.
    05 CUST-ID   PIC 9(6).
    05 PASSWORD  PIC X(20).
01 LK-RES.
    05 RC  PIC 99.
PROCEDURE DIVISION USING LK-REQ RETURNING LK-RES.
MAIN.
    MOVE 7 TO RC
    GOBACK.
"#;

const DEV_SECRET: &str = "dev-only-secret-for-this-test-0123456789";
const PASSWORD: &str = "Pl4nted-Passw0rd!";

#[tokio::test(flavor = "multi_thread")]
async fn a_development_sign_in_and_a_call_with_a_password_leave_neither_in_the_log() {
    let rig = rig_dev(backend_dir("nosecret", EXPORTS, &[("get-customer.cbl", GET_CUSTOMER)]), DEV_SECRET);
    let token = mint_dev_token(DEV_SECRET, "dev-user", &[], 600);

    // A user signs in and calls a program with a password in the payload.
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    assert_eq!(c.hello("1.0.0").await["accepted"], "ok");
    let a = c.auth(&token).await;
    assert_eq!(a["t"], "auth-ok", "the development token signs in on a development run: {a}");
    let req = format!(r#"{{"CUST-ID":4250,"PASSWORD":"{PASSWORD}"}}"#);
    let r = c.call(1, "GET-CUSTOMER", &[("ref", &req)], true).await;
    assert_eq!(r["t"], "reply", "{r}");
    // A call to a program that is not exported fails, with the payload in hand.
    let r = c.call(2, "NOT-EXPORTED", &[("ref", &req)], false).await;
    assert_eq!(r["t"], "error", "{r}");

    // A second client presents a token with a wrong signature: refused.
    let forged = format!("{}x", token);
    let mut c2 = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    assert_eq!(c2.hello("1.0.0").await["accepted"], "ok");
    let refused = c2.auth(&forged).await;
    assert_ne!(refused["t"], "auth-ok", "a token with a bad signature must not sign in: {refused}");

    let lines = rig.sink.lines();
    let log = lines.join("\n");
    let mut needles: Vec<(String, String)> = vec![("the token".into(), token.clone()), ("the refused token".into(), forged.clone()), ("the password".into(), PASSWORD.to_owned()), ("the development secret".into(), DEV_SECRET.to_owned())];
    for (i, part) in token.split('.').enumerate() {
        needles.push((format!("part {} of the token", i + 1), part.to_owned()));
    }
    let mut hits = 0;
    for (what, needle) in &needles {
        if log.contains(needle.as_str()) {
            hits += 1;
            println!("FOUND in the log: {what}");
        }
    }
    assert_eq!(hits, 0, "a secret reached the log:\n{log}");
    assert!(log.contains("\"call\"") && log.contains("dev-user") && log.contains("GET-CUSTOMER") && log.contains("duration_ms"), "the log still says who did what: {log}");
    println!("no secret in logs (server): {} records, {} bytes searched for {} strings (token, its 3 parts, refused token, password, development secret): {} hits", lines.len(), log.len(), needles.len(), hits);
    rig.server.stop();
}
