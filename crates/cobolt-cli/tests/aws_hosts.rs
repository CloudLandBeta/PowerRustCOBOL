// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 078 T-A16 (R28; AC6, AC11, AC16): the same COBOL programs on the
//! three hosts. `tests/cobol/aws/forms` drive every Delivery A operation of
//! `AwsLambda` and `AwsMcp` against the fake MCP server, through a project
//! whose AWS connection routes both servers to it, and report a tally:
//!
//! - under `rcrun run-form` (Run Form),
//! - as embedded child forms, loaded into an application shell's ContentPane,
//! - in a built binary, run headless (`--ignored`: it compiles one), which
//!   also checks that no AWS server outlives the application (AC6).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

struct Project {
    root: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

/// The fake MCP server is `cobolt-runtime`'s test binary: built on demand,
/// beside `rcrun` in the same target folder.
fn fake_mcp() -> PathBuf {
    let path = rcrun().with_file_name(format!("fake_mcp{}", std::env::consts::EXE_SUFFIX));
    if !path.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let status = Command::new(cargo)
            .args(["build", "-p", "cobolt-runtime", "--bin", "fake_mcp"])
            .status()
            .expect("cargo builds the fake MCP server");
        assert!(status.success(), "the fake MCP server did not build");
    }
    assert!(path.exists(), "no fake MCP server at {}", path.display());
    path
}

/// A project holding the three forms, their generated programs, and one AWS
/// connection whose `lambda` and `fake` servers are the fake.
fn project(tag: &str) -> Project {
    let root = std::env::temp_dir().join(format!("prc-078-hosts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["forms", "generated", "src", "data"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/aws/forms");
    let mut forms = Vec::new();
    for entry in std::fs::read_dir(&fixtures).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        std::fs::copy(entry.path(), root.join("forms").join(&name)).unwrap();
        if let Some(stem) = name.strip_suffix(".cfrm") {
            let form = cobolt_forms::load_form(&entry.path()).unwrap();
            std::fs::write(root.join("generated").join(format!("{stem}.cbl")), cobolt_codegen::generate(&form)).unwrap();
            forms.push(stem.to_owned());
        }
    }
    let reply = r#"Function app-f returned: [{"id":1,"name":"Ana"},{"id":2,"name":"Bo"}]"#;
    let script = json!({
        "tools": [
            {"name": "app_f", "inputSchema": {"type": "object", "properties": {"parameters": {"type": "object"}}}},
            {"name": "read_thing", "inputSchema": {"type": "object"}, "annotations": {"readOnlyHint": true}},
            {"name": "write_thing", "inputSchema": {"type": "object"}}
        ],
        "answers": {"app_f": {"content": [{"type": "text", "text": reply}], "isError": false}},
        "pid_file": root.join("pids.txt").to_string_lossy(),
    });
    std::fs::write(root.join("fake.json"), script.to_string()).unwrap();
    let fake = fake_mcp();
    let server = |id: &str| {
        format!(
            "[servers.{id}]\ncommand = '{}'\nargs = ['{}']\n",
            fake.display(),
            root.join("fake.json").display()
        )
    };
    let routes = format!("{}{}", server("lambda"), server("fake"));
    let listed = |dir: &str, ext: &str| forms.iter().map(|f| format!("\"{dir}/{f}.{ext}\"")).collect::<Vec<_>>().join(", ");
    let manifest = format!(
        "[project]\nname = \"AwsHosts\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\ndebug_compilation = true\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [{}]\ngenerated = [{}]\n\n\
         [[integrations.aws_connections]]\nid = \"aws-hosts\"\nname = \"Demo\"\nprofile = \"demo\"\nregion = \"eu-west-1\"\n\
         routes_override = '''\n{routes}'''\n",
        listed("forms", "cfrm"),
        listed("generated", "cbl"),
    );
    std::fs::write(root.join("awshosts.project.toml"), manifest).unwrap();
    std::fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    Project { root }
}

