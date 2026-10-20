// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The dependency guard (WASM-018 R4, AC11).
//!
//! "A crate enters only if it is written in Rust, no build script of its default feature set
//! compiles C or C++, its licence is permissive and compatible with Apache-2.0, its minimum Rust
//! version is at most 1.92, and it passes the dependency guards (no `ring`, no `aws-lc-rs`)."
//!
//! This test walks the **whole dependency tree of `cobolt-graphics`** - normal and build
//! dependencies, for every target platform and the default feature set, as cargo itself
//! resolves it - and fails on
//!
//! * a package of the deny list (`font-kit`, `freetype-sys`, `harfbuzz-sys`, `skia-safe`,
//!   `cairo-sys-rs`, `libwebp-sys`, `mozjpeg-sys` and the like, `ring`, `aws-lc-rs`);
//! * a package whose **build script** has a C compiler or a system-library finder among its
//!   build dependencies (`cc`, `cmake`, `bindgen`, `pkg-config`, `vcpkg`, ...), which is what
//!   a build script that compiles C or links a system library needs;
//! * a licence outside the permissive set, or a minimum Rust version above 1.92.
//!
//! The guard is a function over data, and **it is tested to fail**: with a package of the deny
//! list added to a real cargo project (a stub `font-kit`, so nothing is downloaded) the same
//! walk over real `cargo tree` output reports it, and without it the project is clean.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// Packages that must never be in the tree: C and C++ libraries behind a Rust name, and the crypto
/// back-ends WASM-013 R9 bans.
const DENY: &[&str] = &[
    "font-kit",
    "freetype-sys",
    "freetype",
    "harfbuzz-sys",
    "harfbuzz",
    "skia-safe",
    "skia-bindings",
    "cairo-sys-rs",
    "cairo-rs",
    "libwebp-sys",
    "libwebp-sys2",
    "mozjpeg-sys",
    "mozjpeg",
    "libjpeg-turbo-sys",
    "fontconfig-sys",
    "servo-fontconfig-sys",
    "expat-sys",
    "ring",
    "aws-lc-rs",
    "aws-lc-sys",
    "openssl-sys",
    "libz-sys",
    "zstd-sys",
    "bzip2-sys",
    "lzma-sys",
    "brotli-sys",
    "libsqlite3-sys",
];

/// What a build script needs in order to compile C or C++ or to find a system library.
const NATIVE_BUILD_TOOLS: &[&str] = &["cc", "cmake", "bindgen", "pkg-config", "vcpkg", "system-deps", "metadeps"];

/// Licences compatible with Apache-2.0 and permissive (MPL-2.0 is file-level copyleft and is allowed
/// the way it is throughout this repository's Rust tree).
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
    "NCSA",
    "OFL-1.1",
    "Ubuntu-font-1.0",
];

const MAX_MSRV: (u32, u32) = (1, 92);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Pkg {
    name: String,
    version: String,
    license: String,
    /// The features cargo turns on for it (unified over every target platform).
    features: BTreeSet<String>,
}

/// Build scripts that name a native-build helper but are known not to use it in this tree. The
/// exception holds only while the feature that makes the script skip the helper is what cargo
/// resolves; the guard checks that, so a changed feature set turns the exception into a failure.
///
/// `(package, helper, what must hold, why)`.
const PROBE_ONLY: &[(&str, &str, &str, &str)] = &[
    ("wayland-sys", "pkg-config", "feature `dlopen` is on", "with `dlopen` its build script returns before probing anything: wgpu-hal opens libwayland at run time"),
    ("khronos-egl", "pkg-config", "feature `dynamic` is on, and `static` only together with `no-pkg-config`", "pkg-config is used only with `static`; wgpu-hal asks for `dynamic` (libEGL opened at run time), and for `static` only with `no-pkg-config` on emscripten"),
];

