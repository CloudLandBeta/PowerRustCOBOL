// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **DESKTOP-WASM-001 R26 (T3): the ten-times reference project.**
//!
//! The 30 s Build goal (R17) is judged on every Desktop example without `EXEC RUST` or an
//! external crate, and on one generated project ten times the size of the largest of them in
//! each dimension (C7):
//! - **forms**: the `.cfrm` files the manifest lists;
//! - **COBOL lines**: the lines of every program a Build compiles, after the forms and indexed
//!   files are regenerated as a Build regenerates them (the generated programs and facades, the
//!   Common Code and the copybooks);
//! - **indexed files**: the `.cidx` files the manifest lists, with a floor of 10.
//!
//! The project is written deterministically, so every platform builds the same bytes (AC18):
//! fixed names, a fixed order, `/` in every path, no clock and no random source.

use std::path::{Path, PathBuf};

/// The reference projects of C7: the Desktop examples with neither `EXEC RUST` nor an external crate.
pub const REFERENCE_PROJECTS: &[&str] = &["PowerCallOfDuty", "PowerCloud", "PowerCobolCraft", "PowerGraphics", "PowerSpatial"];

/// The ten-times project has at least this many indexed files (C7: none of the five has one).
pub const INDEXED_FLOOR: usize = 10;

/// The size of a project in the three dimensions of R17.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dimensions {
    pub forms: usize,
    pub cobol_lines: usize,
    pub indexed: usize,
}

/// The `[files]` lists of a project manifest.
#[derive(Debug, Default, serde::Deserialize)]
struct Files {
    #[serde(default)]
    forms: Vec<String>,
    #[serde(default)]
    indexed: Vec<String>,
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    generated: Vec<String>,
    #[serde(default)]
    copybooks: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
struct Manifest {
    #[serde(default)]
    files: Files,
}

/// The project's manifest: the one `*.project.toml` in `dir`.
pub fn manifest_of(dir: &Path) -> Result<PathBuf, String> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".project.toml")))
        .collect();
    found.sort();
    match found.len() {
        1 => Ok(found.remove(0)),
        n => Err(format!("{}: {n} project manifests, expected one", dir.display())),
    }
}

