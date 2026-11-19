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
use crate::kb_door;

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
    /// The last search's results and the lists its methods made ([`kb_door::State`], the
    /// desktop runner's too).
    pub kept: kb_door::State,
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
                .kept
                .results = results;
        }
    }

    /// A KnowledgeBase worker reported. Runs on the interpreter thread from the
    /// async drain; `obj` is the control's id as the operation was started.
    /// What each report raises and carries is [`kb_door::delivered`]'s, the
    /// desktop runner's too (DESKTOP-WASM-001 T47b).
    pub(crate) fn kb_delivered(&mut self, obj: &str, outcome: AsyncOutcome) {
        if let Some(e) = kb_door::delivered(kb_door::Report(outcome)) {
            self.kb_raise(obj, &e.name, e.props, e.results);
        }
    }
}

#[cfg(feature = "kb")]
impl Interpreter {
    fn kb_config(&self, obj: &str) -> crate::kb_runtime::KbConfig {
        kb_door::config(&|p: &str| self.obj_get(obj, p))
    }

    fn kb_state(&mut self, obj: &str) -> &mut KbControlState {
        self.kb_states.entry(obj.trim().to_ascii_uppercase()).or_default()
    }

    /// A method called on a KnowledgeBase. `None` when the name is not one of
    /// its own, so the general dispatch handles it. What each method answers,
    /// writes or starts is [`kb_door::call`]'s, the desktop runner's too
    /// (DESKTOP-WASM-001 T47b); the interpreter runs the operation it starts
    /// on a thread of its own, one at a time per control.
    pub(crate) fn kb_method(&mut self, obj: &str, m: &str, args: &[crate::value::CobolValue]) -> Option<String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        let texts: Vec<String> = args.iter().map(|v| v.as_display_string()).collect();
        let running = self.async_pending.contains_key(obj);
        let mut kept = std::mem::take(&mut self.kb_state(obj).kept);
        let call = kb_door::call(&|p: &str| self.obj_get(obj, p), &mut kept, running, m, &texts);
        self.kb_state(obj).kept = kept;
        match call {
            kb_door::Call::NotOwn => None,
            kb_door::Call::Done { props, answer } => {
                for (p, v) in props {
                    self.obj_set(obj, &p, v);
                }
                Some(answer)
            }
            kb_door::Call::Start { job, props } => {
                let generation = self
                    .async_generations
                    .entry(obj.to_string())
                    .or_insert_with(|| Arc::new(AtomicU64::new(0)))
                    .fetch_add(1, Ordering::Relaxed)
                    + 1;
                let cancel = Arc::new(AtomicBool::new(false));
                self.kb_state(obj).cancel = cancel.clone();
                for (p, v) in props {
                    self.obj_set(obj, &p, v);
                }
                self.async_pending.insert(
                    obj.to_string(),
                    crate::async_op::PendingOp {
                        generation,
                        started_at: std::time::Instant::now(),
                        // No interpreter-side timeout: indexing a large folder takes
                        // as long as it takes, and Cancel is the way out.
                        timeout_ms: 0,
                        awaiting_start: None,
                    },
                );
                let tx = self.async_result_tx.clone();
                let ctrl_id = obj.to_string();
                std::thread::spawn(move || {
                    job.run(cancel, |report| {
                        let _ = tx.send(crate::async_op::AsyncOpResult { ctrl_id: ctrl_id.clone(), generation, outcome: report.0 });
                    });
                });
                Some("1".into())
            }
            kb_door::Call::Cancel => {
                let flag = self.kb_state(obj).cancel.clone();
                flag.store(true, Ordering::Relaxed);
                self.cancel_async_op(obj);
                Some("1".into())
            }
        }
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
    /// The AgentObject it was granted to, upper-cased. A tool is THAT
    /// agent's: granting one to an agent, or taking one from another, never
    /// touches the others' (operator, 2026-09-27 — PowerChat's election
    /// denied the collection to agents 2 and 3 after granting it to the tool
    /// worker, and the shared list left no agent able to search).
    pub agent: String,
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
    pub(crate) fn kb_tool_is(&self, agent: &str, name: &str) -> bool {
        let agent = agent.trim().to_ascii_uppercase();
        self.kb_tools
            .iter()
            .any(|t| t.agent == agent && t.name.eq_ignore_ascii_case(name))
    }

