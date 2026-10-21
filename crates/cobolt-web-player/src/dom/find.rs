// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Find in a Viewer** (spec 058 R26–R30, WASM-004): the browser half.
//!
//! The Find bar and what its keys and buttons ask for are in `view/viewer.rs` and `view/interact.rs`
//! (the needle, the case switch and the current match are the control's properties). This half does what
//! only the page can: it **counts the matches in the text the Viewer shows**, reports the current one back
//! (`View1SearchCurrentMatch`, from 1, 0 = none, as the desktop Viewer reports it), highlights them and
//! brings the current one into view.
//!
//! Highlighting uses the **CSS Custom Highlight API** (`CSS.highlights`): the matches are *ranges*
//! registered under two names, `pr-find` and `pr-find-current`, which the base sheet styles with
//! `::highlight()`. The document is never changed — no node is split or wrapped — so the Viewer's own
//! nodes stay exactly what the view described, and a browser without the API loses only the colour (the
//! count, the current match and the scrolling still work).
//!
//! The matching is `cobolt_forms::viewer::find_matches`, the desktop Viewer's own (case-insensitive
//! without lowercasing the text, no overlapping matches, an empty needle is zero matches).

use std::collections::HashMap;
use std::rc::Rc;

use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, Node, Range, Text};

use super::Shared;
use crate::api::Inner;
use crate::value::truthy;

/// The most matches highlighted at once: a one-letter needle in a long document stays responsive.
const MAX_RANGES: usize = 5000;

/// What the page last did for one Viewer.
#[derive(Default)]
pub(crate) struct FindRec {
    ranges: Vec<Range>,
    current: Option<Range>,
    /// `(needle, case, current)` the current match was last scrolled into view for.
    scrolled: Option<(String, bool, usize)>,
}

fn text_nodes(root: &Node, out: &mut Vec<Text>) {
    let mut child = root.first_child();
    while let Some(c) = child {
        match c.node_type() {
            Node::TEXT_NODE => {
                if let Ok(t) = c.clone().dyn_into::<Text>() {
                    out.push(t);
                }
            }
            Node::ELEMENT_NODE => {
                // The Find bar and the page's own furniture are not the document.
                let skip = c.dyn_ref::<Element>().is_some_and(|e| e.class_list().contains("pr-findbar"));
                if !skip {
                    text_nodes(&c, out);
                }
            }
            _ => {}
        }
        child = c.next_sibling();
    }
}

/// UTF-16 units before byte offset `at` of `s`: what a DOM `Range` counts in.
fn utf16_at(s: &str, at: usize) -> u32 {
    s[..at.min(s.len())].encode_utf16().count() as u32
}

fn registry() -> Option<JsValue> {
    let css = Reflect::get(&js_sys::global(), &"CSS".into()).ok().filter(|v| v.is_object())?;
    Reflect::get(&css, &"highlights".into()).ok().filter(|v| v.is_object())
}

fn set_highlight(reg: &JsValue, name: &str, ranges: &[Range]) {
    let (Ok(ctor), Ok(set), Ok(delete)) = (
        Reflect::get(&js_sys::global(), &"Highlight".into()),
        Reflect::get(reg, &"set".into()),
        Reflect::get(reg, &"delete".into()),
    ) else {
        return;
    };
    let (Some(ctor), Some(set), Some(delete)) = (ctor.dyn_ref::<Function>(), set.dyn_ref::<Function>(), delete.dyn_ref::<Function>()) else { return };
    if ranges.is_empty() {
        let _ = delete.call1(reg, &name.into());
        return;
    }
    let args = Array::new();
    for r in ranges {
        args.push(r);
    }
    if let Ok(h) = Reflect::construct(ctor, &args) {
        let _ = set.call2(reg, &name.into(), &h);
    }
}