/// Copies `src` into `dst`, leaving out what a Build or a run writes.
pub fn copy_project(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    let mut entries: Vec<_> = std::fs::read_dir(src)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name();
        if matches!(name.to_str(), Some("bin" | "dist" | "temp" | "debug" | "target" | "generated")) {
            continue;
        }
        let (from, to) = (entry.path(), dst.join(&name));
        if from.is_dir() {
            copy_project(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn line_count(path: &Path) -> usize {
    std::fs::read(path).map(|b| b.split(|c| *c == b'\n').filter(|l| !l.is_empty()).count()).unwrap_or(0)
}

/// Measures the project in `dir`, regenerating its forms and indexed files there first, as a
/// Build does. `dir` is written to: measure a copy (see [`copy_project`]).
pub fn measure(dir: &Path) -> Result<Dimensions, String> {
    let manifest = manifest_of(dir)?;
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let Manifest { mut files } = toml::from_str(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let done = cobolt_codegen::project::regenerate_project(dir, &files.forms, &files.indexed, &mut files.generated, &mut files.sources)
        .map_err(|e| format!("{}: regenerate: {e}", dir.display()))?;
    if let Some((rel, why)) = done.skipped.first() {
        return Err(format!("{}: {rel} was not regenerated: {why}", dir.display()));
    }
    let mut compiled: Vec<&String> = files.generated.iter().chain(&files.sources).chain(&files.copybooks).collect();
    // The `.SEL` and `.FD` copybooks of the indexed files, which the regeneration writes.
    let mut extra: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("COPYBOOKS")) {
        for e in entries.flatten() {
            let rel = format!("COPYBOOKS/{}", e.file_name().to_string_lossy());
            if !compiled.iter().any(|c| **c == rel) {
                extra.push(rel);
            }
        }
    }
    compiled.extend(extra.iter());
    compiled.sort();
    compiled.dedup();
    Ok(Dimensions {
        forms: files.forms.len(),
        cobol_lines: compiled.iter().map(|rel| line_count(&dir.join(rel))).sum(),
        indexed: files.indexed.len(),
    })
}

/// Ten times the largest of each dimension, with at least [`INDEXED_FLOOR`] indexed files.
pub fn target(of: &[Dimensions]) -> Dimensions {
    let max = |f: fn(&Dimensions) -> usize| of.iter().map(f).max().unwrap_or(0);
    Dimensions {
        forms: 10 * max(|d| d.forms),
        cobol_lines: 10 * max(|d| d.cobol_lines),
        indexed: (10 * max(|d| d.indexed)).max(INDEXED_FLOOR),
    }
}

/// The examples folder of this checkout.
pub fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// Measures each reference project on a copy under `scratch`, in [`REFERENCE_PROJECTS`] order.
pub fn measure_references(scratch: &Path) -> Result<Vec<(&'static str, Dimensions)>, String> {
    let mut out = Vec::new();
    for name in REFERENCE_PROJECTS {
        let copy = scratch.join(name);
        let _ = std::fs::remove_dir_all(&copy);
        copy_project(&examples_dir().join(name), &copy).map_err(|e| format!("copy {name}: {e}"))?;
        out.push((*name, measure(&copy)?));
        let _ = std::fs::remove_dir_all(&copy);
    }
    Ok(out)
}

/// The project's name and the stem of its manifest.
pub const PROJECT_NAME: &str = "TenTimes";

/// The statements of a form handler before its call, one per line.
const FORM_BODY_LINES: usize = 40;

/// The largest Common Code program, in lines.
const MAX_CALC_LINES: usize = 1000;

/// One line of straight-line COBOL, the `k`-th of a cycle of eight. Every line is a whole
/// statement, so a body can stop after any of them; each is safe to run on its own.
fn body_line(k: usize) -> String {
    let s = match k % 8 {
        0 => "ADD 1 TO WS-COUNT".to_string(),
        1 => format!("COMPUTE WS-TOTAL = WS-TOTAL + WS-COUNT * {}", k % 7 + 2),
        2 => "IF WS-TOTAL > 900000000 MOVE 0 TO WS-TOTAL END-IF".to_string(),
        3 => "DIVIDE WS-COUNT BY 10 GIVING WS-Q REMAINDER WS-R".to_string(),
        4 => format!("MOVE WS-COUNT TO WS-ITEM ({})", k % 10 + 1),
        5 => "IF WS-R = 0 ADD 1 TO WS-ZEROS END-IF".to_string(),
        6 => format!("MOVE \"STEP {:05}\" TO WS-TEXT", k % 100_000),
        _ => "INSPECT WS-TEXT TALLYING WS-HITS FOR ALL \"5\"".to_string(),
    };
    format!("           {s}\n")
}

/// The working storage every generated program and form shares in shape.
fn work_items(global: bool) -> String {
    let g = if global { " GLOBAL" } else { "" };
    format!(
        "       01 WS-COUNT{g} PIC 9(6) VALUE 0.\n\
         \x20      01 WS-TOTAL{g} PIC 9(9) VALUE 0.\n\
         \x20      01 WS-Q{g} PIC 9(6) VALUE 0.\n\
         \x20      01 WS-R{g} PIC 9(2) VALUE 0.\n\
         \x20      01 WS-ZEROS{g} PIC 9(6) VALUE 0.\n\
         \x20      01 WS-HITS{g} PIC 9(6) VALUE 0.\n\
         \x20      01 WS-TEXT{g} PIC X(20) VALUE SPACES.\n\
         \x20      01 WS-TABLE{g}.\n\
         \x20         05 WS-ITEM PIC 9(6) OCCURS 10 TIMES.\n"
    )
}

/// A Common Code program of exactly `lines` lines (no blank line): `CALC-<n>`, which works
/// through its body and adds its total to the caller's value.
fn calc_program(n: usize, lines: usize) -> String {
    let id = format!("CALC-{n:03}");
    let head = format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. {id}.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         {}\
         \x20      LINKAGE SECTION.\n\
         \x20      01 LK-VALUE PIC 9(9).\n\
         \x20      PROCEDURE DIVISION USING LK-VALUE.\n\
         \x20      MAIN-PARA.\n",
        work_items(false)
    );
    let tail = format!(
        "           ADD WS-TOTAL TO LK-VALUE\n\
         \x20          IF LK-VALUE > 900000000 MOVE 0 TO LK-VALUE END-IF\n\
         \x20          GOBACK.\n\
         \x20      END PROGRAM {id}.\n"
    );
    let fixed = head.lines().count() + tail.lines().count();
    let mut text = head;
    for k in 0..lines.saturating_sub(fixed) {
        text.push_str(&body_line(k + n));
    }
    text.push_str(&tail);
    text
}

/// The lines a [`calc_program`] cannot go below.
fn calc_floor() -> usize {
    calc_program(1, 0).lines().count()
}

/// `CALC-ALL`, which every form calls: it calls each Common Code program in turn.
fn calc_all(count: usize) -> String {
    let mut text = String::from(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. CALC-ALL.\n\
         \x20      DATA DIVISION.\n\
         \x20      LINKAGE SECTION.\n\
         \x20      01 LK-VALUE PIC 9(9).\n\
         \x20      PROCEDURE DIVISION USING LK-VALUE.\n\
         \x20      MAIN-PARA.\n",
    );
    for n in 1..=count {
        text.push_str(&format!(
            "      *> CALC-{n:03}: works through its statements and adds its total.\n\
             \x20     *> Uses:   LK-VALUE (the caller's running value).\n\
             \x20     *> Result: LK-VALUE grows by CALC-{n:03}'s total, wrapped at 900000000.\n\
             \x20          CALL \"CALC-{n:03}\" USING LK-VALUE\n"
        ));
    }
    text.push_str("           GOBACK.\n       END PROGRAM CALC-ALL.\n");
    text
}

fn calc_all_lines(count: usize) -> usize {
    calc_all(count).lines().count()
}

/// The indexed file `n`: its COBOL name and its `.cidx`.
fn indexed_name(n: usize) -> String {
    format!("IDX-{n:02}-FILE")
}

fn cidx(n: usize) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<IndexedFile name=\"{name}\" finalized=\"true\" version=\"1.0\">\n\
\x20 <assign-path>data/idx-{n:02}.idx</assign-path>\n\
\x20 <access-mode>dynamic</access-mode>\n\
\x20 <record-format fixed-length=\"45\"/>\n\
\x20 <storage mode=\"disk\" compression=\"false\" persistence=\"false\"/>\n\
\x20 <comment><![CDATA[Ten-times reference project: indexed file {n}]]></comment>\n\
\x20 <keys>\n\
\x20   <primary duplicates=\"false\" ordering=\"ascending\"><part field=\"IDX-{n:02}-ID\" offset=\"0\" length=\"6\" encoding=\"bytes\"/></primary>\n\
\x20 </keys>\n\
\x20 <fields>\n\
\x20   <Field level=\"1\" name=\"IDX-{n:02}-RECORD\" usage=\"display\">\n\
\x20     <Field level=\"5\" name=\"IDX-{n:02}-ID\" pic=\"9(6)\" usage=\"display\" offset=\"0\" length=\"6\"/>\n\
\x20     <Field level=\"5\" name=\"IDX-{n:02}-NAME\" pic=\"X(30)\" usage=\"display\" offset=\"6\" length=\"30\"/>\n\
\x20     <Field level=\"5\" name=\"IDX-{n:02}-AMOUNT\" pic=\"9(7)V99\" usage=\"display\" offset=\"36\" length=\"9\"/>\n\
\x20   </Field>\n\
\x20 </fields>\n\
</IndexedFile>\n",
        name = indexed_name(n)
    )
}