    /// A KnowledgeBase control's `MaximumResults`, as a tool description
    /// states it.
    ///
    /// This impl block is compiled WITHOUT the `kb` feature too, and
    /// `kb_config` exists only with it: calling it straight from
    /// [`Self::kb_tool_specs`] (1.70.346) made `cobolt-runtime` fail to build
    /// for every compiled application that does not link the Knowledge Base.
    #[cfg(feature = "kb")]
    fn kb_default_max_results(&self, kb: &str) -> usize {
        self.kb_config(kb).max_results
    }

    /// Without `kb` no tool is ever registered (`AllowKnowledgeBase` lives in
    /// the gated block), so there is no description to write.
    #[cfg(not(feature = "kb"))]
    fn kb_default_max_results(&self, _kb: &str) -> usize {
        0
    }

    pub(crate) fn kb_tool_specs(&self, agent: &str) -> Vec<crate::agent_tools::ToolSpec> {
        let agent = agent.trim().to_ascii_uppercase();
        self.kb_tools
            .iter()
            .filter(|t| t.agent == agent)
            .map(|t| {
                // The default the call really gets is the control's
                // `MaximumResults`; the description used to say 5 whatever it
                // was set to.
                let default = self.kb_default_max_results(&t.kb);
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
            }})
            .collect()
    }
}

#[cfg(feature = "kb")]
impl Interpreter {
    /// `AGT::AllowKnowledgeBase(kb [, collection])` — offer the collection to
    /// the model as a tool. `"1"`, or `"0"` when `kb` is not a KnowledgeBase or
    /// names no collection.
    pub(crate) fn agent_allow_kb(&mut self, agent: &str, kb: &str, collection: &str) -> String {
        let agent = agent.trim().to_ascii_uppercase();
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
        self.kb_tools
            .retain(|t| !(t.agent == agent && t.name.eq_ignore_ascii_case(&name)));
        self.kb_tools.push(KbTool {
            name,
            kb: kb.to_string(),
            collection,
            agent,
        });
        "1".into()
    }

    pub(crate) fn agent_deny_kb(&mut self, agent: &str, kb: &str, collection: &str) {
        let agent = agent.trim().to_ascii_uppercase();
        let kb = kb.trim();
        let collection = collection.trim();
        self.kb_tools.retain(|t| {
            !(t.agent == agent
                && t.kb.eq_ignore_ascii_case(kb)
                && (collection.is_empty() || t.collection.eq_ignore_ascii_case(collection)))
        });
    }

    /// Run a model's search of a collection. `Some(text)` when answered here;
    /// `None` when it went to a worker (the query needs the embedding server)
    /// and a `KbToolResult` will follow for `agent`.
    pub(crate) fn kb_tool_search(&mut self, agent: &str, call: &crate::agent_tools::ToolCall, generation: u64) -> Option<String> {
        use crate::kb_runtime::{self as kr, EmbedderChoice, KbOp};
        let owner = agent.trim().to_ascii_uppercase();
        let tool = self
            .kb_tools
            .iter()
            .find(|t| t.agent == owner && t.name.eq_ignore_ascii_case(&call.name))?
            .clone();
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
        // `generation` is the loop's own: the reply that carried this call has
        // already been drained, so `async_pending` no longer holds the Ask.
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
    pub(crate) fn agent_allow_kb(&mut self, _agent: &str, _kb: &str, _collection: &str) -> String {
        "0".into()
    }
    pub(crate) fn agent_deny_kb(&mut self, _agent: &str, _kb: &str, _collection: &str) {}
    pub(crate) fn kb_tool_search(&mut self, _agent: &str, _call: &crate::agent_tools::ToolCall, _generation: u64) -> Option<String> {
        Some("error: the Knowledge Base is not linked into this program".into())
    }
}
