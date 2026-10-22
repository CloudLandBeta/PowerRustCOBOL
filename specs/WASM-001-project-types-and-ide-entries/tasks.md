# Tasks — Project types, the New Project selector and the IDE entries

- **Status:** in progress — T1–T13 done (evidence kept below); revised 2026-10-22 against plan.md (revision of
  2026-10-22): T14 (toolbox by kind) is no longer blocked and is split into T14.1–T14.3; T15 (`.wcfrm` / `.mcfrm`, Q2
  decided 2026-10-20) is split into T15.1–T15.8; T16 (strings), T17 (R6 authentication programs, waits for WASM-009) and
  T18 (finalize) are new.
- **Plan:** ./plan.md   **Date:** 2026-10-20, revised 2026-10-22

Ordered, small, independently verifiable. T1–T13 were committed on the worktree branch
`worktree-agent-a97ba65c65571c2df` and merged into `features-2x`; the remaining tasks are written on `features-2x` (the
pipeline does the git bookkeeping). On 2.x.x there is **no documentation task**: `README.md` and `docs/` are never
touched; what 2.x.x says about itself goes in `specs/`.

Before any build or test: `df -h .` first, `CARGO_INCREMENTAL=0`, only the touched crates, `cargo check` before
`cargo test` (plan §5).

## Done (T1–T13, evidence as recorded on 2026-10-20)

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
  - Open point (spec Q6, **to confirm**, not decided here): whether the Recent pane and the *Recent Projects ▾* footer
    menu are the operator's *Recent Projects*. Working assumption (plan §4): yes, kept as they are; a "no" changes only
    `panels/new_project.rs` and its `pk_recent*` strings.

- [x] **T7 — Strings** (R16, AC6)
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Done: `pk_*` fields × six tables (recorded as 163 on 2026-10-20; counted again by the plan on 2026-10-22: **160**, plan C6); placeholders filled in order (`replacen("{}", …)`), so the Japanese and Chinese tables need no positional markers.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` — includes `every_project_kind_string_is_in_six_languages` (`i18n.rs`).

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

- [x] **T13 — Final sweep of the first implementation** (AC7) — results at the bottom.

## Open — T14: the toolbox by kind (R14, second half; plan D-T14a, D-T14b)

Order: move the table first (the player stays green), then filter the toolbox.

- [x] **T14.1 — Move the web class table into `cobolt-forms`** (R14; WASM-004 R2 "one table")
  - Done 2026-10-22: `crates/cobolt-forms/src/web_class.rs`. Deviation from "keep `PlayerKind` in the player": the
    player's `class_of` answers the series kinds (WebLink A, Authentication C, Canvas2D/3D B, the E objects) **before**
    the table, so a verbatim `class_of` needs `PlayerKind`, `PLAYER_KINDS` and `player_kind_of` beside it — they are
    pure data with no player dependency and moved too; leaving them behind would make the shared `class_of` answer D
    for WebLink. `player_kind(&Ctl)` and `is_non_visual` (they read the player's `tree`) stay in the player.
    Result: `cargo test -p cobolt-forms --features render --lib web_class` — 4 passed (the moved AC1 test, the class
    sizes, the series kinds, and `available_on_web`: false for `acme:thing`, true for `Button` and `WebLink`; built-in
    controls of class D: 0).
  - Files: `crates/cobolt-forms/src/web_class.rs` (new, no feature gate — the player uses `cobolt-forms` with
    `default-features = false`, `crates/cobolt-web-player/Cargo.toml:55`), `crates/cobolt-forms/src/lib.rs` (`pub mod`).
  - Do: move `Class`, `Row`, `TABLE`, `class_of(&ControlType)` verbatim from `crates/cobolt-web-player/src/kinds.rs`
    (`:31`, `:57`, `:124`); add `available_on_web(&ControlType) -> bool` (`class_of(t) != Class::D`). Move the AC1 test
    (table ⇔ `ControlType::ALL`, `kinds.rs:216`) with the table.
  - Verify: `cargo test -p cobolt-forms web_class` — the moved AC1 test green; a new test asserts
    `available_on_web` is false for a `Custom` plug-in control and true for `Button`.

- [x] **T14.2 — The player re-exports the moved table** (R14)
  - Done 2026-10-22: `kinds.rs` re-exports `available_on_web, class_of, player_kind_of, Class, PlayerKind, Row,
    PLAYER_KINDS, TABLE`; its two table tests moved with the table, `the_series_controls_are_found_by_name` stays (it
    also checks `is_non_visual`). The shipped player is fingerprinted against `cobolt-forms` too, so
    `tests/freshness.rs` went red until `assets/web-player` was rebuilt with its `build_player` example (done; the
    player's `Cargo.lock` picked up `cobolt-numedit`, a `cobolt-forms` dependency it had not recorded yet).
    Result: lib 218 passed, `abi_host` 21, `dependency_guard` 4, `freshness` 3, `sinks` 3 — 0 failed. The grep for
    `const TABLE` now finds only `src/css.rs:52` — the CSS mapping table (`&[Mapping]`), a different table.
  - Files: `crates/cobolt-web-player/src/kinds.rs`
  - Do: `pub use cobolt_forms::web_class::{Class, Row, TABLE, class_of};` keep `PlayerKind`, `player_kind_of` and the
    `tree::Kind` mapping (player-only). No second copy of any row.
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml` (native tests, including
    `tests/dependency_guard.rs`) green; `grep -n 'const TABLE' crates/cobolt-web-player/src` finds nothing.

