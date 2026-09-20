// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Viewer's **live** per-instance state (spec 058) — the part that owns
//! a thread and a decode cache, neither of which `cobolt-forms` may hold.
//!
//! `cobolt-forms` is a pure renderer: given controls and state, it paints. It
//! has no clock, spawns nothing, and owns nothing that outlives a frame. A
//! Viewer's decode work does — a background thread that keeps taking jobs for
//! as long as the control exists, and a decoded-page cache bounded by a
//! budget, not by the source document's size (R2). So both live here, exactly
//! where `SnackbarStack` (spec 055) already established this boundary: the
//! host owns cross-frame state, and the engine is handed only what to draw
//! this frame (plan.md §1, §4).
//!
//! Both live surfaces consume this crate, so `rcrun run-form` and a compiled
//! binary get one implementation and cannot drift (R25).
//!
//! # Why a dedicated thread per instance, not a shared pool
//!
//! `cobolt-runtime`'s `spawn_rest_op` already spawns a thread per ASYNC CALL —
//! but it is thread-per-call, ephemeral, and gone the moment that one call
//! finishes. A Viewer's decode work is not one call: it keeps taking jobs
//! (index the document, decode a page, index the next one) for as long as the
//! control is alive, so it needs a thread that *persists*, the shape
//! `doc_viewer.rs`'s `start_preparer`/`prepare_thread` already proves for the
//! IDE's own Documentation panel — generalised here from that one singleton
//! to N (plan.md §1, §5 risk).
//!
//! # The clock is a parameter, where one is needed
//!
//! Nothing here calls `Instant::now()` on its own initiative for anything a
//! test needs to drive — cache eviction is by INSERTION ORDER (an LRU), never
//! by wall-clock age, so tests stay deterministic without a fabricated clock
//! (`SnackbarStack`'s own reason for taking `now` as a parameter doesn't apply
//! here, because nothing in this file's own logic reads real time).

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

/// One page's decoded, ready-to-paint content.
///
/// A placeholder shape until Stage C wires a real format decoder in — what
/// matters for T6/T7 is the *cache* and *thread* mechanism around this type,
/// not its payload, which later tasks replace without touching either.
#[derive(Debug, Clone)]
pub struct DecodedPage {
    pub index: usize,
    /// Stands in for the real layout model Stage C produces per format.
    pub placeholder_len: usize,
}

/// A unit of work the decode thread can be asked to do.
#[derive(Debug, Clone)]
pub enum ViewerJob {
    /// Decode one page (the T6/T7 mechanism's own stand-in job).
    DecodePage { index: usize },
    /// **R5/R5.1's real work**: open `source`, index it, decode `page`, and
    /// decode a bounded run of page previews for the filmstrip and the card
    /// grid — all of it on this control's own thread.
    ///
    /// This is the job `spec.md`'s headline user story turns on: indexing a
    /// two-gigabyte log takes seconds, and doing it here is the difference
    /// between a form that keeps painting and one that stops dead.
    OpenDocument { source: String, page: usize },
}

/// How many page previews one open decodes. Bounded on purpose (R2): the
/// filmstrip and the card grid can only show a handful at once, and a
/// thirty-two-thousand-page log must not cost thirty-two thousand decodes.
pub const PREVIEW_WINDOW: usize = 24;
/// The longest preview text kept per page.
pub const PREVIEW_CHARS: usize = 400;

/// What the decode thread reports back.
///
/// Not `Debug`: a finished document carries a decoded page (and, for an
/// image, its pixels), and formatting that into a log line would be a
/// megabyte nobody asked for.
#[derive(Clone)]
pub enum ViewerDone {
    PageReady(DecodedPage),
    /// A document finished opening — or failed to, with the reason.
    Document {
        source: String,
        result: Result<std::sync::Arc<cobolt_forms::paint::ViewerDocument>, String>,
    },
}

/// Open a document and build what the paint needs from it.
///
/// A free function, and deliberately: it runs on the worker thread and must
/// touch nothing the UI thread owns. Everything it needs arrives in its
/// arguments and everything it produces leaves down the channel.
fn open_document_for_paint(
    source: &str,
    page: usize,
) -> Result<cobolt_forms::paint::ViewerDocument, String> {
    use cobolt_forms::paint::ViewerPageContent;
    use cobolt_forms::viewer::{self, DocumentSource, ViewerFormat};

    // This runs on the document worker, which exists precisely to wait for slow
    // I/O — so a `http(s)://` Source is downloaded HERE, synchronously, and
    // behaves like a slow disk. Its failure travels the channel every other
    // open failure travels, and so reaches `LastError` and `onError`.
    let resolved = cobolt_forms::viewer_remote::local_path_blocking(source)
        .map_err(|e| format!("could not fetch document: {e}"))?;
    let bytes = std::fs::read(&resolved).map_err(|e| format!("could not read document: {e}"))?;
    let head = &bytes[..bytes.len().min(4096)];
    let format = viewer::detect_format(Some(source), head)
        .ok_or_else(|| "unsupported document format".to_string())?;
    let doc_source = DocumentSource::Path(resolved.to_string_lossy().into_owned());

    let mut previews = HashMap::new();
    let (content, page_count) = match format {
        ViewerFormat::Text => {
            // THE expensive step, and the reason this whole file exists: one
            // sequential pass over the source, here rather than in a paint.
            let index = viewer::index_text(&doc_source).map_err(|e| e.to_string())?;
            let count = index.page_count().max(1);
            let wanted = page.min(count.saturating_sub(1));
            let span = index.pages.get(wanted).copied().unwrap_or(viewer::PageSpan { start: 0, end: 0 });
            let text = viewer::decode_text_page(&doc_source, span).map_err(|e| e.to_string())?;
            // A bounded window of previews AROUND the page in view — what a
            // filmstrip beside it can actually show.
            let first = wanted.saturating_sub(PREVIEW_WINDOW / 2);
            for i in first..(first + PREVIEW_WINDOW).min(count) {
                let Some(s) = index.pages.get(i) else { continue };
                if let Ok(t) = viewer::decode_text_page(&doc_source, *s) {
                    previews.insert(i, t.chars().take(PREVIEW_CHARS).collect());
                }
            }
            (ViewerPageContent::Text(text), count)
        }
        ViewerFormat::Markdown => {
            let raw = String::from_utf8_lossy(&bytes).into_owned();
            let parsed = viewer::parse_markdown(&raw);
            (ViewerPageContent::Markdown { raw, doc: parsed }, 1)
        }
        ViewerFormat::HtmlSubset => {
            let raw = String::from_utf8_lossy(&bytes).into_owned();
            let parsed = viewer::parse_html(&raw);
            (ViewerPageContent::Markdown { raw, doc: parsed }, 1)
        }
        ViewerFormat::Pdf => {
            let pdf = viewer::parse_pdf(&bytes)?;
            let count = pdf.page_count().max(1);
            for p in &pdf.pages {
                let i = (p.number as usize).saturating_sub(1);
                if i < PREVIEW_WINDOW {
                    previews.insert(i, p.text.chars().take(PREVIEW_CHARS).collect());
                }
            }
            (ViewerPageContent::Pdf(pdf), count)
        }
        ViewerFormat::Image => (ViewerPageContent::Image(viewer::decode_image(&bytes)?), 1),
    };

    Ok(cobolt_forms::paint::ViewerDocument {
        format,
        page_count,
        current_page: page.min(page_count.saturating_sub(1)),
        content: std::sync::Arc::new(content),
        previews,
    })
}

