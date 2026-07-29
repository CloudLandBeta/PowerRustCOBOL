// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Agent performance ratings, kept by Grace per project.
//!
//! Two design decisions shape this module:
//!
//! 1. **Ratings are telemetry for the developer, not motivation for the
//!    agents.** Specialists are stateless model calls — a broadcast score is
//!    non-actionable context, and pressure framing ("the user is displeased")
//!    is the same gradient that once produced a fabricated "Aprovação obtida".
//!    Agents therefore never see a star count; they receive only the factual
//!    [`RatingsBook::lessons_digest`] — their own recent correction reasons,
//!    which IS actionable.
//! 2. **Everything mechanical is computed, never asked.** The workflow record
//!    already carries rounds-to-approval and failure states; scoring reads it.
//!    The only human input is the developer's post-workflow star row.
//!
//! Star rules (the developer's scale):
//! - approved with no correction round: **+3**;
//! - approved after one correction round: **+1**;
//! - approved after two or more correction rounds: **0**;
//! - work not done (task failed): **−5** (totals can go negative);
//! - praised by the developer (4–5 stars clicked): **+5**;
//! - rejected by the developer (1–2 stars clicked): **−10**.
//!
//! A `Blocked` task is NOT scored: its agent never ran — the failure belongs
//! to the dependency that failed, which already took the −5.
//!
//! Totals are computed over a rolling window of the last [`WINDOW`] entries
//! per agent (the "reset after 20 rounds" rule as a sliding view; the full
//! history stays in the per-run record files).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cobolt_agents::grace::{TaskRecord, TaskState, WorkflowRecord, MACHINE_VALIDATOR};
use serde::{Deserialize, Serialize};

/// Rolling-window size per agent (the developer's "reset after 20 rounds").
pub const WINDOW: usize = 20;

pub const STARS_FIRST_ROUND: i32 = 3;
pub const STARS_ONE_CORRECTION: i32 = 1;
pub const STARS_LATE: i32 = 0;
pub const STARS_NOT_DONE: i32 = -5;
pub const STARS_DEV_PRAISE: i32 = 5;
pub const STARS_DEV_REJECT: i32 = -10;

/// Entry sources: computed from the workflow record, or the developer's stars.
pub const SOURCE_AUTO: &str = "auto";
pub const SOURCE_DEVELOPER: &str = "developer";

/// One scored event for one agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RatingEntry {
    pub workflow_id: String,
    /// Task id for auto entries; empty for developer feedback (per workflow).
    #[serde(default)]
    pub task_id: String,
    pub stars: i32,
    /// Stable snake_case reason (`approved_first_round`, `not_done`,
    /// `developer_praise`, …) plus an optional detail after `: `.
    pub reason: String,
    /// First line of each correction request the task received — the raw
    /// material of [`RatingsBook::lessons_digest`].
    #[serde(default)]
    pub lessons: Vec<String>,
    /// Seconds since the Unix epoch.
    #[serde(default)]
    pub at: u64,
    /// [`SOURCE_AUTO`] or [`SOURCE_DEVELOPER`].
    #[serde(default)]
    pub source: String,
}

/// The per-project book: newest entries last, capped at [`WINDOW`] per agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RatingsBook {
    #[serde(default)]
    pub agents: BTreeMap<String, Vec<RatingEntry>>,
}

