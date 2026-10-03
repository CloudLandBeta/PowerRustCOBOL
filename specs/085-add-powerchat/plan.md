# Plan — Spec 085: Add PowerChat to an application

## Facts established (research, 2026-10-03)

**Child windows** (`crates/cobolt-form-host/src/host.rs`):
- `OpenForm` → supervisor `open_form` → `HostAction::SpawnWindow` →
  `spawn_child` → `build_form_instance` → `ChildWindow { handle, body, … }`,
  drawn each frame by `update_children` through `show_viewport_immediate`.
- A SideMenu inside a child window is an ordinary control: an item click
  raises `onMenuItemClick` for that form only; the item's **action is
  ignored** (`render.rs` `CT::SideMenu`). No chain, no ContentPane.
- The pane is **per host, not per window**: one `occupants` map, one
  `active_occupant`, one `pane_chrome` / `pane_band` / `last_occupant_rect` /
  `pending_crumb_detail`; `ensure_occupant` opens with caller `ROOT_HANDLE`;
  modal, overlay and blur helpers, `SetBreadcrumbDetail`, `CloseWindow`,
  `SetFormProperty`, `CallProcedure` and `live_modal_caller_viewport` look at
  the root and its occupants only.
- The pane transform (strip the SideMenu, slide by the rail, footer ids,
  narrowed size) is applied in `FormHost::new` for the root only.
- Menu sidecars: rcrun registers only the root form's; the built binary
  registers all, keyed by control id alone (ids collide across forms).
- `ShellApp` (`shell.rs`) owns `Shell`, `NavChain`, the click processing,
  the window resize on fold and the persisted fold state.

**PowerChat** (`examples/PowerChat`):
- Branding is design-time data (form titles, SideMenu `HeaderImage` /
  `HeaderIcon`, the robot picture, flags) plus a few text rows (WELCOME,
  NOT-RAG-FILE) and the `application="PowerChat"` export literal.
- AI: `models.idx` (entries), `settings.idx` (`AGENT-n-ENTRY`, `LANG`,
  `KB-LOCATION`), keys in the application's key store
  (`settings/model-keys.dat`, per application folder); `PC-HAND-MODELS`
  registers the entries in-process with `MODEL-SET`, and AGENT-1..3 ask by
  `ModelEntry`. The manifest `[ai]` section never reaches a running
  application.
- Data paths come from `POWERCHAT_DATA` (default `data`).

## Decisions

- **D1 — the pane becomes per window.** A `Pane` struct (occupants, active
  occupant, chrome, band, last rect, pending detail) owned by its window:
  the root host keeps one; a child window whose form carries a SideMenu gets
  one inside a `WindowShell` (Shell + NavChain + Pane + side-menu id).
  Occupants of a window are opened with that window's handle as caller, so
  `super::` binds to the window's form. This is the design the existing
  code would have had if windows had been shells from the start; it keeps one
  code path for the root shell and child shells.
- **D2 — one click processor.** `ShellApp::process_menu_clicks` becomes a
  function over a `WindowShell` and the host, used by the root shell and by
  every child shell. `close-application` in a child closes that window.
- **D3 — menu source.** rcrun registers every form's sidecar menus the way
  the built binary does, and the registry is keyed by form + control id so
  two forms' `SideMenu-1` never collide.
- **D4 — host AI settings = the application's AI settings.** The AI
  configuration belongs to the application, not to the chat window: the
  embedded PowerChat stores model entries and agent choices in the host's
  data folder (`data/ai-models.idx`, `data/ai-settings.idx`) and keys in the
  host's key store, and its provider / model / agent screens are the host's
  AI settings screens (a Settings ▸ AI group in the host's menu). Any other
  AgentObject in the host can then ask by `ModelEntry` with the same entries.
- **D5 — host branding.** `add_powerchat` rewrites the copied forms' titles,
  the SideMenu header image/icon and the welcome picture to the host's (main
  form title, project icon), and the brand-bearing text rows use the
  application's name, read at run time from the main form's title
  (`super::` chain / `me::Title`) rather than a literal.
- **D6 — PowerChat's own data** (conversations, topics, documents, prompts)
  lives under `data/powerchat/` (`POWERCHAT_DATA` default when embedded).

## Phases

**Phase A — the child-window shell (product, R1–R5).** Fix-shaped: the rail
is drawn but its navigation is dead. All three hosts.
**Phase B — `add_powerchat` (R6–R12).** The tool, the embedded PowerChat
pack, branding (R13).
**Phase C — host AI settings (R14).** PowerChat's AI storage moves to the
application's; its setup screens become the host's AI settings.

## Verification

- Phase A: headless tests in `host.rs`/`shell.rs` modelled on
  `open_form_swaps_occupants_with_preserve_and_breadcrumb` and
  `spawn_runs_a_child_to_completion_and_releases_it`; a `run_form` scenario
  (AC1); the built-binary template test (AC2).
- Phase B: `add_powerchat` on a `create_project` project → `check` clean, one
  main form, no form theme, Assistant item (AC3); collision refused (AC4).
- Phase C: an embedded PowerChat's model entries are the application's —
  an AgentObject on a host form asks through an entry PowerChat registered.
- Every phase: the full `cobolt-form-host`, `cobolt-cli`,
  `cobolt-project-tools`, `cobolt-forms --features render` and
  `cobolt-ide --bin` suites, every `test result:` line read.
