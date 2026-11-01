// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Spec 109 T0.1 and T-M — the dependency inventory and the measurement.**
//!
//! `cargo run -p cobolt-codegen --example spec109_measure -- inventory` prints the
//! control-storage inventory (which `WS-<ID>` groups authored code names, and on
//! which line); `-- measure` prints the line and byte counts per example form,
//! with and without `compact-storage`. Both read the example projects' `.cfrm`
//! files and generate with the current generator, so a reader can re-derive the
//! reports (`specs/109-rustcobol-extensions/inventory.md`, `measurement.md`).
//! Nothing is written; no comment or whitespace is stripped from any count.

use std::path::{Path, PathBuf};

const PROJECTS: [&str; 5] = ["PowerAnalytics", "PowerChat", "PowerCloud", "PowerDemo3", "PowerSpatial"];

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn forms_in(dir: &Path) -> Vec<PathBuf> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("cfrm") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

/// A comment line: free-format `*>` or the fixed-format indicator column.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("*>") || line.chars().nth(6) == Some('*')
}

#[derive(Default, Clone, Copy)]
struct Counts {
    total: usize,
    nonblank: usize,
    executable: usize,
    user: usize,
    bytes: usize,
}

impl Counts {
    fn add(&mut self, o: Counts) {
        self.total += o.total;
        self.nonblank += o.nonblank;
        self.executable += o.executable;
        self.user += o.user;
        self.bytes += o.bytes;
    }
    /// Lines the generator wrote that no developer typed.
    fn generated_only(&self) -> usize {
        self.total.saturating_sub(self.user)
    }
}

fn count(form: &cobolt_forms::Form) -> (Counts, String) {
    let (cbl, user_ranges) = cobolt_codegen::generate_with_user_lines(form);
    let mut c = Counts { bytes: cbl.len(), ..Counts::default() };
    let mut in_procedure = false;
    for line in cbl.lines() {
        c.total += 1;
        if !line.trim().is_empty() {
            c.nonblank += 1;
        }
        if line.trim_start().to_ascii_uppercase().starts_with("PROCEDURE DIVISION") {
            in_procedure = true;
        }
        if in_procedure && !line.trim().is_empty() && !is_comment(line) {
            c.executable += 1;
        }
    }
    c.user = user_ranges.iter().map(|(a, b)| (b - a + 1) as usize).sum::<usize>();
    (c, cbl)
}

fn load(path: &Path) -> cobolt_forms::Form {
    cobolt_forms::load_form(path).unwrap_or_else(|e| panic!("{} must parse: {e}", path.display()))
}

fn whole_word_at(hay: &str, name: &str) -> Option<usize> {
    hay.match_indices(name).map(|(i, _)| i).find(|&i| {
        hay[..i].chars().next_back().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '-'))
    })
}

fn inventory() {
    println!("| Project | Form | Controls with a group | Kept (named elsewhere) | Omittable | Declaration lines the omission saves |");
    println!("|---|---|---|---|---|---|");
    let (mut g, mut k) = (0usize, 0usize);
    let mut kept_rows: Vec<String> = Vec::new();
    for project in PROJECTS {
        let dir = repo().join("examples").join(project).join("forms");
        for path in forms_in(&dir) {
            let rel = path.strip_prefix(&dir).unwrap().display().to_string();
            let mut form = load(&path);
            form.compact_storage = false;
            let (full, full_cbl) = count(&form);
            form.compact_storage = true;
            let (compact, compact_cbl) = count(&form);
            let groups: Vec<&str> = full_cbl
                .lines()
                .filter_map(|l| {
                    let t = l.trim();
                    t.strip_prefix("01 ").filter(|n| n.starts_with("WS-") && n.ends_with('.') && !n.contains(' '))
                })
                .collect();
            let kept: Vec<&str> = groups
                .iter()
                .copied()
                .filter(|g| compact_cbl.lines().any(|l| l.trim() == format!("01 {g}")))
                .collect();
            for g in &kept {
                let name = g.trim_end_matches('.').to_ascii_uppercase();
                let hit = compact_cbl.lines().enumerate().find(|(_, l)| {
                    let t = l.trim();
                    !t.starts_with("01 ") && !t.starts_with("05 ") && whole_word_at(&l.to_ascii_uppercase(), &name).is_some()
                });
                let at = hit
                    .map(|(n, l)| format!("line {}: `{}`", n + 1, l.trim().chars().take(70).collect::<String>()))
                    .unwrap_or_else(|| "no line outside the declarations (kept by a longer name)".into());
                kept_rows.push(format!("| {project} | {rel} | `{}` | {at} |", name.trim_end_matches('.')));
            }
            let (gl, kl) = (groups.len(), kept.len());
            g += gl;
            k += kl;
            if gl > 0 {
                println!(
                    "| {project} | {rel} | {gl} | {kl} | {} | {} |",
                    gl - kl,
                    full.total - compact.total
                );
            }
        }
    }
    println!("| **Total** | | **{g}** | **{k}** | **{}** | |", g - k);
    println!("\n### Groups kept, and the line that names each\n");
    println!("| Project | Form | Group | First line outside its declaration |");
    println!("|---|---|---|---|");
    for r in kept_rows {
        println!("{r}");
    }
}