/// Form `i` of the project. The first `indexed` forms each keep one indexed file: they declare
/// it and write and read a record of it.
fn form(i: usize, indexed: usize) -> cobolt_forms::Form {
    use cobolt_forms::{Control, ControlType, EventBinding, Form};
    let mut form = Form::new(format!("FORM-{i:03}"), format!("Form {i:03}"), 640, 480);
    form.main_form = i == 1;
    form.responsive = true;
    form.definition = format!("Form {i:03} of the ten-times reference project, which measures how long a Build takes");
    let mut ws = work_items(true);
    ws.push_str("       01 WS-VALUE GLOBAL PIC 9(9) VALUE 0.\n");

    let mut title = Control::new("LBL-TITLE", ControlType::Label, 16, 16);
    title.set_prop("Text", format!("Form {i:03}"));
    title.set_prop("Definition", "The form's title, naming which of the ninety forms this is");
    form.controls.push(title);
    let mut input = Control::new("TXT-INPUT", ControlType::TextBox, 16, 56);
    input.set_prop("Definition", "A name the operator types, which Save stores in the indexed file");
    form.controls.push(input);
    let mut result = Control::new("LBL-RESULT", ControlType::Label, 16, 136);
    result.set_prop("Text", "");
    result.set_prop("Definition", "Shows the last text the form worked on, or the name read back");
    form.controls.push(result);

    let mut run = Control::new("BTN-RUN", ControlType::Button, 16, 96);
    run.set_prop("Text", "Run");
    run.set_prop("Definition", "Runs the form's calculation and every Common Code program over it");
    let mut on_click = EventBinding::for_control("BTN-RUN", "onClick");
    let mut code = String::from("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n");
    for k in 0..FORM_BODY_LINES {
        code.push_str(&body_line(k + i));
    }
    code.push_str(
        "           MOVE WS-TOTAL TO WS-VALUE\n\
         \x20          *> CALC-ALL: runs every Common Code program over the value.\n\
         \x20          *> Uses:   WS-VALUE (this form's total, moved just above).\n\
         \x20          *> Result: WS-VALUE grows by each program's total, wrapped at 900000000.\n\
         \x20          CALL \"CALC-ALL\" USING WS-VALUE\n\
         \x20          MOVE WS-TEXT TO LBL-RESULT::Text\n",
    );
    on_click.code = code;
    run.events.push(on_click);
    form.controls.push(run);

    if i <= indexed {
        let name = indexed_name(i);
        form.cobol_structure.file_control = format!(
            "           SELECT {name} ASSIGN TO \"data/idx-{i:02}.idx\"\n\
             \x20              ORGANIZATION IS INDEXED\n\
             \x20              ACCESS MODE IS DYNAMIC\n\
             \x20              RECORD KEY IS IDX-{i:02}-ID\n\
             \x20              FILE STATUS IS WS-FS.\n"
        );
        form.cobol_structure.file_section = format!(
            "       FD  {name} IS GLOBAL.\n\
             \x20      01  IDX-{i:02}-RECORD.\n\
             \x20          05 IDX-{i:02}-ID     PIC 9(6).\n\
             \x20          05 IDX-{i:02}-NAME   PIC X(30).\n\
             \x20          05 IDX-{i:02}-AMOUNT PIC 9(7)V99.\n"
        );
        ws.push_str("       01 WS-FS GLOBAL PIC XX VALUE SPACES.\n");
        let mut save = Control::new("BTN-SAVE", ControlType::Button, 120, 96);
        save.set_prop("Text", "Save");
        save.set_prop("Definition", "Writes a record with the typed name to the indexed file and reads it back");
        let mut on_save = EventBinding::for_control("BTN-SAVE", "onClick");
        on_save.code = format!(
            "       ENVIRONMENT DIVISION.\n\
             \x20      DATA DIVISION.\n\
             \x20      PROCEDURE DIVISION.\n\
             \x20          OPEN I-O {name}\n\
             \x20          IF WS-FS = \"35\"\n\
             \x20              OPEN OUTPUT {name}\n\
             \x20          END-IF\n\
             \x20          ADD 1 TO WS-COUNT\n\
             \x20          MOVE WS-COUNT TO IDX-{i:02}-ID\n\
             \x20          MOVE TXT-INPUT::Text TO IDX-{i:02}-NAME\n\
             \x20          MOVE WS-TOTAL TO IDX-{i:02}-AMOUNT\n\
             \x20          WRITE IDX-{i:02}-RECORD\n\
             \x20              INVALID KEY REWRITE IDX-{i:02}-RECORD\n\
             \x20          END-WRITE\n\
             \x20          READ {name} KEY IS IDX-{i:02}-ID\n\
             \x20              INVALID KEY MOVE \"MISSING\" TO WS-TEXT\n\
             \x20              NOT INVALID KEY MOVE IDX-{i:02}-NAME TO WS-TEXT\n\
             \x20          END-READ\n\
             \x20          CLOSE {name}\n\
             \x20          MOVE WS-TEXT TO LBL-RESULT::Text\n"
        );
        save.events.push(on_save);
        form.controls.push(save);
    }
    form.user_ws_source = ws;
    form
}

