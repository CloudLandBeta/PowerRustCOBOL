// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The worker side of the `KnowledgeBase` control (spec 068 R28–R31).
//!
//! Every operation that touches documents or searches runs on a background
//! thread, so the form keeps painting while a large folder is indexed (R31).
//! The worker reports through the interpreter's async channel: throttled
//! [`AsyncOutcome::KbProgress`] messages, then exactly one final outcome.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use cobolt_kb::convert::Converters;
use cobolt_kb::embed::{Embedder, EndpointApi, EndpointEmbedder, HashingEmbedder};
use cobolt_kb::refresh::{self, Outcome, Progress, Scope};
use cobolt_kb::store::{Collection, KbError};

use crate::async_op::{AsyncOutcome, KbHit};
use crate::kb_transport::RuntimeTransport;

/// At most one progress message per this interval, plus the last one.
const PROGRESS_EVERY: Duration = Duration::from_millis(50);

/// Which embedder a control asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmbedderChoice {
    Lexical,
    Endpoint,
    Builtin,
}

impl EmbedderChoice {
    pub(crate) fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "endpoint" => Self::Endpoint,
            "builtin" | "built-in" | "semantic" => Self::Builtin,
            _ => Self::Lexical,
        }
    }
}

/// Everything a worker needs, read from the control on the interpreter thread.
#[derive(Debug, Clone)]
pub(crate) struct KbConfig {
    pub location: PathBuf,
    pub collection: String,
    pub embedder: EmbedderChoice,
    pub url: String,
    pub api: String,
    pub model: String,
    pub key: String,
    pub write_wait: Duration,
    pub max_results: usize,
    /// Bounds on expanding an archive of documents (spec 074 R15).
    pub archive_limits: cobolt_kb::convert::Limits,
    /// `<app>/assets/models` — where the built-in model is cached (R23a).
    pub models_dir: PathBuf,
}

/// What a worker is asked to do.
#[derive(Debug, Clone)]
pub(crate) enum KbOp {
    Refresh,
    Reindex,
    Put { name: String, bytes: Vec<u8> },
    Import { source: PathBuf, name: String },
    Delete { name: String },
    Search { query: String, max: usize },
    FetchModel,
}

/// The application's folder: where relative Knowledge Base paths start. The
/// built binary and `rcrun` both anchor it; a bare interpreter falls back to
/// the working directory.
pub(crate) fn app_base() -> PathBuf {
    cobolt_forms::assets::current_base()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// A control's `Location`, resolved against the application's folder when
/// relative. Joined directly — never through `assets::resolve`, which falls
/// back to a relative path when the folder does not exist yet.
pub(crate) fn resolve_location(location: &str) -> PathBuf {
    let raw = location.trim();
    let raw = if raw.is_empty() { "assets/KB" } else { raw };
    let p = Path::new(raw);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        app_base().join(p)
    }
}

/// An embedder that cannot run here, saying why every time it is asked. Its
/// stamp is the one it stands for, so the collection is never re-stamped by
/// the stand-in.
struct Unavailable {
    stamp: String,
    why: String,
}

impl Embedder for Unavailable {
    fn stamp(&self) -> String {
        self.stamp.clone()
    }
    fn embed(&self, _: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        Err(self.why.clone())
    }
}

/// Delegates to a shared embedder (the built-in model is loaded once per
/// process and shared by every control).
struct Shared(Arc<dyn Embedder>);

impl Embedder for Shared {
    fn stamp(&self) -> String {
        self.0.stamp()
    }
    fn embed(&self, t: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        self.0.embed(t)
    }
    fn embed_query(&self, t: &str) -> Result<Vec<f32>, String> {
        self.0.embed_query(t)
    }
}

/// The embedder a control's configuration names.
pub(crate) fn make_embedder(cfg: &KbConfig) -> Box<dyn Embedder> {
    match cfg.embedder {
        EmbedderChoice::Lexical => Box::new(HashingEmbedder),
        EmbedderChoice::Endpoint => Box::new(EndpointEmbedder {
            api: EndpointApi::from_name(&cfg.api),
            url: cfg.url.clone(),
            model: cfg.model.clone(),
            key: (!cfg.key.trim().is_empty()).then(|| cfg.key.clone()),
            timeout_ms: 30_000,
            transport: Arc::new(RuntimeTransport),
        }),
        EmbedderChoice::Builtin => builtin(&cfg.models_dir),
    }
}

#[cfg(feature = "kb-semantic")]
fn builtin(models_dir: &Path) -> Box<dyn Embedder> {
    use std::collections::HashMap;
    static LOADED: OnceLock<Mutex<HashMap<PathBuf, Arc<dyn Embedder>>>> = OnceLock::new();
    let cache = LOADED.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(e) = cache.lock().ok().and_then(|m| m.get(models_dir).cloned()) {
        return Box::new(Shared(e));
    }
    match cobolt_kb::semantic::BuiltinEmbedder::load(models_dir) {
        Ok(model) => {
            let shared: Arc<dyn Embedder> = Arc::new(model);
            if let Ok(mut m) = cache.lock() {
                m.insert(models_dir.to_path_buf(), shared.clone());
            }
            Box::new(Shared(shared))
        }
        Err(why) => Box::new(Unavailable {
            stamp: cobolt_kb::model::BUILTIN_STAMP.into(),
            why,
        }),
    }
}