/// An LRU cache of decoded pages, bounded by a page COUNT budget — not by the
/// source document's size, which is exactly what R2 requires ("peak memory
/// shall be a function of the window, not of document size").
///
/// Modeled on `indexed_disk.rs`'s bounded directory-cache philosophy (decode
/// on demand, evict least-recently-viewed once the budget is exceeded) —
/// the closest existing precedent in this codebase for "bounded regardless of
/// source size," even though that one bounds raw B-tree pages and this one
/// bounds decoded rendering output (plan.md §5 risk).
#[derive(Debug)]
pub struct BoundedPageCache {
    budget: usize,
    // Insertion/access order, oldest (least-recently-used) at the front.
    order: VecDeque<usize>,
    pages: HashMap<usize, DecodedPage>,
}

impl BoundedPageCache {
    /// `budget` is the maximum number of decoded pages held at once. A budget
    /// of `0` is treated as `1` — a cache that can hold nothing is not a
    /// cache, it is a decoder called twice for every page.
    pub fn new(budget: usize) -> Self {
        Self {
            budget: budget.max(1),
            order: VecDeque::new(),
            pages: HashMap::new(),
        }
    }

    pub fn budget(&self) -> usize {
        self.budget
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// A page already in the cache, marked as just-used (moved to the
    /// most-recently-used end) — a `get` is itself an access.
    pub fn get(&mut self, index: usize) -> Option<&DecodedPage> {
        if self.pages.contains_key(&index) {
            self.touch(index);
        }
        self.pages.get(&index)
    }

    /// Insert a freshly decoded page, evicting the least-recently-used one
    /// first if the budget would otherwise be exceeded. Re-inserting a page
    /// already present just marks it as most-recently-used.
    ///
    /// Returns the evicted page's index, if one was evicted — callers that
    /// want to *report* what was dropped (this project's "no silent
    /// truncation" standing rule) have it without a second lookup.
    pub fn insert(&mut self, page: DecodedPage) -> Option<usize> {
        let index = page.index;
        let evicted = if self.pages.contains_key(&index) {
            None
        } else if self.pages.len() >= self.budget {
            self.evict_lru()
        } else {
            None
        };
        self.pages.insert(index, page);
        self.touch(index);
        evicted
    }

    fn touch(&mut self, index: usize) {
        self.order.retain(|&i| i != index);
        self.order.push_back(index);
    }

    fn evict_lru(&mut self) -> Option<usize> {
        let lru = self.order.pop_front()?;
        self.pages.remove(&lru);
        Some(lru)
    }
}

/// One view's own position within a document — `View1`/`View2` in the
/// control's COBOL-facing properties (plan.md §3/§4's naming decision).
#[derive(Debug, Clone, Default)]
pub struct ViewState {
    pub source: String,
    pub page: i64,
    pub zoom: i64,
    pub scroll_position: i64,
    /// R21.1 — the decoded document this view is looking at. Two views on
    /// the same path hold **the same `Arc`**, never two decodes of one
    /// file.
    pub document: Option<Arc<SharedDocument>>,
    /// R21.2 — each view's Find is fully independent: its own query, its
    /// own toggles, its own current match. It falls out of living here
    /// rather than being a special case anyone has to maintain.
    pub search: SearchState,
}

/// One view's Find state (R21.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchState {
    pub text: String,
    pub case_sensitive: bool,
    pub highlight_enabled: bool,
    pub current_match: usize,
    pub match_count: usize,
}

/// A decoded document, shared by however many views are looking at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedDocument {
    pub path: String,
    pub page_count: usize,
}

/// R21.1 — "setting a view's source to the document already open in the
/// other view must not reload or re-decode it."
///
/// Every open document is held behind an `Arc`, keyed by its path. A view
/// asking for a path someone already holds gets a clone of that handle; only
/// a path nothing holds is decoded at all. "Attach, don't reload" is
/// therefore enforced **by construction** — there is no runtime check to
/// forget, and no second code path where a reload could creep back in
/// (plan.md §4, §5's own flagged risk).
#[derive(Debug, Default)]
pub struct DocumentRegistry {
    open: HashMap<String, Arc<SharedDocument>>,
    decodes: usize,
}