fn exception_holds(p: &Pkg, helper: &str) -> bool {
    match (p.name.as_str(), helper) {
        ("wayland-sys", "pkg-config") => p.features.contains("dlopen"),
        ("khronos-egl", "pkg-config") => p.features.contains("dynamic") && (!p.features.contains("static") || p.features.contains("no-pkg-config")),
        _ => false,
    }
}

/// The facts a violation needs about one package of the metadata.
#[derive(Clone, Debug, Default)]
struct MetaPkg {
    has_build_script: bool,
    build_deps: BTreeSet<String>,
    rust_version: Option<String>,
}

fn cargo() -> Command {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
}

/// Every package cargo would build for `package` (or for the root package of the manifest when it
/// is `None`): normal and build dependencies, all target platforms, default features.
fn tree(manifest: &Path, package: Option<&str>, offline: bool) -> Vec<Pkg> {
    let mut c = cargo();
    c.args(["tree", "--manifest-path"]).arg(manifest).args(["-e", "normal,build", "--target", "all", "--prefix", "none", "-f", "{p}|{l}|{f}"]);
    if let Some(p) = package {
        c.args(["-p", p]);
    }
    if offline {
        c.arg("--offline");
    }
    let out = c.output().expect("cargo runs");
    assert!(out.status.success(), "cargo tree failed: {}", String::from_utf8_lossy(&out.stderr));
    let mut set = BTreeSet::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(3, '|');
        let (Some(p), Some(l), Some(f)) = (parts.next(), parts.next(), parts.next()) else { continue };
        let mut it = p.split_whitespace();
        let (Some(name), Some(ver)) = (it.next(), it.next()) else { continue };
        let features = f.trim().trim_end_matches("(*)").trim().split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect();
        let license = l.trim().trim_end_matches("(*)").trim().to_string();
        set.insert(Pkg { name: name.to_string(), version: ver.trim_start_matches('v').to_string(), license, features });
    }
    set.into_iter().collect()
}

