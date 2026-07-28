// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Per-model behavior policies.
//!
//! Specific provider/model combinations have mapped quirks that no amount of
//! generic retrying fixes — a provider that rejects function tools together
//! with reasoning effort, a hidden-reasoning model that exhausts small token
//! budgets and returns empty content. Rather than hardcoding workarounds in
//! call sites, each mapped problem becomes a POLICY RULE: matched by
//! case-insensitive substrings of the provider and model ids, merged
//! field-wise, and applied automatically when the host builds a model call.
//!
//! Built-in rules capture the problems mapped so far. The operator extends
//! the map without recompiling by adding rules to `model_policies.json` in
//! the Cobolt data directory (same folder as `llm_config.json`):
//!
//! ```json
//! [
//!   {"provider": "ollama", "model": "gemma", "policy": {
//!       "avoid_native_tools": true,
//!       "note": "emits empty responses when tool definitions are attached"}}
//! ]
//! ```

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Workarounds the host applies when building calls for a matched model.
/// Every field defaults to "no adjustment", so an empty policy is a no-op.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelPolicy {
    /// Never attach native function-tool definitions; the fenced protocol in
    /// the system prompt keeps every host tool reachable as text.
    #[serde(default)]
    pub avoid_native_tools: bool,
    /// Raise the output-token budget to at least this value. Hidden-reasoning
    /// models can burn a small budget thinking and return EMPTY final content.
    #[serde(default)]
    pub min_max_tokens: u32,
    /// Skip provider-native typed extraction (deterministic parsing only).
    /// Mapped case: extraction sends function tools plus reasoning effort,
    /// which some providers reject with HTTP 400.
    #[serde(default)]
    pub avoid_typed_extraction: bool,
    /// Human-readable reason, surfaced in the verbose AI log when applied.
    #[serde(default)]
    pub note: String,
}

impl ModelPolicy {
    pub fn is_noop(&self) -> bool {
        !self.avoid_native_tools && self.min_max_tokens == 0 && !self.avoid_typed_extraction
    }

    /// Field-wise merge: restrictions accumulate across every matching rule.
    fn absorb(&mut self, other: &ModelPolicy) {
        self.avoid_native_tools |= other.avoid_native_tools;
        self.min_max_tokens = self.min_max_tokens.max(other.min_max_tokens);
        self.avoid_typed_extraction |= other.avoid_typed_extraction;
        if !other.note.trim().is_empty() {
            if !self.note.is_empty() {
                self.note.push_str("; ");
            }
            self.note.push_str(other.note.trim());
        }
    }
}

/// One mapped problem: which models it affects and what to do about it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelPolicyRule {
    /// Case-insensitive substring of the provider id; empty matches any.
    #[serde(default)]
    pub provider: String,
    /// Case-insensitive substring of the model id; empty matches any.
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub policy: ModelPolicy,
}

impl ModelPolicyRule {
    fn matches(&self, provider: &str, model: &str) -> bool {
        let matches_part = |needle: &str, hay: &str| {
            needle.trim().is_empty() || hay.to_ascii_lowercase().contains(&needle.trim().to_ascii_lowercase())
        };
        matches_part(&self.provider, provider) && matches_part(&self.model, model)
    }
}

/// The problems mapped so far, as shipped defaults.
pub fn builtin_rules() -> Vec<ModelPolicyRule> {
    vec![
        // Observed live: /v1/chat/completions rejects "function tools with
        // reasoning_effort" for this family — every typed extraction burned a
        // failing HTTP 400 call.
        ModelPolicyRule {
            provider: "openai".into(),
            model: "gpt-5.6".into(),
            policy: ModelPolicy {
                avoid_typed_extraction: true,
                note: "provider rejects function tools with reasoning_effort on chat/completions"
                    .into(),
                ..Default::default()
            },
        },
        // Observed live: ever-longer generations (6s→28s) returning empty
        // final content on large task prompts — the signature of a hidden-
        // reasoning model exhausting its output budget. Also observed live
        // (Grace T2/T3 run, 3-for-3): every first attempt with native tool
        // definitions attached returned empty when the task's real tools use
        // the fenced protocol; the same request succeeded immediately once the
        // native tools were detached, so gemma never gets them.
        ModelPolicyRule {
            provider: "ollama".into(),
            model: "gemma".into(),
            policy: ModelPolicy {
                avoid_native_tools: true,
                min_max_tokens: 16_384,
                note: "returns empty when native tool definitions accompany fenced-protocol \
                       tasks; hidden reasoning can exhaust small token budgets"
                    .into(),
                ..Default::default()
            },
        },
    ]
}

