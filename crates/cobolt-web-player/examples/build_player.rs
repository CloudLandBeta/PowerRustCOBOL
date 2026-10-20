// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The player build** (WASM-016 R12, WASM-017 R16–R17): the one step of the *product's* build
//! that turns this crate into the two prebuilt variants every Web bundle copies.
//!
//! ```text
//! cargo run --manifest-path crates/cobolt-web-player/Cargo.toml --example build_player -- \
//!     --out target/player [--variants plain,threaded] [--opt-level z|s] [--measure]
//! ```
//!
//! * **plain** — stable Rust, `wasm32-unknown-unknown`, no shared memory: the baseline.
//! * **threaded** — the pinned nightly of [`NIGHTLY`], `-Zbuild-std`, atomics + bulk memory +
//!   shared memory, `wasm-bindgen-rayon`. The nightly is confined to this step: nothing else of
//!   the repository builds with it (WASM-017 R16).
//!
//! Each variant: `cargo build --release` (the size-first profile of Cargo.toml: `lto`,
//! `codegen-units = 1`, `panic = "abort"`, stripped, the chosen `opt-level`) → `wasm-bindgen --target
//! web` (the CLI pinned to the crate version, never `wasm-pack`) → `wasm-opt -Oz`. The result is
//! `<out>/<variant>/player.js`, `player_bg.wasm` (+ the rayon worker snippet for the threaded one)
//! and `<out>/player-build.json`, which records the tool versions, the nightly date, the profile and
//! the sizes before and after `wasm-opt` (WASM-017 AC12).
//!
//! The **parts** loaded on demand (WASM-016 R13) — today the Mermaid renderer — are built once with
//! the stable toolchain into `<out>/parts/<name>/<name>.js` + `<name>_bg.wasm` and shared by both
//! variants; `--no-parts` skips them.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The pinned nightly (WASM-017 R16/R17, Q9). Recorded in every build.
const NIGHTLY: &str = "nightly-2026-10-08";
/// `wasm-bindgen`'s CLI must equal the crate's version exactly.
const WASM_BINDGEN: &str = "0.2.129";

const THREADED_RUSTFLAGS: &str = "-C target-feature=+atomics,+bulk-memory,+mutable-globals \
-C link-arg=--shared-memory -C link-arg=--max-memory=1073741824 -C link-arg=--import-memory \
-C link-arg=--export=__wasm_init_tls -C link-arg=--export=__tls_size -C link-arg=--export=__tls_align \
-C link-arg=--export=__tls_base";

struct Args {
    out: PathBuf,
    variants: Vec<String>,
    opt_level: String,
    measure: bool,
    no_parts: bool,
}

fn parse() -> Args {
    let mut a = Args { out: PathBuf::from("target/player"), variants: vec!["plain".into(), "threaded".into()], opt_level: "z".into(), measure: false, no_parts: false };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        match k.as_str() {
            "--out" => a.out = it.next().expect("--out <dir>").into(),
            "--variants" => a.variants = it.next().expect("--variants plain,threaded").split(',').map(str::to_owned).collect(),
            "--opt-level" => a.opt_level = it.next().expect("--opt-level z|s"),
            "--measure" => a.measure = true,
            "--no-parts" => a.no_parts = true,
            other => panic!("unknown argument {other}"),
        }
    }
    a
}

fn run(cmd: &mut Command, what: &str) {
    eprintln!("== {what}");
    let status = cmd.status().unwrap_or_else(|e| panic!("{what}: cannot start: {e}"));
    assert!(status.success(), "{what} failed ({status})");
}

fn capture(cmd: &mut Command) -> String {
    let out = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output().expect("tool runs");
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)).trim().to_owned()
}

fn size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

fn gzip_size(p: &Path) -> u64 {
    let out = Command::new("gzip").arg("-9").arg("-c").arg(p).output().expect("gzip");
    out.stdout.len() as u64
}

/// `wasm-bindgen-rayon`'s worker helper imports the player by the *directory* (`import('../../..')`),
/// which a bundler resolves through `package.json` and a plain static server cannot. The helper is
/// copied next to the player by `wasm-bindgen`, so it names the player file explicitly instead.
fn patch_rayon_worker_helpers(dir: &Path) {
    fn walk(dir: &Path, hits: &mut usize) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, hits);
            } else if p.file_name().is_some_and(|n| n == "workerHelpers.js") {
                let s = std::fs::read_to_string(&p).unwrap();
                if s.contains("import('../../..')") {
                    std::fs::write(&p, s.replace("import('../../..')", "import('../../../player.js')")).unwrap();
                    *hits += 1;
                }
            }
        }
    }
    let mut hits = 0;
    walk(dir, &mut hits);
    assert!(hits > 0, "the rayon worker helper was not found: the threaded player cannot start its workers");
}

