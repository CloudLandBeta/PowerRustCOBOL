// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The KnowledgeBase control's own rules** (spec 068 §4.6), in one place for every host that runs
//! one: the interpreter, and the desktop runner of a program built on the WebAssembly path
//! (DESKTOP-WASM-001 T47b).
//!
//! A method is answered at once (the collections, the documents, the last search's results), or
//! starts one background operation per control ([`Job`]). That operation reports through
//! [`Report`]s: throttled progress, then one ending. Each report becomes an [`Event`] that carries
//! its own values, written on the control only as the event leaves the queue. Two progress
//! reports written straight to the properties would overwrite each other before the first
//! `onProgress` handler read them.
//!
//! The control's properties are read through a getter, so neither host's storage leaks in. What
//! the control keeps between calls ([`State`]) is the host's, one per control.
//!
//! The methods and the operations need the Knowledge Base (runtime feature `kb`); what a report
//! raises, and the results' shape, are compiled in every build, as the interpreter's drain is.

#[cfg(feature = "kb")]
use std::sync::atomic::AtomicBool;
#[cfg(feature = "kb")]
use std::sync::Arc;

use crate::async_op::{AsyncOutcome, KbHit};
#[cfg(feature = "kb")]
use crate::kb_runtime::{self as kr, KbConfig, KbOp};
#[cfg(feature = "kb")]
use crate::rest_door::Prop;

/// What a KnowledgeBase keeps between calls: the last search's results, and the lists
/// `ListCollections` and `ListDocuments` made, which `GetResult…`, `GetCollection` and
/// `GetDocument` read.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub results: Vec<KbHit>,
    pub collections: Vec<String>,
    pub documents: Vec<String>,
}

/// One event, with the values it writes on the control as it leaves the queue: its properties
/// and, for `onSearchComplete`, the results `GetResult…` then read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Event {
    pub name: String,
    pub props: Vec<(String, String)>,
    pub results: Option<Vec<KbHit>>,
}

/// A background operation, ready to run on a thread of its own.
#[cfg(feature = "kb")]
pub struct Job {
    cfg: KbConfig,
    op: KbOp,
}

#[cfg(feature = "kb")]
impl Job {
    /// Run the operation to its end on the calling thread, reporting through `send`: progress,
    /// then exactly one ending. `cancel` stops it between documents.
    pub fn run(self, cancel: Arc<AtomicBool>, send: impl Fn(Report)) {
        kr::run(self.cfg, self.op, cancel, |o| send(Report(o)));
    }
}

/// A report from a running operation.
#[derive(Debug, Clone)]
pub struct Report(pub(crate) AsyncOutcome);

impl Report {
    /// Does this report end its operation? A progress report does not: the control stays busy.
    pub fn is_final(&self) -> bool {
        !matches!(self.0, AsyncOutcome::KbProgress { .. })
    }
}

/// What a method called on a KnowledgeBase asks of the host.
#[cfg(feature = "kb")]
pub enum Call {
    /// Not one of the control's own methods: the host's general dispatch answers it.
    NotOwn,
    /// Answered at once: the properties written, in order, and the answer.
    Done { props: Vec<(String, String)>, answer: String },
    /// Run `job` on a thread of its own: write `props` (`Busy`, `LastError`) and answer `"1"`.
    Start { job: Job, props: Vec<(String, String)> },
    /// `Cancel`: the running operation, if any, is let go (`Busy` cleared, `onCancelled`) and its
    /// reports dropped. The answer is `"1"`.
    Cancel,
}

/// The settings a worker needs, read from the control's properties.
#[cfg(feature = "kb")]
pub(crate) fn config(prop: Prop<'_>) -> KbConfig {
    use crate::kb_runtime::{app_base, resolve_location, EmbedderChoice};
    let num = |p: &str, default: u64| prop(p).trim().parse::<u64>().unwrap_or(default);
    // Spec 076 R13 — a model-list entry gives the endpoint embedder its API, endpoint, model and
    // key.
    let entry_name = prop("ModelEntry").trim().to_string();
    let from_entry = (!entry_name.is_empty())
        .then(|| crate::model_list::get(&entry_name))
        .flatten()
        .map(|(e, _)| (e, crate::key_store::key_store().get(&entry_name).unwrap_or_default()));
    // An entry that names no model leaves the control's own `EmbeddingModel` in charge — it used to
    // hand the embedder an empty model (property audit, 2026-09-26).
    let (url, api, model, key) = match from_entry {
        Some((e, key)) => {
            let model = if e.model.trim().is_empty() { prop("EmbeddingModel") } else { e.model };
            (e.url, e.api, model, key)
        }
        None => (prop("EmbeddingURL"), prop("EmbeddingAPI"), prop("EmbeddingModel"), prop("EmbeddingAPIKey")),
    };
    KbConfig {
        location: resolve_location(&prop("Location")),
        collection: prop("Collection").trim().to_string(),
        embedder: EmbedderChoice::from_name(&prop("Embedder")),
        url,
        api,
        model,
        key,
        write_wait: std::time::Duration::from_millis(num("WriteWaitMilliseconds", 5000)),
        max_results: num("MaximumResults", 5) as usize,
        archive_limits: cobolt_kb::convert::Limits {
            max_unpacked_bytes: num("ArchiveMaximumMegabytes", 500).saturating_mul(1024 * 1024),
            max_files: num("ArchiveMaximumFiles", 10_000) as usize,
            max_depth: num("ArchiveMaximumDepth", 3).min(u64::from(u8::MAX)) as u8,
        },
        models_dir: cobolt_kb::model::app_models_dir(&app_base()),
    }
}