- [x] **T14.3 — The toolbox offers only what the kind can run** (R14, AC4 by extension)
  - Done 2026-10-22: the kind is a field `ToolboxPanel::project_kind`, set by `app.rs` each frame next to
    `visual_disabled` (the panel's existing pattern) rather than a new `show` argument; `tools_for(kind)` feeds the
    category grid and the collapsed rail; a user control is offered only when every control inside it is.
    Result: `cargo test -p cobolt-ide --bin cobolt-ide toolbox` — 21 passed; printed: desktop 62 of 62, web 62 / 0
    hidden, mobile 62 / 0 hidden. Measured correction to plan D-T14b: `TOOLS` holds no `Custom` entry, so no
    plug-in control was in the toolbox to begin with; today the filter's only visible effect is on a user control
    that contains a plug-in control.
  - Files: `crates/cobolt-ide/src/panels/toolbox.rs` (`Toolbox::show`, `:451`), `crates/cobolt-ide/src/app.rs` (the call
    `d.toolbox.show(…)`, `:19799`, passes the open project's kind).
  - Do: `show` takes the `ProjectKind`; for `Web`/`Mobile` it omits every entry whose `available_on_web` is false; for
    `Desktop` the catalogue is exactly today's (R11). User controls follow the same rule through their base type.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide toolbox` — a Desktop count equal to today's (printed); a Web and a
    Mobile count = Desktop minus class D, with the hidden names printed (D-T14b: today only `Custom` plug-in controls).
    Visual: the operator looks at a Web project's toolbox (never driven by the agent).

## Open — T15: form file extensions `.wcfrm` and `.mcfrm` (spec Q2, decided 2026-10-20; plan C1, D-T15a/b/c)

Order: one predicate first, then the writer, then every reader, so each step builds. Before closing T15, re-run the
search `"cfrm"|\.cfrm"` over `crates/` (plan §5) and either convert or justify every non-test hit.

- [x] **T15.1 — One predicate for "is this a form file"** (Q2, R9; D-T15a)
  - Files: `crates/cobolt-compiler/src/project_kind.rs`
  - Do: `ProjectKind::form_extension()` → `"cfrm"` / `"wcfrm"` / `"mcfrm"`; `FORM_EXTENSIONS`; `is_form_extension(&str)`,
    case-insensitive. No manifest field is added (the extension follows from `[project] kind`).
  - Verify: `cargo test -p cobolt-compiler project_kind` — new tests: one extension per kind; `WCFRM` accepted;
    `cfrmx`, `frm` and `""` refused.
  - Done 2026-10-22: `form_extension()`, `of_form_extension()`, `FORM_EXTENSIONS`, `is_form_extension()` and
    `is_form_path()` (`project_kind.rs`). Result: `cargo test -p cobolt-compiler --lib project_kind` — 12 passed;
    printed: desktop `.cfrm`, web `.wcfrm`, mobile `.mcfrm`; `WCFRM`/`Cfrm` accepted, `cfrmx`, `frm`, `""`, `cbl`,
    `.cfrm` refused.

- [x] **T15.2 — New Web/Mobile projects write their own extension** (Q2, R6, R10, AC2, AC3)
  - Files: `crates/cobolt-project-tools/src/create.rs` (`plan()`: the two `format!(… ".cfrm")` at `:362` and `:369`,
    including the error text for `main`).
  - Do: the entry form becomes `{forms_folder}/main-form.{form_extension}`. Desktop unchanged.
  - Verify: `cargo test -p cobolt-project-tools create` — a Web plan writes `webforms/main-form.wcfrm`, a Mobile plan
    `mobileforms/main-form.mcfrm`; `cargo test -p cobolt-ide --bin cobolt-ide project_model` —
    `the_default_request_is_the_golden_desktop_manifest` stays byte-identical and
    `what_the_tools_create_is_what_the_ide_saves_and_the_two_routes_make_one_tree` covers the new names (AC3).
  - Done 2026-10-22. Result: `cobolt-project-tools` lib test `create_plans_the_entry_form_with_the_kinds_extension`
    (web `webforms/main-form.wcfrm`, android-phone and ipad `mobileforms/main-form.mcfrm`, Desktop unchanged, the
    `main` refusal names the `.wcfrm`); IDE `project_model` — `the_default_request_is_the_golden_desktop_manifest`
    green, the agreement test prints *27 requests … 19 forms named with their kind's extension*. The dialog's
    `entry_shown()` (`panels/new_project.rs`) shows the same name. Four assertions in `tests/kinds.rs` and two in
    `project_model.rs` that still expected `main-form.cfrm` in Web/Mobile projects were updated to Q2.

- [x] **T15.3 — The project tools treat the new extensions as forms** (Q2, R10, R13)
  - Files: `crates/cobolt-project-tools/src/host.rs` (`:114`, `FileList::Forms`), `tools/check.rs` (`:142`),
    `tools/validate.rs` (`:26`, and the message at `:52`), `tools/regenerate.rs` (`:53`, message `:57`),
    `tools/run.rs` (`:183`–`:184`), `tools/render.rs` (`:60`), `web/devserver.rs` (`:280`, MIME),
    `reference.rs` (`:436`, the form-format prose names the three extensions).
  - Do: every site calls `is_form_extension`; messages name ".cfrm, .wcfrm or .mcfrm".
  - Verify: `cargo test -p cobolt-project-tools --test kinds` — a `.wcfrm` project is driven through `check`, `validate`,
    `regenerate`, `run`, `render` and `list_files`; one printed line per tool says it was treated as a form.
  - Done 2026-10-22; the five tool path descriptions in `tools/mod.rs` name the three extensions too. Result:
    `cargo test -p cobolt-project-tools --test kinds` — 7 passed; `every_form_tool_takes_the_web_and_mobile_form_extensions`
    prints one line per tool for each kind (list_files, validate — 5 controls, regenerate — `generated/main-form.cbl`,
    check — 0 errors, run_form, render_form). `run_form`/`render_form` are driven with stub runner/renderer (as
    `tests/tools.rs` does): the test proves the tools accept the file, not that a desktop engine draws it.

- [x] **T15.4 — The IDE recognises the new extensions** (Q2, R12, R13, AC4)
  - Files: `crates/cobolt-ide/src/project_model.rs` (`:1402`, `FileKind::Form`), `app.rs` (`:7112`, `:14744` — open/save
    dialog filters and the default file name use the open project's `form_extension()`), `panels/project.rs` (`:722`),
    `panels/designer.rs` (`:16413`), `panels/forms_list.rs` (`:206`), `agent_kit/ide_host.rs` (`:354`).
  - Do: every site calls `is_form_extension` / `form_extension()`; a new form in a Web project is `*.wcfrm`, in a
    Mobile project `*.mcfrm`, in a Desktop project `*.cfrm` (R11).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide project_kind_tests` — a `.wcfrm` is listed under WebForms and a
    `.mcfrm` under MobileForms; the `[+]` of WebForms creates `webforms/<name>.wcfrm`.
  - Done 2026-10-22: `FileKind::from_path`, the add-file filter (all three), the New Form save dialog (the kind's
    extension and default name, through `pasted_form_file_name(name, kind)`, which the pasted form shares), the
    folder view in `panels/project.rs`, the designer's form picker, `forms_list::scan_dir`, the agent kit's
    unsaved-model test. Result: `cargo test -p cobolt-ide --bin cobolt-ide -- project_model new_project pasted_form
    toolbox` — 93 passed, incl. `web_and_mobile_form_files_are_forms_listed_under_their_entry` and the pasted-name
    test (`login.wcfrm`, `login.mcfrm`). The `[+]` itself opens a native Save dialog, so the test covers the name it
    proposes, not the dialog.

- [x] **T15.5 — The Web build's secret scan and the dev server see the new files** (Q2; WASM-002 R7, WASM-013 R7 —
  security)
  - Files: `crates/cobolt-web-bundle/src/secrets.rs` (`:259`, the scanned-extension list), `crates/cobolt-web-bundle/src/devserver.rs` (`:88`, MIME).
  - Verify: `cargo test -p cobolt-web-bundle secrets` — a fake key planted in `webforms/x.wcfrm` (and `mobileforms/x.mcfrm`)
    is reported, as one in `forms/x.cfrm` already is.
  - Done 2026-10-22. Measured: the bundle's scan (`lib.rs:775`) reads the bundle's own files, and the bundle writes
    every form as `forms/<NAME>.cfrm` (`lib.rs:520`, unchanged: a bundle-internal name the player reads from
    `manifest.json`), so web forms were already scanned there; the list now also covers `.wcfrm`/`.mcfrm` for any
    other caller. Result: `cargo test -p cobolt-web-bundle secrets` — 8 passed;
    `a_secret_planted_in_a_web_or_mobile_form_is_reported` prints one finding each for `forms/x.cfrm`,
    `webforms/x.wcfrm`, `mobileforms/x.mcfrm`.

- [x] **T15.6 — `rcrun run-form` accepts the three extensions** (Q2; interpreter/binary parity)
  - Files: `crates/cobolt-cli/src/form_gui.rs` (`:79`).
  - Verify: `cargo check -p cobolt-cli` clean; `cargo test -p cobolt-cli` green; T15.3's `run` line exercises
    `rcrun run-form` on a `.wcfrm`.
  - Done 2026-10-22: `find_cfrm_by_stem` (the `open-form:` lookup) uses `is_form_path`; `run-form`'s own argument
    was never tested by extension. Result: `cargo check -p cobolt-cli` clean; `cargo test -p cobolt-cli
    --no-fail-fast` — every target green (unit 7 incl. the new `a_form_is_found_by_its_name_whatever_its_kind_calls_the_file`).

- [x] **T15.7 — A form whose extension does not match the project's kind** (Q2, R18; D-T15b, D-T15c)
  - **Open point:** D-T15b is a *proposal to be confirmed by the operator* (plan §9); the spec's Q2 does not say.
    Working assumption, applied only to the non-destructive half: a `.cfrm` in a Web/Mobile project (made by 2.0.x before
    T15) and a `.wcfrm`/`.mcfrm` in a project of another kind are **kept, listed and loaded**, never renamed on open, and
    `check` reports one **warning** per such file. The explicit *Rename to .wcfrm / .mcfrm* tree action (renames the
    file and its `files.forms` entry, nothing else) is **built only after the operator confirms D-T15b**; until then the
    box stays open with that note.
  - Files: `crates/cobolt-project-tools/src/tools/check.rs` (the warning, English like the crate's other diagnostics);
    `crates/cobolt-ide/src/panels/project.rs` and `project_model.rs` (listing; the rename action once confirmed).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide project_kind_tests` — a Web project holding
    `webforms/main-form.cfrm` opens with the manifest and the form file byte-identical and their names unchanged, and the
    form is listed under WebForms; `cargo test -p cobolt-project-tools --test kinds` — `check` prints the warning exactly
    once for that file and none for a matching `.wcfrm`.
  - Non-destructive half done 2026-10-22 (box stays open: the rename action waits for the operator's answer on
    D-T15b). `check` warns once per form named for another kind (`form_name_mismatch`, `tools/check.rs`), the form
    still loads and checks. Result: `check_warns_once_about_a_form_named_for_another_kind` (kinds) — a matching
    `.wcfrm` draws none, `webforms/main-form.cfrm` draws exactly one (*"… is a desktop form file name in a web
    project; rename it to main-form.wcfrm (the form loads as it is)"*), 0 errors, nothing renamed, manifest
    untouched; IDE `a_web_project_holding_a_legacy_cfrm_opens_unmodified_and_lists_it` — 17 files byte-identical after
    opening, the form listed under WebForms. The IDE's own Check does not show this warning yet (it runs its own
    path, not the tool's); adding it there is part of the rename work, with its `Tr` strings (T16).
  - Rename half done 2026-10-22, after the operator confirmed D-T15b ("Manter + avisar + Renomear"):
    `project_model::form_rename_for_kind` gives the new name (only the extension changes); the form row's context
    menu offers *Rename to <name>* (`panels/project.rs`, `ProjectPanelEvent::RenameFormForKind`) only for a form named
    for another kind; `app.rs::do_rename_form_for_kind` renames the file and its `files.forms` entry, rewrites open
    tabs and refuses when the new name exists. The IDE's own Check now names each such form too
    (`warn_forms_named_for_another_kind`). Strings: `pk_rename_form_kind`, `pk_check_form_name_kind` (six languages)
    and the Output texts *Renamed {} → {}* / *Could not rename {}: {}* (`ide_text_data.rs`). Result:
    `project_kind_tests` 13 passed (incl. `the_rename_offered_for_a_form_named_for_another_kind`, 7 cases);
    `every_project_kind_string_is_in_six_languages` 1 passed; `ui_text` 6 passed.

- [x] **T15.8 — Agent knowledge and the System KB name the extensions** (Q2, R13; System KB rule)
  - Files: `crates/cobolt-project-tools/src/content.rs` (plugin rules and the `web-kinds` skill, ~`:452`, ~`:491`,
    ~`:1173`–`:1188`, ~`:1303`: the extension next to the folder); `crates/cobolt-compiler/src/lib.rs` (the System KB
    document *Project kinds*, table ~`:6186`: an extension column); `assets/knowledge/chunked.data` regenerated with
    `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`
    (`grace_host.rs`) green after regeneration; `cargo test -p cobolt-project-tools content` green.
  - Done 2026-10-22: four places in `content.rs` name `.wcfrm`/`.mcfrm` beside the folder; the KB *Project kinds*
    table gains a *Form file* column and a sentence on a form named for another kind; the `.cfrm` format reference
    (`reference.rs`) says the format has three names. `chunked.data` regenerated (3516 records from 8 documents).
    Result: `prebuilt_chunked_kb_matches_the_published_documentation` 1 passed; `cobolt-project-tools content` 4 passed.
  - Closing search (`"cfrm"` literals outside tests, re-run 2026-10-22), each remaining hit justified: the predicate
    itself; `powerchat.rs`/`tools/powerchat.rs` and `patterns.rs` (Desktop sample forms copied into `forms/`);
    `cobolt-compiler/src/lib.rs:2613` (the Desktop binary's staging; `build_project` refuses other kinds);
    `cobolt-web-bundle/src/lib.rs:520` (bundle-internal name); the MIME tables (all three); comments; and the i18n
    text `forms_no_cfrm` (an empty-list hint that names `.cfrm`; shown only when a folder holds no form).

## Open — the rest

- [x] **T16 — Strings for anything T14/T15 shows in the IDE** (R16, AC6)
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Do: a `Tr` field × six languages for every message the IDE shows from T14.3/T15.4/T15.7 (the mismatch warning if it
    surfaces in the IDE, the rename action label once D-T15b is confirmed, any toolbox note). If no new IDE string is
    needed, record that here and close the box.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` — `every_project_kind_string_is_in_six_languages` green, with
    the new fields added to its list.
  - Closed 2026-10-22 with no new field: T14.3 shows nothing new (it hides entries), T15.4 reuses the existing
    "RustCOBOL Form" dialog filter label, and T15.7's warning is a `cobolt-project-tools` diagnostic (English, like
    the crate's others). The rename action's label and an IDE-side mismatch message come with D-T15b.
    Reopened and closed again 2026-10-22 with T15.7's rename half: `pk_rename_form_kind` and
    `pk_check_form_name_kind`, in the test's list through their `pk_` prefix.

- [ ] **T17 — Authentication programs of the backend scaffold** (R6, second half; WASM-003 R22) — **waits for WASM-009**
  (plan-only, `9af4dcc4`: "NOT implemented"). Hook fixed by plan §4: WASM-009's generator adds its files to `Plan.files`
  and `Plan.backend` inside `create::plan`, behind the same `backend` flag. Nothing is generated ahead of WASM-009.
  - Verify (when it lands): `cargo test -p cobolt-project-tools create` — a Web plan with `backend` on lists the
    authentication programs; with `backend` off, none.

- [x] **T18 — Finalize** (AC7)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`. Never `README.md` or `docs/`.
  - Do: bump `z` of `VERSION` once (2.0.29 today → the next `z` at the time of the commit; never `x` or `y`); add one
    entry above the previous `## [PowerRustCOBOL …]` heading, in the existing shape:
    `## [PowerRustCOBOL 2.0.N] — <date>` then `*(Written on `features-2x`, after 2.0.N-1.)*`, saying what T14/T15 changed
    and what stays open (T15.7's rename action, T17, Q6).
  - Verify: run the tests of the crates touched, `--no-fail-fast`, every `test result` line read and reported:
    `cargo test -p cobolt-compiler --lib --no-fail-fast`, `cargo test -p cobolt-forms --features render --no-fail-fast`,
    `cargo test -p cobolt-project-tools --no-fail-fast`, `cargo test -p cobolt-web-bundle --no-fail-fast`,
    `cargo test -p cobolt-cli --no-fail-fast`, `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`,
    `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast`. The three known reds of T13 are
    reported again and not counted as new; any other red is a failure of this work. Manual check: none driven by the
    agent; the operator looks at the dialog, the tree and a Web project's toolbox.
  - Done 2026-10-22: `VERSION` 2.0.31 → **2.0.32**; one CHANGELOG entry above 2.0.31. Sweep (`CARGO_INCREMENTAL=0`,
    `--no-fail-fast`, every `test result` line read):

    | Crate / target | Result |
    |---|---|
    | `cobolt-forms --features render` | lib 1357 passed / 1 ignored; every integration target green |
    | `cobolt-project-tools` | lib 101; `kinds` 7; `tools` 13; others green; 1 ignored (`tests/build.rs`) — 0 failed |
    | `cobolt-web-bundle` | 19 + 2 + 1 + 3 passed, 0 failed |
    | `cobolt-cli` | every target green (unit 7) |
    | `cobolt-web-player` (own workspace) | 218 + 21 + 4 + 3 (freshness) + 3, 0 failed |
    | `cobolt-ide --bin cobolt-ide` | **1868 passed, 5 failed, 4 ignored** |
    | `cobolt-compiler --lib` | **194 passed, 4 failed, 1 ignored** |

    The reds, none in a file this work touched: the three known ones of T13
    (`docs_embed::every_document_ships_in_every_language`, `external_crates_service::flow_tests::{live_resolver_verdicts,
    compatible_and_reserved_collisions_still_refuse}`), and six that are new since T13 but have other causes:
    `claude_code::bundle::claude_validates_the_written_bundle` (the installed `claude plugin validate` now refuses a
    plugin without a README.md — README_MISSING; environmental), `editor::completion_text_tests::every_completion_description_exists_in_six_languages`
    (two backend file-transfer completion texts without translations, from the backend work),
    `sdk_tests::sdk_crate_set_is_closed_under_path_dependencies` (*"cobolt-forms depends on cobolt-numedit, which the
    SDK does not ship"* — since 2.0.16) and three `resolve_main_tests` that fail the same way (`cobolt-decimal` missing
    from the staged SDK). They are reported, not fixed here.

## Done criteria

| Acceptance criterion | Covered by |
|---|---|
| AC1 — dialog layout, title/options per template, unavailable entries not creatable | T2, T6 |
| AC2 — kind (and Mobile platform/device) in the manifest, reloaded; old manifest loads as desktop, unmodified | T1, T3, T5; T15.2 (new names), T15.7 (a legacy `.cfrm` opens unmodified) |
| AC3 — IDE, `rcrun` (via `rcrun mcp`) and `create_project` byte-identical | T8; T15.2 extends it to `.wcfrm`/`.mcfrm` |
| AC4 — tree per kind; Desktop unchanged | T5; T15.4 (forms of the new extensions listed in their entry); T14.3 (toolbox by kind) |
| AC5 — a template added by a descriptor only | T2 |
| AC6 — six languages | T7, T16 |
| AC7 — desktop suites unaffected | T13, T18 |

Spec R14's toolbox half closes with T14; Q2 closes with T15. Still open after this list: T15.7's rename action (D-T15b to
confirm), T17 (WASM-009), spec Q6 (to confirm), the `[desktop]` options consumed by nothing yet, and the Desktop target
platform showing the host only (Q4, decided). The spec-text tidy noted in plan §9 belongs to the next `/clarify`, not to
these tasks.

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
