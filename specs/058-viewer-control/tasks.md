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

- [x] **T12 — Navigation: zoom/card slider, ViewMode, fullscreen, filmstrip,
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
  - **DONE — 2026-09-19, in two commits** (an operator hold split the task
    mid-flight; the halves are 1.70.85 and 1.70.86). The pure model lives in
    `crates/cobolt-forms/src/viewer.rs` under `// ── Navigation: zoom, cards,
    filmstrip, scrolling (T12) ──`: `ViewMode`; the zoom ladder/cap/anchor;
    `card_grid`; `slider_target`/`slider_position`/`apply_slider`;
    `filmstrip_width_after_drag`; `ViewRect`/`ChromeOpts`/`ChromeLayout`/
    `chrome_layout`; `ScrollKinetics` + `KeyScrollInput` + `key_accel_factor`/
    `throw_speed`; and `SettleWatch`. The wiring is
    `paint::ViewerPaintState`/`draw_viewer` (toolbar band, filmstrip rail,
    card grid, per-view slider) and `render::viewer_interactive`.
  - **Three decisions taken here, recorded so they are not re-derived:**
    1. **`Streamed` is honoured by `chrome_layout` from the start**, so T35 is
       a paint change rather than a second geometry model that could disagree
       with this one.
    2. **`SharedValue` reconciles the engine against COBOL.** A property write
       that differs from last frame's wins; otherwise the engine's own live
       value stands. Without it a gesture is undone one frame later by any
       host that has not echoed `prop_updates` back, and the value flip-flops
       — which made `onZoomChanged` fire twice for one double-click until
       `one_zoom_gesture_raises_exactly_one_onzoomchanged` caught it.
    3. **T31's corner measurement was answered here, not deferred.** T12's
       chrome is what made Viewer paint past the arc (113 px), so
       `a_child_at_a_rounded_corner_stays_inside_the_arc` went red the moment
       the band and the rail landed. `render::self_clipping_type` now excludes
       `Viewer`, with the measurement and its date in the comment. T31 is
       therefore a re-run and a read, not new work.
  - **The R5.1 gap stays open, deliberately, until T16.** The background
    thread (`ViewerSession` → render path) is **not** wired: both surfaces
    still use `paint`'s synchronous decode. T16 is the first task whose own
    file list already includes `viewer_session.rs` and whose AC8 decode-call
    counter forces that plumbing to be real, so doing it there is one change
    instead of two. The intended shape is a defaulted `FormState` hook
    (`fn viewer_document(&self, _base: &Control) -> Option<…> { None }`) the
    host overrides and the designer canvas does not — which keeps AC11 parity,
    since both surfaces still end at the same `draw_viewer`. **What T12 did do
    is make the interim path bounded:** `paint::viewer_index` memoizes only
    page *offsets*, and `viewer_page_preview` decodes exactly the pages a card
    grid or a filmstrip actually shows, so neither costs the document.
  - **Not yet painted, by design:** the toolbar band is reserved and drawn but
    **empty** — its icons, tooltips and actions are T13's whole subject.

