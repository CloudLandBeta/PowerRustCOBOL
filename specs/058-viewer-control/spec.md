# Spec — Viewer Control

- **Status:** draft → **ready for `/plan`** (every question settled; no implementation yet)
- **Folder:** specs/058-viewer-control/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-08

## 1. Overview

A **Viewer** control for the Commons category: a designable control that
displays a document — text, Markdown, images, PDF and HTML — inside a form, with a toolbar, page navigation, zoom,
thumbnails, a filmstrip, print, OS share and Save As. It can show **two views
side by side** — two different documents, or the same document in both views so
one section can be reviewed while another is browsed. Every function is settable
and readable from COBOL,
and the reading and decoding work happens off the UI thread so a large document
never stalls the form.

It exists because a COBOL application that produces or receives documents
currently has nowhere to *show* them: the developer can write a PDF or a report
and must then hand it to an external program. A form-hosted viewer keeps the
document inside the application the developer built.

## 2. Goals / Non-goals

**Goals**

- One control that opens a document by path or by byte buffer and displays it.
- Documents far larger than RAM open promptly and scroll smoothly.
- The full chrome: toolbar, layouts, zoom, fullscreen, thumbnails, filmstrip,
  split view, font size, Print, Share, Save As.
- A complete COBOL API — every visual state has a property or method.
- Decoding and paging on a worker thread; the UI thread only paints.
- Follows the form theme by default, overridable from COBOL.

**Non-goals**

- **Editing.** The Viewer displays; it never writes back to the source document.
- **Format fidelity beyond what pure Rust provides** — see §3.1. This is the
  single most important boundary in this spec.
- Printing engine of our own: Print hands off to the OS print path.
- A browser. The HTML mode is a subset renderer, not an engine (§3).
- **Office documents** (`.docx` / `.xlsx` / `.pptx`) — dropped from scope by the
  operator, 2026-09-08. They were the largest of the pure-Rust fidelity gaps:
  layout-faithful rendering is a word processor, and no Rust crate does it.

## 2.1 Placement

Commons category (operator instruction, 2026-09-08), `ControlType::Viewer`.

## 3. Format support — what each format actually delivers

The operator has ruled **pure Rust — no C/C++ dependencies** (no PDFium, no
ffmpeg, no LibreOffice). That decision sets the ceiling for several formats, and
this table is the contract. Anything not listed here is out of scope.

| Format | Delivered | Not delivered |
|---|---|---|
| Plain text | Full, any size | — |
| Markdown (+ common extensions: tables, task lists, footnotes, strikethrough) | Full, any size | — |
| Mermaid | **Subset we implement**: flowchart and sequence diagrams | class/state/gantt/ER/journey; exact upstream layout |
| Images (PNG/JPEG/GIF/WebP/APNG/BMP/TIFF/SVG) | Full, incl. animation | — |
| PDF | **Text and basic vector**, page geometry, page breaks, search | Faithful raster of complex pages, embedded fonts with unusual encodings, forms, annotations, scanned-image-only pages beyond the embedded image |
| HTML + CSS | **Subset renderer**: block/inline layout, common typography, colours, borders, tables, images | CSS3 grid/flex/animation/transform, JavaScript, floats beyond the simple case. **Not a browser** |
| Video | **Out of scope.** Animated GIF/WebP/APNG are covered above as images | Any codec requiring a C decoder |
| Office (`.docx`/`.xlsx`/`.pptx`) | **Out of scope** (operator, 2026-09-08) | — |

**R0 (constraint, naming):** Modes whose fidelity is partial shall be **named
honestly** in the property vocabulary and the guide (for example `HtmlSubset`,
not `Html`), so a developer is never told the control does more than it does.

## 4. User stories

- As a COBOL developer, I want to show a generated PDF report inside my form, so
  the user never leaves my application.
- As a COBOL developer, I want to open a 2 GB log and jump to its end
  immediately, so I can inspect production output without a specialised tool.
- As a COBOL developer, I want to set every viewer option from COBOL, so the
  view can follow the data my program is showing.
- As a COBOL developer, I want two views side by side, so a user can compare a
  statement with its supporting detail — or read one section of a long document
  while browsing another section of the *same* document in the other view.
- As a user, I want to zoom, print, share and save the original file, so the
  viewer is as useful as the tools I already know.

## 5. Requirements (EARS)

### Loading and memory

- **R1 (ubiquitous):** The control shall open a document from a file path
  (`Source`) or from a byte buffer supplied by COBOL (`LoadBytes`).
- **R2 (constraint):** The control shall **not** require the whole document in
  memory. It shall build an index and hold a bounded window of decoded pages;
  peak memory shall be a function of the window, not of document size.
- **R3 (event):** When a document is opened, the control shall determine its
  format from content first and extension second, and shall report the resolved
  format in `Format`.
- **R4 (event):** When a document cannot be opened or its format is
  unsupported, the control shall raise `onError` with `LastError` set, and shall
  leave any previously loaded document displayed.
