# Tasks — Indexed File Editor & Grid Browser

- **Status:** complete
- **Plan:** ./plan.md   **Date:** 2026-06-15

Ordered, small, independently-verifiable tasks. Check off as completed.

- [x] **T1 — Scaffold `cobolt-indexed` crate** (R8, R22a)
  - Files: `Cargo.toml`, `crates/cobolt-indexed/src/{lib,model}.rs`
  - Do: Workspace member; core `IndexedDefinition`, `IndexedField`, keys, record format, storage flags.
  - Verify: `cargo build -p cobolt-indexed` && `cargo test -p cobolt-indexed`

- [x] **T2 — XML load/save for `.cidx`** (R22a)
  - Files: `crates/cobolt-indexed/src/xml.rs`, `tests/fixtures/`
  - Do: `load_indexed` / `save_indexed`; round-trip test (fixed + variable + alternates + controls).
  - Verify: `cargo test -p cobolt-indexed xml`

- [x] **T3 — Helpers: paths, inspect, controls, warnings** (R4, R9, R15, R25)
  - Files: `paths.rs`, `inspect.rs`, `control_defaults.rs`, `schema_support.rs`, `import.rs`
  - Do: `store_path`/`resolve_path`; `inspect_any_path`; PIC→control; `finalize_warnings`; import field synthesis.
  - Verify: `cargo test -p cobolt-indexed`

- [x] **T4 — Project model: Indexed Files category** (R1, R2, R7, R24)
  - Files: `crates/cobolt-ide/src/project_model.rs`, `app.rs` (folder back-fill)
  - Do: `files.indexed`, `Category::IndexedFiles`, `FileKind::Indexed`, `TOP` order, `indexed/` folder.
  - Verify: `cargo test -p cobolt-ide project_model`

- [x] **T5 — `generate_indexed` codegen** (R22)
  - Files: `crates/cobolt-codegen/src/indexed.rs`, `Cargo.toml`, `lib.rs`
  - Do: SELECT/FD fragment + banner; golden tests (fixed DISK, variable MEMORY, alternate duplicates).
  - Verify: `cargo test -p cobolt-codegen indexed`

- [x] **T6 — Runtime IDE helpers** (R9, R20, R26)
  - Files: `crates/cobolt-runtime/src/indexed_ide.rs`, `lib.rs`
  - Do: `create_empty_from_definition`, `compare_schema`, `GridSession` (open/read/write/rewrite/delete/commit).
  - Verify: `cargo test -p cobolt-runtime indexed_ide`

- [x] **T7 — i18n strings (×6 languages)** (R27)
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Do: All Indexed File Editor, wizard, grid, warning strings.
  - Verify: `cargo test -p cobolt-ide i18n`

- [x] **T8 — Project tree: Indexed Files section** (R1, R3, R4, AC1, AC2b)
  - Files: `panels/project.rs`, `app.rs` (event handling)
  - Do: Tree section, ➕, Import existing…, field sub-tree, open editor on click.
  - Verify: `cargo build -p cobolt-ide`

- [x] **T9 — New / Import dialogs** (R3, R4, R8, R12, R25, AC2, AC2a, AC4)
  - Files: `panels/indexed_new_dialog.rs`, `indexed_import.rs`, `app.rs`
  - Do: Wizard (fixed/variable, storage); import via `inspect_any_path`; relativized paths.
  - Verify: `cargo build -p cobolt-ide`

- [x] **T10 — Indexed File Editor viewport + properties** (R5–R13, R10, R11, AC3, AC5)
  - Files: `panels/indexed_editor.rs`, `indexed_properties.rs`, `app.rs`
  - Do: Separate viewport; field list; file/field properties; Save / Save & Generate / Finalize; structural lock.
  - Verify: `cargo build -p cobolt-ide`

- [x] **T16 — Four-pane editor layout + IDE theme** (R5a–R7a, AC5a, AC5b, Q9–Q10)
  - Files: `panels/indexed_editor.rs`, `indexed_properties.rs`, `indexed_toolbar.rs`, `app.rs`
  - Do: Toolbar top pane; structure | label | value columns; `>` selection marker;
    split property labels/values; `apply_opaque_viewport_theme`; theme-aware toolbar icons;
    inline inspector three-column body; raw mode hides label/value panes.
  - Verify: `cargo build -p cobolt-ide`; manual walkthrough AC5a/AC5b

- [x] **T11 — Regenerate hook + generated `.cbl`** (R22, R23, AC10, AC11)
  - Files: `app.rs`, `project_model.rs`
  - Do: `write_generated_indexed_for`, `regenerate_all_indexed_files`; hook Build/Run/Debug/Check; read-only generated entries.
  - Verify: `cargo test -p cobolt-ide` + manual mtime check

- [x] **T12 — Grid Browser viewport** (R16–R21, R20a, AC6–AC9, AC9a)
  - Files: `panels/indexed_grid.rs`, `app.rs`
  - Do: Separate window; virtualized grid; add/edit/delete; Commit/Rollback; FILE STATUS → output.
  - Verify: `cargo build -p cobolt-ide` + `cargo test -p cobolt-runtime indexed_ide`

- [x] **T13 — Schema drift + packaging warnings** (R24, R26, AC13)
  - Files: `app.rs`, `project_model.rs`
  - Do: Drift modal on open; block grid writes; package external-path warnings.
  - Verify: `cargo test -p cobolt-ide project_model`

- [ ] **T14 — Docs & registry** (steering) — *stale after T16; rerun `/docsync`*
  - Files: `docs/developers-guide-en.md`, `specs/steering/docs.md`
  - Do: §5 six categories; Indexed File Editor section with **four-pane** layout diagram;
    update `indexed-file-editor.png` placeholder; registry row.
  - Verify: doc anchors present; screenshot caption matches structure | labels | values

- [x] **T15 — Finalize release metadata**
  - Files: `CHANGELOG.md`, `crates/cobolt-ide/src/version.rs`, `tasks.md` checkboxes, `spec.md` AC boxes
  - Do: Minor version bump; CHANGELOG entry; full test run.
  - Verify: `cargo test --workspace` (touched crates green); `cargo build -p cobolt-ide`

## Done criteria

All AC1–AC13 and AC5a–AC5b satisfied, tests pass, English docs updated (T14 after
`/docsync`), no hard-coded UI strings.