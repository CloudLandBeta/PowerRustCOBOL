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
//!
//! The collections a program hands its AgentObjects as tools (`AllowKnowledgeBase`, spec 068 R32)
//! are kept and searched here too, for the same two hosts ([`AgentKbTool`], [`agent_search`]).

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

// ── The collections an AgentObject may search (spec 068 R32) ───────────────────────────────────
//
// `AllowKnowledgeBase(kb [, collection])` hands a collection to ONE agent as a tool, which the
// model calls with a query; the search runs with the KnowledgeBase control's settings as they are
// when the model calls it. A host keeps the list (one per program) and reads its KnowledgeBase
// controls through a getter that answers only for a KnowledgeBase.

/// A property of the program's KnowledgeBase control named by the first argument: `None` when no
/// KnowledgeBase has that name.
pub type KbProp<'a> = &'a dyn Fn(&str, &str) -> Option<String>;

/// A collection handed to an agent as a tool (spec 068 R32).
#[derive(Debug, Clone)]
pub struct AgentKbTool {
    /// The tool name the model sees: `kb_<control>_<collection>`.
    pub name: String,
    /// The KnowledgeBase control whose settings the search uses.
    pub kb: String,
    pub collection: String,
    /// The AgentObject it was granted to, upper-cased. A tool is THAT
    /// agent's: granting one to an agent, or taking one from another, never
    /// touches the others' (operator, 2026-09-27 — PowerChat's election
    /// denied the collection to agents 2 and 3 after granting it to the tool
    /// worker, and the shared list left no agent able to search).
    pub agent: String,
}

/// How a model's search of a collection is answered.
pub enum ToolSearch {
    /// At once: lexical and built-in searches take milliseconds, and are answered as the
    /// indexed-file tools are.
    Now(String),
    /// On a worker: the query needs the embedding server. The host runs it on a thread of its own
    /// and hands the text to the agent's tool loop when it is done
    /// (`agent_door::tool_worker_delivered`).
    Worker(Box<dyn FnOnce() -> String + Send>),
}

fn slug(s: &str) -> String {
    let out: String = s
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();
    out.trim_matches('_').to_string()
}

/// A search's hits as the model reads them: numbered, each naming its
/// document and section, so it can cite them (R33).
pub(crate) fn tool_text(collection: &str, hits: &[KbHit], mode: &str) -> String {
    if hits.is_empty() {
        return format!("no passage in the \"{collection}\" collection matched");
    }
    let mut out = format!("{} passage(s) from the \"{collection}\" collection ({mode} search):\n", hits.len());
    for (i, h) in hits.iter().enumerate() {
        out.push_str(&format!(
            "\n[{}] document: {} § {} (score {:.2})\n{}\n",
            i + 1,
            h.document,
            h.heading,
            h.score,
            h.passage.trim()
        ));
    }
    out
}

/// Is `name` one of the collections handed to `agent`?
pub fn agent_tool_is(tools: &[AgentKbTool], agent: &str, name: &str) -> bool {
    let agent = agent.trim().to_ascii_uppercase();
    tools.iter().any(|t| t.agent == agent && t.name.eq_ignore_ascii_case(name))
}

/// A KnowledgeBase control's `MaximumResults`, as a tool description states it. Without `kb` no
/// tool is ever handed over, so there is no description to write.
#[cfg(feature = "kb")]
fn default_max_results(kb: KbProp<'_>, control: &str) -> usize {
    config(&|p: &str| kb(control, p).unwrap_or_default()).max_results
}

#[cfg(not(feature = "kb"))]
fn default_max_results(_kb: KbProp<'_>, _control: &str) -> usize {
    0
}

/// The collections handed to `agent`, as the model is offered them.
pub fn agent_tool_specs(tools: &[AgentKbTool], agent: &str, kb: KbProp<'_>) -> Vec<crate::agent_tools::ToolSpec> {
    let agent = agent.trim().to_ascii_uppercase();
    tools
        .iter()
        .filter(|t| t.agent == agent)
        .map(|t| {
            // The default the call really gets is the control's
            // `MaximumResults`; the description used to say 5 whatever it
            // was set to.
            let default = default_max_results(kb, &t.kb);
            crate::agent_tools::ToolSpec {
                name: t.name.clone(),
                description: format!(
                    "Search the \"{}\" document collection. Returns the passages that best \
                     match the query, each naming the document and section it comes from.",
                    t.collection
                ),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "What to look for, in plain words"},
                        "max_results": {"type": "integer", "description": format!("At most this many passages (default {default})")}
                    },
                    "required": ["query"]
                }),
            }
        })
        .collect()
}