- **R5 (ubiquitous):** Decoding, indexing and page rendering shall run on a
  worker thread; the UI thread shall only paint already-prepared pages.
- **R5.1 (constraint):** Each Viewer instance shall own a **dedicated
  background thread** for its decoding, indexing and paging work — not a
  thread shared across other Viewer instances or unrelated subsystems — so a
  slow or large document in one Viewer cannot stall another Viewer's loading,
  or work belonging to any other control.
- **R6 (event):** When a document is opening, the control shall raise
  `onLoadProgress` with a 0–100 `Progress`, and `onLoaded` on completion.

### Display and layout

- **R7 (ubiquitous):** The control shall support five layouts in `Layout`:
  `Raw` (no formatting), `Web` (formatted, no page margins), `Print` (page
  margins top/bottom/left/right), `Page` (print layout plus a black-on-white
  document body), and `Streamed` (a single content pane with no toolbar, Find
  bar or thumbnail/filmstrip chrome — see §8.8). `Page` applies to any
  paginated content — Markdown and text included — not to a document class.
- **R8 (state):** While `Layout` is `Print` or `Page`, the control shall draw a
  paper border and a paper shadow around each page.
- **R9 (ubiquitous):** The control shall honour page breaks: explicit breaks in
  the source, and computed breaks in paginated layouts.
- **R10 (ubiquitous):** `FontSize` shall scale document text independently of
  `Zoom`.

### Navigation

- **R11 (event):** When the pointer wheel is used with the zoom modifier, the
  control shall zoom about the pointer position.
- **R12 (event):** When the user double-clicks, the control shall zoom in one
  step, up to a maximum of **16×**.
- **R13 (event):** When Esc is pressed, the control shall return `Zoom` to
  100 %; if fullscreen, it shall first leave fullscreen.

  *(R33–R33.3 sit here in reading order — numbered past R32 to avoid
  renumbering everything below, the same out-of-sequence pattern this spec
  already uses for Search and Events.)*

- **R33 (event):** Arrow Up/Down scroll the content, matching the IDE's own
  Documentation viewer exactly: a tap moves one line (`FontSize × 1.6`); a
  held key accelerates continuously from a base speed to a **4× ceiling over
  2 seconds**, with the acceleration only starting after a short hold delay
  (so a tap always reads as a tap, never a flicker of fast scroll). Never
  while another control — the Find input, for instance — holds keyboard
  focus.
- **R33.1:** Page Up/Down scroll by nearly a full viewport (the viewport
  height minus two lines, so context carries across the jump); Home/End jump
  straight to the top/bottom of the content.
- **R33.2 (event):** A left-button press-drag anywhere over the content pans
  it 1:1 with the pointer. Releasing while still moving **continues
  scrolling** at the speed measured from the drag's own last ~0.12 s of
  movement — never from wherever the pointer happens to be if it leaves the
  content first — decelerating under **constant friction** to a stop (not a
  fixed-duration ease: a fast throw travels further and takes longer than a
  gentle one, both ending at exactly zero). A new press while still gliding
  stops the glide at once — catching a moving page, the way every touch
  surface behaves.
- **R33.3:** R33/R33.1/R33.2 apply to `Full` mode (R14) only, independently
  per split view (R21) when `SplitMode != None` — `Cards` mode's own
  navigation is paging between cards, a different interaction, not a scrolled
  document.

- **R14 (ubiquitous):** Each view (R21 — the single view when `SplitMode =
  None`, or each of the two independently when split) has two mutually
  exclusive view modes, `ViewMode`: **`Full`** (the document itself, under
  whichever `Layout` is active) and **`Cards`** (a reflowing grid of
  page-thumbnail cards, one per page, **replacing** the document rather than
  sitting beside it). Both the number of columns and the number of rows the
  grid reflows to are a function of two things only — the current card size
  (R14.1) and **the control's own width** — never the window, the screen, or
  anything outside the control's own designed rect (a control's layout must
  not depend on something it cannot see). **Two toolbar buttons switch that
  view's mode explicitly** — in split mode, each view's buttons are its own,
  so one side can browse cards while the other reads the document. `ViewMode`
  does not apply while `Layout = Streamed` (§8.8), which has no chrome to
  switch at all.
- **R14.1:** **One slider per view** sits at the **bottom-right of that
  view, directly below its content area** — the same control, the same
  relative location, unified with zoom rather than duplicated (operator,
  2026-09-18: "unify the zoom control for the content, the card"). In `Full`
  mode it drives that view's `Zoom`; in `Cards` mode, that view's `CardSize`.
  Larger always means "more of the pane per item" — more of the page filling
  the view in `Full` mode, fewer, bigger cards per row in `Cards` mode.
  `CardSize` and `ViewMode` are therefore per-view properties
  (`View1CardSize`/`View2CardSize`, `View1ViewMode`/`View2ViewMode`),
  exactly like `Zoom` already is — one shared slider would otherwise have two
  views' independent modes fighting over it.