- [x] **T13 — Toolbar chrome and actions: Save As, Share, Print** (R16–R20,
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

  - **DONE — 2026-09-19 (1.70.87).** Twelve toolbar buttons
    (`viewer::TOOLBAR_ITEMS`), each with a catalogue icon and a tooltip,
    painted by `paint::draw_viewer_toolbar_band` and sensed in
    `render::viewer_interactive`. R18's Save As, R19/R20's Print and Share
    handoffs and R18.1's proposed filename all landed; the interpreter gained
    `LoadBytes`, `SaveAs`, `Print` and `Share`.
  - **Icon finding — plan.md §2 was wrong, and the catalogue is the authority.**
    §2 expected to author a Find/magnifying-glass glyph and to "verify
    layout-switcher/font-size/filmstrip icons exist or author them". Measured:
    `magnifier`, `layout-dashboard`, `thumbnails`, `grid-view` and `doc-text`
    all already exist, so **only two new icons were drawn** — `font-smaller`
    and `font-larger` (a letterform plus a sign; `zoom-in`/`zoom-out` already
    mean *Zoom* on this control, and R10 keeps FontSize and Zoom independent,
    so the two must not look alike). Authoring near-duplicates of five
    existing icons would have grown a 600-icon set for nothing.
  - **Tooltips: English in the engine, with a seam.** spec §7 asks for a `Tr`
    field per tooltip, but `cobolt-forms` has no i18n table and cannot reach
    `cobolt-ide`'s (a binary crate) — the DataGrid's own hardcoded
    `"Export CSV"` is the standing precedent. So the engine ships English and
    `viewer::set_toolbar_tooltips` installs a translated table per thread
    (the shape `theme::set_active()` already uses). **T33 supplies the six
    languages and calls it.**
  - **Two buttons landed ahead of their own tasks, deliberately:** `Split`
    toggles `SplitMode` and raises `onSplitModeChanged`, `Find` toggles
    `FindOpen` and raises `onFindOpened`/`onFindClosed`. Both properties and
    both events are R16/R32's, not T15's or T16's; wiring them here means the
    toolbar never shows a button that does nothing. T15 adds the Find *bar*
    and T16 the second *viewport*.
  - **The deferred R4/R6 COBOL dispatch was folded in here**, as the brief
    directed. Writing a Viewer's `Source` now opens the document on the
    **interpreter's** thread (not the UI thread — `form_runtime.rs`, `rcrun
    run-form` and the compiled binary all run it separately), reports
    `onLoadProgress` every tenth percent, then `onLoaded` with `Format` and
    `Progress` set. A failure sets `LastError`, raises `onError` and **puts
    `Source` back to the last document that loaded**, which is how R4's
    "leave any previously loaded document displayed" becomes true rather than
    merely intended.
  - **Print/Share/SaveAs events come from the OS, never from a flag.**
    `Interpreter::report_viewer_os_outcome(ctrl, ViewerOsAction, completed)`
    is the one door those six events come through, because only the OS dialog
    knows whether the user went through with it. Three tests, one per pair.
  - **Finding: `arg(0)` trims, and for `LoadBytes` that is wrong.** The shared
    method-argument helper trims — right for a padded `PIC X(80)` path, fatal
    for a byte payload, where it silently ate a document's final newline.
    `LoadBytes` reads the argument untrimmed. A COBOL item is still
    fixed-length, so `PIC X(100)` holding 42 characters delivers 100 padded;
    that is the developer's to size and is documented at the call site.

## Stage D — Search (built against Wave 1's extracted text)

- [x] **T14 — Find match-computation** (R26.1, R27)
  - Files: `crates/cobolt-forms/src/viewer.rs`
  - Do: a pure function — case-sensitive/insensitive substring matching over a
        format's extracted text, returning spans — modeled on
        `code_search.rs`'s `find_matches()`, reimplemented here since that
        file is `cobolt-ide`-only. A format with no extractable text (an
        image) reports zero matches, never an error (R26.1).
  - Verify: `cargo test -p cobolt-forms` — case on/off reports different match
        counts for the same query; a textless format reports zero cleanly.

- [x] **T15 — Find bar: navigation, highlighting, COBOL surface** (R26, R28–R31,
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

  - **DONE — 2026-09-19 (1.70.88).** `viewer::find_matches` is the whole of
    Find's searching: one pure function no format knows about, which is why
    T18 (PDF) and T21 (HTML) should need no change in it. The bar itself is
    `paint::draw_viewer_find_bar` + the Find block in
    `render::viewer_interactive`; the COBOL surface is `Find`, `FindNext`,
    `FindPrevious`, `FindClose` and the whole `Search*` property group.
  - **One new icon:** `case-sensitive` (a capital A beside a lowercase a —
    the two letterforms *are* the distinction). plan.md §2 also expected to
    author a Find glyph; `magnifier` already existed, so it was reused.
  - **Case-insensitive search never lowercases the haystack.** Lowercasing
    changes byte lengths (`İ` is two bytes and lowercases to three), which
    would put every span after it one byte off — highlighting `STANBUL `
    instead of `ISTANBUL`. The haystack is walked as a stream of lowercase
    characters, each remembering its **source** char's byte range. The test
    asserts exact offsets, because "two matches" would pass with both of
    them misplaced.
  - **⚠️ Known gap — highlights are painted on the single-galley path only.**
    A plain-`Text` document in any layout, and any document under `Raw`, gets
    R29's coloured overlay. A **formatted Markdown** document is many
    galleys, and mapping a global span into the right one needs a running
    text offset threaded through `paint_block`; the **count and
    Next/Previous work correctly there** (both come from
    `SearchableText`, the prose a reader actually sees), only the overlay is
    missing. Recorded rather than half-done. **T22 is the natural place to
    close it** — it already re-runs Stage D's tests against an HTML-subset
    document, which is exactly when the blocks path matters.
  - **Three engine bugs the tests caught, all worth remembering:**
    1. **Change-detection must compare against LAST FRAME's value, not the
       property.** `diverged()` stays true forever against a host that has
       not echoed `prop_updates` back, so one Ctrl+F raised **four**
       `onFindOpened` events.
    2. **The match total is the engine's own measurement**, not a property
       round trip. Routed through `SearchMatchCount` alone, Next/Previous did
       nothing until the host echoed the count back.
    3. **Shift comes from the key EVENT, never `InputState::modifiers`** —
       that field is the platform's last reported state, not the one that
       accompanied this press, so Shift+F3 stepped *forward*. The same
       distinction that bit `consume_key` before.
  - **Two runtime findings:** `canonical_prop_value` normalises a boolean to
    `"true"`/`"false"`, so comparing `FindOpen` against `"1"` matched nothing
    and raised `onFindOpened` on every call; and `seed_objects` writes
    designed properties straight into the registry, bypassing the alias
    mirror, so a read must prefer `View1*` and fall back to the short name
    (`Interpreter::viewer_prop`).
  - **The test harness now applies `prop_updates`**, the way a real host
    does. Without that it modelled a host that ignores them — which no host
    does — and every engine test read a control frozen at its designed state.

## Stage E — Split view (depends on Search)

- [x] **T16 — Two independent viewports** (R21, R21.1, R32, AC8)
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

- [x] **T17 — Per-view independent search** (R21.2, AC8)
  - Files: `crates/cobolt-forms/src/viewer.rs`
  - Do: each `ViewState` carries its own `SearchState` (T14/T15's engine,
        instantiated per view) — falls out naturally from where the state
        lives, not a special case.
  - Verify: `cargo test -p cobolt-forms` — searching one view leaves the
        other's query/matches/current-index untouched, including when both
        views hold the same document (**AC8**'s search clause, **R21.2**).

  - **DONE — 2026-09-19 (1.70.89).** `viewer::split_geometry` reimplements
    `splitter::geometry()`'s percent-split arithmetic (the math only — that
    control owns two developer-droppable child Panels, the wrong shape
    entirely for one control with two viewports of its own state).
  - **The refactor was smaller than expected, and that is the design.**
    `draw_viewer` never knew how big the control was, only which rect it was
    given — so splitting is handing it half a rect twice, not teaching it
    about split mode. The same held in `render.rs`: the interaction body
    became `viewer_view_interactive(…, view_index, …)`, called once or twice.
  - **Independence is structural, not remembered.** Each view has its own
    live state slot, its own egui widget ids (`vid = ctrl_id.with(("viewer-
    view", i))` — two views of one control must never share an interaction
    id) and its own `View{n}*` properties, written through one `View#`
    marker in the push helper. **The unprefixed aliases are view 1's alone**,
    so view 2 can never quietly overwrite `Zoom` or `SearchText`. R21.2 then
    needed no work at all: per-view search falls out of where the state
    lives, which is exactly what T17 predicted.
  - **One deliberate compromise, recorded:** control-wide state (`Layout`,
    `FontSize`, `Fullscreen`, `SplitMode`) is *written* by whichever view's
    toolbar was clicked, but its change **event** is raised by view 1's pass
    — so a click in view 2 reports one frame later. The alternative was two
    views both reporting the same change.
  - **AC8's decode-once clause is measured, not asserted.**
    `viewer_session::DocumentRegistry` holds every open document behind an
    `Arc` keyed by path: a view asking for a path someone already holds gets
    a clone, and only an unheld path is decoded. The test's `decode`
    closure for the second view **panics if it is ever called**, and the
    registry's own decode counter is asserted `== 1`. `release_unused` keeps
    "attach, don't reload" from becoming "attach, and never let go".

## Stage F — PDF (fidelity wave 2)

- [x] **T18 — PDF: text, basic vector, page geometry** (R7, R9, AC2)
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

- [x] **T19 — PDF Save As is where byte-fidelity actually gets exercised**
      (R18, AC6)
  - Files: `crates/cobolt-forms/src/viewer.rs`
  - Do: confirm T13's byte-identical Save As holds for a PDF specifically —
        the format most likely to tempt a "helpful" re-encode.
  - Verify: `cargo test -p cobolt-runtime` — a PDF Save As compared byte-for-
        byte against the source (**AC6**, PDF case).

  - **GO. The spike's verdict, in numbers (2026-09-19, 1.70.90).** plan.md §5
    called PDF "the largest technical unknown" and asked for the first
    attempt to be treated as a go/no-go. `lopdf 0.26`'s read API delivers
    every item in §3's **delivered** column: page count from the document's
    own tree, per-page text via `extract_text`, page geometry from the
    MediaBox (inherited up the page tree when a page declares none), and
    "basic vector" as `re` rectangles and `m`/`l` segments decoded from the
    content stream. A 7-page fixture reads 7 pages; a 2-page one with a rect
    and a line each reads 4 vectors and, without them, 0. **No C dependency
    was needed and none was reached for** (R25).
  - **§3's not-delivered column is refused, not half-attempted.** Curves
    (`c`/`v`/`y`), shading, patterns and clipping are not decoded — a
    half-drawn Bézier is worse than an honestly absent one. A page with no
    text layer answers `None`, never an invented string, so R26.1's "zero
    matches, cleanly" is reached with no special case.
  - **Find needed no change at all**, which is what T14's design predicted:
    `PdfDocument` implements `SearchableText` and T14's engine searches it,
    case toggle included. A PDF's search is 3 hits for "balance" across 3
    pages, 0 with case sensitivity on — the same engine, the same numbers.
  - **`PageAddressing` is new, and load-bearing.** Plain text's pages ARE
    byte ranges, which is what lets `decode_text_page` jump to the last page
    of a 2 GB log for one page's I/O. A PDF's are not — they live inside
    compressed object streams — so a `DocumentIndex` now says which kind it
    holds rather than every caller inferring it from `format` and handing a
    byte reader the middle of a Flate stream.
  - **T19's byte-fidelity is asserted on BYTES.** The fixture carries a
    binary comment line and a stream holding every one of the 256 byte
    values, so an encoding-aware copy would show up as a difference rather
    than as a plausible-looking file: 518 bytes in, 518 out, first differing
    byte `None`. A second test confirms R24 — the source's contents *and its
    modification time* are untouched.
  - **Fixtures are written by `lopdf`'s own writer**, not by hand: a PDF's
    cross-reference table is a list of byte offsets, and a hand-written
    fixture tests the arithmetic in the test far more than it tests the
    reader.

## Stage G — Mermaid subset (fidelity wave 3 — after PDF, per spec.md's order)

- [x] **T20 — Mermaid flowchart + sequence, inside fenced Markdown blocks**
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

  - **DONE — 2026-09-19 (1.70.91).** A ```` ```mermaid ```` fence is now its
    own `Block::Mermaid` (not a `CodeBlock` with a language), so the painter
    never sniffs a fence's language to know whether to draw a diagram or a
    listing. Rendering goes diagram source → `mermaid-rs-renderer` → SVG →
    the **existing** `resvg` path an SVG document already takes, so no new
    raster dependency (§3's own note). Measured: a flowchart renders 403×112
    px, a sequence diagram 450×265 px.
  - **⚠️ Finding for the operator: the library draws more than §3 promises.**
    `mermaid-rs-renderer 0.2` also renders **class, state and gantt**
    diagrams. §3's contract is "flowchart and sequence" with
    class/state/gantt/ER/journey on the not-delivered side, and AC2's rule
    is that nothing is over- or under-delivered against that table — so this
    control **refuses the others by name** ("Mermaid 'classdiagram' diagrams
    are not supported — this Viewer draws flowchart and sequence diagrams")
    and shows that reason beside the diagram's own source, which is T20's
    "not attempted, not silently ignored". **If the operator wants the wider
    set, §3 is the thing to widen — the code is two match arms behind it.**
  - A diagram's source is part of its searchable text, so a node's label is
    findable. The layout keywords come along with it; that is the honest
    trade against parsing the diagram a second time just for Find.

## Stage H — HTML subset (fidelity wave 4)

- [x] **T21 — HTML parse → the shared layout model** (R7, AC2)
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

- [x] **T22 — Confirm Find/Save-As/Print/Share need nothing HTML-specific**
      (R18–R20, R26–R31)
  - Files: none expected — this is a verification task
  - Do: run Stage D/Stage C's tests against an HTML-subset document.
  - Verify: `cargo test -p cobolt-forms -p cobolt-runtime` — if any of these
        needs an HTML-specific branch, that's new scope to fold back in here,
        not silently patched elsewhere later.

  - **DONE — 2026-09-19 (1.70.92).** HTML maps onto the **same** `Block`/
    `Inline` primitives the Markdown walker produces — plan.md §4's decision
    expressed in the type system (`pub type LayoutDocument =
    MarkdownDocument`). One layout model, one painter, and every rendering
    fix reaching both formats.
  - **⚠️ plan.md §4's parser choice REVERSED, deliberately: `tl`, not
    `html5ever`.** §4 chose html5ever and explicitly left `tl` flagged "for
    implementation-time reconsideration". Reconsidered and taken, on §4's
    *own* argument: a Viewer's dependencies compile into **every** binary
    that drops the control on a form, with no "strip it if unused" escape
    hatch. `tl` has **zero dependencies**; html5ever brings markup5ever,
    string_cache (with build-time codegen), tendril, phf and futf. spec.md
    §3 already disclaims W3C conformance in as many words ("Not a browser"),
    and the mapping work on top is identical either way — which was
    html5ever's only real advantage.
  - **An unknown element is descended into, never dropped.** A `<div>`, a
    `<section>`, a custom element contribute their children — which is what
    "degrade to the supported subset" means in practice: a grid-laid-out
    page loses its grid and keeps its content. `<script>`, `<style>`,
    `<head>`, `<title>`, `<meta>` and `<noscript>` are dropped with their
    contents, so JavaScript is neither run nor shown.
  - **§3's "colours" are honoured where a subset renderer can honestly read
    them** — `<font color>` and an inline `style="color: …"`. A stylesheet
    is not consulted: that is a cascade, and a cascade is a browser.
    `TextStyle` gained `color: Option<String>`, `None` on every Markdown run
    (meaning "the theme's ink"), and `parse_html_color` reads `#rgb`,
    `#rrggbb`, `rgb(…)` and the sixteen original HTML colour names.
  - **Parsing never fails.** HTML a COBOL program received from a
    `RestClient` is not guaranteed well-formed, and refusing to show a page
    because a tag was unclosed is the wrong answer for a *viewer*: whatever
    parses, renders.
  - **T22's answer: nothing needed an HTML-specific branch.** Find searches
    an HTML document through the same `find_matches`/`SearchableText` that
    serve text, Markdown and PDF, with the same case toggle and the same
    wraparound — a table cell and an image's alt text are both findable.
    R18/R18.1's naming follows the resolved format with no branch either.
    Print and Share never look at the document at all: they hand the OS a
    file, so a format cannot change what they do.

## Stage I — Conversation mode (§8, sequenced last — reuses Stage H's HTML layer)

- [x] **T23 — `append_html`/`append_markdown`/`append_raw`/`append_to_message`**
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

- [x] **T24 — Auto-follow scrolling** (§8.3, AC15, AC18)
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

- [x] **T25 — New-content indicator, and the `RenderAsHtml` override** (§8.1,
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

- [x] **T26 — Sanitisation** (§8.5)
  - Files: `crates/cobolt-forms/src/viewer.rs`
  - Do: strip scripts, inline event handlers and unsafe URL schemes from HTML/
        Markdown-derived content before it's appended; Raw's escaping (T23)
        already covers its own case.
  - Verify: `cargo test -p cobolt-forms` — a battery of unsafe HTML fragments
        (script tags, `onclick=`, `javascript:` URLs) each confirmed stripped
        or neutralised, reporting which rule caught each case.

- [x] **T27 — Performance: batching, incremental layout, stable ids** (§8.6)
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

  - **Stage I DONE — 2026-09-19 (1.70.93).** `viewer::Conversation` is the
    model: messages with stable ids, each made of chunks that keep **the
    mode they arrived in**, with layout derived per chunk. The interpreter
    owns one per control and exposes `AppendHtml`, `AppendMarkdown`,
    `AppendRaw`, `AppendToMessage(id, content, mode)` and `JumpToLatest`.
  - **Storage stays native, per chunk — a deviation from this file's own
    preamble, and a deliberate one.** The preamble says a conversation's
    substrate for stitching is HTML. Applied one level down (plan.md §3's
    actual rule) each chunk keeps its own form — HTML text, Markdown text,
    or raw literal text — and `Conversation::to_html()` assembles the stream
    **on demand** rather than holding a second copy that could drift.
    Converting Markdown to HTML on arrival would discard the source for
    nothing: the layout is derived from the chunk either way, and a
    Markdown→HTML serializer is pure loss.
  - **§8.2 item 5 is measured, not asserted.** `Conversation::relayouts()`
    counts messages laid out. Building 2000 messages costs 2000 passes; the
    2001st append costs **1 pass in 833 ns**, where a rebuild would have
    cost 2001. Consecutive same-mode chunks merge before layout, so a
    token-at-a-time stream is one chunk and one block, not five hundred.
  - **Auto-follow and the indicator are one state machine** (`AutoFollow`) —
    they are two faces of the same question ("is the reader at the end?"),
    and splitting them is how they drift apart. Threshold 28 px, inside
    §8.3's 24–32. The same append pins a reader 5 px from the end to 1400
    and leaves a reader 300 px up at exactly 300.
  - **§8.5's inline handlers are neutralised structurally**, not stripped:
    the HTML walker reads only `href`, `src`, `alt`, `title`, `color`,
    `style` and `start`, so an `on*` attribute is never looked at.
    `html_has_event_handler` exists so a test can prove that rule, and so a
    future attribute reader cannot quietly widen the surface without it
    noticing. `iframe`/`object`/`embed`/`form` and friends join
    `script`/`style` on the dropped list; an unsafe URL scheme
    (`javascript:`, `vbscript:`, `file:`, `data:` other than an image)
    loses the LINK and keeps the words.
  - **⚠️ The engine-side incremental path is the deferred host-session work.**
    The interpreter publishes `_ConversationHtml` (the assembled stream) for
    the painter, which T35 memoize-parses by that string. That is one parse
    per *append*, not per frame — bounded, but not the incremental path the
    model itself implements. Closing it is the same `ViewerSession`
    plumbing R5.1 is waiting on (see T12's note).

## Stage J — Streamed layout and conversation management (§8.8)

*(Numbered T35–T37, past Stage K's T28–T34, to avoid renumbering the wrap-up
stage — the same out-of-sequence pattern this project's specs already use
(spec.md's R26–R31 and AC19). Placed here, physically, because it depends on
Stage I's append machinery existing and belongs right after it in reading
order.)*

- [x] **T35 — `Layout = Streamed`: one pane, no chrome** (R7, §8.8, AC25)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: the fifth `Layout` value paints only the conversation content — no
        toolbar, Find bar, thumbnail or filmstrip chrome, regardless of what
        was showing under the previous `Layout`. Nothing else changes: the
        append/auto-follow machinery from Stage I is reused as-is.
  - Verify: `cargo test -p cobolt-forms --features render` —
        `streamed_layout_draws_a_single_pane_and_nothing_else` (plan §6):
        shape count under `Streamed` compared against every other `Layout`
        value, asserting zero chrome shapes (**AC25**).

- [x] **T36 — Conversation management: `NewConversation`, `SelectConversation`,
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

- [x] **T37 — The three conversation events** (§8.8, R32)
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

  - **Stage J DONE — 2026-09-19 (1.70.94).** `Streamed` paints the
    conversation instead of a document, through `chrome_layout`'s existing
    refusal to place any chrome. **AC25 measured:** with every chrome switch
    turned ON, `Raw`/`Web`/`Print`/`Page` each paint 171 chrome shapes and
    `Streamed` paints **0** — a zero that means something because the others
    are not zero.
  - **The AC25 test's first classifier was wrong, and the fix is worth
    keeping in mind:** chrome was classified by POSITION (anything in the
    top band or the left rail), which mistook `Streamed`'s own content — it
    legitimately starts at the top — for a toolbar. The honest discriminator
    is each layout's **own content rect** from `chrome_layout`: chrome is
    what falls outside it.
  - **History is one model in `cobolt-forms`** (`ConversationHistory`,
    `ConversationEntry`, `HISTORY_CAP`), used by **both** the interpreter and
    `viewer_session` — T36 has to satisfy tests in two crates, and two
    implementations of one rule is how they drift. `viewer_session` now
    `pub use`s the type rather than declaring its own.
  - **§8.8's memory rule is structural**: a `ConversationEntry` has an id and
    a title and **no third field**, so a selection has nothing to restore
    even if someone tried. The test serialises the whole entry to prove it.
  - **Ordering, tested: the pane is cleared, *then* the event fires.** A
    handler bound to `onConversationCreated` or `onConversationSelected`
    sees an empty pane, never stale content — asserted for both methods.
    Clearing republishes the stream **without** `onContentRendered`, since
    §8.2 item 6's event is about newly appended content finishing layout,
    not about a pane emptying.
  - An archive titles itself from the conversation's **own first line**,
    trimmed to 60 characters: a history list of "Conversation 1…10" tells a
    reader nothing. Re-archiving an id **moves** it rather than duplicating
    it.

## Stage K — surfaces, parity, docs, KB, finalize

- [x] **T28 — IDE property editors** (R22)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`
  - Do: grouped editors — `Layout`/`Format`/`SplitMode` dropdowns; the
        `View1`/`View2` groups appear only when `SplitMode != None`.
  - Verify: `cargo test -p cobolt-ide --bins`; **manual:** every property in
        plan §3 is editable from the Properties panel.

- [x] **T29 — Codegen** (R22)
  - Files: `crates/cobolt-codegen/src/lib.rs`
  - Do: declaration + handler stubs for whichever events the developer has
        bound in the Designer, the standard banner intact — no per-event
        special-casing needed; the mechanism is already generic, it just now
        has ~16 events to be generic over instead of 3.
  - Verify: `cargo test -p cobolt-codegen`; generated `.cbl` parses under
        `cobolt-parser` and checks clean under `cobolt-semantic` for a form
        binding at least one of the new events (not only the original three).

- [x] **T30 — Engine parity: designer canvas vs. running form** (R25, AC11)
  - Files: `crates/cobolt-forms/tests/`
  - Do: a parity test in the shape of
        `engine_reference_form_parity_static_vs_faces`, covering every format
        wave landed so far.
  - Verify: `cargo test -p cobolt-forms --features render` — same shapes/fills
        on both paths for every format (**AC11**, closing the loop T11 opened).

- [x] **T31 — Rounded-corner measurement** (steering: spec 057)
  - Files: `crates/cobolt-forms/src/render.rs` (`self_clipping_type`, if the
        measurement says Viewer needs the exclusion)
  - Do: nothing assumed — run the harness, read its verdict.
  - Verify: `cargo test -p cobolt-forms
        a_child_at_a_rounded_corner_stays_inside_the_arc` green; if it reports
        Viewer painting past the arc, add the exclusion and re-run, don't
        pre-guess it in an earlier task.
  - **Already answered at T12 (2026-09-19).** The harness went red as soon as
    T12's toolbar band and filmstrip rail landed — measured 113 px past the
    arc — so `Viewer` was added to `render::self_clipping_type`'s exclusion
    list there, with the number and the date in the comment. That is this
    task's rule working as written (measure, then act), not a pre-guess: the
    paint that earned the exclusion is what triggered it. **T31 is now a
    re-run and a read** — confirm the harness is still green and that the
    allow-list still equals what it measures.

  - **T28–T31 DONE — 2026-09-19 (1.70.95).**
  - **T28:** grouped editors in `properties.rs` — Document, Layout & view,
    Split view, Find and Conversation — with the `View2*` group and the
    divider appearing **only** when `SplitMode != None`. Six new `Tr` fields
    in all six languages, and a `text_prop_row` helper lifted out of the
    Snackbar's own `Text` row. Two tests: every property paints an editable
    row, and the second view's rows appear only once the control is split.
  - **T29 needed no codegen change at all**, exactly as the task predicted.
    **51** bound events on one Viewer produce 51 handler stubs; the
    generated program parses with **0** diagnostics and checks clean with
    **0** semantic errors. An unbound Viewer still generates a valid
    program. `cobolt-parser`/`cobolt-semantic` became **dev**-dependencies
    of `cobolt-codegen` so T29's own Verify ("parses under cobolt-parser and
    checks clean under cobolt-semantic") could actually be run.
  - **T30 caught a real AC11 bug, which is what it is for.** The running
    form painted from its own arm and skipped `draw_control`'s frame
    wrapper, leaving it **five shapes short of the canvas**. Fixed
    structurally: the arm now makes **exactly one** painting call — the same
    `paint::draw_control` the canvas makes, which handles the split, the
    chrome and the divider inside itself — and then only **senses**, against
    what that paint measured (`paint::viewer_stash_measurements` /
    `viewer_measurements`). A gesture's visual effect is therefore one frame
    behind, which is imperceptible, and is the price of there being one
    paint rather than two that can disagree.
    **Measured, exact, shape-for-shape:** text/Page 198, text/Raw 128,
    Markdown+Mermaid/Web 135, image/Print 128, PDF/Page 198, HTML/Web 131,
    split LeftRight 207, split TopBottom 253 — identical on both surfaces,
    positions and fills included.
  - **T31 was already answered at T12** (see its note): the harness measured
    113 px past the arc as soon as the toolbar band and filmstrip landed,
    and `Viewer` went into `render::self_clipping_type`'s exclusion list
    there. Re-run and green.

- [x] **T32 — System KB** (steering: hard constraint)
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

  - **DONE — 2026-09-19 (1.70.96).** All four `cobolt-compiler` doc tables
    carry the Viewer: `control_purpose` (including §3's fidelity boundary in
    the developer's own words — a developer must learn what a format does
    NOT deliver from the control's documentation, not from the spec),
    `property_reference` (16 entries), `event_reference` (22 events) and
    `control_method_docs` (17 methods), plus a `## Viewer (spec 058)`
    section in `methods_reference_doc`.
  - **`chunked.data` actually changed**, which is the proof the right file
    was edited: `8807ac46…` → `7e036aed…`, 1700 records from 8 documents.
    `prebuilt_chunked_kb_matches_the_published_documentation` is green — and
    it had been **red before this work started**, for
    `Knowledge Base/form_designer_controls.md`, so regenerating here fixed a
    staleness that predates spec 058.
  - New test `spec_058_viewer_is_fully_published_in_the_system_kb`, on
    spec 039's template plus what its own comment asks for: the needles
    cover the **whole** surface — at least one R32 event and at least one
    §8.8 conversation method — so a section copied from another control
    could not pass it.

- [x] **T33 — Docs & i18n**
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

  - **DONE — 2026-09-19 (1.70.97).** `docs/developers-guide-en.md` gains
    `### Viewer (documents inside your form)` before §9, written for a
    PowerCOBOL/isCOBOL reader: what a Viewer is *instead of* (an OLE
    container or an embedded preview, with no container to register and no
    second process to fail), §3's fidelity table in the developer's own
    terms with a **You do not get** column, every navigation and Find
    affordance, split view, Save As/Print/Share, and the Streamed
    conversation surface with a **worked minimal chatbot form** — a Viewer, a
    ListBox and two buttons, since that is the one part of this material a
    reader has no prior instinct for. COBOL examples and prose only; the
    word "Rust" does not appear in the section. Two screenshot placeholders,
    each saying exactly what to capture. UTF-8 clean, zero double-encoded
    bytes.
  - **GOLDEN RULE #8: nothing to delete.** The five translations of the
    Guide **do not exist on this branch** — `docs/` holds only `-en` files.
    `every_document_ships_in_every_language` was therefore **already red
    before this work began** (verified: `git diff ca0ea64..HEAD` touches no
    file under `docs/`), which is precisely the expected signal plan.md §2
    predicted for the regeneration cycle.
  - **The Guide found a real API gap while being written.** The streaming
    example needed the id of the message an append created, and the append
    methods returned nothing — which would have left `AppendToMessage`
    unusable without the caller inventing ids. `AppendHtml`/`AppendMarkdown`/
    `AppendRaw` now **return the new message's id**, with a test. A second
    example used `SelectedItem`, which no ListBox has; rewritten to
    `SelectedIndex` against the program's own table, which is what a COBOL
    developer would really write.
  - **i18n:** six new `Tr` section headers (T28) plus `Language::
    viewer_tooltips()` — R16's twelve toolbar tooltips in all six languages,
    installed into the render engine each frame by `app.rs` the same way
    `theme::set_active` publishes the palette. A **table**, not nineteen
    `Tr` fields: these belong to a control's chrome and have exactly one
    consumer. Two tests: every language supplies every tooltip in the same
    order, and **no language quietly ships the English strings**.
    `cobolt-forms` keeps its own English fallback, because a compiled COBOL
    binary has no `Tr` table at all.

- [x] **T34 — Finalize**
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

  - **DONE — 2026-09-19 (1.70.98).** The full sweep, every `test result:`
    line read, never grepped for failures:

    | Crate | Binaries | Passed | Failed |
    |---|---|---|---|
    | cobolt-forms (`--features render`) | 43 | 1072 | 0 |
    | cobolt-runtime | 113 | 936 | 0 |
    | cobolt-form-host | 3 | 133 | 0 |
    | cobolt-codegen | 5 | 67 | 0 |
    | cobolt-compiler | 3 | 132 | **1** |
    | cobolt-ide (`--bins`) | 1 | 1215 | **1** |

  - **The two failures are both pre-existing and neither is spec 058's.**
    - `cobolt-ide::docs_embed::every_document_ships_in_every_language` —
      "Portuguese fell back to English for developers-guide-en.md,
      indexed-redb-engine-en.md". `docs/` on this branch holds **only** `-en`
      files; `git diff ca0ea64..HEAD` touches nothing under `docs/` but the
      English Guide. This is **exactly** the red plan.md §2 predicted for
      GOLDEN RULE #8's regeneration cycle, and it was red before this work
      started.
    - `cobolt-compiler::external_crates_build_run_manifest_and_determinism` —
      "csv missing from lock". A spec 044 end-to-end test that builds a real
      Cargo project in a shared `$TMPDIR` and vendors crates from the
      registry. Environmental, per this project's own standing rule about
      live-network failures; nothing in spec 058 touches it.
  - **AC30's named test now exists, in both halves**, because it did not
    before and T34's checklist is the only thing that would have noticed:
    `every_event_in_r32s_table_fires_at_its_documented_moment` in
    `cobolt-forms` (nine gesture-driven events, one table row each, PASS/FAIL
    reported by name) and the same name in `cobolt-runtime` (fourteen
    method- and load-driven events, plus a row proving a control that is
    only **read** raises nothing at all).
  - **Writing it found two real defects**, which is the argument for having
    written it:
    1. **`onError` fired TWICE for one failed load.** The `View1*` alias
       mirror re-entered the load hook. Fixed with a re-entrancy guard, and
       a failed load now leaves **both** spellings of `Source` pointing at
       the document still on screen (R4) rather than having the mirror undo
       the restore.
    2. **The unprefixed alias kept a raw value where its `View1*` twin was
       canonicalised** — `SearchCaseSensitive` read back `1` while
       `View1SearchCaseSensitive` read `true`. The alias now adopts the
       canonical spelling's settled value.
  - Two test-side findings worth keeping: **R15 hides the fullscreen button
    in fullscreen**, so `onFullscreenExited` must be reached with `Esc` (a
    second click is impossible, and the first draft of the test tried it);
    and **a key must be RELEASED** for `onScrolled` to fire, because the
    event is about the content coming to *rest* and egui holds a key down
    until a release arrives.
  - **NIST is not required**, as the task says: every Viewer method is
    ordinary method dispatch and nothing here touched the interpreter's
    grammar. Re-confirmed — the parser and lexer are untouched by this
    feature.

## Done criteria

All 32 acceptance criteria in `spec.md` (AC1–AC11, AC19–AC24, AC30–AC32 from
§6; AC12–AC18, AC25–AC29 from §8.7) are checked, every suite green, docs and
KB updated, and the change sits in feature commit(s) on `features` (do **not**
commit or push unless the operator asks).

> **Still open for the operator, recorded rather than quietly dropped:**
> 1. ~~**R5.1's background thread is not wired into the render path.**~~
>    **CLOSED — 2026-09-19 (1.70.99).** Built exactly as T12 designed it: a
>    defaulted `FormState::viewer_document(base, source)` hook the host
>    overrides and the designer canvas does not, published once per form in
>    `render_form_inner` and read by `draw_control_body`'s `CT::Viewer`
>    branch. `ViewerSession` now does real work — `ViewerJob::OpenDocument`
>    indexes, decodes the page in view and a **bounded window of 24 page
>    previews**, all on the control's own named thread — and
>    `FormBody::tick_viewers` asks and drains once a frame.
>    **Measured on a 6.3 MB / 400-page log: `request()` returns in 9.125 µs,
>    the worker finishes 54.7 ms later.** The hook takes the SOURCE as well
>    as the control, a deliberate widening of T12's sketch: one control can
>    show two documents (R21), and a per-control answer could only ever be
>    right about one of them.
> 2. ~~**R29's coloured highlight overlay is painted on the single-galley
>    path only.**~~ **CLOSED — see the entry under T15.**
> 3. **The manual pass below is the operator's**, per this project's standing
>    "never drive the application" rule.

**Coverage map** — AC1 T7/T34 · AC2 T9/T10/T18/T20/T21 · AC3 T11 · AC4 T12 ·
AC5 T12 · AC6 T13/T19 · AC7 T15/T16/T28 · AC8 T16/T17 · AC9 T6 · AC10 T3/T13 ·
AC11 T11/T30 · AC12 T23 · AC13 T23 · AC14 T25 · AC15 T24 · AC16 T23 ·
AC17 T23 · AC18 T24 · AC19 T12 · AC20 T13 · AC21 T6 · AC22 T15 · AC23 T15 ·
AC24 T15 · AC25 T35 · AC26 T36 · AC27 T36 · AC28 T36 · AC29 T36 · AC30 T34
(exercising T11/T12/T13/T15/T16/T23/T37's events) · AC31 T12 · AC32 T12.
