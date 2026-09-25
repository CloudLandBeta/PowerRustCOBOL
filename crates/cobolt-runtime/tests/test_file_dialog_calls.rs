// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `COBOL-OPEN-FILE-DIALOG`, `COBOL-SAVE-FILE-DIALOG` and `COBOL-FOLDER-DIALOG`
//! from real COBOL, against a stand-in host that records each request and
//! answers as an operator would. What is pinned here is the runtime's half:
//! which dialog is asked for, with what title, filter, start folder and file
//! name, that the program waits for the answer, and that the answer (or SPACES
//! on cancel, or with no window at all) lands in the last argument.

use std::sync::mpsc;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::form_host::{FileDialogKind, FormRequest, ROOT_HANDLE};
use cobolt_runtime::Interpreter;

#[derive(Debug, Clone, PartialEq)]
struct Asked {
    kind: FileDialogKind,
    title: String,
    filters: Vec<(String, Vec<String>)>,
    directory: Option<String>,
    file_name: Option<String>,
}

fn program(body: &str) -> cobolt_ast::program::Program {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-PATH   PIC X(200).
       PROCEDURE DIVISION.
{body}
           STOP RUN.
"#
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    result.program.expect("no program")
}

/// Run `body` with a host that answers each dialog from `answers`, in order
/// (`None` = the operator cancelled). Returns the DISPLAY lines and what was
/// asked.
fn run(body: &str, answers: Vec<Option<&'static str>>) -> (Vec<String>, Vec<Asked>) {
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let (req_tx, req_rx) = mpsc::channel::<FormRequest>();
    let (_closed_tx, closed_rx) = mpsc::channel::<String>();
    let host = std::thread::spawn(move || {
        let mut asked = Vec::new();
        let mut answers = answers.into_iter();
        loop {
            match req_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(FormRequest::FileDialog { kind, title, filters, directory, file_name, reply }) => {
                    asked.push(Asked { kind, title, filters, directory, file_name });
                    // The operator takes a moment: the program must wait.
                    std::thread::sleep(Duration::from_millis(30));
                    let _ = reply.send(answers.next().flatten().map(str::to_owned));
                }
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        asked
    });
    let mut interp = Interpreter::new_with_channels(program(body), event_rx, state_tx, display_tx);
    interp.set_form_host(req_tx, ROOT_HANDLE, "MAIN-FORM", closed_rx);
    interp.run().expect("run failed");
    drop(interp);
    let asked = host.join().expect("host thread");
    (display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect(), asked)
}

#[test]
fn each_dialog_asks_the_host_and_the_program_gets_the_answer() {
    let t = std::time::Instant::now();
    let (out, asked) = run(
        r#"
           CALL "COBOL-FOLDER-DIALOG" USING "Knowledge Base folder" "/srv/kb" WS-PATH
           DISPLAY "FOLDER=" WS-PATH
           CALL "COBOL-SAVE-FILE-DIALOG" USING "Export settings"
                "XML files|xml" "rag-settings.xml" WS-PATH
           DISPLAY "SAVE=" WS-PATH
           CALL "COBOL-OPEN-FILE-DIALOG" USING "Import settings" "xml" WS-PATH
           DISPLAY "OPEN=[" WS-PATH "]"
           CALL "COBOL-OPEN-FILE-DIALOG" USING "Anything" "" WS-PATH
           DISPLAY "ANY=" WS-PATH
"#,
        vec![Some("/srv/kb/hr"), Some("/home/ana/rag-settings.xml"), None, Some("/tmp/a.txt")],
    );
    let joined = out.join("\n");
    assert!(joined.contains("FOLDER=/srv/kb/hr"), "{joined}");
    assert!(joined.contains("SAVE=/home/ana/rag-settings.xml"), "{joined}");
    assert!(joined.contains(&format!("OPEN=[{}]", " ".repeat(200))), "a cancel leaves SPACES: {joined}");
    assert!(joined.contains("ANY=/tmp/a.txt"), "{joined}");

    let xml = vec![("XML files".to_string(), vec!["xml".to_string()])];
    assert_eq!(
        asked,
        vec![
            Asked { kind: FileDialogKind::Folder, title: "Knowledge Base folder".into(), filters: vec![], directory: Some("/srv/kb".into()), file_name: None },
            Asked { kind: FileDialogKind::Save, title: "Export settings".into(), filters: xml.clone(), directory: None, file_name: Some("rag-settings.xml".into()) },
            Asked { kind: FileDialogKind::Open, title: "Import settings".into(), filters: vec![("xml".into(), vec!["xml".into()])], directory: None, file_name: None },
            Asked { kind: FileDialogKind::Open, title: "Anything".into(), filters: vec![], directory: None, file_name: None },
        ]
    );
    println!("\n  ── native file dialogs from COBOL ──");
    println!("  forms: FOLDER-DIALOG(title, start, path) · SAVE-FILE-DIALOG(title, filter, name, path)");
    println!("         OPEN-FILE-DIALOG(title, \"xml\", path) cancelled · OPEN-FILE-DIALOG(title, \"\", path)");
    println!("  4 dialogs asked, 3 answered, 1 cancel → SPACES — {:.0} ms\n", t.elapsed().as_secs_f64() * 1000.0);
}

/// With no window to show a dialog in — a console run — the call does not
/// hang: the path comes back as SPACES.
#[test]
fn with_no_window_the_path_is_spaces() {
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let body = r#"
           MOVE "stale" TO WS-PATH
           CALL "COBOL-FOLDER-DIALOG" USING "Folder" WS-PATH
           DISPLAY "[" WS-PATH(1:5) "]"
"#;
    let mut interp = Interpreter::new_with_channels(program(body), event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    let out: Vec<String> = display_rx.try_iter().collect();
    assert_eq!(out, vec!["[     ]"]);
}