struct Built {
    variant: String,
    dir: PathBuf,
    raw: u64,
    bindgen: u64,
    optimised: u64,
    gzip: u64,
    js: u64,
}

fn build_variant(crate_dir: &Path, target_dir: &Path, out: &Path, variant: &str, opt_level: &str) -> Built {
    let threaded = variant == "threaded";
    let tdir = target_dir.join(if threaded { "threaded" } else { "plain" });
    let mut cargo = if threaded { let mut c = Command::new("cargo"); c.arg(format!("+{NIGHTLY}")); c } else { Command::new("cargo") };
    cargo
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(crate_dir.join("Cargo.toml"))
        .arg("--target")
        .arg("wasm32-unknown-unknown")
        .arg("--target-dir")
        .arg(&tdir)
        .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", opt_level);
    if threaded {
        cargo.args(["--features", "threaded", "-Z", "build-std=std,panic_abort"]).env("RUSTFLAGS", THREADED_RUSTFLAGS);
    }
    run(&mut cargo, &format!("cargo build ({variant}, opt-level {opt_level})"));
    let raw_wasm = tdir.join("wasm32-unknown-unknown/release/cobolt_web_player.wasm");
    let vdir = out.join(variant);
    let _ = std::fs::remove_dir_all(&vdir);
    std::fs::create_dir_all(&vdir).unwrap();
    run(
        Command::new("wasm-bindgen").args(["--target", "web", "--no-typescript", "--out-name", "player", "--out-dir"]).arg(&vdir).arg(&raw_wasm),
        &format!("wasm-bindgen ({variant})"),
    );
    if threaded {
        patch_rayon_worker_helpers(&vdir);
    }
    let bg = vdir.join("player_bg.wasm");
    let bindgen = size(&bg);
    let opt = vdir.join("player_bg.opt.wasm");
    let mut wo = Command::new("wasm-opt");
    wo.args(["-Oz", "--enable-bulk-memory", "--enable-sign-ext", "--enable-mutable-globals", "--enable-nontrapping-float-to-int", "--enable-reference-types", "--enable-multivalue"]);
    if threaded {
        wo.arg("--enable-threads");
    }
    run(wo.arg(&bg).arg("-o").arg(&opt), &format!("wasm-opt ({variant})"));
    std::fs::rename(&opt, &bg).unwrap();
    Built {
        variant: variant.to_owned(),
        dir: vdir.clone(),
        raw: size(&raw_wasm),
        bindgen,
        optimised: size(&bg),
        gzip: gzip_size(&bg),
        js: size(&vdir.join("player.js")),
    }
}

/// The parts loaded on demand (WASM-016 R13): `(name, package)`. Each is a WebAssembly module of its
/// own, built with the stable toolchain once and shared by both player variants.
const PARTS: &[(&str, &str)] = &[("mermaid", "cobolt-web-part-mermaid")];

fn build_part(crate_dir: &Path, target_dir: &Path, out: &Path, name: &str, package: &str, opt_level: &str) -> Built {
    let tdir = target_dir.join("parts");
    run(
        Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--manifest-path")
            .arg(crate_dir.join("Cargo.toml"))
            .args(["-p", package, "--target", "wasm32-unknown-unknown"])
            .arg("--target-dir")
            .arg(&tdir)
            .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", opt_level),
        &format!("cargo build (part {name}, opt-level {opt_level})"),
    );
    let raw_wasm = tdir.join(format!("wasm32-unknown-unknown/release/{}.wasm", package.replace('-', "_")));
    let vdir = out.join("parts").join(name);
    let _ = std::fs::remove_dir_all(&vdir);
    std::fs::create_dir_all(&vdir).unwrap();
    run(
        Command::new("wasm-bindgen").args(["--target", "web", "--no-typescript", "--out-name", name, "--out-dir"]).arg(&vdir).arg(&raw_wasm),
        &format!("wasm-bindgen (part {name})"),
    );
    let bg = vdir.join(format!("{name}_bg.wasm"));
    let bindgen = size(&bg);
    let opt = vdir.join(format!("{name}_bg.opt.wasm"));
    run(
        Command::new("wasm-opt")
            .args(["-Oz", "--enable-bulk-memory", "--enable-sign-ext", "--enable-mutable-globals", "--enable-nontrapping-float-to-int", "--enable-reference-types", "--enable-multivalue"])
            .arg(&bg)
            .arg("-o")
            .arg(&opt),
        &format!("wasm-opt (part {name})"),
    );
    std::fs::rename(&opt, &bg).unwrap();
    Built { variant: format!("part:{name}"), dir: vdir.clone(), raw: size(&raw_wasm), bindgen, optimised: size(&bg), gzip: gzip_size(&bg), js: size(&vdir.join(format!("{name}.js"))) }
}

