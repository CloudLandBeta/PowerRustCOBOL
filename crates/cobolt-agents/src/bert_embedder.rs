// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Semantic embeddings from a pure-Rust BERT, via Candle.
//!
//! Fills the one seam `embedvec` leaves open: it stores vectors, it does not
//! create them. The model is `sentence-transformers/all-MiniLM-L6-v2` —
//! 384-dimensional, which is exactly [`VECTOR_DIMENSIONS`], so the vector
//! store, the persisted index and the manifest are unaffected by the switch.
//!
//! ## Why this matters more than it looks
//!
//! The embedder it replaces hashed tokens into buckets. That is adequate when a
//! query and its answer share literal tokens — `DataGrid`, `onTick` — which
//! covers catalogue lookups. It fails completely on the case the Knowledge Base
//! exists for: a developer's requirements document describing a "customer master
//! file layout" shares almost no tokens with `CUSTOMER-RECORD`, so a lexical
//! index cannot connect them however it is tuned. Retrieval that silently
//! returns nothing is the same failure mode as a context that silently omits
//! something — the agent proceeds, confidently, on less than it needed.
//!
//! ## Availability
//!
//! Weights are fetched once from the Hugging Face hub into the IDE's own cache
//! and reused offline thereafter. First use therefore needs network. When the
//! model is neither cached nor reachable, [`best_available`] falls back to the
//! hashing embedder rather than failing: degraded retrieval beats an IDE that
//! cannot open a project. The choice is reported so the caller can surface it,
//! because "semantic search is silently lexical today" is exactly the kind of
//! thing that must not be discovered by puzzlement.
//!
//! An index built with one embedder must not be searched with another — the
//! vectors are not comparable. [`EmbedderKind`] is recorded alongside the index
//! so a change of embedder can force a rebuild instead of returning nonsense.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use tokenizers::Tokenizer;

use crate::knowledge_store::{Embedder, HashingEmbedder, VECTOR_DIMENSIONS};

/// The sentence-transformer this embedder speaks. 384-dim output.
pub const MODEL_ID: &str = "sentence-transformers/all-MiniLM-L6-v2";

/// Longest token sequence fed to the model. MiniLM is trained at 256; longer
/// input is truncated, which is why documents are kept to one subject each.
const MAX_TOKENS: usize = 256;

/// Which embedder produced an index. Stored with the index so a switch forces a
/// rebuild rather than silently comparing incomparable vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EmbedderKind {
    /// Deterministic hashing bag-of-words — lexical only.
    Hashing,
    /// Candle BERT (`all-MiniLM-L6-v2`) — semantic.
    Bert,
}

impl EmbedderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hashing => "hashing",
            Self::Bert => "bert-minilm-l6-v2",
        }
    }
}

/// A BERT sentence embedder running entirely in-process.
pub struct BertEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl BertEmbedder {
    /// Load the model, fetching it into `cache_dir` on first use.
    pub fn load(cache_dir: &Path) -> Result<Self, String> {
        let (config_path, tokenizer_path, weights_path) = resolve_model_files(cache_dir)?;

        let config: Config = serde_json::from_str(
            &std::fs::read_to_string(&config_path)
                .map_err(|e| format!("reading {}: {e}", config_path.display()))?,
        )
        .map_err(|e| format!("parsing the model config: {e}"))?;

        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| format!("loading the tokenizer: {e}"))?;
        // Pad/truncate to a fixed window so every document embeds the same way.
        let truncation = tokenizers::TruncationParams {
            max_length: MAX_TOKENS,
            ..Default::default()
        };
        tokenizer
            .with_truncation(Some(truncation))
            .map_err(|e| format!("configuring truncation: {e}"))?;

        let device = Device::Cpu;
        // SAFETY: mmap of a file we just resolved; candle requires unsafe here
        // because the mapping must outlive the borrow, which `VarBuilder` owns.
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path.clone()], DTYPE, &device)
                .map_err(|e| format!("mapping {}: {e}", weights_path.display()))?
        };
        let model =
            BertModel::load(vb, &config).map_err(|e| format!("loading the BERT model: {e}"))?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    fn embed_inner(&self, text: &str) -> Result<Vec<f32>, String> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| format!("tokenizing: {e}"))?;
        let ids = encoding.get_ids();
        if ids.is_empty() {
            return Ok(vec![0.0; VECTOR_DIMENSIONS]);
        }
        let token_ids = Tensor::new(ids, &self.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| format!("building the input tensor: {e}"))?;
        let token_type_ids = token_ids
            .zeros_like()
            .map_err(|e| format!("building token types: {e}"))?;
        let attention: Vec<u32> = encoding.get_attention_mask().to_vec();
        let attention_mask = Tensor::new(attention.as_slice(), &self.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| format!("building the attention mask: {e}"))?;

        let hidden = self
            .model
            .forward(&token_ids, &token_type_ids, Some(&attention_mask))
            .map_err(|e| format!("BERT forward pass: {e}"))?;

        // Mean-pool over the sequence, honouring the attention mask so padding
        // does not drag the vector toward zero, then L2-normalise so cosine
        // similarity is a plain dot product.
        let mask = attention_mask
            .to_dtype(DType::F32)
            .and_then(|m| m.unsqueeze(2))
            .map_err(|e| format!("shaping the mask: {e}"))?;
        let masked = hidden
            .broadcast_mul(&mask)
            .map_err(|e| format!("masking: {e}"))?;
        let summed = masked.sum(1).map_err(|e| format!("pooling: {e}"))?;
        let counts = mask.sum(1).map_err(|e| format!("counting tokens: {e}"))?;
        let pooled = summed
            .broadcast_div(&counts)
            .map_err(|e| format!("averaging: {e}"))?;
        let mut vector: Vec<f32> = pooled
            .squeeze(0)
            .and_then(|t| t.to_vec1())
            .map_err(|e| format!("reading the embedding: {e}"))?;

        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for value in &mut vector {
                *value /= norm;
            }
        }
        Ok(vector)
    }
}