/// A method that failed at once: `LastError` says why, and the answer is `"0"`.
#[cfg(feature = "kb")]
fn failed(message: String) -> Call {
    Call::Done { props: vec![("LastError".into(), message)], answer: "0".into() }
}

/// Start `op`, or say why it cannot start: one operation at a time per control (`running`), and a
/// model entry the program has not handed over, or one with no key its API needs, fails here and
/// now rather than as an error from the provider much later.
#[cfg(feature = "kb")]
fn start(prop: Prop<'_>, running: bool, op: KbOp) -> Call {
    if running {
        return failed("another Knowledge Base operation is still running on this control".into());
    }
    let entry_name = prop("ModelEntry").trim().to_string();
    if !entry_name.is_empty() {
        let Some((entry, _)) = crate::model_list::get(&entry_name) else {
            return failed(format!(
                "model entry '{entry_name}' does not exist — the program has not handed it over with COBOL-MODEL-SET"
            ));
        };
        let key = crate::key_store::key_store().get(&entry_name);
        if entry.needs_key() && key.as_deref().map(str::trim).unwrap_or("").is_empty() {
            return failed(format!(
                "model entry '{entry_name}' has no key stored, and its API ({}) needs one — store it with COBOL-KEY-SET",
                entry.api
            ));
        }
    }
    Call::Start { job: Job { cfg: config(prop), op }, props: vec![("Busy".into(), "1".into()), ("LastError".into(), String::new())] }
}

/// What the method `method` (upper case) of a KnowledgeBase asks for, with the argument texts
/// `args`, its properties read through `prop`, what it keeps in `state` (changed by the methods
/// that list), and whether an operation of it is `running`.
#[cfg(feature = "kb")]
pub fn call(prop: Prop<'_>, state: &mut State, running: bool, method: &str, args: &[String]) -> Call {
    let arg = |i: usize| args.get(i).map(|v| v.trim().to_string()).unwrap_or_default();
    // Text is taken as written: trailing spaces a COBOL item pads with are not part of a document.
    let text = |i: usize| args.get(i).map(|v| v.trim_end().to_string()).unwrap_or_default();
    let index = |i: usize| arg(i).parse::<usize>().ok().filter(|n| *n >= 1);
    let done = |answer: String| Call::Done { props: Vec::new(), answer };
    match method {
        "CREATECOLLECTION" => match kr::create_collection(&config(prop), &arg(0)) {
            Ok(()) => done("1".into()),
            Err(e) => failed(e),
        },
        "REMOVECOLLECTION" => match kr::remove_collection(&config(prop), &arg(0)) {
            Ok(()) => done("1".into()),
            Err(e) => failed(e),
        },
        "LISTCOLLECTIONS" => {
            let names = kr::list_collections(&config(prop));
            let n = names.len();
            state.collections = names;
            Call::Done { props: vec![("CollectionCount".into(), n.to_string())], answer: n.to_string() }
        }
        "GETCOLLECTION" => done(index(0).and_then(|i| state.collections.get(i - 1).cloned()).unwrap_or_default()),
        "LISTDOCUMENTS" => match kr::list_documents(&config(prop)) {
            Ok(names) => {
                let n = names.len();
                state.documents = names;
                Call::Done { props: vec![("DocumentCount".into(), n.to_string())], answer: n.to_string() }
            }
            Err(e) => failed(e),
        },
        "GETDOCUMENT" => done(index(0).and_then(|i| state.documents.get(i - 1).cloned()).unwrap_or_default()),
        "GETRESULTDOCUMENT" | "GETRESULTHEADING" | "GETRESULTPASSAGE" | "GETRESULTSCORE" => done(
            index(0)
                .and_then(|i| state.results.get(i - 1))
                .map(|h| match method {
                    "GETRESULTDOCUMENT" => h.document.clone(),
                    "GETRESULTHEADING" => h.heading.clone(),
                    "GETRESULTPASSAGE" => h.passage.clone(),
                    _ => format!("{:.4}", h.score),
                })
                .unwrap_or_default(),
        ),
        "ADDDOCUMENT" | "UPDATEDOCUMENT" => start(prop, running, KbOp::Put { name: arg(0), bytes: text(1).into_bytes() }),
        "IMPORTDOCUMENT" => {
            let source = std::path::PathBuf::from(arg(0));
            let name = if arg(1).is_empty() { source.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string() } else { arg(1) };
            start(prop, running, KbOp::Import { source, name })
        }
        "DELETEDOCUMENT" => start(prop, running, KbOp::Delete { name: arg(0) }),
        "REFRESH" => start(prop, running, KbOp::Refresh),
        "REINDEX" => start(prop, running, KbOp::Reindex),
        "SEARCH" => {
            let max = arg(1).parse::<usize>().ok().filter(|n| *n > 0).unwrap_or_else(|| config(prop).max_results);
            start(prop, running, KbOp::Search { query: text(0), max })
        }
        "FETCHMODEL" => start(prop, running, KbOp::FetchModel),
        "CANCEL" => Call::Cancel,
        _ => Call::NotOwn,
    }
}

