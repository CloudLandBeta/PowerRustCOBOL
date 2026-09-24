# Spec — Document import for the Application Knowledge Base

- **Status:** draft → awaiting operator review
- **Folder:** specs/074-document-import/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-24
- **Parents:** `specs/063-rag-chatbot-boilerplate/spec.md` (§7 Q3),
  `specs/068-application-knowledge-base/spec.md` (R14 — the "document in, text
  out" seam this spec fills), `specs/071-powerchat/spec.md` (§4.3 R17, §7 Q3).

## 1. Overview

Spec 068's `KnowledgeBase` reads Markdown and plain text, and takes every other
format through one seam: a document goes in, text comes out, and the KB chunks
and indexes that text exactly as it would a Markdown file. This spec fills the
seam, so that the documents users actually keep — Word, PowerPoint and Excel
files, OpenDocument files, PDFs, tables, saved web pages, and archives holding
any of these — become searchable.

The conversion runs inside the built application, in the runtime, and needs no
C compiler to build.

## 2. Goals / Non-goals

**Goals**

- A user drops their existing documents into a collection and they are
  searchable, with no conversion step of their own.
- Structure survives where the format has it — headings, slides, sheets,
  pages — so 068 R15 can chunk by subject rather than by size.
- Every document that cannot be read is named, with the reason.
- The default build of an application stays free of a C compiler.

**Non-goals**

- Reading text out of images: no OCR, so a scanned PDF has nothing to index.
- The old binary Office formats (`.doc`, `.xls`, `.ppt`).
- Opening password-protected documents.
- Converting documents back, or editing them.
- Showing the converted text to the user. The Viewer already displays the
  original formats; this spec only feeds the index.

## 3. User stories

- As an **end user**, I put the company's Word policies, PowerPoint decks and
  Excel price lists into a topic, and the assistant answers from them.
- As an **end user**, I add a PDF manual, and the assistant quotes the page it
  found the answer on.
- As an **end user**, I drop in a ZIP of last year's documents, and each file
  inside is searchable and cited by its own name.
- As an **end user**, when a document is skipped, I am told which one and why,
  so I can convert or replace it.
- As a **developer**, I get all of this by using the `KnowledgeBase` control,
  with nothing to configure per format.

## 4. Requirements (EARS)

### 4.1 Formats

- **R1 (ubiquitous):** The KB shall accept Word documents (`.docx`, `.docm`,
  `.dotx`, `.dotm`), PowerPoint presentations (`.pptx`, `.pptm`, `.potx`,
  `.potm`, `.ppsx`, `.ppsm`) and Excel workbooks (`.xlsx`).
- **R2 (ubiquitous):** The KB shall accept OpenDocument text (`.odt`, `.ott`,
  `.odm`, `.oth`) and spreadsheets (`.ods`, `.ots`) (operator, 2026-09-24).
- **R3 (ubiquitous):** The KB shall accept delimited tables (`.csv`, `.tsv`),
  each read as a table (operator, 2026-09-24).
- **R4 (ubiquitous):** The KB shall accept PDF documents, reading each page's
  text the way the Viewer reads it today, one page at a time (071 Q3; operator,
  2026-09-24).
- **R5 (ubiquitous):** The KB shall accept HTML files (`.html`, `.htm`),
  keeping their headings, lists and tables and dropping scripts, styles and
  navigation markup (operator, 2026-09-24).
- **R6 (ubiquitous):** The KB shall accept ZIP and TAR archives, and index each
  supported document inside as a document of its own (operator, 2026-09-24).
- **R7 (ubiquitous):** A format is recognised by its content where the content
  identifies it, and by its extension otherwise, so a misnamed file is still
  read correctly.

### 4.2 What the conversion keeps

- **R8 (ubiquitous):** Conversion shall keep the structure the format carries —
  document headings, one section per slide, one section per sheet, one section
  per PDF page — so that the KB can chunk by subject (068 R15).
- **R9 (ubiquitous):** A search hit from a PDF shall name its page; from a
  presentation, its slide; from a workbook, its sheet — so that the model can
  cite the place and the user can find it (068 R33).
- **R10 (ubiquitous):** A document inside an archive shall be named by the
  archive and its path within it (`contracts-2025.zip › legal/nda.docx`), both
  in search hits and in reports.
- **R11 (constraint):** Images embedded in documents shall not be indexed; only
  text is.

### 4.3 What cannot be read

- **R12 (event):** When a document cannot be converted — an old binary Office
  format, a password-protected file, a PDF with no text layer, a corrupt file,
  an unsupported format — the KB shall skip it, leave it in the folder, and
  report it by name with the reason (operator, 2026-09-24).
- **R13 (ubiquitous):** Reasons shall distinguish at least: unsupported format,
  old binary Office format, password protected, no text (scanned), and damaged.
- **R14 (constraint):** A document that cannot be read shall never stop the
  rest of the collection from being indexed (068 R20).

### 4.4 Safety

- **R15 (constraint):** An archive shall be expanded within bounds — on total
  unpacked size, on the number of files, and on nesting depth — and one that
  exceeds them shall be reported as too large rather than expanded, so that a
  small archive cannot exhaust the machine's memory or disk.
- **R16 (constraint):** A path inside an archive shall never be written to disk
  outside the collection; conversion reads archive members in memory.
- **R17 (constraint):** Conversion shall never execute anything a document
  contains — macros, scripts, embedded objects.

### 4.5 Build

- **R18 (constraint):** Every converter shall be pure Rust. The default build of
  an application using `KnowledgeBase` shall need no C compiler: in particular,
  the ZIP support shall keep its `bzip2` and `zstd` back-ends off (063 Q3).
- **R19 (constraint):** Every new dependency shall carry a licence compatible
  with the project's (Apache-2.0), such as MIT or Apache-2.0.
- **R20 (ubiquitous):** The converters shall live in the runtime, with no
  dependency on the IDE or on `cobolt-agents` (068 R4).
- **R21 (ubiquitous):** A converted document shall be handed to the KB through
  068's seam, so that adding a format later means adding a converter and
  nothing in the KB.
- **R22 (ubiquitous):** The converters shall be a **runtime crate of their own**,
  usable without the KB — the KB depends on it, never the reverse (operator,
  2026-09-24).
- **R23 (ubiquitous):** HTML shall be converted with **`htmd`** — the most
  downloaded pure-Rust HTML-to-Markdown converter meeting R18 and R19
  (operator, 2026-09-24).

## 5. Acceptance criteria

- [ ] **AC1** — A fixture of each format in R1–R6 is indexed, and a query for a
      phrase that appears only in it returns it. *(R1–R6)*
- [ ] **AC2** — A `.docx` renamed to `.txt` is still read as Word. *(R7)*
- [ ] **AC3** — A multi-heading `.docx`, a multi-slide `.pptx`, a multi-sheet
      `.xlsx` and a multi-page PDF each produce one chunk per heading, slide,
      sheet or page, and hits name their slide, sheet or page. *(R8, R9)*
- [ ] **AC4** — A document inside a ZIP inside a ZIP is found and named with its
      full path through both archives. *(R6, R10)*
- [ ] **AC5** — A `.doc`, a password-protected `.docx`, an image-only PDF, a
      truncated `.pptx` and a `.exe` are each skipped and reported with the
      right one of R13's reasons, and the rest of the folder is indexed.
      *(R12–R14)*
- [ ] **AC6** — An archive that unpacks past the size, file-count or depth bound
      is reported as too large, and memory stays within the bound while it is
      examined. *(R15)*
- [ ] **AC7** — An archive member named `../../escape.txt` writes nothing outside
      the collection. *(R16)*
- [ ] **AC8** — `cargo tree` for an application using `KnowledgeBase` shows no
      `bzip2`, `zstd` or other C-compiled crate, and the build succeeds on a
      machine with no C compiler. *(R18)*
- [ ] **AC9** — `cargo deny`-style check (or a listing of each new crate's
      licence) shows only licences compatible with Apache-2.0. *(R19)*
- [ ] **AC10** — `cargo tree` shows neither `cobolt-ide` nor `cobolt-agents`.
      *(R20)*
- [ ] **AC10a** — The converter crate builds and converts a document in a test
      that does not link the KB crate. *(R22)*
- [ ] **AC11** — Tests report quantified results: per format, documents
      converted, characters produced, and conversion time per document and per
      megabyte (GOLDEN RULE #7).

## 6. Constraints & steering check

- **Fix vs feature.** Feature: new formats for a new control. `features` branch.
- **i18n.** No IDE text is added. Skip reasons (R13) reach the program as codes
  it can translate in its own tables; the KB does not show them itself.
- **System KB.** The `KnowledgeBase` control's documentation in the
  `cobolt-compiler` doc tables lists the accepted formats and the skip reasons;
  regenerate `assets/knowledge/chunked.data` in the same change.
- **Developer's Guide.** The `KnowledgeBase` chapter gains the list of formats,
  what each keeps, the skip reasons, and the archive bounds. GOLDEN RULE #8
  applies.
- **Toolchain.** Verified 2026-09-24: `markdownify` 0.3.8 (MIT) uses `zip`
  with `default-features = false, features = ["deflate-flate2"]`, and its
  `calamine` 0.36.1 uses `zip` with only `deflate` (zopfli + zlib-rs), all pure
  Rust. `lopdf` is already in `cobolt-forms` for the Viewer.
- **Depends on 068.** This spec plugs into 068's seam and cannot ship before it.

## 7. Open questions

All settled with the operator on 2026-09-24:

- **Q1 — ✅ HTML: the most popular pure-Rust converter meeting the same rules.**
  Measured on crates.io, 2026-09-24:

  | Crate | Downloads (total / recent) | Licence | Output |
  |---|---|---|---|
  | `html2text` | 6.16 M / 1.55 M | MIT | Plain text, not Markdown: headings lost |
  | **`htmd` 0.5.5** | **4.58 M / 3.28 M** | **Apache-2.0** | **Markdown** |
  | `html-to-markdown-rs` | 1.37 M / 0.75 M | MIT | Markdown |
  | `html2md` | 1.04 M / 0.34 M | GPL-3.0+ | Excluded by R19 |

  `html2text` leads in total downloads but emits plain text, which would lose
  the headings R8 chunks by; among Markdown converters `htmd` leads on both
  counts. Its dependencies (`html5ever`, `markup5ever_rcdom`, `phf`) are pure
  Rust. Captured as R23.
- **Q2 — ✅ Archive bounds:** 500 MB unpacked in total, 10,000 files, 3 levels
  of nesting, each changeable per `KnowledgeBase` (R15).
- **Q3 — ✅ A separate runtime crate**, usable without the KB (R22).