/// Budget floor for the reasoning-first Anthropic generation, DERIVED rather
/// than enumerated.
///
/// Observed live (Form Designer Agent, 30-control task): the first response
/// carried ONE reasoning block, zero rendered characters, and `stop_reason:
/// max_tokens` — the model spent the whole 8192-token budget thinking and the
/// answer never started.
///
/// The discriminator is the SAME allowlist that withholds `temperature`
/// (`rig_transport::sends_sampling_params`): Anthropic removed the sampling
/// parameters and turned extended thinking on in the same generation, so the
/// models outside that allowlist are exactly the ones that think before they
/// answer. Deriving from it keeps both mechanisms aging in the same fail-safe
/// direction — an Anthropic model this build has never heard of gets the floor
/// on release day, while Claude 3.x/2.x (whose output ceiling is BELOW the
/// floor, where asking for it is an HTTP 400) stay inside the allowlist and
/// are never floored.
fn reasoning_first_floor(provider: &str, model: &str) -> Option<ModelPolicy> {
    if cobolt_agents::rig_transport::sends_sampling_params(provider, model) {
        return None;
    }
    Some(ModelPolicy {
        min_max_tokens: 32_768,
        note: "extended thinking is on by default; a small budget is spent reasoning and the \
               reply arrives empty"
            .into(),
        ..Default::default()
    })
}

/// Operator-extensible rules: `model_policies.json` in the Cobolt data dir.
/// A missing or unparseable file contributes nothing (and parse errors land
/// in the connection log once per load rather than failing calls).
fn operator_rules_at(path: &Path) -> Vec<ModelPolicyRule> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<ModelPolicyRule>>(&text) {
        Ok(rules) => rules,
        Err(error) => {
            crate::llm::push_connection_log(&format!(
                "model_policies.json ignored (parse error): {error}\n"
            ));
            Vec::new()
        }
    }
}

/// Resolve the effective policy for a provider/model: every matching rule —
/// built-in first, then operator rules — merged field-wise, then the derived
/// reasoning-first floor absorbed on top.
pub fn policy_for(provider: &str, model: &str) -> ModelPolicy {
    let path = crate::llm::base_dir().join("model_policies.json");
    let mut rules = builtin_rules();
    rules.extend(operator_rules_at(&path));
    effective_policy(&rules, provider, model)
}

/// The pure composition `policy_for` applies: matching rules merged, plus the
/// derived floor. Separate so tests can exercise it without the operator file.
fn effective_policy(rules: &[ModelPolicyRule], provider: &str, model: &str) -> ModelPolicy {
    let mut merged = policy_from_rules(rules, provider, model);
    if let Some(floor) = reasoning_first_floor(provider, model) {
        merged.absorb(&floor);
    }
    merged
}

