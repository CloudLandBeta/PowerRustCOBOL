// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Model Leaderboard — the ranked record of every COBOL-proficiency test.
//!
//! The store is **machine-wide**, not per project: it lives beside
//! `llm_config.json` in [`crate::llm::base_dir`], so a model tested while one
//! project was open still ranks when the next is. (The per-project
//! `agentic_ai/model-benchmarks.jsonl` archive keeps the full report text and
//! is untouched by this module — this is the parsed, rankable view.)
//!
//! One [`Entry`] per provider+model, holding what the test actually judges:
//! COBOL and PowerRustCOBOL competence. Speed, memory and token cost are
//! deliberately **not** ranked here — they measure the machine and the moment,
//! not the model's grasp of the language, and a board that mixes the two
//! invites picking a model for being quick at being wrong.
//!
//! A run that could not start at all (no key, refused connection, rate limit)
//! is recorded with an error and **no** scores. Such an entry is unrated: it
//! has no rank, shows no stars, and sorts below every rated model.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Where a model runs and what it costs, which is what separates the boards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tier {
    CloudFree,
    CloudPaid,
    Local,
}

impl Tier {
    /// Best-effort classification from the connection alone.
    ///
    /// **Local** means the weights run on hardware the developer controls —
    /// plain `ollama`, llama.cpp. `ollama_cloud` is deliberately *not* local
    /// despite the shared name: nothing runs on this machine, there is no
    /// quantization or hardware of the developer's to report, and it bills.
    ///
    /// OpenRouter marks its no-charge routes with a `:free` model suffix, and
    /// HuggingFace's router serves its free tier the same way. Everything else
    /// is assumed to bill, because assuming a paid model is free is the mistake
    /// that costs money — the developer can override it per entry.
    pub fn classify(provider: &str, model: &str) -> Self {
        let p = provider.trim().to_ascii_lowercase();
        if p == "ollama" || p == "local" || p == "llamacpp" || p == "llama_cpp" {
            return Tier::Local;
        }
        let m = model.trim().to_ascii_lowercase();
        if m.ends_with(":free") || m.contains(":free/") || m.contains("-free") {
            return Tier::CloudFree;
        }
        Tier::CloudPaid
    }
}

/// Which board is being read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Board {
    Overall,
    CloudFree,
    CloudPaid,
    Local,
}

impl Board {
    pub const ALL: [Board; 4] = [
        Board::Overall,
        Board::CloudFree,
        Board::CloudPaid,
        Board::Local,
    ];

    pub fn accepts(self, tier: Tier) -> bool {
        match self {
            Board::Overall => true,
            Board::CloudFree => tier == Tier::CloudFree,
            Board::CloudPaid => tier == Tier::CloudPaid,
            Board::Local => tier == Tier::Local,
        }
    }
}

/// What the connection probe learned about a model before the benchmark ran.
///
/// Every field is optional on purpose: a provider that does not publish its
/// limits leaves them unknown, and "unknown" must stay distinguishable from
/// "zero" — a context window shown as 0 would be a lie the developer could act
/// on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    /// Supported input (context) tokens.
    #[serde(default)]
    pub ctx_in: Option<u32>,
    /// Supported output (completion) tokens.
    #[serde(default)]
    pub ctx_out: Option<u32>,
    /// Parameter count in billions, where the provider states it.
    #[serde(default)]
    pub params_b: Option<f32>,
    /// Weight quantization, for local models ("Q4_K_M").
    #[serde(default)]
    pub quantization: Option<String>,
    /// Price per 1M output tokens, in USD.
    #[serde(default)]
    pub usd_per_mtok_out: Option<f32>,
}

