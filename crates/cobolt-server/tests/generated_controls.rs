// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-005 T11: a class C control's method, run on the backend.
//!
//! The Build's program for `REST-1::Get("customers")` on a web form is served
//! over a real TLS loopback socket; the RestClient it drives is seeded from the
//! program's declaration with `backend.toml`'s `[[configuration]] CRM` laid
//! over it — the address of a local HTTP stub and a token read from a file.
//! The stub sees the token; the browser's answer never carries it.

mod support;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use cobolt_codegen::remote_controls::{fixtures, generate_control_programs, generate_slots, GeneratedBackend};
use cobolt_server::backend::Backend;
use cobolt_server::config::{RunMode, ServerConfig};
use serde_json::Value;
use support::*;

const EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n";
const TOKEN: &str = "s3cret-crm-token-0042";

/// A one-shot-per-connection HTTP server on loopback: answers every request
/// with `body` and keeps each request's first line and `Authorization` header.
fn http_stub(body: &'static str) -> (std::net::SocketAddr, Arc<Mutex<Vec<(String, String)>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            let _ = reader.read_line(&mut first);
            let mut auth = String::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                    break;
                }
                if let Some((k, v)) = line.split_once(':') {
                    if k.trim().eq_ignore_ascii_case("authorization") {
                        auth = v.trim().to_owned();
                    }
                }
            }
            seen2.lock().unwrap().push((first.trim().to_owned(), auth));
            let mut out = stream;
            let _ = write!(
                out,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (addr, seen)
}

/// The fixture form's method programs in a backend folder's `generated/`.
fn backend_with_controls(name: &str) -> (std::path::PathBuf, GeneratedBackend) {
    let dir = backend_dir(name, EXPORTS, &[]);
    let (form, sites) = fixtures::control_form();
    let g = GeneratedBackend {
        programs: generate_control_programs(&form, &sites).expect("generates"),
        slots: generate_slots(&form).expect("slots"),
    };
    let out = dir.join("generated");
    std::fs::create_dir_all(&out).unwrap();
    for p in &g.programs {
        std::fs::write(out.join(&p.file_name), &p.source).unwrap();
    }
    std::fs::write(out.join("exports.toml"), g.exports_toml()).unwrap();
    (dir, g)
}

