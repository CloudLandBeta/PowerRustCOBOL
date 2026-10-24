# Plan — Project types, the New Project selector and the IDE entries

- **Status:** revised 2026-10-22 against the spec as it stands (decisions of 2026-10-20 recorded in §7 of the spec). The first
  implementation (T1–T13) is on `features-2x`; this revision plans what the spec now asks and the code does not yet do (§8).
- **Spec:** ./spec.md   **Date:** 2026-10-20, revised 2026-10-22
- **Read with:** `specs/WASM-INDEX.md` (D12, D13), `specs/WASM-PLAN.md` (W5), `specs/WASM-ABI-v1.md` (not affected: the
  ABI addresses forms by manifest index, never by file name).

## 0. What changed in this revision, and why

| # | Change | Why |
|---|---|---|
| C1 | **Q2 is decided: web forms are `.wcfrm`, mobile forms `.mcfrm`** (operator, 2026-10-20). The first plan kept `.cfrm` for every kind. §3, §4 and new task T15 now carry the extension work, with the list of every place that tests the extension (§2). | The spec's Q2 was rewritten from "recommended: stay `.cfrm`" to the operator's decision. Code still writes `webforms/main-form.cfrm` (`cobolt-project-tools/src/create.rs`, `plan()`, the `format!("{}/{}.cfrm", …)` of the entry form). |
| C2 | **R7 is now ubiquitous** (footer always offers both entries; *Create from Template…* copies a saved project). The existing `create::clone_project` and the footer of `panels/new_project.rs` already do this; kept. | Spec R7 was `optional` and became `ubiquitous` on 2026-10-21. |
| C3 | **Q6 has a point "to confirm"**: whether the Recent view already in `new_project.rs` is the operator's *Recent Projects*. Not decided here; working assumption in §4. | Binding rule: a "to confirm" stays open. |
| C4 | **R14's toolbox half is no longer blocked on a missing table.** WASM-004's class table exists — `crates/cobolt-web-player/src/kinds.rs` (`Class`, `TABLE`, `class_of`) — but in the player's **own Cargo workspace** (its `Cargo.toml` declares `[workspace]`), which `cobolt-ide` cannot depend on. T14 is re-planned around moving the data to `cobolt-forms` (§4). | The first plan said "the class table does not exist yet"; it now does. |
| C5 | **Q1, Q4, Q5, Q7, Q8 are decided** (operator, 2026-10-20: the recommendations). The first plan already followed every recommendation, so no design change; the plan now cites them as decisions, not assumptions. | Spec §7. |
| C6 | Corrected a wrong test name: the six-language check is `every_project_kind_string_is_in_six_languages` (`crates/cobolt-ide/src/i18n.rs`), not `project_kind_strings_wasm001`; `pk_*` fields counted again: **160**, not 163. Dropped the claim that `grace_session.rs` was changed (no WebForms/Backend reference found there; the tree vocabulary is in `tool_exec.rs` and `target_select.rs`). | "Ground every claim in a file you read." |
| C7 | Versioning: the first plan said `version.rs`/`CHANGELOG.md` were not touched (a worktree agent leaving that to the lead). tech.md requires a `z` bump and a CHANGELOG entry with every change; the implementing commit of T14/T15 carries them (2.x.x numbering). `README.md` and `docs/` stay untouched. | tech.md *Versioning*; CLAUDE.md 2.x.x rule. |

## 1. Approach

One shared model, one shared plan, three readers — unchanged. Everything that is *about* a kind lives in `cobolt-compiler`
(`src/project_kind.rs`: `ProjectKind`, `has_backend`, `forms_folder`, the settings tables); everything that is *about
creating one* lives in `cobolt-project-tools` (`src/templates.rs`, `src/create.rs`); the IDE draws what those two hold.
Nothing in the dialog's drawing code names a template. The revision adds one more thing *about a kind* to the shared model:
its form file extension (C1), so that every reader asks the kind instead of testing a literal `"cfrm"`.