impl ModelCapabilities {
    /// Merge `other` in, keeping what this already knows. A later probe against
    /// a provider that publishes less must not erase what an earlier one found.
    pub fn merge(&mut self, other: ModelCapabilities) {
        if other.ctx_in.is_some() {
            self.ctx_in = other.ctx_in;
        }
        if other.ctx_out.is_some() {
            self.ctx_out = other.ctx_out;
        }
        if other.params_b.is_some() {
            self.params_b = other.params_b;
        }
        if other.quantization.is_some() {
            self.quantization = other.quantization;
        }
        if other.usd_per_mtok_out.is_some() {
            self.usd_per_mtok_out = other.usd_per_mtok_out;
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == ModelCapabilities::default()
    }
}

/// One completed proficiency run.
pub struct RunOutcome {
    /// The parsed `metrics` JSON block from the report.
    pub metrics: serde_json::Value,
}

/// Every metric key the board can display, in report order.
///
/// The first block is what the proficiency prompt has always returned; the
/// second was added for the leaderboard and is absent from any result recorded
/// before it. A missing key reads as "not collected", never as zero.
pub const SCORE_KEYS: &[&str] = &[
    "overall_score",
    "compilation_score",
    "functional_score",
    "instruction_following",
    "semantic_correctness",
    "code_preservation",
    "runtime_correctness",
    "hallucination_resistance",
    "formatting_preservation",
    "cobol85_score",
    "powerrustcobol_score",
    "program_structure_score",
    "data_description_score",
    "control_flow_score",
    "file_handling_score",
    "forms_extensions_score",
    "unsupported_feature_avoidance",
    // Added for the leaderboard.
    "indexed_file_score",
    "modification_score",
    "debugging_score",
    "refactoring_score",
    "table_driven_score",
    "type_inference_score",
    "inline_invoke_score",
    "code_explanation_score",
];

/// Metric key carrying the count (not the percentage) of invented verbs,
/// controls, properties and methods the reviewer found.
pub const HALLUCINATION_COUNT_KEY: &str = "hallucination_count";

/// One tested model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub endpoint: String,
    /// Set only when the developer disagrees with [`Tier::classify`].
    #[serde(default)]
    pub tier_override: Option<Tier>,
    /// Unix seconds of the most recent attempt, successful or not.
    #[serde(default)]
    pub tested_at_unix: i64,
    /// How many proficiency runs this model has completed.
    #[serde(default)]
    pub runs: u32,
    /// The most recent completed run's scores.
    #[serde(default)]
    pub scores: BTreeMap<String, f32>,
    #[serde(default)]
    pub caps: ModelCapabilities,
    /// Why the most recent attempt could not run. Cleared by a run that does.
    #[serde(default)]
    pub last_error: Option<String>,
}

impl Entry {
    fn new(provider: &str, model: &str, endpoint: &str) -> Self {
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
            endpoint: endpoint.to_string(),
            tier_override: None,
            tested_at_unix: 0,
            runs: 0,
            scores: BTreeMap::new(),
            caps: ModelCapabilities::default(),
            last_error: None,
        }
    }

    pub fn tier(&self) -> Tier {
        self.tier_override
            .unwrap_or_else(|| Tier::classify(&self.provider, &self.model))
    }

    pub fn score(&self, key: &str) -> Option<f32> {
        self.scores.get(key).copied()
    }

    /// The headline score. `None` until a run completes — an entry that only
    /// ever failed has no score to show and no rank to claim.
    pub fn overall(&self) -> Option<f32> {
        self.score("overall_score")
    }

    /// Whether this entry can be ranked at all.
    pub fn rated(&self) -> bool {
        self.runs > 0 && self.overall().is_some()
    }

    /// A model that is configured but has never been put through the test.
    ///
    /// Distinct from a model whose test *failed*: one has nothing to report
    /// yet, the other has a reason. Both are unrated, and telling them apart is
    /// the difference between "run the test" and "fix the connection".
    pub fn never_tested(&self) -> bool {
        self.runs == 0 && self.last_error.is_none()
    }

    /// How many distinct invented or unsupported constructs the reviewer
    /// counted in the last run.
    pub fn hallucinations(&self) -> Option<u32> {
        self.score(HALLUCINATION_COUNT_KEY).map(|v| v.max(0.0) as u32)
    }

    /// Display name for the row.
    pub fn label(&self) -> String {
        format!("{} · {}", self.provider, self.model)
    }

    fn matches(&self, provider: &str, model: &str) -> bool {
        self.provider.eq_ignore_ascii_case(provider.trim())
            && self.model.eq_ignore_ascii_case(model.trim())
    }
}

/// Why a model left the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetiredBecause {
    /// The provider stopped offering it: a successful, non-empty catalogue
    /// refresh for that provider no longer lists the model.
    Decommissioned,
    /// The developer pressed Remove.
    Removed,
}

/// A model taken off the board, and kept off it.
///
/// Dropping the row is not enough on its own. `backfill_leaderboard_from_archive`
/// replays every scored report a project ever archived, so a model deleted
/// today is back the next time the board opens — which is why the board had no
/// working Remove at all. The tombstone is what makes a removal stick.
///
/// It is deliberately NOT implemented by editing
/// `agentic_ai/model-benchmarks.jsonl`: that archive is the record of what was
/// actually run and paid for, and deleting evidence to tidy a list is the wrong
/// trade. The scores stay on disk; the board just stops showing them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Retired {
    pub provider: String,
    pub model: String,
    /// Unix seconds, for the hover text on the "retired models" line.
    #[serde(default)]
    pub at_unix: i64,
    pub because: RetiredBecause,
}

