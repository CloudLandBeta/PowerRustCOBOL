// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Semantic embeddings from a pure-Rust BERT, via Candle.
//!
//! Implements the [`Embedder`] seam of [`crate::knowledge_store`]: the index
//! ([`crate::project_knowledge`]) stores vectors, this module creates them.
//! The model is `intfloat/multilingual-e5-small` — 384-dimensional, which is
//! exactly [`VECTOR_DIMENSIONS`], so the persisted index format is unaffected
//! by a model switch.
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
//! ## Multilingual, because the developers are
//!
//! The IDE speaks six languages (en, pt, es, fr, ja, zh) and the developer
//! requests arrive in all of them, while the Knowledge Base reference material
//! is written in English. `multilingual-e5-small` (~100 languages, MIT) was
//! trained for exactly that asymmetry: a Portuguese query lands next to the
//! English passage that answers it. Its predecessor here, `all-MiniLM-L6-v2`,
//! was English-only — a Portuguese request usually retrieved nothing unless it
//! happened to contain English identifiers.
//!
//! ## E5 prefixes are part of the contract
//!
//! E5 models are trained with a role marker in the text itself: passages are
//! embedded as `"passage: …"` and queries as `"query: …"`. Dropping the
//! prefixes (or using one for both roles) silently costs most of the model's
//! retrieval quality, so the prefixing lives INSIDE this embedder — callers
//! choose `embed` (documents) or `embed_query` (searches) and never see the
//! markers. The hashing fallback ignores the distinction entirely.
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
pub const MODEL_ID: &str = "intfloat/multilingual-e5-small";

/// Longest token sequence fed to the model. E5's window is 512; 256 keeps the
/// CPU cost of a full reindex bounded, and documents are kept to one subject
/// each so the head of a document is representative.
const MAX_TOKENS: usize = 256;

/// E5 role markers. Trained into the model — see the module note.
const PASSAGE_PREFIX: &str = "passage: ";
const QUERY_PREFIX: &str = "query: ";

/// Which embedder produced an index. Stored with the index so a switch forces a
/// rebuild rather than silently comparing incomparable vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EmbedderKind {
    /// Deterministic hashing bag-of-words — lexical only.
    Hashing,
    /// Candle BERT (`multilingual-e5-small`) — semantic, cross-lingual.
    Bert,
}

impl EmbedderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hashing => "hashing",
            Self::Bert => "bert-multilingual-e5-small",
        }
    }
}

/// A BERT sentence embedder running entirely in-process.
pub struct BertEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

/// The embedding device chosen for this process, as a human-readable label
/// ("Metal", "CUDA 0", "CPU · low-power (2 threads)"). Set by the first
/// [`pick_device`] call; empty before any embedder loads. Every embedding
/// path shares it — the System KB, project KBs, and query embedding.
static ACTIVE_DEVICE_LABEL: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The label of the device the embedder runs on, for CLI/UI reporting.
/// `None` until an embedder has loaded.
pub fn active_device_label() -> Option<&'static str> {
    ACTIVE_DEVICE_LABEL.get().map(|s| s.as_str())
}

/// How many rayon threads the LOW-POWER CPU fallback uses. Two keeps the
/// machine cool and responsive; the reindex simply takes longer.
const CPU_LOW_POWER_THREADS: &str = "2";

/// Choose the embedding device (one policy, both KBs):
///
/// - Metal (every macOS build carries it) / CUDA (opt-in `embed-cuda`
///   feature) usable at runtime ⇒ **full speed** on the GPU.
/// - CPU fallback ⇒ **low-power**: the rayon pool that candle's CPU matmuls
///   use is capped to [`CPU_LOW_POWER_THREADS`] so a reindex never pins all
///   cores (fan noise was the reported symptom). The cap only works when set
///   before rayon's global pool first initialises — which holds because
///   nothing else in the workspace uses rayon — and an operator-set
///   `RAYON_NUM_THREADS` is always respected.
///
/// `PRC_EMBED_DEVICE=cpu|metal|cuda` forces a backend (a forced GPU that
/// fails to initialise still falls back to low-power CPU rather than
/// crashing).
fn pick_device() -> Device {
    let forced = std::env::var("PRC_EMBED_DEVICE")
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default();

    #[cfg(target_os = "macos")]
    if forced.is_empty() || forced == "metal" {
        match Device::new_metal(0) {
            Ok(d) => {
                let _ = ACTIVE_DEVICE_LABEL.set("Metal".to_owned());
                return d;
            }
            Err(e) => tracing::warn!(%e, "Metal unavailable; falling back"),
        }
    }
    #[cfg(feature = "embed-cuda")]
    if forced.is_empty() || forced == "cuda" {
        match Device::new_cuda(0) {
            Ok(d) => {
                let _ = ACTIVE_DEVICE_LABEL.set("CUDA 0".to_owned());
                return d;
            }
            Err(e) => tracing::warn!(%e, "CUDA unavailable; falling back"),
        }
    }

    // CPU fallback (or forced "cpu") — low-power unless the operator chose
    // their own thread count.
    let threads = match std::env::var("RAYON_NUM_THREADS") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            std::env::set_var("RAYON_NUM_THREADS", CPU_LOW_POWER_THREADS);
            CPU_LOW_POWER_THREADS.to_owned()
        }
    };
    let _ = ACTIVE_DEVICE_LABEL.set(format!("CPU ({threads} threads)"));
    Device::Cpu
}

