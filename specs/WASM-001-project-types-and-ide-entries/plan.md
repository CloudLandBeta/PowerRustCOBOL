# Plan — Project types, the New Project selector and the IDE entries

- **Status:** implemented on `worktree-agent-a97ba65c65571c2df` (branch cut from `features-2x`); see §8 for what is not finished
- **Spec:** ./spec.md   **Date:** 2026-10-20

## 1. Approach

One shared model, one shared plan, three readers. Everything that is *about* a kind lives in `cobolt-compiler` (the
manifest types), everything that is *about creating one* lives in `cobolt-project-tools` (the template data and the
creation plan), and the IDE draws what those two hold. Nothing in the dialog's drawing code names a template.

| Requirement | Where it is met |
|---|---|
| R1, R17 | `cobolt-ide/src/panels/new_project.rs` — a modal with an explicit `size`, one grip in its own foreground `Area` as the only writer, a left list with search and a right pane. Tests `the_window_does_not_grow_while_nobody_touches_it` (frames over six languages × the panes; prints the count) and `the_size_only_changes_by_a_drag_and_stops_at_its_limits`. |
| R2, AC5 | `cobolt-project-tools/src/templates.rs` — `TemplateDescriptor` is `&'static` data: id, kind, mobile `(platform, device)`, `creatable`, options (`Flag` / `Choice`) with defaults. A test registers a fake descriptor and sees it listed with its options. |
| R3, R4 | `new_project.rs` collects name, version, entry, location, description, locale; `create::plan` turns the request and the descriptor into a `Plan` (manifest text + files). |
| R5 | `creatable: false` entries are listed with a localised "not yet available" note; *Create* is disabled for them. Creatable in 2.0: Desktop, Web, Android phone/tablet, iPhone, iPad. |
| R6 | `Plan::write_into` lays down the folders, the sample main form with a button handler (`GREET_HANDLER`) and, unless cleared, `backend/src/health.cbl` (`HEALTH`, exported). The main-form designation is sealed with `host::record_in_manifest`. |
| R7 | *Recent Projects* is `ui_prefs::{load,record}_recent_project(s)`; *Create from Template…* is "copy a saved project" (`create::clone_project`), not a second list. |
| R8, R9 | `cobolt-compiler/src/project_kind.rs`: `ProjectKind`, `MobilePlatform`, `DeviceClass`, `Orientation`, `ApplicationStyle`, `Isolation`, `BrowserMatrix`, `WebSettings`, `MobileSettings`, `BackendSettings`, `DesktopSettings`. `ProjectMeta.kind`, `ProjectFiles.backend` and the optional tables are `#[serde(default)]`; a Desktop project writes **nothing** new. `ManifestView` carries the kind to every reader; `rcrun package` reads `[files] backend`. |
| R10, AC3 | `create_project` (tool) and the IDE call the **same** `create::plan`; test `what_the_tools_create_is_what_the_ide_saves_and_the_two_routes_make_one_tree` (in `project_model.rs`) makes 27 requests both ways and compares trees byte for byte, then re-saves through the IDE's `save_project` and compares again; `the_default_request_is_the_golden_desktop_manifest` pins the unchanged desktop manifest `new_project.toml`. |
| R11, R12, AC4 | `project_model.rs`: `Category::{WebForms, MobileForms, Backend}`, `Category::top_for(kind)`, `forms_for(kind)`, `is_forms()`, `root_subdir()` (`webforms`, `mobileforms`, `backend`). Desktop shows `Category::TOP` exactly as before. |
| R13 | `panels/project.rs` (the tree: `[+]`, folder, icon, label), `tool_exec.rs` and `grace_session.rs` (Grace's tree disambiguation), `panels/walkthrough.rs`, `agent_kit/ide_host.rs` and `tools/list.rs` (the agent's listing: `kind`, `forms_folder`, `backend`). |
| R14 | Toolbar Preview / Run / Build do what WASM-002 and WASM-008 say for the kind (`app.rs`: `is_web_project`, `start_web`). **The toolbox filter by kind is not done** — see §8. |
| R16 | 163 `Tr` fields (`pk_*`) in all six tables; `project_kind_strings_wasm001` fails on an empty or untranslated one. |
| R18 | `load_project` of an old manifest does not rewrite it; `project_kind_tests::an_old_manifest_loads_as_desktop_and_loading_leaves_the_file_alone`. |

## 2. Affected crates / files

- `crates/cobolt-compiler/src/{project_kind.rs, lib.rs}` — shared types; `CompilerError::NotADesktopProject` (a native build refuses a Web/Mobile project instead of producing a binary of nothing).
- `crates/cobolt-project-tools/src/{templates.rs, create.rs, host.rs, content.rs}`, `tools/{mod,list,check,build,register}.rs` — data, plan, tool surface.
- `crates/cobolt-ide/src/{project_model.rs, app.rs, i18n.rs, ui_prefs.rs, target_select.rs, tool_exec.rs, grace_session.rs}`, `panels/{new_project, project, target_picker, walkthrough, settings_form, web_settings}.rs`.
- `crates/cobolt-cli/src/main.rs` — `rcrun package` includes `[files] backend`.
- Not touched, by rule: `version.rs`, `CHANGELOG.md`, `README.md`, `docs/`.

