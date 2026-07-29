// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Live agent-action status (spec 036).
//!
//! The typed action stream that feeds the conversation history pane: each
//! entry names the *action* an agent is performing — never the content the
//! action produced or consumed (R4). The string progress log remains the full
//! trace; this module carries only the lightweight, localizable signal.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cobolt_agents::grace::ActionLogEntry;

use crate::i18n::Tr;

/// The chat-history role that marks a persisted action-history turn (spec
/// 036 R3/R11): its content is the JSON `Vec<ActionLogEntry>` of the run.
pub const ACTIONS_ROLE: &str = "actions";

/// Parse a persisted "actions" turn body back into renderable actions.
/// `None` when the content is not an action-log JSON (never panics on old or
/// hand-edited histories).
pub fn parse_actions_turn(content: &str) -> Option<Vec<AgentAction>> {
    let entries: Vec<ActionLogEntry> = serde_json::from_str(content).ok()?;
    Some(entries.iter().map(AgentAction::from_log_entry).collect())
}

/// The canonical action vocabulary (spec 036 R8/R9). Closed set: the IDE
/// localizes each kind at render time; records store the stable string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    ReceivingRequest,
    RetrievingContext,
    Planning,
    Drafting,
    RunningTool,
    Reviewing,
    ApplyingCorrections,
    Finishing,
    Blocked,
    Failed,
}

impl ActionKind {
    /// Every variant, for exhaustive tests and vocabulary review.
    pub const ALL: [ActionKind; 10] = [
        ActionKind::ReceivingRequest,
        ActionKind::RetrievingContext,
        ActionKind::Planning,
        ActionKind::Drafting,
        ActionKind::RunningTool,
        ActionKind::Reviewing,
        ActionKind::ApplyingCorrections,
        ActionKind::Finishing,
        ActionKind::Blocked,
        ActionKind::Failed,
    ];

    /// The stable snake_case identifier persisted in run records (R11):
    /// language-neutral, so a record replays correctly under any IDE language.
    pub fn stable(self) -> &'static str {
        match self {
            ActionKind::ReceivingRequest => "receiving_request",
            ActionKind::RetrievingContext => "retrieving_context",
            ActionKind::Planning => "planning",
            ActionKind::Drafting => "drafting",
            ActionKind::RunningTool => "running_tool",
            ActionKind::Reviewing => "reviewing",
            ActionKind::ApplyingCorrections => "applying_corrections",
            ActionKind::Finishing => "finishing",
            ActionKind::Blocked => "blocked",
            ActionKind::Failed => "failed",
        }
    }

    /// Reverse of [`Self::stable`], for rendering persisted records.
    pub fn from_stable(s: &str) -> Option<ActionKind> {
        Self::ALL.iter().copied().find(|k| k.stable() == s)
    }

    /// The localized, human-readable phrase for this action (R9).
    pub fn label(self, tr: &Tr) -> &'static str {
        match self {
            ActionKind::ReceivingRequest => tr.action_receiving_request,
            ActionKind::RetrievingContext => tr.action_retrieving_context,
            ActionKind::Planning => tr.action_planning,
            ActionKind::Drafting => tr.action_drafting,
            ActionKind::RunningTool => tr.action_running_tool,
            ActionKind::Reviewing => tr.action_reviewing,
            ActionKind::ApplyingCorrections => tr.action_applying_corrections,
            ActionKind::Finishing => tr.action_finishing,
            ActionKind::Blocked => tr.action_blocked,
            ActionKind::Failed => tr.action_failed,
        }
    }
}

/// One live action: who is doing what, with an optional short dynamic
/// fragment (task id, tool name…). Never carries payloads (R4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentAction {
    /// "Grace", a specialist name, or a reviewer name (R7).
    pub agent: String,
    pub kind: ActionKind,
    /// Verbatim dynamic fragment; may be empty. Interpolated, not localized.
    pub detail: String,
    /// Milliseconds since the Unix epoch when the action started.
    pub at: u64,
}

impl AgentAction {
    pub fn now(agent: impl Into<String>, kind: ActionKind, detail: impl Into<String>) -> Self {
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            agent: agent.into(),
            kind,
            detail: detail.into(),
            at,
        }
    }

    /// The persisted-record form (stable kind string, spec 036 R11).
    pub fn to_log_entry(&self) -> ActionLogEntry {
        ActionLogEntry {
            agent: self.agent.clone(),
            kind: self.kind.stable().to_string(),
            detail: self.detail.clone(),
            at: self.at,
        }
    }

    /// Rebuild from a persisted entry; unknown kinds map to `Drafting` so a
    /// record written by a newer IDE still renders rather than vanishing.
    pub fn from_log_entry(entry: &ActionLogEntry) -> Self {
        Self {
            agent: entry.agent.clone(),
            kind: ActionKind::from_stable(&entry.kind).unwrap_or(ActionKind::Drafting),
            detail: entry.detail.clone(),
            at: entry.at,
        }
    }

    /// The one-line pane text: `<agent>: <localized kind>` plus the verbatim
    /// detail when present (R7, R8, R9).
    pub fn display_line(&self, tr: &Tr) -> String {
        let label = self.kind.label(tr);
        if self.detail.is_empty() {
            format!("{}: {label}", self.agent)
        } else {
            format!("{}: {label} — {}", self.agent, self.detail)
        }
    }
}

