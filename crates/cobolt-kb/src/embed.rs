// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Turning text into vectors (spec 068 §4.5).
//!
//! Three embedders sit behind one [`Embedder`] trait:
//!
//! - [`HashingEmbedder`] — **lexical**, built in, always available. Lifted
//!   unchanged from the IDE Knowledge Base, so the two score alike.
//! - [`EndpointEmbedder`] — a model on a server, over HTTP. The HTTP itself is
//!   the host's: this crate asks a [`Transport`] to send, which is what keeps TLS
//!   out of it.
//! - the built-in semantic model — behind the `semantic` feature.
//!
//! Every vector is L2-normalised, so the cosine similarity of two is their dot
//! product.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::chunk::fnv1a;

/// The width of a lexical vector.
pub const HASHING_DIMENSIONS: usize = 384;

/// The stamp recorded with every lexical vector.
pub const HASHING_STAMP: &str = "hashing";

/// Turns text into vectors.
pub trait Embedder: Send + Sync {
    /// Names the embedder and its model. Recorded with every vector, so an index
    /// never compares vectors from two embedders (spec 068 R26).
    fn stamp(&self) -> String;

    /// Embed passages being indexed.
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String>;

    /// Embed the text being searched for. Symmetric embedders embed it like a
    /// passage; retrieval models trained with distinct query and passage
    /// prefixes override this.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        self.embed(&[text])?
            .pop()
            .ok_or_else(|| "the embedder returned no vector".to_string())
    }
}

/// Deterministic hashing bag-of-words — the lexical embedder.
///
/// Each token is hashed to a bucket with a sign drawn from the same hash, so
/// collisions cancel rather than accumulate. It matches words, not meanings.
#[derive(Debug, Clone, Copy, Default)]
pub struct HashingEmbedder;

impl HashingEmbedder {
    /// The vector for one text. Infallible.
    pub fn vector(text: &str) -> Vec<f32> {
        let mut vector = vec![0.0_f32; HASHING_DIMENSIONS];
        for token in text
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|token| token.len() > 1)
        {
            let hash = fnv1a(token.to_lowercase().as_bytes());
            vector[(hash as usize) % HASHING_DIMENSIONS] +=
                if hash & (1_u64 << 63) == 0 { 1.0 } else { -1.0 };
        }
        normalize(&mut vector);
        vector
    }
}

impl Embedder for HashingEmbedder {
    fn stamp(&self) -> String {
        HASHING_STAMP.to_string()
    }

    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        Ok(texts.iter().map(|t| Self::vector(t)).collect())
    }
}

/// Scale a vector to unit length (a zero vector stays zero).
pub fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for value in vector {
            *value /= norm;
        }
    }
}

/// The network, as the host provides it. A host implements this with its own
/// HTTP client; a test implements it with a table of canned replies.
pub trait Transport: Send + Sync {
    /// POST a JSON body; answers the HTTP status and the response body.
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        timeout_ms: u64,
    ) -> Result<(u16, String), String>;

    /// GET `url` into the file `dest`, reporting bytes received and the total
    /// when known, and stopping when `cancel` is raised.
    fn get_to_file(
        &self,
        url: &str,
        dest: &Path,
        progress: &mut dyn FnMut(u64, Option<u64>),
        cancel: &AtomicBool,
    ) -> Result<(), String>;
}

/// The two wire formats an embedding server speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointApi {
    /// `POST <base>/embeddings` — OpenAI and every server that copies it
    /// (LM Studio, vLLM, most gateways).
    OpenAi,
    /// `POST <base>/api/embed` — Ollama.
    Ollama,
}

impl EndpointApi {
    /// From a control's API name, as `AgentObject` spells them.
    pub fn from_name(name: &str) -> Self {
        if name.trim().eq_ignore_ascii_case("ollama") {
            Self::Ollama
        } else {
            Self::OpenAi
        }
    }

    fn path(self) -> &'static str {
        match self {
            Self::OpenAi => "/embeddings",
            Self::Ollama => "/api/embed",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Ollama => "ollama",
        }
    }
}

/// An embedding model on a server.
pub struct EndpointEmbedder {
    pub api: EndpointApi,
    /// The server's base URL; the API's path is added unless it is already
    /// there.
    pub url: String,
    pub model: String,
    /// Sent as a bearer token when present. Never logged or echoed.
    pub key: Option<String>,
    pub timeout_ms: u64,
    pub transport: Arc<dyn Transport>,
}

impl EndpointEmbedder {
    fn endpoint(&self) -> String {
        let base = self.url.trim().trim_end_matches('/');
        let path = self.api.path();
        if base.ends_with(path) {
            base.to_string()
        } else {
            format!("{base}{path}")
        }
    }
}

impl Embedder for EndpointEmbedder {
    fn stamp(&self) -> String {
        format!("endpoint:{}:{}", self.api.name(), self.model.trim())
    }

    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let body = serde_json::json!({ "model": self.model.trim(), "input": texts }).to_string();
        let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        if let Some(key) = self.key.as_deref().filter(|k| !k.trim().is_empty()) {
            headers.push(("Authorization".to_string(), format!("Bearer {}", key.trim())));
        }
        let url = self.endpoint();
        let (status, reply) = self
            .transport
            .post_json(&url, &headers, &body, self.timeout_ms)
            .map_err(|e| format!("the embedding server at {url} could not be reached: {e}"))?;
        if !(200..300).contains(&status) {
            let excerpt: String = reply.chars().take(200).collect();
            return Err(format!("the embedding server at {url} answered {status}: {excerpt}"));
        }
        let mut vectors = parse_vectors(self.api, &reply)?;
        if vectors.len() != texts.len() {
            return Err(format!(
                "the embedding server returned {} vectors for {} texts",
                vectors.len(),
                texts.len()
            ));
        }
        for v in &mut vectors {
            normalize(v);
        }
        Ok(vectors)
    }
}

