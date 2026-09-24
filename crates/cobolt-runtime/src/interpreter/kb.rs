// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `KnowledgeBase` control's methods and events (spec 068 §4.6).
//!
//! Methods are routed here **by the object's class**, before the general
//! dispatch — `Search`, `Cancel` and `Refresh` mean something else on other
//! controls.
//!
//! **Events carry their own values.** The async drain applies every delivered
//! result before any handler runs, so two progress reports written straight to
//! the properties would overwrite each other before the first `onProgress`
//! handler read them. Each event therefore queues its property values beside
//! it, and they are written only when that event is dispatched: every handler
//! sees exactly the values its own event carried.

use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::Interpreter;
use crate::async_op::{AsyncOutcome, KbHit};

/// The events a KnowledgeBase raises; each one has a payload queued with it.
const KB_EVENTS: [&str; 5] = ["onProgress", "onIndexed", "onSearchComplete", "onBusy", "onError"];

/// The values one KnowledgeBase event carries, applied when it is dispatched.
#[derive(Debug, Clone, Default)]
pub(crate) struct KbPayload {
    /// The control as the program named it (the key its properties use).
    pub obj: String,
    pub props: Vec<(String, String)>,
    /// A finished search's hits, which `GetResult…` then read.
    pub results: Option<Vec<KbHit>>,
}

/// What the interpreter keeps per KnowledgeBase between calls.
#[derive(Debug, Default)]
pub(crate) struct KbControlState {
    pub results: Vec<KbHit>,
    pub collections: Vec<String>,
    pub documents: Vec<String>,
    pub cancel: Arc<AtomicBool>,
}

impl Interpreter {
    pub(crate) fn is_knowledge_base(&self, obj: &str) -> bool {
        self.object_class(obj.trim()).as_deref() == Some("KnowledgeBase")
    }

    /// Queue one KnowledgeBase event with the values it carries.
    fn kb_raise(&mut self, obj: &str, event: &str, props: Vec<(String, String)>, results: Option<Vec<KbHit>>) {
        let spelled = self.form_spelling(obj);
        self.kb_payloads
            .entry(spelled.to_ascii_uppercase())
            .or_default()
            .push_back(KbPayload {
                obj: obj.to_string(),
                props,
                results,
            });
        self.async_dispatch_queue.push_back((spelled, event.to_string()));
    }

    /// Called as an event leaves the queue: write the values its KnowledgeBase
    /// event carried. Any other event is left alone.
    pub(crate) fn kb_apply_payload(&mut self, ctrl: &str, event: &str) {
        if !KB_EVENTS.contains(&event) {
            return;
        }
        let Some(payload) = self
            .kb_payloads
            .get_mut(&ctrl.to_ascii_uppercase())
            .and_then(VecDeque::pop_front)
        else {
            return;
        };
        for (prop, value) in payload.props {
            self.obj_set(&payload.obj, &prop, value);
        }
        if let Some(results) = payload.results {
            self.kb_states
                .entry(payload.obj.trim().to_ascii_uppercase())
                .or_default()
                .results = results;
        }
    }

    /// A KnowledgeBase worker reported. Runs on the interpreter thread from the
    /// async drain; `obj` is the control's id as the operation was started.
    pub(crate) fn kb_delivered(&mut self, obj: &str, outcome: AsyncOutcome) {
        let s = |k: &str, v: String| (k.to_string(), v);
        match outcome {
            AsyncOutcome::KbProgress { document, current, total } => self.kb_raise(
                obj,
                "onProgress",
                vec![
                    s("ProgressDocument", document),
                    s("ProgressCurrent", current.to_string()),
                    s("ProgressTotal", total.to_string()),
                ],
                None,
            ),
            AsyncOutcome::KbIndexed { added, updated, removed, skipped, note } => {
                let mut props = vec![
                    s("AddedCount", added.to_string()),
                    s("UpdatedCount", updated.to_string()),
                    s("RemovedCount", removed.to_string()),
                    s("SkippedCount", skipped.len().to_string()),
                    s("SkippedDocuments", skipped.join("; ")),
                    s("Busy", "0".into()),
                ];
                if !note.is_empty() {
                    props.push(s("SearchMode", "Lexical".into()));
                    props.push(s("SearchModeReason", note));
                }
                self.kb_raise(obj, "onIndexed", props, None);
            }
            AsyncOutcome::KbSearchDone { hits, mode, reason } => self.kb_raise(
                obj,
                "onSearchComplete",
                vec![
                    s("ResultCount", hits.len().to_string()),
                    s("SearchMode", mode),
                    s("SearchModeReason", reason),
                    s("Busy", "0".into()),
                ],
                Some(hits),
            ),
            AsyncOutcome::KbBusy { message } => self.kb_raise(
                obj,
                "onBusy",
                vec![s("LastError", message), s("Busy", "0".into())],
                None,
            ),
            AsyncOutcome::KbError { message } => self.kb_raise(
                obj,
                "onError",
                vec![s("LastError", message), s("Busy", "0".into())],
                None,
            ),
            // Not a KnowledgeBase outcome; never routed here.
            _ => {}
        }
    }
}