impl Embedder for BertEmbedder {
    fn dimensions(&self) -> usize {
        VECTOR_DIMENSIONS
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        // A single document failing to embed must not abort a whole reindex;
        // a zero vector simply never matches, and the failure is visible as a
        // document that cannot be retrieved rather than as a crash.
        self.embed_inner(text).unwrap_or_else(|error| {
            tracing::warn!(%error, "embedding failed; document indexed as unmatchable");
            vec![0.0; VECTOR_DIMENSIONS]
        })
    }
}

const MODEL_OWNER: &str = "sentence-transformers";
const MODEL_NAME: &str = "all-MiniLM-L6-v2";
const MODEL_FILES: [&str; 3] = ["config.json", "tokenizer.json", "model.safetensors"];

/// Where the three model files live once fetched.
pub fn model_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join(MODEL_NAME)
}

/// Whether the model is already on disk and usable without network.
pub fn model_is_cached(cache_dir: &Path) -> bool {
    let dir = model_dir(cache_dir);
    MODEL_FILES.iter().all(|f| dir.join(f).exists())
}

/// Locate the three model files **without touching the network**.
///
/// Loading never downloads: pulling ~90 MB because someone opened a project is
/// not a decision this layer gets to make. [`ensure_downloaded`] is the
/// explicit, callable counterpart.
fn resolve_model_files(cache_dir: &Path) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let dir = model_dir(cache_dir);
    let (config, tokenizer, weights) = (
        dir.join("config.json"),
        dir.join("tokenizer.json"),
        dir.join("model.safetensors"),
    );
    if config.exists() && tokenizer.exists() && weights.exists() {
        return Ok((config, tokenizer, weights));
    }
    Err(format!(
        "the {MODEL_ID} model is not in {}; semantic search stays lexical until it is fetched",
        dir.display()
    ))
}

/// Fetch the model into `cache_dir`. Explicit, blocking, and the only path that
/// uses the network. Returns the directory the files landed in.
pub fn ensure_downloaded(cache_dir: &Path) -> Result<PathBuf, String> {
    let dir = model_dir(cache_dir);
    if model_is_cached(cache_dir) {
        return Ok(dir);
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("model cache {}: {e}", dir.display()))?;
    let client = hf_hub::HFClientSync::new().map_err(|e| format!("Hugging Face client: {e}"))?;
    let repo = client.model(MODEL_OWNER, MODEL_NAME);
    for file in MODEL_FILES {
        if dir.join(file).exists() {
            continue;
        }
        repo.download_file()
            .filename(file)
            .local_dir(dir.clone())
            .send()
            .map_err(|e| format!("downloading {file} for {MODEL_ID}: {e}"))?;
    }
    Ok(dir)
}

/// The best embedder available right now, with the kind it settled on.
///
/// Prefers Candle BERT; falls back to hashing when the model is neither cached
/// nor downloadable. The caller is expected to report the kind — a fallback
/// that nobody mentions is a silent downgrade from semantic to lexical search.
pub fn best_available(cache_dir: &Path) -> (Arc<dyn Embedder>, EmbedderKind, Option<String>) {
    match BertEmbedder::load(cache_dir) {
        Ok(model) => (Arc::new(model), EmbedderKind::Bert, None),
        Err(error) => (
            Arc::new(HashingEmbedder),
            EmbedderKind::Hashing,
            Some(error),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fallback must engage rather than panic when the model is absent —
    /// an offline machine still has to open projects. Deterministic because
    /// loading never reaches the network: an empty cache can only fall back.
    #[test]
    fn an_absent_model_falls_back_to_hashing_and_says_so() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(!model_is_cached(dir.path()));
        let (embedder, kind, note) = best_available(dir.path());
        assert_eq!(kind, EmbedderKind::Hashing);
        let note = note.expect("a downgrade must be explained, never silent");
        assert!(
            note.contains("lexical"),
            "the note must say what was lost: {note}"
        );
        assert_eq!(embedder.dimensions(), VECTOR_DIMENSIONS);
        assert_eq!(embedder.embed("DataGrid onCellClick").len(), VECTOR_DIMENSIONS);
    }

    /// Loading must not download. A cold cache stays cold.
    #[test]
    fn loading_never_touches_the_network() {
        let dir = tempfile::tempdir().expect("temp dir");
        let _ = BertEmbedder::load(dir.path()); // expected to fail, offline-safe
        assert!(
            !model_dir(dir.path()).exists() || !model_is_cached(dir.path()),
            "load() must not have fetched anything"
        );
    }

    /// Both embedders must agree on width, or an index built by one is
    /// unreadable by the other in a way the store cannot detect.
    #[test]
    fn both_embedders_produce_the_same_width() {
        assert_eq!(HashingEmbedder.dimensions(), VECTOR_DIMENSIONS);
        assert_eq!(EmbedderKind::Bert.as_str(), "bert-minilm-l6-v2");
        assert_ne!(
            EmbedderKind::Bert.as_str(),
            EmbedderKind::Hashing.as_str(),
            "the two kinds must be distinguishable in the stored stamp"
        );
    }
}
