// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The dependency guard of the generator's host (WASM-010 WP1, T2).
//!
//! `cobolt-wasm` with its `host` feature is what the tests, the NIST gate and the IDE's Build
//! (through `cobolt-project-tools`) link. This test walks that tree - normal and build
//! dependencies, every target platform, as cargo resolves it - and fails on
//!
//! * a package whose **build script** has a C compiler or a system-library finder among its build
//!   dependencies (`cc`, `cmake`, `bindgen`, `pkg-config`, ...), except the one the operator
//!   allowed on 2026-10-22: `wasmtime`'s `helpers.c`, compiled with `cc`;
//! * a licence outside the permissive set, or a minimum Rust version above 1.92.
//!
//! The guard is a function over data, in the shape of `cobolt-graphics/tests/dependency_guard.rs`,
//! and **it is tested to fail**: a real cargo project with a planted package whose build script
//! build-depends on a stub `cc` is reported, and the same project without it is clean.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// What a build script needs in order to compile C or C++ or to find a system library.
const NATIVE_BUILD_TOOLS: &[&str] = &["cc", "cmake", "bindgen", "pkg-config", "vcpkg", "system-deps", "metadeps"];

/// The allowed native build scripts: `(package, helper, why)`. Nothing else may compile C.
const ALLOWED_NATIVE: &[(&str, &str, &str)] = &[(
    "wasmtime",
    "cc",
    "the `runtime` feature compiles src/runtime/vm/helpers.c (operator, 2026-10-22, WASM-010 plan WP1 choice 1)",
)];

/// Licences compatible with Apache-2.0 and permissive.
const ALLOWED_LICENCES: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "MPL-2.0",
    "CC0-1.0",
    "0BSD",
    "BSL-1.0",
    "Unlicense",
    "LLVM-exception",
];

const MAX_MSRV: (u32, u32) = (1, 92);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Pkg {
    name: String,
    version: String,
    license: String,
}

#[derive(Clone, Debug, Default)]
struct MetaPkg {
    has_build_script: bool,
    build_deps: BTreeSet<String>,
    rust_version: Option<String>,
}

fn cargo() -> Command {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
}

fn tree(manifest: &Path, package: Option<&str>, features: Option<&str>, offline: bool) -> Vec<Pkg> {
    let mut c = cargo();
    c.args(["tree", "--manifest-path"]).arg(manifest).args(["-e", "normal,build", "--target", "all", "--prefix", "none", "-f", "{p}|{l}"]);
    if let Some(p) = package {
        c.args(["-p", p]);
    }
    if let Some(f) = features {
        c.args(["--features", f]);
    }
    if offline {
        c.arg("--offline");
    }
    let out = c.output().expect("cargo runs");
    assert!(out.status.success(), "cargo tree failed: {}", String::from_utf8_lossy(&out.stderr));
    let mut set = BTreeSet::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(2, '|');
        let (Some(p), Some(l)) = (parts.next(), parts.next()) else { continue };
        let mut it = p.split_whitespace();
        let (Some(name), Some(ver)) = (it.next(), it.next()) else { continue };
        let license = l.trim().trim_end_matches("(*)").trim().to_string();
        set.insert(Pkg { name: name.to_string(), version: ver.trim_start_matches('v').to_string(), license });
    }
    set.into_iter().collect()
}

