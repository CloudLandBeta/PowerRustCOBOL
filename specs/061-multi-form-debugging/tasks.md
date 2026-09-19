# Tasks — Debugging an application, not a form

- **Status:** in progress — T1–T9 done (T5/T6 absorbed by T3). As of 1.70.99
  the debugger follows the program into any form. Next: T10, then docs.
  **AC1–AC4 want the operator's manual walk** against PowerDemo3's
  caller/called pair — they are behaviour no unit test reaches.
- **Plan:** ./plan.md   **Date:** 2026-09-19
- **Branch:** `debug` (operator, 2026-09-19), worked in
  `.claude/worktrees/debug` because the shared checkout is on `features`

Ordered so the tree stays green after every task. There is **no red-by-design
stage** here: each task either adds inert machinery or completes a behaviour,
and the existing debugger tests are the gate throughout (AC6 — they must stay
green *unchanged*, never edited to fit).

> **Classification: fix** (spec §6, operator ruling) — `z` bump and a
> `CHANGELOG.md` entry per commit, `fixes` branch, never sharing a commit with
> a feature (GOLDEN RULE #5).
>
> **Standing rule for every task:** `cobolt-runtime` is not edited. If a task
> seems to need a change there, the design has drifted — stop and say so
> (plan §1). The one exception is a genuine bug found on the way, which is its
> own commit.
>
> ⚠️ **The rule was invoked once, at T3, and the answer was to edit it**
> (1.70.96). `attach_debug_channels` sets `StepMode::Into`, so a program
> joining a session **starts paused at its first statement** — right for the
> form the developer pressed Debug on, wrong for one the application opens
> while the session is live: the application would halt every time any form
> opened, at a line nobody asked about, with nobody able to resume it. The
> runtime could express "start paused" and had no way to say "start running",
> and a child form is the first caller that needs it, so
> `attach_debug_channels_running` was added — four lines, delegating to the
> existing one and setting `StepMode::Run`. The plan predicted no runtime
> change; the prediction was wrong, not the rule. Everything else in
> `cobolt-runtime` stays untouched.
>
> ⚠️ **And once more at T7, for a different reason: the envelopes moved
> there** (1.70.97). `DebugWire` and `RemoteDebugMsg` were defined in
> `debug_link.rs`, which is in a crate the IDE depends on only as a
> **dev-dependency** — a deliberate boundary (`cobolt-ide/Cargo.toml`: "The
> IDE itself is not a form host and takes no runtime dependency on it"). The
> file's own rule says where they belong: "The switch itself is part of the
> PROTOCOL, so it lives with the protocol — `cobolt_runtime::debugger` —
> where the IDE … and this crate can both see one spelling of the name."
> They are types, not behaviour; `debug_link` re-exports them.
>
> **Shipping constraint (plan §8):** T1–T4 and T6 are inert on their own and
> may be pushed singly. **T5 and T7–T11 are one behavioural change** — between
> them the debugger reports stops against the wrong listing, so they land
> together or T5 waits.

---

## Stage A — the router (plan S1)

- [x] **T1 — `DebugWire`, `RemoteDebugMsg`, `DebugRouter`** (R2, R5, R10)
  - Files: `crates/cobolt-form-host/src/debug_link.rs`
  - Do: add the three types from plan §3.1–3.2.
    `DebugRouter::new(out: mpsc::Sender<DebugWire>) -> Arc<Self>` holds
    `handle → (Sender<DebugCmd>, Breakpoints, DebugUserScope)`;
    `register(handle, form) -> DebugWiring` mints a **fresh** channel,
    `Breakpoints` and `DebugUserScope`, spawns the per-debuggee forwarding
    thread that stamps `DebugWire::Event { handle, .. }` into `out`, and
    announces `Attached`; `unregister(handle)` emits `Detached`;
    `dispatch(RemoteDebugMsg)` routes a command to `target` (or to
    `ROOT_HANDLE` when `None`) and applies `SetBreakpoints`/`SetUserScope` to
    **that** debuggee's shared sets.
    `DebugRouter::stdio()` is the thin wrapper that owns the stdin reader and
    the `println!("@DBG …")` pump — `new` + `dispatch` are what tests drive,
    so no test ever touches real stdio.
    Keep `stdio_debug_wiring()` working, reimplemented as
    `DebugRouter::stdio().register(ROOT_HANDLE, …)`, so today's callers compile
    unchanged.
    `PAUSED` is **not** touched (spec Q1).
  - Verify: `cargo build -p cobolt-form-host`; existing form-host suite green.

- [x] **T2 — Router tests** (R2, R5, R10)
  - Files: `crates/cobolt-form-host/src/debug_link.rs` (`#[cfg(test)]`)
  - Do: over `DebugRouter::new` + `dispatch`, assert
    (a) a command with `target: Some("W1")` arrives on `W1`'s receiver and
    **nothing** arrives on `W0`'s — the core R10 claim, so assert the *absence*
    with `try_recv().is_err()`, not just the presence;
    (b) `target: None` reaches the root;
    (c) each debuggee's events come out as `DebugWire::Event` carrying its own
    handle;
    (d) `register` yields **distinct** `Breakpoints`: `SetBreakpoints([42])`
    at `W1` leaves `W0`'s set empty (this is R5 in one assertion);
    (e) `register` emits `Attached`, `unregister` emits `Detached`.
  - Verify: `cargo test -p cobolt-form-host --no-fail-fast` — every
    `test result:` line read, not grepped for failures.

## Stage B — children can be debugged (plan S2)

- [x] **T3 — children join the session** (R1) — *done differently from the
      plan: no `FormHostConfig` field.* The hook would have had to be named at
      **35** construction sites, every one of them a test that has nothing to
      do with debugging. A debug session is already process-wide (one stdio
      link, one router, one `PAUSED`), so `build_form_instance` simply asks
      `debug_link::active_router()` and registers the child when the answer is
      `Some`. Same one-shot registration, same single call site covering child
      windows, modal children and pane occupants; a normal run pays one atomic
      load. **This also absorbs T5 and T6**: `rcrun` and the compiled binary
      already call `stdio_debug_wiring()`, which is what makes the router
      active, so neither host needs a line changed.
  - Files: `crates/cobolt-form-host/src/host.rs`
  - Do: add the field from plan §3.3, defaulted `None` at **every** existing
    construction site (`host.rs` tests, `shell.rs` tests, `form_gui.rs:611`,
    `compiler/src/lib.rs`). In `build_form_instance`'s interpreter thread
    (`host.rs:3387-3417`), after `set_super_form` and before
    `child_interpreter_setup`, call the hook once with `(handle, form_object)`
    and, when it returns `Some(wiring)`, `attach_debug_channels` +
    `set_debug_user_scope`. Both child paths — `spawn_child` (`host.rs:3280`)
    and `ensure_occupant` (`host.rs:2977`) — go through this one function, so
    a modal child, a plain child window and a SideMenu pane occupant are all
    covered by the single call.
  - Verify: `cargo test -p cobolt-form-host --no-fail-fast` green; with the
    hook `None` the spawned thread does exactly what it does today (diff the
    function and confirm the only new code is inside `if let Some`).

- [x] **T4 — Guard: event delivery stays per form** (R7)
  - Files: `crates/cobolt-form-host/src/host.rs` (`#[cfg(test)]`)
  - Do: assert that a click forwarded to a child body's `forward_interaction`
    reaches **that body's** `ev_tx` and that the root's channel stays empty.
    This protects behaviour that already exists (spec Q2) — it is written so
    this change cannot quietly break the wait-state requirement.
  - Verify: `cargo test -p cobolt-form-host --no-fail-fast`.

## Stage C — the hosts (plan S3, S4)

- [x] **T5 — `rcrun run-form --debug` uses the router** (R1, AC7 sibling) —
      **absorbed by T3, no code needed.** `rcrun` already calls
      `stdio_debug_wiring()` under `--debug`, and that is what makes the
      router active, so its children register themselves.
  - Files: `crates/cobolt-cli/src/form_gui.rs`
  - Do: under `--debug` build `DebugRouter::stdio()`, register the root as
    `ROOT_HANDLE` with the form object name, and pass
    `child_debug: Some(Arc::new(move |handle, form| Some(router.register(handle, form))))`.
    Without `--debug`, `child_debug` stays `None` and nothing changes.
  - Verify: `cargo build --release -p cobolt-cli`; a single-form debug session
    behaves exactly as before (AC6 by hand: start, stop at entry, step,
    continue, stop).
  - **Does not ship alone** — see the shipping constraint above.

- [x] **T6 — The compiled binary does the same** (AC7) — **absorbed by T3,
      no code needed**, for the same reason, which is also why the parity
      trap of wiring three hosts separately cannot arise here.
  - Files: `crates/cobolt-compiler/src/lib.rs` (`run_form_app`)
  - Do: mirror T5 exactly — the third host, and the one the developer ships
    (`interpreter-binary-parity`).
  - Verify: `cargo test -p cobolt-compiler --no-fail-fast` (the golden
    generator tests must not move); build one PowerDemo3 binary and confirm it
    still starts with no debugger attached.

## Stage D — the IDE follows the program (plan S5–S7)

- [x] **T7 — Read the envelope, per run** (R2; fixes the latent merge)
  - Files: `crates/cobolt-ide/src/app.rs` (the `@DBG` route, `app.rs:14638-14652`)
  - Do: parse `DebugWire`, falling back to a bare `DebugEvent` (treated as
    `ROOT_HANDLE`) so a half-updated pair degrades to single-form debugging.
    Carry the owning run with each event instead of merging every external
    run's events into one vec, and feed the panel only the events of the run
    that owns the session (`debug_owner_form`). This is the latent defect in
    plan §2 — reachable today the moment two debugged runs exist.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`; a
    single-form session still stops, steps and finishes.

- [x] **T8 — The panel switches listings without losing anything** (R3, R4)
  - Files: `crates/cobolt-ide/src/panels/debugger.rs`
  - Do: add `sources: HashMap<String, SourceEntry>` (plan §3.4) with
    `add_source(path, text, bps)` / `show_source(path)` / `has_source(path)`;
    `show_source` swaps the active entry's lines, breakpoints, folds and
    scroll memory in and out. `set_source` stays the session-start entry point
    and is reimplemented on top (clear, add, show). Session state — the dock,
    watches, font size, `only_user_code` — is **not** touched by a switch.
    `apply_event` gains the event's source (`Option<&str>`, `None` =
    session-level) and applies `Stopped`/`Paused` only when it matches the
    active listing: a guard, since T9 switches first.
  - Do (tests): switching away and back preserves each file's breakpoints and
    folds, and leaves the dock and watch list intact; a `Stopped` for a
    non-active source does **not** write its line into the displayed listing.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide -- debugger`.

- [x] **T9 — Address by handle; follow a stop into another form** (R3, R5, R6)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: add `debug_forms: HashMap<String, DebugForm>` (plan §3.4). At session
    attach, build the form-object → `.cfrm` map once from the project's forms
    (each form's `name`), so an `Attached` can be resolved without a scan. On
    `Attached`: resolve → `.cfrm` → generated `.cbl` (`generated_cbl_path`,
    `app.rs:6558`), compute that form's user lines from its own source map
    (as `app.rs:2362-2363` does for the launched form), and send that handle
    its own `SetBreakpoints` and `SetUserScope`. On `Stopped`: if the handle's
    file is not the one displayed, `add_source`/`show_source` it, then apply.
    Commands (Continue, the steps, Pause) go out as
    `RemoteDebugMsg { target: Some(shown_handle), … }`. Breakpoint sync
    becomes per handle, sending only when that handle's set changed. On
    `Detached`: drop the entry. A handle that cannot be resolved reports the
    stop with its handle and form name and **leaves the listing put** rather
    than showing the wrong file's line (plan §5).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`; then
    **AC1–AC4 by hand** against PowerDemo3's `call-form-demo` /
    `called-form-demo` — AC2 (same line number in both forms, only the owner
    stops) is the one that proves R5.

## Stage E — the edge the plan found (plan S8)

- [ ] **T10 — More than one debuggee stopped at once** (R6)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: `onTick` flows while the application is paused
    (`host.rs:1387`), so a Timer in form B can hit a breakpoint while form A
    is stopped. Keep the set of stopped handles; show the most recent; when
    the shown one resumes and another is still stopped, switch to it. Without
    this a stop nobody is displaying looks like a hang.
  - Verify: a unit test over the bookkeeping (two stops, resume one, the other
    becomes current); by hand, a form with a Timer plus a breakpoint in both
    forms.

## Stage F — finish

- [ ] **T11 — Docs** (spec §6)
  - Files: `docs/developers-guide-en.md` §19 *Debugging*; delete
    `developers-guide-{es,pt,fr,jp,cn}.md`
  - Do: a passage on debugging an application of several forms — a breakpoint
    in any form is reached, the panel follows the program, breakpoints belong
    to a file, and a stop stops the whole application. GOLDEN RULE #8: update
    the English canonical and delete its five translations; the two
    `docs_embed.rs` guards go red by design until the next minor regenerates
    them, and their `#[ignore]` is **not** re-added.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide -- docs_embed` — the
    language guard red, everything else green; `iconv -f UTF-8 -t UTF-8` on
    the edited file.

- [ ] **T12 — i18n** (spec §6)
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Do: expected to be **nothing** — the breadcrumb already names the file
    being shown. If T9's unresolvable-handle notice needs words, it is a `Tr`
    field in all six tables (EN, ES, PT, JA, ZH, FR).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide -- i18n` (no empty
    translations, nothing untranslated).

- [ ] **T13 — Finalize**
  - Do: full sweeps with `--no-fail-fast`, reading **every** `test result:`
    line: `cobolt-form-host`, `cobolt-runtime`, `cobolt-compiler`,
    `cobolt-ide --bin cobolt-ide`, `cobolt-forms --features render`. Known
    environmental reds listed explicitly (the docs-language guard by design
    after T11; `external_crates_service`'s live-network tests, which are
    re-run with `--test-threads=1` before being excused). Rebuild **both**
    release binaries — `-p cobolt-ide --bin cobolt-ide` *and* `-p cobolt-cli`
    in separate commands, or `rcrun` is silently skipped.
  - Verify: AC1–AC8 walked by the operator; `git diff` read, not skimmed.

## Done criteria

Every acceptance criterion in `spec.md` is checked; the pre-existing debugger
tests pass **unedited** (AC6); `cobolt-runtime` shows no diff; the Developer's
Guide carries the change and its five translations are gone; each commit is a
pure fix with a `z` bump and a changelog entry. Commit and push per the
operator's standing rules; merge to `main` only when asked.
