# Spec — Viewer Control

- **Status:** draft → **awaiting review** (open questions in §9; no implementation yet)
- **Folder:** specs/058-viewer-control/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-08

## 1. Overview

A **Viewer** control for the Commons category: a designable control that
displays a document — text, Markdown, images, PDF and HTML — inside a form, with a toolbar, page navigation, zoom,
thumbnails, a filmstrip, print, OS share and Save As. It can show **two
documents side by side**. Every function is settable and readable from COBOL,
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
- As a COBOL developer, I want two documents side by side, so a user can compare
  a statement with its supporting detail.
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
- **R6 (event):** When a document is opening, the control shall raise
  `onLoadProgress` with a 0–100 `Progress`, and `onLoaded` on completion.

### Display and layout

- **R7 (ubiquitous):** The control shall support four layouts in `Layout`:
  `Raw` (no formatting), `Web` (formatted, no page margins), `Print` (page
  margins top/bottom/left/right), and `Page` (print layout plus a black-on-white
  document body). `Page` applies to any paginated content — Markdown and text
  included — not to a document class.
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
- **R14 (optional):** Where `ShowThumbnails` is on, the control shall show a
  page thumbnail pane; where `ShowFilmstrip` is on, a filmstrip whose size the
  user can drag.
- **R15 (event):** When `Fullscreen` is entered, the toolbar shall be hidden;
  when left, it shall reappear.

### Toolbar and actions

- **R16 (ubiquitous):** The toolbar shall sit at the top of the control and
  offer: layout, zoom, font size, thumbnails, filmstrip, fullscreen, split,
  Print, Share, Save As.
- **R17 (constraint):** Toolbar icons shall be **hand-drawn painter icons only**
  — no font glyphs, no bitmaps. Each shall carry its function name as a tooltip.
  Any icon the project lacks shall be drawn as part of this work.
- **R18 (event):** When Save As is chosen, the control shall write the
  **original bytes of the source, unmodified**, with the source's own extension.
  It shall **not** write a rendered or re-encoded document.
- **R19 (event):** When Share is chosen, the control shall hand the document to
  the operating system's share facility.
- **R20 (event):** When Print is chosen, the control shall hand the document to
  the operating system's print path.

### Split view

- **R21 (optional):** Where `SplitMode` is `LeftRight` or `TopBottom`, the
  control shall display two documents at once, each with its own source, page
  and zoom; `SplitMode = None` shows one.

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
- [ ] **AC8** — Split view shows two documents with independent page and zoom.
- [ ] **AC9** — Opening a large document does not block the UI thread: the form
      keeps painting and answering input while loading.
- [ ] **AC10** — Every toolbar icon is painter-drawn and carries a tooltip.
- [ ] **AC11** — The control paints identically on the designer canvas and the
      running form (spec 017 parity).

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

## 8. Open questions

- **Q1 — What remains of the scope/purity conflict, now that Office is out.**
  Dropping Office removes the largest gap. What still falls short of "everything"
  under R25 is narrow and worth confirming: **video is dropped entirely** (no
  pure-Rust decoder worth shipping), **HTML is a subset renderer** rather than a
  browser, and **PDF is text plus basic vector** rather than a faithful raster of
  complex pages. If PDF fidelity matters more than purity, PDFium is the single
  dependency that would lift it — that is the one lever left worth pulling.

- **Q2 — Ordering.** §3 is still a broad surface for one pass, though smaller without Office. Recommend implementing
  in fidelity order (text/Markdown/images → PDF → Mermaid subset → HTML subset) so each lands on a proven frame. Does the operator want it
  sequenced that way inside the one delivery, or strictly all-at-once?
- **Q3 — Share and Print** are OS facilities with no pure-Rust cross-platform
  crate. Acceptable to shell out to the platform's own mechanism (macOS
  `NSSharingService` / Windows share contract / `xdg-open`) rather than
  implement them?
- **Q4 — `LoadBytes`** implies the viewer may hold a document COBOL built in
  memory. Does Save As for such a document write those bytes (yes, by R18) even
  though there is no source file to name — i.e. does `SaveAs` require an
  explicit target name in that case?