/// Display-side throttle (spec 036 R6): the *visible* current-action line
/// changes at most once per interval; faster actions coalesce by jumping to
/// the latest once the window elapses. Nothing is dropped from the caller's
/// history — this type only decides what the single live line shows.
///
/// Pure over caller-supplied millisecond timestamps so it is unit-testable
/// with a simulated clock.
#[derive(Debug, Clone)]
pub struct ActionThrottle {
    interval: Duration,
    /// Index (into the caller's action history) currently on display.
    visible: Option<usize>,
    /// When the visible line last changed, in caller-clock milliseconds.
    last_change_ms: u64,
}

impl Default for ActionThrottle {
    fn default() -> Self {
        Self::new(Duration::from_secs(1))
    }
}

impl ActionThrottle {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            visible: None,
            last_change_ms: 0,
        }
    }

    /// Given the full action history and the current clock, return the index
    /// of the action to display. The first action shows immediately; after
    /// that the visible index only advances (to the **latest** entry) when
    /// the interval has elapsed since the last visible change.
    pub fn visible_index(&mut self, len: usize, now_ms: u64) -> Option<usize> {
        if len == 0 {
            self.visible = None;
            return None;
        }
        let latest = len - 1;
        match self.visible {
            None => {
                self.visible = Some(latest);
                self.last_change_ms = now_ms;
            }
            Some(current) if current < latest => {
                if now_ms.saturating_sub(self.last_change_ms) >= self.interval.as_millis() as u64 {
                    self.visible = Some(latest);
                    self.last_change_ms = now_ms;
                }
            }
            Some(_) => {}
        }
        self.visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 036 T2: every kind round-trips through its stable string.
    #[test]
    fn stable_strings_round_trip_every_variant() {
        for kind in ActionKind::ALL {
            let s = kind.stable();
            assert_eq!(
                ActionKind::from_stable(s),
                Some(kind),
                "{s} must round-trip"
            );
            assert!(
                s.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{s} must be snake_case"
            );
        }
        println!(
            "stable-string round-trip: {} variants verified",
            ActionKind::ALL.len()
        );
    }

    /// Spec 036 T2 / AC5: 10 actions in 2 s of simulated time yield at most
    /// one visible transition per second, the latest action wins each window,
    /// and the caller's history keeps all 10.
    #[test]
    fn throttle_coalesces_to_at_most_one_line_per_second() {
        let mut history: Vec<AgentAction> = Vec::new();
        let mut throttle = ActionThrottle::default();
        let mut displayed: Vec<usize> = Vec::new();

        // 10 actions, one every 200 ms (2 s total), polling after each.
        for i in 0..10u64 {
            let now_ms = i * 200;
            history.push(AgentAction {
                agent: "Grace".into(),
                kind: ActionKind::Drafting,
                detail: format!("step {i}"),
                at: now_ms,
            });
            if let Some(idx) = throttle.visible_index(history.len(), now_ms) {
                if displayed.last() != Some(&idx) {
                    displayed.push(idx);
                }
            }
        }
        // Transitions: t=0 shows #0; t=1000 shows #5; t=2000 window not yet
        // elapsed within the loop (last poll t=1800) → 2 visible lines.
        assert!(
            displayed.len() <= 3,
            "≤1 transition per second over 2 s (got {displayed:?})"
        );
        // A final poll once the stream is idle lands on the latest action.
        let idx = throttle.visible_index(history.len(), 3000).unwrap();
        assert_eq!(idx, 9, "the latest action wins after the window elapses");
        assert_eq!(history.len(), 10, "no action lost from the history");
        println!(
            "throttle: {} actions emitted, {} visible transitions (+ final catch-up to #{idx}), history retains {}",
            10,
            displayed.len(),
            history.len()
        );
    }

    /// Spec 036 T2: the display line is attributed and phrase-based (no
    /// internal function names — vocabulary comes from the Tr table).
    #[test]
    fn display_line_is_attributed_and_localized() {
        let tr = crate::i18n::Language::English.tr();
        let action = AgentAction {
            agent: "Form Designer Agent".into(),
            kind: ActionKind::RetrievingContext,
            detail: "BTN-OK".into(),
            at: 0,
        };
        let line = action.display_line(&tr);
        assert!(line.starts_with("Form Designer Agent: "), "attributed (R7)");
        assert!(line.contains(tr.action_retrieving_context), "localized (R9)");
        assert!(line.ends_with("— BTN-OK"), "detail verbatim");
        let bare = AgentAction {
            detail: String::new(),
            ..action
        };
        assert!(!bare.display_line(&tr).contains('—'), "no dash without detail");
        println!("display line: {line}");
    }
}