impl DocumentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// The handle for `path`, decoding it **only** if nothing holds it yet.
    pub fn attach(
        &mut self,
        path: &str,
        decode: impl FnOnce() -> SharedDocument,
    ) -> Arc<SharedDocument> {
        if let Some(existing) = self.open.get(path) {
            return Arc::clone(existing);
        }
        self.decodes += 1;
        let doc = Arc::new(decode());
        self.open.insert(path.to_string(), Arc::clone(&doc));
        doc
    }

    /// How many decodes have actually happened — AC8's own assertion is
    /// against this number, not against a belief about the code.
    pub fn decode_count(&self) -> usize {
        self.decodes
    }

    /// Documents no view holds any more are dropped, so closing a view
    /// releases its decode rather than leaking it for the session's life.
    pub fn release_unused(&mut self) -> usize {
        let before = self.open.len();
        self.open.retain(|_, doc| Arc::strong_count(doc) > 1);
        before - self.open.len()
    }

    pub fn open_count(&self) -> usize {
        self.open.len()
    }
}

/// A history entry (spec 058 §8.8) — an id and a title, **never** a
/// conversation's rendered content. Selecting one always re-asks the host
/// (`onConversationSelected`) rather than restoring anything cached here,
/// which is what keeps a long Streamed-layout session's memory bounded the
/// same way [`BoundedPageCache`] bounds a single large document.
///
/// The type and the cap are `cobolt-forms`' own, so the host and the
/// interpreter cannot disagree about what history means or how big it gets
/// — T36 has to satisfy tests in both crates, and two implementations of
/// one rule is how they drift.
pub use cobolt_forms::viewer::{ConversationEntry as HistoryEntry, HISTORY_CAP};

/// One Viewer control instance's live state: its dedicated background
/// thread, the channel pair to talk to it, its bounded decode cache, up to
/// two independent views, and (for Streamed layout, §8.8) its conversation
/// history.
pub struct ViewerSession {
    ctrl_id: String,
    thread: Option<JoinHandle<()>>,
    jobs_tx: Option<Sender<ViewerJob>>,
    done_rx: Receiver<ViewerDone>,
    pub cache: BoundedPageCache,
    pub views: [ViewState; 2],
    /// R21.1's "attach, don't reload", shared by both views.
    pub documents: DocumentRegistry,
    /// What the worker has finished, by source path — what the paint reads
    /// through `FormState::viewer_document`.
    documents_ready: HashMap<String, std::sync::Arc<cobolt_forms::paint::ViewerDocument>>,
    /// `(source, page)` already asked for, so one change is one decode.
    requested: HashMap<String, usize>,
    last_error: Option<(String, String)>,
    history: cobolt_forms::viewer::ConversationHistory,
}

/// The default decode-cache budget: how many pages stay resident at once.
/// Deliberately small — R2's whole point is that this number, not the
/// document's page count, is what bounds memory.
pub const DEFAULT_CACHE_BUDGET: usize = 32;

impl ViewerSession {
    /// Spawns the dedicated background thread now. `ctrl_id` names the
    /// thread (`viewer-<ctrl_id>`) so a hung or slow decode is identifiable
    /// in a thread dump without guessing which control owns it.
    pub fn new(ctrl_id: impl Into<String>) -> Self {
        let ctrl_id = ctrl_id.into();
        let (jobs_tx, jobs_rx) = mpsc::channel::<ViewerJob>();
        let (done_tx, done_rx) = mpsc::channel::<ViewerDone>();

        // The worker's own loop: take jobs until the sender is dropped, then
        // exit. No `Instant::now()`, no sleep — it is driven entirely by
        // what arrives on `jobs_rx`, so shutdown is exactly "stop sending
        // jobs and let the channel close," never a signal this thread has to
        // poll for.
        let thread = thread::Builder::new()
            .name(format!("viewer-{ctrl_id}"))
            .spawn(move || {
                for job in jobs_rx.iter() {
                    match job {
                        ViewerJob::DecodePage { index } => {
                            let _ = done_tx.send(ViewerDone::PageReady(DecodedPage {
                                index,
                                placeholder_len: 0,
                            }));
                        }
                        ViewerJob::OpenDocument { source, page } => {
                            let result = open_document_for_paint(&source, page)
                                .map(std::sync::Arc::new);
                            let _ = done_tx.send(ViewerDone::Document { source, result });
                        }
                    }
                }
            })
            .ok();

        Self {
            ctrl_id,
            thread,
            jobs_tx: Some(jobs_tx),
            done_rx,
            cache: BoundedPageCache::new(DEFAULT_CACHE_BUDGET),
            views: [ViewState::default(), ViewState::default()],
            documents: DocumentRegistry::new(),
            documents_ready: HashMap::new(),
            requested: HashMap::new(),
            last_error: None,
            history: cobolt_forms::viewer::ConversationHistory::new(),
        }
    }

    pub fn ctrl_id(&self) -> &str {
        &self.ctrl_id
    }

    /// Point view `index` at `path`, attaching to an existing decode when
    /// one of this session's views already holds it (R21.1/AC8). Returns
    /// the shared handle.
    pub fn open_in_view(
        &mut self,
        index: usize,
        path: &str,
        decode: impl FnOnce() -> SharedDocument,
    ) -> Arc<SharedDocument> {
        let doc = self.documents.attach(path, decode);
        if let Some(view) = self.views.get_mut(index) {
            view.source = path.to_string();
            view.document = Some(Arc::clone(&doc));
        }
        doc
    }

    /// The worker thread's OS-visible name, for a test (or a thread dump) to
    /// tell two sessions apart. `None` only if the spawn itself failed.
    pub fn thread_name(&self) -> Option<String> {
        self.thread
            .as_ref()
            .map(|h| h.thread().name().unwrap_or("").to_owned())
    }

    /// Queue a job for the background thread. Silently dropped if the
    /// session is already shutting down (`jobs_tx` taken) — a job for a
    /// session on its way out has nowhere useful to land.
    pub fn submit(&self, job: ViewerJob) {
        if let Some(tx) = &self.jobs_tx {
            let _ = tx.send(job);
        }
    }