- **R14.2:** The slider's range adapts to whichever property it is currently
  driving — `Zoom`'s full range in `Full` mode (down to a legible minimum, up
  to R12's 16× cap), `CardSize`'s 0–100 % in `Cards` mode — and switching
  `ViewMode` never changes the *other* mode's own remembered value: leaving
  `Cards` mode and returning later shows the same card size as before, and
  likewise for `Zoom`. *(Retires the original rule that dragging the card
  slider to 100 % auto-exited to the document — superseded, since mode
  switching is now the two buttons' explicit job; see §9.)*
- **R14.3 (ubiquitous):** Independent of `ViewMode`, each view may also show a
  **filmstrip** — a resizable rail of page thumbnails **docked to the left
  edge of that view's content**, opened by its own toolbar button (in `Full`
  mode only; `Cards` mode is already a page browser, so a filmstrip beside it
  would duplicate its own job). `ShowFilmstrip` is therefore also a per-view
  property, `View1ShowFilmstrip`/`View2ShowFilmstrip`.
- **R14.4 (event):** The filmstrip closes exactly two ways, both leaving
  `Full` mode showing just the document: clicking its own toolbar button a
  second time (the button shows pressed while the filmstrip is open), or
  dragging the filmstrip's own right-hand splitter all the way to the view's
  left edge — the same gesture that resizes it, taken to its limit.
- **R15 (event):** When `Fullscreen` is entered, the toolbar shall be hidden;
  when left, it shall reappear.

### Toolbar and actions

- **R16 (ubiquitous):** The toolbar shall sit at the top of the control and
  offer: layout, view mode (`Full`/`Cards`, R14), font size, filmstrip,
  fullscreen, split, Find, Print, Share, Save As. **Zoom and card size are not
  toolbar items** — R14.1's single bottom-right slider is their only control,
  unified rather than duplicated in the toolbar too.
- **R17 (constraint):** Toolbar icons shall be **hand-drawn painter icons only**
  — no font glyphs, no bitmaps. Each shall carry its function name as a tooltip.
  Any icon the project lacks shall be drawn as part of this work.
- **R18 (event):** When Save As is chosen, the control shall write the
  **original bytes of the source, unmodified**, with the source's own extension.
  It shall **not** write a rendered or re-encoded document.
- **R18.1 (event):** When Save As is chosen for a document opened via
  `LoadBytes` (no source path to name it after), the control shall **propose a
  default filename**: the document's first three words of extracted text,
  joined, plus the extension matching the resolved `Format` (`.pdf`, `.txt`,
  `.md`, and so on). If the document has no extractable text (for example, an
  image), the control shall fall back to a generic base name plus the correct
  extension. The user may edit the proposed name in the save dialog; if the
  edited name is missing its extension, the control shall append the correct
  one when writing the file regardless of what the user typed.
- **R19 (event):** When Share is chosen, the control shall hand the document to
  the operating system's share facility — `NSSharingService` on macOS, the
  Windows share contract, or the Linux desktop's share portal / `xdg-open` —
  rather than implement sharing itself.
- **R20 (event):** When Print is chosen, the control shall hand the document to
  the operating system's native print path (its print dialog and spooler)
  rather than implement printing itself.

### Search

*(Numbered R26–R31, past Boundaries' R25, to avoid renumbering Split view /
Programmatic control / Boundaries — the same out-of-sequence pattern AC19
already uses in §6. Placed here, next to Toolbar and actions, since Find is a
toolbar-adjacent feature.)*

- **R26 (ubiquitous):** The control shall provide in-document **Find**: a
  toolbar button and the `Ctrl+F` / `Cmd+F` shortcut open a Find bar; `Esc`
  closes it and takes priority over R13's Zoom/fullscreen Esc behaviour while
  the bar is open.
- **R26.1 (constraint):** Find shall operate on any format with extractable
  text — plain text, Markdown, the HTML subset, and PDF's text layer. A format
  with no extractable text (for example, a standalone image) has no matches;
  the Find bar reports zero results rather than raising an error.
- **R27 (ubiquitous):** The Find bar shall offer a **case-sensitivity toggle**
  (`SearchCaseSensitive`), off (case-insensitive) by default.