#[cfg(feature = "kb")]
impl Interpreter {
    fn kb_config(&self, obj: &str) -> crate::kb_runtime::KbConfig {
        use crate::kb_runtime::{app_base, resolve_location, EmbedderChoice, KbConfig};
        let get = |p: &str| self.obj_get(obj, p);
        let num = |p: &str, default: u64| get(p).trim().parse::<u64>().unwrap_or(default);
        KbConfig {
            location: resolve_location(&get("Location")),
            collection: get("Collection").trim().to_string(),
            embedder: EmbedderChoice::from_name(&get("Embedder")),
            url: get("EmbeddingURL"),
            api: get("EmbeddingAPI"),
            model: get("EmbeddingModel"),
            key: get("EmbeddingAPIKey"),
            write_wait: std::time::Duration::from_millis(num("WriteWaitMilliseconds", 5000)),
            max_results: num("MaximumResults", 5) as usize,
            archive_limits: cobolt_kb::convert::Limits {
                max_unpacked_bytes: num("ArchiveMaximumMegabytes", 500).saturating_mul(1024 * 1024),
                max_files: num("ArchiveMaximumFiles", 10_000) as usize,
                max_depth: num("ArchiveMaximumDepth", 3).min(u64::from(u8::MAX)) as u8,
            },
            models_dir: app_base().join("assets").join("models"),
        }
    }

    fn kb_state(&mut self, obj: &str) -> &mut KbControlState {
        self.kb_states.entry(obj.trim().to_ascii_uppercase()).or_default()
    }

    fn kb_fail(&mut self, obj: &str, message: String) -> String {
        self.obj_set(obj, "LastError", message);
        "0".into()
    }

