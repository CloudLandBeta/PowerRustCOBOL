// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The built-in semantic embedder (spec 068 R22) — `multilingual-e5-small`
//! running inside the application. Ported from the IDE's `bert_embedder`:
//! same model, prefixes, pooling and device choice (Metal on macOS, otherwise a
//! low-power CPU), returning errors instead of logging them.
//!
//! Only in a build with the `semantic` feature, which links candle and
//! tokenizers — and tokenizers' `onig` regex engine compiles C.

use std::path::Path;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use tokenizers::Tokenizer;

use crate::embed::{normalize, Embedder};
use crate::model::{model_dir, BUILTIN_STAMP};


const MAX_TOKENS: usize = 256;
const PASSAGE_PREFIX: &str = "passage: ";
const QUERY_PREFIX: &str = "query: ";
const CPU_LOW_POWER_THREADS: &str = "2";

/// The model, loaded.
pub struct BuiltinEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

fn pick_device() -> Device {
    let forced = std::env::var("PRC_EMBED_DEVICE")
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default();
    #[cfg(target_os = "macos")]
    if forced.is_empty() || forced == "metal" {
        if let Ok(d) = Device::new_metal(0) {
            return d;
        }
    }
    let _ = forced;
    if std::env::var("RAYON_NUM_THREADS").map(|v| v.trim().is_empty()).unwrap_or(true) {
        std::env::set_var("RAYON_NUM_THREADS", CPU_LOW_POWER_THREADS);
    }
    Device::Cpu
}

impl BuiltinEmbedder {
    /// Load the model from `models_dir` (`<app>/assets/models`). Never fetches.
    pub fn load(models_dir: &Path) -> Result<Self, String> {
        let dir = model_dir(models_dir);
        let (config_path, tokenizer_path, weights_path) = (
            dir.join("config.json"),
            dir.join("tokenizer.json"),
            dir.join("model.safetensors"),
        );
        if !(config_path.is_file() && tokenizer_path.is_file() && weights_path.is_file()) {
            return Err(format!(
                "the built-in model is not in {} yet; search is lexical until it is fetched",
                dir.display()
            ));
        }
        let config: Config = serde_json::from_str(
            &std::fs::read_to_string(&config_path)
                .map_err(|e| format!("reading {}: {e}", config_path.display()))?,
        )
        .map_err(|e| format!("parsing the model config: {e}"))?;
        let mut tokenizer =
            Tokenizer::from_file(&tokenizer_path).map_err(|e| format!("loading the tokenizer: {e}"))?;
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: MAX_TOKENS,
                ..Default::default()
            }))
            .map_err(|e| format!("configuring truncation: {e}"))?;
        let device = pick_device();
        // SAFETY: the weights file is memory-mapped read-only and is not
        // modified while the model is loaded — it is replaced only by a rename.
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path.clone()], DTYPE, &device)
                .map_err(|e| format!("mapping {}: {e}", weights_path.display()))?
        };
        let model = BertModel::load(vb, &config).map_err(|e| format!("loading the model: {e}"))?;
        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    fn vector(&self, text: &str) -> Result<Vec<f32>, String> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| format!("tokenizing: {e}"))?;
        let ids = encoding.get_ids();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let token_ids = Tensor::new(ids, &self.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| format!("building the input tensor: {e}"))?;
        let token_type_ids = token_ids.zeros_like().map_err(|e| format!("token types: {e}"))?;
        let attention: Vec<u32> = encoding.get_attention_mask().to_vec();
        let attention_mask = Tensor::new(attention.as_slice(), &self.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| format!("attention mask: {e}"))?;
        let hidden = self
            .model
            .forward(&token_ids, &token_type_ids, Some(&attention_mask))
            .map_err(|e| format!("forward pass: {e}"))?;
        let mask = attention_mask
            .to_dtype(DType::F32)
            .and_then(|m| m.unsqueeze(2))
            .map_err(|e| format!("shaping the mask: {e}"))?;
        let pooled = hidden
            .broadcast_mul(&mask)
            .and_then(|m| m.sum(1))
            .and_then(|s| s.broadcast_div(&mask.sum(1)?))
            .map_err(|e| format!("pooling: {e}"))?;
        let mut vector: Vec<f32> = pooled
            .squeeze(0)
            .and_then(|t| t.to_vec1())
            .map_err(|e| format!("reading the embedding: {e}"))?;
        normalize(&mut vector);
        Ok(vector)
    }
}

impl Embedder for BuiltinEmbedder {
    fn stamp(&self) -> String {
        BUILTIN_STAMP.to_string()
    }

    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        texts
            .iter()
            .map(|t| self.vector(&format!("{PASSAGE_PREFIX}{t}")))
            .collect()
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        self.vector(&format!("{QUERY_PREFIX}{text}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::Converters;
    use crate::refresh::{refresh, Scope};
    use crate::search::{search, Mode};
    use crate::store::Collection;
    use std::sync::atomic::AtomicBool;

    /// Where a test finds the model: `KB_TEST_MODELS`, else the IDE's cache.
    fn models_dir() -> Option<std::path::PathBuf> {
        let dir = std::env::var_os("KB_TEST_MODELS")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|h| std::path::PathBuf::from(h).join("PowerRustCOBOL/data/models"))
            })?;
        crate::model::model_is_cached(&dir).then_some(dir)
    }

    /// Offline, the built-in model ranks a document by meaning when it shares
    /// no word with the query — which lexical search cannot (AC11).
    #[test]
    fn the_builtin_model_finds_meaning_offline() {
        let Some(models) = models_dir() else {
            println!("built-in model not cached on this machine — semantic test skipped");
            return;
        };
        let started = std::time::Instant::now();
        let model = BuiltinEmbedder::load(&models).expect("the cached model loads");
        let load_ms = started.elapsed().as_millis();
        let root = tempfile::tempdir().unwrap();
        let c = Collection::open(root.path(), "hr").unwrap();
        std::fs::write(c.documents_dir().join("vacation.md"), "# Time off\nStaff receive twenty days of paid vacation every year.").unwrap();
        std::fs::write(c.documents_dir().join("billing.md"), "# Billing\nCustomer invoices must be settled within thirty days.").unwrap();
        let cancel = AtomicBool::new(false);
        let t = std::time::Instant::now();
        refresh(&c, &model, &Converters::default(), Scope::All, &mut |_| {}, &cancel).unwrap();
        let index_ms = t.elapsed().as_millis();
        let r = search(&c, &model, "how much annual leave do employees get", 2).unwrap();
        assert_eq!(r.mode, Mode::Semantic);
        assert_eq!(r.hits[0].document, "vacation.md", "found by meaning: {:?}", r.hits);
        println!(
            "built-in model: loaded in {load_ms} ms, 2 documents indexed in {index_ms} ms, \
             'annual leave' → vacation.md (score {:.3})",
            r.hits[0].score
        );
    }
}
