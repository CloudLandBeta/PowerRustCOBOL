# Tasks — Project types, the New Project selector and the IDE entries

- **Status:** in progress — T1–T13 done (results at the bottom); T14 is open (blocked on WASM-004); T15 is open (form file extensions, decided 2026-10-20)
- **Plan:** ./plan.md   **Date:** 2026-10-20

Ordered, small, independently verifiable. Commits are on the worktree branch `worktree-agent-a97ba65c65571c2df`
(never pushed; the lead merges).

- [x] **T1 — The shared model** (R8, R9, R18)
  - Files: `crates/cobolt-compiler/src/project_kind.rs`, `crates/cobolt-compiler/src/lib.rs`
  - Done: `ProjectKind`, `MobilePlatform`, `DeviceClass`, `Orientation`, `ApplicationStyle`, `Isolation`, `BrowserMatrix`, `WebSettings`, `MobileSettings`, `BackendSettings`, `DesktopSettings`; `ProjectMeta.kind`, `ProjectFiles.backend`; `ManifestView` carries them; `build_project` refuses a non-Desktop project (`NotADesktopProject`).
  - Verify: `cargo test -p cobolt-compiler project_kind` — 10 tests plus `project_kind_build_tests`.

- [x] **T2 — Template data** (R2, R4, R5, AC5)
  - Files: `crates/cobolt-project-tools/src/templates.rs`
  - Done: the catalogue as `&'static` data; creatable ids `desktop, web, android-phone, android-tablet, iphone, ipad`; the rest listed unavailable; a fake descriptor test.
  - Verify: `cargo test -p cobolt-project-tools templates`.

- [x] **T3 — The creation plan** (R3, R6, R10)
  - Files: `crates/cobolt-project-tools/src/create.rs`, `host.rs`
  - Done: `plan()` + `Plan::write_into()`; folders, sample main form with a button handler, `backend/src/health.cbl` (exported `HEALTH`); typed table wrappers so the manifest bytes equal the IDE's `save_project`; `clone_project` for *Create from Template…*.
  - Verify: `cargo test -p cobolt-project-tools create`.

- [x] **T4 — The `create_project` tool and the agent surface** (R10, R13)
  - Files: `crates/cobolt-project-tools/src/tools/{mod,list,check,build,register}.rs`
  - Done: `kind`, `platform`, `device`, `template`, `options`, `locale`, `description`; `list_files` reports `kind`, `forms_folder`, `backend`, `web_assets`; `check` covers Backend files; `build` for Web reports a `BuildReport`.
  - Verify: `cargo test -p cobolt-project-tools --test kinds` (5).

- [x] **T5 — Tree categories and routing** (R11, R12, R13, AC4)
  - Files: `crates/cobolt-ide/src/project_model.rs`, `panels/project.rs`, `app.rs`
  - Done: `Category::{WebForms, MobileForms, Backend}`, `top_for(kind)`, `forms_for(kind)`, `category_of_path`, drop acceptance; folders back-filled; `[+]` creates an item of its kind.
  - Verify: `project_kind_tests::{the_tree_follows_the_kind, files_are_routed_by_the_projects_kind, walking_the_forms_finds_them_in_whichever_folder_the_kind_keeps_them}`.

- [x] **T6 — The New Project dialog** (R1, R5, R7, R17, AC1)
  - Files: `crates/cobolt-ide/src/panels/new_project.rs`, `ui_prefs.rs`, `app.rs`
  - Done: left list with search, right pane (properties + options), Recent Projects, *Create from Template…*; explicit size and one grip.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide new_project` (19); the frame tests print their frame counts.

- [x] **T7 — Strings** (R16, AC6)
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Done: 163 `pk_*` fields × six tables; placeholders filled in order (`replacen("{}", …)`), so the Japanese and Chinese tables need no positional markers.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n`.

