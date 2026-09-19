# Tasks — Viewer control

- **Status:** in progress
- **Plan:** ./plan.md   **Date:** 2026-09-18

Ordered so the tree stays green after every task, following spec.md's own
fidelity order (text/Markdown/images → PDF → Mermaid subset → HTML subset) and
plan.md's dependency chain: scaffolding → the threading engine → Wave 1
formats → Search → Split view → PDF → Mermaid → HTML → conversation mode →
surfaces/docs/KB → finalize. Each stage is independently useful and
independently verifiable — plan §5 flags scope as the main risk, and this
sequencing is the mitigation, same as it was for Snackbar.

> **Branch:** `features` (already the branch spec.md and plan.md were clarified
> and designed on). Commits stay split from any fix work (Golden Rule #5). Do
> not commit or push unless the operator asks.

**Storage principle, corrected by the operator (2026-09-18):** every format's
internal storage is its **own native form** — Markdown stays Markdown text,
HTML stays HTML text, PDF stays its own binary, in memory, on disk and while
displayed alike. Rendering (tables, links, images, formatted text) is a
separate, derived step computed *from* that stored form, never a replacement
for it — plan.md §3/§4 now state this explicitly, and it's what makes R18's
byte-identical Save As correct by construction rather than a special case.

§8's conversation mode is a narrower, additional design point **on top of**
this, not an exception to it: a conversation is an **assembled stream** of
heterogeneous chunks (some HTML, some Markdown-converted, some Raw-escaped),
not one single-format file, so its own storage substrate for stitching that
stream together is HTML — each incoming chunk's mode still governs how it's
folded in (Markdown converted first, Raw escaped). `append_html`/
`append_markdown`/`append_raw` stay sequenced **after** the HTML-subset wave
(Stage H) purely so they can reuse its HTML-to-layout mapping on each appended
chunk instead of building a second, smaller one — an engineering-reuse call,
not a claim about what §8.1 means architecturally.

---

## Stage A — the control exists

- [x] **T1 — Add `ControlType::Viewer`** (R1)
  - Files: `crates/cobolt-forms/src/model.rs`
  - Do: the variant (appended last, never inserted), `as_str()`,
        `supported_events()` (`onError`, `onLoadProgress`, `onLoaded`),
        `default_size()`, `ControlType::ALL`. All four are exhaustive matches —
        the compiler enforces this list.
  - Verify: `cargo build --workspace` fails **only** with the non-exhaustive-
        match errors these four sites don't cover; write that list down — it's
        T2's checklist.

- [x] **T2 — Satisfy every match the compiler named, and the two matches it
      won't** (R1, R22)
  - Files: whatever T1's build listed, **plus explicitly**
        `crates/cobolt-forms/src/model.rs` (`Control::new`'s property-seeding
        match and `from_str()` — **neither is exhaustive, so the compiler
        stays silent if either is skipped**)
  - Do: seed the properties in plan.md §3 with their documented defaults;
        `from_str("Viewer")` → `ControlType::Viewer`.
  - Verify: `cargo test -p cobolt-forms` — **new regression test**: build a
        Viewer control, `save_form`/`load_form` round-trip it, assert the
        reloaded type is `ControlType::Viewer`, not `Custom` (plan §5's
        highest-severity flagged risk — this is the only thing that catches a
        forgotten `from_str` arm). A second test prints the seeded property
        table and confirms every default matches plan §3.

- [x] **T3 — The `control-viewer` icon** (AC10)
  - Files: `crates/cobolt-forms/src/icons.rs`
  - Do: one icon, the existing 24-unit-grid/1.5-unit-stroke treatment.
  - Verify: `cargo test -p cobolt-forms --features render
        every_control_type_has_an_icon` green.

- [x] **T4 — `render_interactive` arm** (R5, AC9)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: an explicit `ControlType::Viewer => { … }` arm — the wildcard fallback
        this match already has silently strips interactivity, and already did
        once for Snackbar/WebSearch/IndexedFile per the comment sitting above
        it. No real content yet; a Viewer that compiles, paints a placeholder,
        and **responds to a click**.
  - Verify: `cargo test -p cobolt-forms --features render` — an *interaction*
        test (simulate a click inside the control's rect, assert something
        observable changes), not just a paint-diff — a paint-diff would pass
        even against the generic fallback painter this task exists to avoid.

- [x] **T5 — Toolbox palette entry** (R1)
  - Files: `crates/cobolt-ide/src/panels/toolbox.rs`
  - Do: the `TOOLS` entry (plain static list — **no test cross-checks its
        length today**, so a forgotten entry compiles clean and Viewer simply
        never appears in the Designer palette).
  - Verify: **new coverage assertion** — `TOOLS.len() == ControlType::ALL.len()
        - 1` (minus `Custom`), or equivalent — closing this gap for every
        future control, not just Viewer. **Manual:** Viewer appears in the
        Designer toolbox and can be dropped on a form.

## Stage B — the host session (threading engine)

- [x] **T6 — `ViewerSession`: spawn, drain, shut down** (R5, R5.1, AC21)
  - Files: `crates/cobolt-form-host/src/viewer_session.rs` (new), `src/lib.rs`,
        `src/host.rs` (`FormBody` owns `viewer_sessions: HashMap<String,
        ViewerSession>`, ticked/drained per frame — the `SnackbarStack`
        precedent)
  - *(Scoping note: the field and its 3 construction sites are wired; the
        per-frame paint-loop tick/drain call is deferred to whichever Stage C
        task first has real content to drain INTO — nothing to paint yet
        means nothing to tick against, the same reasoning T4 applied to its
        own placeholder.)*
  - Do: one named background thread per control instance
        (`thread::Builder::new().name(format!("viewer-{ctrl_id}"))`), a
        jobs/done channel pair, non-blocking bounded per-frame drain +
        `ctx.request_repaint()` — modeled on `doc_viewer.rs`'s
        `start_preparer`/`prepare_thread`, generalized from one instance to N.
        Clean shutdown: drop the jobs channel on disposal so the thread's
        `recv()` unblocks and it exits — the `DebugRunner::stop()` detach
        pattern, never `.join()` on the UI thread.
  - Verify: `cargo test -p cobolt-form-host` — two sessions report two
        distinct thread names (**AC21**); shutdown reports a **measured**
        completion signal within a bound, never a sleep-and-hope.

- [x] **T7 — Bounded decoded-page cache** (R2, AC1)
  - Files: `crates/cobolt-form-host/src/viewer_session.rs`
  - Do: an LRU-evicted, budget-configurable page cache — modeled on
        `indexed_disk.rs`'s bounded directory-cache philosophy (decode on
        demand, evict least-recently-viewed once the budget is exceeded).
  - Verify: `cargo test -p cobolt-form-host` — open a document larger than the
        configured window; **report measured peak resident pages vs. the
        budget** (this project's quantify-performance rule, and directly what
        AC1 demands).