fn metadata(manifest: &Path) -> BTreeMap<(String, String), MetaPkg> {
    let out = cargo().args(["metadata", "--format-version", "1", "--manifest-path"]).arg(manifest).output().expect("cargo runs");
    assert!(out.status.success(), "cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr));
    let v: Value = serde_json::from_slice(&out.stdout).expect("metadata is JSON");
    let mut map = BTreeMap::new();
    for p in v["packages"].as_array().expect("packages") {
        let name = p["name"].as_str().unwrap_or("").to_string();
        let version = p["version"].as_str().unwrap_or("").to_string();
        let has_build_script = p["targets"].as_array().map(|t| t.iter().any(|t| t["kind"].as_array().map(|k| k.iter().any(|k| k == "custom-build")).unwrap_or(false))).unwrap_or(false);
        let build_deps = p["dependencies"]
            .as_array()
            .map(|d| d.iter().filter(|d| d["kind"] == "build").map(|d| d["name"].as_str().unwrap_or("").to_string()).collect())
            .unwrap_or_default();
        map.insert((name, version), MetaPkg { has_build_script, build_deps, rust_version: p["rust_version"].as_str().map(str::to_string) });
    }
    map
}

fn licence_ok(expr: &str) -> bool {
    if expr.trim().is_empty() {
        return false;
    }
    let normalised = expr.replace(['(', ')'], " ").replace('/', " OR ");
    normalised.split(" OR ").any(|alt| {
        alt.split(" AND ").all(|term| {
            term.split(" WITH ").all(|t| {
                let t = t.trim().trim_end_matches('+');
                t.is_empty() || ALLOWED_LICENCES.contains(&t)
            })
        })
    })
}

fn msrv_ok(v: &str) -> bool {
    let mut it = v.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let (major, minor) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
    (major, minor) <= MAX_MSRV
}

/// The guard: what is wrong with this tree. Empty means clean.
fn violations(tree: &[Pkg], meta: &BTreeMap<(String, String), MetaPkg>) -> Vec<String> {
    let mut bad = Vec::new();
    for p in tree {
        if let Some(m) = meta.get(&(p.name.clone(), p.version.clone())) {
            if m.has_build_script {
                let tools: Vec<&String> = m
                    .build_deps
                    .iter()
                    .filter(|d| NATIVE_BUILD_TOOLS.contains(&d.as_str()) && tree.iter().any(|t| &t.name == *d))
                    .filter(|d| !ALLOWED_NATIVE.iter().any(|(n, h, _)| *n == p.name && *h == d.as_str()))
                    .collect();
                if !tools.is_empty() {
                    bad.push(format!("{} {} has a build script that depends on {tools:?}: it compiles C or links a system library", p.name, p.version));
                }
            }
            if let Some(v) = &m.rust_version {
                if !msrv_ok(v) {
                    bad.push(format!("{} {} needs Rust {v}, above 1.{}", p.name, p.version, MAX_MSRV.1));
                }
            }
        }
        if !licence_ok(&p.license) {
            bad.push(format!("{} {} has the licence \"{}\", which is not on the permissive list", p.name, p.version, p.license));
        }
    }
    bad
}

fn workspace_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("Cargo.toml")
}

#[test]
fn the_host_tree_of_cobolt_wasm_compiles_no_c_but_wasmtimes_helper_and_is_permissive() {
    let root = workspace_manifest();
    let tree = tree(&root, Some("cobolt-wasm"), Some("host"), false);
    let meta = metadata(&root);
    let wt = tree.iter().find(|p| p.name == "wasmtime").expect("wasmtime is in the host tree");
    assert_eq!(wt.version, "44.0.3", "wasmtime is pinned to the last line with rust-version 1.92");
    assert!(!tree.iter().any(|p| p.name.starts_with("wasmi")), "wasmi left cobolt-wasm (WASM-010 D2)");
    let with_scripts = tree.iter().filter(|p| meta.get(&(p.name.clone(), p.version.clone())).map(|m| m.has_build_script).unwrap_or(false)).count();
    let native = tree
        .iter()
        .filter(|p| meta.get(&(p.name.clone(), p.version.clone())).map(|m| m.has_build_script && m.build_deps.iter().any(|d| NATIVE_BUILD_TOOLS.contains(&d.as_str()) && tree.iter().any(|t| &t.name == d))).unwrap_or(false))
        .count();
    eprintln!("dependency guard: {} packages in the host tree, {with_scripts} with a build script, {native} compiling C (allowed: {})", tree.len(), ALLOWED_NATIVE.len());
    let bad = violations(&tree, &meta);
    assert!(bad.is_empty(), "the host tree of cobolt-wasm breaks the dependency rules:\n  {}", bad.join("\n  "));
}