pub fn policy_from_rules(
    rules: &[ModelPolicyRule],
    provider: &str,
    model: &str,
) -> ModelPolicy {
    let mut merged = ModelPolicy::default();
    for rule in rules.iter().filter(|rule| rule.matches(provider, model)) {
        merged.absorb(&rule.policy);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_rules_cover_the_mapped_problems() {
        // gpt-5.6-terra: typed extraction is disabled (HTTP 400 mapped live).
        let terra = policy_from_rules(&builtin_rules(), "openai", "gpt-5.6-terra");
        assert!(terra.avoid_typed_extraction);
        assert!(!terra.avoid_native_tools);

        // gemma via ollama cloud: token budget floor + no native tool
        // definitions (both empty-response mappings from live runs).
        let gemma = policy_from_rules(&builtin_rules(), "ollama_cloud", "gemma4:31b");
        assert!(gemma.min_max_tokens >= 16_384);
        assert!(gemma.avoid_native_tools);
        assert!(!gemma.avoid_typed_extraction);

        // Unmapped models get a no-op policy.
        let clean = policy_from_rules(&builtin_rules(), "mistral", "mistral-large");
        assert!(clean.is_noop());
    }

    /// The reasoning-first floor is DERIVED from the sampling allowlist, not
    /// enumerated: it must cover today's thinking generation, every future
    /// Anthropic model by default (fail-safe direction), and never the older
    /// line whose output ceiling sits below the floor (32k there is an HTTP
    /// 400, not a bigger budget).
    #[test]
    fn reasoning_first_floor_is_derived_from_the_sampling_allowlist() {
        // Today's thinking generation: floored, tools and extraction intact.
        for model in ["claude-opus-5", "claude-sonnet-5", "claude-fable-5", "claude-opus-4-7"] {
            let policy = effective_policy(&builtin_rules(), "anthropic", model);
            assert_eq!(policy.min_max_tokens, 32_768, "{model} gets the floor");
            assert!(!policy.avoid_native_tools);
            assert!(!policy.avoid_typed_extraction);
        }

        // The point of deriving: a model this build has never heard of is
        // covered on release day, with no list to edit.
        let future = effective_policy(&builtin_rules(), "anthropic", "claude-haiku-5");
        assert_eq!(future.min_max_tokens, 32_768, "unknown Anthropic models are assumed new");

        // The allowlisted line (still accepts temperature, no default
        // thinking) is never floored — including via gateway-prefixed ids.
        for model in ["claude-3-5-sonnet-latest", "claude-sonnet-4-5", "claude-haiku-4-5", "anthropic.claude-3-haiku"] {
            assert!(
                effective_policy(&builtin_rules(), "anthropic", model).is_noop(),
                "{model} must not be floored"
            );
        }

        // Non-Anthropic providers are untouched by the floor.
        assert!(effective_policy(&builtin_rules(), "mistral", "mistral-large").is_noop());
        assert!(reasoning_first_floor("openai", "gpt-5.6-terra").is_none());

        // The floor merges with rules like any other policy: operator rules
        // still accumulate on top and the largest floor wins.
        let rules = vec![ModelPolicyRule {
            provider: "anthropic".into(),
            model: "claude-sonnet-5".into(),
            policy: ModelPolicy {
                avoid_typed_extraction: true,
                ..Default::default()
            },
        }];
        let merged = effective_policy(&rules, "anthropic", "claude-sonnet-5");
        assert_eq!(merged.min_max_tokens, 32_768);
        assert!(merged.avoid_typed_extraction);
    }

    #[test]
    fn matching_is_case_insensitive_substring_and_merge_accumulates() {
        let rules = vec![
            ModelPolicyRule {
                provider: "Ollama".into(),
                model: "GEMMA".into(),
                policy: ModelPolicy {
                    min_max_tokens: 16_384,
                    ..Default::default()
                },
            },
            ModelPolicyRule {
                provider: String::new(), // any provider
                model: "gemma4".into(),
                policy: ModelPolicy {
                    avoid_native_tools: true,
                    min_max_tokens: 8_192,
                    note: "operator-mapped".into(),
                    ..Default::default()
                },
            },
        ];
        let merged = policy_from_rules(&rules, "ollama_cloud", "gemma4:31b");
        assert!(merged.avoid_native_tools, "restrictions accumulate");
        assert_eq!(merged.min_max_tokens, 16_384, "largest floor wins");
        assert!(merged.note.contains("operator-mapped"));
        // Non-matching model is untouched by both rules.
        assert!(policy_from_rules(&rules, "ollama_cloud", "llama3").is_noop());
    }

    #[test]
    fn operator_rules_extend_the_builtin_map() {
        let dir = std::env::temp_dir().join(format!(
            "prc-model-policy-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("t").len()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("model_policies.json");
        std::fs::write(
            &path,
            r#"[{"provider":"ollama","model":"gemma","policy":{"avoid_native_tools":true,"note":"mapped by operator"}}]"#,
        )
        .unwrap();
        let mut rules = builtin_rules();
        rules.extend(operator_rules_at(&path));
        let merged = policy_from_rules(&rules, "ollama_cloud", "gemma4:31b");
        assert!(merged.avoid_native_tools, "operator rule applies");
        assert!(merged.min_max_tokens >= 16_384, "built-in rule still applies");
        // Garbage files contribute nothing instead of failing.
        std::fs::write(&path, "not json").unwrap();
        assert!(operator_rules_at(&path).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