fn main() {
    let args = parse();
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = crate_dir.join("target-player");
    let out = if args.out.is_absolute() { args.out.clone() } else { std::env::current_dir().unwrap().join(&args.out) };
    std::fs::create_dir_all(&out).unwrap();

    // Tool versions: the build refuses a mismatched wasm-bindgen CLI (the glue and the module must agree).
    let bindgen_v = capture(Command::new("wasm-bindgen").arg("--version"));
    assert!(bindgen_v.contains(WASM_BINDGEN), "wasm-bindgen CLI is `{bindgen_v}`; the crate pins {WASM_BINDGEN}: install it with `cargo install wasm-bindgen-cli --version {WASM_BINDGEN}`");
    let wasm_opt_v = capture(Command::new("wasm-opt").arg("--version"));
    let rustc_v = capture(Command::new("rustc").arg("--version"));
    let nightly_v = if args.variants.iter().any(|v| v == "threaded") { capture(Command::new("rustc").arg(format!("+{NIGHTLY}")).arg("--version")) } else { String::new() };

    let mut levels = vec![args.opt_level.clone()];
    if args.measure {
        levels = vec!["z".into(), "s".into()];
    }
    let mut report = Vec::new();
    let mut chosen: Vec<Built> = Vec::new();
    for level in &levels {
        for v in &args.variants {
            let sub_out = if args.measure { out.join(format!("opt-{level}")) } else { out.clone() };
            let b = build_variant(&crate_dir, &target_dir, &sub_out, v, level);
            report.push(serde_json::json!({
                "variant": b.variant, "opt_level": level, "cargo_output_bytes": b.raw, "after_bindgen_bytes": b.bindgen,
                "after_wasm_opt_bytes": b.optimised, "gzip_bytes": b.gzip, "glue_js_bytes": b.js,
            }));
            println!("{:<9} opt-level {level}: cargo {:>9}  bindgen {:>9}  wasm-opt {:>9}  gzip {:>9}  glue {:>7}", b.variant, b.raw, b.bindgen, b.optimised, b.gzip, b.js);
            if level == &levels[0] || !args.measure {
                chosen.push(b);
            }
        }
    }
    // The parts: one build each, in the directory the bundle assembler reads (`<out>/parts/<name>`).
    let mut parts = Vec::new();
    if !args.no_parts {
        let part_out = if args.measure { out.join(format!("opt-{}", levels[0])) } else { out.clone() };
        for (name, package) in PARTS {
            let b = build_part(&crate_dir, &target_dir, &part_out, name, package, &levels[0]);
            println!("part {name:<8} opt-level {}: cargo {:>9}  bindgen {:>9}  wasm-opt {:>9}  gzip {:>9}  glue {:>7}", levels[0], b.raw, b.bindgen, b.optimised, b.gzip, b.js);
            parts.push(serde_json::json!({
                "name": name, "opt_level": levels[0], "cargo_output_bytes": b.raw, "after_bindgen_bytes": b.bindgen,
                "after_wasm_opt_bytes": b.optimised, "gzip_bytes": b.gzip, "glue_js_bytes": b.js,
            }));
        }
    }
    let info = serde_json::json!({
        "profile": {"lto": true, "codegen_units": 1, "panic": "abort", "strip": true, "opt_level": args.opt_level, "wasm_opt": "-Oz"},
        "tools": {"wasm_bindgen": bindgen_v, "wasm_opt": wasm_opt_v, "rustc_stable": rustc_v, "rustc_nightly": nightly_v, "nightly": NIGHTLY, "wasm_pack": "not used (WASM-017 R17)"},
        "measurements": report,
        "parts": parts,
        "variants": chosen.iter().map(|b| serde_json::json!({"name": b.variant, "dir": b.dir.file_name().unwrap().to_string_lossy()})).collect::<Vec<_>>(),
    });
    std::fs::write(out.join("player-build.json"), serde_json::to_string_pretty(&info).unwrap()).unwrap();
    println!("wrote {}", out.join("player-build.json").display());
}
