// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `add_crate` and `remove_crate` end to end (operator, 2026-10-11: a session
//! that cannot reach the IDE must still be able to register the crates its
//! `EXEC RUST` blocks use).
//!
//! The registry is a `TcpListener` speaking the crates.io API shape, so the
//! add is deterministic; the resolver probe behind it is the real one — cargo
//! on the platform's own dependency graph — exactly as in the IDE's flow tests.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_project_tools::{FileList, HeadlessHost, NoProject, ProjectHost, ProjectRoot, ProjectTools};
use serde_json::{json, Value};

/// A headless host whose registry is the mock.
struct MockHost {
    inner: HeadlessHost,
    registry: String,
}

impl ProjectHost for MockHost {
    fn project(&self) -> Result<ProjectRoot, NoProject> {
        self.inner.project()
    }
    fn record(&mut self, rel: &str, list: FileList) -> Result<(), String> {
        self.inner.record(rel, list)
    }
    fn external_crates(&self) -> Vec<String> {
        self.inner.external_crates()
    }
    fn crates_registry(&self) -> String {
        self.registry.clone()
    }
    fn version(&self) -> String {
        "test".into()
    }
}

/// A minimal but real `.crate`: gzipped tar with `Cargo.toml` + `lib.rs` under
/// `mockcrate-<version>/`.
fn tarball(version: &str) -> Vec<u8> {
    fn append<W: std::io::Write>(builder: &mut tar::Builder<W>, path: &str, data: &[u8]) {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, path, data).unwrap();
    }
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let manifest = format!("[package]\nname = \"mockcrate\"\nversion = \"{version}\"\nedition = \"2021\"\n");
    append(&mut builder, &format!("mockcrate-{version}/Cargo.toml"), manifest.as_bytes());
    append(&mut builder, &format!("mockcrate-{version}/src/lib.rs"), b"pub fn answer() -> i64 { 42 }\n");
    builder.into_inner().unwrap().finish().unwrap()
}

/// The mock registry: one crate, `mockcrate` 0.1.0. Returns its base URL.
fn start_mock() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock registry");
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || handle(stream));
        }
    });
    base
}

fn handle(mut stream: TcpStream) {
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).unwrap_or(0);
    let request = String::from_utf8_lossy(&buf[..n]);
    let path = request.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("/").to_string();
    let (status, content_type, body): (&str, &str, Vec<u8>) = if path == "/api/v1/crates/mockcrate" {
        let json = json!({ "versions": [{ "num": "0.1.0", "yanked": false }] });
        ("200 OK", "application/json", json.to_string().into_bytes())
    } else if path == "/api/v1/crates/mockcrate/0.1.0/download" {
        ("200 OK", "application/gzip", tarball("0.1.0"))
    } else {
        ("404 Not Found", "application/json", b"{}".to_vec())
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(&body);
}

const MANIFEST: &str = "\
# Zap — the developer's own notes, kept at the top.
[project]
name = \"Zap\"
version = \"1.0.0\"
structure = 1
main = \"\"

[files]
sources = []
";

fn project(tag: &str, text: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("prc-crate-tools-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("Zap.project.toml");
    std::fs::write(&manifest, text).unwrap();
    (dir, manifest)
}

fn call(tools: &mut ProjectTools<impl ProjectHost>, name: &str, args: Value) -> Result<Value, String> {
    tools.call(name, &args)
}

fn dir_has(dir: &Path, rel: &str) -> bool {
    dir.join(rel).is_dir()
}

/// Both tools are on offer, with their arguments, and a call that leaves out the
/// one thing it needs says so without touching anything.
#[test]
fn the_tools_are_listed_and_a_call_without_a_name_is_refused() {
    let names: Vec<String> = ProjectTools::<HeadlessHost>::tool_list().into_iter().map(|t| t.name).collect();
    assert!(names.contains(&"add_crate".to_string()) && names.contains(&"remove_crate".to_string()), "{names:?}");
    let (dir, manifest) = project("noname", MANIFEST);
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    assert!(call(&mut tools, "add_crate", json!({})).unwrap_err().contains("needs a 'name'"));
    assert!(call(&mut tools, "remove_crate", json!({})).unwrap_err().contains("needs a 'name'"));
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), MANIFEST, "nothing was written");
}

