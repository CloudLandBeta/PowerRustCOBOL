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
    /// Decode one page. The real decoder (text/Markdown/image/PDF/HTML) is
    /// Stage C's job; this thread's own job is only to run *something*
    /// off the UI thread and report back through `done_tx`.
    DecodePage { index: usize },
}

/// What the decode thread reports back.
#[derive(Debug, Clone)]
pub enum ViewerDone {
    PageReady(DecodedPage),
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
}

/// A history entry (spec 058 §8.8) — an id and a title, **never** a
/// conversation's rendered content. Selecting one always re-asks the host
/// (`onConversationSelected`) rather than restoring anything cached here,
/// which is what keeps a long Streamed-layout session's memory bounded the
/// same way [`BoundedPageCache`] bounds a single large document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: String,
    pub title: String,
}

/// History holds at most this many entries (spec 058 §8.8); past it, the
/// oldest is evicted.
pub const HISTORY_CAP: usize = 10;

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
    history: VecDeque<HistoryEntry>,
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
                        // Stage C replaces this with the real per-format
                        // decoder; T6/T7's job is only the thread/cache
                        // mechanism around it, proven with a stand-in.
                        ViewerJob::DecodePage { index } => {
                            let _ = done_tx.send(ViewerDone::PageReady(DecodedPage {
                                index,
                                placeholder_len: 0,
                            }));
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
            history: VecDeque::new(),
        }
    }

    pub fn ctrl_id(&self) -> &str {
        &self.ctrl_id
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
            }
        }
        evicted
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
    pub fn archive_current(&mut self, has_content: bool, entry: HistoryEntry) {
        if !has_content {
            return;
        }
        self.history.push_back(entry);
        while self.history.len() > HISTORY_CAP {
            self.history.pop_front();
        }
    }

    /// Seeds a shell entry (`RegisterConversation`, §8.8) with no content —
    /// content is only ever fetched on selection, never held here.
    pub fn register_history_entry(&mut self, entry: HistoryEntry) {
        self.history.push_back(entry);
        while self.history.len() > HISTORY_CAP {
            self.history.pop_front();
        }
    }

    /// Removes and returns the entry with `id`, for `SelectConversation(id)`
    /// to promote to "current" — removed, not merely found, because a
    /// selected entry is no longer history, it is the open conversation.
    pub fn take_history_entry(&mut self, id: &str) -> Option<HistoryEntry> {
        let pos = self.history.iter().position(|e| e.id == id)?;
        self.history.remove(pos)
    }

    /// Every current entry, oldest first — the source `HistoryList` (the
    /// `id|title` multi-line property) reads from.
    pub fn history_entries(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.history.iter()
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
        let ids: Vec<String> = session.history_entries().map(|e| e.id.clone()).collect();
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