    /// Non-blocking drain of whatever the worker has finished since the last
    /// call, applying each into [`Self::cache`]. Returns the evicted page
    /// indices, if the cache's budget dropped any — the caller (the render
    /// loop, once Stage C wires it in) is expected to report these, not
    /// silently lose them.
    pub fn drain_completed(&mut self) -> Vec<usize> {
        let mut evicted = Vec::new();
        while let Ok(msg) = self.done_rx.try_recv() {
            match msg {
                ViewerDone::PageReady(page) => {
                    if let Some(dropped) = self.cache.insert(page) {
                        evicted.push(dropped);
                    }
                }
                ViewerDone::Document { source, result } => match result {
                    Ok(doc) => {
                        self.last_error = None;
                        self.documents_ready.insert(source, doc);
                    }
                    // A document that will not open is R4's business, not a
                    // reason to lose the one already on screen: the previous
                    // entry stays exactly where it is.
                    Err(why) => self.last_error = Some((source, why)),
                },
            }
        }
        evicted
    }

    /// The decoded document for `source`, if the worker has finished it.
    pub fn document(&self, source: &str) -> Option<std::sync::Arc<cobolt_forms::paint::ViewerDocument>> {
        self.documents_ready.get(source).cloned()
    }

    /// Ask for `source` at `page`, unless that exact request is already in
    /// flight or already answered.
    ///
    /// The guard is what keeps this a decode per CHANGE rather than a decode
    /// per frame — sixty full indexes a second would be worse than the
    /// synchronous read it replaces.
    pub fn request(&mut self, source: &str, page: usize) {
        if source.trim().is_empty() {
            return;
        }
        if self.requested.get(source) == Some(&page) {
            return;
        }
        self.requested.insert(source.to_string(), page);
        self.submit(ViewerJob::OpenDocument { source: source.to_string(), page });
    }