/// The steps that run one demo; `open` loads it into the shell's pane first.
fn steps(open: Option<&str>) -> Value {
    let mut s = Vec::new();
    if let Some(form) = open {
        s.push(json!({"open_form": form}));
    }
    s.push(json!({"event": {"control": "BTN-RUN", "name": "onClick"}}));
    s.push(json!({"wait_ms": 2500}));
    s.push(json!({"event": {"control": "BTN-SUMMARY", "name": "onClick"}}));
    s.push(json!({"read": {"control": "LBL-RESULT", "property": "Caption"}}));
    Value::Array(s)
}

/// The demo's tally, and its summary block, from a headless run's stdout.
fn outcome(stdout: &str) -> (String, Vec<String>) {
    let report: Value = stdout
        .lines()
        .find_map(|l| l.strip_prefix("@RUN-FORM-RESULT "))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"));
    let tally = report["steps"]
        .as_array()
        .and_then(|s| s.iter().find_map(|st| st["value"].as_str().map(str::to_owned)))
        .unwrap_or_default();
    let block = stdout
        .lines()
        .skip_while(|l| !l.starts_with("===="))
        .take_while(|l| !l.starts_with("@RUN-FORM-RESULT"))
        .map(|l| l.trim_end().to_owned())
        .collect();
    (tally.trim().to_owned(), block)
}