impl Shared {
    /// Count, report, highlight and scroll for every Viewer of `inst` whose Find bar is open — and take the
    /// highlights off one whose bar has closed.
    pub(crate) fn sync_find(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32) {
        let wanted: Vec<(usize, bool, String, bool, usize)> = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            tree.ctls
                .iter()
                .enumerate()
                .filter(|(_, c)| matches!(c.control_type(), Some(cobolt_forms::model::ControlType::Viewer)))
                .map(|(i, _)| {
                    let open = truthy(&tree.get(i, "View1FindOpen"));
                    let needle = tree.get(i, "View1SearchText");
                    let case = truthy(&tree.get(i, "View1SearchCaseSensitive"));
                    let cur = tree.get(i, "View1SearchCurrentMatch").trim().parse::<f64>().map(|v| v.max(0.0) as usize).unwrap_or(0);
                    (i, open, needle, case, cur)
                })
                .collect()
        };
        let mut changed = false;
        for (ctl, open, needle, case, current) in wanted {
            let key = (inst, ctl);
            if !open || needle.is_empty() {
                changed |= self.find.borrow_mut().remove(&key).is_some();
                if self.state.try_borrow().map(|st| st.instances.get(&inst).is_some_and(|iv| iv.ui.find_total.contains_key(&ctl))).unwrap_or(false) {
                    self.with_ui(inst, |ui| {
                        ui.find_total.remove(&ctl);
                    });
                }
                continue;
            }
            let Some(content) = self.control_element(inst, ctl).and_then(|el| el.query_selector(".pr-content").ok().flatten()) else { continue };
            // The text the Viewer shows, node by node, joined by a line feed so that no match runs across two nodes.
            let mut nodes = Vec::new();
            text_nodes(&content, &mut nodes);
            let mut hay = String::new();
            let mut starts = Vec::with_capacity(nodes.len());
            for n in &nodes {
                starts.push(hay.len());
                hay.push_str(&n.data());
                hay.push('\n');
            }
            let spans = cobolt_forms::viewer::find_matches(&hay, &needle, case);
            let total = spans.len();
            let reported = if total == 0 { 0 } else { current.clamp(1, total) };
            let mut ranges = Vec::new();
            let mut current_range = None;
            for (k, s) in spans.iter().enumerate().take(MAX_RANGES.max(reported)) {
                // The node the span starts in (starts are ascending): a match never crosses a node.
                let at = starts.partition_point(|&b| b <= s.start).saturating_sub(1);
                let (Some(node), Some(base)) = (nodes.get(at), starts.get(at)) else { continue };
                let text = node.data();
                let (from, to) = (s.start - base, (s.end - base).min(text.len()));
                if let (Some(doc), true) = (node.owner_document(), from <= to) {
                    if let Ok(r) = doc.create_range() {
                        let _ = r.set_start(node, utf16_at(&text, from));
                        let _ = r.set_end(node, utf16_at(&text, to));
                        if k + 1 == reported {
                            current_range = Some(r.clone());
                        }
                        ranges.push(r);
                    }
                }
            }
            // The desktop reports the current match back once it has laid the text out; so does the page.
            if reported != current {
                if let Ok(mut h) = inner.host.try_borrow_mut() {
                    h.ui_set(inst, ctl, "View1SearchCurrentMatch", &reported.to_string());
                }
            }
            let old_total = self.state.try_borrow().ok().and_then(|st| st.instances.get(&inst).and_then(|iv| iv.ui.find_total.get(&ctl).copied()));
            if old_total != Some(total) {
                self.with_ui(inst, |ui| {
                    ui.find_total.insert(ctl, total);
                });
                self.mark_dirty(inst, ctl);
            }
            let mut find = self.find.borrow_mut();
            let rec = find.entry(key).or_default();
            // Scroll only when the needle, the case or the current match moved: a redraw must not drag the
            // document back while the user reads on.
            let marker = (needle.clone(), case, reported);
            if reported > 0 && rec.scrolled.as_ref() != Some(&marker) {
                if let Some(r) = &current_range {
                    scroll_into_view(r);
                }
                rec.scrolled = Some(marker);
            }
            rec.ranges = ranges;
            rec.current = current_range;
            changed = true;
        }
        if changed {
            self.publish_find();
        }
    }

    /// `FindNext` / `FindPrevious` from the program: the next (or previous) match, wrapping round.
    pub(crate) fn find_step(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize, forward: bool) {
        let total = self.state.try_borrow().ok().and_then(|st| st.instances.get(&inst).and_then(|iv| iv.ui.find_total.get(&ctl).copied())).unwrap_or(0);
        let current = inner.host.try_borrow().ok().and_then(|h| h.trees.get(&inst).map(|t| t.get(ctl, "View1SearchCurrentMatch").trim().parse::<f64>().unwrap_or(0.0).max(0.0) as usize)).unwrap_or(0);
        if total == 0 {
            return;
        }
        let to = if forward {
            if current == 0 || current >= total { 1 } else { current + 1 }
        } else if current <= 1 {
            total
        } else {
            current - 1
        };
        if let Ok(mut h) = inner.host.try_borrow_mut() {
            h.ui_set(inst, ctl, "View1SearchCurrentMatch", &to.to_string());
        }
    }

    /// Put every Viewer's ranges under the two names the base sheet styles.
    fn publish_find(&self) {
        let Some(reg) = registry() else { return };
        let find = self.find.borrow();
        let all: Vec<Range> = find.values().flat_map(|r| r.ranges.iter().cloned()).collect();
        let cur: Vec<Range> = find.values().filter_map(|r| r.current.clone()).collect();
        set_highlight(&reg, "pr-find", &all);
        set_highlight(&reg, "pr-find-current", &cur);
    }

    /// An instance went away: its highlights go with it.
    pub(crate) fn clear_find(&self, inst: u32) {
        let had = {
            let mut find = self.find.borrow_mut();
            let before = find.len();
            find.retain(|(i, _), _| *i != inst);
            find.len() != before
        };
        if had {
            self.publish_find();
        }
    }
}

fn scroll_into_view(r: &Range) {
    let Some(el) = r.start_container().ok().and_then(|n| if n.node_type() == Node::ELEMENT_NODE { n.dyn_into::<Element>().ok() } else { n.parent_element() }) else { return };
    let opts = Object::new();
    let _ = Reflect::set(&opts, &"block".into(), &"center".into());
    let _ = Reflect::set(&opts, &"inline".into(), &"nearest".into());
    if let Some(f) = Reflect::get(&el, &"scrollIntoView".into()).ok().and_then(|f| f.dyn_into::<Function>().ok()) {
        let _ = f.call1(&el, &opts);
    }
}

/// The per-Viewer records, kept by the surface.
pub(crate) type FindMap = HashMap<(u32, usize), FindRec>;
