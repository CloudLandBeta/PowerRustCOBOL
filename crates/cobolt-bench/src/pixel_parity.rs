// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **DESKTOP-WASM-001 T7: `cobolt-bench pixel-parity`** (R27's harness; AC19's second guard;
//! AC20 for each form).
//!
//! ```sh
//! cargo run -p cobolt-bench --release -- pixel-parity --write-scripts         # the base scripts
//! cargo run -p cobolt-bench --release -- pixel-parity --cargo                 # against the goldens
//! cargo run -p cobolt-bench --release -- pixel-parity --cargo --record        # write the goldens
//! cargo run -p cobolt-bench --release -- pixel-parity --cargo --project PowerSpatial
//! cargo run -p cobolt-bench --release -- pixel-parity --console               # console programs, T24
//! ```
//!
//! **What it runs:** the five reference projects (C7) **as committed** (`HEAD`), never the working
//! tree, so a golden recorded on one machine is the one a clean checkout compares with. Each is
//! copied out of git into a scratch folder, and in the copy every AWS connection is routed to a
//! server that does not exist. Every run also gets an empty home folder, no AWS variables and a
//! proxy that refuses connections. Nothing a script raises can reach the network.
//!
//! **The base scripts** (`tests/pixel-parity/scripts/<project>/<form>.json`, written by
//! `--write-scripts`) raise every bound event of the form once, in the order of the file, with a
//! `shot` after each:
//! - a handler that ends the form (`me::Close`, `STOP RUN`) goes last;
//! - a handler that opens a form modally (`OpenFormSync`, even one that closes after it) is raised
//!   without waiting, just before; once one is, the handlers that end the form are raised without
//!   waiting too, as the modal form holds their program up;
//! - a handler that opens a file dialog gets an `answer` step (cancel) before it;
//! - a handler that reaches the network (a control of an AWS, agent, REST, SQL, knowledge-base,
//!   map, search or sign-in type, `COBOL::"AWS-…"`, `EXEC SQL`) is not raised.
//! - in a project whose forms open with an entrance effect (`[forms] entrance-effect`), the script
//!   waits for the longest effect to end (`MATRIX_HARD_MAX_MS`) after the first picture and takes
//!   another (`00-ready`), since T8. An event moves no virtual clock, so otherwise every later
//!   picture would show the effect where it stood when the form opened.
//!
//! The script's `comment` names each of them. A handler's text includes the procedures it calls.
//!
//! **The runs:** each project is built on the Cargo path, then every script runs three times: at
//! the form's designed size, then 800 and 480 wide (the Medium and Compact widths of AC19), the
//! height kept. Each of those runs twice (three times with `--record`), from the same data, under
//! the pins of R28
//! (`PRC_TEST_CLOCK`, `PRC_TEST_SEED`), the form named by `COBOLT_DESIGNER_FORM`.
//!
//! **A frame's hash** is the sha256 of its width and height (each a little-endian u32) followed by
//! its RGBA bytes. A form whose frames are the same in every run is repeatable, and `--record`
//! writes its hashes into `tests/pixel-parity/golden/<platform>.toml`. A form that is not is named
//! "not repeatable". Since T8 the host keeps the run's virtual time, so every form must repeat
//! (AC20): without `--record` a form that does not fails the run.
//!
//! **Without `--record`** every recorded frame must equal its golden. A frame that differs fails
//! the run. The frames of both runs and a picture of the differing pixels go under
//! `target/pixel-parity/`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::build_goal::{platform, product_version, repo_root};
use crate::ten_times;

/// The pinned clock and seed every run takes (R28).
pub(crate) const CLOCK: &str = "2026-01-02T03:04:05-03:00";
pub(crate) const SEED: u64 = 20_261_114;
/// A run's own time limit (`PRC_HEADLESS_LIMIT`), and how much longer the harness waits for it.
const LIMIT_SECS: u64 = 60;
const GRACE: Duration = Duration::from_secs(60);
/// The three window sizes of AC19: the designed one, then the Medium and Compact widths.
const SIZES: [(&str, Option<f32>); 3] = [("designed", None), ("medium", Some(800.0)), ("compact", Some(480.0))];
/// How many times each script runs at each size. Two to check; three to record, because a form
/// whose frames depend on timing can draw the same twice by chance (PowerGraphics' bridge form
/// did, at T7's first recording) and a golden must not be recorded from luck.
fn runs_per_size(record: bool) -> usize {
    if record {
        3
    } else {
        2
    }
}
/// How long a modal form is given to open before its frame is taken.
const MODAL_WAIT_MS: u64 = 500;

/// T8: how long a base script waits, after the picture of the form opening, for an entrance effect
/// to end. MatrixRain's hard ceiling, which is longer than any other effect's (`FX_MAX_MS`).
const ENTRANCE_WAIT_MS: u64 = cobolt_forms::window_fx::MATRIX_HARD_MAX_MS as u64;
/// The line a headless run reports on (`cobolt_form_host::headless::RESULT_MARKER`).
const MARKER: &str = "@RUN-FORM-RESULT ";