fn measure() {
    println!("| Project | Form | Total lines | Non-blank | Executable | User-typed | Generated-only | Bytes | Total lines (compact) | Bytes (compact) | Lines saved |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    let (mut all, mut all_c) = (Counts::default(), Counts::default());
    for project in PROJECTS {
        let dir = repo().join("examples").join(project).join("forms");
        let (mut p, mut pc) = (Counts::default(), Counts::default());
        for path in forms_in(&dir) {
            let rel = path.strip_prefix(&dir).unwrap().display().to_string();
            let mut form = load(&path);
            form.compact_storage = false;
            let (a, _) = count(&form);
            form.compact_storage = true;
            let (b, _) = count(&form);
            p.add(a);
            pc.add(b);
            println!(
                "| {project} | {rel} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                a.total, a.nonblank, a.executable, a.user, a.generated_only(), a.bytes, b.total, b.bytes, a.total - b.total
            );
        }
        if p.total > 0 {
            println!(
                "| **{project}** | all forms | **{}** | **{}** | **{}** | **{}** | **{}** | **{}** | **{}** | **{}** | **{}** |",
                p.total, p.nonblank, p.executable, p.user, p.generated_only(), p.bytes, pc.total, pc.bytes, p.total - pc.total
            );
        }
        all.add(p);
        all_c.add(pc);
    }
    println!(
        "| **All** | | **{}** | **{}** | **{}** | **{}** | **{}** | **{}** | **{}** | **{}** | **{}** |",
        all.total, all.nonblank, all.executable, all.user, all.generated_only(), all.bytes, all_c.total, all_c.bytes, all.total - all_c.total
    );
}


/// What a paragraph of the outer program is for, by the names the generator gives.
fn helper_kind(name: &str) -> &'static str {
    let n = name.to_ascii_uppercase();
    if n == "COBOL-EVENT-LOOP" {
        "event loop"
    } else if n == "COBOL-PLAY-ANIMATION" || n == "COBOL-STOP-ANIMATION" {
        "animation dispatch"
    } else if n.contains("-PLAY-") || n.contains("-STOP-") && n.starts_with("COBOL-") {
        "animation trigger"
    } else if ["-SET-TABLE", "-ADD-POINT", "-CLEAR", "-REFRESH"].iter().any(|s| n.ends_with(s)) {
        "chart wrapper"
    } else if ["-CONNECT", "-EXEC", "-FETCH-ALL", "-CLOSE", "-ON-CONNECT", "-ON-QUERY-DONE", "-ON-ERROR"].iter().any(|s| n.ends_with(s)) {
        "database wrapper"
    } else if n.ends_with("-EXPORT-CSV") {
        "csv export"
    } else if n.starts_with("COBOL-DATA-BINDING") || n.starts_with("COBOL-BIND") {
        "binding"
    } else if n.starts_with("COBOL-") {
        "other runtime paragraph"
    } else {
        "handler or procedure"
    }
}

fn helpers() {
    use std::collections::BTreeMap;
    println!("| Project | Kind of paragraph | Paragraphs | Lines | Named by authored code |");
    println!("|---|---|---|---|---|");
    let mut all: BTreeMap<&'static str, (usize, usize, usize)> = BTreeMap::new();
    for project in PROJECTS {
        let dir = repo().join("examples").join(project).join("forms");
        let mut rows: BTreeMap<&'static str, (usize, usize, usize)> = BTreeMap::new();
        for path in forms_in(&dir) {
            if path.to_string_lossy().contains(".recovered.") {
                continue;
            }
            let form = load(&path);
            let (cbl, user) = cobolt_codegen::generate_with_user_lines(&form);
            let lines: Vec<&str> = cbl.lines().collect();
            let user_text: String = user
                .iter()
                .flat_map(|(a, b)| lines[(*a as usize - 1)..(*b as usize).min(lines.len())].iter().copied())
                .collect::<Vec<_>>()
                .join("\n")
                .to_ascii_uppercase();
            // The outer program: from the first PROCEDURE DIVISION to the first nested unit.
            let Some(start) = lines.iter().position(|l| l.trim_start().to_ascii_uppercase().starts_with("PROCEDURE DIVISION")) else { continue };
            let end = lines[start..]
                .iter()
                .position(|l| l.trim_start().to_ascii_uppercase().starts_with("IDENTIFICATION DIVISION"))
                .map_or(lines.len(), |p| start + p);
            let labels: Vec<(usize, &str)> = (start..end)
                .filter_map(|i| {
                    let t = lines[i];
                    let name = t.strip_prefix("       ")?.strip_suffix('.')?;
                    (!name.is_empty() && !name.contains(' ') && !name.contains("DIVISION") && !name.starts_with('*'))
                        .then_some((i, name))
                })
                .collect();
            for (k, (at, name)) in labels.iter().enumerate() {
                let stop = labels.get(k + 1).map_or(end, |(n, _)| *n);
                let kind = helper_kind(name);
                let named = user_text.contains(&format!("PERFORM {}", name.to_ascii_uppercase()))
                    || user_text.contains(&format!("\"{}\"", name.to_ascii_uppercase()));
                let r = rows.entry(kind).or_default();
                r.0 += 1;
                r.1 += stop - at;
                r.2 += usize::from(named);
            }
        }
        for (kind, (p, l, n)) in &rows {
            println!("| {project} | {kind} | {p} | {l} | {n} |");
            let a = all.entry(kind).or_default();
            a.0 += p;
            a.1 += l;
            a.2 += n;
        }
    }
    for (kind, (p, l, n)) in &all {
        println!("| **All** | **{kind}** | **{p}** | **{l}** | **{n}** |");
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("inventory") => inventory(),
        Some("measure") => measure(),
        Some("helpers") => helpers(),
        _ => eprintln!("usage: spec109_measure inventory | measure | helpers"),
    }
}