/// Read the vectors out of a server's reply, in input order.
fn parse_vectors(api: EndpointApi, reply: &str) -> Result<Vec<Vec<f32>>, String> {
    let json: serde_json::Value =
        serde_json::from_str(reply).map_err(|e| format!("the embedding reply is not JSON: {e}"))?;
    let as_vector = |v: &serde_json::Value| -> Option<Vec<f32>> {
        v.as_array()?
            .iter()
            .map(|x| x.as_f64().map(|f| f as f32))
            .collect()
    };
    let vectors: Option<Vec<Vec<f32>>> = match api {
        EndpointApi::Ollama => json
            .get("embeddings")
            .and_then(|e| e.as_array())
            .map(|rows| rows.iter().filter_map(as_vector).collect()),
        EndpointApi::OpenAi => json.get("data").and_then(|d| d.as_array()).map(|rows| {
            let mut indexed: Vec<(usize, Vec<f32>)> = rows
                .iter()
                .enumerate()
                .filter_map(|(i, row)| {
                    let index = row.get("index").and_then(|x| x.as_u64()).map_or(i, |x| x as usize);
                    Some((index, as_vector(row.get("embedding")?)?))
                })
                .collect();
            indexed.sort_by_key(|(i, _)| *i);
            indexed.into_iter().map(|(_, v)| v).collect()
        }),
    };
    vectors.ok_or_else(|| "the embedding reply carries no vectors".to_string())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A transport answering from a script, recording what it was sent.
    pub(crate) struct FakeTransport {
        pub reply: Mutex<Result<(u16, String), String>>,
        pub sent: Mutex<Vec<(String, Vec<(String, String)>, String)>>,
    }

    impl FakeTransport {
        pub fn answering(status: u16, body: &str) -> Arc<Self> {
            Arc::new(Self {
                reply: Mutex::new(Ok((status, body.to_string()))),
                sent: Mutex::new(Vec::new()),
            })
        }
    }

    impl Transport for FakeTransport {
        fn post_json(
            &self,
            url: &str,
            headers: &[(String, String)],
            body: &str,
            _timeout_ms: u64,
        ) -> Result<(u16, String), String> {
            self.sent
                .lock()
                .unwrap()
                .push((url.to_string(), headers.to_vec(), body.to_string()));
            self.reply.lock().unwrap().clone()
        }

        fn get_to_file(
            &self,
            _url: &str,
            _dest: &Path,
            _progress: &mut dyn FnMut(u64, Option<u64>),
            _cancel: &AtomicBool,
        ) -> Result<(), String> {
            Err("not scripted".into())
        }
    }

    fn endpoint(api: EndpointApi, t: Arc<FakeTransport>) -> EndpointEmbedder {
        EndpointEmbedder {
            api,
            url: "http://host:1234/v1/".into(),
            model: "nomic-embed".into(),
            key: Some("sk-secret".into()),
            timeout_ms: 1000,
            transport: t,
        }
    }

    #[test]
    fn hashing_is_deterministic_normalised_and_lexical() {
        let a = HashingEmbedder::vector("Annual leave is twenty days");
        let b = HashingEmbedder::vector("annual LEAVE is twenty days");
        assert_eq!(a, b, "case does not matter");
        let norm: f32 = a.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
        let dot = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>();
        let near = HashingEmbedder::vector("leave days");
        let far = HashingEmbedder::vector("invoice shipping address");
        assert!(dot(&a, &near) > dot(&a, &far), "shared words score higher");
    }

    #[test]
    fn openai_request_and_reply_in_input_order() {
        let t = FakeTransport::answering(
            200,
            r#"{"data":[{"index":1,"embedding":[0,2]},{"index":0,"embedding":[3,4]}]}"#,
        );
        let e = endpoint(EndpointApi::OpenAi, t.clone());
        let v = e.embed(&["first", "second"]).unwrap();
        assert_eq!(v, vec![vec![0.6, 0.8], vec![0.0, 1.0]], "ordered by index, normalised");
        let sent = t.sent.lock().unwrap();
        assert_eq!(sent[0].0, "http://host:1234/v1/embeddings");
        assert!(sent[0].1.contains(&("Authorization".into(), "Bearer sk-secret".into())));
        assert_eq!(sent[0].2, r#"{"input":["first","second"],"model":"nomic-embed"}"#);
        assert_eq!(e.stamp(), "endpoint:openai:nomic-embed");
    }

    #[test]
    fn ollama_reply_and_failures_are_reported_without_the_key() {
        let t = FakeTransport::answering(200, r#"{"embeddings":[[1,0]]}"#);
        let e = endpoint(EndpointApi::Ollama, t.clone());
        assert_eq!(e.embed_query("q").unwrap(), vec![1.0, 0.0]);
        assert_eq!(t.sent.lock().unwrap()[0].0, "http://host:1234/v1/api/embed");

        let bad = FakeTransport::answering(401, "unauthorized");
        let err = endpoint(EndpointApi::Ollama, bad).embed(&["q"]).unwrap_err();
        assert!(err.contains("401"), "{err}");
        assert!(!err.contains("sk-secret"), "the key never appears in a message");
    }
}
