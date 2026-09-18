# Plan — Viewer Control

- **Status:** approved
- **Spec:** ./spec.md   **Date:** 2026-09-18

## 1. Approach

Viewer is the **44th `ControlType`** (the catalogue is 43 today — `ControlType::ALL`
has 43 entries per `cobolt-forms/src/icons.rs`'s own coverage assertion; spec.md
§7's "grows from 42 to 43" is stale by one control and worth fixing in passing,
though `/plan` doesn't edit `spec.md`). Unlike Snackbar (43rd, non-visual), Viewer
is **visual** and paints continuously — closer in shape to DataGrid than to any
non-visual control.

Three mechanisms in this spec are genuinely new to the codebase; everything else
is an instance of a pattern already proven by an existing control. The plan is
organized around naming which is which, because that's where the real risk is.

| Mechanism | Precedent in this codebase | Verdict |
|---|---|---|
| Per-instance dedicated background thread, bounded decode cache (R5.1, R2) | `doc_viewer.rs`'s `start_preparer`/`prepare_thread` (jobs/done channels, named thread, bounded per-frame drain) — but that's a **singleton**, one Documentation panel. `async_op.rs`'s `spawn_rest_op` is thread-**per-call**, not per-instance. Neither generalizes as-is. | **New**: generalize the doc-viewer shape to N instances, host-owned (§4). |
| Two independently-scrolled/zoomed/searched views of one control's own data (R21, R21.2) | `Splitter` owns two *developer-droppable* child Panels (wrong shape — different content by design). The debugger's hand-rolled split shows two *different* panes, not the same content twice. `TabControl` shows one of N, never two at once. | **New**: no existing control does "one control, two viewports of its own state." Reuses only `splitter::geometry()`'s pure percent-split math for the divider rect. |
| Markdown+Mermaid, PDF, HTML-subset decoders | `doc_viewer.rs`/`md_render.rs` already do Markdown (via `pulldown-cmark`) and Mermaid (via `mermaid-rs-renderer` + `resvg`) — well proven, **but `cobolt-ide` is a binary crate with no `lib` target**, so nothing in `cobolt-forms` can call it. PDF and HTML have **no existing code anywhere** in the workspace (confirmed by exhaustive grep — see §4). | **New code, in the right crate**: `cobolt-forms` needs its own copy of the Markdown/Mermaid pipeline (same crates, fresh integration) and net-new PDF/HTML pipelines. |

Everything else — the `ControlType` variant, `Control::new` seeding, `as_str`/
`from_str`, icons, the System KB doc tables, i18n, `tests/controls/` — is the
same checklist this codebase has executed 43 times before, most recently for
Snackbar. §2 below is written as that checklist, with the **silent-failure
traps this codebase has already been bitten by once** called out explicitly,
because they don't show up as compile errors.

**A fourth thing added after this plan's first draft is not architecturally
new, just larger than usual:** R32's comprehensive event surface (~16 events)
reuses the exact `onComplete`/`onCancelled` async-lifecycle pattern already
proven by `RestClient`/`SqlDatabase`/`IndexedFile`/`Maps`/`WebSearch` (spec
032) — no new dispatch mechanism, just more entries through the same
`FormRequest`-style channel R6 already uses for `onLoadProgress`/`onLoaded`.
§8.8's conversation-management API (`NewConversation`/`SelectConversation`/
`RegisterConversation`/`HistoryList`) is genuinely new in one specific way —
history holds only an id and a title, **never** a past conversation's
rendered content, so selecting one is always a fresh request back to the host
rather than a cache restore (§3, §4) — everything else about it is ordinary
method/property/event plumbing, the same shape as every other control's
COBOL surface.

**Requirement coverage:** R1–R6 the decode engine/host session; R7–R10 layout
modes and the format renderers; R11–R15 navigation chrome; R16–R20+R18.1 the
toolbar and its actions; R26–R31 Search; R32 the event surface; R21–R21.2
split view; R22–R23 the COBOL surface (cross-cutting); R24–R25 boundaries
(governance, cross-cutting); §8/AC12–18 the conversation-mode extension and
§8.8/AC25–29 Streamed layout (§7 below, the least-precedented part of this
plan — flagged honestly as such, not dressed up as proven).

## 2. Affected crates / files

**`cobolt-forms`** (the pure model + the one render engine, spec 017)
- `src/model.rs` — `ControlType::Viewer` variant; `Control::new` arm seeding the
  properties in §3; `as_str()`/`supported_events()`/`default_size()` (all three
  **exhaustive — a compile error if forgotten**); `from_str()` (⚠️ **NOT
  exhaustive — silently deserializes a saved Viewer control as `Custom` on next
  load if forgotten; no test catches this today, see §5**).