struct Args {
    write_scripts: bool,
    /// T24: the console fixtures on both paths (`console_parity`).
    console: bool,
    cargo: bool,
    record: bool,
    project: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args { write_scripts: false, console: false, cargo: false, record: false, project: None };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--write-scripts" => out.write_scripts = true,
            "--console" => out.console = true,
            "--cargo" => out.cargo = true,
            "--record" => out.record = true,
            "--project" => out.project = Some(it.next().ok_or("--project takes a reference project's name")?.clone()),
            other => {
                return Err(format!(
                    "pixel-parity: unknown argument {other} (known: --write-scripts, --cargo, --record, --project NAME, --console)"
                ))
            }
        }
    }
    if out.console {
        if out.write_scripts || out.cargo || out.record || out.project.is_some() {
            return Err("pixel-parity: --console runs alone".into());
        }
        return Ok(out);
    }
    if !out.write_scripts && !out.cargo {
        return Err("pixel-parity: say what to do: --write-scripts, --cargo, both, or --console".into());
    }
    if out.record && !out.cargo {
        return Err("pixel-parity: --record needs --cargo".into());
    }
    if let Some(p) = &out.project {
        if !ten_times::REFERENCE_PROJECTS.contains(&p.as_str()) {
            return Err(format!("pixel-parity: {p} is not a reference project ({})", ten_times::REFERENCE_PROJECTS.join(", ")));
        }
    }
    Ok(out)
}

// ── The committed projects ───────────────────────────────────────────────────────────────────────

fn git(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_root())
        .args(args)
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout)
}

/// Writes `examples/<name>` as committed at `HEAD` into `dst`, without what a Build writes.
fn copy_committed(name: &str, dst: &Path) -> Result<(), String> {
    let prefix = format!("examples/{name}/");
    let listed = git(&["ls-tree", "-r", "-z", "--name-only", "HEAD", "--", &prefix])?;
    let mut copied = 0;
    for path in listed.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let path = String::from_utf8_lossy(path).into_owned();
        let rel = &path[prefix.len()..];
        if matches!(rel.split('/').next(), Some("bin" | "dist" | "temp" | "debug" | "target" | "generated")) {
            continue;
        }
        let bytes = git(&["show", &format!("HEAD:{path}")])?;
        let to = dst.join(rel);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&to, bytes).map_err(|e| format!("{}: {e}", to.display()))?;
        copied += 1;
    }
    if copied == 0 {
        return Err(format!("{prefix} has no committed file"));
    }
    Ok(())
}

/// Routes every AWS connection of the copy to a server that does not exist, with no profile.
/// Returns how many there were.
fn cut_off_aws(manifest: &Path, scratch: &Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let mut doc: toml::Value = toml::from_str(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let routes = format!("[servers.hosted]\ncommand = '{}'\nargs = []\n", scratch.join("no-network-server").display());
    let mut n = 0;
    if let Some(list) = doc.get_mut("integrations").and_then(|i| i.get_mut("aws_connections")).and_then(|a| a.as_array_mut()) {
        for conn in list.iter_mut().filter_map(toml::Value::as_table_mut) {
            conn.insert("profile".into(), "".into());
            conn.insert("routes_override".into(), routes.clone().into());
            n += 1;
        }
    }
    if n == 0 {
        return Ok(0);
    }
    let text = toml::to_string(&doc).map_err(|e| e.to_string())?;
    std::fs::write(manifest, text).map_err(|e| format!("{}: {e}", manifest.display()))?;
    Ok(n)
}

// ── The base scripts ─────────────────────────────────────────────────────────────────────────────

/// A control type whose methods reach outside the machine.
fn reaches_network(type_name: &str) -> bool {
    type_name.starts_with("Aws")
        || matches!(type_name, "AgentObject" | "RestClient" | "SqlDatabase" | "KnowledgeBase" | "Maps" | "WebSearch" | "Authentication")
}

fn all_controls<'a>(cs: &'a [cobolt_forms::Control], out: &mut Vec<&'a cobolt_forms::Control>) {
    for c in cs {
        out.push(c);
        all_controls(&c.children, out);
    }
}

/// The names in `CALL "NAME"` in `text` (upper case).
fn called(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("CALL") {
        rest = &rest[at + 4..];
        let t = rest.trim_start();
        if let Some(q) = t.strip_prefix('"') {
            if let Some(end) = q.find('"') {
                out.push(q[..end].trim().to_owned());
            }
        }
    }
    out
}

#[derive(PartialEq)]
enum Kind {
    Plain,
    Modal,
    Ends,
}

struct BaseScript {
    form: String,
    comment: String,
    steps: Vec<Value>,
}

/// Whether the project's forms open with an entrance effect (`[forms] entrance-effect` in its
/// manifest), as the product reads the setting.
fn opens_with_effect(manifest: &str) -> bool {
    let value: toml::Value = toml::from_str(manifest).unwrap_or(toml::Value::Table(Default::default()));
    let effect = value.get("forms").and_then(|f| f.get("entrance-effect")).and_then(toml::Value::as_str).unwrap_or("");
    cobolt_forms::window_fx::WindowEffect::from_str(effect) != cobolt_forms::window_fx::WindowEffect::None
}