- **R28 (event):** **Next**/**Previous** controls — and `F3`/`Shift+F3`, or
  `Enter`/`Shift+Enter` while the Find bar has focus — shall move to the next
  or previous match, wrapping past the last/first match, and shall scroll the
  current match into view.
- **R29 (state):** While Find has one or more matches, the control shall
  **highlight every match**, with the current match visually distinguished from
  the rest, unless highlighting is turned off (`SearchHighlightEnabled`); Find
  still runs and Next/Previous still navigate with highlighting off.
- **R30 (ubiquitous):** The Find bar shall show a live **match counter**
  ("current of total"), updating as the user types and as Next/Previous are
  used.
- **R31 (ubiquitous, ties to R22):** Every Find property and action — search
  text, case sensitivity, highlight toggle, current/total match count, Next,
  Previous, open/close — shall be readable and writable from COBOL, with no
  Find capability reachable only by mouse.

### Events

- **R32 (ubiquitous):** Every user-driven state change, and every
  asynchronous, OS-handoff action (Print, Share, Save As), shall raise a
  matching event, so a COBOL program is never blind to an interaction it did
  not itself trigger through a property or method call. This extends R22's
  "no capability mouse-only" principle from *control* to *observability*.
  Naming follows this project's existing async-lifecycle convention (the
  `onComplete`/`onCancelled` pair already used by `RestClient`/`SqlDatabase`/
  `IndexedFile`/`Maps`/`WebSearch` — no "Finished"/"Done"/"Success" wording),
  applied per action via a prefix wherever a control has more than one
  asynchronous action to disambiguate.

  | Event | Fires when | Ties to |
  |---|---|---|
  | `onError` | a document fails to open, or its format is unsupported | R4 |
  | `onLoadProgress` | while a document is opening, with 0–100 `Progress` | R6 |
  | `onLoaded` | a document finishes opening | R6 |
  | `onLayoutChanged` | `Layout` changes (`Raw`/`Web`/`Print`/`Page`/`Streamed`) | R7, §8.8 |
  | `onZoomChanged` | `Zoom` settles after a wheel, double-click, the R14.1 slider, or a programmatic change | R11–R13, R14.1 |
  | `onCardSizeChanged` | `CardSize` settles after the R14.1 slider, or a programmatic change | R14.1 |
  | `onScrolled` | the content's scroll position comes to rest — after a key, a throw's glide, or a programmatic change, never mid-glide | R33 |
  | `onViewModeChanged` | a view's `ViewMode` changes between `Full` and `Cards` | R14 |
  | `onFilmstripToggled` | a view's `ShowFilmstrip` turns on or off | R14.3 |
  | `onFullscreenEntered` / `onFullscreenExited` | `Fullscreen` is entered / left | R15 |
  | `onFindOpened` / `onFindClosed` | the Find bar opens / closes | R26 |
  | `onPrintComplete` / `onPrintCancelled` | the OS print handoff finishes / the user cancels it | R20 |
  | `onShareComplete` / `onShareCancelled` | the OS share handoff finishes / the user cancels it | R19 |
  | `onSaveComplete` / `onSaveCancelled` | Save As finishes writing / the user cancels the dialog | R18, R18.1 |
  | `onSplitModeChanged` | `SplitMode` changes | R21 |
  | `onConversationCreated` | `NewConversation()` is called | §8.8 |
  | `onConversationSelected` | `SelectConversation(id)` is called, carrying `id` | §8.8 |
  | `onContentRendered` | newly appended content (§8.2) finishes laying out — not merely after the data is accepted | §8.2, §8.8 |

### Split view

- **R21 (optional):** Where `SplitMode` is `LeftRight` or `TopBottom`, the
  control shall display two views at once, each with its own source, page,
  zoom, scroll position and search state; `SplitMode = None` shows one.
- **R21.1:** The two views may hold **two different documents** *or* the **same
  document shown twice**. When both views point at the same document, each keeps
  its own page/zoom/scroll independently, so the user can review one section in
  one view while browsing another section of the same document in the other.
  Setting a view's source to the document already open in the other view must
  not reload or re-decode it — the second view attaches to the same underlying
  document, only its own viewport state differs.
- **R21.2:** Each view's Find (§5 "Search") is fully independent: its own
  search text, case-sensitivity and highlight toggles, current match and match
  count, and open/closed state. Searching in one view — including the
  same-document case of R21.1 — shall never change, clear or re-scope the
  other view's search, so the user can search each side freely and
  simultaneously.

### Programmatic control

- **R22 (ubiquitous):** Every function above shall be readable and writable from
  COBOL — properties for state, methods for actions — with no viewer capability
  reachable only by mouse.
- **R23 (ubiquitous):** The control shall follow the form theme by default, and
  shall accept a programmatic override of its own colours.

### Boundaries

- **R24 (constraint):** The control shall **not** modify the source document.
- **R25 (constraint):** The implementation shall use **no C or C++
  dependencies** (operator ruling, 2026-09-08).

## 6. Acceptance criteria

- [ ] **AC1** — A text file larger than available RAM opens, and jumping to the
      last page is prompt; measured peak RSS stays within the configured window
      rather than scaling with file size. *(Quantified per GOLDEN RULE #7.)*
- [ ] **AC2** — Each supported format in §3 opens and displays, and each
      partial-fidelity mode is named as a subset in its property value. An
      Office file is refused through `onError`, not half-rendered.
- [ ] **AC3** — `Layout = Print` shows page margins, a paper border and a paper
      shadow; `Web` shows none of them; `Raw` shows unformatted source.
- [ ] **AC4** — Double-click zooms in and stops at 16×; Esc returns to 100 %;
      wheel zoom keeps the point under the pointer fixed.
- [ ] **AC5** — Entering fullscreen hides the toolbar; leaving restores it.
- [ ] **AC6** — Save As produces a file **byte-identical** to the source
      (asserted by comparing bytes, not by opening the result).
- [ ] **AC7** — Every property and method in the COBOL API round-trips: set from
      COBOL, read back, and the displayed state matches.
- [ ] **AC8** — Split view shows two views with independent page, zoom, scroll
      and search — whether the two hold different documents or the **same**
      document; in the same-document case the document is decoded once, and
      moving or searching in one view does not move or affect the other.
- [ ] **AC9** — Opening a large document does not block the UI thread: the form
      keeps painting and answering input while loading.
- [ ] **AC10** — Every toolbar icon is painter-drawn and carries a tooltip.
- [ ] **AC11** — The control paints identically on the designer canvas and the
      running form (spec 017 parity).
- [ ] **AC19** — Switching a view's `ViewMode` to `Cards` replaces its content
      with a reflowing card grid (one card per page) whose row and column
      counts both track that view's own width and card size — resizing the
      *control*, not the window, changes the count; switching back to `Full`
      shows the document, with each mode's own slider value (`Zoom`,
      `CardSize`) remembered independently across the switch. A view's
      filmstrip opens docked to its content's left edge from its own toolbar
      button, and closes exactly two ways — that button again, or dragging
      its splitter to the view's left edge — without leaving `Full` mode.
      *(AC12–AC18 are the conversation-mode criteria in §8.7.)*
- [ ] **AC31** — Holding Arrow Down scrolls at a steady base speed at first,
      measurably faster by 2 s in, capped at 4× — reported as the measured
      speed at three points (tap, ~1 s held, ~2 s+ held); Page Up/Down and
      Home/End move by the documented amounts; none of it fires while the
      Find input has focus.
- [ ] **AC32** — Dragging the content pans it 1:1 with the pointer; releasing
      while moving continues scrolling and comes to rest under constant
      friction, always at exactly the scroll limit or zero velocity, never a
      fixed-duration animation; a drag that stopped moving before release
      throws nothing; pressing during a glide stops it immediately.
- [ ] **AC20** — Save As on a `LoadBytes` document with no source path defaults
      the filename to its first three words plus the extension matching its
      `Format`; an image or other textless document falls back to a generic
      name plus the correct extension; the user can edit the name, and the
      correct extension is restored at save time even if the user deletes it.
- [ ] **AC21** — Two Viewer instances open large documents at the same time; a
      slow decode in one does not delay the other's `onLoadProgress`,
      `onLoaded`, or UI responsiveness — each runs its decode/index/paging work
      on its own dedicated thread, not a shared pool.
- [ ] **AC22** — Typing in the Find bar highlights every match in the document
      (unless highlighting is off) and shows a "current of total" count that
      updates live; Next/Previous move between matches, wrapping at the ends,
      and bring the current match into view.
- [ ] **AC23** — The case-sensitivity toggle changes which matches are found
      (e.g. "COBOL" vs. "cobol") without retyping the search text; the
      highlight-enabled toggle turns all highlighting on/off without breaking
      Next/Previous navigation or the match count.
- [ ] **AC24** — Search text, case sensitivity, highlight toggle, and match
      count/index all round-trip through the COBOL API (set/read), and Find,
      Next and Previous are all COBOL-callable.
- [ ] **AC30** — Every event in R32's table fires at its documented moment and
      never at another one — verified per event, not by sampling a few.
      *(AC25–AC29 are the Streamed-layout/conversation-management criteria in
      §8.7.)*

## 7. Constraints & steering check

- **i18n (6 languages):** yes — every toolbar tooltip, layout name and error
  string is a `Tr` field in EN/ES/PT/JA/ZH/FR. No hard-coded literals.
- **Generated-code contract:** yes — a Viewer on a form generates its
  property/method facade; generated COBOL stays English and is regenerated on
  Build/Run/Debug/Check.
- **System KB:** yes — the `cobolt-compiler` property/method/event tables gain
  the Viewer, and `assets/knowledge/chunked.data` is regenerated **in the same
  change**.
- **Docs:** yes — `docs/developers-guide-en.md` gains a Viewer section,
  including the §3 fidelity table verbatim. Translations untouched.
- **Fix vs feature:** **feature** — a new control, beyond existing scope.
  `features` branch; `z` bump; announced on f=96 only when the operator asks.
- **Control catalogue:** the catalogue grows from 42 to 43 types; per-control
  round-trip test added under `tests/controls/`.
- **Pure Rust:** R25. Crate choices are `/plan`'s business, but any candidate
  pulling a C toolchain is disqualified at that stage.
- **Delivery order** (operator ruling, 2026-09-18): built in fidelity order,
  inside the one delivery — text/Markdown/images → PDF → Mermaid subset → HTML
  subset — each landing on a proven frame before the next begins. Not
  parallelized across formats; `/plan` and `/tasks` sequence accordingly.

## 8. Behavioral spec — dynamic chatbot conversation (incremental append & streaming)

This section specifies the Viewer's behaviour when it hosts a **dynamic chatbot
conversation** in which messages and streamed content are appended incrementally,
without rebuilding the whole document. It complements the static-document
behaviour of §1–§7: the same control, driven as an append-only, self-following
conversation surface.

### 8.1 Content model

The viewer's internal document format is **HTML**. It supports three input modes:

- **HTML** — append the supplied content and render it as HTML.
- **Markdown** — convert the Markdown to HTML *before* appending it. Markdown is
  never inserted directly into the viewer.
- **Raw** — append the supplied content as literal text: HTML tags and entities
  are escaped so they are displayed rather than interpreted.

Raw content therefore remains part of the viewer's HTML document, but it is
inserted as an escaped text node or inside an appropriate element such as
`<pre>`.

For this particular use case, `render_as_html` is **disabled** for newly arriving
content. Consequently every incoming chunk is appended in **raw** mode and
displayed exactly as received. The viewer must not automatically reinterpret the
accumulated raw content as HTML unless explicitly requested by the caller.

### 8.2 Appending content

The control must provide operations equivalent to:

- `append_html(content)`
- `append_markdown(content)`
- `append_raw(content)`
- `append_to_message(message_id, content, mode)`

A new item may create a conversation message, while streamed chunks may extend an
existing message identified by a stable ID. Content must always appear in
**arrival order**. Appending content must:

1. Preserve all existing conversation content.
2. Preserve the current text selection and keyboard focus.
3. Update only the affected message whenever possible.
4. Recalculate the layout before applying any scrolling behaviour.
5. Avoid rebuilding or reparsing the entire conversation.
6. Raise `onContentRendered` once the newly appended content's layout is
   complete — not merely once the data is accepted (§5's events table, R32).

### 8.3 Automatic scrolling (auto-follow)

Before appending content, the viewer must determine whether the viewport is
already at, or sufficiently close to, the end of the conversation. A small
threshold — **24–32 px** — should be used so that minor rounding or layout
differences do not incorrectly disable automatic scrolling.

- If the viewport was at the end before the update, the viewer must remain
  **pinned to the new end** after the content is appended: the existing content
  moves upward and the newly arrived content becomes visible.
- If the user has scrolled upward to read previous messages, the viewport must
  **not** move when new content arrives.
- Automatic following **resumes** as soon as the user manually returns to the end.
- The decision must be based on the scroll position **before** the new content
  changes the document height.

During streamed responses, the viewport must continue following each incoming
chunk only while the auto-follow state remains active. If images, fonts or other
asynchronously loaded elements later change the document height, the viewer must
remain pinned to the end when auto-follow is active; otherwise it must preserve
the user's current reading position.

### 8.4 New-content indication

When new content arrives while the user is not at the end, show a non-intrusive
indicator such as "New messages" or "Jump to latest". Activating it must:

1. Scroll to the end of the conversation.
2. Clear the pending-content indicator.
3. Re-enable automatic following.

### 8.5 Rendering and safety

- Rendered HTML must be **sanitised** according to the application's security
  policy. Scripts, inline event handlers, unsafe URLs and other executable
  content must not run unless explicitly supported and trusted.
- Raw mode must always escape characters such as `&`, `<` and `>` before
  inserting the content into the HTML document. Newlines and whitespace should be
  preserved.
- The rendering mode must be **specified for each append operation** rather than
  inferred from the content. Content received in raw mode must remain raw even if
  it contains valid HTML markup.

### 8.6 Performance

The control must support long conversations and frequent streamed updates. It
should:

- Batch rapid incoming chunks when appropriate.
- Avoid a complete document rebuild after every append.
- Minimise layout recalculations and repainting.
- Preserve stable message identifiers.
- Support pruning or virtualisation if conversation size becomes excessive.
- Keep scrolling smooth while content is streaming.

### 8.7 Acceptance criteria (conversation mode)

These extend §6 for the append/streaming use case.

- [ ] **AC12** — HTML content can be appended and rendered correctly; Markdown is
      converted to HTML before insertion.
- [ ] **AC13** — Raw content is displayed literally and never interpreted as
      markup, even when it contains valid HTML; `&`, `<`, `>` are escaped and
      whitespace preserved.
- [ ] **AC14** — Newly arriving content uses `render_as_html = false` and is
      never reinterpreted as HTML unless the caller explicitly asks.
- [ ] **AC15** — The viewport follows new content only when it was already showing
      the end; it stays stable when the user is reading older content; returning
      to the end re-enables automatic following. The at-end decision uses the
      pre-append scroll position and the 24–32 px threshold.
- [ ] **AC16** — Streamed chunks extend an existing chatbot message
      (`append_to_message`), and content stays in correct arrival order.
- [ ] **AC17** — Appending content does not clear the conversation, move keyboard
      focus, or lose the current text selection, and does not rebuild/reparse the
      whole conversation.
- [ ] **AC18** — Late layout changes (images or fonts loading) keep the viewport
      pinned to the end when auto-follow is active and otherwise preserve the
      user's reading position.
- [ ] **AC25** — Streamed layout shows exactly one content pane with no
      toolbar, Find bar, thumbnail or filmstrip chrome, regardless of what
      chrome the previous `Layout` had shown.
- [ ] **AC26** — `NewConversation()` archives a non-empty pane into history,
      clears it, and raises `onConversationCreated`; calling it on an empty
      pane raises no event and creates no history entry.
- [ ] **AC27** — `SelectConversation(id)` archives the currently-open
      conversation, removes the selected id from history, clears the pane, and
      raises `onConversationSelected(id)` — the control never repaints content
      from anywhere but a subsequent host-supplied append call.
- [ ] **AC28** — History never exceeds 10 entries; archiving or registering an
      11th evicts the oldest, verified by id.
- [ ] **AC29** — `HistoryList` lists every current entry as `id|title`, one per
      line, staying in sync after every archive, selection and eviction.

### 8.8 Streamed layout and conversation management

The Viewer's fifth `Layout` value, **Streamed**, is built for hosting a
chatbot conversation with nothing else on screen: a single content pane, no
toolbar, no Find bar, no thumbnail or filmstrip chrome. Starting, browsing and
managing conversations is not drawn by the control at all — every affordance
for it (a "new chat" action, a history list, search, save, share) is the
developer's own UI, built from whatever controls fit the application, wired to
the methods and events below. *(An earlier draft of this section built a fixed
sidebar into the control, reusing `ControlType::SideMenu`; the operator ruled
this out — the developer must not be forced into a UI they cannot customize,
and separately, SideMenu turned out to have no hamburger/trigger concept at
all and is not architecturally reusable by a second control regardless.)*

- **`NewConversation()`** — a method. If the content pane holds a non-empty
  conversation, it is archived into history first (below); the pane is then
  cleared; `onConversationCreated` is raised. Calling it on an already-empty
  pane does not create a spurious history entry.
- **History** holds **up to 10** entries, each carrying only an id and a
  title — never a conversation's rendered content. Past 10, the oldest entry
  is evicted. History starts every run holding only what that run itself
  archives; `RegisterConversation(id, title)` lets the host seed an entry left
  over from an earlier run (its content, like every other entry's, is fetched
  on selection, never held by the control).
- **`SelectConversation(id)`** — a method. The currently open conversation (if
  any) is archived into history exactly as `NewConversation()` does; the entry
  named by `id` is removed from history and becomes current; the pane is
  cleared; `onConversationSelected` is raised carrying `id`, telling the host
  to supply that conversation's content via the append methods of §8.2. The
  control never restores content from a cache of its own — every selection is
  a fresh request to the host, which is what keeps memory bounded no matter
  how long a session runs (ties to R2).
- **`HistoryList`** — a read-only property, one entry per line as `id|title`
  (the same multi-line-list convention `Buttons` already uses on Snackbar), so
  the developer's own UI can enumerate, sort or search history without a
  dedicated search method — nothing here that the developer's own COBOL cannot
  already do against a dozen lines of text.

The three events this section introduces — `onConversationCreated`,
`onConversationSelected`, `onContentRendered` — are listed with every other
event in R32's table (§5); nothing about them is unique to Streamed layout
except that they are the ones most likely to fire while it is active.

## 9. Decisions already taken

Four questions the operator has now answered directly (2026-09-18), plus five
instructions given alongside them. Recorded here with the reasoning folded into
the requirements above, so the resolution is traceable back to its source.

- **Q1 — PDF stays pure-Rust** (operator, 2026-09-18). No PDFium. The table in
  §3 and R25 were already written this way; this confirms the fidelity gap is
  accepted rather than closed with a C dependency, even now that Office is out
  of scope. Nothing in §3 changes.
- **Q2 — Fidelity order, one delivery** (operator, 2026-09-18): text/Markdown/
  images → PDF → Mermaid subset → HTML subset, each landing on a proven frame
  before the next starts. Recorded in §7 as a delivery-order constraint for
  `/plan`/`/tasks`.
- **Q3 — Share and Print shell out to the OS** (operator, 2026-09-18): yes.
  R19/R20 now name the mechanisms directly (`NSSharingService` on macOS, the
  Windows share contract, the Linux share portal / `xdg-open`).
- **Q4 — Save As default filename for a `LoadBytes` document** (operator,
  2026-09-18): the document's first three words plus the extension matching its
  format, editable by the user, with the correct extension restored at save
  time even if the user deletes it. New requirement R18.1, new AC20.
- **The Viewer runs on its own thread** (operator instruction, 2026-09-18,
  given alongside the four answers above). Read as: each Viewer instance owns a
  **dedicated** background thread for decode/index/paging work — not a thread
  pool shared across other Viewer instances or other controls — so one
  document cannot stall another's loading. This sharpens R5 rather than
  replacing it: painting itself still happens only on the UI thread (R5
  unchanged). New requirement R5.1, new AC21. *(This is the author's reading of
  a terse instruction, written out so a different reading is a one-line fix
  rather than a re-derivation.)*
- **In-document Find** (operator, 2026-09-18): case sensitivity, Previous/Next
  navigation, and an enable/disable highlight-results toggle, confirmed as
  proposed. The single word "search" in §3's PDF row had no requirement behind
  it; this gives it one. New "Search" subsection, requirements R26–R31 (§5),
  a new toolbar entry in R16, and new AC22–AC24 (§6).
- **Streamed layout for chatbot use, with a pure COBOL API — no built-in
  sidebar** (operator, 2026-09-18, in two parts). First asked for a
  distraction-free single-pane layout with a hamburger-triggered sidebar
  reusing `ControlType::SideMenu` (New chat / Search / Save / Share / up to 10
  past conversations); investigation found SideMenu has **no** hamburger or
  trigger concept at all (deliberately rejected in its own code) and is not
  architecturally reusable by a second control regardless. Before that
  research even finished, the operator corrected course: **no built-in
  sidebar at all** — "the developer will decide how to implement it rather
  [than] be forced to use one he cannot customize." The control now exposes
  the capability as a pure API (`NewConversation()`, `SelectConversation(id)`,
  `RegisterConversation(id, title)`, `HistoryList`) and draws nothing for it.
  Also folded in: selecting a past conversation never restores cached content
  — it always re-requests it from the host by event, and the previously-open
  conversation swaps into history while the selected one becomes current.
  New §8.8, new events in R32's table, new AC25–AC29 (§8.7).
- **Events for every interaction, not just Print/Share** (operator,
  2026-09-18): "provide events for every possible interaction," given Print/
  Share-cancelled as examples. Read broadly — extended to every state change
  (layout, zoom, fullscreen, split mode, thumbnails/filmstrip, Find open/
  closed) alongside the OS-handoff actions (Print/Share/Save), reusing this
  project's existing `onComplete`/`onCancelled` async-lifecycle naming
  (spec 032), prefixed per action. New "Events" subsection, R32 (§5).
- **Split view: search is per-view** (operator, 2026-09-18): each view's Find
  is fully independent — its own search text, toggles, current match and
  count — including when both views hold the **same** document (R21.1), so
  searching one side never disturbs the other. R21 now lists search state
  among the per-view independent state; new R21.2; AC8 extended.
- **Content scrolling matches the Documentation viewer exactly** (operator,
  2026-09-18): arrow keys and mouse grab-and-throw, "just like the
  documentation viewer." Investigated rather than assumed —
  `crates/cobolt-ide/src/panels/doc_viewer.rs`'s own key-accel and throw/
  friction formulas (base speed ramping to a 4× ceiling over 2 s; release
  speed measured from the drag's own last ~0.12 s, decelerating under
  constant friction, never a fixed-duration ease) are real, precisely
  measured, and **not reusable code** (private to that module, and
  `cobolt-ide` is a binary crate `cobolt-forms` cannot call regardless) — so
  this reproduces the same mechanics, not a shared implementation. New
  R33–R33.3, new AC31–AC32.
- **Zoom and card size unify into one bottom-right slider; thumbnails become
  an explicit `ViewMode`** (operator, 2026-09-18): "unify the zoom control for
  the content, the card... a slider on the bottom right... right below the
  content." Read together with "the card viewer is a view mode (content is
  the other mode) activated by buttons" as a single redesign: `ShowThumbnails`
  (a togglable chrome) is replaced by `ViewMode` (`Content`/`Card`, switched
  by two toolbar buttons), and R14.1's slider now drives `Zoom` in `Content`
  mode and `CardSize` in `Card` mode — one control, one location, instead of
  a toolbar zoom group **and** a separate card-size slider. This retires the
  original "slider at 100 % auto-exits card mode" rule (R14.2) — mode
  switching is now the buttons' explicit job, so an implicit slider-triggered
  exit would just be a second, competing way to do the same thing. Landed
  before Stage C touched Navigation or the toolbar, so no implementation
  needed reworking — only the design. R14/R14.1/R14.2 rewritten, R16's toolbar
  list updated, AC19 rewritten.
- **Three refinements to that redesign** (operator, 2026-09-18): (1)
  `ViewMode`'s two values are named **`Full`** and **`Cards`**, not
  `Content`/`Card` — renamed throughout. (2) The card grid's row/column count
  depends on **the control's own width**, never the window or screen — this
  corrects a stray "(i.e. the screen resolution)" gloss that had sat in §3's
  original `/specify` output since before this feature had a plan, unnoticed
  until now; a control's layout must not depend on something it cannot see.
  (3) Filmstrip is **left-docked to its view's content** (R14.3's own
  "collapses to its border (leftward)" wording already implied this — the
  published mockups had drawn it as a bottom strip, which contradicted the
  spec they were illustrating), closes by its own toolbar button or by
  dragging its splitter to the view's left edge (new R14.4), and — since a
  control-wide filmstrip cannot mean anything once two split views can hold
  two different documents — is a **per-view** property,
  `View1ShowFilmstrip`/`View2ShowFilmstrip`, matching everything else split
  view already made independent. R14/R14.1/R14.3 amended, new R14.4, R16/AC19/
  the events table updated; `model.rs`'s already-seeded defaults corrected to
  match (§2's silent-drift risk, caught here rather than later).

## 10. Open questions

*(None. The spec is ready for `/plan`.)*
