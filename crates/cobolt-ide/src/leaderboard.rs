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
    /// Ollama (local or its cloud relay) runs the weights on hardware the
    /// developer controls. OpenRouter marks its no-charge routes with a
    /// `:free` model suffix, and HuggingFace's router serves its free tier the
    /// same way. Everything else is assumed to bill, because assuming a paid
    /// model is free is the mistake that costs money — the developer can
    /// override it per entry.
    pub fn classify(provider: &str, model: &str) -> Self {
        let p = provider.trim().to_ascii_lowercase();
        if p.starts_with("ollama") || p == "local" || p == "llamacpp" {
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

/// The whole board.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Leaderboard {
    #[serde(default)]
    pub entries: Vec<Entry>,
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
    pub fn ensure_models(&mut self, profiles: &[crate::llm::ModelProfile]) -> bool {
        let mut added = false;
        for p in profiles {
            if p.model.trim().is_empty() || p.provider.trim().is_empty() {
                continue;
            }
            if self.get(&p.provider, &p.model).is_some() {
                continue;
            }
            self.entries
                .push(Entry::new(&p.provider, &p.model, &p.endpoint));
            added = true;
        }
        added
    }

    /// Whether this model has a row yet, whatever its state.
    pub fn contains(&self, provider: &str, model: &str) -> bool {
        self.get(provider, model).is_some()
    }

    /// Record a run that finished and produced scores.
    pub fn record_success(
        &mut self,
        provider: &str,
        model: &str,
        endpoint: &str,
        outcome: RunOutcome,
    ) {
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

    #[test]
    fn tier_classification_follows_provider_then_model() {
        assert_eq!(Tier::classify("ollama", "qwen2.5-coder:32b"), Tier::Local);
        assert_eq!(
            Tier::classify("ollama_cloud", "deepseek-v3"),
            Tier::Local,
            "the cloud relay still runs a model the developer manages"
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

    fn profile(provider: &str, model: &str) -> crate::llm::ModelProfile {
        crate::llm::ModelProfile {
            id: format!("{provider}-{model}"),
            name: format!("{provider} · {model}"),
            provider: provider.to_string(),
            endpoint: "https://example".to_string(),
            endpoint_user_edited: false,
            model: model.to_string(),
            temperature: 0.7,
            max_tokens: 8192,
            timeout_secs: 30,
        }
    }

    #[test]
    fn every_configured_model_is_listed_even_when_never_tested() {
        let mut lb = Leaderboard::default();
        assert!(lb.ensure_models(&[
            profile("ollama", "qwen2.5-coder:32b"),
            profile("anthropic", "claude-opus-5"),
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
        let profiles = [profile("ollama", "tested"), profile("ollama", "fresh")];
        assert!(lb.ensure_models(&profiles));
        assert!(
            !lb.ensure_models(&profiles),
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
    fn a_profile_with_no_model_id_is_not_listed() {
        let mut lb = Leaderboard::default();
        assert!(!lb.ensure_models(&[profile("ollama", "   ")]));
        assert!(lb.entries.is_empty());
    }

    #[test]
    fn never_tested_and_failed_are_different_states() {
        let mut lb = Leaderboard::default();
        lb.ensure_models(&[profile("ollama", "fresh")]);
        lb.record_failure("ollama", "broken", "", "connection refused");
        assert!(lb.get("ollama", "fresh").unwrap().never_tested());
        assert!(!lb.get("ollama", "broken").unwrap().never_tested());
    }

    #[test]
    fn untested_models_rank_below_tested_ones() {
        let mut lb = Leaderboard::default();
        lb.ensure_models(&[profile("ollama", "fresh")]);
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
