// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

use std::path::{Path, PathBuf};
use std::sync::{Arc, Once, Mutex};
use rig_core::embeddings::{EmbeddingModel, EmbeddingError, Embedding};
use ort::session::Session;
use ort::value::Value;
use tokenizers::Tokenizer;
use thiserror::Error;

static ORT_INIT: Once = Once::new();

#[derive(Error, Debug)]
pub enum LocalEmbeddingError {
    #[error("ONNX Runtime error: {0}")]
    Ort(String),
    #[error("Tokenizer error: {0}")]
    Tokenize(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Shape error")]
    ShapeError,
}

#[derive(Clone)]
pub struct LocalEmbeddingModel {
    session: Arc<Mutex<Session>>,
    tokenizer: Arc<Tokenizer>,
    ndims: usize,
}

impl LocalEmbeddingModel {
    pub fn new(model_dir: impl AsRef<Path>) -> Result<Self, LocalEmbeddingError> {
        let model_dir = model_dir.as_ref();
        
        let tokenizer = Tokenizer::from_file(model_dir.join("tokenizer.json"))
            .map_err(LocalEmbeddingError::Tokenize)?;
        
        ORT_INIT.call_once(|| {
            let _ = ort::init().commit();
        });
            
        let session = Session::builder()
            .map_err(|e| LocalEmbeddingError::Ort(e.to_string()))?
            .with_intra_threads(num_cpus::get() as usize)
            .map_err(|e| LocalEmbeddingError::Ort(e.to_string()))?
            .commit_from_file(model_dir.join("model.onnx"))
            .map_err(|e| LocalEmbeddingError::Ort(e.to_string()))?;
            
        // all-MiniLM-L6-v2 outputs 384 dimensions
        let ndims = 384; 

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
            tokenizer: Arc::new(tokenizer),
            ndims,
        })
    }
}

impl EmbeddingModel for LocalEmbeddingModel {
    type Client = ();

    fn make(_client: &Self::Client, _model: impl Into<String>, _ndims: Option<usize>) -> Self {
        unimplemented!("LocalEmbeddingModel cannot be created via make")
    }

    const MAX_DOCUMENTS: usize = 32;

    fn ndims(&self) -> usize {
        self.ndims
    }

    #[allow(clippy::type_complexity)]
    async fn embed_texts(
        &self,
        texts: impl IntoIterator<Item = String> + Send,
    ) -> Result<Vec<Embedding>, EmbeddingError> {
        let texts: Vec<String> = texts.into_iter().collect();
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let encodings = self.tokenizer.encode_batch(texts.clone(), true)
            .map_err(|e| EmbeddingError::ProviderError(e.to_string()))?;
        
        let batch_size = encodings.len();
        let seq_len = encodings[0].get_ids().len();

        let mut input_ids = vec![0i64; batch_size * seq_len];
        let mut attention_mask = vec![0i64; batch_size * seq_len];
        let mut token_type_ids = vec![0i64; batch_size * seq_len];

        for (i, encoding) in encodings.iter().enumerate() {
            for (j, &id) in encoding.get_ids().iter().enumerate() {
                let idx = i * seq_len + j;
                input_ids[idx] = id as i64;
                attention_mask[idx] = encoding.get_attention_mask()[j] as i64;
                token_type_ids[idx] = encoding.get_type_ids()[j] as i64;
            }
        }

        let input_ids_val = Value::from_array(([batch_size, seq_len], input_ids))
            .map_err(|e| EmbeddingError::ProviderError(e.to_string()))?;
        let attention_mask_val = Value::from_array(([batch_size, seq_len], attention_mask.clone()))
            .map_err(|e| EmbeddingError::ProviderError(e.to_string()))?;
        let token_type_ids_val = Value::from_array(([batch_size, seq_len], token_type_ids))
            .map_err(|e| EmbeddingError::ProviderError(e.to_string()))?;

        let inputs = ort::inputs![
            "input_ids" => input_ids_val,
            "attention_mask" => attention_mask_val,
            "token_type_ids" => token_type_ids_val,
        ];

        let mut session_guard = self.session.lock().unwrap();
        let outputs = session_guard.run(inputs)
        .map_err(|e| EmbeddingError::ProviderError(e.to_string()))?;

        let extracted = outputs["last_hidden_state"].try_extract_tensor::<f32>()
            .map_err(|e| EmbeddingError::ProviderError(e.to_string()))?;
        
        let (_shape, embeddings_data) = extracted;
        
        let mut final_embeddings = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let mut pooled = vec![0.0f32; self.ndims];
            let mut sum_mask = 0.0f32;

            for j in 0..seq_len {
                let idx = i * seq_len + j;
                let mask = attention_mask[idx] as f32;
                sum_mask += mask;
                for k in 0..self.ndims {
                    let val_idx = i * seq_len * self.ndims + j * self.ndims + k;
                    let val = embeddings_data[val_idx];
                    pooled[k] += val * mask;
                }
            }

            for k in 0..self.ndims {
                pooled[k] /= f32::max(sum_mask, 1e-9);
            }

            // L2 normalize
            let norm: f32 = pooled.iter().map(|v| v * v).sum::<f32>().sqrt();
            for val in &mut pooled {
                *val /= f32::max(norm, 1e-12);
            }

            final_embeddings.push(Embedding {
                document: texts[i].clone(),
                vec: pooled.into_iter().map(|v| v as f64).collect(),
            });
        }

        Ok(final_embeddings)
    }
}