| Requirement | Where it is met (status) |
|---|---|
| R1, R17 | `cobolt-ide/src/panels/new_project.rs` — `NewProjectDialog::show`; explicit size, one grip, left list + right pane, title from `title()`. Tests `the_window_does_not_grow_while_nobody_touches_it`, `the_size_only_changes_by_a_drag_and_stops_at_its_limits`. **Done.** |
| R2, AC5 | `cobolt-project-tools/src/templates.rs` — `TemplateDescriptor` (`id`, `kind`, `platforms`, `creatable`, options `OptionDescriptor`/`Choice`), `builtin()`, `find`, `resolve`; the fake-descriptor test (`id: "fake"`). **Done.** |
| R3 | `create::plan` writes `[project] description, locale` and the theme (`CreateRequest.theme`, checked against `cobolt_forms::theme::ThemeCatalog::builtin()`). **Done.** |
| R4 | `templates.rs`: Desktop options `window_style`, `ui_framework`, `high_dpi`, `gpu_acceleration`, `sample`; Web/Mobile `style` (`APP_STYLES`: `spa` default, `standalone` — WASM-011 R1), `backend`; Mobile `orientation`, `min_os`. The test asserting that Desktop options do not appear on Web/Mobile and vice versa stays. **Done.** |
| R5 | `creatable_ids()` = `desktop, web, android-phone, android-tablet, iphone, ipad` (D12/Q3); others listed under `pk_unavailable`, *Create* disabled (`add_enabled_ui(t.creatable, …)`). **Done.** |
| R6 | `Plan::write_into`: folders (`folders_for(kind, backend)`), the entry form, and when the `backend` flag is set (default on, Q5) `backend/src/health.cbl` (`BACKEND_STARTER`, `starter_backend`, exported `HEALTH`). **Partly:** the authentication programs WASM-003 R22 asks for come from WASM-009, which is plan-only (`9af4dcc4`: "NOT implemented"). §4 says where they hook in. |
| R7 | Footer: *Create from Template…* (`DialogAction.from_template` → `create::clone_project`) and *Recent Projects ▾* (`ui_prefs::{load_recent_projects, record_recent_project}`, rows `RecentRow`). **Done; Q6's "to confirm" open** (§4). |
| R8, R9, R18 | `ProjectMeta.kind`, `ProjectFiles.backend` and the optional tables `#[serde(default)]`; a Desktop manifest gains nothing; `project_kind_tests::an_old_manifest_loads_as_desktop_and_loading_leaves_the_file_alone`; `rcrun package` reads `[files] backend`. **Done.** T15 adds no manifest field (the extension is a function of the kind). |
| R10, AC3 | IDE and the `create_project` tool call the same `create::plan`; `rcrun`'s creation path **is** that tool, served by `rcrun mcp` (`cobolt-cli/src/main.rs`, `cobolt_mcp::serve`). Tests `what_the_tools_create_is_what_the_ide_saves_and_the_two_routes_make_one_tree` and `the_default_request_is_the_golden_desktop_manifest` (`project_model.rs`). **Done;** T15 extends the tree comparison to the new extensions. |
| R11, R12, AC4 | `Category::{WebForms, MobileForms, Backend}`, `top_for(kind)`, `forms_for(kind)`, `is_forms()`, `root_subdir()` (`webforms`, `mobileforms`, `backend`), `category_of_path` (`project_model.rs`). Desktop shows `Category::TOP` unchanged. **Done.** |
| R13 | `panels/project.rs` (`[+]`, folder, icon, label), `tool_exec.rs`, `target_select.rs`, `panels/walkthrough.rs`, `agent_kit/ide_host.rs`, `cobolt-project-tools/src/tools/list.rs`. **Done.** |
| R14 | Run / Build / Preview by kind (`app.rs` `is_web_project()`, `panels/web_preview.rs`) **done**; **toolbox by kind open → T14** (§4). |
| R15 | No new executable or window mode; nothing in this plan adds one. |
| R16, AC6 | 160 `pk_*` fields × six tables in `i18n.rs`; `every_project_kind_string_is_in_six_languages`. T14/T15 add strings only if a message is shown (§4). |
| Q2 | **T15**: `.wcfrm` / `.mcfrm` (§3, §4). **Open work.** |