fn run_form(p: &Project, form: &str, program: &str, script: &Value) -> String {
    let script_path = p.root.join(format!("steps-{}.json", form.replace('/', "-")));
    std::fs::write(&script_path, script.to_string()).unwrap();
    let out = Command::new(rcrun())
        .args([
            "run-form",
            form,
            program,
            "--designer",
            "--headless",
            script_path.to_str().unwrap(),
            "--headless-limit",
            "60",
        ])
        .current_dir(&p.root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form {form} failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    stdout
}

const DEMOS: [(&str, &str); 2] = [("aws-lambda-demo", "PASS 009 FAIL 000"), ("aws-mcp-demo", "PASS 009 FAIL 000")];

#[test]
fn the_demos_pass_under_run_form_and_as_embedded_child_forms() {
    let p = project("rf");
    let mut table = Vec::new();
    for (demo, want) in DEMOS {
        let direct = outcome(&run_form(&p, &format!("forms/{demo}.cfrm"), &format!("generated/{demo}.cbl"), &steps(None)));
        let embedded = outcome(&run_form(&p, "forms/aws-shell.cfrm", "generated/aws-shell.cbl", &steps(Some(demo))));
        println!("\n  {demo} under Run Form:\n    {}", direct.1.join("\n    "));
        assert_eq!(direct.0, want, "{demo} under Run Form");
        assert_eq!(embedded.0, want, "{demo} as an embedded child form");
        table.push(format!("{demo}: Run Form {} · embedded {}", direct.0, embedded.0));
    }
    println!("\n078 T-A16 — {}", table.join("; "));
}

/// AC6/AC16 — the built binary gives the same tallies, and leaves no AWS
/// server running behind it.
#[test]
#[ignore = "builds a real binary; run with --ignored"]
fn the_demos_pass_in_a_built_binary_and_leave_no_server_behind() {
    let p = project("bin");
    let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let opts = cobolt_compiler::BuildOptions { verbose: false, workspace_root: Some(ws), ..Default::default() };
    let t = std::time::Instant::now();
    let built = cobolt_compiler::build_project(&p.root.join("awshosts.project.toml"), &opts)
        .unwrap_or_else(|e| panic!("the build failed: {e}"));
    let build_s = t.elapsed().as_secs_f32();
    let mut table = Vec::new();
    for (demo, want) in DEMOS {
        let script = p.root.join(format!("bin-{demo}.json"));
        std::fs::write(&script, steps(Some(demo)).to_string()).unwrap();
        let out = Command::new(&built.binary_path)
            .env("PRC_HEADLESS_SCRIPT", &script)
            .env("PRC_HEADLESS_LIMIT", "60")
            .current_dir(&p.root)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success(), "{demo}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr));
        let (tally, block) = outcome(&stdout);
        println!("\n  {demo} in the built binary:\n    {}", block.join("\n    "));
        assert_eq!(tally, want, "{demo} in the built binary");
        table.push(format!("{demo}: {tally}"));
    }
    // AC6: every server the binary started has ended with it.
    std::thread::sleep(std::time::Duration::from_millis(500));
    let pids = std::fs::read_to_string(p.root.join("pids.txt")).unwrap_or_default();
    let alive: Vec<&str> = pids
        .lines()
        .filter(|pid| Command::new("kill").args(["-0", pid]).output().is_ok_and(|o| o.status.success()))
        .collect();
    println!("\n078 T-A16 built binary (build {build_s:.0} s) — {}; servers started {}, still running {}", table.join("; "), pids.lines().count(), alive.len());
    assert!(alive.is_empty(), "AWS servers outlived the binary: {alive:?}");
}

/// Spec 078 AC19 — the Guide's "Calling AWS" handlers compile as the designer
/// would compile them: each `PROGRAM-ID. CONTROL--EVENT.` example becomes that
/// control's event handler on a form, and the generated program must parse
/// and pass the semantic analyser.
#[test]
fn the_guides_aws_examples_compile() {
    let guide = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/developers-guide-en.md")).unwrap();
    let start = guide.find("### Calling AWS").expect("the Calling AWS section");
    let end = start + guide[start..].find("\n## ").expect("the section ends");
    let section = &guide[start..end];
    let mut form = cobolt_forms::Form::new("AWS-GUIDE", "AWS guide", 640, 480);
    for (id, ct) in [
        ("LAMBDA-1", cobolt_forms::ControlType::AwsLambda),
        ("MCP-1", cobolt_forms::ControlType::AwsMcp),
        ("LBL-QUOTE", cobolt_forms::ControlType::Label),
        ("TXT-OUT", cobolt_forms::ControlType::TextBox),
        ("BTN-QUOTE", cobolt_forms::ControlType::Button),
        ("BTN-TOOLS", cobolt_forms::ControlType::Button),
    ] {
        form.controls.push(cobolt_forms::Control::new(id, ct, 0, 0));
    }
    let mut handlers = Vec::new();
    for block in section.split("```cobol").skip(1).map(|b| &b[..b.find("```").unwrap()]) {
        for part in block.split("PROGRAM-ID.").skip(1) {
            let (name, body) = part.split_once('\n').unwrap();
            let name = name.trim().trim_end_matches('.');
            let (ctrl, ev) = name.split_once("--").unwrap();
            let event = match ev {
                "ONCLICK" => "onClick",
                "ONINVOKED" => "onInvoked",
                "ONTOOLRESULT" => "onToolResult",
                other => panic!("an example handler for an unexpected event: {other}"),
            };
            let c = form.controls.iter_mut().find(|c| c.id == ctrl).unwrap_or_else(|| panic!("no control {ctrl}"));
            c.events.push(cobolt_forms::EventBinding {
                event: event.to_owned(),
                paragraph: name.to_owned(),
                code: format!("       ENVIRONMENT DIVISION.\n{}", body.trim_end()),
            });
            handlers.push(name.to_owned());
        }
    }
    assert_eq!(handlers.len(), 4, "{handlers:?}");
    let src = cobolt_codegen::generate(&form);
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<String> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the Guide's AWS examples do not parse:\n{}", errors.join("\n"));
    let program = parsed.program.expect("a program");
    let sem: Vec<String> = cobolt_semantic::analyze(&program).errors().map(|d| d.message.clone()).collect();
    assert!(sem.is_empty(), "the Guide's AWS examples fail the semantic analyser: {sem:?}");
    println!("078 AC19: {} Guide handlers compile: {}", handlers.len(), handlers.join(", "));
}