    /// Every document this session's worker has finished, by source path.
    pub fn ready_documents(
        &self,
    ) -> impl Iterator<Item = (String, std::sync::Arc<cobolt_forms::paint::ViewerDocument>)> + '_ {
        self.documents_ready.iter().map(|(k, v)| (k.clone(), v.clone()))
    }

    /// The last open that failed, as `(source, reason)`.
    pub fn last_error(&self) -> Option<&(String, String)> {
        self.last_error.as_ref()
    }

    /// Drop the jobs channel (unblocking the worker's `for job in
    /// jobs_rx.iter()` loop) and join it, returning once the thread has
    /// actually exited.
    ///
    /// **This blocks the calling thread** until the worker exits — which is
    /// fast here (the loop has nothing left to do once its channel closes),
    /// but it is still a join, so it is never called from the UI thread's own
    /// per-frame path. Ordinary disposal (a control removed, a form closed)
    /// drops the `ViewerSession` and lets its `jobs_tx` close on its own,
    /// the same detach-don't-join shape `DebugRunner::stop()` already uses,
    /// for the same reason. This method exists for callers that must know
    /// the thread is actually gone — a test proving the mechanism, or a
    /// controlled application-exit path.
    pub fn shutdown_and_join(mut self) -> std::thread::Result<()> {
        self.jobs_tx = None; // dropping the sender closes the channel
        match self.thread.take() {
            Some(handle) => handle.join(),
            None => Ok(()),
        }
    }

    // ── §8.8 conversation history ───────────────────────────────────────

    /// Archives the current content into history if `has_content` is true
    /// (a no-op on an already-empty pane creates no spurious entry — AC26),
    /// then clears it via a fresh entry becoming current — the caller
    /// (`NewConversation()`/`SelectConversation(id)`, wired in a later task)
    /// decides what "current" means; this method's only job is the history
    /// side-effect and its bound.
    pub fn archive_current(&mut self, has_content: bool, entry: HistoryEntry) -> Option<String> {
        if !has_content {
            return None;
        }
        self.history.push(entry)
    }

    /// §8.8's `SelectConversation(id)`: the currently open conversation is
    /// archived exactly as `NewConversation()` does it, and the named entry
    /// is **removed from history** as it becomes current.
    ///
    /// Returns the entry that was selected, if history held it. **No
    /// content is read back from anywhere** — there is none to read: an
    /// entry is an id and a title, and the pane is refilled only by the
    /// host's own subsequent append calls.
    pub fn select_conversation(
        &mut self,
        id: &str,
        current: Option<HistoryEntry>,
    ) -> Option<HistoryEntry> {
        if let Some(open) = current {
            self.history.push(open);
        }
        self.history.take(id)
    }

    /// §8.8's `HistoryList`, one `id|title` per line.
    pub fn history_list(&self) -> String {
        self.history.to_list()
    }

    /// Every current entry's id, oldest first.
    pub fn history_ids(&self) -> Vec<String> {
        self.history.ids()
    }

    /// Seeds a shell entry (`RegisterConversation`, §8.8) with no content —
    /// content is only ever fetched on selection, never held here.
    pub fn register_history_entry(&mut self, entry: HistoryEntry) -> Option<String> {
        self.history.push(entry)
    }

    /// Removes and returns the entry with `id`, for `SelectConversation(id)`
    /// to promote to "current" — removed, not merely found, because a
    /// selected entry is no longer history, it is the open conversation.
    pub fn take_history_entry(&mut self, id: &str) -> Option<HistoryEntry> {
        self.history.take(id)
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── T6: thread lifecycle ────────────────────────────────────────────

    /// Spec 058 R5.1/AC21 — each Viewer instance gets its OWN dedicated
    /// thread, not a pool shared across instances. Reported by name, since
    /// that is what actually distinguishes them in a thread dump.
    #[test]
    fn two_sessions_get_two_distinct_named_threads() {
        let a = ViewerSession::new("VWR-A");
        let b = ViewerSession::new("VWR-B");
        let name_a = a.thread_name().expect("session A's thread must spawn");
        let name_b = b.thread_name().expect("session B's thread must spawn");
        println!("thread A = {name_a:?}, thread B = {name_b:?}");
        assert_ne!(name_a, name_b, "two sessions must not share one thread");
        assert_eq!(name_a, "viewer-VWR-A");
        assert_eq!(name_b, "viewer-VWR-B");
    }

    /// Spec 058 plan §5 — shutdown is a measured completion signal, never a
    /// sleep. `shutdown_and_join` blocking successfully IS that signal: if
    /// the worker never exits, this test hangs rather than passing on a
    /// guess, which is the honest failure mode for a thread-lifecycle bug.
    #[test]
    fn session_thread_exits_on_shutdown() {
        let session = ViewerSession::new("VWR-1");
        let result = session.shutdown_and_join();
        assert!(result.is_ok(), "the worker thread must exit cleanly: {result:?}");
    }

    /// A job submitted before shutdown is still processed — shutdown closes
    /// the channel, it does not discard what was already queued (`for job in
    /// jobs_rx.iter()` drains the channel before it observes the close).
    #[test]
    fn a_submitted_job_completes_before_shutdown_joins() {
        let session = ViewerSession::new("VWR-1");
        session.submit(ViewerJob::DecodePage { index: 3 });
        // Give the worker a chance to run — no sleep needed: join() itself
        // blocks until it has, so this only proves the *message* arrived,
        // not that it arrived quickly.
        let session = {
            // Drain is non-blocking, so poll until the one job lands or the
            // thread proves it never will (via a bound on attempts, not a
            // wall-clock timeout — deterministic either way).
            let mut s = session;
            let mut got = false;
            for _ in 0..10_000 {
                if !s.drain_completed().is_empty() || s.cache.len() > 0 {
                    got = true;
                    break;
                }
                if s.cache.get(3).is_some() {
                    got = true;
                    break;
                }
                std::thread::yield_now();
            }
            assert!(got, "the submitted job must complete");
            s
        };
        assert!(session.shutdown_and_join().is_ok());
    }

    // ── T7: bounded LRU cache ───────────────────────────────────────────

    /// Spec 058 R2/AC1 — peak memory is a function of the CACHE BUDGET, not
    /// of how many pages are inserted. Reports the measured peak vs. the
    /// budget, per this project's quantify-performance rule.
    #[test]
    fn decode_cache_stays_within_its_configured_budget() {
        let budget = 8;
        let mut cache = BoundedPageCache::new(budget);
        for i in 0..1000 {
            cache.insert(DecodedPage { index: i, placeholder_len: 0 });
        }
        println!(
            "decode cache: inserted 1000 pages, budget {budget}, peak resident {}",
            cache.len()
        );
        assert_eq!(
            cache.len(),
            budget,
            "peak resident pages must equal the budget, not the 1000 inserted"
        );
    }

    /// The evicted page is reported, never silently dropped (this project's
    /// standing "no silent truncation" rule).
    #[test]
    fn inserting_past_budget_reports_the_evicted_page() {
        let mut cache = BoundedPageCache::new(2);
        assert_eq!(cache.insert(DecodedPage { index: 0, placeholder_len: 0 }), None);
        assert_eq!(cache.insert(DecodedPage { index: 1, placeholder_len: 0 }), None);
        let evicted = cache.insert(DecodedPage { index: 2, placeholder_len: 0 });
        assert_eq!(evicted, Some(0), "page 0 is the least-recently-used, and must be named");
    }

    /// `get` counts as use — an LRU that evicted a page you just READ, not
    /// one you never touched, is not an LRU.
    #[test]
    fn getting_a_page_protects_it_from_the_next_eviction() {
        let mut cache = BoundedPageCache::new(2);
        cache.insert(DecodedPage { index: 0, placeholder_len: 0 });
        cache.insert(DecodedPage { index: 1, placeholder_len: 0 });
        // Touch page 0 — now page 1 is the least-recently-used.
        assert!(cache.get(0).is_some());
        let evicted = cache.insert(DecodedPage { index: 2, placeholder_len: 0 });
        assert_eq!(evicted, Some(1), "page 1, not the just-read page 0, must be evicted");
        assert!(cache.get(0).is_some(), "the protected page must survive");
    }

    // ── §8.8: history ────────────────────────────────────────────────────

    /// Spec 058 §8.8/AC28 — history never exceeds 10 entries; an 11th
    /// evicts the oldest. Reported by id, per this project's standing rule.
    #[test]
    fn history_never_exceeds_ten_and_evicts_the_oldest() {
        let mut session = ViewerSession::new("VWR-1");
        for i in 0..12 {
            session.register_history_entry(HistoryEntry {
                id: format!("conv-{i}"),
                title: format!("Conversation {i}"),
            });
        }
        let ids: Vec<String> = session.history_ids();
        println!("history after 12 registrations, cap {HISTORY_CAP}: {ids:?}");
        assert_eq!(session.history_len(), HISTORY_CAP);
        assert_eq!(ids.first().map(String::as_str), Some("conv-2"), "conv-0 and conv-1 must be evicted");
        assert_eq!(ids.last().map(String::as_str), Some("conv-11"));
    }

    /// Spec 058 §8.8/AC26 — `NewConversation()` on an already-empty pane
    /// creates no spurious history entry.
    #[test]
    fn archiving_an_empty_pane_creates_no_entry() {
        let mut session = ViewerSession::new("VWR-1");
        session.archive_current(false, HistoryEntry { id: "x".into(), title: "x".into() });
        assert_eq!(session.history_len(), 0);
        session.archive_current(true, HistoryEntry { id: "y".into(), title: "y".into() });
        assert_eq!(session.history_len(), 1);
    }

    /// Spec 058 §8.8/AC27 — selecting a history entry removes it (it is no
    /// longer history, it is the open conversation), never merely reads it.
    #[test]
    fn selecting_a_history_entry_removes_it() {
        let mut session = ViewerSession::new("VWR-1");
        session.register_history_entry(HistoryEntry { id: "a".into(), title: "A".into() });
        session.register_history_entry(HistoryEntry { id: "b".into(), title: "B".into() });
        let taken = session.take_history_entry("a");
        assert_eq!(taken.map(|e| e.id), Some("a".to_owned()));
        assert_eq!(session.history_len(), 1, "the selected entry must leave history");
        assert!(session.take_history_entry("a").is_none(), "it cannot be selected twice");
    }
}