#[cfg(not(feature = "kb-semantic"))]
fn builtin(_models_dir: &Path) -> Box<dyn Embedder> {
    // Keeps the unused-import lints quiet in this configuration.
    let _ = (&OnceLock::<()>::new(), &Mutex::new(()), Shared);
    Box::new(Unavailable {
        stamp: cobolt_kb::model::BUILTIN_STAMP.into(),
        why: "this application was built without the built-in semantic model \
              (set [rag] embedder = \"builtin\" in the project to include it)"
            .into(),
    })
}

/// Run one operation to completion on the calling (worker) thread, reporting
/// through `send`.
pub(crate) fn run(cfg: KbConfig, op: KbOp, cancel: Arc<AtomicBool>, send: impl Fn(AsyncOutcome)) {
    let final_outcome = match run_inner(&cfg, op, &cancel, &send) {
        Ok(outcome) => outcome,
        Err(KbError::Busy(_)) => AsyncOutcome::KbBusy {
            message: KbError::Busy(cfg.write_wait).to_string(),
        },
        Err(e) => AsyncOutcome::KbError { message: e.to_string() },
    };
    send(final_outcome);
}

fn run_inner(
    cfg: &KbConfig,
    op: KbOp,
    cancel: &AtomicBool,
    send: &impl Fn(AsyncOutcome),
) -> Result<AsyncOutcome, KbError> {
    let mut last = Instant::now() - PROGRESS_EVERY;
    let mut progress = |p: &Progress| {
        if p.current >= p.total || last.elapsed() >= PROGRESS_EVERY {
            last = Instant::now();
            send(AsyncOutcome::KbProgress {
                document: p.document.clone(),
                current: p.current,
                total: p.total,
            });
        }
    };
    if let KbOp::FetchModel = op {
        let mut report = |file: &str, got: u64, total: Option<u64>| {
            if last.elapsed() >= PROGRESS_EVERY || total == Some(got) {
                last = Instant::now();
                send(AsyncOutcome::KbProgress {
                    document: file.to_string(),
                    current: (got / 1024) as usize,
                    total: total.map_or(0, |t| (t / 1024) as usize),
                });
            }
        };
        cobolt_kb::model::fetch_model(&cfg.models_dir, &RuntimeTransport, &mut report, cancel)
            .map_err(KbError::Io)?;
        return Ok(indexed(Outcome::default()));
    }
    if cfg.collection.trim().is_empty() {
        return Err(KbError::InvalidName(
            "no collection is set — assign the control's Collection first".into(),
        ));
    }
    let mut collection = Collection::open(&cfg.location, cfg.collection.trim())?;
    collection.set_write_wait(cfg.write_wait);
    let embedder = make_embedder(cfg);
    let conv = Converters::with_limits(cfg.archive_limits);
    let outcome = match op {
        KbOp::Refresh => refresh::refresh(&collection, &*embedder, &conv, Scope::All, &mut progress, cancel)?,
        KbOp::Reindex => refresh::reindex(&collection, &*embedder, &conv, &mut progress, cancel)?,
        KbOp::Put { name, bytes } => refresh::put(&collection, &name, &bytes, &*embedder, &conv, &mut progress, cancel)?,
        KbOp::Import { source, name } => {
            let bytes = std::fs::read(&source)
                .map_err(|e| KbError::Io(format!("could not read {}: {e}", source.display())))?;
            refresh::put(&collection, &name, &bytes, &*embedder, &conv, &mut progress, cancel)?
        }
        KbOp::Delete { name } => refresh::delete(&collection, &name, &*embedder, &conv, &mut progress, cancel)?,
        KbOp::Search { query, max } => {
            let r = cobolt_kb::search::search(&collection, &*embedder, &query, max.max(1))?;
            return Ok(AsyncOutcome::KbSearchDone {
                hits: r
                    .hits
                    .into_iter()
                    .map(|h| KbHit {
                        document: h.document,
                        heading: h.heading,
                        passage: h.passage,
                        score: h.score,
                    })
                    .collect(),
                mode: r.mode.as_str().to_string(),
                reason: r.reason.unwrap_or_default(),
            });
        }
        KbOp::FetchModel => unreachable!("handled above"),
    };
    if cancel.load(Ordering::Relaxed) {
        return Err(KbError::Cancelled);
    }
    Ok(indexed(outcome))
}

fn indexed(o: Outcome) -> AsyncOutcome {
    AsyncOutcome::KbIndexed {
        added: o.added,
        updated: o.updated,
        removed: o.removed,
        skipped: o
            .skipped
            .into_iter()
            // The code first, for a program to translate (spec 074 §6).
            .map(|(doc, skip)| format!("{doc}: {} ({})", skip.code, skip.message))
            .collect(),
        note: match (o.text_only, o.embedder_error) {
            (true, _) => "stored text-only: this collection was indexed with another embedder".into(),
            (false, Some(e)) => format!("stored text-only for now: {e}"),
            (false, None) => String::new(),
        },
    }
}

/// Synchronous helpers for the methods that only read folders.
pub(crate) fn list_collections(cfg: &KbConfig) -> Vec<String> {
    cobolt_kb::store::list_collections(&cfg.location)
}

pub(crate) fn create_collection(cfg: &KbConfig, name: &str) -> Result<(), String> {
    Collection::open(&cfg.location, name.trim()).map(|_| ()).map_err(|e| e.to_string())
}

pub(crate) fn remove_collection(cfg: &KbConfig, name: &str) -> Result<(), String> {
    cobolt_kb::store::remove_collection(&cfg.location, name.trim())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub(crate) fn list_documents(cfg: &KbConfig) -> Result<Vec<String>, String> {
    let c = Collection::open(&cfg.location, cfg.collection.trim()).map_err(|e| e.to_string())?;
    refresh::list_documents(&c).map_err(|e| e.to_string())
}