fn manifest_text(forms: &[String], indexed: &[String], sources: &[String]) -> String {
    let list = |items: &[String]| items.iter().map(|s| format!("    \"{s}\",\n")).collect::<String>();
    format!(
        "[project]\nname = \"{PROJECT_NAME}\"\nversion = \"1.0.0\"\nmain = \"\"\n\n\
         [files]\nforms = [\n{}]\nindexed = [\n{}]\nsources = [\n{}]\n",
        list(forms),
        list(indexed),
        list(sources)
    )
}

/// Writes the ten-times project for `target` into `dir` (emptied first) and returns its
/// measured dimensions, which equal `target`. The project holds only what a developer writes:
/// the manifest, `forms/`, `indexed/` and `src/`, never `generated/`.
pub fn generate(target: &Dimensions, dir: &Path) -> Result<Dimensions, String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    let _ = std::fs::remove_dir_all(dir);
    for sub in ["forms", "indexed", "src", "data"] {
        std::fs::create_dir_all(dir.join(sub)).map_err(io)?;
    }
    let forms: Vec<String> = (1..=target.forms).map(|i| format!("forms/form-{i:03}.cfrm")).collect();
    let indexed: Vec<String> = (1..=target.indexed).map(|n| format!("indexed/idx-{n:02}.cidx")).collect();
    for (i, rel) in forms.iter().enumerate() {
        cobolt_forms::save_form(&form(i + 1, target.indexed), &dir.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
    }
    for (n, rel) in indexed.iter().enumerate() {
        std::fs::write(dir.join(rel), cidx(n + 1)).map_err(io)?;
    }
    std::fs::write(dir.join("data/README.md"), "The indexed files of the ten-times project are written here.\n").map_err(io)?;
    let manifest = dir.join(format!("{PROJECT_NAME}.project.toml"));

    // How many lines the forms and the indexed files bring once regenerated; the Common Code
    // makes up the rest.
    let scratch = dir.with_extension("measure");
    let measured = || -> Result<Dimensions, String> {
        let _ = std::fs::remove_dir_all(&scratch);
        copy_project(dir, &scratch).map_err(|e| format!("copy: {e}"))?;
        let d = measure(&scratch);
        let _ = std::fs::remove_dir_all(&scratch);
        d
    };
    std::fs::write(&manifest, manifest_text(&forms, &indexed, &[])).map_err(io)?;
    let base = measured()?.cobol_lines;
    let rest = target.cobol_lines.checked_sub(base).ok_or_else(|| {
        format!("the forms and indexed files alone come to {base} lines, over the target of {}", target.cobol_lines)
    })?;
    let count = (1..)
        .find(|&k| rest.saturating_sub(calc_all_lines(k)) <= k * MAX_CALC_LINES)
        .expect("a count is always found");
    let calc_total = rest
        .checked_sub(calc_all_lines(count))
        .filter(|t| *t >= count * calc_floor())
        .ok_or_else(|| format!("{rest} lines are too few for {count} Common Code programs"))?;
    let mut sources = vec!["src/calc-all.cbl".to_string()];
    std::fs::write(dir.join("src/calc-all.cbl"), calc_all(count)).map_err(io)?;
    for n in 1..=count {
        let lines = calc_total / count + usize::from(n <= calc_total % count);
        let rel = format!("src/calc-{n:03}.cbl");
        std::fs::write(dir.join(&rel), calc_program(n, lines)).map_err(io)?;
        sources.push(rel);
    }
    std::fs::write(&manifest, manifest_text(&forms, &indexed, &sources)).map_err(io)?;
    measured()
}