/// Spec 058 T16/T17 — R21.1's "attach, don't reload" and R21.2's
/// independent per-view search, asserted where the state lives.
#[cfg(test)]
mod split_view_tests {
    use super::*;

    fn doc(path: &str, pages: usize) -> SharedDocument {
        SharedDocument { path: path.to_string(), page_count: pages }
    }

    /// **AC8**, the same-document clause — "in the same-document case the
    /// document is decoded once."
    ///
    /// Asserted against a measured decode COUNT, not against a belief about
    /// the code, and against `Arc::ptr_eq` so the two views demonstrably
    /// hold the same decode rather than two equal ones.
    #[test]
    fn two_views_of_one_document_decode_it_exactly_once() {
        let mut session = ViewerSession::new("VWR-1");
        let path = "/reports/quarterly.txt";

        let first = session.open_in_view(0, path, || doc(path, 40));
        let second = session.open_in_view(1, path, || {
            panic!("R21.1: the second view must ATTACH, never decode again")
        });

        println!(
            "two views on {path}: {} decode(s), {} open document(s), same handle = {}",
            session.documents.decode_count(),
            session.documents.open_count(),
            Arc::ptr_eq(&first, &second)
        );
        assert_eq!(session.documents.decode_count(), 1, "AC8: decoded once");
        assert!(Arc::ptr_eq(&first, &second), "both views must hold the SAME decode");
        assert_eq!(first.page_count, 40);
    }

    #[test]
    fn two_views_of_two_documents_decode_each_of_them() {
        let mut session = ViewerSession::new("VWR-1");
        session.open_in_view(0, "/a.txt", || doc("/a.txt", 3));
        session.open_in_view(1, "/b.txt", || doc("/b.txt", 7));
        println!(
            "two different documents: {} decode(s), {} open",
            session.documents.decode_count(),
            session.documents.open_count()
        );
        assert_eq!(session.documents.decode_count(), 2, "two documents are two decodes");
        assert_eq!(session.views[0].document.as_ref().unwrap().page_count, 3);
        assert_eq!(session.views[1].document.as_ref().unwrap().page_count, 7);
    }

    /// Re-pointing a view releases the decode nothing is looking at any
    /// more — "attach, don't reload" must not become "attach, and never let
    /// go".
    #[test]
    fn a_document_no_view_holds_any_more_is_released() {
        let mut session = ViewerSession::new("VWR-1");
        session.open_in_view(0, "/a.txt", || doc("/a.txt", 3));
        session.open_in_view(1, "/a.txt", || doc("/a.txt", 3));
        session.open_in_view(1, "/b.txt", || doc("/b.txt", 5));
        let before = session.documents.open_count();
        // View 0 still holds /a.txt, so nothing is released yet.
        let released_while_held = session.documents.release_unused();
        session.views[0].document = None;
        let released_after = session.documents.release_unused();
        println!(
            "{before} open -> released {released_while_held} while view 1 held it, \
             {released_after} once it let go, {} open now",
            session.documents.open_count()
        );
        assert_eq!(released_while_held, 0, "a document a view is showing is never released");
        assert_eq!(released_after, 1, "one nobody holds is");
        assert_eq!(session.documents.open_count(), 1, "only /b.txt is still open");
    }

    /// **R21.2** — "each view's Find is fully independent: its own search
    /// text, case-sensitivity and highlight toggles, current match and
    /// match count... **including the same-document case of R21.1**."
    #[test]
    fn each_view_keeps_its_own_search_even_on_the_same_document() {
        let mut session = ViewerSession::new("VWR-1");
        let path = "/shared.txt";
        session.open_in_view(0, path, || doc(path, 12));
        session.open_in_view(1, path, || panic!("must attach"));

        session.views[0].search = SearchState {
            text: "invoice".into(),
            case_sensitive: true,
            highlight_enabled: true,
            current_match: 2,
            match_count: 9,
        };
        session.views[1].search = SearchState {
            text: "total".into(),
            case_sensitive: false,
            highlight_enabled: false,
            current_match: 0,
            match_count: 4,
        };

        println!("one document, two searches:");
        for (i, v) in session.views.iter().enumerate() {
            println!(
                "  view {}: {:?} case={} highlight={} match {} of {}",
                i + 1,
                v.search.text,
                v.search.case_sensitive,
                v.search.highlight_enabled,
                v.search.current_match + 1,
                v.search.match_count
            );
        }
        assert_ne!(session.views[0].search, session.views[1].search);
        assert_eq!(session.views[0].search.text, "invoice");
        assert_eq!(session.views[1].search.text, "total");
        assert_eq!(session.documents.decode_count(), 1, "still one decode behind both");

        // Searching one side again must not disturb the other.
        let untouched = session.views[1].search.clone();
        session.views[0].search.current_match = 5;
        session.views[0].search.text = "balance".into();
        println!("after re-searching view 1: view 2 is {:?}", session.views[1].search.text);
        assert_eq!(session.views[1].search, untouched, "R21.2: view 2 is untouched");
    }

    /// AC8's other half at this level: each view's own page/zoom/scroll.
    #[test]
    fn each_view_keeps_its_own_page_zoom_and_scroll() {
        let mut session = ViewerSession::new("VWR-1");
        let path = "/shared.txt";
        session.open_in_view(0, path, || doc(path, 100));
        session.open_in_view(1, path, || panic!("must attach"));
        session.views[0] = ViewState { page: 1, zoom: 100, scroll_position: 0, ..session.views[0].clone() };
        session.views[1] = ViewState { page: 87, zoom: 250, scroll_position: 4200, ..session.views[1].clone() };
        for (i, v) in session.views.iter().enumerate() {
            println!("  view {}: page {} zoom {}% scroll {}", i + 1, v.page, v.zoom, v.scroll_position);
        }
        assert_eq!((session.views[0].page, session.views[0].zoom), (1, 100));
        assert_eq!((session.views[1].page, session.views[1].zoom), (87, 250));
        assert_eq!(session.documents.decode_count(), 1, "one decode, two viewports");
    }
}

