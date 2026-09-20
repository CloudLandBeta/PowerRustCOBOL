// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The shipped Viewer example's COBOL **compiles**, not merely parses as XML.
//!
//! Same gate, and the same reason, as `snackbar_demo_compiles.rs`: a form whose
//! handlers do not compile loads perfectly and the developer only finds out at
//! Run. It matters more here than for most controls, because the Viewer's
//! surface is mostly METHODS (`Find`, `SaveAs`, `AppendMarkdown`,
//! `RegisterConversation`) reached through `Ctrl::Name(...)`, and that
//! vocabulary is CLOSED: an unlisted name parses its parentheses as a
//! collection subscript instead of a call, so `VWR-1::Print()` would silently
//! mean "element … of Print" and do nothing at all. No error, no event, no
//! clue. This test is what makes that failure loud.
//!
//! Unlike its siblings, this one reads the form from the REPOSITORY's own
//! `examples/PowerDemo3`, not from `~/Documents/PowerDemo3`. The sibling tests
//! skip themselves when that home-directory copy is absent, and on a machine
//! that never had one they have simply never run. A guard that silently does
//! nothing is not a guard.

use std::path::{Path, PathBuf};

fn demo_form() -> Option<PathBuf> {
    // `CARGO_MANIFEST_DIR` is crates/cobolt-ide; the example sits two levels up.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.parent()?;
    let p = repo.join("examples/PowerDemo3/forms/Common/viewer-form.cfrm");
    p.exists().then_some(p)
}

#[test]
fn the_viewer_example_generates_a_program_that_compiles() {
    let Some(path) = demo_form() else {
        panic!("examples/PowerDemo3/forms/Common/viewer-form.cfrm is missing");
    };
    let form = cobolt_forms::load_form(&path).expect("the example form must parse");

    // The control the whole example is about must survive the round trip as a
    // Viewer — `ControlType::from_str` is not an exhaustive match, so a control
    // it does not recognise comes back as `Custom` and paints as a grey box.
    let viewer = form
        .controls
        .iter()
        .find(|c| c.id == "VWR-1")
        .expect("the example must contain VWR-1");
    assert_eq!(
        viewer.control_type,
        cobolt_forms::ControlType::Viewer,
        "VWR-1 must reload as a Viewer, not as {:?}",
        viewer.control_type
    );

    let src = cobolt_codegen::generate(&form);

    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(
        &src,
        cobolt_lexer::SourceFormat::Free,
    ));
    let parse_errors: Vec<String> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("  line {}: {}", d.span.line, d.message))
        .collect();
    assert!(
        parse_errors.is_empty(),
        "the example's generated program does not parse:\n{}",
        parse_errors.join("\n")
    );
    let program = parsed
        .program
        .expect("a parse with no errors yields a program");

    let sem = cobolt_semantic::analyze(&program);
    let errors: Vec<String> = sem
        .errors()
        .map(|d| format!("  line {}: {}", d.span.line, d.message))
        .collect();
    assert!(
        errors.is_empty(),
        "the example's generated program has {} semantic error(s):\n{}",
        errors.len(),
        errors.join("\n")
    );

    // Every capability the example exists to demonstrate, still present in the
    // code it generates. A demo that quietly stops exercising one of these is
    // worse than no demo, because it reads as coverage.
    let must_carry = [
        ("the five layouts", "\"Streamed\""),
        ("page layout", "\"Page\""),
        ("raw layout", "\"Raw\""),
        ("split view", "\"LeftRight\""),
        ("stacked split", "\"TopBottom\""),
        ("card grid", "\"Cards\""),
        ("the filmstrip", "View1ShowFilmstrip"),
        ("zoom", "View1Zoom"),
        ("font size apart from zoom", "FontSize"),
        ("page navigation", "View1Page"),
        ("find", "VWR-1::Find()"),
        ("next match", "VWR-1::FindNext()"),
        ("previous match", "VWR-1::FindPrevious()"),
        ("case sensitivity", "View1SearchCaseSensitive"),
        ("match counting", "View1SearchMatchCount"),
        ("save as", "VWR-1::SaveAs()"),
        ("print", "VWR-1::Print()"),
        ("share", "VWR-1::Share()"),
        ("streamed appends", "VWR-1::AppendMarkdown("),
        ("jump to latest", "VWR-1::JumpToLatest()"),
        ("a new conversation", "VWR-1::NewConversation()"),
        ("registering a past one", "VWR-1::RegisterConversation("),
        ("selecting a past one", "VWR-1::SelectConversation("),
        ("opening a file the OS chose", "StagedFiles"),
        ("the resolved format", "VWR-1::Format"),
        ("the error text", "VWR-1::LastError"),
    ];
    eprintln!("\n  capability                      present");
    eprintln!("  -----------------------------   -------");
    let mut missing: Vec<&str> = Vec::new();
    for (what, needle) in must_carry {
        let ok = src.contains(needle);
        eprintln!("  {what:<29}   {}", if ok { "yes" } else { "NO" });
        if !ok {
            missing.push(what);
        }
    }
    assert!(
        missing.is_empty(),
        "the example stopped covering: {missing:?}"
    );

    // Each bound Viewer event needs a generated paragraph, or a handler wired
    // in the designer is simply unreachable at run time.
    let events = ["ONLOADED", "ONERROR", "ONCONVERSATIONSELECTED"];
    let bound = events
        .iter()
        .filter(|e| src.contains(&format!("VWR-1--{e}")))
        .count();
    eprintln!(
        "\n  {bound}/{} bound Viewer events have a generated handler",
        events.len()
    );
    assert_eq!(bound, events.len(), "every bound Viewer event must be wired");

    eprintln!(
        "  → {} controls, {} lines generated, parsed and analysed with 0 errors\n",
        form.controls.len(),
        src.lines().count()
    );

    // The documents the example opens on sight must actually be there, and be
    // what the form says they are. A demo whose first screen is "No document
    // loaded" teaches the wrong thing about the control.
    let project = path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("the form sits under <project>/forms/Common");

    let md = project.join("assets/docs/viewer-sample.md");
    let txt = project.join("assets/docs/viewer-sample.txt");
    assert!(md.exists(), "the Markdown sample is missing: {}", md.display());
    assert!(txt.exists(), "the text sample is missing: {}", txt.display());

    use cobolt_forms::viewer::{detect_format, index_text, DocumentSource, ViewerFormat};

    let md_bytes = std::fs::read(&md).expect("the Markdown sample must be readable");
    assert_eq!(
        detect_format(md.to_str(), &md_bytes),
        Some(ViewerFormat::Markdown),
        "the Markdown sample must resolve as Markdown"
    );

    let txt_bytes = std::fs::read(&txt).expect("the text sample must be readable");
    assert_eq!(
        detect_format(txt.to_str(), &txt_bytes),
        Some(ViewerFormat::Text),
        "the text sample must resolve as plain text"
    );

    // The text sample earns its place by carrying REAL page breaks, so the
    // page buttons in the demo move between pages the file declares rather
    // than scrolling one sheet.
    let index = index_text(&DocumentSource::Path(txt.to_string_lossy().into_owned()))
        .expect("the text sample must index");
    eprintln!(
        "  sample documents: markdown {} bytes, text {} bytes in {} pages\n",
        md_bytes.len(),
        txt_bytes.len(),
        index.page_count()
    );
    assert_eq!(
        index.page_count(),
        12,
        "the text sample's form feeds must yield 12 pages"
    );
}