/// Removing a registered crate deletes its pin and its vendored source and
/// nothing else — the developer's comments and the other crate stay — and an
/// unregistered or already-registered name is refused without touching a file.
#[test]
fn remove_crate_deletes_the_pin_and_its_source_only() {
    let text = format!(
        "{MANIFEST}
# keep this note with the csv crate
[[crates]]
name = \"csv\"
requirement = \"1\"
version = \"1.4.0\"
features = []
url = \"https://crates.io/crates/csv\"

[[crates]]
name = \"itoa\"
requirement = \"\"
version = \"1.0.10\"
features = []
url = \"https://crates.io/crates/itoa\"
"
    );
    let (dir, manifest) = project("remove", &text);
    std::fs::create_dir_all(dir.join("crates/csv-1.4.0")).unwrap();
    std::fs::create_dir_all(dir.join("crates/itoa-1.0.10")).unwrap();
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));

    // Already registered: refused before any network or resolver work.
    let started = Instant::now();
    let err = call(&mut tools, "add_crate", json!({ "name": "CSV" })).unwrap_err();
    assert!(err.contains("already registered"), "{err}");
    assert!(started.elapsed().as_secs() < 5, "a cheap refusal must not run the resolver");

    let v = call(&mut tools, "remove_crate", json!({ "name": "csv" })).unwrap();
    assert_eq!(v["removed"], "csv");
    let left: Vec<&str> = v["registered"].as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(left, ["itoa"]);
    assert!(!dir_has(&dir, "crates/csv-1.4.0"), "the vendored source is gone");
    assert!(dir_has(&dir, "crates/itoa-1.0.10"), "the other crate's source stays");

    let after = std::fs::read_to_string(&manifest).unwrap();
    assert!(after.starts_with("# Zap — the developer's own notes"), "{after}");
    assert!(!after.contains("name = \"csv\""), "{after}");
    assert!(after.contains("name = \"itoa\""), "{after}");

    let err = call(&mut tools, "remove_crate", json!({ "name": "csv" })).unwrap_err();
    assert!(err.contains("not registered"), "{err}");
    println!("remove_crate: csv pin + source removed, itoa kept, comment kept, 2 refusals (already registered, not registered)");
}

/// The whole of the report: a crate is registered from the registry with no IDE
/// — its source is vendored under crates/, the pin lands in the manifest with
/// the registry URL it came from, every other byte of the manifest is kept, the
/// project then lists the crate for `use`, and removing it undoes all of it.
#[test]
fn add_crate_registers_a_crate_with_no_ide_and_remove_crate_undoes_it() {
    let started = Instant::now();
    let base = start_mock();
    let (dir, manifest) = project("add", MANIFEST);
    let mut tools = ProjectTools::new(MockHost { inner: HeadlessHost::new(&dir, "test"), registry: base.clone() });

    let t = Instant::now();
    let v = call(&mut tools, "add_crate", json!({ "name": "mockcrate" })).unwrap_or_else(|e| panic!("add_crate: {e}"));
    let add_s = t.elapsed().as_secs_f32();
    assert_eq!(v["added"], "mockcrate");
    assert_eq!(v["version"], "0.1.0");
    assert_eq!(v["use_as"], "mockcrate");
    assert_eq!(v["vendored"], "crates/mockcrate-0.1.0");
    assert_eq!(v["registered"].as_array().unwrap().len(), 1);
    assert!(v["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("resolver")), "{v}");
    assert!(dir_has(&dir, "crates/mockcrate-0.1.0"), "the source is vendored");

    let written = std::fs::read_to_string(&manifest).unwrap();
    assert!(written.starts_with(MANIFEST), "every byte the tool does not own is kept:\n{written}");
    assert!(written.contains("name = \"mockcrate\""), "{written}");
    assert!(written.contains(&format!("url = \"{base}/crates/mockcrate\"")), "the pin names the registry it came from:\n{written}");
    assert_eq!(cobolt_compiler::project_manifest_view(&manifest).unwrap().crates, ["mockcrate"], "the project now links it for `use`");

    // A second add is refused cheaply, and the manifest is not rewritten.
    let err = call(&mut tools, "add_crate", json!({ "name": "mockcrate" })).unwrap_err();
    assert!(err.contains("already registered"), "{err}");
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), written);

    // An unknown crate is refused with the registry's own words, nothing left behind.
    let err = call(&mut tools, "add_crate", json!({ "name": "no-such-crate" })).unwrap_err();
    assert!(err.contains("does not know a crate named"), "{err}");
    assert!(!dir_has(&dir, "crates/no-such-crate-0.1.0"));

    let v = call(&mut tools, "remove_crate", json!({ "name": "mockcrate" })).unwrap();
    assert_eq!(v["registered"].as_array().unwrap().len(), 0);
    assert!(!dir_has(&dir, "crates/mockcrate-0.1.0"), "the source is gone");
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), MANIFEST, "the manifest is back byte for byte");

    println!("──────────────────────────────────────────────");
    println!("add_crate / remove_crate, no IDE");
    println!("cases: add from a mock registry (resolver probe real); pin + URL in manifest;");
    println!("       manifest bytes kept; project lists the crate; second add refused;");
    println!("       unknown crate refused; remove restores the manifest byte for byte");
    println!("add (incl. system closure + resolver probe): {add_s:.1} s");
    println!("total: {:.1} s", started.elapsed().as_secs_f32());
    println!("──────────────────────────────────────────────");
    let _ = std::fs::remove_dir_all(&dir);
}
