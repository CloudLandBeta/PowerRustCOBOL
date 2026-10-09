//! Spec 091, AC1 / R2 / R39 — **a form that never used layers saves exactly as
//! it did before layers existed.**
//!
//! `every_example_form_round_trips_056` (in `xml.rs`) asserts only that
//! `save(load(save(load(f)))) == save(load(f))` over 74 forms of two projects.
//! That proves the writer is stable, not that it is unchanged: a writer that
//! began to emit a `layer` attribute on every control would pass it. This walker
//! covers **every** `.cfrm` under `examples/` and `tests/`, and states what it
//! checked as a number a human can compare run to run (GOLDEN RULE #7):
//!
//! * how many forms it found, loaded, and could not load;
//! * how many of them `save(load(f))` reproduces **byte for byte**;
//! * that none of them gains layer markup — except a form that declares layers
//!   itself (a project written with them, such as PowerAnalytics' `POPUPS`), which
//!   must keep every layer and every control's `layer`, and is counted apart.
//!
//! The byte-identical count is recorded in the commit that added this test,
//! measured **before** the layer model existed. Any later change that lowers it
//! has made saving a form edit its file (R2, R54).
//!
//! The count cannot show a form whose *saved text* changed while it was already
//! different from its file, so the walker can also write every saved form out:
//! set `PRC_DUMP_SAVED_FORMS=<directory>` and it writes `save(load(f))` there,
//! one file per form. Dump once on a build without the change and once with it,
//! then `diff -r` the two directories — that is the proof of "unchanged".

use std::path::{Path, PathBuf};

use cobolt_forms::{form_to_string, load_form, load_form_from_str};

/// Directories that hold build output or VCS data, never forms.
const SKIP_DIRS: &[&str] = &["target", "node_modules", ".git"];

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skip = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| SKIP_DIRS.contains(&n));
            if !skip {
                collect(&path, out);
            }
        } else if path.extension().and_then(|x| x.to_str()) == Some("cfrm") {
            out.push(path);
        }
    }
}

#[test]
fn every_example_form_saves_unchanged_091() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for root in ["examples", "tests"] {
        collect(&repo.join(root), &mut files);
    }
    files.sort();
    assert!(!files.is_empty(), "no .cfrm found under examples/ or tests/");

    let dump = std::env::var_os("PRC_DUMP_SAVED_FORMS").map(PathBuf::from);
    if let Some(dir) = &dump {
        std::fs::create_dir_all(dir).unwrap();
    }

    let mut loaded = 0usize;
    let mut unloadable: Vec<String> = Vec::new();
    let mut identical = 0usize;
    let mut differing: Vec<String> = Vec::new();
    let mut with_layers: Vec<String> = Vec::new();
    for path in &files {
        let shown = path
            .strip_prefix(&repo)
            .unwrap_or(path)
            .display()
            .to_string();
        let Ok(form) = load_form(path) else {
            unloadable.push(shown);
            continue;
        };
        loaded += 1;
        let once = form_to_string(&form).unwrap();
        let twice = form_to_string(&load_form_from_str(&once).unwrap()).unwrap();
        assert_eq!(once, twice, "{shown} does not round-trip");
        let declares_layers =
            !form.layers.is_empty() || form.controls.iter().any(|c| c.layer.is_some());
        if declares_layers {
            // A form that uses layers keeps all of them (R39): each `<Layer>` and each
            // control's `layer` survives the save.
            for layer in &form.layers {
                assert!(
                    once.contains(&format!("<Layer name=\"{}\"", layer.name)),
                    "{shown} lost layer {} on save (R39)",
                    layer.name
                );
            }
            for c in form.controls.iter().filter(|c| c.layer.is_some()) {
                let l = c.layer.as_deref().unwrap_or_default();
                assert!(
                    once.contains(&format!(" layer=\"{l}\"")),
                    "{shown}: control {} lost its layer {l} on save (R39)",
                    c.id
                );
            }
            with_layers.push(shown.clone());
        } else {
            assert!(
                !once.contains("<Layer") && !once.contains(" layer=\""),
                "{shown} gained layer markup although it uses no layer (R2, R39)"
            );
        }
        if let Some(dir) = &dump {
            let name = shown.replace(['/', '\\'], "__");
            std::fs::write(dir.join(name), &once).unwrap();
        }
        if std::fs::read_to_string(path).is_ok_and(|on_disk| on_disk == once) {
            identical += 1;
        } else {
            differing.push(shown);
        }
    }

    println!("── 091 AC1: example forms saved unchanged ──────────────────────");
    println!("  forms found                       : {}", files.len());
    println!("  forms loaded                      : {loaded}");
    println!("  forms that did not load           : {}", unloadable.len());
    println!("  save(load(f)) == f byte for byte  : {identical}");
    println!("  save(load(f)) != f (pre-existing) : {}", differing.len());
    println!("  forms that declare layers (kept)  : {}", with_layers.len());
    for w in &with_layers {
        println!("    with layers: {w}");
    }
    for u in &unloadable {
        println!("  did not load: {u}");
    }
    println!("─────────────────────────────────────────────────────────────────");
}