    /// Start a background operation. `"1"` when it started; `"0"` with
    /// `LastError` set when it could not — one operation at a time per control.
    fn kb_start(&mut self, obj: &str, op: crate::kb_runtime::KbOp) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        if self.async_pending.contains_key(obj) {
            return self.kb_fail(
                obj,
                "another Knowledge Base operation is still running on this control".into(),
            );
        }
        let cfg = self.kb_config(obj);
        let generation = self
            .async_generations
            .entry(obj.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)))
            .fetch_add(1, Ordering::Relaxed)
            + 1;
        let cancel = Arc::new(AtomicBool::new(false));
        self.kb_state(obj).cancel = cancel.clone();
        self.obj_set(obj, "Busy", "1".into());
        self.obj_set(obj, "LastError", String::new());
        self.async_pending.insert(
            obj.to_string(),
            crate::async_op::PendingOp {
                generation,
                started_at: std::time::Instant::now(),
                // No interpreter-side timeout: indexing a large folder takes
                // as long as it takes, and Cancel is the way out.
                timeout_ms: 0,
            },
        );
        let tx = self.async_result_tx.clone();
        let ctrl_id = obj.to_string();
        std::thread::spawn(move || {
            crate::kb_runtime::run(cfg, op, cancel, |outcome| {
                let _ = tx.send(crate::async_op::AsyncOpResult {
                    ctrl_id: ctrl_id.clone(),
                    generation,
                    outcome,
                });
            });
        });
        "1".into()
    }

    /// A method called on a KnowledgeBase. `None` when the name is not one of
    /// its own, so the general dispatch handles it.
    pub(crate) fn kb_method(&mut self, obj: &str, m: &str, args: &[crate::value::CobolValue]) -> Option<String> {
        use crate::kb_runtime::{self as kr, KbOp};
        let arg = |i: usize| {
            args.get(i)
                .map(|v| v.as_display_string().trim().to_string())
                .unwrap_or_default()
        };
        // Text is taken as written: trailing spaces a COBOL item pads with are
        // not part of a document.
        let text = |i: usize| {
            args.get(i)
                .map(|v| v.as_display_string().trim_end().to_string())
                .unwrap_or_default()
        };
        let index = |i: usize| arg(i).parse::<usize>().ok().filter(|n| *n >= 1);
        let answer = match m {
            "CREATECOLLECTION" => {
                let cfg = self.kb_config(obj);
                match kr::create_collection(&cfg, &arg(0)) {
                    Ok(()) => "1".into(),
                    Err(e) => self.kb_fail(obj, e),
                }
            }
            "REMOVECOLLECTION" => {
                let cfg = self.kb_config(obj);
                match kr::remove_collection(&cfg, &arg(0)) {
                    Ok(()) => "1".into(),
                    Err(e) => self.kb_fail(obj, e),
                }
            }
            "LISTCOLLECTIONS" => {
                let cfg = self.kb_config(obj);
                let names = kr::list_collections(&cfg);
                let n = names.len();
                self.kb_state(obj).collections = names;
                self.obj_set(obj, "CollectionCount", n.to_string());
                n.to_string()
            }
            "GETCOLLECTION" => {
                let st = self.kb_state(obj);
                index(0).and_then(|i| st.collections.get(i - 1).cloned()).unwrap_or_default()
            }
            "LISTDOCUMENTS" => {
                let cfg = self.kb_config(obj);
                match kr::list_documents(&cfg) {
                    Ok(names) => {
                        let n = names.len();
                        self.kb_state(obj).documents = names;
                        self.obj_set(obj, "DocumentCount", n.to_string());
                        n.to_string()
                    }
                    Err(e) => self.kb_fail(obj, e),
                }
            }
            "GETDOCUMENT" => {
                let st = self.kb_state(obj);
                index(0).and_then(|i| st.documents.get(i - 1).cloned()).unwrap_or_default()
            }
            "GETRESULTDOCUMENT" | "GETRESULTHEADING" | "GETRESULTPASSAGE" | "GETRESULTSCORE" => {
                let st = self.kb_state(obj);
                index(0)
                    .and_then(|i| st.results.get(i - 1))
                    .map(|h| match m {
                        "GETRESULTDOCUMENT" => h.document.clone(),
                        "GETRESULTHEADING" => h.heading.clone(),
                        "GETRESULTPASSAGE" => h.passage.clone(),
                        _ => format!("{:.4}", h.score),
                    })
                    .unwrap_or_default()
            }
            "ADDDOCUMENT" | "UPDATEDOCUMENT" => {
                let op = KbOp::Put { name: arg(0), bytes: text(1).into_bytes() };
                self.kb_start(obj, op)
            }
            "IMPORTDOCUMENT" => {
                let source = std::path::PathBuf::from(arg(0));
                let name = if arg(1).is_empty() {
                    source.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string()
                } else {
                    arg(1)
                };
                self.kb_start(obj, KbOp::Import { source, name })
            }
            "DELETEDOCUMENT" => {
                let op = KbOp::Delete { name: arg(0) };
                self.kb_start(obj, op)
            }
            "REFRESH" => self.kb_start(obj, KbOp::Refresh),
            "REINDEX" => self.kb_start(obj, KbOp::Reindex),
            "SEARCH" => {
                let max = arg(1).parse::<usize>().ok().filter(|n| *n > 0);
                let max = max.unwrap_or_else(|| self.kb_config(obj).max_results);
                self.kb_start(obj, KbOp::Search { query: text(0), max })
            }
            "FETCHMODEL" => self.kb_start(obj, KbOp::FetchModel),
            "CANCEL" => {
                let flag = self.kb_state(obj).cancel.clone();
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
                self.cancel_async_op(obj);
                "1".into()
            }
            _ => return None,
        };
        Some(answer)
    }
}

#[cfg(not(feature = "kb"))]
impl Interpreter {
    /// This build left the Knowledge Base out: every one of its methods answers
    /// `"0"` and says why, instead of falling through to another control's
    /// method of the same name.
    pub(crate) fn kb_method(&mut self, obj: &str, m: &str, _args: &[crate::value::CobolValue]) -> Option<String> {
        const KB_METHODS: [&str; 20] = [
            "CREATECOLLECTION", "REMOVECOLLECTION", "LISTCOLLECTIONS", "GETCOLLECTION",
            "LISTDOCUMENTS", "GETDOCUMENT", "GETRESULTDOCUMENT", "GETRESULTHEADING",
            "GETRESULTPASSAGE", "GETRESULTSCORE", "ADDDOCUMENT", "UPDATEDOCUMENT",
            "IMPORTDOCUMENT", "DELETEDOCUMENT", "REFRESH", "REINDEX", "SEARCH",
            "FETCHMODEL", "CANCEL", "",
        ];
        if !KB_METHODS.contains(&m) {
            return None;
        }
        self.obj_set(
            obj,
            "LastError",
            "the Knowledge Base is not linked into this program (runtime feature `kb`)".into(),
        );
        Some("0".into())
    }
}

