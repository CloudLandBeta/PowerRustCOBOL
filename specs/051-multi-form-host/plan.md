<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Multi-form host

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-08-15

## 1. Approach

The supervisor already speaks multi-form fluently — handles, parent/child
(`HandleInfo.caller`), modal reply holding, deepest-first cascades, Async
detachment, Waiting vetoes, per-handle published properties — and its eight
lifecycle tests prove it (`form_host.rs:594-856`). The interpreter is already
multi-instance clean: every channel and handle is a per-instance field, and
`test_super_receiver.rs:266/305` already wires **two** interpreters to one
supervisor with a cloned `form_host_tx` and distinct handles. What does not
exist is (a) a second **program** in the compiled binary, (b) a second
**interpreter thread** in any host, and (c) anything that **renders** a second
form. The design adds exactly those three, and the new designer/method surface
on top.

The build (R1/R2): `build_core` today parses only `sources[0]` and serialises
one `program.bin`; every non-main form's generated `.cbl` is never even read
(`lib.rs:705`, `resolve_main` doc at `lib.rs:313`). We add a parse loop over
the other forms' generated programs, stage each as
`assets/programs/<ID>.bin`, and emit a `PROGRAMS: &[(&str, &[u8])]` table in
the generated `main.rs` mirroring the existing `FORMS` table
(`forms_entries`, `lib.rs:1568`) with a by-id loader mirroring
`load_program()` (`lib.rs:1880`) and the `THEMES.iter().find` lookup pattern
(`lib.rs:1654`). The main path — `program.bin`, `FORMS[0]`, `run_form_app` —
is untouched, which is R2's byte-for-byte-behaviour guarantee.

The host (R3–R9, R13, R15): extract the per-form state of `FormHost` (today
singular fields: `controls`, `state`, `anim`, `hovered`, `form_size`,
channels, lifecycle one-shots — `host.rs:451-576`) into a **`FormBody`**
struct, and parametrise the frame path (`ui_impl` → `render_form`) over
`&mut FormBody`. `FormHost` keeps the root body plus
`children: Vec<ChildWindow { handle, body, viewport_id, builder }>`. The
`HostAction::SpawnWindow` arm (the stub at `host.rs:609`) becomes real: it
asks a new **`FormSource`** provider (in `FormHostConfig`) for
`(Form, Program)` by form id, builds a channel set, spawns the interpreter
thread through a shared `spawn_form_interpreter` helper (deduplicating
`form_gui.rs:299-324` and the compiled template `lib.rs:1769-1802`), and
pushes a `ChildWindow`. Each frame, children are re-declared with
`ctx.show_viewport_immediate` using the IDE's proven idiom (stable
`ViewportId::from_hash_of(handle)`, `close_requested` → `supervisor.try_close`
— `app.rs:11465-11529`), which the spec-037 spike
(`crates/cobolt-cli/examples/mv_spike.rs`) validated on this egui version,
including modal blocking via `ui.disable()` on the caller. A failed spawn
(unknown id, parse error) replies with an error and raises a visible runtime
error instead of a silent release (R15).

The shell (R10–R12, R16–R20): `ShellApp` gains an **occupant registry**
aligned with the existing `NavChain` (`shell.rs:238-297`, which already
models push/replace/pop residency). The pane renders the *active* occupant's
`FormBody` through the same `pane_frame` path; `open-form:` clicks swap
occupants — `onDeactivate` to the outgoing, `onActivate` to the incoming,
teardown + `onDestroy` when not preserved; a preserved occupant's
interpreter thread stays alive, parked on its event channel (Decision D2).
The two new standalone actions are two new arms in the shell's click match
(`shell.rs:1195`): they call `supervisor.handle_request(OpenForm { caller:
ROOT_HANDLE, sync, modal: sync, … })` directly (the shell owns the host owns
the supervisor — no thread to block); Sync modality is enforced by disabling
shell input while `modal_children_of(W0)` is non-empty (R19, spike-proven).
Embedded occupants register with the supervisor under a new
`Kind::Embedded` so the handle/property surface is uniform (Decision D3);
window-only methods on an embedded handle return the existing
"no method" style error.

The interpreter (R21–R24): two new arms in `exec_method` **before** the
property-access fallback (`interpreter.rs:7469`) — gated on the receiver
control's seeded class being `SideMenu` — send `FormRequest::OpenForm` with
`caller: ROOT_HANDLE` (the shell is the parent, R18/R21) and block on the
reply for Sync exactly as `me::"OpenFormSync"` does (`interpreter.rs:1280`).
The `RETURNING`-handle gate (`starts_with("OPENFORM")`,
`interpreter.rs:2549`) becomes a `method_returns_window_handle()` helper
covering both families. The inline `::` member path reaches the same arms
(`eval_member` → `exec_member_method` → `exec_method`). Semantic: two new
rows in the signature table (`resolver.rs:394`) and two new names in the
load-path gate (`resolver.rs:465`), flipped to `allows_standalone`.

