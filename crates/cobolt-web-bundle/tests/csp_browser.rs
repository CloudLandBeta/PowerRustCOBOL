// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The browser enforces the Content-Security-Policy** (WASM-013 T12; R2, AC2).
//!
//! A real Chrome loads the shipped bundle with its policy delivered as a response **header**, plus
//! `report-uri /__csp-report`, which the development server records (`csp_report_sink`; the route is
//! never part of a bundle):
//!
//! * the clean bundle renders its main form and the browser reports **no** violation;
//! * a tampered copy, written to a temporary folder by this test and never into the repository, adds an
//!   inline `<script>`, a same-origin script that calls `eval`, and a `fetch` outside `connect-src`; the
//!   browser blocks all three, runs none of them, and reports `script-src-elem`, the eval and `connect-src`.
//!   Under the strict policy the eval is stopped by Trusted Types (`require-trusted-types-for`) before
//!   `script-src` is asked; with Trusted Types off the same eval is reported as `script-src`. Both runs are made.
//!
//! With no Chrome the test prints `SKIPPED: no Chrome` and fails when `PR_SECURITY_GATE=1`.

mod support;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{BundleOptions, PlayerInput};
use support::cdp::{find_chrome, Chrome};

const HELPERS: &str = r#"
window.__pr = {
  app() { return document.getElementById('pr-app'); },
  handle() { return PowerRustCOBOL.mount(this.app()); },
};
true
"#;

fn shipped_player() -> PlayerInput {
    let dir: PathBuf = support::repo_root().join("assets/web-player");
    PlayerInput {
        plain: Some(support::player_variant(&dir, "plain")),
        threaded: Some(support::player_variant(&dir, "threaded")),
        parts: support::player_parts(&dir),
        ..Default::default()
    }
}

/// Serve `dir` with the bundle's policy as a header and the report sink on; load it; give the browser
/// time to report; answer the reports.
fn load(chrome: &Chrome, dir: &std::path::Path, header: (String, String), ready: &str, want_reports: usize) -> Result<(bool, Vec<String>, String), String> {
    let server = serve(dir, "127.0.0.1:0", ServeOptions { extra_headers: vec![header], csp_report_sink: true, ..Default::default() }).map_err(|e| e.to_string())?;
    let mut p = chrome.new_page()?;
    p.resize(1024, 768)?;
    p.navigate(&format!("{}/index.html", server.url()))?;
    p.eval(HELPERS)?;
    let rendered = p.wait_for(ready, 40).is_ok();
    // reports are sent asynchronously: wait for the expected number, or a quiet 3 s for none
    let t0 = Instant::now();
    let limit = Duration::from_secs(if want_reports == 0 { 3 } else { 15 });
    while t0.elapsed() < limit && server.csp_reports().len() < want_reports {
        std::thread::sleep(Duration::from_millis(100));
    }
    let reports = server.csp_reports();
    let ran = p.eval("JSON.stringify({ inline: document.documentElement.dataset.inlineRan || null, eval: document.documentElement.dataset.evalRan || null })")?;
    server.stop();
    Ok((rendered, reports, ran.as_str().unwrap_or("?").to_owned()))
}

fn directives(reports: &[String]) -> Vec<String> {
    reports
        .iter()
        .map(|r| {
            let key = "\"effective-directive\":\"";
            r.find(key).map(|i| r[i + key.len()..].split('"').next().unwrap_or("?").to_owned()).unwrap_or_else(|| "?".into())
        })
        .collect()
}

fn bundle(trusted_types: bool) -> (cobolt_web_bundle::BundleOutput, (String, String)) {
    let mut options = BundleOptions::default();
    options.csp.report_uri = Some("/__csp-report".into());
    options.csp.trusted_types = trusted_types;
    let out = support::assemble(shipped_player(), options);
    let policy = out.report.csp_header.clone();
    assert!(policy.contains("report-uri /__csp-report"), "{policy}");
    assert_eq!(policy.contains("require-trusted-types-for"), trusted_types, "{policy}");
    (out, ("Content-Security-Policy".to_string(), policy))
}

/// A tampered copy in a temporary folder: an inline script, a same-origin script that evals, a fetch out.
fn tamper(out: &cobolt_web_bundle::BundleOutput) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(out, dir.path());
    std::fs::write(
        dir.path().join("tamper.js"),
        "try { eval('document.documentElement.dataset.evalRan = 1'); } catch (e) {}\nfetch('http://127.0.0.1:9/outside').catch(() => {});\n",
    )
    .unwrap();
    let index = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
    let at = index.find("</body>").expect("index.html has a body");
    let injected = format!("{}<script>document.documentElement.dataset.inlineRan = 1;</script><script src=\"tamper.js\"></script>{}", &index[..at], &index[at..]);
    std::fs::write(dir.path().join("index.html"), injected).unwrap();
    dir
}

#[test]
fn the_browser_enforces_the_policy_and_reports_what_it_blocked() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome");
        assert!(std::env::var_os("PR_SECURITY_GATE").is_none(), "PR_SECURITY_GATE=1 but no Chrome was found (set CHROME)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let ready = "__pr.handle().then(a => Object.keys(JSON.parse(a.player.snapshot())).length > 0)";
    println!("CSP in a real Chrome ({})", exe.display());

    let (out, header) = bundle(true);
    let clean = tempfile::tempdir().unwrap();
    support::write_bundle(&out, clean.path());
    let t0 = Instant::now();
    let (rendered, reports, _) = load(&chrome, clean.path(), header.clone(), ready, 0).unwrap_or_else(|e| panic!("{e}"));
    println!("  clean bundle:           main form rendered = {rendered}, violation reports = {} ({} ms)", reports.len(), t0.elapsed().as_millis());
    assert!(rendered, "the clean bundle's main form did not render under its own policy");
    assert!(reports.is_empty(), "the clean bundle must report nothing, got: {reports:?}");

    for (label, trusted_types, eval_directive) in [("strict policy", true, "require-trusted-types-for"), ("Trusted Types off", false, "script-src")] {
        let (out, header) = bundle(trusted_types);
        let dir = tamper(&out);
        let t = Instant::now();
        let (_, bad, ran) = load(&chrome, dir.path(), header, ready, 3).unwrap_or_else(|e| panic!("{e}"));
        let kinds = directives(&bad);
        println!("  tampered, {label}: {} reports in {} ms: {}; ran = {ran}", bad.len(), t.elapsed().as_millis(), kinds.join(", "));
        for r in &bad {
            println!("    {}", r.chars().take(200).collect::<String>());
        }
        for want in ["script-src-elem", eval_directive, "connect-src"] {
            assert!(kinds.iter().any(|k| k == want), "{label}: no `{want}` report; the browser reported {kinds:?}");
        }
        assert_eq!(ran, r#"{"inline":null,"eval":null}"#, "{label}: the browser ran something the policy forbids");
    }
}
