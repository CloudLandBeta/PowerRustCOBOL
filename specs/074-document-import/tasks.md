# Tasks — Document import for the Application Knowledge Base

- **Status:** done
- **Plan:** approved in session 2026-09-24 (`cobolt-docs` crate + 068 seam)   **Date:** 2026-09-24

## Phase 1 — `cobolt-docs`

- [x] **T1 — Crate skeleton, SDK registration, format detection** (R7, R18, R22)
  - Files: `crates/cobolt-docs/{Cargo.toml,src/lib.rs,src/sniff.rs}`, workspace
    `Cargo.toml`, `cobolt-compiler` `SDK_CRATES`.
  - Verify: `sniff` unit tests (compound files, text vs binary).
- [x] **T2 — Office, OpenDocument, tables** (R1–R3, R8)
  - Files: `src/office.rs` — docx outline-level normalisation, markdownify's
    sheet/opendoc/csv readers called directly.
  - Verify: unit test (Portuguese style ids → headings); AC1, AC3.
- [x] **T3 — PowerPoint, PDF, HTML** (R4, R5, R9)
  - Files: `src/pptx.rs` (own reader, presentation order, numbered slides),
    `src/pdf.rs` (page text, Encrypt, no text), `src/html.rs` (`htmd`).
  - Verify: AC1, AC3, AC5.
- [x] **T4 — Archives within bounds** (R6, R10, R14–R16)
  - Files: `src/archive.rs` — one budget across nesting, member skips, name
    cleaning, gzip transparent.
  - Verify: AC4, AC6, AC7 (`tests/formats.rs`).

## Phase 2 — Knowledge Base

- [x] **T5 — Seam, parts, schema 2** (R10, R21)
  - Files: `cobolt-kb/src/{convert,refresh,store,search}.rs`.
  - Verify: `cobolt-kb/tests/documents.rs` — hits name `Page 2` and a member
    two archives deep; skips named through the archive.
- [x] **T6 — Control properties, runtime, docs**
  - Files: `cobolt-forms` model (3 `ArchiveMaximum…` properties), runtime
    `kb_config`/`KbConfig`, IDE properties pane, `SkippedDocuments` codes,
    compiler doc tables + `chunked.data`, guide section *Which documents it
    reads*.
  - Verify: `test_knowledge_base.rs` archive test; `every_control_property_is_documented`;
    `prebuilt_chunked_kb_matches_the_published_documentation`.
- [x] **T7 — Finalize**: `cargo tree` (AC8, AC10), licence listing (AC9), sweeps,
  CHANGELOG + `z`.