/// The sha256 of every file under `dir`: each file's path from `dir` (with `/`), its length and
/// its bytes, in path order.
pub fn tree_hash(dir: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
        for e in std::fs::read_dir(dir)?.flatten() {
            let path = e.path();
            if path.is_dir() {
                walk(root, &path, out)?;
            } else {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
                out.push((rel, path));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, dir, &mut files).map_err(|e| format!("{}: {e}", dir.display()))?;
    files.sort();
    let mut h = Sha256::new();
    for (rel, path) in &files {
        let bytes = std::fs::read(path).map_err(|e| format!("{rel}: {e}"))?;
        h.update(rel.as_bytes());
        h.update([0]);
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(&bytes);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-ten-times-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// **DESKTOP-WASM-001 T3 (R26; AC18 generation).** Measures the five reference projects,
    /// writes the ten-times project twice and compares the two trees byte for byte, then has the
    /// project tools `check` it as an agent would.
    #[test]
    fn ten_times_project_is_ten_times_and_deterministic() {
        let base = scratch("t3");
        let t = std::time::Instant::now();
        let refs = measure_references(&base.join("refs")).unwrap();
        let measure_ms = t.elapsed().as_millis();
        let goal = target(&refs.iter().map(|(_, d)| *d).collect::<Vec<_>>());

        let t = std::time::Instant::now();
        let first = generate(&goal, &base.join("one").join(PROJECT_NAME)).unwrap();
        let generate_ms = t.elapsed().as_millis();
        let second = generate(&goal, &base.join("two").join(PROJECT_NAME)).unwrap();
        let hash_one = tree_hash(&base.join("one").join(PROJECT_NAME)).unwrap();
        let hash_two = tree_hash(&base.join("two").join(PROJECT_NAME)).unwrap();

        let project = base.join("one").join(PROJECT_NAME);
        let t = std::time::Instant::now();
        let mut tools = cobolt_project_tools::ProjectTools::new(cobolt_project_tools::HeadlessHost::new(base.clone(), "test"));
        let manifest = project.join(format!("{PROJECT_NAME}.project.toml"));
        tools.call("open_project", &serde_json::json!({"path": manifest.to_string_lossy()})).unwrap();
        tools.call("regenerate", &serde_json::json!({})).unwrap();
        let checked: serde_json::Value = tools.call("check", &serde_json::json!({})).unwrap();
        let check_ms = t.elapsed().as_millis();

        println!("== ten_times_project_is_ten_times_and_deterministic (DESKTOP-WASM-001 T3, R26) ==");
        println!("reference projects (measured in {measure_ms} ms, after regenerating each copy):");
        for (name, d) in &refs {
            println!("  {name:<16} forms {:>3}  COBOL lines {:>6}  indexed files {:>2}", d.forms, d.cobol_lines, d.indexed);
        }
        println!("  target           forms {:>3}  COBOL lines {:>6}  indexed files {:>2}  (ten times the largest; indexed floor {INDEXED_FLOOR})", goal.forms, goal.cobol_lines, goal.indexed);
        println!("  generated        forms {:>3}  COBOL lines {:>6}  indexed files {:>2}  (written in {generate_ms} ms)", first.forms, first.cobol_lines, first.indexed);
        println!("tree sha256: {hash_one} (first), {hash_two} (second)");
        println!("check: {} error(s), {} warning(s), in {check_ms} ms", checked["errors"], checked["warnings"]);
        let checks = [
            ("generated equals the target", first == goal),
            ("the second generation measures the same", second == first),
            ("the two trees hash the same", hash_one == hash_two),
            ("check reports no error", checked["errors"] == 0),
            ("check reports no warning", checked["warnings"] == 0),
        ];
        let passed = checks.iter().filter(|(_, ok)| *ok).count();
        for (what, ok) in &checks {
            println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
        }
        println!("result: {passed} of {} checks passed", checks.len());
        if let Some(first) = checked["diagnostics"].as_array().and_then(|d| d.first()) {
            println!("first diagnostic: {first}");
        }
        let _ = std::fs::remove_dir_all(&base);
        assert_eq!(passed, checks.len());
    }
}