/// Spec 058 T36 — §8.8's conversation history, at the host's own level
/// (AC27, AC28). The interpreter has its own tests for the COBOL surface;
/// these are the two plan.md §6 names, asserting the *rules* rather than
/// the plumbing.
#[cfg(test)]
mod conversation_history_tests {
    use super::*;

    fn entry(id: &str, title: &str) -> HistoryEntry {
        HistoryEntry { id: id.to_string(), title: title.to_string() }
    }

    /// **AC28** — "history never exceeds 10 entries; archiving or
    /// registering an 11th evicts the oldest, verified by id."
    #[test]
    fn history_never_exceeds_ten_and_evicts_the_oldest() {
        let mut session = ViewerSession::new("VWR-1");
        let mut evicted = Vec::new();
        for i in 0..12 {
            if let Some(gone) = session.register_history_entry(entry(&format!("c{i}"), &format!("Thread {i}"))) {
                evicted.push(gone);
            }
        }
        let ids = session.history_ids();
        println!("12 archived -> {} kept: {ids:?}", ids.len());
        println!("evicted, in order: {evicted:?}");
        assert_eq!(session.history_len(), HISTORY_CAP, "AC28: never more than {HISTORY_CAP}");
        assert_eq!(evicted, vec!["c0".to_string(), "c1".to_string()], "AC28: the OLDEST, by id");
        assert_eq!(ids.first().map(String::as_str), Some("c2"));
        assert_eq!(ids.last().map(String::as_str), Some("c11"));
        println!("HistoryList:\n{}", session.history_list());
        assert_eq!(session.history_list().lines().count(), HISTORY_CAP, "one id|title per line");
    }

    /// **AC27** — "`SelectConversation(id)` archives the currently-open
    /// conversation, removes the selected id from history... the control
    /// never repaints content from anywhere but a subsequent host-supplied
    /// append call."
    ///
    /// The last clause is the one worth proving structurally: a
    /// `HistoryEntry` has **no content field at all**, so there is nothing
    /// for a selection to restore even if someone tried.
    #[test]
    fn select_conversation_swaps_current_and_history_without_touching_content() {
        let mut session = ViewerSession::new("VWR-1");
        session.register_history_entry(entry("older", "An older thread"));
        session.register_history_entry(entry("target", "The one being opened"));

        let selected = session.select_conversation("target", Some(entry("open-now", "What was on screen")));
        let ids = session.history_ids();
        println!("selected {:?}", selected.as_ref().map(|e| (&e.id, &e.title)));
        println!("history now {ids:?}");
        assert_eq!(selected.as_ref().map(|e| e.id.as_str()), Some("target"), "the entry comes back");
        assert!(!ids.contains(&"target".to_string()), "AC27: and LEAVES history");
        assert!(ids.contains(&"open-now".to_string()), "AC27: what was open is archived");
        assert_eq!(ids, ["older".to_string(), "open-now".to_string()]);

        // §8.8's memory rule, made structural: an entry is an id and a
        // title, and there is no third field for content to hide in.
        let e = selected.unwrap();
        let round_trip = format!("{}|{}", e.id, e.title);
        println!("the whole entry, serialised: {round_trip:?}");
        assert_eq!(round_trip, "target|The one being opened");
    }

    #[test]
    fn archiving_the_same_conversation_twice_moves_it_rather_than_duplicating_it() {
        let mut session = ViewerSession::new("VWR-1");
        session.register_history_entry(entry("a", "Alpha"));
        session.register_history_entry(entry("b", "Beta"));
        session.register_history_entry(entry("a", "Alpha, continued"));
        let ids = session.history_ids();
        println!("after re-archiving 'a': {ids:?}\n{}", session.history_list());
        assert_eq!(ids, ["b".to_string(), "a".to_string()], "moved to the end, not duplicated");
        assert!(session.history_list().contains("a|Alpha, continued"), "with its newer title");
    }

    /// AC26's own clause at this level: archiving an EMPTY pane creates no
    /// entry.
    #[test]
    fn an_empty_pane_is_never_archived() {
        let mut session = ViewerSession::new("VWR-1");
        let evicted = session.archive_current(false, entry("nothing", "Nothing here"));
        println!("archive_current(has_content: false) -> evicted {evicted:?}, history {}", session.history_len());
        assert!(evicted.is_none());
        assert_eq!(session.history_len(), 0, "AC26: no spurious entry");
        session.archive_current(true, entry("real", "Real content"));
        assert_eq!(session.history_len(), 1, "and a real one is archived");
    }
}

