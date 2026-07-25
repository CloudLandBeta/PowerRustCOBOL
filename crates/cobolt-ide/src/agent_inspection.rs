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

/// Execute an `egui.*` **observe-only** tool call for a specialist agent
/// (spec 030 R4/R5). Reads the cached snapshot the main thread keeps fresh —
/// worker-thread safe, and there is deliberately **no** mutation path here
/// (design changes go through the reviewable change-set path, not the live UI).
pub fn observe(tool: &str) -> crate::tool_exec::ToolResult {
    use crate::tool_exec::ToolResult;
    match tool {
        // Both expose the cached widget census (role · label · rounded bounds);
        // `rects` is an alias emphasising the geometry already carried per node.
        "egui.tree" | "egui.rects" => match latest_summary() {
            Some(s) => ToolResult::ok("read the live UI snapshot", s),
            None => ToolResult::err(
                "no live UI snapshot is available yet",
                "The inspection snapshot has not been captured this session.".to_string(),
            ),
        },
        other => ToolResult::err(
            format!("unknown egui observe tool \u{201c}{other}\u{201d}"),
            "Supported observe tools: egui.tree, egui.rects.".to_string(),
        ),
    }
}

/// Seed the snapshot cache directly. Test-only — production snapshots arrive
/// through [`request_snapshot`] on the main thread.
#[cfg(test)]
pub fn set_cache_for_test(summary: Option<String>) {
    *cache().lock().unwrap() = summary;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_returns_cached_census_and_is_read_only() {
        // With a snapshot, egui.tree / egui.rects surface it as evidence.
        set_cache_for_test(Some(
            "LIVE UI TREE (frame 7, 1 named nodes shown):\nButton \"OK\" @[10,10 60x24]".into(),
        ));
        let tree = observe("egui.tree");
        assert!(tree.ok);
        assert!(tree.detail.contains("Button"), "census surfaced: {tree:?}");
        let rects = observe("egui.rects");
        assert!(rects.ok && rects.detail.contains("60x24"));

        // With no snapshot, a clear (recoverable) message — never fabricated data.
        set_cache_for_test(None);
        let none = observe("egui.tree");
        assert!(!none.ok && !none.critical);

        // An unknown observe tool is rejected, not fabricated.
        set_cache_for_test(Some("x".into()));
        assert!(
            !observe("egui.click").ok,
            "there is no mutation/observe tool by that name"
        );
        set_cache_for_test(None);
    }
}