- `src/viewer.rs` **(new)** — pure, `egui`-free model code: `ViewerLayout`
  (`Raw`/`Web`/`Print`/`Page`/`Streamed` — §8.8), `ViewerFormat` (`Text`/`Markdown`/`Image`/`Pdf`/
  `HtmlSubset` — R0's honest naming), `SplitMode` (`None`/`LeftRight`/
  `TopBottom`), each with the `as_str()`/lenient-`from_str()` idiom
  `DataGridGridLineStyle` already establishes (`model.rs:163-181`); the Markdown
  walker (pulldown-cmark events → a layout model, informed by but not copied
  from `cobolt-ide`'s `md_render.rs`, since that crate is unreachable); the
  HTML-subset parser-to-layout mapping; the PDF text/vector extraction wrapper;
  image format dispatch; the Find match-computation pure function (modeled on
  `code_search.rs`'s `find_matches()`, reimplemented here since that file is
  also `cobolt-ide`-only); the two-view divider geometry (reuses
  `splitter::geometry()`'s percent-split math, not its child-Panel model).
- `src/paint.rs` — `draw_viewer(painter, rect, &ViewerPaintState, …)`, reusing
  the existing glass/theme primitives; the design-canvas preview path (§4).
- `src/render.rs` — the `render_interactive` arm for `ControlType::Viewer`
  (⚠️ **this match has a wildcard fallback — forgetting the arm compiles fine
  and silently gives Viewer the generic static-face painter with no
  interactivity; this exact failure already hit Snackbar/WebSearch/IndexedFile
  once, per the comment already sitting above that match**); `self_clipping_type`
  — Viewer's toolbar/thumbnail-chrome geometry likely needs the same exclusion
  DataGrid has, but that must be **measured** by the harness test in §5, not
  assumed.
- `src/icons.rs` — `control-viewer` catalogue entry (required — coverage test).
  Two genuinely new action icons (confirmed absent by grep): a Find/magnifying-
  glass glyph and a case-sensitivity "Aa" toggle. Everything else R16 needs
  **already exists**: `chevron-up`/`down`/`left`/`right` (Previous/Next),
  `highlighter` (highlight toggle), `fullscreen`, **`split-view`** (already
  named exactly that), `share`, `printer`, `zoom-in`/`zoom-out`, `doc-save-as`.
  Layout-switcher, font-size and thumbnails/filmstrip icons need a targeted
  check at implementation time — not confirmed either way by this research.
- `Cargo.toml` — new deps, all pure-Rust, none touching the two real C crates
  in this workspace (`libsqlite3-sys`, `openssl-sys` — see §4 Key decisions):
  `pulldown-cmark` (match `cobolt-ide`'s `0.12`, `default-features = false`),
  `mermaid-rs-renderer` (match `cobolt-ide`'s `~0.2`), `lopdf` (~0.26 — already
  resolved in this workspace's `Cargo.lock` transitively via `genpdf`, so this
  promotes an already-proven-to-build crate to a direct, read-capable
  dependency rather than introducing an unknown one), `html5ever` (new). Extend
  the existing `image` dependency's feature list (currently `png`,`jpeg` only,
  optional behind the `render` feature) to add `gif`, `webp`, `bmp`, `tiff` —
  `resvg` (SVG) is already present. Whether animated GIF/WebP/APNG playback
  reuses `cobolt-media` directly or needs its own minimal frame-timing is a
  **to-verify item** (§5) — the research did not confirm whether `cobolt-forms`
  already depends on `cobolt-media` for the Animator control.

**`cobolt-form-host`** (the live per-instance state — engine stays pure)
- `src/viewer_session.rs` **(new)** — modeled directly on `snackbar_stack.rs`'s
  shape: `ViewerSession` per control id, owning the dedicated named background
  thread (`thread::Builder::new().name(format!("viewer-{ctrl_id}"))`), a
  jobs/done channel pair, a **bounded** decoded-page cache (LRU-evicted,
  budget-configurable — modeled on `indexed_disk.rs`'s bounded directory cache
  philosophy, since nothing in `cobolt-forms`/`cobolt-media` bounds memory
  against *source* size the way R2 needs), and one or two `ViewState` structs
  (source/page/zoom/scroll/search-cursor) depending on `SplitMode`. Clean
  shutdown on disposal: drop the jobs channel so the thread's `recv()` unblocks
  and it exits on its own — the same detach-don't-join shape `DebugRunner::stop()`
  already uses, for the same reason ("never block the UI thread").
- `src/host.rs` — `FormBody` gains `viewer_sessions: HashMap<String,
  ViewerSession>`, ticked (drain the done-channel, bounded per frame, like
  `doc_viewer.rs`'s `drain_prepared`) and painted per frame, the same place
  `SnackbarStack` already hooks in.

**`cobolt-runtime`**
- `src/interpreter.rs` — method dispatch: `"LOADBYTES"`, `"FINDNEXT"`,
  `"FINDPREVIOUS"`, `"SAVEAS"`, `"PRINT"`, `"SHARE"`, and the conversation-mode
  `"APPENDHTML"`/`"APPENDMARKDOWN"`/`"APPENDRAW"`/`"APPENDTOMESSAGE"` in the
  control-method table (the `"SHOW"`/`"DISMISSALL"` precedent, ~line 10949) and
  the known-method list (~13730). Event delivery for `onError`/`onLoadProgress`/
  `onLoaded` via a `FormRequest`-style message, the same shape Snackbar's raise
  uses.
- `src/form_host.rs` — the request/event pair carrying load-progress and error
  payloads from the host session back into COBOL-visible state.

**`cobolt-ide`**
- `src/panels/toolbox.rs` — the palette entry (⚠️ **`TOOLS` is a plain static
  list with no test cross-checking its length against `ControlType::ALL` — a
  forgotten entry compiles fine and Viewer simply never appears in the
  Designer's toolbox; see §5 for a proposed coverage test that closes this for
  every future control, not just this one**); `paint_control_icon`'s dispatch
  (same risk shape — falls back to a generic stroked rectangle silently).
- `src/panels/properties.rs` — grouped property editors: `Layout`/`Format`/
  `SplitMode` dropdowns, the `View1`/`View2` property groups when `SplitMode !=
  None` (§3).
- `src/i18n.rs` — every new label/tooltip/diagnostic, **six languages**: layout
  names, toolbar tooltips, Find-bar labels ("Case sensitive", "Highlight
  results", "N of M"), error strings.

**`cobolt-codegen`** — `src/lib.rs`: declaration + `onError`/`onLoadProgress`/
`onLoaded` handler stubs (the Snackbar `onButtonClick`-stub precedent).

**`cobolt-compiler`** — `src/lib.rs`: entries in the four System KB tables
(`property_reference`, `event_reference`, `control_purpose`,
`control_method_docs` — all keyed by name, not `ControlType`-exhaustive, so
these are easy to under-fill without a compile error; §5's coverage test is
what actually catches a gap). Then `cargo run -p cobolt-ide --example
build_chunked_kb` and commit the regenerated `assets/knowledge/chunked.data`
**in the same change**.

**`docs/developers-guide-en.md`** — a Viewer section (COBOL examples only, no
Rust, per the Guide's own standing rules). Per Golden Rule #8: this change
**deletes** `developers-guide-{es,pt,fr,jp,cn}.md` — they are not patched, and
the two `docs_embed.rs` guards (`every_document_ships_in_every_language`,
`every_translation_is_complete_and_current`) are **expected to go red** until
the next operator-declared minor/major regenerates all six languages. That red
is correct, not a bug to chase.

**`tests/controls/viewer_test.rs`** **(new)** — the established per-control
round-trip pattern (34 files today → 35).

## 3. Data / model changes

**Storage is native-format, always — rendering is a separate, derived step.**
Operator correction, 2026-09-18, replacing an earlier misreading of §8.1: every
document's **stored** representation is its own source format, never a shared
intermediate one — a Markdown document's internal storage is its Markdown
text, an HTML document's is its HTML text, a PDF's is its PDF bytes, an
image's is its own encoded bytes; each **lives in that format** whether in
memory, on disk, or on screen. What gets **displayed** (tables, links, images,
formatted text) is computed *from* that stored form at layout/paint time —
cacheable for performance, but never the system of record and never a
replacement for it. This is what makes R18's byte-identical Save As correct by
construction rather than a special case, and what keeps R24 ("never modify the
source") structurally true rather than merely intended. The "shared
layout-primitive set" decision in §4 is about that *derived* computation, not
about storage — see its amended wording there.

**`.cfrm`:** one new `<Control type="Viewer">`. `ControlType` serialises **by
name** (confirmed against the Snackbar precedent), so appending the variant is
backward compatible.

**Properties** (all COBOL r/w per R22; naming decision explained in §4):

| Property | Type | Notes |
|---|---|---|
| `Source` | string | path; alias for `View1Source` when `SplitMode = None` |
| `Format` | string, read-only | resolved per R3/R0 |
| `Layout` | string enum | `Raw`/`Web`/`Print`/`Page`/`Streamed` (§8.8) |
| `Zoom` | int (%) | alias for `View1Zoom` |
| `FontSize` | int | |
| `ShowThumbnails` / `ShowFilmstrip` | bool | |
| `CardSize` | int (%) | the R14.1 slider |
| `Fullscreen` | bool | |
| `Progress` | int, read-only | 0–100 |
| `LastError` | string, read-only | |
| `SplitMode` | string enum | `None`/`LeftRight`/`TopBottom` |
| `View1*` / `View2*` | — | `Source`, `Page`, `Zoom`, `ScrollPosition`, `SearchText`, `SearchCaseSensitive`, `SearchHighlightEnabled`, `SearchCurrentMatch` (read-only), `SearchMatchCount` (read-only), `FindOpen` (bool) — present only meaningfully once `SplitMode != None`, but always addressable |
| `RenderAsHtml` | bool, default true | §8.1's global override — see §7 |
| `HistoryList` | string, read-only, multi-line | one `id\|title` per line, ≤10 lines (§8.8) — the `Buttons` multi-line-list idiom |

**Methods:** `LoadBytes(bytes)`, `FindNext()`, `FindPrevious()`, `SaveAs(path)`
(COBOL-driven Save As always takes an explicit path — R18.1's proposed-default-
filename behaviour is specifically the **interactive dialog's** convenience,
not the method's contract), `Print()`, `Share()`, plus the conversation-mode
`AppendHtml(content)` / `AppendMarkdown(content)` / `AppendRaw(content)` /
`AppendToMessage(messageId, content, mode)`, and §8.8's conversation-management
trio `NewConversation()` / `SelectConversation(id)` / `RegisterConversation(id,
title)`.

**Events:** the full ~16-event set in spec.md R32's table — the original
`onError`/`onLoadProgress`/`onLoaded` (R4/R6) plus a change event for every
other state R22 already makes COBOL-writable (`onLayoutChanged`,
`onZoomChanged`, `onThumbnailsToggled`, `onFilmstripToggled`,
`onFullscreenEntered`/`Exited`, `onFindOpened`/`Closed`, `onSplitModeChanged`),
a Complete/Cancelled pair for each OS-handoff action (`onPrintComplete`/
`Cancelled`, `onShareComplete`/`Cancelled`, `onSaveComplete`/`Cancelled`), and
the three §8.8 conversation events (`onConversationCreated`,
`onConversationSelected`, `onContentRendered`). All dispatched through the
same `FormRequest`-style channel R6 already uses — no new delivery mechanism,
just more call sites (§1, §5 risk).

**Live model (host-side only, never serialised — does not exist at rest):**
```
ViewerSession { ctrl_id, thread: JoinHandle, jobs_tx, done_rx,
                page_cache: BoundedLru<PageIndex, DecodedPage>,
                views: [ViewState; 1 or 2],
                history: VecDeque<HistoryEntry>  // capped at 10 (§8.8)
              }
ViewState { source, page, zoom, scroll, search: SearchState }
SearchState { text, case_sensitive, highlight_enabled, current_match, matches: Vec<Span> }
HistoryEntry { id: String, title: String }  // NEVER content — see §8.8, R2
```
When both views hold the same document (R21.1), both `ViewState`s hold a
shared `Arc`-style handle into the *same* `ViewerSession`'s decoded-document
state — "attach, don't reload" is enforced by construction, not by a check.
`history` holds only `HistoryEntry`, deliberately — a past conversation's
rendered content is never cached (§8.8's `SelectConversation` always re-asks
the host), which is what keeps a long Streamed-layout session's memory bounded
the same way the page cache bounds a large static document (R2, T7).

**No AST change** — every Viewer method is an ordinary `Stmt::Invoke`, matching
Snackbar's `Show()`; the bincode append-only hazard is untouched.

## 4. Key decisions & alternatives

- **The live session lives in `cobolt-form-host`, not `cobolt-forms`.**
  *Why:* the exact reasoning Snackbar's plan already established and this plan
  reuses verbatim — `cobolt-forms` is a pure renderer with no clock and no
  cross-frame ownership; the host already owns exactly this kind of state, and
  both `rcrun run-form` and a compiled binary consume `cobolt-form-host`, so
  both get the one implementation for free (R25 parity). *Rejected:* an
  engine-owned session — would give the Form Designer's static canvas a live
  thread and clock it has no business owning.

- **`View1`/`View2`-prefixed properties, aliased from the unprefixed names in
  single-view mode.** *Why:* R22 requires flat, named COBOL properties (no
  indexed/parameterized property access exists anywhere in this codebase to
  borrow instead); prefixing keeps the common case — one view, `SplitMode =
  None` — using the plain names a developer expects, with split view opting
  into the explicit names only when it's actually used. *Rejected:* an
  `ActiveView` selector property gating which view the plain names address —
  adds a stateful "which view am I talking to" hazard a COBOL program can
  forget to reset, silently reading/writing the wrong view; the prefixed
  scheme has no such failure mode.

- **Design-canvas preview is synchronous and capped; the runtime engine is
  async and windowed.** *Why:* the static Form Designer canvas does not run
  through `cobolt-form-host` at all — none of the three form hosts is "the
  designer canvas" — so it cannot rely on `ViewerSession`'s thread/channel
  machinery. `PictureBox` already sets the precedent for a synchronous,
  UI-thread, first-paint-blocks decode with an `egui::Context` texture cache
  for design-time preview. **This is the least-verified decision in this
  plan** — flagged in §5 as a risk, not asserted as settled, because no
  existing control needs to preview something this expensive (a multi-hundred-
  page PDF) at design time; the working default is "decode only the first page,
  synchronously, time-boxed" and confirm against AC11 (designer/run parity)
  once built.

- **`cobolt-forms` gets its own `pulldown-cmark`/`mermaid-rs-renderer`
  dependencies rather than sharing `cobolt-ide`'s.** *Why:* `cobolt-ide` is a
  binary crate with no `lib` target — nothing outside it can call
  `md_render.rs`, this is a hard constraint, not a preference. Duplicating the
  dependency (not the rendering *code*, which needs Viewer-specific layout
  modes `md_render.rs` doesn't have) matches the precedent already set by
  `cobolt-forms` carrying its **own**, narrower `image` dependency separate
  from `cobolt-media`'s. *Rejected:* extracting a shared rendering crate both
  `cobolt-ide` and `cobolt-forms` depend on — cleaner long-term, but a
  standalone refactor an order of magnitude bigger than this feature; not
  "surgical."

- **`lopdf` for PDF, promoted from transitive to direct.** *Why:* it is
  **already proven to build** in this exact toolchain (pulled today via
  `genpdf` → `lopdf`/`printpdf`, write-only), is pure Rust, and reading PDF
  object structure/text streams is squarely inside what `lopdf` is for.
  *Rejected:* PDFium — explicitly rejected by the operator (spec.md Decisions
  §9, Q1); no C dependency is acceptable here regardless of fidelity gain, and
  unlike `libsqlite3-sys`/`openssl-sys` (the workspace's two existing, narrow
  C exceptions), Viewer's dependencies compile into **every** binary that
  drops the control on a form — there is no "strip if the program never calls
  it" escape hatch the way there is for the `sql`/`http` runtime features, so
  the bar for a Viewer dependency is genuinely zero-C, not "acceptable if
  optional."

- **`html5ever` for the HTML subset, over the lighter `tl` crate.** *Why:*
  spec-conformant parsing reduces surprises on real-world HTML a COBOL program
  might receive (e.g. from `RestClient`); the subset-layout code on top is
  net-new regardless of parser choice, so the heavier, more standard parser
  doesn't cost extra layout work. *Rejected (not eliminated — flag for
  implementation-time reconsideration):* `tl` — lighter and faster, a
  reasonable fallback if `html5ever`'s footprint proves heavy in practice,
  since R25's "not a browser" framing already disclaims strict W3C conformance
  as a goal.

- **The Markdown and HTML renderers share one internal layout-primitive set**
  (headings/paragraphs/lists/tables/images) **for the derived rendering step
  only** — not two independent layout engines, and **not a shared storage
  format**: each document's stored form stays its own native text (§3). A
  Markdown file's canonical representation is its Markdown text throughout;
  the layout-model tree is computed from it at paint time, discarded/recomputed
  as needed, never persisted as a replacement. *Why:* keeps the HTML
  milestone's *incremental* cost to "parse HTML into the same shape Markdown
  already produces," not a second full layout engine — directly serves
  spec.md §7's fidelity-ordering decision (Markdown proves the layout
  primitives; HTML reuses them last). *Rejected:* fully independent renderers
  per format — simpler to reason about per-format, but duplicates every layout
  bug fix across two engines going forward.

- **Streamed layout draws no navigation chrome of its own — conversation
  management is a pure COBOL API.** *Why:* the operator ruled this out
  directly after an initial sidebar-reuse plan was investigated and found
  wanting on its own technical merits too — `ControlType::SideMenu` has no
  hamburger/trigger concept at all (its own code rejects one explicitly: "the
  header pane carries the logo... a ☰ would be a second affordance for
  something the whole pane already does") and isn't architecturally reusable
  by a second control (`state_for_control`/`form_content_pane`/`rail_view`
  are all hard-gated to `ControlType::SideMenu` specifically, not a shared
  entry point the way `splitter::geometry()` is). *Rejected:* building
  Streamed layout's own fixed sidebar (the original ask) — even setting the
  SideMenu findings aside, it would have been the one part of this whole
  control reachable only by mouse, sitting awkwardly next to R22's "no
  capability mouse-only" principle that governs everything else.

- **History holds an id and a title, never a conversation's content.** *Why:*
  the same bounded-memory ethos R2/T7 already establish for a single large
  document, applied to a Streamed-layout session that could otherwise
  accumulate dozens of conversations over a long run. `SelectConversation`
  therefore always re-asks the host rather than restoring from a cache — one
  mechanism (a host round-trip on selection), not two (a cache with its own
  invalidation rules). *Rejected:* caching each conversation's rendered
  layout model on eviction from view — bounded by nothing, and the operator's
  own words ("unless the past sessions are loaded by the host, it appears
  empty") already assume the host is the source of truth, not the control.

## 5. Risks & mitigations

- **Risk: `from_str()`'s missing-arm failure is silent and data-corrupting** —
  a saved Viewer control would deserialize back as `ControlType::Custom` on the
  next load, with no compile error and no existing test catching it (confirmed:
  `from_str()` is not exhaustive, unlike `as_str()`). → *Mitigation:* add an
  explicit round-trip test — save a Viewer control, reload, assert
  `ControlType::Viewer` survives, not `Custom` — as a **must-have**, not a
  nice-to-have; this is the single highest-severity trap this research
  surfaced, precisely because nothing forces it to be caught otherwise.

- **Risk: `render_interactive`'s wildcard match silently strips
  interactivity** — already bit Snackbar/WebSearch/IndexedFile once per the
  comment already sitting above that match in `render.rs`. → *Mitigation:* an
  explicit `ControlType::Viewer => { … }` arm, verified by an *interaction*
  test (click the Find toolbar button, assert the Find bar opens), not just a
  paint-diff test that would pass even against the generic fallback painter.

- **Risk: the Designer toolbox has no coverage test at all** — `TOOLS` is a
  plain static list; a forgotten entry compiles clean and Viewer simply never
  appears in the palette. → *Mitigation:* add a small `TOOLS.len() ==
  ControlType::ALL.len()`-shaped assertion (minus `Custom`) as part of this
  feature — closes the gap for every future control too, not just Viewer; small
  enough to be in-scope rather than scope creep, and it's the same class of gap
  this project already treats as debt worth repaying when found.

- **Risk: rounded-corner self-clipping is unverified** — DataGrid's exclusion
  from `self_clipping_type` was earned by measurement (header band too short
  for the lifted radius, border draws square at 96px), not by "it's large and
  paginated." Viewer's toolbar/thumbnail chrome is geometrically similar in
  kind. → *Mitigation:* budget for the exclusion, but let
  `a_child_at_a_rounded_corner_stays_inside_the_arc` **measure** it — the test
  fails loudly (`missing`/`stale` lists) if the code's classification disagrees
  with what's actually painted, so this is self-correcting, not a guess to get
  right on the first try.

- **Risk: PDF is the largest technical unknown** — `lopdf`'s read-side API has
  never been exercised in this codebase (only its write-adjacent siblings
  `printpdf`/`genpdf` have). → *Mitigation:* spec.md's own fidelity ordering
  already sequences PDF second, after text/Markdown/images prove the paging
  and rendering pipeline on easier ground; treat the first PDF milestone as a
  go/no-go spike — if `lopdf`'s text/position extraction proves inadequate for
  R-level fidelity, that's a finding to bring back before sinking further time
  into it, not a reason to reach for a C dependency.

- **Risk: HTML-subset layout is the single largest block of genuinely new
  code in this feature** (no precedent anywhere in the workspace). →
  *Mitigation:* sequenced last (spec.md fidelity order) and designed from the
  start to reuse the Markdown walker's layout primitives (§4), so its
  incremental cost is parsing + DOM-to-layout mapping, not a second engine.

- **Risk: the bounded decode cache is a new mechanism with no direct copy-
  paste precedent** — `indexed_disk.rs`'s bounded page/directory cache is the
  closest analog but caches raw B-tree pages, not decoded rendering output. →
  *Mitigation:* keep the budget explicit and configurable, and instrument it —
  the eventual test reports **measured peak RSS vs. document size** (this
  project's "quantify performance" standing rule, and directly what AC1 already
  demands), so a wrong budget shows up as a number, not a guess.

- **Risk: thread lifecycle** — an unbounded stream of opened/closed documents
  or forms must not leak background threads. → *Mitigation:* clean shutdown on
  disposal via dropping the jobs channel (the `DebugRunner::stop()` detach
  pattern); test it as a **measured completion signal**, never a sleep-and-hope
  check (this project's stated house style for timing-sensitive tests).

- **Risk: same-document split view accidentally double-decodes** — R21.1's
  "attach, don't reload" is a real invariant, not just a nice property. →
  *Mitigation:* the `Arc`-shared handle in §3 makes double-decoding structurally
  hard to reach by accident; test it directly with a decode-call counter,
  asserted `== 1` across both views.

- **Risk: `cobolt-media` reuse for animated formats is unconfirmed** — the
  research could not confirm whether `cobolt-forms` already depends on
  `cobolt-media` for the Animator control. → *Mitigation:* verify at
  implementation start, before deciding whether Viewer reuses `cobolt-media`'s
  GIF/WebP/APNG playback or needs its own minimal frame-timing logic on top of
  the extended `image` crate features.

- **Risk: ~16 events is a lot of near-identical "does this fire at exactly
  this moment" tests to write by hand, and hand-written near-duplicates are
  exactly where one gets silently skipped.** → *Mitigation:* one table-driven
  test iterating R32's event table (trigger → expected event name), rather
  than sixteen bespoke test functions — the table itself becomes the
  single place a missing case would have to hide, and AC30 is written to
  match ("verified per event, not by sampling a few").

- **Risk: scope, now larger.** 32 requirements across the static-document
  surface plus the full conversation-mode extension (§8, including §8.8) is a
  large feature, comparable to or larger than the spec-039 batch (6 controls).
  → *Mitigation:* `/tasks` sequences by the fidelity order spec.md already
  settled — a Viewer that opens text/Markdown/images and paints on both
  surfaces lands before PDF, before HTML, before Search, before split view,
  before conversation mode and Streamed layout. Each stage independently
  green, matching this project's own precedent for large single-control work.

## 6. Test strategy

**`cobolt-forms` (pure, no thread, no UI)**
- `viewer_layout_format_splitmode_round_trip` — `as_str`/`from_str` for all
  three new enums, reporting every value checked.
- `find_matches_reports_case_sensitivity_and_spans` — mirrors `code_search.rs`'s
  test shape; on/off case sensitivity, reports match counts for both.
- `two_view_geometry_matches_the_splitter_math` — reuses `splitter::geometry()`
  for `LeftRight`/`TopBottom`, reporting the produced rects.
- `every_control_type_has_an_icon` and `a_child_at_a_rounded_corner_stays_inside_the_arc`
  — must stay green with Viewer added; the corner test's own printed verdict is
  read, not assumed, per §5.
- `markdown_walker_produces_the_documented_node_shapes` — headings/lists/
  tables/images, reporting node counts per case.
- `mermaid_subset_renders_flowchart_and_sequence` — via `mermaid-rs-renderer` +
  `resvg`, reporting produced pixel dimensions per diagram.
- `every_listed_image_format_decodes` — PNG/JPEG/GIF/WebP/APNG/BMP/TIFF/SVG,
  reporting dimensions/frame counts per format (§3's Format table, verified,
  not assumed).
- `from_str_recovers_viewer_not_custom` (§5) — the round-trip regression guard.
- `streamed_layout_draws_a_single_pane_and_nothing_else` (§8.8, AC25) — asserts
  no toolbar/Find-bar/thumbnail/filmstrip shapes are emitted under `Layout =
  Streamed`, reporting the shape count against every other layout's for
  contrast.

**`cobolt-form-host` (the session, deterministic — a fabricated clock, never a sleep)**
- `viewer_session_spawns_exactly_one_thread_per_instance` — two sessions,
  reports two distinct thread ids/names.
- `decode_cache_stays_within_its_configured_budget` — open a document larger
  than the window; report **measured peak resident pages vs. the budget**
  (AC1's own quantified-performance demand).
- `same_document_two_views_decode_once` — report the decode-call count,
  asserted `== 1`.
- `session_thread_exits_on_disposal` — report a measured completion signal
  (channel closed / thread joined within a bound), not a sleep.
- `each_view_keeps_independent_search_state` (R21.2) — search one view, assert
  the other's `SearchState` is untouched; report both views' match counts.
- `history_never_exceeds_ten_and_evicts_the_oldest` (§8.8, AC28) — archive/
  register 12 entries, report the surviving 10 ids in order and confirm the
  first two are gone.
- `select_conversation_swaps_current_and_history_without_touching_content`
  (§8.8, AC27) — asserts the currently-open entry lands in history and the
  selected one is removed from it, and that no cached content is read back
  from anywhere (the mitigation for §4's "history holds an id and a title,
  never content" decision, made concrete).

**`cobolt-runtime`**
- `loadbytes_findnext_findprevious_saveas_print_share_dispatch` — each method
  reaches the host session; report which fired.
- `every_event_in_r32s_table_fires_at_its_documented_moment` (AC30) — **one
  table-driven test**, not sixteen bespoke ones (§5's mitigation): drives each
  trigger in turn and asserts the matching event and *only* that event fired,
  reporting a pass/fail per row so a gap is visible by name, not inferred from
  a missing test function.
- `new_conversation_and_select_conversation_dispatch` (§8.8) — `NewConversation()`
  on empty vs. non-empty panes (AC26's two branches, reported separately);
  `SelectConversation(id)` raises `onConversationSelected` carrying the right
  id and does **not** itself append any content (that's the host's job, via
  §8.2's methods, on receipt of the event).
- conversation-mode: `append_html_markdown_raw_preserve_arrival_order`,
  `render_as_html_false_forces_raw_regardless_of_append_mode` (§7's flagged
  reading of §8.1, tested explicitly so the interpretation is checked, not just
  asserted in prose), `oncontentrendered_fires_after_layout_not_after_accept`
  (§8.2's new item 6, AC29's "in sync" claim depends on this ordering being
  real).

**`cobolt-compiler`**
- `spec_058_viewer_is_fully_published_in_the_system_kb` — the exact template
  `spec_039_six_controls_are_fully_published_in_the_system_kb` already sets:
  `## Control: Viewer` present, representative property/event/method needles
  (`"SplitMode"`, `"onLoadProgress"`, `"FindNext()"`, …) present in both
  published docs.

**`cobolt-ide`**
- the new toolbox-coverage assertion from §5.
- `i18n_tests` covers all six languages automatically, as for every prior
  control.

**`tests/controls/viewer_test.rs`** — the standard per-control property
round-trip (35th file).

**Every test reports quantified results** (this project's standing rule):
counts, timings, measured RSS, thread ids — never a bare pass, matching how
Snackbar's own test strategy was written.

**Manual/visual** (operator only — never driven by the agent, per this
project's standing rule): open each format and confirm it matches §3's fidelity
table exactly, with no format silently over- or under-delivering; split view
with two different documents and with the same document twice, confirming
independent search per view; Find bar case-sensitivity/highlight-toggle/
wraparound; design-canvas vs. running-form parity (AC11); corner rendering
once `self_clipping_type` is set, on every surface; Streamed layout in a
sample chatbot form — confirm no chrome bleeds through, that a developer-built
sidebar (any controls, not a Viewer-supplied one) can drive `NewConversation`/
`SelectConversation` convincingly, and that the events in R32's table actually
reach a bound COBOL handler, not just the automated test harness.

## 7. Open note carried into this plan (not a spec change)

§8.1 of spec.md reads as if `render_as_html` and the three explicit append
modes (HTML/Markdown/Raw) are two separate things layered on top of each
other — the modes already say per-call how to treat content, so a *second*,
global "disable HTML interpretation" flag would be redundant unless it's meant
as a hard override. §3 above commits to that reading: `RenderAsHtml` (default
true) is a **safety override** — when explicitly set false, every append call
behaves as Raw regardless of which mode was actually invoked, useful as a
blanket guarantee for a COBOL program showing untrusted content. This is
written out plainly, the same way the thread-ownership reading was in
spec.md's own Decisions section, so a different reading is a one-line fix
rather than a re-derivation — flagging for your confirmation rather than
treating it as settled.

## 8. Steering compliance

- [ ] **i18n:** every new IDE string a `Tr` field in **all six** languages.
      COBOL property/method/event names stay **English in every language**
      (the CRITICAL constraint).
- [ ] **Generated code:** declaration + handler stubs emitted with the
      standard banner; regenerate-on-action contract untouched; generated
      COBOL stays English.
- [ ] **System KB:** all four doc tables updated **and**
      `assets/knowledge/chunked.data` regenerated in the same change; a red
      `prebuilt_chunked_kb_matches_the_published_documentation` is a real
      failure.
- [ ] **Docs:** English `developers-guide-en.md` gains a Viewer section; its
      five translations are **deleted** in the same change (Golden Rule #8) —
      not patched, not left stale. The two `docs_embed.rs` language-coverage
      guards going red afterward is the expected, correct signal.
- [ ] **Fix vs feature:** **FEATURE** — new control, beyond existing scope.
      `features` branch (already the branch this spec was clarified and
      committed on); its own commit(s), never sharing with a fix; forum **f=96**
      prefix `[Noticia]` only if the operator asks for an announcement.
- [ ] **Versioning:** `z` bump only, with a `CHANGELOG.md` entry — only the
      operator raises `x`/`y`.
- [ ] **Pure Rust (R25):** every new dependency audited against the two real
      C crates already in this workspace (`libsqlite3-sys`, `openssl-sys`) —
      none of `pulldown-cmark`/`mermaid-rs-renderer`/`lopdf`/`html5ever`/the
      extended `image` features touches either, and unlike those two, Viewer's
      deps have no "optional if unused" escape hatch (§4), so the bar is
      unconditional.
- [ ] No "cobolt" in user-facing text; COBOL identifiers stay English.
- [ ] **A window never resizes itself** — Viewer's own toolbar/Find-bar/
      filmstrip UI must never grow its host form; only the developer's own
      resize of the control's designed rect changes its size.
