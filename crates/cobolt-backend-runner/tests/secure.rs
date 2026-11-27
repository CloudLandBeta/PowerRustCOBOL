// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T44 (R21, AC11): the module path weakens no protection of WASM-013.
//!
//! 1. A secret a project keeps in `backend/backend.toml` or in the process environment never reaches anything a Build
//!    writes: not the file it installs (the runner with the module inside), not its bill of materials, not the interface,
//!    not the staging folder the module was made in. The same scan finds the value in the one place it is meant to be, so the
//!    scan is shown to work.
//! 2. The file a Build makes refuses a setting a release refuses (`unsafe_*`), with the setting named and exit code 2, and
//!    accepts the same configuration without it. (`executable.rs` holds the same cases equal to a Cargo-built file's.)
//! 3. A module's imports are what its manifest declares: a module that imports a native function whose capability it did
//!    not declare is refused at load, naming the import and the capability, however the manifest came to say less.
//!
//! No fixture copies an example form: only a Backend's own folder is copied, and every planted value is built from
//! fragments, so this file holds none for a repository scan to find.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_backend_runner::payload::{load, Package};
use cobolt_compiler::backend_build::{build_backend, dev_module};
use cobolt_compiler::backend_module::{backend_container, BackendPath};
use cobolt_server::testkit::backend_dir;
use cobolt_wasm::{compile_backend, BackendProgram, Options};

mod common;
use common::{one_build_at_a_time, options, project, run, runner_with_sbom};

/// AWS's documented example key, from fragments: a recognisable fake.
fn planted_key() -> String {
    format!("A{}{}", "KIA", "IOSFODNN7EXAMPLE")
}

/// A value a secrets file or the environment would carry, from fragments.
fn planted_env() -> String {
    format!("{}{}", "needle-for-", "the-environment-0042")
}

/// Every file under `dir` with its bytes (the absolute path as the name).
fn files(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(at: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let Ok(read) = std::fs::read_dir(at) else { return };
        for e in read.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if let Ok(bytes) = std::fs::read(&p) {
                out.push((p, bytes));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out
}

fn holds(haystack: &[u8], needle: &str) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle.as_bytes())
}

#[test]
fn a_secret_in_backend_toml_or_the_environment_never_reaches_what_a_build_writes() {
    let _one = one_build_at_a_time();
    let started = Instant::now();
    let (key, value) = (planted_key(), planted_env());
    let var = format!("PRC_SECURE_PROBE_{}", std::process::id());
    std::env::set_var(&var, &value);

    let runner = runner_with_sbom(&std::env::temp_dir().join(format!("prc-backend-secure-runner-{}", std::process::id())));
    let dir = project("PowerWebHello", "secure");
    let toml = dir.join("backend").join("backend.toml");
    std::fs::write(
        &toml,
        format!("[server]\nlisten = \"127.0.0.1:8443\"\n[tls]\npkcs12 = \"backend.p12\"\npkcs12_password = \"{key}\"\n[[configuration]]\nname = \"CRM\"\nAuthToken = \"env:{var}\"\n"),
    )
    .unwrap();
    let secrets = dir.join("secrets.env");
    std::fs::write(&secrets, format!("{var}={value}\n")).unwrap();

    let built = build_backend(&dir, &options(&runner, Some(BackendPath::Wasm))).unwrap_or_else(|e| panic!("the Backend builds: {e}"));
    let staging = cobolt_compiler::build_staging_dir(None, "powerwebhello-backend");

    // Everything the Build wrote: the project's folders and the staging folder, but for the files that hold a secret by design.
    let mut seen: Vec<(PathBuf, Vec<u8>)> = files(&dir);
    seen.extend(files(&staging));
    let own = [toml.clone(), secrets.clone()];
    let (mut scanned, mut bytes, mut passed, mut failed) = (0usize, 0usize, 0u32, 0u32);
    let mut check = |ok: bool, what: String| {
        if ok {
            passed += 1;
        } else {
            failed += 1;
            eprintln!("FAIL {what}");
        }
    };
    // The scan works: it finds each value where the project keeps it.
    check(holds(&std::fs::read(&toml).unwrap(), &key), "the scan finds the key in backend.toml".into());
    check(holds(&std::fs::read(&secrets).unwrap(), &value), "the scan finds the value in secrets.env".into());
    for (path, content) in &seen {
        if own.contains(path) {
            continue;
        }
        scanned += 1;
        bytes += content.len();
        check(!holds(content, &key), format!("{} holds the key of backend.toml", path.display()));
        check(!holds(content, &value), format!("{} holds the environment's value", path.display()));
    }
    check(seen.iter().any(|(p, _)| *p == built.binary), "the file the Build installed was among those scanned".into());
    let installed = std::fs::read(&built.binary).unwrap();
    check(!holds(&installed, &key) && !holds(&installed, &value), "the installed file holds neither value".into());
    println!(
        "a secret in backend.toml and one in the environment: {scanned} files, {bytes} bytes scanned after a module-path Build ({} bytes installed): neither value in any of them, both found where the project keeps them; PASS {passed:03} FAIL {failed:03} ({} ms)",
        installed.len(),
        started.elapsed().as_millis()
    );
    std::env::remove_var(&var);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    assert_eq!(failed, 0, "{failed} checks failed");
}