#[cfg(test)]
mod device_tests {
    use super::*;

    /// The CPU path is LOW-POWER — the rayon pool is capped unless the
    /// operator chose a thread count — and the label reports the device.
    /// CPU is forced via the override so the test holds on machines whose
    /// build carries a GPU backend (every macOS build has Metal). (The only
    /// test touching these env vars.)
    #[test]
    fn cpu_fallback_is_low_power_and_reports_a_label() {
        std::env::set_var("PRC_EMBED_DEVICE", "cpu");
        std::env::remove_var("RAYON_NUM_THREADS");
        let device = pick_device();
        assert!(matches!(device, Device::Cpu), "forced cpu picks CPU");
        assert_eq!(
            std::env::var("RAYON_NUM_THREADS").as_deref(),
            Ok(CPU_LOW_POWER_THREADS),
            "low-power cap applied when the operator set nothing"
        );
        let label = active_device_label().expect("label set by pick_device");
        assert!(label.starts_with("CPU"), "label names the device: {label}");
        println!("embedding device label: {label}");

        // An operator-set thread count is respected, never overwritten.
        std::env::set_var("RAYON_NUM_THREADS", "7");
        let _ = pick_device();
        assert_eq!(std::env::var("RAYON_NUM_THREADS").as_deref(), Ok("7"));
        std::env::remove_var("RAYON_NUM_THREADS");
        std::env::remove_var("PRC_EMBED_DEVICE");
    }
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

        let device = pick_device();
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
        self.embed_inner(&format!("{PASSAGE_PREFIX}{text}"))
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "embedding failed; document indexed as unmatchable");
                vec![0.0; VECTOR_DIMENSIONS]
            })
    }

    fn embed_query(&self, text: &str) -> Vec<f32> {
        // A query that fails to embed returns the zero vector: it matches
        // nothing, and the empty result is the visible symptom.
        self.embed_inner(&format!("{QUERY_PREFIX}{text}"))
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "query embedding failed; search returns nothing");
                vec![0.0; VECTOR_DIMENSIONS]
            })
    }
}

const MODEL_OWNER: &str = "intfloat";
const MODEL_NAME: &str = "multilingual-e5-small";
const MODEL_FILES: [&str; 3] = ["config.json", "tokenizer.json", "model.safetensors"];
/// Hugging Face's public file endpoint. Three plain GETs replace the `hf-hub`
/// crate, whose xet transfer stack dragged in a rustls/aws-lc-rs/blake3 subtree
/// — a C toolchain requirement for downloading three files over HTTPS.
const HF_RESOLVE_BASE: &str = "https://huggingface.co";

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
/// Loading never downloads: pulling ~470 MB because someone opened a project
/// is not a decision this layer gets to make. [`ensure_downloaded`] is the
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

/// Live progress of a model download, shared between the download thread and
/// whatever surface is rendering a progress bar. All counters are in bytes;
/// `total` is 0 while the preflight is still sizing the transfer.
#[derive(Debug, Default)]
pub struct DownloadProgress {
    pub downloaded: std::sync::atomic::AtomicU64,
    pub total: std::sync::atomic::AtomicU64,
    /// Set by the UI to abort; the download loop notices between chunks,
    /// removes its partial file and returns an error containing "cancelled".
    pub cancel: std::sync::atomic::AtomicBool,
}

/// Fetch the model into `cache_dir` without progress reporting. Blocking, and
/// (with [`ensure_downloaded_with_progress`]) the only path that uses the
/// network. Returns the directory the files landed in.
pub fn ensure_downloaded(cache_dir: &Path) -> Result<PathBuf, String> {
    ensure_downloaded_with_progress(cache_dir, &DownloadProgress::default())
}