/// Where the book lives — next to the saved workflow runs.
pub fn ratings_path(project_dir: &Path) -> PathBuf {
    project_dir
        .join("agentic_ai")
        .join("Grace")
        .join("ratings.json")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Score one finished task per the star rules. `None` when the task is not
/// scoreable (blocked, still pending, or Grace's own conversational record).
pub fn score_task(rec: &TaskRecord) -> Option<(i32, String, Vec<String>)> {
    // Correction rounds actually charged to the specialist: pedantic
    // rejections and machine-validation rejections alike — both sent the work
    // back. Lessons keep the first line of each correction so the digest can
    // name the recurring defect, with the machine validator's findings
    // included (they are the cheapest defects to stop repeating).
    let corrections: Vec<&str> = rec
        .reviews
        .iter()
        .filter(|round| round.defects)
        .map(|round| {
            round
                .correction_request
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("")
        })
        .collect();
    let lessons: Vec<String> = rec
        .reviews
        .iter()
        .filter(|round| round.defects)
        .map(|round| {
            let first = round
                .correction_request
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("(unspecified defect)")
                .trim();
            let mut line: String = first.chars().take(160).collect();
            if round.reviewer == MACHINE_VALIDATOR {
                line.push_str(" [machine validation]");
            }
            line
        })
        .collect();
    match rec.final_state {
        TaskState::Approved => {
            let (stars, reason) = match corrections.len() {
                0 => (STARS_FIRST_ROUND, "approved_first_round".to_string()),
                1 => (STARS_ONE_CORRECTION, "approved_after_1_correction".to_string()),
                n => (STARS_LATE, format!("approved_after_{n}_corrections")),
            };
            Some((stars, reason, lessons))
        }
        TaskState::Failed => {
            let detail: String = rec.failure_reason.chars().take(160).collect();
            Some((STARS_NOT_DONE, format!("not_done: {detail}"), lessons))
        }
        // The blocked agent never ran; the −5 already landed on the failed
        // dependency's agent.
        _ => None,
    }
}

impl RatingsBook {
    pub fn load(project_dir: &Path) -> Self {
        std::fs::read_to_string(ratings_path(project_dir))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, project_dir: &Path) -> Result<(), String> {
        let path = ratings_path(project_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    fn push(&mut self, agent: &str, entry: RatingEntry) {
        let entries = self.agents.entry(agent.to_string()).or_default();
        entries.push(entry);
        let excess = entries.len().saturating_sub(WINDOW);
        if excess > 0 {
            entries.drain(..excess);
        }
    }

    /// The agent's running total over its rolling window.
    pub fn total(&self, agent: &str) -> i32 {
        self.agents
            .get(agent)
            .map(|entries| entries.iter().map(|e| e.stars).sum())
            .unwrap_or(0)
    }

    /// Score every specialist task of a finished workflow. Returns one
    /// `(agent, stars, running_total)` triple per scored task, in task order.
    /// Grace's own record entries (direct answers) are never scored.
    pub fn record_workflow(&mut self, record: &WorkflowRecord) -> Vec<(String, i32, i32)> {
        let mut out = Vec::new();
        for task in &record.tasks {
            if task.spec.agent.eq_ignore_ascii_case(crate::agents_db::GRACE) {
                continue;
            }
            let Some((stars, reason, lessons)) = score_task(task) else {
                continue;
            };
            self.push(
                &task.spec.agent,
                RatingEntry {
                    workflow_id: record.workflow_id.clone(),
                    task_id: task.spec.id.clone(),
                    stars,
                    reason,
                    lessons,
                    at: now_secs(),
                    source: SOURCE_AUTO.into(),
                },
            );
            out.push((
                task.spec.agent.clone(),
                stars,
                self.total(&task.spec.agent),
            ));
        }
        out
    }

    /// Record the developer's star row for one agent of one workflow:
    /// 4–5 stars → +5 (praise), 1–2 stars → −10 (rejection), 3 → 0 (neutral),
    /// 0 → clear (the toggle switched off). Re-rating replaces the previous
    /// developer entry for the same workflow — the row is a toggle, not an
    /// append-only counter. Returns the recorded delta.
    pub fn record_developer_feedback(
        &mut self,
        workflow_id: &str,
        agent: &str,
        selected_stars: u8,
    ) -> i32 {
        if let Some(entries) = self.agents.get_mut(agent) {
            entries.retain(|e| !(e.source == SOURCE_DEVELOPER && e.workflow_id == workflow_id));
        }
        let (stars, reason) = match selected_stars {
            0 => return 0, // cleared
            1 | 2 => (STARS_DEV_REJECT, "developer_reject"),
            3 => (0, "developer_neutral"),
            _ => (STARS_DEV_PRAISE, "developer_praise"),
        };
        self.push(
            agent,
            RatingEntry {
                workflow_id: workflow_id.to_string(),
                task_id: String::new(),
                stars,
                reason: format!("{reason}: {selected_stars} star(s)"),
                lessons: Vec::new(),
                at: now_secs(),
                source: SOURCE_DEVELOPER.into(),
            },
        );
        stars
    }

    /// The factual feedback an agent DOES receive: its own recent correction
    /// reasons, deduplicated, newest first, capped at three. No scores, no
    /// praise, no displeasure — actionable facts only. `None` when the agent
    /// has no recorded corrections in its window.
    pub fn lessons_digest(&self, agent: &str) -> Option<String> {
        let entries = self.agents.get(agent)?;
        let mut lines: Vec<String> = Vec::new();
        for entry in entries.iter().rev() {
            for lesson in &entry.lessons {
                let lesson = lesson.trim();
                if lesson.is_empty()
                    || lines
                        .iter()
                        .any(|kept| kept.eq_ignore_ascii_case(lesson))
                {
                    continue;
                }
                lines.push(lesson.to_string());
                if lines.len() == 3 {
                    break;
                }
            }
            if lines.len() == 3 {
                break;
            }
        }
        if lines.is_empty() {
            return None;
        }
        let numbered = lines
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{}. {line}", i + 1))
            .collect::<Vec<_>>()
            .join("\n");
        Some(format!(
            "\n\n--- RECENT REVIEW LESSONS (factual; corrections your recent submissions in this project received) ---\n{numbered}\nDo not repeat these defects.\n"
        ))
    }

    /// Render a signed total as a compact star string for the progress log:
    /// `★★★` for 3, `☆` for 0, `−5` for negatives.
    pub fn stars_label(stars: i32) -> String {
        match stars {
            n if n < 0 => format!("−{}", -n),
            0 => "☆".into(),
            n => "★".repeat(n.min(5) as usize),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_agents::grace::{ReviewRound, TaskSpec};

    fn task(agent: &str, state: TaskState, corrections: &[&str]) -> TaskRecord {
        TaskRecord {
            spec: TaskSpec {
                id: "T1".into(),
                agent: agent.into(),
                objective: "obj".into(),
                context: String::new(),
                reviewer: Some("R".into()),
                depends_on: vec![],
                acceptance: "ok".into(),
            },
            states: vec![],
            submissions: vec!["work".into()],
            reviews: corrections
                .iter()
                .map(|c| ReviewRound {
                    reviewer: "R".into(),
                    defects: true,
                    correction_request: (*c).to_string(),
                    raw: (*c).to_string(),
                })
                .collect(),
            final_state: state,
            failure_reason: if state == TaskState::Failed {
                "provider unreachable".into()
            } else {
                String::new()
            },
        }
    }

    fn record(tasks: Vec<TaskRecord>) -> WorkflowRecord {
        WorkflowRecord {
            workflow_id: "wf-1".into(),
            tasks,
            status: "completed".into(),
            ..Default::default()
        }
    }

    fn tmp_project(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "prc-ratings-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The developer's scale, mechanically: 3 / 1 / 0 stars by correction
    /// rounds, −5 for a failed task, blocked tasks unscored.
    #[test]
    fn star_rules_follow_the_correction_rounds() {
        assert_eq!(
            score_task(&task("A", TaskState::Approved, &[])).unwrap().0,
            3
        );
        assert_eq!(
            score_task(&task("A", TaskState::Approved, &["1. fix X"]))
                .unwrap()
                .0,
            1
        );
        assert_eq!(
            score_task(&task("A", TaskState::Approved, &["1. fix X", "2. fix Y"]))
                .unwrap()
                .0,
            0
        );
        let (stars, reason, _) = score_task(&task("A", TaskState::Failed, &[])).unwrap();
        assert_eq!(stars, -5);
        assert!(reason.starts_with("not_done"), "{reason}");
        assert!(
            score_task(&task("A", TaskState::Blocked, &[])).is_none(),
            "a blocked agent never ran — nothing to score"
        );
    }

    #[test]
    fn workflow_recording_skips_grace_and_persists_a_rolling_window() {
        let dir = tmp_project("window");
        let mut book = RatingsBook::load(&dir);
        let scored = book.record_workflow(&record(vec![
            task(crate::agents_db::GRACE, TaskState::Approved, &[]),
            task("Form Designer Agent", TaskState::Approved, &[]),
            task("COBOL Event Handler Script Agent", TaskState::Failed, &[]),
        ]));
        assert_eq!(scored.len(), 2, "Grace's own record is never scored");
        assert_eq!(scored[0], ("Form Designer Agent".into(), 3, 3));
        assert_eq!(
            scored[1],
            ("COBOL Event Handler Script Agent".into(), -5, -5)
        );
        book.save(&dir).unwrap();

        // The window caps at 20 entries; totals slide, they don't grow forever.
        let mut book = RatingsBook::load(&dir);
        for _ in 0..30 {
            book.record_workflow(&record(vec![task(
                "Form Designer Agent",
                TaskState::Approved,
                &[],
            )]));
        }
        assert_eq!(book.agents["Form Designer Agent"].len(), WINDOW);
        assert_eq!(book.total("Form Designer Agent"), 3 * WINDOW as i32);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The star row is a toggle: re-rating replaces the previous developer
    /// entry for that workflow, and switching off clears it.
    #[test]
    fn developer_feedback_maps_and_replaces() {
        let dir = tmp_project("dev");
        let mut book = RatingsBook::load(&dir);
        assert_eq!(book.record_developer_feedback("wf-1", "A", 5), 5);
        assert_eq!(book.total("A"), 5);
        assert_eq!(book.record_developer_feedback("wf-1", "A", 1), -10);
        assert_eq!(book.total("A"), -10, "re-rating replaced the praise");
        assert_eq!(book.record_developer_feedback("wf-1", "A", 3), 0);
        assert_eq!(book.total("A"), 0, "neutral records a zero entry");
        assert_eq!(book.record_developer_feedback("wf-1", "A", 0), 0);
        assert!(book.agents["A"].is_empty(), "toggle off clears the entry");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Agents receive lessons, never scores: the digest carries recent
    /// correction reasons (deduped, capped at 3) and no star arithmetic.
    #[test]
    fn lessons_digest_is_factual_and_score_free() {
        let mut book = RatingsBook::default();
        book.record_workflow(&record(vec![task(
            "Form Designer Agent",
            TaskState::Approved,
            &[
                "1. handler bodies must include the three division headers",
                "1. handler bodies must include the three division headers",
                "2. do not hardcode colors across distinct controls",
            ],
        )]));
        let digest = book.lessons_digest("Form Designer Agent").unwrap();
        assert!(digest.contains("RECENT REVIEW LESSONS"));
        assert!(digest.contains("division headers"), "{digest}");
        assert!(digest.contains("hardcode colors"), "{digest}");
        assert_eq!(
            digest.matches("division headers").count(),
            1,
            "duplicates collapse"
        );
        for forbidden in ["star", "rating", "displeased", "−", "score"] {
            assert!(
                !digest.to_lowercase().contains(forbidden),
                "digest must stay factual, found {forbidden:?} in: {digest}"
            );
        }
        assert!(book.lessons_digest("Nobody").is_none());
    }

    #[test]
    fn stars_labels_render_the_scale() {
        assert_eq!(RatingsBook::stars_label(3), "★★★");
        assert_eq!(RatingsBook::stars_label(1), "★");
        assert_eq!(RatingsBook::stars_label(0), "☆");
        assert_eq!(RatingsBook::stars_label(-5), "−5");
        assert_eq!(RatingsBook::stars_label(-10), "−10");
    }
}