/// The base script of `form` (see the module's comment). `entrance`: the project's forms open with
/// an entrance effect.
fn base_script(form: &cobolt_forms::Form, entrance: bool) -> BaseScript {
    let procs: BTreeMap<String, String> =
        form.user_procedures.iter().map(|p| (p.name.trim().to_ascii_uppercase(), p.code.to_ascii_uppercase())).collect();
    // A handler's text, with the procedures it calls, theirs, and so on.
    let reach = |code: &str| -> String {
        let mut text = code.to_ascii_uppercase();
        let mut seen = BTreeSet::new();
        let mut todo = called(&text);
        while let Some(name) = todo.pop() {
            if let Some(body) = procs.get(&name) {
                if seen.insert(name) {
                    todo.extend(called(body));
                    text.push('\n');
                    text.push_str(body);
                }
            }
        }
        text
    };
    let mut controls = Vec::new();
    all_controls(&form.controls, &mut controls);
    let network: Vec<String> = controls
        .iter()
        .filter(|c| reaches_network(&format!("{:?}", c.control_type)))
        .map(|c| c.id.to_ascii_uppercase())
        .collect();
    let mut bound: Vec<(String, String, String)> = controls
        .iter()
        .flat_map(|c| c.events.iter().map(move |e| (c.id.clone(), e.event.clone(), e.code.clone())))
        .collect();
    bound.extend(
        form.form_events
            .iter()
            .filter(|e| !matches!(e.event.as_str(), "onLoad" | "onClose"))
            .map(|e| (form.name.clone(), e.event.clone(), e.code.clone())),
    );

    let (mut plain, mut modal, mut ends) = (Vec::new(), Vec::new(), Vec::new());
    let (mut offline, mut answered) = (Vec::new(), Vec::new());
    for (control, event, code) in bound {
        if code.trim().is_empty() {
            continue;
        }
        let text = reach(&code);
        let label = format!("{control}.{event}");
        let online = text.contains("COBOL::\"AWS-")
            || text.contains("EXEC SQL")
            || network.iter().any(|id| text.contains(&format!("{id}::")) || text.contains(&format!("\"{id}\"")));
        if online {
            offline.push(label);
            continue;
        }
        let dialog = ["OPEN-FILE-DIALOG", "SAVE-FILE-DIALOG", "FOLDER-DIALOG"].iter().any(|d| text.contains(d));
        if dialog {
            answered.push(label.clone());
        }
        // A handler that opens a form modally and closes after it is a modal one: it does not end
        // the form until the modal form closes, which no script step does.
        let kind = if text.contains("OPENFORMSYNC") {
            Kind::Modal
        } else if text.contains("ME::CLOSE") || text.contains("STOP RUN") {
            Kind::Ends
        } else {
            Kind::Plain
        };
        let entry = (control, event, dialog, label);
        match kind {
            Kind::Plain => plain.push(entry),
            Kind::Modal => modal.push(entry),
            Kind::Ends => ends.push(entry),
        }
    }

    let mut steps = vec![json!({"shot": "00-open"})];
    // In virtual time an event moves no clock, so the frames after it would all show the entrance
    // effect where it stood when the form opened. Let it end first: the form as it is ready to use.
    if entrance {
        steps.push(json!({"wait_ms": ENTRANCE_WAIT_MS}));
        steps.push(json!({"shot": "00-ready"}));
    }
    let mut shot = 1;
    let named = |list: &[(String, String, bool, String)]| list.iter().map(|e| e.3.clone()).collect::<Vec<_>>();
    let (modal_names, ends_names) = (named(&modal), named(&ends));
    // Once a modal form is open its opener's program waits for it, so every event after the first
    // modal one is raised without waiting too: none of them is handled while the modal form is up.
    let held_up = !modal.is_empty();
    for (kind, list) in [(Kind::Plain, plain), (Kind::Modal, modal), (Kind::Ends, ends)] {
        for (control, event, dialog, _) in list {
            if dialog {
                steps.push(json!({"answer": {"cancel": true}}));
            }
            let raise = json!({"control": control, "name": event});
            if kind == Kind::Modal || (kind == Kind::Ends && held_up) {
                steps.push(json!({"event": raise, "wait": false}));
                steps.push(json!({"wait_ms": MODAL_WAIT_MS}));
            } else {
                steps.push(json!({"event": raise}));
            }
            let safe: String =
                format!("{control}-{event}").chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
            steps.push(json!({"shot": format!("{shot:02}-{safe}")}));
            shot += 1;
        }
    }
    let list = |v: &[String]| if v.is_empty() { "none".to_owned() } else { v.join(", ") };
    let comment = format!(
        "Base script (DESKTOP-WASM-001 T7), written by `cobolt-bench pixel-parity --write-scripts`: every bound event \
         raised once, in the order of the file, with a frame after each. Raised without waiting, as they open a form \
         modally: {}. Last, as they end the form (without waiting too when a modal form is open): {}. Answered with cancel, as they open a file dialog: {}. Not raised, \
         as they reach the network: {}.",
        list(&modal_names),
        list(&ends_names),
        list(&answered),
        list(&offline)
    );
    BaseScript { form: form.name.clone(), comment, steps }
}

fn scripts_dir(project: &str) -> PathBuf {
    repo_root().join("tests/pixel-parity/scripts").join(project)
}

/// A form's file stem under `forms/`, a subfolder joined with `__`.
fn form_stem(rel: &str) -> String {
    let rel = rel.strip_prefix("forms/").unwrap_or(rel);
    rel.strip_suffix(".cfrm").unwrap_or(rel).replace(['/', '\\'], "__")
}

// ── The runs ─────────────────────────────────────────────────────────────────────────────────────