#[test]
fn the_file_a_build_makes_refuses_an_unsafe_setting_naming_it_and_accepts_the_same_configuration_without_it() {
    let _one = one_build_at_a_time();
    let started = Instant::now();
    let runner = runner_with_sbom(&std::env::temp_dir().join(format!("prc-backend-secure-unsafe-{}", std::process::id())));
    let dir = project("PowerWebHello", "unsafe");
    let built = build_backend(&dir, &options(&runner, Some(BackendPath::Wasm))).unwrap_or_else(|e| panic!("the Backend builds: {e}"));

    let base = "[server]\nlisten = \"127.0.0.1:8443\"\nallowed_origins = [\"https://app.example.test\"]\n";
    let tail = "[auth]\nissuer = [\"https://idp.example.test/pool\"]\naudiences = [\"prc-api\"]\n";
    let cases: [(&str, String, Option<i32>, &str); 4] = [
        ("accepted as it is", format!("{base}{tail}"), Some(0), "configuration ok"),
        ("unsafe_allow_any_origin", format!("{base}unsafe_allow_any_origin = true\n{tail}"), Some(2), "unsafe_allow_any_origin"),
        ("unsafe_plain_ws (removed)", format!("{base}unsafe_plain_ws = true\n{tail}"), Some(2), "unsafe_plain_ws"),
        ("a non-loopback address with no TLS", format!("[server]\nlisten = \"0.0.0.0:8443\"\nallowed_origins = [\"https://app.example.test\"]\n{tail}"), Some(2), "not a loopback address"),
    ];
    let (mut passed, mut failed) = (0u32, 0u32);
    for (what, config, code, says) in &cases {
        let file = dir.join(format!("case-{}.toml", what.split_whitespace().next().unwrap_or("x")));
        std::fs::write(&file, config).unwrap();
        let ran = run(&built.binary, &dir, &["--check", "--config", file.to_str().unwrap()]);
        let said = format!("{}{}", ran.stdout, ran.stderr);
        let ok = ran.code == *code && said.contains(says);
        if ok {
            passed += 1;
        } else {
            failed += 1;
            eprintln!("FAIL {what}: exit {:?} (wanted {code:?}), output {said:?}", ran.code);
        }
    }
    println!(
        "the Build-made file under --check: {} configurations, the three a release refuses exit 2 naming the reason and the plain one accepted; PASS {passed:03} FAIL {failed:03} ({} ms)",
        cases.len(),
        started.elapsed().as_millis()
    );
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    assert_eq!(failed, 0, "{failed} cases failed");
}

/// The module with `capability` taken out of what its manifest declares, every import untouched.
fn without_declared(wasm: &[u8], capability: Option<&str>) -> Vec<u8> {
    fn read(bytes: &[u8], at: &mut usize) -> usize {
        let (mut v, mut shift) = (0usize, 0u32);
        loop {
            let b = bytes[*at];
            *at += 1;
            v |= usize::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return v;
            }
            shift += 7;
        }
    }
    fn put(out: &mut Vec<u8>, mut v: usize) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                return;
            }
            out.push(b | 0x80);
        }
    }
    let mut out = wasm[..8].to_vec();
    let mut at = 8;
    while at < wasm.len() {
        let id = wasm[at];
        at += 1;
        let size = read(wasm, &mut at);
        let body = &wasm[at..at + size];
        at += size;
        if id == 0 {
            let mut p = 0;
            let n = read(body, &mut p);
            if &body[p..p + n] == cobolt_wasm_abi::manifest::MANIFEST_SECTION.as_bytes() {
                let mut manifest: serde_json::Value = serde_json::from_slice(&body[p + n..]).expect("the manifest is JSON");
                let list = manifest["capabilities"].as_array_mut().expect("the manifest lists capabilities");
                match capability {
                    Some(c) => list.retain(|d| d != c),
                    None => list.clear(),
                }
                let mut section = Vec::new();
                put(&mut section, n);
                section.extend_from_slice(cobolt_wasm_abi::manifest::MANIFEST_SECTION.as_bytes());
                section.extend_from_slice(&serde_json::to_vec(&manifest).unwrap());
                out.push(0);
                put(&mut out, section.len());
                out.extend_from_slice(&section);
                continue;
            }
        }
        out.push(id);
        put(&mut out, size);
        out.extend_from_slice(body);
    }
    out
}