/// Fetch the model into `cache_dir`, streaming byte counts into `progress`.
///
/// The client keeps NO whole-request timeout: reqwest's 30-second default is
/// measured to the END OF THE BODY, which silently caps the transfer at
/// ~16 MB/s — on a slower link the 470 MB weights file would abort mid-body
/// every time. Connecting still times out, and the UI holds the cancel switch.
pub fn ensure_downloaded_with_progress(
    cache_dir: &Path,
    progress: &DownloadProgress,
) -> Result<PathBuf, String> {
    use std::io::{Read, Write};
    use std::sync::atomic::Ordering;

    let dir = model_dir(cache_dir);
    if model_is_cached(cache_dir) {
        return Ok(dir);
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("model cache {}: {e}", dir.display()))?;
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("PowerRustCOBOL/", env!("CARGO_PKG_VERSION")))
        .timeout(None)
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let missing: Vec<&str> = MODEL_FILES
        .iter()
        .filter(|file| !dir.join(file).exists())
        .copied()
        .collect();
    let url_for =
        |file: &str| format!("{HF_RESOLVE_BASE}/{MODEL_OWNER}/{MODEL_NAME}/resolve/main/{file}");

    // Preflight: size the whole transfer so the progress bar has a real total
    // from the first byte. A HEAD that fails or lacks a length is not fatal —
    // that file's length joins the total when its GET answers instead.
    let mut sized: Vec<&str> = Vec::new();
    let mut total = 0u64;
    for file in &missing {
        if let Ok(response) = client.head(url_for(file)).send() {
            if let Some(length) = response.content_length() {
                total += length;
                sized.push(file);
            }
        }
    }
    progress.total.store(total, Ordering::Relaxed);

    for file in missing {
        let target = dir.join(file);
        let mut response = client
            .get(url_for(file))
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("downloading {file} for {MODEL_ID}: {e}"))?;
        if !sized.contains(&file) {
            if let Some(length) = response.content_length() {
                progress.total.fetch_add(length, Ordering::Relaxed);
            }
        }
        // Write via a temporary sibling: an interrupted download must not leave a
        // truncated file behind that `model_is_cached` would then call complete.
        let partial = dir.join(format!("{file}.partial"));
        let result = (|| -> Result<(), String> {
            let mut out = std::fs::File::create(&partial)
                .map_err(|e| format!("writing {}: {e}", partial.display()))?;
            let mut buffer = vec![0u8; 256 * 1024];
            loop {
                if progress.cancel.load(Ordering::Relaxed) {
                    return Err(format!("download cancelled while fetching {file}"));
                }
                let n = response
                    .read(&mut buffer)
                    .map_err(|e| format!("downloading {file} for {MODEL_ID}: {e}"))?;
                if n == 0 {
                    break;
                }
                out.write_all(&buffer[..n])
                    .map_err(|e| format!("writing {}: {e}", partial.display()))?;
                progress.downloaded.fetch_add(n as u64, Ordering::Relaxed);
            }
            out.flush()
                .map_err(|e| format!("writing {}: {e}", partial.display()))
        })();
        if let Err(error) = result {
            let _ = std::fs::remove_file(&partial);
            return Err(error);
        }
        std::fs::rename(&partial, &target)
            .map_err(|e| format!("writing {}: {e}", target.display()))?;
    }
    Ok(dir)
}

/// Delete the cached model files so the next download starts clean. Used when
/// the files exist but cannot be loaded (truncation, corruption).
pub fn discard_cached_model(cache_dir: &Path) -> Result<(), String> {
    let dir = model_dir(cache_dir);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("removing {}: {e}", dir.display()))?;
    }
    Ok(())
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
        assert_eq!(EmbedderKind::Bert.as_str(), "bert-multilingual-e5-small");
        assert_ne!(
            EmbedderKind::Bert.as_str(),
            EmbedderKind::Hashing.as_str(),
            "the two kinds must be distinguishable in the stored stamp"
        );
    }

    /// The E5 role markers are the exact strings the model was trained with —
    /// a typo here (extra space, missing colon) degrades retrieval silently.
    #[test]
    fn e5_prefixes_are_the_trained_literals() {
        assert_eq!(PASSAGE_PREFIX, "passage: ");
        assert_eq!(QUERY_PREFIX, "query: ");
    }

    /// The hashing fallback must treat queries and passages identically:
    /// prefixes are an E5 contract, and leaking them into the lexical path
    /// would make "query" and "passage" spurious match tokens.
    #[test]
    fn hashing_embedder_is_symmetric() {
        let doc = HashingEmbedder.embed("DataGrid onCellClick");
        let query = HashingEmbedder.embed_query("DataGrid onCellClick");
        assert_eq!(doc, query);
    }
}