/// What the workspace metadata says about every package it resolved: build script, build
/// dependencies, minimum Rust version.
fn metadata(manifest: &Path) -> BTreeMap<(String, String), MetaPkg> {
    let out = cargo().args(["metadata", "--format-version", "1", "--manifest-path"]).arg(manifest).output().expect("cargo runs");
    assert!(out.status.success(), "cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr));
    parse_metadata(&serde_json::from_slice::<Value>(&out.stdout).expect("metadata is JSON"))
}

fn parse_metadata(v: &Value) -> BTreeMap<(String, String), MetaPkg> {
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
    // "A OR B": any alternative will do. An alternative is "X AND Y" or "X WITH Z": all its terms must be allowed.
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
        if DENY.contains(&p.name.as_str()) {
            bad.push(format!("{} {} is on the deny list (C or C++ behind a Rust name, or a banned crypto back-end)", p.name, p.version));
        }
        if let Some(m) = meta.get(&(p.name.clone(), p.version.clone())) {
            if m.has_build_script {
                // Only a helper that is actually in the tree is active (an optional build dependency that no feature turns on is not).
                let tools: Vec<&String> = m
                    .build_deps
                    .iter()
                    .filter(|d| NATIVE_BUILD_TOOLS.contains(&d.as_str()) && tree.iter().any(|t| &t.name == *d))
                    .filter(|d| !(PROBE_ONLY.iter().any(|(n, h, _, _)| *n == p.name && *h == d.as_str()) && exception_holds(p, d)))
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

// ---------------------------------------------------------------------------

#[test]
fn the_tree_of_cobolt_graphics_has_no_denied_package_no_c_build_script_and_only_permissive_licences() {
    let root = workspace_manifest();
    let tree = tree(&root, Some("cobolt-graphics"), false);
    let meta = metadata(&root);
    assert!(tree.iter().any(|p| p.name == "cobolt-graphics") && tree.iter().any(|p| p.name == "tiny-skia"), "the walk found the crate's tree ({} packages)", tree.len());
    if std::env::var("GRAPHICS_DEPS_REPORT").map(|v| v == "1").unwrap_or(false) {
        for p in &tree {
            eprintln!("DEP {} {} [{}]", p.name, p.version, p.license);
        }
    }
    let with_scripts = tree.iter().filter(|p| meta.get(&(p.name.clone(), p.version.clone())).map(|m| m.has_build_script).unwrap_or(false)).count();
    eprintln!("dependency guard: {} packages in the tree, {with_scripts} with a build script, none compiling C", tree.len());
    let bad = violations(&tree, &meta);
    assert!(bad.is_empty(), "the dependency tree of cobolt-graphics breaks WASM-018 R4:\n  {}", bad.join("\n  "));
    // The specific names the spec calls out.
    for banned in ["ring", "aws-lc-rs", "font-kit", "skia-safe", "freetype-sys", "harfbuzz-sys", "cairo-sys-rs", "libwebp-sys", "mozjpeg-sys"] {
        assert!(!tree.iter().any(|p| p.name == banned), "{banned} is in the tree");
    }
}

#[test]
fn the_guard_fails_on_a_denied_package_a_c_build_script_a_bad_licence_and_a_new_rust() {
    let tree = vec![
        Pkg { name: "fine".into(), version: "1.0.0".into(), license: "MIT OR Apache-2.0".into(), features: BTreeSet::new() },
        Pkg { name: "font-kit".into(), version: "0.14.0".into(), license: "MIT".into(), features: BTreeSet::new() },
        Pkg { name: "sneaky-sys".into(), version: "0.1.0".into(), license: "MIT".into(), features: BTreeSet::new() },
        Pkg { name: "gpl-thing".into(), version: "2.0.0".into(), license: "GPL-3.0-only".into(), features: BTreeSet::new() },
        Pkg { name: "shiny".into(), version: "3.0.0".into(), license: "MIT".into(), features: BTreeSet::new() },
        Pkg { name: "script-only".into(), version: "1.0.0".into(), license: "MIT".into(), features: BTreeSet::new() },
        Pkg { name: "cc".into(), version: "1.0.0".into(), license: "MIT OR Apache-2.0".into(), features: BTreeSet::new() },
    ];
    let mut meta = BTreeMap::new();
    meta.insert(("sneaky-sys".to_string(), "0.1.0".to_string()), MetaPkg { has_build_script: true, build_deps: ["cc".to_string()].into(), rust_version: None });
    meta.insert(("shiny".to_string(), "3.0.0".to_string()), MetaPkg { has_build_script: false, build_deps: BTreeSet::new(), rust_version: Some("1.95".into()) });
    meta.insert(("script-only".to_string(), "1.0.0".to_string()), MetaPkg { has_build_script: true, build_deps: ["autocfg".to_string()].into(), rust_version: Some("1.60.0".into()) });
    let bad = violations(&tree, &meta);
    let text = bad.join("\n");
    assert!(text.contains("font-kit 0.14.0 is on the deny list"), "{text}");
    assert!(text.contains("sneaky-sys") && text.contains("compiles C"), "{text}");
    assert!(text.contains("gpl-thing") && text.contains("GPL-3.0-only"), "{text}");
    assert!(text.contains("shiny") && text.contains("1.95"), "{text}");
    assert!(!text.contains("fine 1.0.0") && !text.contains("script-only"), "a build script that only probes the compiler is not C: {text}");
    assert_eq!(bad.len(), 4, "{text}");
    // A clean tree passes.
    assert!(violations(&tree[..1], &meta).is_empty());

    // The two probe-only exceptions hold exactly while the feature that disables the probe is on.
    let pk = Pkg { name: "pkg-config".into(), version: "0.3.0".into(), license: "MIT OR Apache-2.0".into(), features: BTreeSet::new() };
    let mut meta = BTreeMap::new();
    for n in ["wayland-sys", "khronos-egl"] {
        meta.insert((n.to_string(), "1.0.0".to_string()), MetaPkg { has_build_script: true, build_deps: ["pkg-config".to_string()].into(), rust_version: None });
    }
    let pkg = |name: &str, f: &[&str]| Pkg { name: name.into(), version: "1.0.0".into(), license: "MIT".into(), features: f.iter().map(|s| s.to_string()).collect() };
    let ok = vec![pk.clone(), pkg("wayland-sys", &["client", "dlopen"]), pkg("khronos-egl", &["dynamic", "static", "no-pkg-config"])];
    assert!(violations(&ok, &meta).is_empty(), "{:?}", violations(&ok, &meta));
    let no_dlopen = vec![pk.clone(), pkg("wayland-sys", &["client"])];
    assert!(violations(&no_dlopen, &meta).iter().any(|v| v.contains("wayland-sys")), "without dlopen the script probes with pkg-config");
    let static_egl = vec![pk.clone(), pkg("khronos-egl", &["dynamic", "static"])];
    assert!(violations(&static_egl, &meta).iter().any(|v| v.contains("khronos-egl")), "static without no-pkg-config probes with pkg-config");
    // And a helper that no feature turned on is not active, so a script that could use it does not count.
    let inactive = vec![pkg("wayland-sys", &["client"])];
    assert!(violations(&inactive, &meta).is_empty());
}

#[test]
fn licence_expressions_are_read_the_way_spdx_writes_them() {
    for ok in ["MIT", "MIT OR Apache-2.0", "Apache-2.0 OR MIT", "MIT/Apache-2.0", "Apache-2.0 WITH LLVM-exception", "(MIT OR Apache-2.0) AND Unicode-3.0", "Unlicense OR MIT", "Zlib OR Apache-2.0 OR MIT", "MPL-2.0"] {
        assert!(licence_ok(ok), "{ok}");
    }
    for bad in ["", "GPL-3.0-or-later", "LGPL-2.1", "MIT AND GPL-2.0", "AGPL-3.0", "Proprietary"] {
        assert!(!licence_ok(bad), "{bad}");
    }
    assert!(licence_ok("GPL-2.0 OR MIT"), "an OR with one permissive alternative is permissive");
}

/// A real cargo project in a temp folder that depends on a stub package called `font-kit` (a path
/// dependency, so nothing is downloaded): the guard, run over real `cargo tree` and `cargo
/// metadata` output, reports it - and reports nothing for the same project without it.
#[test]
fn adding_a_denied_package_to_a_real_cargo_project_makes_the_guard_fail() {
    let dir = std::env::temp_dir().join(format!("cobolt-graphics-guard-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("stubs").join("font-kit").join("src")).unwrap();
    std::fs::write(dir.join("stubs").join("font-kit").join("Cargo.toml"), "[package]\nname = \"font-kit\"\nversion = \"0.14.0\"\nedition = \"2021\"\nlicense = \"MIT\"\n").unwrap();
    std::fs::write(dir.join("stubs").join("font-kit").join("src").join("lib.rs"), "").unwrap();
    std::fs::write(dir.join("src").join("lib.rs"), "").unwrap();
    let manifest = dir.join("Cargo.toml");

    let write = |with_denied: bool| {
        let deps = if with_denied { "font-kit = { path = \"stubs/font-kit\" }\n" } else { "" };
        std::fs::write(&manifest, format!("[package]\nname = \"guardprobe\"\nversion = \"0.0.0\"\nedition = \"2021\"\nlicense = \"MIT\"\n\n[dependencies]\n{deps}\n[workspace]\n")).unwrap();
    };

    write(false);
    let (t, m) = (tree(&manifest, None, true), metadata(&manifest));
    assert_eq!(t.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["guardprobe"]);
    assert!(violations(&t, &m).is_empty(), "the clean project passes");

    write(true);
    let (t, m) = (tree(&manifest, None, true), metadata(&manifest));
    assert!(t.iter().any(|p| p.name == "font-kit"), "cargo tree lists the added package");
    let bad = violations(&t, &m);
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].contains("font-kit") && bad[0].contains("deny list"), "{}", bad[0]);
    let _ = std::fs::remove_dir_all(&dir);
}
