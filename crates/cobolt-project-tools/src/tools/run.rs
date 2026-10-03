// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`run_form`** (spec 084 R31): run a form off screen with a script — set
//! properties and raise events as the operator would, wait, read properties
//! back — and answer with what the program DISPLAYed, the values read, any
//! runtime error and a picture of the final state.
//!
//! The run happens in a separate `rcrun run-form --headless` process: the
//! interpreter is a free-running thread and the host keeps process-wide state,
//! so a process is what a hard time limit can stop, and what contains a
//! panic. The host injects the [`FormRunner`]; [`rcrun_runner`] is the one the
//! IDE and `rcrun` both use.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::host::ProjectHost;
use crate::root::ProjectRoot;

/// The marker of the result line `rcrun run-form --headless` prints.
pub const RESULT_MARKER: &str = "@RUN-FORM-RESULT ";
/// Most DISPLAY lines an answer carries.
const MAX_DISPLAY_LINES: usize = 500;

/// What one run produced.
#[derive(Debug, Clone, Default)]
pub struct RunOutcome {
    pub report: Value,
    pub display: Vec<String>,
    pub png: Option<Vec<u8>>,
}

/// Run `cfrm` with its program `cbl` through `steps` for at most `limit_s`.
pub type FormRunner = Arc<dyn Fn(&Path, &Path, &[Value], u64) -> Result<RunOutcome, String> + Send + Sync>;

/// Run forms with the `rcrun` at `rcrun`, one process per run.
pub fn rcrun_runner(rcrun: PathBuf) -> FormRunner {
    Arc::new(move |cfrm: &Path, cbl: &Path, steps: &[Value], limit_s: u64| {
        let work = std::env::temp_dir().join(format!(
            "prc-run-form-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
        ));
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        let script = work.join("script.json");
        let png = work.join("final.png");
        std::fs::write(&script, serde_json::to_vec(steps).unwrap_or_default()).map_err(|e| e.to_string())?;
        let result = run_process(&rcrun, cfrm, cbl, &script, &png, limit_s);
        let picture = std::fs::read(&png).ok();
        let _ = std::fs::remove_dir_all(&work);
        result.map(|(report, display)| RunOutcome { report, display, png: picture })
    })
}

fn run_process(rcrun: &Path, cfrm: &Path, cbl: &Path, script: &Path, png: &Path, limit_s: u64) -> Result<(Value, Vec<String>), String> {
    let mut child = Command::new(rcrun)
        .arg("run-form")
        .arg(cfrm)
        .arg(cbl)
        .arg("--designer")
        .arg("--headless")
        .arg(script)
        .arg("--headless-png")
        .arg(png)
        .arg("--headless-limit")
        .arg(limit_s.to_string())
        .env("PRC_NO_WINDOW_FX", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("rcrun could not be started: {e}"))?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let reader = std::thread::spawn(move || BufReader::new(stdout).lines().map_while(Result::ok).collect::<Vec<_>>());
    let deadline = Instant::now() + Duration::from_secs(limit_s + 20);
    let killed = loop {
        match child.try_wait() {
            Ok(Some(_)) => break false,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break true;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let mut stderr = String::new();
    if let Some(mut e) = child.stderr.take() {
        use std::io::Read;
        let _ = e.read_to_string(&mut stderr);
    }
    let lines = reader.join().unwrap_or_default();
    let mut report = None;
    let mut display = Vec::new();
    for line in lines {
        match line.strip_prefix(RESULT_MARKER) {
            Some(json) => report = serde_json::from_str::<Value>(json).ok(),
            None if display.len() < MAX_DISPLAY_LINES => display.push(line),
            None => {}
        }
    }
    match report {
        Some(r) => Ok((r, display)),
        None if killed => Err(format!("the run did not finish within {} s and was stopped", limit_s + 20)),
        None => {
            let why = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("rcrun ended without a result");
            Err(format!("the form could not be run: {why}"))
        }
    }
}

/// Run `run_form`: regenerate the form's COBOL, run it, and answer.
pub fn run(host: &mut impl ProjectHost, root: &ProjectRoot, runner: Option<&FormRunner>, rel: &str, steps: &[Value], limit_s: u64) -> Result<(Option<Vec<u8>>, Value), String> {
    let runner = runner.ok_or("run_form is not available in this server")?;
    if !rel.to_ascii_lowercase().ends_with(".cfrm") {
        return Err("run_form takes a form (.cfrm)".to_owned());
    }
    let cfrm = root.resolve(rel)?;
    if !cfrm.is_file() {
        return Err(format!("no form at {rel}"));
    }
    // What Run Form does first: the form's program, generated fresh.
    let regenerated = super::regenerate::run(host, root, Some(rel))?;
    let cbl = regenerated["written"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find(|p| p.to_ascii_lowercase().ends_with(".cbl"))
        .map(|p| root.dir().join(p))
        .ok_or("the form's program was not generated")?;
    let outcome = runner(&cfrm, &cbl, steps, limit_s)?;
    let mut answer = outcome.report;
    answer["form"] = json!(rel);
    answer["display"] = json!(outcome.display);
    Ok((outcome.png, answer))
}