/// Spec 058 **R5/R5.1** — the decode really does happen off the calling
/// thread, and a request really is cheap.
///
/// `spec.md`'s headline user story is opening a two-gigabyte log without the
/// form stalling. These tests measure the two halves of that claim: asking
/// costs microseconds, and the answer arrives on a thread that is not this
/// one.
#[cfg(test)]
mod off_thread_tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// A text file of `pages` form-feed-separated pages.
    fn paged_file(dir: &tempfile::TempDir, name: &str, pages: usize, bytes_per_page: usize) -> String {
        use std::io::Write;
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 0..pages {
            let marker = (b'A' + (i % 26) as u8) as char;
            write!(f, "page {i} ").unwrap();
            f.write_all(marker.to_string().repeat(bytes_per_page).as_bytes()).unwrap();
            f.write_all(&[cobolt_forms::viewer::FORM_FEED]).unwrap();
        }
        path.to_string_lossy().into_owned()
    }

    /// Wait for the worker by DRAINING, never by sleeping — a measured
    /// completion signal within a bound, which is this project's house style
    /// for anything timing-sensitive.
    fn wait_for(session: &mut ViewerSession, source: &str, bound: Duration) -> Option<Duration> {
        let start = Instant::now();
        while start.elapsed() < bound {
            session.drain_completed();
            if session.document(source).is_some() {
                return Some(start.elapsed());
            }
            std::thread::yield_now();
        }
        None
    }

    /// **R5.1** — asking for a large document returns at once; the indexing
    /// happens somewhere else.
    ///
    /// The numbers are what matter: if `request()` were doing the work, its
    /// own elapsed time would be the whole decode rather than a rounding
    /// error beside it.
    #[test]
    fn requesting_a_large_document_returns_immediately_and_decodes_off_thread() {
        let dir = tempfile::tempdir().unwrap();
        // ~6 MB across 400 pages: big enough that indexing is measurable,
        // small enough that a test suite stays quick.
        let source = paged_file(&dir, "big.log", 400, 16 * 1024);
        let size = std::fs::metadata(&source).unwrap().len();

        let mut session = ViewerSession::new("VWR-1");
        let asked = Instant::now();
        session.request(&source, 0);
        let ask_cost = asked.elapsed();

        let decoded = wait_for(&mut session, &source, Duration::from_secs(20))
            .expect("the worker must finish within the bound");

        let doc = session.document(&source).expect("and hand back a document");
        println!("R5.1, measured on a {:.1} MB / 400-page document:", size as f64 / 1_048_576.0);
        println!("  request() returned in         {:?}", ask_cost);
        println!("  the worker finished after     {:?}", decoded);
        println!("  pages {}, previews decoded {}", doc.page_count, doc.previews.len());
        assert_eq!(doc.page_count, 400, "the whole document was indexed");
        assert!(
            ask_cost < Duration::from_millis(5),
            "R5.1: asking must not do the work — it took {ask_cost:?}"
        );
        assert!(
            doc.previews.len() <= PREVIEW_WINDOW,
            "R2: previews are bounded, got {}",
            doc.previews.len()
        );
        assert!(!doc.previews.is_empty(), "but the filmstrip does get something to show");
    }

    /// The work happens on **this control's own** thread, named for it
    /// (AC21) — not on the caller's, and not on a pool shared with another
    /// Viewer.
    #[test]
    fn two_sessions_decode_on_two_differently_named_threads_of_their_own() {
        let dir = tempfile::tempdir().unwrap();
        let a = paged_file(&dir, "a.log", 8, 512);
        let b = paged_file(&dir, "b.log", 12, 512);

        let mut one = ViewerSession::new("VWR-1");
        let mut two = ViewerSession::new("VWR-2");
        println!("threads: {:?} and {:?}", one.thread_name(), two.thread_name());
        assert_eq!(one.thread_name().as_deref(), Some("viewer-VWR-1"));
        assert_eq!(two.thread_name().as_deref(), Some("viewer-VWR-2"));
        assert_ne!(one.thread_name(), two.thread_name(), "AC21: one thread each");

        one.request(&a, 0);
        two.request(&b, 0);
        let ta = wait_for(&mut one, &a, Duration::from_secs(10)).expect("VWR-1 finished");
        let tb = wait_for(&mut two, &b, Duration::from_secs(10)).expect("VWR-2 finished");
        println!("VWR-1 {:?} ({} pages), VWR-2 {:?} ({} pages)", ta, one.document(&a).unwrap().page_count, tb, two.document(&b).unwrap().page_count);
        assert_eq!(one.document(&a).unwrap().page_count, 8);
        assert_eq!(two.document(&b).unwrap().page_count, 12);
        assert!(one.document(&b).is_none(), "and neither knows the other's work");
    }

    /// One change is one decode. Asking sixty times a second for the same
    /// thing would be worse than the synchronous read this replaces.
    #[test]
    fn asking_again_for_the_same_page_does_not_decode_again() {
        let dir = tempfile::tempdir().unwrap();
        let source = paged_file(&dir, "steady.log", 20, 1024);
        let mut session = ViewerSession::new("VWR-1");

        session.request(&source, 0);
        wait_for(&mut session, &source, Duration::from_secs(10)).expect("first decode");
        let first = session.document(&source).unwrap();

        // Sixty more frames asking for exactly the same thing.
        for _ in 0..60 {
            session.request(&source, 0);
            session.drain_completed();
        }
        let after = session.document(&source).unwrap();
        println!(
            "60 identical requests later: same decode = {}",
            std::sync::Arc::ptr_eq(&first, &after)
        );
        assert!(
            std::sync::Arc::ptr_eq(&first, &after),
            "the guard must make one change one decode"
        );

        // A different PAGE is a different request, and is answered.
        session.request(&source, 9);
        let start = Instant::now();
        let mut changed = false;
        while start.elapsed() < Duration::from_secs(10) {
            session.drain_completed();
            let now = session.document(&source).unwrap();
            if !std::sync::Arc::ptr_eq(&first, &now) {
                changed = true;
                println!("asking for page 9 produced a new decode, current_page = {}", now.current_page);
                assert_eq!(now.current_page, 9);
                break;
            }
            std::thread::yield_now();
        }
        assert!(changed, "a different page must actually be decoded");
    }

    /// R4 from the host's side: a document that will not open leaves the one
    /// already decoded exactly where it is.
    #[test]
    fn a_failed_open_never_disturbs_the_document_already_decoded() {
        let dir = tempfile::tempdir().unwrap();
        let good = paged_file(&dir, "good.log", 4, 256);
        let bad = dir.path().join("mystery.bin");
        std::fs::write(&bad, (0..256u16).map(|b| (b % 256) as u8).collect::<Vec<u8>>()).unwrap();
        let bad = bad.to_string_lossy().into_owned();

        let mut session = ViewerSession::new("VWR-1");
        session.request(&good, 0);
        wait_for(&mut session, &good, Duration::from_secs(10)).expect("the good one opens");

        session.request(&bad, 0);
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) && session.last_error().is_none() {
            session.drain_completed();
            std::thread::yield_now();
        }
        println!("after the bad open: last_error {:?}", session.last_error());
        println!("the good document is still there: {}", session.document(&good).is_some());
        assert!(session.last_error().is_some(), "the failure is reported");
        assert!(session.document(&bad).is_none(), "and nothing was stored for it");
        assert_eq!(
            session.document(&good).map(|d| d.page_count),
            Some(4),
            "R4: the document already decoded is untouched"
        );
    }
}