/// `AGT::AllowKnowledgeBase(kb [, collection])` — offer the collection to
/// the model as a tool. `true`, or `false` when `kb` is not a KnowledgeBase or
/// names no collection.
#[cfg(feature = "kb")]
pub fn allow_for_agent(tools: &mut Vec<AgentKbTool>, agent: &str, kb_name: &str, collection: &str, kb: KbProp<'_>) -> bool {
    let agent = agent.trim().to_ascii_uppercase();
    let kb_name = kb_name.trim();
    let Some(own) = kb(kb_name, "Collection") else {
        return false;
    };
    let collection = if collection.trim().is_empty() { own.trim().to_string() } else { collection.trim().to_string() };
    if cobolt_kb::store::validate_name(&collection).is_err() {
        return false;
    }
    let name = format!("kb_{}_{}", slug(kb_name), slug(&collection));
    tools.retain(|t| !(t.agent == agent && t.name.eq_ignore_ascii_case(&name)));
    tools.push(AgentKbTool { name, kb: kb_name.to_string(), collection, agent });
    true
}

/// Without the Knowledge Base no collection can be handed over.
#[cfg(not(feature = "kb"))]
pub fn allow_for_agent(_tools: &mut Vec<AgentKbTool>, _agent: &str, _kb_name: &str, _collection: &str, _kb: KbProp<'_>) -> bool {
    false
}

/// `AGT::DenyKnowledgeBase(kb [, collection])` — take the collection (every collection of `kb`,
/// when none is named) from `agent`. The others' are untouched.
pub fn deny_for_agent(tools: &mut Vec<AgentKbTool>, agent: &str, kb_name: &str, collection: &str) {
    let agent = agent.trim().to_ascii_uppercase();
    let kb_name = kb_name.trim();
    let collection = collection.trim();
    tools.retain(|t| {
        !(t.agent == agent
            && t.kb.eq_ignore_ascii_case(kb_name)
            && (collection.is_empty() || t.collection.eq_ignore_ascii_case(collection)))
    });
}

/// A model's search of a collection handed to `agent`, with its KnowledgeBase's settings as they are
/// now. `None` when `call` names none of them.
#[cfg(feature = "kb")]
pub fn agent_search(tools: &[AgentKbTool], agent: &str, call: &crate::agent_tools::ToolCall, kb: KbProp<'_>) -> Option<ToolSearch> {
    use crate::kb_runtime::EmbedderChoice;
    let owner = agent.trim().to_ascii_uppercase();
    let tool = tools.iter().find(|t| t.agent == owner && t.name.eq_ignore_ascii_case(&call.name))?.clone();
    let args = call.arguments.clone().unwrap_or_default();
    let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("").to_string();
    if query.trim().is_empty() {
        return Some(ToolSearch::Now("error: the search needs a \"query\"".into()));
    }
    let prop = |p: &str| kb(&tool.kb, p).unwrap_or_default();
    let mut cfg = config(&prop);
    let endpoint = cfg.embedder == EmbedderChoice::Endpoint;
    cfg.collection = tool.collection.clone();
    let max = args
        .get("max_results")
        .and_then(|m| m.as_u64())
        .map(|m| m as usize)
        .filter(|m| *m > 0)
        .unwrap_or(cfg.max_results)
        .min(20);
    let collection = tool.collection.clone();
    let run = move || -> String {
        let (tx, rx) = std::sync::mpsc::channel();
        kr::run(cfg, KbOp::Search { query, max }, Arc::new(AtomicBool::new(false)), |o| {
            let _ = tx.send(o);
        });
        match rx.try_iter().last() {
            Some(AsyncOutcome::KbSearchDone { hits, mode, .. }) => tool_text(&collection, &hits, &mode),
            Some(AsyncOutcome::KbError { message }) | Some(AsyncOutcome::KbBusy { message }) => {
                format!("error: {message}")
            }
            _ => "error: the search did not run".into(),
        }
    };
    if !endpoint {
        return Some(ToolSearch::Now(run()));
    }
    Some(ToolSearch::Worker(Box::new(run)))
}

/// Without the Knowledge Base no collection is ever handed over; a call naming one is told why.
#[cfg(not(feature = "kb"))]
pub fn agent_search(tools: &[AgentKbTool], agent: &str, call: &crate::agent_tools::ToolCall, _kb: KbProp<'_>) -> Option<ToolSearch> {
    agent_tool_is(tools, agent, &call.name).then(|| ToolSearch::Now("error: the Knowledge Base is not linked into this program".into()))
}
