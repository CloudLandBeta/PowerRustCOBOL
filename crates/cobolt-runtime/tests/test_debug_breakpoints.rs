// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Breakpoints that do more than stop: conditions, hit counts, logpoints and
//! temporaries.
//!
//! Each is checked against a loop of a known length, so "stopped on the right
//! iteration" is a fact rather than an impression.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{
    new_breakpoint_specs, new_breakpoints, BreakpointSpec, DebugCmd, DebugEvent, Interpreter,
};

/// `WS-I` runs 1..=10; line 9 is the body statement every breakpoint sits on.
const LINES: &[&str] = &[
    "IDENTIFICATION DIVISION.",              // 1
    "PROGRAM-ID. BPTEST.",                   // 2
    "DATA DIVISION.",                        // 3
    "WORKING-STORAGE SECTION.",              // 4
    "01 WS-I PIC 9(3) VALUE 0.",             // 5
    "01 WS-T PIC 9(4) VALUE 0.",             // 6
    "PROCEDURE DIVISION.",                   // 7
    "MAIN.",                                 // 8
    "    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 10", // 9
    "        ADD WS-I TO WS-T",              // 10  <- the breakpoint line
    "    END-PERFORM",                       // 11
    "    STOP RUN.",                         // 12
];
const BP_LINE: u32 = 10;

