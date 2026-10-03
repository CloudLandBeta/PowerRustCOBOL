# Tasks — Spec 085: Add PowerChat to an application

One commit per task, each bumping `z` with a CHANGELOG entry.

## Phase A — the child-window shell (R1–R5)

- [x] **T1 — The pane per window.** Move occupants, the active occupant, the
  pane chrome/band, the last occupant rect and the pending crumb detail into a
  `Pane` owned by its window (root: `FormHost`; child: its `WindowShell`).
  Every occupant lookup searches every pane and answers with the owning
  window. Behaviour unchanged for the root shell. *Verify:* the whole
  form-host suite unchanged.
- [x] **T2 — One click processor.** `ShellApp`'s menu-click, breadcrumb,
  reset and fold logic over a `WindowShell` + the host, so a child can use it.
  *Verify:* the shell tests unchanged.
- [x] **T3 — A child window runs its SideMenu as a shell.** `spawn_child`
  gives a SideMenu form a `WindowShell` (pane transform, mounted menu); the
  child viewport draws the shell; menu clicks load forms into that window's
  ContentPane with the window as caller; Home, breadcrumb, close.
  *Verify:* AC1 (headless).
- [x] **T4 — Menu sidecars for every form.** rcrun registers all forms'
  menus; the registry is keyed by form + control id. *Verify:* two forms
  with `SideMenu-1` keep their own menus.
- [x] **T5 — All three hosts.** rcrun, embedded children and the built
  binary; `run_form` drives a child shell. *Verify:* AC2.
- [x] **T6 — Docs.** Guide chapter 22: a side-menu form opened as a window is
  a shell of its own.

## Phase B — `add_powerchat` (R6–R13)

- [x] **T7 — The embedded PowerChat pack** compiled in from
  `examples/PowerChat` (forms, menu, flags), data under `data/powerchat/`.
- [x] **T8 — The tool.** Copy, rename collisions refused, strip themes and
  main-form, host branding, Assistant menu item (modeless), register,
  regenerate, check. *Verify:* AC3, AC4.
- [x] **T9 — Docs and skill.** Guide coding-agent section; the golden rule
  for PowerChat names the tool.

## Phase C — host AI settings (R14)

- [x] **T10 — The application's AI settings.** *(Approved 2026-10-03: the runtime
  keeps the list, `settings/models.json`; PowerChat mirrors it.)* Embedded PowerChat stores
  model entries and agents in the host's data; its provider/model/agent
  screens become the host's Settings ▸ AI. *Verify:* a host AgentObject asks
  through an entry PowerChat registered.