/// The event a report raises, with the values it carries (`None` for anything a worker does not
/// send).
pub fn delivered(report: Report) -> Option<Event> {
    let s = |k: &str, v: String| (k.to_string(), v);
    let event = |name: &str, props: Vec<(String, String)>, results: Option<Vec<KbHit>>| Some(Event { name: name.into(), props, results });
    match report.0 {
        AsyncOutcome::KbProgress { document, current, total, passage, passages } => event(
            "onProgress",
            vec![
                s("ProgressDocument", document),
                s("ProgressCurrent", current.to_string()),
                s("ProgressTotal", total.to_string()),
                s("ProgressPassage", passage.to_string()),
                s("ProgressPassages", passages.to_string()),
            ],
            None,
        ),
        AsyncOutcome::KbIndexed { added, updated, removed, skipped, note, passages, mode } => {
            let mut props = vec![
                s("AddedCount", added.to_string()),
                s("UpdatedCount", updated.to_string()),
                s("RemovedCount", removed.to_string()),
                s("SkippedCount", skipped.len().to_string()),
                s("SkippedDocuments", skipped.join("; ")),
                s("PassageCount", passages.to_string()),
                s("Busy", "0".into()),
            ];
            // Said on EVERY update that indexed something, the clean case included — a reason left
            // over from an earlier update must not read as this one's (operator, 2026-09-27: no
            // confirmation that the embedding was done).
            if !mode.is_empty() {
                props.push(s("SearchMode", mode));
                props.push(s("SearchModeReason", note));
            }
            event("onIndexed", props, None)
        }
        AsyncOutcome::KbSearchDone { hits, mode, reason } => event(
            "onSearchComplete",
            vec![s("ResultCount", hits.len().to_string()), s("SearchMode", mode), s("SearchModeReason", reason), s("Busy", "0".into())],
            Some(hits),
        ),
        AsyncOutcome::KbBusy { message } => event("onBusy", vec![s("LastError", message), s("Busy", "0".into())], None),
        AsyncOutcome::KbError { message } => event("onError", vec![s("LastError", message), s("Busy", "0".into())], None),
        // A worker sends nothing else.
        _ => None,
    }
}

/// Results as the desktop's ABI host carries them with `onSearchComplete`: one JSON object per
/// result (`document`, `heading`, `passage`, `score`).
pub fn result_rows(hits: &[KbHit]) -> Vec<serde_json::Value> {
    hits.iter()
        .map(|h| serde_json::json!({"document": h.document, "heading": h.heading, "passage": h.passage, "score": h.score}))
        .collect()
}

/// The results [`result_rows`] made, back. A score survives the trip exactly: an `f32` widened to
/// JSON's `f64` narrows back to itself.
pub fn results_of(rows: &[serde_json::Value]) -> Vec<KbHit> {
    let text = |r: &serde_json::Value, k: &str| r.get(k).and_then(serde_json::Value::as_str).unwrap_or_default().to_owned();
    rows.iter()
        .map(|r| KbHit {
            document: text(r, "document"),
            heading: text(r, "heading"),
            passage: text(r, "passage"),
            score: r.get("score").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32,
        })
        .collect()
}