## 2. Affected crates / files

Already changed by T1–T13 (kept as listed in tasks.md). New for this revision:

**T15 — form extensions**
- `crates/cobolt-compiler/src/project_kind.rs` — `ProjectKind::form_extension()` (`"cfrm"`, `"wcfrm"`, `"mcfrm"`),
  `FORM_EXTENSIONS` and `is_form_extension(&str)` (case-insensitive). The single predicate every site below calls.
- `crates/cobolt-project-tools/src/create.rs` — `plan()`: the entry form becomes `{forms_folder}/main-form.{form_extension}`
  (the two `format!(… ".cfrm")` in `plan`, including the error text for `main`).
- `crates/cobolt-project-tools/src/host.rs:114` (`FileList::Forms` by extension), `tools/check.rs:142`, `tools/validate.rs:26`,
  `tools/regenerate.rs:53`, `tools/run.rs:183`, `tools/render.rs:60` (refuse a non-form), `web/devserver.rs:280` (MIME),
  `reference.rs:436` (the form-format prose names the three extensions).
- `crates/cobolt-ide/src/project_model.rs:1402` (`FileKind::Form` by extension), `app.rs:7112` and `app.rs:14744` (open/save
  dialog filters and the default file name use the open project's `form_extension()`), `panels/project.rs:722`,
  `panels/designer.rs:16413`, `panels/forms_list.rs:206`, `agent_kit/ide_host.rs:354`.
- `crates/cobolt-web-bundle/src/secrets.rs:259` — the Web build's secret scan **must** scan `.wcfrm`/`.mcfrm` (otherwise a key
  typed into a web form escapes WASM-002 R7 / WASM-013 R7); `src/devserver.rs:88` (MIME).
- `crates/cobolt-cli/src/form_gui.rs:79` — `rcrun run-form` accepts the three extensions.
- `crates/cobolt-project-tools/src/content.rs` — plugin rules and the `web-kinds` skill (lines ~452, ~491, ~1173–1188,
  ~1303) name the extension next to the folder.
- `crates/cobolt-compiler/src/lib.rs` — System KB document *Project kinds* (the table at ~6186–6187 gains an extension
  column); then `cargo run -p cobolt-ide --example build_chunked_kb` and the regenerated `assets/knowledge/chunked.data`.
- Unchanged on purpose: the compiler's build path and `main_form_guard.rs` read `files.forms` from the manifest and use
  `file_stem()`, so they are extension-agnostic; corpus tests that walk `examples/*/forms` for `.cfrm` (desktop examples).

**T14 — toolbox by kind**
- `crates/cobolt-forms/src/web_class.rs` (new; no feature gate, because the player uses `cobolt-forms` with
  `default-features = false`) — `Class`, `Row`, `TABLE`, `class_of(&ControlType)` moved verbatim from
  `crates/cobolt-web-player/src/kinds.rs`, plus `available_on_web(&ControlType) -> bool` (`class != D`).
- `crates/cobolt-web-player/src/kinds.rs` — re-exports them; keeps `PlayerKind`, `player_kind_of` and the `tree::Kind`
  mapping (player-only). Its AC1 test (table ⇔ `ControlType::ALL`) moves with the table.
- `crates/cobolt-ide/src/panels/toolbox.rs` — the toolbox takes the project kind and, for Web/Mobile, omits controls for which
  `available_on_web` is false.
- `crates/cobolt-ide/src/app.rs` — passes the kind where the toolbox is drawn.

Not touched: `README.md`, anything under `docs/` (2.x.x rule).

## 3. Data / model changes