/// Run with one breakpoint on line 10 carrying `spec`, continuing at every
/// stop. Returns the value of `WS-I` at each stop, plus any debugger output.
fn run(spec: BreakpointSpec) -> (Vec<String>, Vec<String>) {
    let src = LINES.join("\n");
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");

    let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
    let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
    let bps = new_breakpoints();
    bps.lock().unwrap().insert(BP_LINE);
    let specs = new_breakpoint_specs();
    specs.lock().unwrap().insert(BP_LINE, spec);

    let handle = thread::spawn(move || {
        let mut interp = Interpreter::new_with_debug_channels(program, cmd_rx, ev_tx, bps);
        interp.set_debug_breakpoint_specs(specs);
        let _ = interp.run();
    });

    let (mut stops, mut output) = (Vec::new(), Vec::new());
    // The session starts stopped at the first statement; continue past it and
    // then record only the breakpoint stops.
    let mut first = true;
    loop {
        match ev_rx.recv_timeout(Duration::from_secs(10)) {
            // Console only: the Events channel now carries ENTER/EXIT lines,
            // which are not what a logpoint test is measuring.
            Ok(DebugEvent::Output { text, channel })
                if channel == cobolt_runtime::OutputChannel::Console =>
            {
                output.push(text)
            }
            Ok(DebugEvent::Paused { vars, .. }) => {
                if first {
                    first = false;
                } else {
                    let i = vars
                        .iter()
                        .find(|v| v.name == "WS-I")
                        .map(|v| v.value.trim().to_owned())
                        .unwrap_or_default();
                    stops.push(i);
                }
                if cmd_tx.send(DebugCmd::Continue).is_err() {
                    break;
                }
            }
            Ok(DebugEvent::Finished) => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    drop(cmd_tx);
    let _ = handle.join();
    (stops, output)
}

/// A plain breakpoint stops on every pass — the baseline the rest are measured
/// against.
#[test]
fn a_plain_breakpoint_stops_on_every_iteration() {
    let (stops, _) = run(BreakpointSpec::default());
    assert_eq!(stops.len(), 10, "ten iterations: {stops:?}");
}

#[test]
fn a_condition_stops_only_where_it_holds() {
    let (stops, _) = run(BreakpointSpec {
        condition: Some("WS-I = 7".into()),
        ..Default::default()
    });
    assert_eq!(stops.len(), 1, "exactly one stop: {stops:?}");
    assert_eq!(stops[0].trim_start_matches('0'), "7");
}

#[test]
fn a_condition_can_match_a_range() {
    let (stops, _) = run(BreakpointSpec {
        condition: Some("WS-I > 8".into()),
        ..Default::default()
    });
    assert_eq!(stops.len(), 2, "9 and 10: {stops:?}");
}

/// The hit count is the number of times the line was REACHED, not the number of
/// times a condition held — which is what `>= 5` says.
#[test]
fn a_hit_count_fires_from_the_nth_arrival() {
    let (stops, _) = run(BreakpointSpec {
        hit_condition: Some(">= 8".into()),
        ..Default::default()
    });
    assert_eq!(stops.len(), 3, "hits 8, 9, 10: {stops:?}");

    let (exact, _) = run(BreakpointSpec {
        hit_condition: Some("3".into()),
        ..Default::default()
    });
    assert_eq!(exact.len(), 1, "only the 3rd: {exact:?}");
    assert_eq!(exact[0].trim_start_matches('0'), "3");

    let (every, _) = run(BreakpointSpec {
        hit_condition: Some("% 4".into()),
        ..Default::default()
    });
    assert_eq!(every.len(), 2, "the 4th and 8th: {every:?}");
}

/// The defining property of a logpoint: output without stopping.
#[test]
fn a_logpoint_produces_output_and_never_stops() {
    let (stops, output) = run(BreakpointSpec {
        log_message: Some("i is {WS-I}, total {WS-T}".into()),
        ..Default::default()
    });
    assert!(stops.is_empty(), "a logpoint must not stop: {stops:?}");
    assert_eq!(output.len(), 10, "one line per pass: {output:?}");
    assert!(output[0].starts_with("i is "), "{:?}", output[0]);
    assert!(output[0].contains("total"), "{:?}", output[0]);
    // The expressions were interpolated, not printed literally.
    assert!(!output[0].contains("{WS-I}"), "{:?}", output[0]);
}

/// An expression a logpoint cannot read must not silence the whole line — the
/// other fields are still worth having.
#[test]
fn a_logpoint_marks_what_it_could_not_read() {
    let (_, output) = run(BreakpointSpec {
        log_message: Some("i={WS-I} x={WS-NOPE}".into()),
        ..Default::default()
    });
    assert!(!output.is_empty());
    assert!(output[0].contains("i="), "{:?}", output[0]);
    assert!(output[0].contains("WS-NOPE=?"), "{:?}", output[0]);
}

#[test]
fn a_temporary_breakpoint_fires_once() {
    let (stops, _) = run(BreakpointSpec {
        temporary: true,
        ..Default::default()
    });
    assert_eq!(stops.len(), 1, "removed after firing: {stops:?}");
}

/// A condition that cannot be parsed or evaluated must be REPORTED and the
/// breakpoint still stop. Silently never firing is indistinguishable from a
/// debugger that is broken.
#[test]
fn a_broken_condition_is_reported_rather_than_silently_ignored() {
    let (stops, output) = run(BreakpointSpec {
        condition: Some("WS-I >".into()),
        ..Default::default()
    });
    assert!(!stops.is_empty(), "it must still stop");
    assert!(
        output.iter().any(|o| o.contains("line 10")),
        "the developer must be told: {output:?}"
    );

    let (_, bad_hits) = run(BreakpointSpec {
        hit_condition: Some("soon".into()),
        ..Default::default()
    });
    assert!(
        bad_hits.iter().any(|o| o.contains("not a hit count")),
        "{bad_hits:?}"
    );
}

/// Condition and hit count together: the count is of arrivals, so this is "the
/// 2nd time WS-I is even", not "the 2nd iteration".
#[test]
fn a_condition_and_a_hit_count_compose() {
    let (stops, _) = run(BreakpointSpec {
        condition: Some("WS-I > 5".into()),
        hit_condition: Some(">= 9".into()),
        ..Default::default()
    });
    // Hits 9 and 10 pass the count; both also satisfy WS-I > 5.
    assert_eq!(stops.len(), 2, "{stops:?}");
}

// ── Data breakpoints and exception filters ───────────────────────────────────

/// A program that changes one item, then opens a file that is not there.
const IO_SRC: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. IOTEST.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT MISSING-FILE ASSIGN TO \"no-such-file-for-the-debugger.dat\"
        ORGANIZATION IS LINE SEQUENTIAL
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD MISSING-FILE.
01 MISSING-REC PIC X(80).
WORKING-STORAGE SECTION.
01 WS-FS   PIC XX VALUE \"00\".
01 WS-N    PIC 9(3) VALUE 0.
PROCEDURE DIVISION.
MAIN.
    MOVE 5 TO WS-N
    OPEN INPUT MISSING-FILE
    CLOSE MISSING-FILE
    STOP RUN.
";

/// Run `IO_SRC`, continuing at every stop, and collect the stop reasons and the
/// debugger's channelled output.
fn run_io(
    watch: Option<&str>,
    filters: Vec<String>,
) -> (Vec<cobolt_runtime::StopReason>, Vec<(String, String)>) {
    use cobolt_runtime::OutputChannel;
    let result = parse(tokenize(IO_SRC, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
    let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
    let watch = watch.map(|w| w.to_owned());

    let handle = thread::spawn(move || {
        let mut interp =
            Interpreter::new_with_debug_channels(program, cmd_rx, ev_tx, new_breakpoints());
        interp.set_debug_exception_filters(filters);
        if let Some(w) = watch {
            interp.add_debug_data_watch(&w);
        }
        let _ = interp.run();
    });

    let (mut reasons, mut out) = (Vec::new(), Vec::new());
    loop {
        match ev_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(DebugEvent::Stopped { reason, .. }) => reasons.push(reason),
            Ok(DebugEvent::Output { text, channel }) => out.push((
                match channel {
                    OutputChannel::Console => "console",
                    OutputChannel::Events => "events",
                    OutputChannel::FileIo => "fileio",
                    OutputChannel::Problems => "problems",
                    OutputChannel::Timeline => "timeline",
                }
                .to_owned(),
                text,
            )),
            Ok(DebugEvent::Paused { .. }) => {
                if cmd_tx.send(DebugCmd::Continue).is_err() {
                    break;
                }
            }
            Ok(DebugEvent::Finished) => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    drop(cmd_tx);
    let _ = handle.join();
    (reasons, out)
}

/// Every file verb reports through one hook, so the File I/O tab is populated
/// without seven separate places to forget.
#[test]
fn file_operations_are_reported_on_the_file_io_channel() {
    let (_, out) = run_io(None, vec![]);
    let io: Vec<&String> = out.iter().filter(|(c, _)| c == "fileio").map(|(_, t)| t).collect();
    assert!(!io.is_empty(), "nothing on the File I/O channel: {out:?}");
    assert!(
        io.iter().any(|l| l.starts_with("OPEN")),
        "the OPEN must name its verb: {io:?}"
    );
    assert!(
        io.iter().any(|l| l.contains("MISSING-FILE")),
        "and its file: {io:?}"
    );
    // 35 — the file is not there, which is the point of the fixture.
    assert!(io.iter().any(|l| l.contains("status 35")), "{io:?}");
}

#[test]
fn program_entry_and_exit_are_reported_on_the_events_channel() {
    let (_, out) = run_io(None, vec![]);
    let ev: Vec<&String> = out.iter().filter(|(c, _)| c == "events").map(|(_, t)| t).collect();
    assert!(ev.iter().any(|l| l.contains("ENTER")), "{ev:?}");
    assert!(ev.iter().any(|l| l.contains("EXIT")), "{ev:?}");
    assert!(ev.iter().any(|l| l.contains("IOTEST")), "{ev:?}");
}

/// Off by default: a bad FILE STATUS is extremely common in normal COBOL and
/// stopping on every one uninvited would make the debugger unusable.
#[test]
fn a_file_status_error_stops_only_when_its_filter_is_on() {
    let (quiet, _) = run_io(None, vec![]);
    assert!(
        !quiet.iter().any(|r| matches!(r, cobolt_runtime::StopReason::Exception { .. })),
        "no filter, no exception stop: {quiet:?}"
    );

    let (loud, out) = run_io(None, vec!["fileStatus".into()]);
    let exception = loud.iter().find_map(|r| match r {
        cobolt_runtime::StopReason::Exception { filter, detail } => Some((filter, detail)),
        _ => None,
    });
    let (filter, detail) = exception.unwrap_or_else(|| panic!("no exception stop: {loud:?}"));
    assert_eq!(filter, "fileStatus");
    assert!(detail.contains("35"), "{detail}");
    assert!(
        out.iter().any(|(c, t)| c == "problems" && t.contains("35")),
        "and it is reported in Problems: {out:?}"
    );
}

/// A data breakpoint fires on the CHANGE, not on the fact that the item exists.
#[test]
fn a_data_breakpoint_stops_when_the_item_changes() {
    let (reasons, out) = run_io(Some("WS-N"), vec![]);
    let changed = reasons.iter().find_map(|r| match r {
        cobolt_runtime::StopReason::DataChanged { name, value } => Some((name, value)),
        _ => None,
    });
    let (name, value) = changed.unwrap_or_else(|| panic!("no data stop: {reasons:?}"));
    assert!(name.contains("WS-N"), "{name}");
    assert_eq!(value.trim_start_matches('0'), "5", "MOVE 5 TO WS-N");
    assert!(
        out.iter().any(|(c, t)| c == "events" && t.contains("CHANGED")),
        "{out:?}"
    );
}

/// A data breakpoint on a name that does not exist is REFUSED, not accepted and
/// then silently never fired.
#[test]
fn a_data_breakpoint_on_an_unknown_item_is_refused() {
    let result = parse(tokenize(IO_SRC, SourceFormat::Free));
    let program = result.program.expect("no program");
    let (_cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
    let (ev_tx, _ev_rx) = mpsc::channel::<DebugEvent>();
    let mut interp =
        Interpreter::new_with_debug_channels(program, cmd_rx, ev_tx, new_breakpoints());
    assert!(interp.add_debug_data_watch("WS-N"), "a real item is accepted");
    assert!(
        !interp.add_debug_data_watch("WS-NOT-A-THING"),
        "an unknown item must be refused"
    );
}