/// The sha256 of a frame: its width and height, then its RGBA bytes.
fn frame_hash(png: &Path) -> Result<String, String> {
    let img = image::open(png).map_err(|e| format!("{}: {e}", png.display()))?.to_rgba8();
    let mut h = Sha256::new();
    h.update(img.width().to_le_bytes());
    h.update(img.height().to_le_bytes());
    h.update(img.as_raw());
    Ok(format!("{:x}", h.finalize()))
}

/// One run of a built application on one script: the hash of every frame, by shot name.
struct Run {
    frames: BTreeMap<String, String>,
    shots_dir: PathBuf,
    timed_out: bool,
}

struct Launch<'a> {
    binary: &'a Path,
    project: &'a Path,
    pristine_data: Option<&'a Path>,
    form_id: &'a str,
}

impl Launch<'_> {
    fn run(&self, steps: &[Value], out: &Path) -> Result<Run, String> {
        let _ = std::fs::remove_dir_all(out);
        let (shots, home) = (out.join("shots"), out.join("home"));
        for d in [&shots, &home] {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        // Every run starts from the same data.
        if let Some(pristine) = self.pristine_data {
            let data = self.project.join("data");
            let _ = std::fs::remove_dir_all(&data);
            ten_times::copy_project(pristine, &data).map_err(|e| format!("{}: {e}", data.display()))?;
        }
        let script = out.join("script.json");
        std::fs::write(&script, Value::Array(steps.to_vec()).to_string()).map_err(|e| e.to_string())?;
        let stdout = std::fs::File::create(out.join("stdout.txt")).map_err(|e| e.to_string())?;
        let stderr = std::fs::File::create(out.join("stderr.txt")).map_err(|e| e.to_string())?;
        let refuse = "http://127.0.0.1:9";
        let mut cmd = Command::new(self.binary);
        cmd.current_dir(self.project)
            .env("PRC_HEADLESS_SCRIPT", &script)
            .env("PRC_HEADLESS_SHOTS", &shots)
            .env("PRC_HEADLESS_LIMIT", LIMIT_SECS.to_string())
            .env("PRC_TEST_CLOCK", CLOCK)
            .env("PRC_TEST_SEED", SEED.to_string())
            .env("COBOLT_DESIGNER_FORM", self.form_id)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("AWS_SHARED_CREDENTIALS_FILE", home.join("no-credentials"))
            .env("AWS_CONFIG_FILE", home.join("no-config"))
            .env_remove("AWS_ACCESS_KEY_ID")
            .env_remove("AWS_SECRET_ACCESS_KEY")
            .env_remove("AWS_SESSION_TOKEN")
            .env_remove("AWS_PROFILE")
            .env_remove("NO_PROXY")
            .env_remove("no_proxy")
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr);
        for var in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"] {
            cmd.env(var, refuse);
        }
        let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", self.binary.display()))?;
        let deadline = Instant::now() + Duration::from_secs(LIMIT_SECS) + GRACE;
        let status = loop {
            if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
                break s;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("still running after {} s", (LIMIT_SECS + GRACE.as_secs())));
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let text = std::fs::read_to_string(out.join("stdout.txt")).unwrap_or_default();
        let Some(report) = text.lines().rev().find_map(|l| l.strip_prefix(MARKER)).and_then(|j| serde_json::from_str::<Value>(j).ok())
        else {
            let err = std::fs::read_to_string(out.join("stderr.txt")).unwrap_or_default();
            let tail: Vec<&str> = err.lines().rev().take(3).collect();
            return Err(format!("no result ({status}): {}", tail.into_iter().rev().collect::<Vec<_>>().join(" / ")));
        };
        let mut frames = BTreeMap::new();
        for entry in std::fs::read_dir(&shots).map_err(|e| e.to_string())?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("png") {
                let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                frames.insert(name, frame_hash(&path)?);
            }
        }
        Ok(Run { frames, shots_dir: shots, timed_out: report.get("timed_out").and_then(Value::as_bool) == Some(true) })
    }
}

/// Writes both frames and a picture of the pixels that differ (red) into `dir`. Returns how many
/// pixels differ, or `None` when the frames are not the same size.
fn picture_difference(a: &Path, b: &Path, dir: &Path, shot: &str) -> Result<Option<u64>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let _ = std::fs::copy(a, dir.join(format!("{shot}-a.png")));
    let _ = std::fs::copy(b, dir.join(format!("{shot}-b.png")));
    let (ia, ib) = (
        image::open(a).map_err(|e| e.to_string())?.to_rgba8(),
        image::open(b).map_err(|e| e.to_string())?.to_rgba8(),
    );
    if ia.dimensions() != ib.dimensions() {
        return Ok(None);
    }
    let mut diff = image::RgbaImage::new(ia.width(), ia.height());
    let mut n = 0;
    for (x, y, pa) in ia.enumerate_pixels() {
        let pb = ib.get_pixel(x, y);
        let out = if pa == pb {
            let grey = ((pa[0] as u16 + pa[1] as u16 + pa[2] as u16) / 9) as u8;
            image::Rgba([grey, grey, grey, 255])
        } else {
            n += 1;
            image::Rgba([255, 0, 0, 255])
        };
        diff.put_pixel(x, y, out);
    }
    diff.save(dir.join(format!("{shot}-diff.png"))).map_err(|e| e.to_string())?;
    Ok(Some(n))
}

// ── The goldens ──────────────────────────────────────────────────────────────────────────────────

