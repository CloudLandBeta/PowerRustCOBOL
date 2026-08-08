// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Spec 044 prototype — External Crates for EXEC RUST.
//!
//! Two frontends over one action layer (`ops.rs`), mirroring the final
//! architecture (IDE dialog → service functions → modules):
//!
//! ```text
//! cargo run                              the INTERACTIVE dialog (default)
//! cargo run -- ui                        same, explicitly
//!
//! cargo run -- search <query>            scripted CLI, same actions:
//! cargo run -- add <name> [--req R] [--features a,b]
//! cargo run -- update <name> | update --all
//! cargo run -- remove <name> [--yes]
//! cargo run -- list
//! cargo run -- manifest [--dest DIR]
//! ```
//!
//! Global flags: `--registry <base-url>` (the pluggable endpoint, R4;
//! default https://crates.io) · `--project <dir>` (default ./demo-project) ·
//! `--workspace <dir>` (PowerRustCOBOL checkout; auto-detected) ·
//! `--skip-probe` (skip the resolver probe on add/update).

mod conflict;
mod manifest;
mod ops;
mod project;
mod registry;
mod ui;

use std::path::{Path, PathBuf};

use ops::Note;
use project::CratesFile;
use registry::Registry;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(message) = run(args) {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

fn run(raw: Vec<String>) -> Result<(), String> {
    let mut registry_base = "https://crates.io".to_string();
    let mut project_dir = PathBuf::from("demo-project");
    let mut workspace: Option<PathBuf> = None;
    let mut skip_probe = false;
    let mut rest: Vec<String> = Vec::new();

    let mut it = raw.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--registry" => registry_base = expect_value(&mut it, "--registry")?,
            "--project" => project_dir = PathBuf::from(expect_value(&mut it, "--project")?),
            "--workspace" => workspace = Some(PathBuf::from(expect_value(&mut it, "--workspace")?)),
            "--skip-probe" => skip_probe = true,
            _ => rest.push(arg),
        }
    }

    let mut rest = rest.into_iter();
    // No command = the interactive dialog: the prototype the IDE flow reuses.
    let command = rest.next().unwrap_or_else(|| "ui".into());
    let rest: Vec<String> = rest.collect();

    // The CLI narrates ops progress to stdout; the dialog to its log pane.
    let mut print = |note: Note| match note {
        Note::Info(text) => println!("  {text}"),
        Note::Warn(text) => println!("  warning: {text}"),
    };
    let registry = Registry::new(&registry_base);

    match command.as_str() {
        "ui" => ui::run(registry_base, project_dir),
        "search" => cmd_search(&registry, &rest),
        "add" => {
            let (name, req, features) = parse_crate_args(&rest, "add")?;
            let done = ops::add(
                &registry,
                &project_dir,
                workspace,
                skip_probe,
                &name,
                req.as_deref(),
                features,
                &mut print,
            )?;
            println!("{done}");
            Ok(())
        }
        "update" => {
            let targets: Vec<String> = if rest.iter().any(|a| a == "--all") {
                Vec::new()
            } else {
                vec![rest
                    .first()
                    .cloned()
                    .ok_or("usage: update <name> | update --all")?]
            };
            let summary = ops::update(
                &registry,
                &project_dir,
                workspace,
                skip_probe,
                &targets,
                &mut print,
            )?;
            println!("{summary}");
            Ok(())
        }
        "remove" => cmd_remove(&project_dir, &rest),
        "list" => cmd_list(&project_dir),
        "manifest" => {
            let mut dest = None;
            let mut it = rest.iter();
            while let Some(arg) = it.next() {
                if arg == "--dest" {
                    dest = Some(PathBuf::from(it.next().ok_or("--dest needs a value")?));
                }
            }
            println!("{}", ops::write_manifest(&project_dir, dest)?);
            Ok(())
        }
        "help" | "--help" | "-h" => {
            println!("{HELP}");
            Ok(())
        }
        other => Err(format!("unknown command `{other}` — try `help`")),
    }
}