#[test]
fn the_guard_fails_on_a_c_build_script_a_bad_licence_and_a_new_rust() {
    let p = |n: &str, l: &str| Pkg { name: n.into(), version: "1.0.0".into(), license: l.into() };
    let tree = vec![p("fine", "MIT OR Apache-2.0"), p("sneaky-sys", "MIT"), p("gpl-thing", "GPL-3.0-only"), p("shiny", "MIT"), p("wasmtime", "Apache-2.0 WITH LLVM-exception"), p("cc", "MIT OR Apache-2.0")];
    let mut meta = BTreeMap::new();
    let cc: BTreeSet<String> = ["cc".to_string()].into();
    meta.insert(("sneaky-sys".to_string(), "1.0.0".to_string()), MetaPkg { has_build_script: true, build_deps: cc.clone(), rust_version: None });
    meta.insert(("wasmtime".to_string(), "1.0.0".to_string()), MetaPkg { has_build_script: true, build_deps: cc, rust_version: Some("1.92.0".into()) });
    meta.insert(("shiny".to_string(), "1.0.0".to_string()), MetaPkg { has_build_script: false, build_deps: BTreeSet::new(), rust_version: Some("1.93".into()) });
    let bad = violations(&tree, &meta);
    let text = bad.join("\n");
    assert!(text.contains("sneaky-sys") && text.contains("compiles C"), "{text}");
    assert!(text.contains("gpl-thing") && text.contains("GPL-3.0-only"), "{text}");
    assert!(text.contains("shiny") && text.contains("1.93"), "{text}");
    assert!(!text.contains("wasmtime"), "wasmtime's helper is the one allowed C build script: {text}");
    assert_eq!(bad.len(), 3, "{text}");
    assert!(licence_ok("Apache-2.0 WITH LLVM-exception"));
}

/// A real cargo project with a planted `sneaky-sys` whose build script build-depends on a stub
/// `cc` (path dependencies, nothing downloaded): the guard, run over real `cargo tree` and `cargo
/// metadata` output, reports it - and reports nothing for the same project without it.
#[test]
fn planting_a_c_build_script_in_a_real_cargo_project_makes_the_guard_fail() {
    let dir = std::env::temp_dir().join(format!("cobolt-wasm-guard-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for d in ["src", "stubs/cc/src", "stubs/sneaky-sys/src"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    std::fs::write(dir.join("stubs/cc/Cargo.toml"), "[package]\nname = \"cc\"\nversion = \"1.0.0\"\nedition = \"2021\"\nlicense = \"MIT\"\n").unwrap();
    std::fs::write(dir.join("stubs/cc/src/lib.rs"), "").unwrap();
    std::fs::write(
        dir.join("stubs/sneaky-sys/Cargo.toml"),
        "[package]\nname = \"sneaky-sys\"\nversion = \"0.1.0\"\nedition = \"2021\"\nlicense = \"MIT\"\nbuild = \"build.rs\"\n\n[build-dependencies]\ncc = { path = \"../cc\" }\n",
    )
    .unwrap();
    std::fs::write(dir.join("stubs/sneaky-sys/build.rs"), "fn main() {}\n").unwrap();
    std::fs::write(dir.join("stubs/sneaky-sys/src/lib.rs"), "").unwrap();
    std::fs::write(dir.join("src/lib.rs"), "").unwrap();
    let manifest = dir.join("Cargo.toml");
    let write = |planted: bool| {
        let deps = if planted { "sneaky-sys = { path = \"stubs/sneaky-sys\" }\n" } else { "" };
        std::fs::write(&manifest, format!("[package]\nname = \"guardprobe\"\nversion = \"0.0.0\"\nedition = \"2021\"\nlicense = \"MIT\"\n\n[dependencies]\n{deps}\n[workspace]\n")).unwrap();
    };

    write(false);
    let (t, m) = (tree(&manifest, None, None, true), metadata(&manifest));
    assert!(violations(&t, &m).is_empty(), "the clean project passes");

    write(true);
    let (t, m) = (tree(&manifest, None, None, true), metadata(&manifest));
    let bad = violations(&t, &m);
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].contains("sneaky-sys") && bad[0].contains("compiles C"), "{}", bad[0]);
    let _ = std::fs::remove_dir_all(&dir);
}