/// A collection handed to agents as a tool (spec 068 R32).
#[derive(Debug, Clone)]
pub(crate) struct KbTool {
    /// The tool name the model sees: `kb_<control>_<collection>`.
    pub name: String,
    /// The KnowledgeBase control whose settings the search uses.
    pub kb: String,
    pub collection: String,
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

impl Interpreter {
    pub(crate) fn kb_tool_is(&self, name: &str) -> bool {
        self.kb_tools.iter().any(|t| t.name.eq_ignore_ascii_case(name))
    }

    pub(crate) fn kb_tool_specs(&self) -> Vec<crate::agent_tools::ToolSpec> {
        self.kb_tools
            .iter()
            .map(|t| crate::agent_tools::ToolSpec {
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
                        "max_results": {"type": "integer", "description": "At most this many passages (default 5)"}
                    },
                    "required": ["query"]
                }),
            })
            .collect()
    }
}

#[cfg(feature = "kb")]
impl Interpreter {
    /// `AGT::AllowKnowledgeBase(kb [, collection])` — offer the collection to
    /// the model as a tool. `"1"`, or `"0"` when `kb` is not a KnowledgeBase or
    /// names no collection.
    pub(crate) fn agent_allow_kb(&mut self, kb: &str, collection: &str) -> String {
        let kb = kb.trim();
        if !self.is_knowledge_base(kb) {
            return "0".into();
        }
        let collection = if collection.trim().is_empty() {
            self.obj_get(kb, "Collection").trim().to_string()
        } else {
            collection.trim().to_string()
        };
        if cobolt_kb::store::validate_name(&collection).is_err() {
            return "0".into();
        }
        let name = format!("kb_{}_{}", slug(kb), slug(&collection));
        self.kb_tools.retain(|t| !t.name.eq_ignore_ascii_case(&name));
        self.kb_tools.push(KbTool {
            name,
            kb: kb.to_string(),
            collection,
        });
        "1".into()
    }

    pub(crate) fn agent_deny_kb(&mut self, kb: &str, collection: &str) {
        let kb = kb.trim();
        let collection = collection.trim();
        self.kb_tools.retain(|t| {
            !(t.kb.eq_ignore_ascii_case(kb)
                && (collection.is_empty() || t.collection.eq_ignore_ascii_case(collection)))
        });
    }

    /// Run a model's search of a collection. `Some(text)` when answered here;
    /// `None` when it went to a worker (the query needs the embedding server)
    /// and a `KbToolResult` will follow for `agent`.
    pub(crate) fn kb_tool_search(&mut self, agent: &str, call: &crate::agent_tools::ToolCall) -> Option<String> {
        use crate::kb_runtime::{self as kr, EmbedderChoice, KbOp};
        let tool = self.kb_tools.iter().find(|t| t.name.eq_ignore_ascii_case(&call.name))?.clone();
        let args = call.arguments.clone().unwrap_or_default();
        let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("").to_string();
        if query.trim().is_empty() {
            return Some("error: the search needs a \"query\"".into());
        }
        let mut cfg = self.kb_config(&tool.kb);
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
        if self.kb_config(&tool.kb).embedder != EmbedderChoice::Endpoint {
            // Lexical and built-in searches take milliseconds: answered here,
            // as the indexed-file tools are.
            return Some(run());
        }
        // The loop's own generation: the reply that carried this call has
        // already been drained, so `async_pending` no longer holds the Ask.
        let generation = self.tool_loop_generation(agent).unwrap_or_default();
        let tx = self.async_result_tx.clone();
        let agent = agent.to_string();
        let call_id = call.id.clone();
        std::thread::spawn(move || {
            let text = run();
            let _ = tx.send(crate::async_op::AsyncOpResult {
                ctrl_id: agent,
                generation,
                outcome: AsyncOutcome::KbToolResult { call_id, text },
            });
        });
        None
    }
}

#[cfg(not(feature = "kb"))]
impl Interpreter {
    pub(crate) fn agent_allow_kb(&mut self, _kb: &str, _collection: &str) -> String {
        "0".into()
    }
    pub(crate) fn agent_deny_kb(&mut self, _kb: &str, _collection: &str) {}
    pub(crate) fn kb_tool_search(&mut self, _agent: &str, _call: &crate::agent_tools::ToolCall) -> Option<String> {
        Some("error: the Knowledge Base is not linked into this program".into())
    }
}