/// `tests/pixel-parity/golden/<platform>.toml`: project → form → size → shot → hash.
#[derive(Serialize, Deserialize, Default)]
struct Golden {
    version: String,
    commit: String,
    clock: String,
    seed: u64,
    #[serde(default)]
    not_repeatable: BTreeMap<String, String>,
    #[serde(default)]
    frames: BTreeMap<String, BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>>,
}

fn golden_path() -> PathBuf {
    repo_root().join("tests/pixel-parity/golden").join(format!("{}.toml", platform()))
}

const GOLDEN_HEADER: &str = "\
# DESKTOP-WASM-001 T7 (R27, AC19's second guard): the hash of every frame the Cargo-built reference
# projects draw under their base scripts, on this platform, with the pinned clock and seed below.
# A hash is the sha256 of the frame's width and height (little-endian u32) and its RGBA bytes.
# Written by `cobolt-bench pixel-parity --cargo --record`. Re-recorded only by a change that moves
# a pixel on purpose, named in the CHANGELOG. A form is recorded only when three runs of the same
# binary draw every frame the same; `not_repeatable` lists any other, which breaks AC20 and is not
# recorded.

";

// ── The mode ─────────────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct FormResult {
    stem: String,
    steps: usize,
    frames: usize,
    identical: usize,
    /// Every frame of the first runs, by size, when both runs agree.
    hashes: BTreeMap<String, BTreeMap<String, String>>,
    problems: Vec<String>,
    golden_match: usize,
    golden_differ: Vec<String>,
}

impl FormResult {
    fn repeatable(&self) -> bool {
        self.problems.is_empty() && self.frames > 0 && self.frames == self.identical
    }
}

struct ProjectResult {
    name: String,
    forms: usize,
    scripted: usize,
    build: Duration,
    runs: Duration,
    results: Vec<FormResult>,
    failed: Option<String>,
}

/// Runs one form's script at the three sizes, `runs` times each, and compares the runs: a frame
/// is identical when every run drew it the same.
fn run_form(launch: &Launch<'_>, stem: &str, steps: &[Value], height: f32, runs: usize, work: &Path, out_root: &Path) -> FormResult {
    let mut r = FormResult { stem: stem.to_owned(), steps: steps.len(), ..Default::default() };
    for (size, width) in SIZES {
        let mut script = Vec::new();
        if let Some(w) = width {
            script.push(json!({"size": [w, height]}));
        }
        script.extend(steps.iter().cloned());
        let done: Result<Vec<Run>, String> =
            (0..runs).map(|i| launch.run(&script, &work.join(format!("{size}-{}", (b'a' + i as u8) as char)))).collect();
        let all = match done {
            Ok(all) => all,
            Err(e) => {
                r.problems.push(format!("{size}: {e}"));
                continue;
            }
        };
        if all.iter().any(|run| run.timed_out) {
            r.problems.push(format!("{size}: the run reached its {LIMIT_SECS} s limit"));
        }
        let first = &all[0];
        let names: BTreeSet<&String> = all.iter().flat_map(|run| run.frames.keys()).collect();
        for name in names {
            r.frames += 1;
            let hashes: Vec<Option<&String>> = all.iter().map(|run| run.frames.get(name)).collect();
            if hashes.iter().any(Option::is_none) {
                r.problems.push(format!("{size}: frame {name} was not taken in every run"));
            } else if let Some(other) = all.iter().find(|run| run.frames.get(name) != first.frames.get(name)) {
                let dir = out_root.join(stem).join(size);
                let png = format!("{name}.png");
                let _ = picture_difference(&first.shots_dir.join(&png), &other.shots_dir.join(&png), &dir, name);
            } else {
                r.identical += 1;
            }
        }
        r.hashes.insert(size.to_owned(), first.frames.clone());
    }
    r
}