```toml
[project]
kind = "web"            # absent for desktop
[web]    style, multi_memory, isolation, backend_url        [web.browsers] chromium firefox safari ios_webview android_webview
[mobile] platform, device, orientation, min_os, style, multi_memory, backend_url
[backend] exports = [...]
[desktop] window_style, ui_framework, high_dpi, gpu_acceleration   # recorded; no run-time effect yet
[files] forms = ["webforms/main-form.wcfrm"]  backend = [...]
```

- All fields `#[serde(default)]`; the kind is immutable after creation (Q7, decided).
- **Form files (Q2, decided):** desktop `.cfrm`, web `.wcfrm`, mobile `.mcfrm`. The XML inside is the same model
  (`cobolt_forms::load_form` / `save_form`, WASM-004 R1); only the name differs. They stay in one `files.forms` list, in the
  kind's folder (Q8). No new manifest field: the extension follows from `[project] kind`.
- **A `.cfrm` already inside a Web or Mobile project** (made by 2.0.x before T15, or copied from a desktop project) — see §4 D-T15.
- `.cfrm` files of desktop projects are not affected in any way (R11, R18).

## 4. Key decisions & alternatives

Kept from the first plan:
- **The shared types live in `cobolt-compiler`.** Rejected: IDE-private types plus a tool-side copy.
- **One `plan()` for the IDE and the tool.** AC3 true by construction. Rejected: a second generator.
- **Descriptors are `&'static` data; the IDE maps ids to `Tr` fields.** OS-version choices are proper names, untranslated.
- **Typed table wrappers when the manifest is written** (byte identity with `save_project`).
- **Backend is not a kind** (D13): `backend/{src,COPYBOOKS}`; indexed files and SQL connections stay project-level.
- **Fixed-width rows, not `egui::Grid`**, in the dialog and the settings section.
- **Target platform for Desktop shows the host, disabled** (`new_project.rs` `host_platform()`; Q4 decided).
- **Backend scaffold on by default with an option to clear it** (Q5 decided; option `backend`).

New:
- **D-T15a — the extension is a function of the kind, asked through one predicate.** Every site in §2 calls
  `is_form_extension` / `form_extension()` instead of comparing with `"cfrm"`. Why: about 20 literal sites outside tests today; a fourth kind or a
  missed site would otherwise reappear as "the file is not a form" in one tool only. Rejected: adding `| "wcfrm" | "mcfrm"` at
  each site (the same drift that made the manifest shape exist three times).