## Stage C — Wave 1: text, Markdown, images (one view, no split, no search yet)

- [x] **T8 — Plain text: load, index, paginate** (R1, R2, R3, R4, R6, R9)
  - Files: `crates/cobolt-forms/src/viewer.rs` (new)
  - Do: `Source`/`LoadBytes` loading; format resolution (content first,
        extension second) into `ViewerFormat`; **the loaded bytes/text are
        retained as the canonical stored representation** — every later
        format wave's layout model is computed *from* this, never a
        replacement for it (plan §3, operator-corrected); page-break indexing;
        `onError`/`onLoadProgress`/`onLoaded` wiring through T6's session.
  - Verify: `cargo test -p cobolt-forms` — a text file larger than the decode
        window opens, jumping to the last page stays within the cache budget
        (**AC1**, tied to T7); an unsupported format raises `onError` and
        leaves any prior document displayed (R4).

- [x] **T9 — Markdown walker → internal layout model** (R7, R9, AC2)
  - Files: `crates/cobolt-forms/src/viewer.rs`, `Cargo.toml` (`pulldown-cmark`,
        matching `cobolt-ide`'s `0.12`, `default-features = false`)
  - Do: walk `pulldown-cmark` events into a layout model — paragraphs,
        headings, lists, tables, images, task lists, footnotes, strikethrough
        (§3's "common extensions"). Fresh code, informed by but not copied
        from `cobolt-ide`'s unreachable `md_render.rs`.
  - Verify: `cargo test -p cobolt-forms` — reports node counts per construct
        checked; each of the "common extensions" has its own case.

- [x] **T10 — Image decoding** (R7, AC2)
  - Files: `crates/cobolt-forms/src/viewer.rs`, `Cargo.toml` (extend `image`
        features: `gif`, `webp`, `bmp`, `tiff` — `png`/`jpeg` and `resvg`/SVG
        already present)
  - Do: dispatch by resolved format; reuse `paint.rs`'s existing
        `decode_image_bytes`/`decode_svg_bytes` where the format overlaps
        with PictureBox's pipeline. **Verify first** whether `cobolt-forms`
        already depends on `cobolt-media` for the Animator control (plan §5,
        unconfirmed) — reuse it for GIF/WebP/APNG playback if so, otherwise
        the minimum frame-timing needed on top of `image`'s own decode.
  - Verify: `cargo test -p cobolt-forms` — every format in §3's table decodes,
        reporting dimensions/frame counts per format, checked against what §3
        promises (nothing silently over- or under-delivered).

- [x] **T11 — `paint::draw_viewer`: layouts, and the design-canvas preview**
      (R7, R8, R10, R32, AC3, AC11)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: the four non-Streamed `Layout` modes (`Raw`/`Web`/`Print`/`Page`)
        painting text, Markdown and images; paper border/shadow for `Print`/
        `Page`; `FontSize` independent of `Zoom`. **Separately**, the
        design-canvas preview path: synchronous, first-page-only, time-boxed
        — it cannot use T6's session (the static canvas runs through none of
        the three form hosts). Plan §4's **least-verified decision** — confirm
        against AC11 once built, don't assume it's right on the first pass.
        Raise `onLayoutChanged` whenever `Layout` changes (R32). *(The fifth
        value, `Streamed`, is Stage J's own task — it has no chrome to paint
        here at all.)*
  - Verify: `cargo test -p cobolt-forms --features render` — `Print`/`Page`
        show margins+border+shadow, `Web` shows none, `Raw` is unformatted
        (**AC3**); a parity check between the design-canvas path and the
        interactive path on the same document (**AC11**, full parity test is
        T30 — this is the first check, not the last); changing `Layout` fires
        `onLayoutChanged` exactly once per change.

- [ ] **T12 — Navigation: zoom/card slider, ViewMode, fullscreen, filmstrip,
      scrolling** (R11–R15, R33–R33.3, R32, AC4, AC5, AC19, AC31, AC32)
  - Files: `crates/cobolt-forms/src/viewer.rs`, `src/paint.rs`
  - Do: wheel-zoom about the pointer; double-click zoom to a 16× cap; Esc → 100%
        then leave fullscreen; fullscreen hides/restores the toolbar;
        `ViewMode` (**`Full`**/**`Cards`**) switched by two toolbar buttons,
        **per view** — `View1ViewMode`/`View2ViewMode`, each independent under
        `SplitMode`; `Cards` mode **replaces** that view's content with a
        reflowing grid whose row **and** column counts are a function of card
        size and **the control's own width only** — never the window or
        screen; **one slider per view**, bottom-right, directly below that
        view's own content, driving `Zoom` in `Full` mode and `CardSize` in
        `Cards` mode (R14.1 — switching a view's mode never touches the
        *other* mode's remembered slider value, R14.2). **Filmstrip**
        (R14.3/R14.4), independently per view (`View1ShowFilmstrip`/
        `View2ShowFilmstrip` — a control-wide filmstrip can't mean anything
        once the two views may hold different documents): a resizable rail of
        page thumbnails **docked to that view's content's left edge**, opened
        by its own toolbar button (`Full` mode only), closed by that same
        button again or by dragging its splitter to the view's left edge —
        both leaving `Full` mode showing just the document, never the retired
        card-mode auto-exit. **Scrolling** (R33–R33.3), reproduced from
        `doc_viewer.rs`'s own mechanics (not reusable code — private items in
        a binary crate — the algorithm only): arrow-key tap (one line =
        `FontSize × 1.6`) and hold-to-accelerate (base speed → 4× ceiling over
        2 s, after a short hold delay), Page Up/Down (viewport minus two
        lines) and Home/End, all skipped while another control holds focus;
        left-drag pans 1:1, release throws at the drag's own last ~0.12 s
        speed and decelerates under constant friction to exactly zero, never
        a fixed-duration ease; a new press mid-glide stops it at once. Raise
        `onZoomChanged`/`onCardSizeChanged`/`onScrolled` once each **settles**
        (never on an intermediate tick or glide frame), `onViewModeChanged`,
        `onFullscreenEntered`/`onFullscreenExited`, `onFilmstripToggled`
        (R32).
  - Verify: `cargo test -p cobolt-forms --features render` — zoom stops at 16×
        (**AC4**), fullscreen toggles the toolbar (**AC5**); switching a
        view's `ViewMode` to `Cards` replaces its content, row/column count
        tracks *that view's* width (resize the control, not the window/pane,
        and confirm the count changes — resizing anything else must not) and
        the per-view slider, switching back keeps each mode's own remembered
        slider value, a view's filmstrip opens on its content's left edge and
        closes by its button or by dragging to that edge without leaving
        `Full` mode (**AC19**); held-arrow-key speed reported at three points
        (tap / ~1 s / ~2 s+), Page/Home/End by the documented amounts, none of
        it while Find has focus (**AC31**); a throw's release speed comes only
        from the drag itself, decelerates under constant friction to exactly
        the limit or zero, a non-moving release throws nothing, and a press
        mid-glide stops it (**AC32**); each
        settle-event fires once per actual change, not per input tick.
  - **⏸ PART 1 OF 2 LANDED — 2026-09-19. T12 stays UNTICKED.** The task was
    split in half by an operator hold (another session was fixing a broken
    GitHub build; no further work was to be stacked on an unstable base).
    What is **done, committed and green** is T12's entire **pure model**, all
    of it in `crates/cobolt-forms/src/viewer.rs` under
    `// ── Navigation: zoom, cards, filmstrip, scrolling (T12) ──`, with 23
    new tests in `mod nav_tests` (all passing; full lib suite 852/852):
    `ViewMode`; the zoom ladder/cap/anchor (`zoom_in_step`, `zoom_out_step`,
    `zoom_by_notches`, `zoom_anchored_offset`, `clamp_zoom`); `card_grid`/
    `CardGrid`; `slider_target`/`slider_position`/`apply_slider` (R14.1/R14.2);
    `filmstrip_width_after_drag` (R14.4); `ViewRect`/`ChromeOpts`/
    `ChromeLayout`/`chrome_layout` (R15/R16, and `Streamed` honoured from the
    start so T35 is a paint change, not a second geometry model);
    `ScrollKinetics` + `KeyScrollInput` + `key_accel_factor`/`throw_speed`/
    `line_height`/`page_step` (R33–R33.2, reproducing `doc_viewer.rs`'s
    constants name-for-name); and `SettleWatch` for R32's "fires when it
    settles".
  - **PART 2 — where the next run resumes.** Nothing outside `viewer.rs` has
    been touched, so `paint.rs` and `render.rs` are exactly as T11 left them.
    Remaining: (a) refactor `paint::draw_viewer`'s parameter list into the
    `ViewerPaintState` struct plan.md §2 already names, carrying zoom, scroll
    offset, `ViewMode`, `CardSize`, filmstrip width and `Fullscreen`; (b) paint
    the toolbar band (geometry only — T13 fills it with icons), the filmstrip
    rail, the per-view slider, and the `Cards` grid, **decoding only the pages
    a strip/grid actually shows**, never the whole document; (c) drive it from
    `render.rs`'s `CT::Viewer` arm — wheel+modifier zoom, double-click, Esc,
    the key/drag/throw input into `ScrollKinetics`, with the transient
    kinematics in `ctx.memory()` and the *values* written back through
    `RenderOutput::prop_updates` (the established channel) so COBOL reads them;
    (d) fire `onZoomChanged`/`onCardSizeChanged`/`onScrolled`/
    `onViewModeChanged`/`onFullscreenEntered`/`onFullscreenExited`/
    `onFilmstripToggled` off the settle helpers.
  - **Judgment call on the R5.1 gap, recorded so it is not re-derived.** The
    background-thread wiring (`ViewerSession` → render path) is deliberately
    **deferred to T16**, not done at T12: T16 is the first task whose own file
    list already includes `viewer_session.rs` and whose AC8 decode-call
    counter forces that plumbing to be real, so doing it there is one change
    instead of two. The intended shape is a defaulted `FormState` hook
    (`fn viewer_document(&self, _base: &Control) -> Option<…> { None }`) that
    the host overrides and the designer canvas does not — which keeps AC11
    parity intact, since both surfaces still end at the same `draw_viewer`.
    Until then Part 2 must keep the interim synchronous decode **bounded by
    only ever decoding visible pages**, so a 2 GB log never costs a full index
    on the UI thread.

- [ ] **T13 — Toolbar chrome and actions: Save As, Share, Print** (R16–R20,
      R18.1, R32, AC6, AC10, AC20)
  - Files: `crates/cobolt-forms/src/paint.rs` (toolbar), `src/viewer.rs`,
        `crates/cobolt-runtime/src/interpreter.rs` (`"SAVEAS"`, `"PRINT"`,
        `"SHARE"` in the method table, the `"SHOW"`/`"DISMISSALL"` precedent)
  - Do: hand-drawn painter icons with tooltips for every R16 item (most already
        exist in the catalogue per plan §2 — `chevron-*`, `highlighter`,
        `fullscreen`, `split-view`, `share`, `printer`, `doc-save-as`; verify
        layout-switcher/font-size/filmstrip icons exist or author them; the
        two `ViewMode` buttons — Full/Cards — need their own icons, and
        `zoom-in`/`zoom-out` move from a toolbar group onto the small ends of
        T12's per-view slider instead, not a separate toolbar control); Save
        As writes original bytes unmodified (R18); the
        `LoadBytes`-with-no-source-path default filename — first
        three words of extracted text + matching extension, generic fallback
        for textless documents, user-editable, extension always restored
        (R18.1); Share/Print hand off to the OS (`NSSharingService` / Windows
        share contract / `xdg-open` — named directly in R19/R20). Raise
        `onSaveComplete`/`onSaveCancelled`, `onShareComplete`/
        `onShareCancelled`, `onPrintComplete`/`onPrintCancelled` from whatever
        the OS handoff itself reports back (R32) — these are the three
        events this control cannot fake with a local flag, since only the OS
        dialog knows whether the user actually went through with it.
  - Verify: `cargo test -p cobolt-runtime` — Save As produces a byte-identical
        file, asserted by comparing bytes (**AC6**); `SaveAs(path)` from COBOL
        always uses the given path, no defaulting (plan §3's scope note); each
        of the three Complete/Cancelled pairs fires from a simulated OS
        outcome, one test per pair rather than assuming symmetry.
        `cargo test -p cobolt-forms` — the default-filename logic across a
        `LoadBytes` text doc and an image (fallback case), reporting the
        proposed name each time (**AC20**). Every toolbar icon has a tooltip
        (**AC10**).

## Stage D — Search (built against Wave 1's extracted text)

- [ ] **T14 — Find match-computation** (R26.1, R27)
  - Files: `crates/cobolt-forms/src/viewer.rs`
  - Do: a pure function — case-sensitive/insensitive substring matching over a
        format's extracted text, returning spans — modeled on
        `code_search.rs`'s `find_matches()`, reimplemented here since that
        file is `cobolt-ide`-only. A format with no extractable text (an
        image) reports zero matches, never an error (R26.1).
  - Verify: `cargo test -p cobolt-forms` — case on/off reports different match
        counts for the same query; a textless format reports zero cleanly.

- [ ] **T15 — Find bar: navigation, highlighting, COBOL surface** (R26, R28–R31,
      R32, AC22, AC23, AC24)
  - Files: `crates/cobolt-forms/src/paint.rs` (the bar), `src/viewer.rs` (state:
        `find_idx`/`find_total`/active-match handoff, modeled on
        `doc_viewer.rs`'s shape), `crates/cobolt-runtime/src/interpreter.rs`
        (`"FINDNEXT"`, `"FINDPREVIOUS"`, plus the Search property group)
  - Do: `Ctrl+F`/`Cmd+F` opens the bar, `Esc` closes it and takes priority over
        R13's existing Esc behaviour while open; two new icons (Find/
        magnifying-glass, case-sensitivity "Aa" — confirmed absent from the
        catalogue, author both); Next/Previous with wraparound
        (`F3`/`Shift+F3`, `Enter`/`Shift+Enter`); highlight-all with the
        current match distinguished, toggleable without breaking navigation;
        a live "current of total" counter; every property/action COBOL r/w
        (R31). Raise `onFindOpened`/`onFindClosed` (R32).
  - Verify: `cargo test -p cobolt-forms --features render` — typing highlights
        matches and updates the counter live, Next/Previous wrap at both ends
        (**AC22**); toggling case sensitivity changes which matches are found
        without retyping, toggling highlight off doesn't break navigation or
        the count (**AC23**); opening/closing the bar fires the matching event
        exactly once each. `cargo test -p cobolt-runtime` — every Search
        property/method round-trips from COBOL (**AC24**).

## Stage E — Split view (depends on Search)

- [ ] **T16 — Two independent viewports** (R21, R21.1, R32, AC8)
  - Files: `crates/cobolt-forms/src/viewer.rs` (the `View1`/`View2`-prefixed
        properties, aliased from the plain names when `SplitMode = None` — plan
        §4's naming decision), `src/paint.rs` (divider, reusing
        `splitter::geometry()`'s percent-split math only — not its child-Panel
        ownership), `crates/cobolt-form-host/src/viewer_session.rs` (the
        `Arc`-shared decoded-document handle for the same-document case)
  - Do: `LeftRight`/`TopBottom` show two views, each with independent
        source/page/zoom/scroll; pointing both at the same document attaches
        to the existing decode rather than reloading (R21.1) — enforced by the
        shared handle, not a runtime check. Raise `onSplitModeChanged` when
        `SplitMode` changes (R32).
  - Verify: `cargo test -p cobolt-form-host` — a decode-call counter across two
        views of one path, asserted `== 1` (**AC8**'s same-document clause);
        `cargo test -p cobolt-forms --features render` — moving one view's
        page/zoom/scroll leaves the other's untouched (**AC8**'s independence
        clause); switching `SplitMode` fires `onSplitModeChanged` exactly once.

- [ ] **T17 — Per-view independent search** (R21.2, AC8)
  - Files: `crates/cobolt-forms/src/viewer.rs`
  - Do: each `ViewState` carries its own `SearchState` (T14/T15's engine,
        instantiated per view) — falls out naturally from where the state
        lives, not a special case.
  - Verify: `cargo test -p cobolt-forms` — searching one view leaves the
        other's query/matches/current-index untouched, including when both
        views hold the same document (**AC8**'s search clause, **R21.2**).

## Stage F — PDF (fidelity wave 2)

- [ ] **T18 — PDF: text, basic vector, page geometry** (R7, R9, AC2)
  - Files: `crates/cobolt-forms/src/viewer.rs`, `Cargo.toml` (`lopdf ~0.26`,
        promoted from transitive-only to a direct, read-capable dependency)
  - Do: page/text/basic-vector extraction into the same layout model Wave 1
        established — **the PDF's own bytes stay the canonical stored form
        throughout** (the user's own example of the corrected storage
        principle above: PDF is binary, not editable, and lives in that format
        in memory, on disk and on screen alike); this extraction is a derived
        read for painting, never a conversion. Page breaks from PDF page
        boundaries; feed the extracted text into T14's Find engine (Search
        "just works" on PDF once this lands — no changes to T14/T15 expected). **Treat the first attempt as
        a go/no-go spike** (plan §5's largest technical unknown) — if `lopdf`'s
        read API can't deliver R-level fidelity, that's a finding to report,
        not a reason to reach for a C dependency.
  - Verify: `cargo test -p cobolt-forms` — a real PDF opens, page count and
        page breaks match the source, extracted text is searchable via T14/T15
        unchanged (**AC2**); complex-page/embedded-font/form/annotation/
        scanned-image cases are confirmed **not** delivered (§3's "Not
        delivered" column), not silently attempted.

- [ ] **T19 — PDF Save As is where byte-fidelity actually gets exercised**
      (R18, AC6)
  - Files: `crates/cobolt-forms/src/viewer.rs`
  - Do: confirm T13's byte-identical Save As holds for a PDF specifically —
        the format most likely to tempt a "helpful" re-encode.
  - Verify: `cargo test -p cobolt-runtime` — a PDF Save As compared byte-for-
        byte against the source (**AC6**, PDF case).

## Stage G — Mermaid subset (fidelity wave 3 — after PDF, per spec.md's order)

- [ ] **T20 — Mermaid flowchart + sequence, inside fenced Markdown blocks**
      (R7, AC2)
  - Files: `crates/cobolt-forms/src/viewer.rs`, `Cargo.toml`
        (`mermaid-rs-renderer`, matching `cobolt-ide`'s pin)
  - Do: detect ` ```mermaid ` blocks in T9's Markdown walker; render via
        `mermaid-rs-renderer` → SVG → `resvg` (already an optional dependency
        of `cobolt-forms` via the SVG image path — no new raster dependency).
        Flowchart and sequence only; other diagram types are out of scope
        (§3).
  - Verify: `cargo test -p cobolt-forms` — a flowchart and a sequence diagram
        both render, reporting produced pixel dimensions; a `class`/`state`/
        `gantt` block is confirmed **not** attempted, not silently ignored.

## Stage H — HTML subset (fidelity wave 4)

- [ ] **T21 — HTML parse → the shared layout model** (R7, AC2)
  - Files: `crates/cobolt-forms/src/viewer.rs`, `Cargo.toml` (`html5ever`)
  - Do: parse into a DOM-ish tree, map block/inline layout, typography,
        colours, borders, tables and images onto the **same** layout
        primitives T9's Markdown walker produces (plan §4's explicit decision
        — this is what keeps this task's cost to "parse + map," not a second
        engine). CSS3 grid/flex/animation, JavaScript and floats beyond the
        simple case are confirmed out of scope, not half-attempted.
  - Verify: `cargo test -p cobolt-forms` — a representative HTML subset
        document (tables, images, nested inline formatting) renders onto the
        shared primitives; a JS-bearing or grid-laid-out fixture is confirmed
        to degrade to the supported subset rather than silently break.

- [ ] **T22 — Confirm Find/Save-As/Print/Share need nothing HTML-specific**
      (R18–R20, R26–R31)
  - Files: none expected — this is a verification task
  - Do: run Stage D/Stage C's tests against an HTML-subset document.
  - Verify: `cargo test -p cobolt-forms -p cobolt-runtime` — if any of these
        needs an HTML-specific branch, that's new scope to fold back in here,
        not silently patched elsewhere later.

## Stage I — Conversation mode (§8, sequenced last — reuses Stage H's HTML layer)

- [ ] **T23 — `append_html`/`append_markdown`/`append_raw`/`append_to_message`**
      (§8.1, §8.2, R32, AC12, AC13, AC16, AC17)
  - Files: `crates/cobolt-forms/src/viewer.rs` (append-only incremental layout-
        model update — **not** a full rebuild each call, the actual point of
        this stage), `crates/cobolt-runtime/src/interpreter.rs` (the four
        `"APPEND*"` methods)
  - Do: HTML appends as-is; Markdown converts to HTML-shaped layout nodes
        first (T9's walker, reused); Raw escapes `&`/`<`/`>`, preserves
        whitespace, never interpreted as markup even if it contains valid
        markup; `append_to_message(id, …)` extends an existing message by
        stable id; strict arrival order; selection/keyboard focus preserved
        across an append. Raise `onContentRendered` once the appended chunk's
        layout pass is complete — **after** layout, not after the data is
        merely accepted (§8.2 item 6, R32).
  - Verify: `cargo test -p cobolt-runtime` — HTML/Markdown/Raw each land
        correctly (**AC12, AC13**); a raw chunk containing `<script>` displays
        the literal text (**AC13**); `append_to_message` extends the right
        message and arrival order holds under interleaved calls (**AC16**);
        a running text selection survives an unrelated append (**AC17**);
        `onContentRendered` fires strictly after the layout pass, verified by
        ordering it against a layout-completion marker, not a timer.

- [ ] **T24 — Auto-follow scrolling** (§8.3, AC15, AC18)
  - Files: `crates/cobolt-forms/src/viewer.rs`
  - Do: capture the **pre-append** scroll-at-end state with a 24–32px
        threshold; pin to the new end only if it was already there; never move
        the viewport for a user reading older content; auto-follow resumes the
        moment the user scrolls back to the end; late layout changes (an image
        finishing decode) keep the pin only while auto-follow is active.
  - Verify: `cargo test -p cobolt-forms` — at-end vs. scrolled-up produce
        different post-append viewport positions from the same append
        (**AC15**); a simulated late image-decode event after the user has
        scrolled up leaves their position untouched, but pins to the end when
        they were following (**AC18**).

- [ ] **T25 — New-content indicator, and the `RenderAsHtml` override** (§8.1,
      §8.4, AC14)
  - Files: `crates/cobolt-forms/src/viewer.rs`
  - Do: a non-intrusive indicator when content arrives off-screen; activating
        it scrolls to the end, clears itself, and re-enables auto-follow.
        `RenderAsHtml` (default true) — **plan §7's flagged reading**: when
        explicitly set false, every append behaves as Raw regardless of which
        mode was actually called, as a blanket safety override. Confirm this
        reading before or during this task, not after — it's the cheapest
        point to correct it.
  - Verify: `cargo test -p cobolt-runtime` — activating the indicator does all
        three things in order; `RenderAsHtml = false` forces Raw behaviour even
        for an explicit `append_html` call (**AC14**).

- [ ] **T26 — Sanitisation** (§8.5)
  - Files: `crates/cobolt-forms/src/viewer.rs`
  - Do: strip scripts, inline event handlers and unsafe URL schemes from HTML/
        Markdown-derived content before it's appended; Raw's escaping (T23)
        already covers its own case.
  - Verify: `cargo test -p cobolt-forms` — a battery of unsafe HTML fragments
        (script tags, `onclick=`, `javascript:` URLs) each confirmed stripped
        or neutralised, reporting which rule caught each case.

- [ ] **T27 — Performance: batching, incremental layout, stable ids** (§8.6)
  - Files: `crates/cobolt-forms/src/viewer.rs`
  - Do: coalesce rapid successive appends within a frame; the layout model
        only re-lays-out the newly appended tail, not the whole conversation
        (this is what makes "avoid a full rebuild" real, not just a claim);
        message ids stay stable across appends; a pruning/virtualisation
        strategy once conversation size crosses a configurable ceiling.
  - Verify: `cargo test -p cobolt-forms` — appending to a long, pre-built
        conversation **reports the time spent laying out old vs. new content**
        — old content's layout-recompute cost should be ~zero, not
        proportional to conversation length (this project's quantify-
        performance rule, applied to §8.6's own requirement).

## Stage J — Streamed layout and conversation management (§8.8)

*(Numbered T35–T37, past Stage K's T28–T34, to avoid renumbering the wrap-up
stage — the same out-of-sequence pattern this project's specs already use
(spec.md's R26–R31 and AC19). Placed here, physically, because it depends on
Stage I's append machinery existing and belongs right after it in reading
order.)*

- [ ] **T35 — `Layout = Streamed`: one pane, no chrome** (R7, §8.8, AC25)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: the fifth `Layout` value paints only the conversation content — no
        toolbar, Find bar, thumbnail or filmstrip chrome, regardless of what
        was showing under the previous `Layout`. Nothing else changes: the
        append/auto-follow machinery from Stage I is reused as-is.
  - Verify: `cargo test -p cobolt-forms --features render` —
        `streamed_layout_draws_a_single_pane_and_nothing_else` (plan §6):
        shape count under `Streamed` compared against every other `Layout`
        value, asserting zero chrome shapes (**AC25**).

- [ ] **T36 — Conversation management: `NewConversation`, `SelectConversation`,
      `RegisterConversation`, `HistoryList`** (§8.8, AC26, AC27, AC28, AC29)
  - Files: `crates/cobolt-form-host/src/viewer_session.rs` (`history:
        VecDeque<HistoryEntry>`, capped at 10, id+title only — **never**
        content, plan §3/§4), `crates/cobolt-runtime/src/interpreter.rs` (the
        three new methods)
  - Do: `NewConversation()` archives a non-empty pane into history, clears it;
        a no-op on an already-empty pane (no spurious history entry).
        `SelectConversation(id)` archives the current conversation the same
        way, removes `id` from history, clears the pane — content is **never**
        restored from a cache, only ever re-requested from the host.
        `RegisterConversation(id, title)` seeds a shell entry with no content,
        for history left over from an earlier run. Past 10 entries, the
        oldest is evicted on either archive or register. `HistoryList` reads
        back every current entry as `id|title`, one per line.
  - Verify: `cargo test -p cobolt-form-host` —
        `history_never_exceeds_ten_and_evicts_the_oldest` (**AC28**, plan §6);
        `select_conversation_swaps_current_and_history_without_touching_content`
        (**AC27**). `cargo test -p cobolt-runtime` —
        `new_conversation_and_select_conversation_dispatch` — the empty vs.
        non-empty branches of `NewConversation()` reported separately
        (**AC26**); `HistoryList` matches the live entry set after every
        archive/select/evict (**AC29**).

- [ ] **T37 — The three conversation events** (§8.8, R32)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`,
        `crates/cobolt-forms/src/model.rs` (`SUPPORTED_EVENTS`)
  - Do: `onConversationCreated` on `NewConversation()`;
        `onConversationSelected` (carrying `id`) on `SelectConversation(id)` —
        this is the signal that tells the host to start calling §8.2's append
        methods; both feed R32's table alongside `onContentRendered` (already
        wired in T23).
  - Verify: `cargo test -p cobolt-runtime` — both events fire with the right
        payload and in the right order relative to the pane-clear each method
        performs (clear happens, *then* the event — a handler bound to either
        event sees an empty pane, never stale content).

## Stage K — surfaces, parity, docs, KB, finalize

- [ ] **T28 — IDE property editors** (R22)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`
  - Do: grouped editors — `Layout`/`Format`/`SplitMode` dropdowns; the
        `View1`/`View2` groups appear only when `SplitMode != None`.
  - Verify: `cargo test -p cobolt-ide --bins`; **manual:** every property in
        plan §3 is editable from the Properties panel.

- [ ] **T29 — Codegen** (R22)
  - Files: `crates/cobolt-codegen/src/lib.rs`
  - Do: declaration + handler stubs for whichever events the developer has
        bound in the Designer, the standard banner intact — no per-event
        special-casing needed; the mechanism is already generic, it just now
        has ~16 events to be generic over instead of 3.
  - Verify: `cargo test -p cobolt-codegen`; generated `.cbl` parses under
        `cobolt-parser` and checks clean under `cobolt-semantic` for a form
        binding at least one of the new events (not only the original three).

- [ ] **T30 — Engine parity: designer canvas vs. running form** (R25, AC11)
  - Files: `crates/cobolt-forms/tests/`
  - Do: a parity test in the shape of
        `engine_reference_form_parity_static_vs_faces`, covering every format
        wave landed so far.
  - Verify: `cargo test -p cobolt-forms --features render` — same shapes/fills
        on both paths for every format (**AC11**, closing the loop T11 opened).

- [ ] **T31 — Rounded-corner measurement** (steering: spec 057)
  - Files: `crates/cobolt-forms/src/render.rs` (`self_clipping_type`, if the
        measurement says Viewer needs the exclusion)
  - Do: nothing assumed — run the harness, read its verdict.
  - Verify: `cargo test -p cobolt-forms
        a_child_at_a_rounded_corner_stays_inside_the_arc` green; if it reports
        Viewer painting past the arc, add the exclusion and re-run, don't
        pre-guess it in an earlier task.

- [ ] **T32 — System KB** (steering: hard constraint)
  - Files: `crates/cobolt-compiler/src/lib.rs` (all four doc tables),
        `assets/knowledge/chunked.data`
  - Do: property/method/event entries for Viewer — **all** of it: the static-
        document surface, the conversation-mode append methods, and §8.8's
        `NewConversation`/`SelectConversation`/`RegisterConversation`/
        `HistoryList`, plus every event in R32's table — then `cargo run -p
        cobolt-ide --example build_chunked_kb`.
  - Verify: `chunked.data` **actually changes** (unchanged = wrong file edited);
        **new** `spec_058_viewer_is_fully_published_in_the_system_kb`
        (`spec_039`'s template — `## Control: Viewer` present, representative
        property/event/method needles present in both published docs,
        including at least one R32 event and one §8.8 method so the expanded
        surface isn't silently under-published); `cargo test -p cobolt-ide
        --bins prebuilt_chunked_kb` green.

- [ ] **T33 — Docs & i18n**
  - Files: `docs/developers-guide-en.md` (a `### Viewer` subsection in §8 "The
        control catalogue," alongside `### Snackbar (transient notifications)`
        at line 3518 — COBOL examples only, no Rust, Notes + ⚠️ Caveats for the
        §3 fidelity boundaries **and** for Streamed layout's "you build the
        chrome" contract), `crates/cobolt-ide/src/i18n.rs`
  - Do: every new IDE string (layout names, toolbar tooltips, Find-bar labels,
        error strings) a `Tr` field in **all six** languages. A worked example
        of a minimal Streamed-layout chatbot form — a plausible sidebar of the
        developer's own, wired to `NewConversation`/`SelectConversation` — since
        this is the one part of the guide's new material a PowerCOBOL/isCOBOL
        reader has no prior instinct for at all. Per Golden Rule #8: this
        change **deletes** `docs/developers-guide-{es,pt,fr,jp,cn}.md` — not
        patched, not left stale; the two `docs_embed.rs` language-coverage
        guards going red afterward is the **expected** signal, not a bug to
        chase.
  - Verify: `cargo test -p cobolt-ide i18n`; the guide passes
        `iconv -f UTF-8 -t UTF-8` with zero double-encoded bytes.

- [ ] **T34 — Finalize**
  - Do: bump `z` in `version.rs` **once** for the whole feature + one
        `CHANGELOG.md` entry.
  - Verify: `cargo test -p cobolt-forms --features render --no-fail-fast`,
        `-p cobolt-form-host`, `-p cobolt-runtime`, `-p cobolt-ide --bins`,
        `-p cobolt-codegen`, `-p cobolt-compiler` — **read every
        `test result:` line**, never grep for failures. Specifically confirm
        `every_event_in_r32s_table_fires_at_its_documented_moment` (T-numbered
        in `cobolt-runtime`, plan §5/§6) is green — this is the one test whose
        job is exactly AC30, and a full-suite run that skipped it would report
        green without ever having checked AC30 at all. NIST is **not**
        required: every Viewer method is ordinary method dispatch, nothing
        here touches the interpreter's language grammar (the Snackbar T19
        precedent — re-open this if a later stage ends up touching parsing).
        **Manual** (operator, not agent — never driven from here): open one
        document per format, confirm each matches §3 exactly; split view with
        different documents and with the same document twice; Find's case-
        sensitivity/highlight-toggle/wraparound; a live chatbot-style append
        sequence with auto-follow; Streamed layout with a developer-built
        sidebar driving `NewConversation`/`SelectConversation`, confirming the
        events actually reach a bound COBOL handler; design-canvas vs.
        running-form parity; corner rendering on every surface.

## Done criteria

All 32 acceptance criteria in `spec.md` (AC1–AC11, AC19–AC24, AC30–AC32 from
§6; AC12–AC18, AC25–AC29 from §8.7) are checked, every suite green, docs and
KB updated, and the change sits in feature commit(s) on `features` (do **not**
commit or push unless the operator asks).

**Coverage map** — AC1 T7/T34 · AC2 T9/T10/T18/T20/T21 · AC3 T11 · AC4 T12 ·
AC5 T12 · AC6 T13/T19 · AC7 T15/T16/T28 · AC8 T16/T17 · AC9 T6 · AC10 T3/T13 ·
AC11 T11/T30 · AC12 T23 · AC13 T23 · AC14 T25 · AC15 T24 · AC16 T23 ·
AC17 T23 · AC18 T24 · AC19 T12 · AC20 T13 · AC21 T6 · AC22 T15 · AC23 T15 ·
AC24 T15 · AC25 T35 · AC26 T36 · AC27 T36 · AC28 T36 · AC29 T36 · AC30 T34
(exercising T11/T12/T13/T15/T16/T23/T37's events) · AC31 T12 · AC32 T12.