fn declared(wasm: &[u8]) -> Vec<String> {
    let scan = cobolt_wasm_abi::scan::scan_module(wasm).expect("the module scans");
    let text = scan.custom(cobolt_wasm_abi::manifest::MANIFEST_SECTION).expect("the manifest section");
    let manifest: serde_json::Value = serde_json::from_slice(text).unwrap();
    manifest["capabilities"].as_array().unwrap().iter().filter_map(|c| c.as_str().map(str::to_string)).collect()
}

fn native_imports(wasm: &[u8]) -> Vec<String> {
    let scan = cobolt_wasm_abi::scan::scan_module(wasm).expect("the module scans");
    scan.imports.iter().filter(|i| i.module == cobolt_wasm_abi::codes::NATIVE_IMPORT_MODULE).map(|i| i.name.clone()).collect()
}

#[test]
fn a_module_that_imports_what_its_manifest_does_not_declare_is_refused_at_load() {
    let started = Instant::now();
    let provided = cobolt_backend_runner::provided();
    let (mut passed, mut failed) = (0u32, 0u32);
    let mut check = |ok: bool, what: String| {
        if ok {
            passed += 1;
        } else {
            failed += 1;
            eprintln!("FAIL {what}");
        }
    };

    // 1. A module that reaches a database, from source: it declares `sql` and imports the SQL object's function.
    let source = "IDENTIFICATION DIVISION.\nPROGRAM-ID. ADD-TWO.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-N PIC 9(4).\nLINKAGE SECTION.\n01 REQ.\n    05 A PIC 9(4).\n01 RES.\n    05 TOTAL PIC 9(5).\nPROCEDURE DIVISION USING REQ RETURNING RES.\nMAIN-PARA.\n    MOVE COBOL::SqlDatabase::Open(\":memory:\") TO WS-N\n    COBOL::SqlDatabase::Close()\n    GOBACK.\n";
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(source, cobolt_lexer::SourceFormat::Free));
    let program = parsed.program.expect("a program");
    let wasm = compile_backend(&[BackendProgram { name: "ADD-TWO".into(), program: &program }], &Options { native: true, ..Options::default() }).unwrap().wasm;
    let dir = backend_dir("secure-sql", "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"ADD-TWO\"\n", &[("add.cbl", source)]);
    let backend = cobolt_server::backend::Backend::load(&dir).unwrap();
    let package = |module: &[u8]| Package { container: backend_container(&backend, module, backend.state, "Secure").unwrap(), code: None };
    check(declared(&wasm) == ["sql"], format!("the module declares exactly `sql`: {:?}", declared(&wasm)));
    check(native_imports(&wasm).iter().any(|n| n == "sql_obj"), format!("and imports `prn.sql_obj`: {:?}", native_imports(&wasm)));
    check(load(package(&wasm), &provided).is_ok(), "the module as the generator made it loads".into());
    let tampered = without_declared(&wasm, Some("sql"));
    check(declared(&tampered).is_empty() && native_imports(&tampered) == native_imports(&wasm), "the manifest says less and the imports are the same".into());
    let refused = load(package(&tampered), &provided).err().map(|e| e.join("\n")).unwrap_or_default();
    check(refused.contains("prn.sql_obj") && refused.contains("sql"), format!("refused naming the import and the capability: {refused:?}"));

    // 2. The shipped PowerOficina module (files, a PartiQL and a SORT): as made it loads; with its manifest emptied it is refused.
    let oficina = project("PowerOficina", "secure-oficina");
    let made = dev_module(&oficina).unwrap_or_else(|e| panic!("PowerOficina has a module: {e:?}"));
    let package = |module: &[u8]| Package { container: backend_container(&made.backend, module, made.backend.state, "PowerOficina").unwrap(), code: None };
    let (caps, imports) = (declared(&made.compiled.wasm), native_imports(&made.compiled.wasm));
    check(!imports.is_empty(), "PowerOficina's module has native imports to hold against its manifest".into());
    check(load(package(&made.compiled.wasm), &provided).is_ok(), "PowerOficina's module as the generator made it loads".into());
    let emptied = without_declared(&made.compiled.wasm, None);
    let refused = load(package(&emptied), &provided).err().map(|e| e.join("\n")).unwrap_or_default();
    check(refused.contains("prn.") && refused.contains("does not declare"), format!("with no capability declared it is refused naming an import: {refused:?}"));
    let _ = std::fs::remove_dir_all(oficina.parent().unwrap());

    println!(
        "imports against the manifest: a database module ({} declared, {} native imports) and PowerOficina ({} declared, {} native imports) load as made and are refused with the declarations taken out, naming the import; PASS {passed:03} FAIL {failed:03} ({} ms)",
        declared(&wasm).len(),
        native_imports(&wasm).len(),
        caps.len(),
        imports.len(),
        started.elapsed().as_millis()
    );
    assert_eq!(failed, 0, "{failed} checks failed");
}
