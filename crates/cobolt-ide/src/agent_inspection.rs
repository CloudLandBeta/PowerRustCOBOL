// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Live-UI eyes for the in-IDE agents (spec 027, operator decision 2026-07-16).
//!
//! The designer agent's context has always been the *form model* (JSON-ish
//! control descriptions). Since T10 the IDE also runs the egui inspection
//! plugin — the same machinery external MCP agents use — so the in-IDE agents
//! can additionally *see the rendered UI*: the AccessKit widget tree of the
//! current frame. This module drives the plugin **in-process** (no TCP
//! round-trip) and keeps the latest compact tree summary for the agent
//! context builder.
//!
//! Flow: [`request_snapshot`] submits a `GetTree` to the plugin (answered a
//! frame later); the reply is condensed and cached; [`latest_summary`] hands
//! the cache to whoever assembles the next agent prompt. Model mutations stay
//! model operations — this is eyes, not hands (the operator ruled UI-driving
//! out for form edits).

use std::sync::{Mutex, OnceLock};

use egui_inspection::{InspectionPlugin, Request, Response};

/// Cap on named nodes included in a summary (keeps prompt cost bounded).
const MAX_NODES: usize = 150;

fn cache() -> &'static Mutex<Option<String>> {
    static C: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

/// Ask the inspection plugin for a fresh widget-tree snapshot. The reply
/// arrives on a later frame; the condensed summary replaces the cache then.
/// Call when an agent turn starts (context for this turn uses the previous
/// snapshot) and again after a change-set is applied (so the next turn sees
/// the post-change UI).
pub fn request_snapshot(ctx: &egui::Context) {
    ctx.with_plugin(|p: &mut InspectionPlugin| {
        p.submit(Request::GetTree, |resp| {
            let summary = summarize(&resp);
            *cache().lock().unwrap() = Some(summary);
        });
    });
    ctx.request_repaint();
}

/// The most recent snapshot summary, if one has arrived yet.
pub fn latest_summary() -> Option<String> {
    cache().lock().unwrap().clone()
}

/// Condense a `Response::Tree` into an agent-friendly text block: one line
/// per *named* node — role, label, rounded bounds — bounded by [`MAX_NODES`].
fn summarize(resp: &Response) -> String {
    let Response::Tree {
        step,
        accesskit: Some(update),
        ..
    } = resp
    else {
        return "LIVE UI TREE: unavailable this frame".to_owned();
    };
    let mut lines = Vec::new();
    for (_id, node) in &update.nodes {
        if let Some(label) = node.label() {
            let b = node.bounds().unwrap_or_default();
            lines.push(format!(
                "{:?} {:?} @[{:.0},{:.0} {:.0}x{:.0}]",
                node.role(),
                label,
                b.x0,
                b.y0,
                b.x1 - b.x0,
                b.y1 - b.y0,
            ));
            if lines.len() >= MAX_NODES {
                lines.push(format!("… (truncated at {MAX_NODES} named nodes)"));
                break;
            }
        }
    }
    format!(
        "LIVE UI TREE (egui inspection, frame {step}, {} named nodes shown):\n{}",
        lines.len(),
        lines.join("\n"),
    )
}