impl Retired {
    fn matches(&self, provider: &str, model: &str) -> bool {
        self.provider.eq_ignore_ascii_case(provider.trim())
            && self.model.eq_ignore_ascii_case(model.trim())
    }

    pub fn label(&self) -> String {
        format!("{} · {}", self.provider, self.model)
    }
}

/// The whole board.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Leaderboard {
    #[serde(default)]
    pub entries: Vec<Entry>,
    /// Models that must not come back on their own (see [`Retired`]). `default`
    /// so a board written before this existed still loads.
    #[serde(default)]
    pub retired: Vec<Retired>,
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Leaderboard {
    pub fn path() -> PathBuf {
        crate::llm::base_dir().join("model_leaderboard.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())
    }

    fn slot(&mut self, provider: &str, model: &str, endpoint: &str) -> &mut Entry {
        if let Some(i) = self.entries.iter().position(|e| e.matches(provider, model)) {
            // The endpoint can move (a provider changes host); the identity is
            // provider + model, so follow the move rather than forking a row.
            if !endpoint.trim().is_empty() {
                self.entries[i].endpoint = endpoint.to_string();
            }
            return &mut self.entries[i];
        }
        self.entries.push(Entry::new(provider, model, endpoint));
        self.entries.last_mut().expect("just pushed")
    }

    pub fn get(&self, provider: &str, model: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.matches(provider, model))
    }

    /// Put every configured model on the board, tested or not.
    ///
    /// Without this the board only ever showed models that had already been
    /// through the test, which on a fresh install is none of them: a developer
    /// with eight models configured opened an empty window and had no way to
    /// tell whether that meant "nothing tested yet" or "this is broken". A
    /// model with no result is listed as untested, with its Run tests button
    /// as the obvious next step.
    ///
    /// Returns whether anything was added, so the caller only writes the file
    /// when there is something new in it.
    /// `assigned` is the `(provider, model, endpoint)` list the project's agents
    /// actually run on — [`crate::agents_db::AgentsDb::assigned_models`]
    /// (spec 048 R17). It replaced the model-profile registry, which no longer
    /// exists; listing every model a provider *offers* is not an option, since
    /// one provider can offer several hundred.
    pub fn ensure_models(&mut self, assigned: &[(String, String, String)]) -> bool {
        let mut added = false;
        for (provider, model, endpoint) in assigned {
            if model.trim().is_empty() || provider.trim().is_empty() {
                continue;
            }
            if self.get(provider, model).is_some() {
                continue;
            }
            // A retired model does not come back because an agent still points
            // at it — that agent is exactly what the developer is being asked
            // to re-point. Testing it again is what revives it (`record_*`).
            if self.is_retired(provider, model) {
                continue;
            }
            self.entries.push(Entry::new(provider, model, endpoint));
            added = true;
        }
        added
    }

    /// Is this model tombstoned?
    pub fn is_retired(&self, provider: &str, model: &str) -> bool {
        self.retired.iter().any(|r| r.matches(provider, model))
    }

    /// Drop the tombstone, if any. A model that runs again is alive, whatever
    /// the board decided earlier — so a wrong retirement costs one test, not a
    /// hand-edited JSON file.
    fn revive(&mut self, provider: &str, model: &str) {
        self.retired.retain(|r| !r.matches(provider, model));
    }

    /// Take one model off the board and keep it off. Returns its label when a
    /// row actually went, `None` when there was nothing to remove.
    pub fn retire(
        &mut self,
        provider: &str,
        model: &str,
        because: RetiredBecause,
    ) -> Option<String> {
        let label = self.get(provider, model).map(|e| e.label());
        self.entries.retain(|e| !e.matches(provider, model));
        if !self.is_retired(provider, model) {
            self.retired.push(Retired {
                provider: provider.trim().to_string(),
                model: model.trim().to_string(),
                at_unix: now_unix(),
                because,
            });
        }
        label
    }

    /// Retire every row of `provider` whose model the provider no longer
    /// offers. Returns the labels of what went.
    ///
    /// ⚠️ **An empty `catalogue` retires nothing, ever.** "The provider listed
    /// no models" and "the provider offers no models" are the same sentence
    /// from here and mean opposite things: a failed request, an expired key, a
    /// provider that has not been refreshed yet, all produce an empty list, and
    /// acting on one would delete a whole board's worth of paid-for scores on
    /// the strength of one network call. Only a refresh that came back with
    /// models is evidence about which models exist.
    ///
    /// Scoped to ONE provider for the same reason: a successful refresh of
    /// Anthropic says nothing whatsoever about OpenAI's catalogue.
    pub fn retire_missing(&mut self, provider: &str, catalogue: &[String]) -> Vec<String> {
        if catalogue.is_empty() || provider.trim().is_empty() {
            return Vec::new();
        }
        let gone: Vec<(String, String)> = self
            .entries
            .iter()
            .filter(|e| e.provider.eq_ignore_ascii_case(provider.trim()))
            .filter(|e| {
                !catalogue
                    .iter()
                    .any(|m| m.trim().eq_ignore_ascii_case(&e.model))
            })
            .map(|e| (e.provider.clone(), e.model.clone()))
            .collect();
        gone.iter()
            .filter_map(|(p, m)| self.retire(p, m, RetiredBecause::Decommissioned))
            .collect()
    }

    /// Whether this model has a row yet, whatever its state.
    pub fn contains(&self, provider: &str, model: &str) -> bool {
        self.get(provider, model).is_some()
    }

    /// Housekeeping: drop the rows that are neither used nor tested. Returns
    /// the labels of what went, so the caller can put the removals on record.
    ///
    /// **A row with runs on it is never removed (spec 048 R19).** This is the
    /// important half of the rule and it supersedes 1.61.6, which pruned by
    /// registry membership alone and could therefore delete a model's entire
    /// score history because an agent had moved on. A score costs real tokens
    /// and real time; an empty row costs nothing to recreate. Only the empty
    /// ones are swept.
    ///
    /// ⚠️ **Scope.** The board is machine-wide; `assigned` belongs to the OPEN
    /// PROJECT. A model used only by some other project counts as unassigned
    /// here — but since it can only be pruned when it has never been tested,
    /// the consequence is now trivial rather than destructive.
    ///
    /// An EMPTY `assigned` prunes nothing. No project loaded is not the same as
    /// a project that uses nothing.
    pub fn prune_untested_orphans(&mut self, assigned: &[(String, String, String)]) -> Vec<String> {
        if assigned.is_empty() {
            return Vec::new();
        }
        let mut removed = Vec::new();
        self.entries.retain(|e| {
            // Tested at least once ⇒ history, kept regardless of assignment.
            if e.runs > 0 {
                return true;
            }
            let in_use = assigned
                .iter()
                .any(|(provider, model, _)| e.matches(provider, model));
            if !in_use {
                removed.push(e.label());
            }
            in_use
        });
        removed
    }

    /// Record a run that finished and produced scores.
    pub fn record_success(
        &mut self,
        provider: &str,
        model: &str,
        endpoint: &str,
        outcome: RunOutcome,
    ) {
        // It answered, so it exists — whatever the catalogue said when it was
        // retired. Reviving here is what keeps a wrong retirement cheap.
        self.revive(provider, model);
        let e = self.slot(provider, model, endpoint);
        e.runs += 1;
        e.tested_at_unix = now_unix();
        e.last_error = None;

        // Rebuilt from this run, not merged into the last one: a stale score
        // from an older run of a model that has since changed is not evidence.
        e.scores.clear();
        for key in SCORE_KEYS
            .iter()
            .chain(std::iter::once(&HALLUCINATION_COUNT_KEY))
        {
            if let Some(v) = outcome.metrics.get(*key).and_then(|v| v.as_f64()) {
                e.scores.insert((*key).to_string(), v as f32);
            }
        }
    }

    /// Record a run that could not be carried out. The model keeps whatever it
    /// scored previously; only its error changes.
    pub fn record_failure(&mut self, provider: &str, model: &str, endpoint: &str, error: &str) {
        // A FAILED run is not evidence the model exists — the failure may be
        // "no such model". It does mean the developer deliberately aimed a test
        // at it, though, so the row comes back carrying the reason rather than
        // vanishing silently.
        self.revive(provider, model);
        let e = self.slot(provider, model, endpoint);
        e.tested_at_unix = now_unix();
        e.last_error = Some(error.trim().to_string());
    }

    /// Fold in what the connection probe found. Creates the row if the model
    /// has never been tested, so a probed-but-untested model is still visible.
    pub fn apply_capabilities(
        &mut self,
        provider: &str,
        model: &str,
        endpoint: &str,
        caps: ModelCapabilities,
    ) {
        if caps.is_empty() {
            return;
        }
        self.slot(provider, model, endpoint).caps.merge(caps);
    }

    pub fn set_tier_override(&mut self, provider: &str, model: &str, tier: Option<Tier>) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.matches(provider, model)) {
            e.tier_override = tier;
        }
    }

    /// Drop a row without tombstoning it — it may be recreated by the next
    /// sync. Use [`Self::retire`] for a removal that is meant to stick.
    pub fn remove(&mut self, provider: &str, model: &str) {
        self.entries.retain(|e| !e.matches(provider, model));
    }

    /// Indices into `entries` on `board`, best first. Unrated rows (a test that
    /// never completed) sort last, in name order, because they have no score to
    /// be ordered by.
    pub fn ranked(&self, board: Board) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.entries.len())
            .filter(|i| board.accepts(self.entries[*i].tier()))
            .collect();
        idx.sort_by(|a, b| {
            let (ea, eb) = (&self.entries[*a], &self.entries[*b]);
            eb.rated()
                .cmp(&ea.rated())
                .then_with(|| {
                    eb.overall()
                        .unwrap_or(0.0)
                        .partial_cmp(&ea.overall().unwrap_or(0.0))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| ea.model.cmp(&eb.model))
        });
        idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(overall: f32) -> serde_json::Value {
        serde_json::json!({
            "overall_score": overall,
            "compilation_score": 80.0,
            "debugging_score": 77.0,
            "hallucination_count": 3,
        })
    }

    fn outcome(overall: f32) -> RunOutcome {
        RunOutcome {
            metrics: metrics(overall),
        }
    }

    /// **A failed or unlisted refresh retires nothing.** This is the guard that
    /// makes automatic retirement safe to have at all: an empty catalogue is
    /// what a network error, an expired key and a provider nobody has refreshed
    /// yet all look like from here, and acting on it would delete a whole
    /// board's worth of paid-for scores on one bad request.
    #[test]
    fn an_empty_catalogue_never_retires_anything() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "claude-opus-5", "", outcome(90.0));
        lb.record_success("anthropic", "claude-sonnet-5", "", outcome(80.0));

        assert!(
            lb.retire_missing("anthropic", &[]).is_empty(),
            "an empty listing is not evidence about which models exist"
        );
        assert_eq!(lb.entries.len(), 2, "and nothing was taken off the board");
        assert!(lb.retired.is_empty(), "nor tombstoned");
    }

    /// A refresh of ONE provider says nothing about another's catalogue.
    #[test]
    fn retiring_is_scoped_to_the_provider_that_answered() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "claude-opus-5", "", outcome(90.0));
        lb.record_success("openai", "gpt-9", "", outcome(70.0));

        let gone = lb.retire_missing("anthropic", &["claude-opus-5".into()]);
        assert!(gone.is_empty(), "the listed model stays: {gone:?}");
        assert!(
            lb.get("openai", "gpt-9").is_some(),
            "another provider's model is untouched by Anthropic's catalogue"
        );
    }

    /// The operator's case: a provider decommissions a model, so it goes — and
    /// a score is not a reason to keep a model that no longer exists (which is
    /// what separates this from `prune_untested_orphans`, where runs protect a
    /// row absolutely).
    #[test]
    fn a_model_the_provider_dropped_is_retired_even_with_scores() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "claude-opus-4", "", outcome(88.0));
        lb.record_success("anthropic", "claude-opus-5", "", outcome(93.0));

        let gone = lb.retire_missing("anthropic", &["claude-opus-5".into()]);
        assert_eq!(gone.len(), 1, "exactly the decommissioned one: {gone:?}");
        assert!(gone[0].contains("claude-opus-4"), "named: {gone:?}");
        assert!(lb.get("anthropic", "claude-opus-4").is_none());
        assert!(lb.get("anthropic", "claude-opus-5").is_some());
    }

    /// **A retirement sticks.** The board is re-synced from the agents on every
    /// startup and project open, so without a tombstone the very next sync puts
    /// a still-assigned model straight back and the removal is theatre.
    #[test]
    fn a_retired_model_is_not_re_added_by_the_next_sync() {
        let mut lb = Leaderboard::default();
        let assigned = vec![(
            "anthropic".to_string(),
            "claude-opus-4".to_string(),
            String::new(),
        )];
        lb.ensure_models(&assigned);
        assert!(lb.get("anthropic", "claude-opus-4").is_some());

        lb.retire("anthropic", "claude-opus-4", RetiredBecause::Removed);
        assert!(lb.is_retired("anthropic", "claude-opus-4"));

        assert!(
            !lb.ensure_models(&assigned),
            "the sync must not resurrect a retired model"
        );
        assert!(lb.get("anthropic", "claude-opus-4").is_none());
    }

    /// …and it is not a life sentence. A model that answers again is alive,
    /// whatever the catalogue said — so a wrong retirement costs one test run,
    /// not a hand-edited JSON file.
    #[test]
    fn testing_a_retired_model_brings_it_back() {
        let mut lb = Leaderboard::default();
        lb.retire("anthropic", "claude-opus-4", RetiredBecause::Decommissioned);
        assert!(lb.is_retired("anthropic", "claude-opus-4"));

        lb.record_success("anthropic", "claude-opus-4", "", outcome(85.0));
        assert!(
            !lb.is_retired("anthropic", "claude-opus-4"),
            "a live run revives it"
        );
        assert!(lb.get("anthropic", "claude-opus-4").is_some());
    }

    /// A board written before retirement existed still loads, with no
    /// tombstones — `#[serde(default)]` on the new field.
    #[test]
    fn a_board_from_before_this_feature_still_loads() {
        let old = r#"{"entries":[{"provider":"anthropic","model":"claude-opus-5",
                       "endpoint":"","runs":1,"scores":{},"tested_at_unix":0}]}"#;
        let lb: Leaderboard = serde_json::from_str(old).expect("older board parses");
        assert_eq!(lb.entries.len(), 1);
        assert!(lb.retired.is_empty());
    }

    #[test]
    fn tier_classification_follows_provider_then_model() {
        assert_eq!(Tier::classify("ollama", "qwen2.5-coder:32b"), Tier::Local);
        assert_eq!(
            Tier::classify("ollama_cloud", "gemma4:31b"),
            Tier::CloudPaid,
            "Ollama's hosted service runs nothing on this machine and bills for it"
        );
        assert_eq!(
            Tier::classify("openrouter", "qwen3-coder:free"),
            Tier::CloudFree
        );
        assert_eq!(
            Tier::classify("anthropic", "claude-opus-5"),
            Tier::CloudPaid,
            "an unknown route must be assumed to bill"
        );
    }

    /// **A score is history and is never swept (spec 048 R19).**
    ///
    /// This supersedes the 1.61.6 rule, which pruned on registry membership
    /// alone and would therefore delete a model's entire score history the
    /// moment the last agent moved off it. A result costs real tokens and real
    /// time; an empty row costs nothing to recreate. So runs > 0 is kept,
    /// whatever the current assignments say.
    #[test]
    fn a_row_with_runs_is_never_pruned() {
        let mut lb = Leaderboard::default();
        lb.record_success("openai", "retired-model", "https://y", outcome(71.0));
        assert_eq!(lb.get("openai", "retired-model").unwrap().runs, 1);

        // Nothing is assigned to it any more, and it still stays.
        let removed = lb.prune_untested_orphans(&[assigned("anthropic", "claude-opus-5")]);

        assert!(
            removed.is_empty(),
            "a scored row was pruned: {removed:?} — 1.61.6's rule is back"
        );
        assert!(lb.get("openai", "retired-model").is_some());
        println!("scored row survived reassignment: openai/retired-model, 1 run kept");
    }

    /// The other half: a row nobody uses and nobody ever tested is noise, and
    /// goes — named on the way out (spec 048 R18).
    #[test]
    fn an_untested_row_no_agent_uses_is_pruned_and_named() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "claude-opus-5", "https://x", outcome(90.0));
        lb.ensure_models(&[
            assigned("ollama", "still-assigned"),
            assigned("openai", "dropped-and-untested"),
        ]);
        assert_eq!(lb.entries.len(), 3);

        let removed = lb.prune_untested_orphans(&[
            assigned("anthropic", "claude-opus-5"),
            assigned("ollama", "still-assigned"),
        ]);

        assert_eq!(removed.len(), 1, "exactly the untested orphan goes");
        assert!(
            removed[0].contains("dropped-and-untested"),
            "the removal must name the model: {removed:?}"
        );
        assert!(lb.get("ollama", "still-assigned").is_some());
        assert!(lb.get("anthropic", "claude-opus-5").is_some());
        println!("pruned untested orphans: {}", removed.join(", "));
    }

    /// The models the project's agents run on get a row, so the board is never
    /// a shorter list than the agents table (spec 048 R17).
    #[test]
    fn assigned_models_populate_the_board() {
        let mut lb = Leaderboard::default();
        let added = lb.ensure_models(&[
            assigned("anthropic", "claude-sonnet-5"),
            assigned("ollama", "gemma4:31b"),
        ]);
        assert!(added);
        assert_eq!(lb.entries.len(), 2);
        assert!(lb.get("anthropic", "claude-sonnet-5").is_some());
        assert!(lb.get("ollama", "gemma4:31b").is_some());

        // Idempotent: opening the board again adds nothing.
        assert!(!lb.ensure_models(&[assigned("anthropic", "claude-sonnet-5")]));
        assert_eq!(lb.entries.len(), 2);
        println!("board seeded from 2 assigned models; reopening added none");
    }

    /// **The guard that stops one startup erasing the board.**
    ///
    /// The board is machine-wide and the assignment list is the open project's,
    /// so an EMPTY list means "not loaded yet", never "everything is an
    /// orphan".
    #[test]
    fn an_empty_assignment_list_prunes_nothing() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "claude-opus-5", "", outcome(90.0));
        lb.ensure_models(&[assigned("openai", "untested")]);

        assert!(
            lb.prune_untested_orphans(&[]).is_empty(),
            "it reported removals"
        );
        assert_eq!(lb.entries.len(), 2, "an empty list emptied the board");
    }

    /// Assignment matches the way every other lookup on the board does —
    /// case-insensitively, ignoring surrounding space — so a model that differs
    /// only in spelling is not mistaken for an orphan.
    #[test]
    fn assignment_matching_ignores_case_and_padding() {
        let mut lb = Leaderboard::default();
        lb.ensure_models(&[assigned("Anthropic", "Claude-Opus-5")]);
        let removed = lb.prune_untested_orphans(&[assigned(" anthropic ", " claude-opus-5 ")]);
        assert!(removed.is_empty(), "a spelling difference read as an orphan");
        assert_eq!(lb.entries.len(), 1);
    }

    #[test]
    fn a_completed_run_is_rated_and_counted() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "claude-opus-5", "https://x", outcome(90.0));
        let e = lb.get("anthropic", "claude-opus-5").unwrap();
        assert_eq!(e.overall(), Some(90.0));
        assert_eq!(e.runs, 1);
        assert!(e.rated());
    }

    #[test]
    fn a_later_run_replaces_the_earlier_scores() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "m", "", outcome(90.0));
        lb.record_success("anthropic", "m", "", outcome(86.0));
        let e = lb.get("anthropic", "m").unwrap();
        assert_eq!(e.overall(), Some(86.0), "the newest run is the standing one");
        assert_eq!(e.runs, 2);
    }

    #[test]
    fn a_failed_run_keeps_the_old_score_and_shows_the_error() {
        let mut lb = Leaderboard::default();
        lb.record_success("openrouter", "free-model", "", outcome(70.0));
        lb.record_failure("openrouter", "free-model", "", "HTTP 429");
        let e = lb.get("openrouter", "free-model").unwrap();
        assert_eq!(e.overall(), Some(70.0));
        assert_eq!(e.runs, 1, "a run that never ran is not a run");
        assert_eq!(e.last_error.as_deref(), Some("HTTP 429"));
    }

    #[test]
    fn a_model_that_never_completed_is_unrated_and_ranks_last() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "good", "", outcome(88.0));
        lb.record_failure("anthropic", "broken", "", "connection refused");
        let order = lb.ranked(Board::Overall);
        assert_eq!(lb.entries[order[0]].model, "good");
        assert_eq!(lb.entries[order[1]].model, "broken");
        assert!(!lb.entries[order[1]].rated());
        assert_eq!(lb.entries[order[1]].overall(), None);
    }

    #[test]
    fn boards_partition_by_tier() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "paid", "", outcome(90.0));
        lb.record_success("openrouter", "x:free", "", outcome(70.0));
        lb.record_success("ollama", "local-one", "", outcome(60.0));
        assert_eq!(lb.ranked(Board::Overall).len(), 3);
        assert_eq!(lb.ranked(Board::CloudPaid).len(), 1);
        assert_eq!(lb.ranked(Board::CloudFree).len(), 1);
        assert_eq!(lb.ranked(Board::Local).len(), 1);
    }

    #[test]
    fn missing_metrics_stay_missing_rather_than_zero() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "m", "", outcome(90.0));
        let e = lb.get("anthropic", "m").unwrap();
        assert_eq!(e.score("debugging_score"), Some(77.0));
        assert_eq!(
            e.score("refactoring_score"),
            None,
            "a key the model never returned must not read as 0%"
        );
    }

    #[test]
    fn the_hallucination_count_is_a_count_not_a_percentage() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "m", "", outcome(90.0));
        assert_eq!(lb.get("anthropic", "m").unwrap().hallucinations(), Some(3));
    }

    #[test]
    fn capabilities_merge_without_erasing_what_is_known() {
        let mut lb = Leaderboard::default();
        lb.apply_capabilities(
            "openrouter",
            "m",
            "",
            ModelCapabilities {
                ctx_in: Some(128_000),
                ctx_out: Some(8_192),
                ..Default::default()
            },
        );
        lb.apply_capabilities(
            "openrouter",
            "m",
            "",
            ModelCapabilities {
                params_b: Some(24.0),
                ..Default::default()
            },
        );
        let caps = &lb.get("openrouter", "m").unwrap().caps;
        assert_eq!(caps.ctx_in, Some(128_000));
        assert_eq!(caps.params_b, Some(24.0));
    }

    /// One entry of the "models the project's agents run on" list that replaced
    /// the model-profile registry (spec 048 R17).
    fn assigned(provider: &str, model: &str) -> (String, String, String) {
        (
            provider.to_string(),
            model.to_string(),
            "https://example".to_string(),
        )
    }

    #[test]
    fn every_configured_model_is_listed_even_when_never_tested() {
        let mut lb = Leaderboard::default();
        assert!(lb.ensure_models(&[
            assigned("ollama", "qwen2.5-coder:32b"),
            assigned("anthropic", "claude-opus-5"),
        ]));
        assert_eq!(
            lb.ranked(Board::Overall).len(),
            2,
            "a configured model must appear before it is ever tested"
        );
        let e = lb.get("ollama", "qwen2.5-coder:32b").unwrap();
        assert!(e.never_tested());
        assert!(!e.rated());
        assert_eq!(e.overall(), None);
    }

    #[test]
    fn listing_models_is_idempotent_and_keeps_results() {
        let mut lb = Leaderboard::default();
        lb.record_success("ollama", "tested", "", outcome(84.0));
        let models = [assigned("ollama", "tested"), assigned("ollama", "fresh")];
        assert!(lb.ensure_models(&models));
        assert!(
            !lb.ensure_models(&models),
            "a second pass must report no change so the file is not rewritten"
        );
        assert_eq!(lb.entries.len(), 2);
        assert_eq!(
            lb.get("ollama", "tested").unwrap().overall(),
            Some(84.0),
            "listing must not wipe a result the model already earned"
        );
    }

    #[test]
    fn an_assignment_with_no_model_id_is_not_listed() {
        let mut lb = Leaderboard::default();
        assert!(!lb.ensure_models(&[assigned("ollama", "   ")]));
        assert!(lb.entries.is_empty());
    }

    #[test]
    fn never_tested_and_failed_are_different_states() {
        let mut lb = Leaderboard::default();
        lb.ensure_models(&[assigned("ollama", "fresh")]);
        lb.record_failure("ollama", "broken", "", "connection refused");
        assert!(lb.get("ollama", "fresh").unwrap().never_tested());
        assert!(!lb.get("ollama", "broken").unwrap().never_tested());
    }

    #[test]
    fn untested_models_rank_below_tested_ones() {
        let mut lb = Leaderboard::default();
        lb.ensure_models(&[assigned("ollama", "fresh")]);
        lb.record_success("anthropic", "scored", "", outcome(60.0));
        let order = lb.ranked(Board::Overall);
        assert_eq!(lb.entries[order[0]].model, "scored");
        assert_eq!(lb.entries[order[1]].model, "fresh");
    }

    #[test]
    fn a_tier_override_beats_the_classifier() {
        let mut lb = Leaderboard::default();
        lb.record_success("openrouter", "mystery", "", outcome(80.0));
        assert_eq!(lb.get("openrouter", "mystery").unwrap().tier(), Tier::CloudPaid);
        lb.set_tier_override("openrouter", "mystery", Some(Tier::CloudFree));
        assert_eq!(lb.get("openrouter", "mystery").unwrap().tier(), Tier::CloudFree);
    }
}