- **D-T15b — an existing `.cfrm` in a Web/Mobile project is kept, loaded and listed; it is never renamed silently.**
  **Confirmed by the operator 2026-10-22** ("Manter + avisar + Renomear"; the spec's Q2 does not say; tasks.md T15 left it to `/plan`). Behaviour: the
  tree, loaders and tools accept it as a form of that project; Check reports one **warning** per such file ("`webforms/x.cfrm`
  is a desktop form file name in a Web project; rename it to `x.wcfrm`"), and the tree offers an explicit *Rename to .wcfrm*
  action that renames the file, its `files.forms` entry and nothing else. New forms are always written with the kind's
  extension. Why: *user code is sacred* and R18 (opening never rewrites a file) rule out an automatic rename on load; refusing
  the file would lock the developer out of their own form. Rejected: convert on open (rewrites files on open); refuse
  (breaks every 2.0.x Web project made before T15). The warning text is a `Tr` field ×6 (R16) if it surfaces in the IDE;
  the Check message from `cobolt-project-tools` follows that crate's existing English diagnostics.
- **D-T15c — a `.wcfrm`/`.mcfrm` in a project of another kind** (copied by hand) is listed and loadable — the model is one —
  and Check warns the same way. No refusal: WASM-004 R5's Check of class D controls is what guards a copied form's content.
- **D-T14a — the class table moves to `cobolt-forms`.** WASM-004 R2 requires *one* table that the toolbox, Check, the KB and the
  agent kit read; the player's workspace cannot be a dependency of the IDE (separate `[workspace]`, wasm-bindgen pin), and a
  copy would be a second table. `cobolt-forms` is already a dependency of both and owns `ControlType`. Rejected: an IDE copy
  (drifts); a new crate (one more workspace member for ~70 rows). This touches WASM-004's file; its plan already lists "the
  IDE half (toolbox, …)" as not done, so the move is the shared first step, and WASM-004's Check (R5) and Properties (R6)
  reuse it.
- **D-T14b — what the filter hides today.** In `TABLE` no built-in `ControlType` is class D; `class_of` answers D only for
  `Custom` plug-in controls and any type missing from the table (ToolBar is A with a D *operation*). So on the day T14 lands
  the visible effect is that plug-in controls disappear from a Web/Mobile toolbox. That is the rule as specified, not a
  defect; the test reports the hidden count so a later reclassification is visible. The series-only controls
  (Hyperlink/WebLink, Authentication, Canvas2D/3D, the E objects) are not `ControlType` variants yet and are not offered by
  the toolbox at all — adding them is WASM-004/006/009/018 work, not this spec's.
- **R6 authentication programs — hook, not stub.** When WASM-009 lands, its generator adds its files to `Plan.files` and
  `Plan.backend` inside `create::plan` (the only creation path), behind the same `backend` flag. Nothing is generated here
  ahead of WASM-009: a placeholder program would be a guess at its interface.
- **Q6 — working assumption, not a decision.** The operator said *Recent Projects* "does not exist yet"; the code has a
  Recent pane pinned at the foot of the left list and a *Recent Projects ▾* footer menu, both fed by
  `ui_prefs::load_recent_projects`. Assumption: these **are** the R7 entry, kept as they are. If the operator says otherwise,
  the change is confined to `panels/new_project.rs` (and its `pk_recent*` strings); nothing else depends on it.

## 5. Risks & mitigations

- *A window that grows with its text* → explicit size, one grip, frame tests (kept).
- *Two writers of a manifest disagreeing* → typed wrappers + the 27-request byte-identity test (kept; T15 adds the extension).
- *An old project rewritten merely by opening it* → `#[serde(default)]`; load test leaves the file alone (kept). T15 must
  not rename on open (D-T15b); a test opens a Web project holding `webforms/main-form.cfrm` and checks bytes and names.
- *A missed `"cfrm"` site* → after T15 a test greps nothing; instead each tool is driven with a `.wcfrm` project
  (`check`, `validate`, `regenerate`, `run`, `render`, `list_files`) and must treat it as a form. The audit list in §2 came
  from a search of `"cfrm"` literals on 2026-10-22; the implementer re-runs that search before closing T15.
- *The Web secret scan skipping web forms* (security) → a test in `cobolt-web-bundle` plants a fake key in a `.wcfrm` and
  expects the scan to report it.
- *Moving the class table breaks the player build* → the player is built with `--manifest-path`; T14 runs its native tests
  (`cargo test --manifest-path crates/cobolt-web-player/Cargo.toml`) and its dependency guard after the move.
- *Disk exhaustion while testing* → `df` first, `CARGO_INCREMENTAL=0`, only the touched crates, `check` before `test`.

## 6. Test strategy (results are printed by the tests)

Kept (T1–T13): `cobolt-compiler` `project_kind::tests`, `project_kind_build_tests`; `cobolt-project-tools` `templates`,
`create`, `tests/kinds.rs`; `cobolt-ide` `panels::new_project`, `project_model::project_kind_tests`, `ui_prefs`,
`every_project_kind_string_is_in_six_languages` (`i18n.rs`).

New:
- **T15** — `project_kind::tests`: `form_extension()` per kind, `is_form_extension` case-insensitive and refusing `cfrmx`.
  `create` tests: a Web plan writes `webforms/main-form.wcfrm`, a Mobile plan `mobileforms/main-form.mcfrm`, Desktop
  unchanged (`the_default_request_is_the_golden_desktop_manifest` stays byte-identical). `tests/kinds.rs`: each tool accepts
  the new extensions (prints one line per tool). `project_model`: the byte-identity test covers the new names; a Web project
  with a legacy `.cfrm` opens unmodified and is listed under WebForms; Check prints the D-T15b warning once.
  `cobolt-web-bundle`: the secret scan finds a planted key in a `.wcfrm`. System KB freshness test
  (`prebuilt_chunked_kb_matches_the_published_documentation`) green after regeneration.
- **T14** — `cobolt-forms` `web_class` tests: the table ⇔ `ControlType::ALL` test moved from the player; `available_on_web`
  false for a `Custom` plug-in. `toolbox` test: a Desktop project offers today's full catalogue (count printed, equal to
  before); a Web and a Mobile project offer the catalogue minus class D (both counts and the hidden names printed). Player
  native tests still green.
- **AC7 sweep** — `cargo test -p cobolt-ide --bin cobolt-ide`, `-p cobolt-project-tools`, `-p cobolt-compiler --lib`,
  `-p cobolt-forms --features render`, `-p cobolt-web-bundle`, with `--no-fail-fast`, every `test result` line read; the
  three known reds of T13 (`docs_embed::every_document_ships_in_every_language`, two live-index `external_crates_service`
  tests) reported again, not counted as new.
- Manual / visual: not driven (operator rule: never drive the IDE window). The operator looks at the dialog, the tree and a
  Web project's toolbox.

## 7. Steering compliance

- [x] i18n: every IDE string a `Tr` field ×6 (existing 160 `pk_*`; any T14/T15 IDE message added the same way).
- [x] Generated-code banner and regenerate-on-action unchanged: `regenerate` handles `.wcfrm`/`.mcfrm` like `.cfrm`.
- [x] Interpreter/binary parity: nothing here changes the interpreter; `rcrun run-form` (`form_gui.rs`) accepts the new
  extensions in the same change as the IDE.
- [x] System KB: the *Project kinds* document gains the extensions; `chunked.data` regenerated in the same change.
- [x] Window never resizes itself; one grip owns the dialog's size.
- [x] `README.md` and `docs/` untouched (2.x.x); the guide chapter is written at release (WASM-028).
- [x] Feature on the `features-2x` lineage; the implementing commit bumps `z` of 2.x.x and adds a CHANGELOG entry.
- [x] PRIME DIRECTIVE: Rust only; no new dependency (T14 moves code between existing crates).
- [x] User code is sacred: no form file renamed or rewritten without the developer's explicit action (D-T15b).
- [x] COBOL identifiers English; no "cobolt" in user-facing text.

## 8. Not finished, plainly

1. **T15 — `.wcfrm` / `.mcfrm`** (Q2, decided): planned in §2–§4, not implemented; the code writes `.cfrm` for every kind.
2. **T14 — toolbox by kind** (R14 second half): planned (D-T14a/b); depends on moving WASM-004's table into `cobolt-forms`.
3. **R6 — authentication programs of the backend scaffold**: wait for WASM-009's implementation; hook point fixed (§4).
4. **`[desktop]` options** are recorded and consumed by nothing yet (unchanged).
5. **Desktop target platform** shows the host only until cross-building exists (Q4, decided).

## 9. Open questions carried

- **Q6 (to confirm):** is the existing Recent pane / *Recent Projects ▾* menu the operator's *Recent Projects*? Working
  assumption: yes (§4).
- **D-T15b (proposal for the operator):** a `.cfrm` inside a Web/Mobile project is kept, warned about and renamed only on
  the developer's explicit action.
- **Spec text to tidy at the next `/clarify`** (not edited here — this run writes only plan.md): R7 ends with a stray
  duplicated line ("and shall not be needed to create a project."); §1 *What exists today* (checked 2026-10-19) still says
  there is no `ProjectKind` and no template list, which T1–T13 have since added.