- [x] **T8 — Byte identity** (AC3)
  - Done: `what_the_tools_create_is_what_the_ide_saves_and_the_two_routes_make_one_tree` (27 requests), `the_default_request_is_the_golden_desktop_manifest`.

- [x] **T9 — Project Settings for the kinds** (R4, WASM-002 R20)
  - Files: `crates/cobolt-ide/src/panels/web_settings.rs`, `settings_form.rs`
  - Done: style, browser matrix, multi-memory (iOS off by default), isolation, backend address (refused unless `wss://`, or `ws://` on this machine), Mobile platform / device / orientation / minimum OS, Backend exports; a live line says which program-module variants the choices produce.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_settings` (7).

- [x] **T10 — Run / Build / Preview by kind** (R14, first half)
  - Files: `crates/cobolt-ide/src/app.rs`, `panels/web_preview.rs`
  - Done: see WASM-008 tasks. Desktop paths unchanged (`is_web_project()` guards every branch).

- [x] **T11 — Agent knowledge** (WASM-015 R10, R13, R14, in part)
  - Files: `crates/cobolt-project-tools/src/content.rs` (two rules `web-kinds`, `backend-secrets`; two skills; a reviewer check; the architecture paragraph), `crates/cobolt-compiler/src/lib.rs` (the System KB document `project_model_and_settings.md` gains *Project kinds*), `assets/knowledge/chunked.data`.

- [x] **T12 — `rcrun package`** (R9)
  - Files: `crates/cobolt-cli/src/main.rs` — `[files] backend` is packaged with the rest.

- [x] **T13 — Final sweep** (AC7) — results below.

- [ ] **T14 — Toolbox by kind** (R14, second half) — **blocked**: needs the web class table of WASM-004 R5/R12.

- [ ] **T15 — Form file extensions `.wcfrm` and `.mcfrm`** (Q2, decided 2026-10-20) — **open**: web forms are written as `.wcfrm` and mobile forms as `.mcfrm`; load, save, the tree, the compiler's project reader, the project tools, the agent kit and the plugin rules recognise both. The rule for a `.cfrm` already inside a web or mobile project (keep, convert or report) is decided in `/plan`.

## Done criteria

AC1–AC6 are met by the tests named above; AC7 by T13; R14's toolbox half and the authentication programs of the backend
scaffold remain open and are listed in plan.md §8.

## Results (T13, run 2026-10-20 with `CARGO_INCREMENTAL=0`, `--no-fail-fast`, every `test result` line read)

| Crate / target | Result |
|---|---|
| `cobolt-compiler` `--lib` | 126 passed in the run without `resolve_main_tests`, plus `resolve_main_tests` 69 passed / 1 ignored (730 s: those tests build real programs) |
| `cobolt-compiler` `--test dependency_guards_063` | 3 passed (`ring` and `aws-lc-rs` stay out) |
| `cobolt-project-tools` (lib + 7 integration targets) | 137 passed, 1 ignored (`tests/build.rs`, needs a toolchain), 0 failed |
| `cobolt-web-preview` | 5 passed |
| `cobolt-ide --bin cobolt-ide` | **1866 passed, 3 failed, 4 ignored** |
| `cargo check -p cobolt-cli` | clean |

The three IDE failures are not this work's, and none is new here:
`docs_embed::every_document_ships_in_every_language` (eleven English-only documents under `docs/` — the red GOLDEN RULE #8 asks to leave red until the next minor regenerates the translations; `docs/` is untouched on 2.x.x), and
`external_crates_service::flow_tests::{live_resolver_verdicts, compatible_and_reserved_collisions_still_refuse}` (they resolve the live `crates.io` index, which now answers `egui 0.37.0`; environmental).

Two earlier red results happened while the shared volume was 100 % full (`ENOSPC` was seen by the tools in the same minutes) and are not counted: 20 `resolve_main_tests` that passed when the module was re-run alone (69 passed), and one link failure that passed when repeated.
