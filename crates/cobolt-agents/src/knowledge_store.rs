// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Embedding seam and shared locations for the vector Knowledge Bases.
//!
//! The actual index lives in [`crate::project_knowledge`] (a redb table read
//! by full scan and scored by dot product). This module owns what the index
//! builds on:
//!
//! - [`Embedder`] — the seam between "store vectors" and "create vectors".
//!   The semantic implementation is [`crate::bert_embedder`] (Candle,
//!   `multilingual-e5-small`); [`HashingEmbedder`] is the offline fallback.
//! - [`VECTOR_DIMENSIONS`] — the shared vector width.
//! - [`ide_data_dir`] — where the machine-level (System) index and the model
//!   cache live: `~/PowerRustCOBOL/data`.

use std::path::PathBuf;

/// Vector width. 384 matches the common sentence-transformer sizes (including
/// `multilingual-e5-small`), so a future model swap needs no reindex format
/// change.
pub const VECTOR_DIMENSIONS: usize = 384;

/// Turns text into a vector. See the module note on embeddings.
pub trait Embedder: Send + Sync {
    fn dimensions(&self) -> usize;
    /// Embed a DOCUMENT (a passage being indexed).
    fn embed(&self, text: &str) -> Vec<f32>;
    /// Embed a QUERY (the text being searched for). Defaults to [`embed`]:
    /// symmetric embedders make no distinction. Asymmetric retrieval models
    /// (E5) are trained with distinct query/passage prefixes and override this
    /// — searching an E5 index with passage-embedded queries quietly costs
    /// most of the model's retrieval quality.
    ///
    /// [`embed`]: Embedder::embed
    fn embed_query(&self, text: &str) -> Vec<f32> {
        self.embed(text)
    }
}

/// Deterministic hashing bag-of-words — the offline default.
///
/// Each token is hashed to a bucket with a sign drawn from the same hash, so
/// collisions cancel rather than accumulate; the result is L2-normalised, which
/// makes the cosine distance a plain dot product. It is NOT semantic — it
/// cannot match "grid control" to `DataGrid` — but it is effective for the
/// keyed lookups that dominate here ("DataGrid", "Timer", `onTick`), because an
/// identifier either occurs in the document or does not.
#[derive(Debug, Clone, Default)]
pub struct HashingEmbedder;

impl HashingEmbedder {
    fn fnv1a(text: &str) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in text.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
}

impl Embedder for HashingEmbedder {
    fn dimensions(&self) -> usize {
        VECTOR_DIMENSIONS
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0_f32; VECTOR_DIMENSIONS];
        for token in text
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|token| token.len() > 1)
        {
            let token = token.to_lowercase();
            let hash = Self::fnv1a(&token);
            vector[(hash as usize) % VECTOR_DIMENSIONS] +=
                if hash & (1_u64 << 63) == 0 { 1.0 } else { -1.0 };
        }
        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for value in &mut vector {
                *value /= norm;
            }
        }
        vector
    }
}

/// The IDE's own Knowledge Base directory: `~/PowerRustCOBOL/data`.
///
/// IDE-owned reference material belongs to the IDE, not to whichever project
/// happens to be open, so it is written once per machine and shared by every
/// project rather than copied into each one.
pub fn ide_data_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("PowerRustCOBOL").join("data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ide_store_lives_outside_any_project() {
        let ide = ide_data_dir();
        assert!(ide.ends_with("PowerRustCOBOL/data"));
    }

    /// The fallback embedder's contract: deterministic, normalised, and
    /// sensitive to the tokens that matter (identifiers).
    #[test]
    fn hashing_embedder_is_deterministic_and_normalised() {
        let embedder = HashingEmbedder;
        let a = embedder.embed("DataGrid supports onTick events");
        let b = embedder.embed("DataGrid supports onTick events");
        assert_eq!(a, b, "same text, same vector");
        assert_eq!(a.len(), VECTOR_DIMENSIONS);
        let norm = a.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "L2-normalised (got {norm})");
        let other = embedder.embed("Timer control interval");
        assert_ne!(a, other, "different identifiers, different vectors");
        println!(
            "hashing embedder: {} dims, norm {:.6}, distinct vectors for distinct identifiers",
            a.len(),
            norm
        );
    }
}