fn configurations(dir: &std::path::Path, base_url: &str) -> String {
    let token_file = dir.join("crm.token");
    std::fs::write(&token_file, format!("{TOKEN}\n")).unwrap();
    format!(
        "[[configuration]]\nname = \"CRM\"\nBaseURL = \"{base_url}\"\nAuthToken = \"file:{}\"\n\n[[configuration]]\nname = \"ASSISTANT\"\nAgentAPIKey = \"env:PR_T11_UNSET_KEY_IS_NEVER_READ_HERE\"\n",
        token_file.display()
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rest_client_call_runs_on_the_backend_with_the_configuration_read_there() {
    let (addr, seen) = http_stub(r#"{"customers":["Ana","Bruno"]}"#);
    let (dir, g) = backend_with_controls("t11-rest");
    // The environment key of ASSISTANT is unset on purpose: a configuration
    // the call does not use must still resolve, so it is given by file too.
    let extra = configurations(&dir, &format!("http://{addr}/v1")).replace(
        "AgentAPIKey = \"env:PR_T11_UNSET_KEY_IS_NEVER_READ_HERE\"",
        &format!("AgentAPIKey = \"file:{}\"", dir.join("crm.token").display()),
    );
    let rig = rig(dir, &extra);
    let token = rig.issuer.access("clerk-7", AUDIENCE, 600, serde_json::json!({ "roles": ["clerk"] }));
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    assert_eq!(c.hello("1.0.0").await["accepted"], "ok");
    assert_eq!(c.auth(&token).await["t"], "auth-ok");

    let program = &g.programs[0].program_id;
    assert_eq!(program, "RC-4-SHOP-6-REST-1-GET-1");
    let started = std::time::Instant::now();
    let r = c.call(1, program, &[("value", r#"{"ARG-COUNT":0,"ARGS":[]}"#)], true).await;
    let took = started.elapsed();
    assert_eq!(r["t"], "reply", "{r}");
    let ret: &Value = &r["returning"];

    // The stub was reached at the configured address, with the configured token.
    let requests = seen.lock().unwrap().clone();
    assert_eq!(requests, vec![("GET /v1/customers HTTP/1.1".to_owned(), format!("Bearer {TOKEN}"))]);

    // The answer: the events the desktop raises, the properties the call set.
    let events: Vec<&str> = ret["EVENTS"].as_array().unwrap().iter().map(|e| e["EVENT-NAME"].as_str().unwrap()).collect();
    // An async RestClient (the default Mode) raises onComplete on the desktop.
    assert_eq!(events, vec!["onComplete"], "{ret}");
    let prop = |name: &str| {
        ret["PROPS"].as_array().unwrap().iter().find(|p| p["PROP-NAME"] == name).map(|p| p["PROP-VALUE"].as_str().unwrap().to_owned())
    };
    assert_eq!(prop("RESPONSEBODY").as_deref(), Some(r#"{"customers":["Ana","Bruno"]}"#), "{ret}");
    assert_eq!(prop("STATUSCODE").as_deref(), Some("200"));
    assert_eq!(ret["LAST-ERROR"], "");
    // Neither the token nor the configured address goes back to the browser.
    let text = r.to_string();
    assert!(!text.contains(TOKEN), "{text}");
    assert!(prop("AUTHTOKEN").is_none() && prop("BASEURL").is_none());
    let log = format!("{:?}", rig.sink.lines());
    assert!(!log.contains(TOKEN), "the log never carries the token");
    println!(
        "GET through the backend in {} ms; events {events:?}; {} properties back: {}",
        took.as_millis(),
        ret["PROP-COUNT"],
        ret["PROPS"].as_array().unwrap().iter().map(|p| p["PROP-NAME"].as_str().unwrap()).collect::<Vec<_>>().join(", ")
    );

    // The browser sends no resource: an argument the program does not take is refused.
    let r = c.call(2, program, &[("value", r#"{"ARG-COUNT":1,"ARGS":[{"ARG-VALUE":"../admin"}]}"#)], true).await;
    assert_eq!(class(&r), "application", "{r}");
    assert_eq!(seen.lock().unwrap().len(), 1, "nothing was requested for the refused call");
    rig.server.stop();
}

#[test]
fn a_named_configuration_that_backend_toml_lacks_stops_the_start() {
    let (dir, _) = backend_with_controls("t11-missing");
    let cert = test_cert();
    std::fs::write(dir.join("c.pem"), &cert.cert_pem).unwrap();
    std::fs::write(dir.join("k.pem"), &cert.key_pem).unwrap();
    let text = format!(
        "[server]\nlisten = \"127.0.0.1:0\"\nallowed_origins = [\"{ORIGIN}\"]\n[tls]\ncert_chain = \"c.pem\"\nprivate_key = \"k.pem\"\n[auth]\nissuer = [\"https://idp.example.test/pool\"]\naudiences = [\"{AUDIENCE}\"]\n\n[[configuration]]\nname = \"CRM\"\nBaseURL = \"http://127.0.0.1:9/v1\"\n"
    );
    let resolved = ServerConfig::from_toml(&text).unwrap().resolve(RunMode::Release, &dir).unwrap();
    let backend = Backend::load(&dir).unwrap_or_else(|e| panic!("{e:#?}"));
    let e = cobolt_server::start(resolved, backend, cobolt_server::Options { sink: None, fetcher: None }).err().expect("refused");
    assert!(e.contains("no [[configuration]] named \"ASSISTANT\""), "{e}");
    assert!(e.contains("RC-4-SHOP-7-AGENT-1-ASK-1"), "{e}");
    assert!(!e.contains("CRM\""), "CRM is there: {e}");
    println!("{e}");
}

#[test]
fn configurations_resolve_their_values_and_never_print_them() {
    let dir = std::env::temp_dir().join(format!("prc-t11-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let text = configurations(&dir, "https://crm.example.com/api")
        .replace("env:PR_T11_UNSET_KEY_IS_NEVER_READ_HERE", "plain-key-in-the-file");
    let cfg = ServerConfig::from_toml(&format!("unsafe_dev_token_secret = \"0123456789abcdef0123\"\n{text}")).unwrap();
    assert!(!format!("{cfg:?}").contains("plain-key-in-the-file"), "a configuration prints its names only");
    let r = cfg.resolve(RunMode::Development, &dir).unwrap_or_else(|e| panic!("{e:#?}"));
    let crm = r.configurations.get("crm").expect("names ignore case");
    let get = |k: &str| crm.iter().find(|(n, _)| n == k).map(|(_, v)| v.expose().to_owned());
    assert_eq!(get("AuthToken").as_deref(), Some(TOKEN), "file: is read, trimmed");
    assert_eq!(get("BaseURL").as_deref(), Some("https://crm.example.com/api"));
    assert!(!format!("{r:?}").contains(TOKEN), "nor does the resolved configuration");
    // Twice the same name, an empty name, an unreadable value: each stops the start.
    let bad = "[[configuration]]\nname = \"CRM\"\n[[configuration]]\nname = \"crm\"\n[[configuration]]\nname = \" \"\n[[configuration]]\nname = \"X\"\nApiKey = \"env:PR_T11_SURELY_UNSET_VARIABLE\"\n";
    let errs = ServerConfig::from_toml(&format!("unsafe_dev_token_secret = \"0123456789abcdef0123\"\n{bad}")).unwrap().resolve(RunMode::Development, &dir).unwrap_err();
    let errs = errs.join("\n");
    assert!(errs.contains("configuration \"crm\" is given twice"), "{errs}");
    assert!(errs.contains("an entry has an empty name"), "{errs}");
    assert!(errs.contains("configuration \"X\" ApiKey: the environment variable PR_T11_SURELY_UNSET_VARIABLE is not set"), "{errs}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_the_builds_exports_may_declare_a_control() {
    let (dir, _) = backend_with_controls("t11-mine");
    std::fs::write(
        dir.join("exports.toml"),
        format!("{EXPORTS}\n[[export]]\nprogram = \"MINE\"\n\n[export.control]\nid = \"REST-9\"\nclass = \"RestClient\"\n"),
    )
    .unwrap();
    let e = Backend::load(&dir).err().expect("refused").join("\n");
    assert!(e.contains("export MINE declares a control; only the Build's generated/exports.toml does"), "{e}");
    // Nor an upload slot (T12): the deployment's go in backend.toml.
    std::fs::write(dir.join("exports.toml"), format!("{EXPORTS}\n[[slot]]\nname = \"MINE\"\n")).unwrap();
    let e = Backend::load(&dir).err().expect("refused").join("\n");
    assert!(e.contains("exports.toml declares an upload slot"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── T12: a FileDropZone's slot ───────────────────────────────────────────────

fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// `put-begin` for `name`, then — when the backend is ready — the bytes and `put-end`.
async fn upload(c: &mut Raw, id: u64, slot: &str, name: &str, data: &[u8], mime: &str) -> Value {
    use base64::Engine;
    c.send(serde_json::json!({"t": "put-begin", "id": id, "slot": slot, "name": name, "size": data.len(), "mime": mime, "sha256": sha256_hex(data)})).await;
    let r = c.next().await.expect("put-begin answer");
    if r["t"] != "put-ready" {
        return r;
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    c.send(serde_json::json!({"t": "put-chunk", "id": id + 1, "offset": 0, "data": b64})).await;
    let r = c.next().await.expect("put-chunk answer");
    if r["t"] != "put-ready" {
        return r;
    }
    c.send(serde_json::json!({"t": "put-end", "id": id + 2})).await;
    c.next().await.expect("put-end answer")
}

/// Every file under `dir`, relative.
fn files_under(dir: &std::path::Path) -> Vec<String> {
    fn walk(base: &std::path::Path, d: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_drop_zones_slot_takes_what_the_zone_takes_and_nothing_else() {
    let (dir, g) = backend_with_controls("t12-slot");
    let slot = &g.slots[0].name;
    assert_eq!(slot, "RC-4-SHOP-6-DROP-1-COMMITFILES");
    let extra = format!("{}\n[files]\ndir = \"uploads\"\n", configurations(&dir, "http://127.0.0.1:9/v1"))
        .replace("env:PR_T11_UNSET_KEY_IS_NEVER_READ_HERE", "plain-key");
    let before = files_under(&dir);
    let rig = rig(dir.clone(), &extra);
    let clerk = rig.issuer.access("clerk-7", AUDIENCE, 600, serde_json::json!({ "roles": ["clerk"] }));
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    assert_eq!(c.hello("1.0.0").await["accepted"], "ok");
    assert_eq!(c.auth(&clerk).await["t"], "auth-ok");

    // An allowed type within the limit is stored, under the backend's own name.
    let csv = b"id,name\n1,Ana\n2,Bruno\n";
    let r = upload(&mut c, 10, slot, "Customers.CSV", csv, "text/csv").await;
    assert_eq!(r["t"], "put-done", "{r}");
    let file = r["file"].as_str().unwrap().to_owned();
    assert_eq!(r["size"], csv.len());
    let stored = dir.join("uploads").join("files").join(&file);
    assert_eq!(std::fs::read(&stored).unwrap(), csv);

    // A type the zone does not take, and a file over its 64 KB, are refused before a byte is kept.
    let r = upload(&mut c, 20, slot, "setup.exe", b"MZ....", "application/octet-stream").await;
    assert_eq!((class(&r).as_str(), code(&r)), ("validation", 4000), "{r}");
    assert!(r.to_string().contains("takes only csv, xlsx files"), "{r}");
    let big = vec![b'x'; 64 * 1024 + 1];
    let r = upload(&mut c, 30, slot, "big.csv", &big, "text/csv").await;
    assert_eq!((class(&r).as_str(), code(&r)), ("limit", 7004), "{r}");
    // A name that is a path is refused: nothing is written outside the slot's store.
    for name in ["../escape.csv", "sub/dir.csv", "..\\win.csv", "/etc/passwd.csv"] {
        let r = upload(&mut c, 40, slot, name, csv, "text/csv").await;
        assert_eq!(class(&r), "validation", "{name}: {r}");
    }
    // The zone's BackendRoles: another caller may not use its slot.
    let other = rig.issuer.access("guest-1", AUDIENCE, 600, serde_json::json!({}));
    let mut c2 = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    assert_eq!(c2.hello("1.0.0").await["accepted"], "ok");
    assert_eq!(c2.auth(&other).await["t"], "auth-ok");
    let r = upload(&mut c2, 50, slot, "ok.csv", csv, "text/csv").await;
    assert_eq!(class(&r), "authorisation", "{r}");
    rig.server.stop();

    // What changed on disk: the one stored file and its record, and nothing else.
    let after: Vec<String> = files_under(&dir).into_iter().filter(|p| !before.contains(p)).collect();
    let kept: Vec<String> =
        after.into_iter().filter(|p| !p.ends_with(".pem") && !p.ends_with(".token") && !p.starts_with("uploads/incoming/")).collect();
    assert_eq!(kept, vec![format!("uploads/files/{file}"), format!("uploads/files/{file}.meta")]);
    println!("slot {slot}: 1 stored ({} bytes), .exe refused, {} bytes refused, 4 path names refused, a guest refused", csv.len(), big.len());
}