fn run_project(name: &str, args: &Args, scratch: &Path, build_root: &Path, golden: Option<&Golden>) -> ProjectResult {
    let mut res =
        ProjectResult { name: name.to_owned(), forms: 0, scripted: 0, build: Duration::ZERO, runs: Duration::ZERO, results: Vec::new(), failed: None };
    let copy = scratch.join("work").join(name);
    let outcome = (|| -> Result<(), String> {
        copy_committed(name, &copy)?;
        let (_, forms) = ten_times::regenerate(&copy)?;
        res.forms = forms.len();
        let mut loaded = Vec::new();
        for rel in &forms {
            let text = std::fs::read_to_string(copy.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
            let form = cobolt_forms::xml::load_form_from_str(&text).map_err(|e| format!("{rel}: {e}"))?;
            let id = Path::new(rel).file_stem().map(|s| s.to_string_lossy().to_ascii_uppercase()).unwrap_or_default();
            loaded.push((form_stem(rel), id, form));
        }
        if args.write_scripts {
            let dir = scripts_dir(name);
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let manifest = ten_times::manifest_of(&copy)?;
            let entrance = opens_with_effect(&std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?);
            for (stem, _, form) in &loaded {
                let s = base_script(form, entrance);
                let file = json!({"form": s.form, "comment": s.comment, "steps": s.steps});
                let text = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())? + "\n";
                std::fs::write(dir.join(format!("{stem}.json")), text).map_err(|e| e.to_string())?;
            }
        }
        if !args.cargo {
            return Ok(());
        }
        // The data every run starts from.
        let pristine = scratch.join("pristine").join(name);
        let data = copy.join("data");
        let pristine_data = if data.is_dir() {
            ten_times::copy_project(&data, &pristine).map_err(|e| format!("{}: {e}", pristine.display()))?;
            Some(pristine.as_path())
        } else {
            None
        };
        let manifest = ten_times::manifest_of(&copy)?;
        cut_off_aws(&manifest, scratch)?;
        eprintln!("pixel-parity: building {name} on the Cargo path…");
        let t = Instant::now();
        let opts = cobolt_compiler::BuildOptions {
            verbose: false,
            workspace_root: Some(repo_root()),
            build_dir: Some(build_root.to_path_buf()),
            ..Default::default()
        };
        let built = cobolt_compiler::build_project(&manifest, &opts).map_err(|e| format!("the Build failed: {e}"))?;
        res.build = t.elapsed();
        let t = Instant::now();
        let out_root = repo_root().join("target/pixel-parity").join(name);
        for (stem, id, form) in &loaded {
            let file = scripts_dir(name).join(format!("{stem}.json"));
            let Some(steps) = std::fs::read_to_string(&file)
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v.get("steps").and_then(Value::as_array).cloned())
            else {
                res.results.push(FormResult { stem: stem.clone(), problems: vec!["no script".into()], ..Default::default() });
                continue;
            };
            res.scripted += 1;
            let runs = runs_per_size(args.record);
            eprintln!("pixel-parity: {name}/{stem}, {} steps at {} sizes, {runs} runs each…", steps.len(), SIZES.len());
            let launch = Launch { binary: &built.binary_path, project: &copy, pristine_data, form_id: id };
            let work = scratch.join("runs").join(name).join(stem);
            let mut r = run_form(&launch, stem, &steps, form.height as f32, runs, &work, &out_root);
            let _ = std::fs::remove_dir_all(&work);
            if let Some(want) = golden.and_then(|g| g.frames.get(name)).and_then(|p| p.get(stem.as_str())) {
                for (size, frames) in want {
                    for (shot, hash) in frames {
                        match r.hashes.get(size).and_then(|f| f.get(shot)) {
                            Some(h) if h == hash => r.golden_match += 1,
                            Some(_) => r.golden_differ.push(format!("{size}/{shot}")),
                            None => r.golden_differ.push(format!("{size}/{shot} (not drawn)")),
                        }
                    }
                }
            }
            res.results.push(r);
        }
        res.runs = t.elapsed();
        Ok(())
    })();
    if let Err(e) = outcome {
        res.failed = Some(e);
    }
    let _ = std::fs::remove_dir_all(&copy);
    res
}