Designer + validation (R16–R17, R25–R26): `MenuEditorModal` learns whether it
is editing a SideMenu (set where the modal opens, `app.rs:14057`); the Action
combo shows 5 options for SideMenu, 3 for MenuBar. Encodings
`open-standalone-sync:<stem>` / `open-standalone-async:<stem>` follow the
`open-form:` pattern through `action_type_of` / `sync_bufs_from_selection` /
the two write sites. `forms_under` returns both capability flags (the
`form-format` sniff extended: missing = Standalone; unreadable = both, never
hiding a form on a guess), and the Target picker filters per action, retiring
the orange "⚠ Standalone" advisory. `cobolt-forms::menu` gains
`open_standalone_target()` and `validate_menu_targets` reports both mismatch
kinds; the compiler's sole call site (`lib.rs:781`) emits the mirrored error
message.

## 2. Affected crates / files

- `crates/cobolt-runtime/src/form_host.rs` — `Kind::Embedded`; open-embedded
  entry; no change to the existing lifecycle rules or tests.
- `crates/cobolt-runtime/src/interpreter.rs` — `exec_method` arms for
  `OPENSTANDALONEFORMSYNC/ASYNC`; `method_returns_window_handle()` widening
  the `RETURNING` gate at 2549.
- `crates/cobolt-form-host/src/host.rs` — `FormBody` extraction;
  `FormSource` + `spawn_form_interpreter` in `FormHostConfig`; real
  `SpawnWindow`; child-viewport loop; closed-handle **fan-out**
  (`Vec<mpsc::Sender<String>>`, one per interpreter — mpsc is
  single-consumer); modal input disable.
- `crates/cobolt-form-host/src/shell.rs` — occupant registry over `NavChain`;
  `open-form:` real swap; two standalone click arms; breadcrumb feed;
  activate/deactivate/destroy dispatch.
- `crates/cobolt-form-host/src/seeding.rs` — unchanged (already per-form).
- `crates/cobolt-compiler/src/lib.rs` — parse loop for non-main form
  programs; `assets/programs/<ID>.bin` staging; `PROGRAMS` table + loader in
  `generate_main_rs`; template interpreter-spawn via the shared helper;
  `validate_menu_targets` call extended (both kinds); KB:
  `control_method_docs` SideMenu arm, `methods_reference_doc` SideMenu
  section, `control_purpose` SideMenu string, 037/shell prose updates
  (3307-3322, 3346-3359).
- `crates/cobolt-semantic/src/resolver.rs` — signature-table rows;
  load-path gate names + `allows_standalone` flip for the new pair.
- `crates/cobolt-forms/src/menu.rs` — `open_standalone_target()`;
  `validate_menu_targets` both-kinds (error carries the kind).
- `crates/cobolt-cli/src/form_gui.rs` — disk-backed `FormSource` (project
  scan + generated-`.cbl` parse) for `run-form` and the shell.
- `crates/cobolt-ide/src/panels/designer.rs` — modal `is_side_menu`; combo
  ×5/×3; encode/decode; `forms_under` capability flags; picker filtering;
  warn-styling removal.
- `crates/cobolt-ide/src/app.rs` — pass the control type when opening the
  menu editor; IDE Run Form wiring of the same `FormSource`/spawn helper.
- `crates/cobolt-ide/src/i18n.rs` — `menu_action_open_standalone_sync` /
  `menu_action_open_standalone_async` ×6.
- `docs/developers-guide-en.md` — three doors, methods, filtering, Sync
  modality; caveats (timers off-pane, indexed-file sharing, EXEC RUST scope).
- `assets/knowledge/chunked.data` — rebuilt.
- `CHANGELOG.md`, `crates/cobolt-ide/src/version.rs` — feature entry, `z` bump.

## 3. Data / model changes

- **Menu YAML:** two new `action` encodings (`open-standalone-sync:<stem>`,
  `open-standalone-async:<stem>`). Backward compatible: old IDE builds
  classify unknown actions as "event" (`action_type_of` fallback) and the old
  runtime's wildcard arm dispatches them to the COBOL handler — no crash, no
  data loss. The YAML HMAC covers them like any action string.