## 3. Data / model changes

```toml
[project]
kind = "web"            # absent for desktop
[web]    style, multi_memory, isolation, backend_url        [web.browsers] chromium firefox safari ios_webview android_webview
[mobile] platform, device, orientation, min_os, style, multi_memory, backend_url
[backend] exports = [...]
[desktop] window_style, ui_framework, high_dpi, gpu_acceleration   # recorded; no run-time effect yet
[files] backend = [...]
```

All fields `#[serde(default)]`; unknown tables are preserved by the tool-side edits (`toml_edit`). The kind is immutable
after creation (Q7). Forms stay `.cfrm` (Q2) in one `files.forms` list, told apart by their folder (Q8).

## 4. Key decisions & alternatives

- **The shared types live in `cobolt-compiler`, not in the IDE.** Why: the IDE, `build_project`, `ManifestView` and the tools all read the manifest; one definition cannot drift. Rejected: IDE-private types plus a tool-side copy (that is how byte-identity tests start failing).
- **One `plan()` for the IDE and the tool.** Why: AC3 is then true by construction and the test only has to prove the re-save. Rejected: a second generator in the tool.
- **Descriptors are `&'static` data and the IDE maps ids to `Tr` fields** (`template_text`, `option_label`, `choice_label`). Why: R2/R16 together — the data is language-free, the words are translated. OS-version choices ("Android 14 (API 34)") are proper names and stay untranslated.
- **Typed table wrappers when the manifest is written** (`DesktopTable`, `WebTable`, …). Why: a bare `toml::Value` sorts keys alphabetically, and the IDE's `save_project` writes struct order; byte-identity needs the same serializer for both.
- **`Backend` is not a kind** (D13): it lives at `backend/{src,COPYBOOKS}`; indexed files and SQL connections stay project-level.
- **Fixed-width rows instead of `egui::Grid`** in the settings section and the dialog: a Grid takes its column widths from the widest label, which is what made windows grow with the language.

## 5. Risks & mitigations

- *A window that grows with its text* (this project's most repeated defect) → explicit size, one grip, `resizable(false).fixed_size`, every row of fixed height with truncated text, and the frame tests above asserting the rectangle never changes (the run prints its frame count).
- *Two writers of a manifest disagreeing* → typed wrappers + the 27-request byte-identity test.
- *An old project rewritten merely by opening it* → `#[serde(default)]` everywhere; the load test leaves the file alone and checks it.
- *Disk exhaustion while testing* (the volume ran under 1 GiB several times) → `CARGO_INCREMENTAL=0`, only the crates touched, `check` before `test`.

## 6. Test strategy (results are printed by the tests, GOLDEN RULE #7)

- `cobolt-compiler` `project_kind::tests` (10) and `project_kind_build_tests`: defaults, round trips, module-variant rule, backend-address rule, native build refuses a non-Desktop project.
- `cobolt-project-tools`: `templates` (data, ids, fake descriptor), `create` (plans per template, clone, nested-destination refusal), `tests/kinds.rs` (5: tool surface, byte identity across 27 requests), `layout_rule`, `form_design_rule`, `tools`, `http`.
- `cobolt-ide`: `panels::new_project` (19), `project_model::project_kind_tests` (8), `ui_prefs` (recents), `i18n::project_kind_strings_wasm001`.
- Manual / visual: not driven (the operator rule: never drive the IDE window). The operator looks at the dialog and the tree.

## 7. Steering compliance

- [x] i18n: every string a `Tr` field in six languages.
- [x] COBOL identifiers stay English; generated COBOL is English.
- [x] Window never resizes itself; one grip is the only writer of the dialog's size.
- [x] `version.rs`, `CHANGELOG.md`, `README.md`, `docs/` untouched (2.x.x rule); the Developer's Guide chapter for kinds is written at release.
- [x] Feature, `features-2x` lineage; the lead bumps `z`.
- [x] No dependency added to the IDE for this; `toml` (workspace) and `toml_edit` 0.22 (already pulled by `toml` 0.8) added to `cobolt-project-tools`.

## 8. Not finished, plainly

1. **R14, toolbox by kind.** The toolbox still offers the desktop catalogue in a Web/Mobile project. The filter needs the class table of WASM-004 (which control is A/B/C/D on the web); it does not exist yet, and inventing it here would be a guess.
2. **R6, the full backend scaffold.** `HEALTH` is created and exported; the authentication programs of WASM-003 R22 / WASM-009 are not generated (they depend on the Authentication control).
3. **`[desktop]` options** (window style, UI framework, High DPI, GPU) are recorded in the manifest and have no run-time effect; nothing consumes them yet.
4. **Target platform (Q4)** for Desktop shows the host and is not a choice (cross-building does not exist).
5. **AC7 sweep** — see tasks.md T13 for the exact test runs and their results.