const HELP: &str = "\
external-crates-prototype (spec 044)

  (no command) | ui                       open the interactive dialog
  search <query>                          search the configured registry
  add <name> [--req R] [--features a,b]   resolve, conflict-check, vendor, record
  update <name> | update --all            re-resolve within the recorded requirement
  remove <name> [--yes]                   remove record + vendored source
  list                                    show registered crates
  manifest [--dest DIR]                   write (or clean) dist/rust_manifest.md

global: --registry <url>  --project <dir>  --workspace <dir>  --skip-probe";

// ── CLI-only leaves ──────────────────────────────────────────────────────────

fn cmd_search(registry: &Registry, rest: &[String]) -> Result<(), String> {
    let query = rest.first().ok_or("usage: search <query>")?;
    let hits = registry.search(query, 10).map_err(|e| e.to_string())?;
    if hits.is_empty() {
        println!("no crates on {} match \"{query}\"", registry.base());
        return Ok(());
    }
    println!("{:<24} {:<12} description", "name", "newest");
    for hit in hits {
        let mut desc = hit.description;
        if desc.len() > 60 {
            desc.truncate(59);
            desc.push('…');
        }
        println!("{:<24} {:<12} {desc}", hit.name, hit.newest);
    }
    Ok(())
}

fn cmd_remove(project_dir: &Path, rest: &[String]) -> Result<(), String> {
    let name = rest.first().ok_or("usage: remove <name> [--yes]")?;
    let assume_yes = rest.iter().any(|a| a == "--yes");
    let state = CratesFile::load(project_dir)?;
    let Some(found) = state.find(name) else {
        return Err(format!("`{name}` is not registered"));
    };

    if !assume_yes {
        // R19 — removal is explicit and confirmed; the dialog shows a modal,
        // the CLI asks on stdin.
        print!(
            "remove `{}` {} and delete its vendored source? [y/N] ",
            found.name, found.version
        );
        use std::io::Write as _;
        std::io::stdout().flush().ok();
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .map_err(|e| e.to_string())?;
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            println!("kept `{}` — nothing changed", found.name);
            return Ok(());
        }
    }
    println!("{}", ops::remove(project_dir, name)?);
    Ok(())
}

fn cmd_list(project_dir: &Path) -> Result<(), String> {
    let state = CratesFile::load(project_dir)?;
    if state.crates.is_empty() {
        println!("no external crates registered");
        return Ok(());
    }
    println!("External Crates ({}):", state.crates.len());
    for c in &state.crates {
        let features = if c.features.is_empty() {
            String::new()
        } else {
            format!("  features: {}", c.features.join(", "))
        };
        let requirement = if c.requirement.is_empty() {
            "newest stable".into()
        } else {
            format!("req {}", c.requirement)
        };
        println!("  {} {}  ({requirement}){features}", c.name, c.version);
    }
    Ok(())
}

// ── plumbing ─────────────────────────────────────────────────────────────────

fn expect_value(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    it.next().ok_or_else(|| format!("{flag} needs a value"))
}

/// `add csv --req ^1 --features serde,unicode`
fn parse_crate_args(
    rest: &[String],
    command: &str,
) -> Result<(String, Option<String>, Vec<String>), String> {
    let mut name = None;
    let mut requirement = None;
    let mut features = Vec::new();
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--req" => requirement = Some(it.next().ok_or("--req needs a value")?.clone()),
            "--features" => {
                features = it
                    .next()
                    .ok_or("--features needs a value")?
                    .split(',')
                    .map(|f| f.trim().to_string())
                    .filter(|f| !f.is_empty())
                    .collect()
            }
            other if !other.starts_with('-') && name.is_none() => {
                name = Some(other.to_ascii_lowercase())
            }
            other => return Err(format!("unexpected argument `{other}`")),
        }
    }
    Ok((
        name.ok_or_else(|| format!("usage: {command} <name> [--req R] [--features a,b]"))?,
        requirement,
        features,
    ))
}