- **Compiled binary layout:** new staged `assets/programs/<ID>.bin` files and
  a `PROGRAMS` static. `program.bin`/`FORMS`/`THEMES` unchanged. A
  single-form project emits an empty `PROGRAMS` table (the `THEMES`-empty
  precedent), keeping the template-compile test green.
- **`.cfrm`:** no schema change. `FormFormat` already round-trips.
- **Supervisor:** `Kind` gains `Embedded`; `HandleInfo` unchanged otherwise.
- No config or on-disk migration anywhere.

## 4. Key decisions & alternatives

- **D1 (settles spec Q1) — EXEC RUST bridge is per form instance.** The
  recon overturned the spec's lean: the "one object bridge" was never a
  process-global — each block already runs against the *calling*
  interpreter's `env`/`objects`/`bridge` (`exec_rust.rs:179-186`), and the
  registry is a per-interpreter field of stateless `fn` pointers
  (`interpreter.rs:658`). With per-form interpreters each form's blocks see
  that form's state, which *matches R3/R4 isolation*. The genuinely
  process-wide piece — the generated `cobolt_windows` viewport registry and
  `PAINTER_READY` (`compiler/exec_rust.rs:139-140`) — stays shared: one
  window-id namespace per process. — **Rejected:** a process-wide shared
  `ObjectRegistry`, which would require locking the interpreters' `&mut`
  state across threads and quietly break the isolation model; 041's wording
  is updated to "one bridge per program instance" instead.
- **D2 (settles spec Q2) — preserved occupants: thread alive, timers
  paused.** The interpreter thread parks on its event channel; state, files
  and storage stay warm. Timers are *render-driven* (`CT::Timer` fires from
  `render_form`, clocked in egui memory — `render.rs:6377`), so an off-pane
  form does not tick, and resumes on reactivation. Documented as a guide
  caveat. — **Rejected:** hidden rendering to keep ticks (surprising
  background work, wasted frames); moving timers off the render path (a
  cross-cutting refactor with its own spec).
- **D3 (settles spec Q3) — uniform handle surface for embedded occupants.**
  `Kind::Embedded` registers occupants in the supervisor, so
  `super::X`, `GetProperty`/`SetProperty`, `SUPERHANDLE` and
  `PublishFormProps` behave identically on both surfaces; window-only
  methods error with the existing style. — **Rejected:** a parallel
  pane-only mechanism (two property surfaces to document and test).
- **D4 — `FormBody` extraction + provider closures, not a second host.** One
  render path serves root, children and occupants; `FormSource` +
  `spawn_form_interpreter` give the three glues (compiled, rcrun, IDE) the
  same spawn behaviour — that *is* R13. — **Rejected:** a separate
  child-window host struct (drift, double maintenance — the exact disease
  042 cured).
- **D5 — per-form programs as separate staged `.bin`s,** not one merged
  archive: preserves R2's untouched main path, `include_bytes!` stays
  cache-friendly per form, and the empty-table degenerate case is the proven
  `THEMES` pattern.
- **D6 — singleton semantics unchanged.** Only the main form is a singleton
  (`open_form`, `form_host.rs:300`); a non-main form may open twice — the
  supervisor's tests assert this (R11 there), and spec R9's "where the
  singleton rule applies" is written for exactly that.
- **D7 — menu-Sync modality via supervisor state, not thread blocking.** The
  shell's UI thread must never block; `modal_children_of(ROOT)` non-empty →
  shell chrome and pane input disabled (`ui.disable()`, spike-proven).
- **D8 — copybook posture unchanged.** The compiler path performs no COPY
  expansion today (only `rcrun run/check` do); per-form programs are parsed
  exactly as the main program is. Generated form programs contain no COPY,
  so nothing regresses; noted, not fixed, here.

## 5. Risks & mitigations

- **Risk: the `FormBody` refactor destabilises the single-form paths** (fx
  playback, pane observability, backdrop, one-shot lifecycle flags all live
  on `FormHost`). → Mechanical extraction with no behaviour change as its own
  commit-sized step; the 042 parity suite plus the paint/elegance baselines
  gate it; children are added *after* the extraction proves green.
- **Risk: IDE Run Form parity (R13)** — the IDE hosts running forms in its
  own viewport loop, not through `FormHost::run`. → The shared pieces
  (`FormSource`, `spawn_form_interpreter`, `ChildWindow` application) are
  free functions the IDE calls from its loop; parity is asserted by the AC8
  scenario driven headlessly through the shared engine. If the IDE seam turns
  out deeper than expected, the IDE wiring lands as the last phase without
  blocking the compiled/rcrun doors.
- **Risk: modal input leaks** (shortcuts, wheel, close buttons while a modal
  child is up). → Single choke point: one `modal_active()` check disables the
  shell chrome, the pane body and root-window close routing; a headless test
  drives a click during modality and asserts nothing lands.