/// The `pixel-parity` mode.
pub fn run(args: &[String]) -> Result<(), String> {
    let args = parse_args(args)?;
    if args.console {
        return crate::console_parity::run();
    }
    let started = Instant::now();
    let commit = String::from_utf8_lossy(&git(&["rev-parse", "--short", "HEAD"])?).trim().to_owned();
    let scratch = std::env::temp_dir().join(format!("prc-pixel-parity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    let _ = std::fs::remove_dir_all(repo_root().join("target/pixel-parity"));
    let build_root = scratch.join("build");
    let golden: Option<Golden> =
        std::fs::read_to_string(golden_path()).ok().and_then(|t| toml::from_str(&t).ok());

    let projects: Vec<&str> =
        ten_times::REFERENCE_PROJECTS.iter().copied().filter(|p| args.project.as_deref().is_none_or(|w| w == *p)).collect();
    let results: Vec<ProjectResult> =
        projects.iter().map(|p| run_project(p, &args, &scratch, &build_root, golden.as_ref().filter(|_| !args.record))).collect();
    let _ = std::fs::remove_dir_all(&scratch);

    if args.write_scripts {
        let written: usize = results.iter().map(|r| r.forms).sum();
        println!("pixel-parity: base scripts written for {written} forms under {}", repo_root().join("tests/pixel-parity/scripts").display());
    }
    for r in results.iter().filter(|r| r.failed.is_some()) {
        println!("{}: {}", r.name, r.failed.as_deref().unwrap_or_default());
    }
    if !args.cargo {
        return if results.iter().any(|r| r.failed.is_some()) { Err("pixel-parity: a project failed".into()) } else { Ok(()) };
    }

    // The result block (GOLDEN RULE #7).
    println!("== cobolt-bench pixel-parity --cargo (DESKTOP-WASM-001 T7: R27 harness, AC19 second guard, AC20) ==");
    println!(
        "platform {}, version {}, commit {commit} (the committed projects); clock {CLOCK}, seed {SEED}; sizes: designed, 800 wide, 480 wide; {} runs each",
        platform(),
        product_version(),
        runs_per_size(args.record)
    );
    println!(
        "{:<16} {:>5} {:>8} {:>6} {:>6} {:>9} {:>14} {:>9} {:>8}  golden",
        "project", "forms", "scripted", "steps", "frames", "identical", "not repeatable", "build s", "runs s"
    );
    let (mut frames, mut identical, mut differ_golden, mut matched_golden) = (0, 0, 0, 0);
    let mut not_repeatable = Vec::new();
    for r in &results {
        let f: usize = r.results.iter().map(|x| x.frames).sum();
        let i: usize = r.results.iter().map(|x| x.identical).sum();
        let steps: usize = r.results.iter().map(|x| x.steps).sum();
        let nr = r.results.iter().filter(|x| !x.repeatable()).count();
        let gm: usize = r.results.iter().map(|x| x.golden_match).sum();
        let gd: usize = r.results.iter().map(|x| x.golden_differ.len()).sum();
        frames += f;
        identical += i;
        matched_golden += gm;
        differ_golden += gd;
        let golden_text = if args.record {
            "recording".to_owned()
        } else if gm + gd == 0 {
            "none recorded".to_owned()
        } else {
            format!("{gm} of {} match", gm + gd)
        };
        println!(
            "{:<16} {:>5} {:>8} {:>6} {:>6} {:>9} {:>14} {:>9.1} {:>8.1}  {golden_text}",
            r.name,
            r.forms,
            r.scripted,
            steps,
            f,
            i,
            nr,
            r.build.as_secs_f64(),
            r.runs.as_secs_f64()
        );
        for x in &r.results {
            if !x.repeatable() {
                let why = if x.problems.is_empty() {
                    format!("{} of {} frames differ between runs", x.frames - x.identical, x.frames)
                } else {
                    x.problems.join("; ")
                };
                not_repeatable.push((format!("{}/{}", r.name, x.stem), why));
            }
            for d in &x.golden_differ {
                println!("  {}/{}: differs from its golden at {d}", r.name, x.stem);
            }
        }
    }
    if !not_repeatable.is_empty() {
        println!("not repeatable (AC20):");
        for (form, why) in &not_repeatable {
            println!("  {form}: {why}");
        }
    }
    let unrepeatable = not_repeatable.len();
    println!(
        "frames compared {frames}, identical in every run {identical}; forms repeatable {} of {}; golden {matched_golden} matched, {differ_golden} differ; total {:.1} min",
        results.iter().flat_map(|r| &r.results).filter(|x| x.repeatable()).count(),
        results.iter().map(|r| r.results.len()).sum::<usize>(),
        started.elapsed().as_secs_f64() / 60.0
    );
    if frames > identical || differ_golden > 0 {
        println!("pictures of the differences: {}", repo_root().join("target/pixel-parity").display());
    }

    if args.record {
        let path = golden_path();
        let mut g: Golden = std::fs::read_to_string(&path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        g.version = product_version();
        g.commit = commit;
        g.clock = CLOCK.into();
        g.seed = SEED;
        for r in results.iter().filter(|r| r.failed.is_none()) {
            g.frames.remove(&r.name);
            g.not_repeatable.retain(|k, _| !k.starts_with(&format!("{}/", r.name)));
            for x in &r.results {
                if x.repeatable() {
                    g.frames.entry(r.name.clone()).or_default().insert(x.stem.clone(), x.hashes.clone());
                }
            }
        }
        for (form, why) in not_repeatable {
            g.not_repeatable.insert(form, why);
        }
        let text = toml::to_string(&g).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&path, format!("{GOLDEN_HEADER}{text}")).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("recorded: {}", path.display());
    }
    if results.iter().any(|r| r.failed.is_some()) {
        return Err("pixel-parity: a project failed (above)".into());
    }
    // T8: the host keeps the run's virtual time, so a form that does not repeat breaks AC20. A
    // recording only leaves it out; a check fails.
    if unrepeatable > 0 && !args.record {
        return Err(format!("pixel-parity: {unrepeatable} form(s) draw different frames in two runs (AC20)"));
    }
    if differ_golden > 0 {
        return Err(format!("pixel-parity: {differ_golden} frame(s) differ from their golden"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One handler of each kind the base script tells apart, in this order in the file.
    const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="T-FORM" title="T" width="400" height="300" main-form="true">
  <Control id="BTN-PLAIN" type="Button" x="0" y="0" w="80" h="24" tab-order="0" z-order="0" visible="true" enabled="true">
    <Event name="onClick" paragraph="BTN-PLAIN--ONCLICK"><![CDATA[           MOVE "x" TO BTN-PLAIN::Caption]]></Event>
  </Control>
  <Control id="BTN-QUIT" type="Button" x="0" y="30" w="80" h="24" tab-order="1" z-order="1" visible="true" enabled="true">
    <Event name="onClick" paragraph="BTN-QUIT--ONCLICK"><![CDATA[           CALL "LEAVE"]]></Event>
  </Control>
  <Control id="BTN-PLAY" type="Button" x="0" y="60" w="80" h="24" tab-order="2" z-order="2" visible="true" enabled="true">
    <Event name="onClick" paragraph="BTN-PLAY--ONCLICK"><![CDATA[           me::OpenFormSync( "WORLD-FORM" )
           me::Close()]]></Event>
  </Control>
  <Control id="BTN-OPEN" type="Button" x="0" y="90" w="80" h="24" tab-order="3" z-order="3" visible="true" enabled="true">
    <Event name="onClick" paragraph="BTN-OPEN--ONCLICK"><![CDATA[           COBOL::"OPEN-FILE-DIALOG" ( "Pick" WS-PATH )]]></Event>
  </Control>
  <Control id="AI-1" type="AgentObject" x="0" y="120" w="24" h="24" tab-order="4" z-order="4" visible="true" enabled="true">
  </Control>
  <Control id="BTN-ASK" type="Button" x="0" y="150" w="80" h="24" tab-order="5" z-order="5" visible="true" enabled="true">
    <Event name="onClick" paragraph="BTN-ASK--ONCLICK"><![CDATA[           SET WS-ANSWER TO AI-1::Ask( "Hello" )]]></Event>
  </Control>
  <Control id="BTN-EMPTY" type="Button" x="0" y="180" w="80" h="24" tab-order="6" z-order="6" visible="true" enabled="true">
    <Event name="onClick" paragraph="BTN-EMPTY--ONCLICK"><![CDATA[   ]]></Event>
  </Control>
  <form-events>
    <Event name="onLoad" paragraph="T-FORM--ONLOAD"><![CDATA[           MOVE 1 TO WS-N]]></Event>
    <Event name="onShow" paragraph="T-FORM--ONSHOW"><![CDATA[           MOVE 2 TO WS-N]]></Event>
  </form-events>
  <user-procedures>
    <Event name="LEAVE" paragraph="LEAVE"><![CDATA[           STOP RUN]]></Event>
  </user-procedures>
</Form>
"#;

    /// The base script's rules, the stem of a form's file, the names a handler calls, and the
    /// hash of a frame.
    #[test]
    fn base_script_orders_and_names_the_handlers() {
        let form = cobolt_forms::load_form_from_str(FORM).expect("the form loads");
        let script = base_script(&form, false);
        let with_effect = base_script(&form, true);
        let expected = vec![
            json!({"shot": "00-open"}),
            json!({"event": {"control": "BTN-PLAIN", "name": "onClick"}}),
            json!({"shot": "01-BTN-PLAIN-onClick"}),
            json!({"answer": {"cancel": true}}),
            json!({"event": {"control": "BTN-OPEN", "name": "onClick"}}),
            json!({"shot": "02-BTN-OPEN-onClick"}),
            json!({"event": {"control": "T-FORM", "name": "onShow"}}),
            json!({"shot": "03-T-FORM-onShow"}),
            json!({"event": {"control": "BTN-PLAY", "name": "onClick"}, "wait": false}),
            json!({"wait_ms": MODAL_WAIT_MS}),
            json!({"shot": "04-BTN-PLAY-onClick"}),
            json!({"event": {"control": "BTN-QUIT", "name": "onClick"}, "wait": false}),
            json!({"wait_ms": MODAL_WAIT_MS}),
            json!({"shot": "05-BTN-QUIT-onClick"}),
        ];

        let dir = std::env::temp_dir().join(format!("prc-pixel-parity-unit-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let png = dir.join("frame.png");
        let pixels: Vec<u8> = vec![255, 0, 0, 255, 0, 0, 255, 128];
        image::RgbaImage::from_raw(2, 1, pixels.clone()).expect("2 x 1").save(&png).expect("the PNG is written");
        let mut want = Sha256::new();
        want.update(2u32.to_le_bytes());
        want.update(1u32.to_le_bytes());
        want.update(&pixels);
        let want = format!("{:x}", want.finalize());
        let got = frame_hash(&png);
        let _ = std::fs::remove_dir_all(&dir);

        let checks = [
            ("the steps: plain in file order, then modal, then ending; the empty and the network handlers skipped", script.steps == expected),
            ("the comment names the modal handler", script.comment.contains("modally: BTN-PLAY.onClick.")),
            ("the comment names the ending handler, reached through CALL \"LEAVE\"", script.comment.contains("end the form (without waiting too when a modal form is open): BTN-QUIT.onClick.")),
            ("the comment names the dialog handler", script.comment.contains("file dialog: BTN-OPEN.onClick.")),
            ("the comment names the handler that reaches the network", script.comment.contains("network: BTN-ASK.onClick.")),
            ("the form's name", script.form == "T-FORM"),
            ("with an entrance effect, a wait for it and a picture of the form ready follow the first picture", {
                let mut want = expected.clone();
                want.splice(1..1, [json!({"wait_ms": ENTRANCE_WAIT_MS}), json!({"shot": "00-ready"})]);
                with_effect.steps == want
            }),
            ("the manifest's entrance effect is read as the product reads it", opens_with_effect("[forms]\nentrance-effect = \"matrix-rain\"\n") && !opens_with_effect("[forms]\nentrance-effect = \"\"\n") && !opens_with_effect("name = \"x\"\n") && !opens_with_effect("[forms]\nentrance-effect = \"no-such\"\n")),
            ("a stem: forms/sub/a-form.cfrm is sub__a-form", form_stem("forms/sub/a-form.cfrm") == "sub__a-form"),
            ("CALL \"A\" and CALL  \"B\" are found, CALL WS-NAME is not", called("CALL \"A\" CALL WS-NAME CALL  \"B\"") == ["A", "B"]),
            ("a frame's hash is the sha256 of its size and RGBA bytes", got.as_deref() == Ok(want.as_str())),
        ];

        // The result block (GOLDEN RULE #7).
        let passed = checks.iter().filter(|(_, ok)| *ok).count();
        println!("== pixel_parity::base_script_orders_and_names_the_handlers (DESKTOP-WASM-001 T7, T8) ==");
        println!("fixture: 7 controls, 8 handlers (onLoad is never raised), 1 procedure; {} steps written, {} expected", script.steps.len(), expected.len());
        for (what, ok) in &checks {
            println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
        }
        println!("checks: {passed} of {} passed", checks.len());
        assert_eq!(passed, checks.len(), "a check failed: see the result block\n{:#?}\n{}", script.steps, script.comment);
    }
}
