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

/// The interpreter's side of the collections handed to agents (spec 068 R32): the rules are
/// `kb_door`'s, shared with the desktop runner; the interpreter reads its KnowledgeBase controls and
/// runs a search that needs the embedding server on its async channel.
impl Interpreter {
    /// A property of the KnowledgeBase named `ctl`, `None` when no KnowledgeBase has that name.
    fn kb_peer(&self, ctl: &str, prop: &str) -> Option<String> {
        self.is_knowledge_base(ctl).then(|| self.obj_get(ctl.trim(), prop))
    }

    pub(crate) fn kb_tool_is(&self, agent: &str, name: &str) -> bool {
        kb_door::agent_tool_is(&self.kb_tools, agent, name)
    }

    pub(crate) fn kb_tool_specs(&self, agent: &str) -> Vec<crate::agent_tools::ToolSpec> {
        kb_door::agent_tool_specs(&self.kb_tools, agent, &|c: &str, p: &str| self.kb_peer(c, p))
    }

    /// `AGT::AllowKnowledgeBase(kb [, collection])` — offer the collection to
    /// the model as a tool. `"1"`, or `"0"` when `kb` is not a KnowledgeBase or
    /// names no collection.
    pub(crate) fn agent_allow_kb(&mut self, agent: &str, kb: &str, collection: &str) -> String {
        let mut tools = std::mem::take(&mut self.kb_tools);
        let ok = kb_door::allow_for_agent(&mut tools, agent, kb, collection, &|c: &str, p: &str| self.kb_peer(c, p));
        self.kb_tools = tools;
        if ok { "1" } else { "0" }.into()
    }

    pub(crate) fn agent_deny_kb(&mut self, agent: &str, kb: &str, collection: &str) {
        kb_door::deny_for_agent(&mut self.kb_tools, agent, kb, collection);
    }

    /// Run a model's search of a collection. `Some(text)` when answered here;
    /// `None` when it went to a worker (the query needs the embedding server)
    /// and a `KbToolResult` will follow for `agent`.
    pub(crate) fn kb_tool_search(&mut self, agent: &str, call: &crate::agent_tools::ToolCall, generation: u64) -> Option<String> {
        let search = kb_door::agent_search(&self.kb_tools, agent, call, &|c: &str, p: &str| self.kb_peer(c, p))?;
        let run = match search {
            kb_door::ToolSearch::Now(text) => return Some(text),
            kb_door::ToolSearch::Worker(run) => run,
        };
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