- **Risk: closed-handle fan-out misses an interpreter** → the fan-out is a
  tiny owned struct with a unit test: N receivers, one close, N deliveries.
- **Risk: R2 regression in the compiled template.** → Existing template tests
  (`generated_binary_source_actually_compiles`,
  `the_build_puts_the_main_form_first`, effect-baking) run unchanged; new
  assertions cover the `PROGRAMS` table without touching old ones.
- **Risk: two live forms opening the same INDEXED file** — engines coordinate
  in-process per instance, not across instances; concurrent writers could
  conflict. → Out of scope to change (spec R5: "nothing newly shared");
  documented as a ⚠️ caveat in the guide (one owner form, or pass data via
  published properties).
- **Risk: stale generated `.cbl` on `rcrun build`** (the IDE regenerates
  before building; bare `rcrun build` trusts disk — true today for the main
  form, now true for all forms). → Noted in the guide; unchanged posture.

## 6. Test strategy

New tests report quantified, human-readable results (counts, scenario names)
per the standing rule; no invented measurements.

- **cobolt-runtime** — supervisor: existing 8 lifecycle tests unchanged
  (guard); new: `Kind::Embedded` registration + window-only-method error.
  Interpreter dispatch: `INVOKE <SideMenu> "OpenStandAloneFormAsync"` lands a
  `FormRequest::OpenForm{caller: "W0", sync:false}` on a test channel and
  registers the `RETURNING` handle; the Sync variant blocks until the test
  thread replies; a control *property* named `OpenStandAloneFormSync` on a
  non-SideMenu control still resolves as a property (the fallback is
  preserved — AC11's last clause). Extend `test_open_form_invoke.rs` with the
  new pair through a real `FormSupervisor`.
- **cobolt-semantic** — mirror `test_open_form_signature.rs` (arity/type
  errors for the space form, comma form exempt) and
  `test_form_load_path.rs` (standalone gate: Embedded target → error, `Both`
  → clean, data-item target → clean) for the new names.
- **cobolt-forms** — `menu.rs`: `open_standalone_target` parsing (both
  prefixes, trim, empty → None); `validate_menu_targets` both kinds (embedded
  action × Standalone form, standalone action × Embedded form, `Both` passes
  everywhere, unknown skipped).
- **cobolt-form-host** — closed fan-out unit test (N receivers × 1 close);
  shell click arms: the two standalone actions produce supervisor requests
  with the right sync/modal flags (extending the `shell.rs:2170+` harness);
  occupant swap headless: event order
  `onDeactivate(out) → onActivate(in)`, `onDestroy` iff not preserved,
  NavChain/breadcrumb segments after push/replace/pop; modality: input
  disabled while a modal child lives.
- **cobolt-compiler** — template text: `PROGRAMS` table entries
  (`include_bytes!("../assets/programs/<ID>.bin")`), empty-table form for
  single-form projects, loader present; the compile-the-generated-crate test
  stays green; KB freshness test
  (`prebuilt_chunked_kb_matches_the_published_documentation`) green after the
  chunked rebuild.
- **cobolt-ide** — `action_type_of`/encode-decode round-trips for the new
  encodings; combo option-count gating (SideMenu 5, MenuBar 3); picker
  filtering table-test over the four format cases (Standalone / Embedded /
  Both / unreadable).
- **Manual/visual (operator):** two-form shell app — sidebar `open-form:`
  swap with preserve on/off; `open-standalone-async` window while the shell
  stays live; Sync blocking the shell; `INVOKE SideMenu-1 …` from a button
  handler; the same app via `rcrun run-form` and IDE Run Form (AC8); a
  single-form project behaving exactly as 1.61.49 (AC4).

## 7. Steering compliance

- [ ] i18n: 2 new `Tr` fields (`menu_action_open_standalone_sync/_async`) in
      all 6 languages; no other new UI strings planned.
- [ ] Generated-code banner + regenerate-on-action contract preserved (the
      compiler only *reads more* generated files; IDE regeneration already
      covers all forms before Build/Run/Debug/Check).
- [ ] English dev guide updated (three doors, methods, filtering, caveats);
      translations untouched.
- [ ] Fix vs feature: **feature** → `z` bump in `version.rs`, `CHANGELOG.md`
      entry, f=96 announcement after merge (operator-gated, post-push).
- [ ] System KB: SideMenu methods tables + 037/shell prose updated in the
      same change; `chunked.data` rebuilt; freshness test green.
- [ ] No "cobolt" in user-facing text; COBOL identifiers/source stay English.
