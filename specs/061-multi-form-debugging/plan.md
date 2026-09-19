# Plan — Spec 061: debugging an application, not a form

- **Status:** draft → **approved?**
- **Spec:** ./spec.md   **Date:** 2026-09-19
- **Classification:** fix (operator ruling) — `fixes` branch, `z` bumps

## 1. Approach

The spec's ten obstacles reduce to one structural question: **where does
debuggee identity live?** Answer it in the *router* rather than in the
protocol, and most of the rest follows cheaply.

Today `stdio_debug_wiring()` hands one `(Receiver<DebugCmd>, Sender<DebugEvent>,
Breakpoints, DebugUserScope)` tuple to one interpreter. It is replaced by a
**`DebugRouter`** that can hand out that tuple **once per debuggee**, remembers
which handle each tuple belongs to, and does the addressing on both sides:

- **Inbound** — one stdin reader, as now, but each parsed command is delivered
  to **that debuggee's own channel**. No `Receiver` is ever shared, so a
  command cannot reach the wrong interpreter (**R10 by construction**, not by
  care).
- **Outbound** — each debuggee's events are stamped with its handle by a small
  forwarding thread and serialised onto the one stdout stream (**R2**).

The consequence worth stating plainly: **`cobolt-runtime` does not change.**
`DebugCmd`, `DebugEvent` and `Interpreter::attach_debug_channels` are already
per-interpreter; what was missing was a second caller and someone to keep the
names straight. The protocol change (§3) lives in `cobolt-form-host`, where the
`@DBG` link already lives, and in the IDE that reads it.

Requirements map as follows. **R1** — `FormHostConfig` gains a one-shot debug
hook that `build_form_instance` calls for every child (window, modal child and
pane occupant all pass through it). **R2** — the wire envelope. **R3/R4** — the
panel gains per-source state so switching listings is non-destructive. **R5** —
the router gives each debuggee its own `Breakpoints` set, so line numbers stop
colliding. **R6** — commands carry a target handle. **R7** — already true; see
§5 Risks. **R8** — `PAUSED` untouched (Q1). **R9** — `target: None` means the
root, and a bare `DebugEvent` is still accepted, so a single-form session is
bit-for-bit the session it is today.

## 2. Affected crates / files

- `crates/cobolt-form-host/src/debug_link.rs` — **the change's centre.** New
  `DebugRouter`, `DebugWire` envelope, `RemoteDebugMsg`; `stdio_debug_wiring()`
  keeps working as the single-debuggee shorthand.
- `crates/cobolt-form-host/src/host.rs` — `FormHostConfig::child_debug`
  (new field, `None` everywhere today); `build_form_instance` attaches it to
  the child interpreter it spawns (`host.rs:3387-3417`).
- `crates/cobolt-cli/src/form_gui.rs` — build the router under `--debug`,
  register the root, pass `child_debug`.
- `crates/cobolt-compiler/src/lib.rs` — `run_form_app`: the same two changes
  (`interpreter-binary-parity`; AC7).
- `crates/cobolt-ide/src/panels/debugger.rs` — per-source state and a
  non-destructive `show_source`; `apply_event` takes the handle.
- `crates/cobolt-ide/src/app.rs` — handle → generated `.cbl` index; per-handle
  breakpoints and user scope; switch the panel on a stop from a new handle;
  route each external run's `@DBG` lines separately (fixes the latent merge).
- `crates/cobolt-ide/src/i18n.rs` — only if §6 adds a user-facing string;
  the breadcrumb already names the file being shown.
- `docs/developers-guide-en.md` — §19 *Debugging* gains the multi-form
  passage; its five translations are deleted (GOLDEN RULE #8).
- **Unchanged on purpose:** `crates/cobolt-runtime/**` (§1), and
  `crates/cobolt-ide/src/runner.rs` — the in-IDE `DebugRunner` debugs a single
  hand-written `.cbl` with no forms, so it stays a one-debuggee path.

## 3. Data / model changes

### 3.1 The wire (new, in `debug_link.rs`)

```rust
/// One `@DBG` line, outbound. The router stamps the handle; the interpreter
/// knows nothing about it.
#[derive(Serialize, Deserialize)]
pub enum DebugWire {
    /// A debuggee has attached: `handle` is the supervisor handle
    /// (`W0` = root, `W1`… = children), `form` its form-object name.
    Attached { handle: String, form: String },
    /// Its program ended or its window closed.
    Detached { handle: String },
    /// Anything the interpreter emitted.
    Event { handle: String, event: DebugEvent },
}

/// One `@DBG` line, inbound. `target: None` means the ROOT debuggee, so every
/// line the IDE sends today keeps its meaning.
#[derive(Serialize, Deserialize)]
pub struct RemoteDebugMsg {
    #[serde(default)]
    pub target: Option<String>,
    pub cmd: RemoteDebugCmd,
}
```

`DebugCmd`, `DebugEvent`, `RemoteDebugCmd`, `StopReason` are **unchanged**.

### 3.2 The router (new)

```rust
pub struct DebugRouter { /* handle -> cmd_tx, breakpoints, scope; out_tx */ }

impl DebugRouter {
    /// Take stdin/stdout for the process and start the reader + pump.
    pub fn stdio() -> Arc<Self>;
    /// One debuggee. Returns what `attach_debug_channels` +
    /// `set_debug_user_scope` need, and announces `Attached` upstream.
    pub fn register(&self, handle: &str, form: &str) -> DebugWiring;
    /// Its interpreter has finished.
    pub fn unregister(&self, handle: &str);
}
```

Each `register` mints a **fresh** `Breakpoints` and `DebugUserScope` — that is
**R5**: two forms can hold line 42 without seeing each other's.

### 3.3 Host config

```rust
pub child_debug: Option<Arc<dyn Fn(&str, &str) -> Option<DebugWiring> + Send + Sync>>,
```
Called once per child with `(handle, form_object)`. `None` ⇒ exactly today's
behaviour.

### 3.4 IDE state

- `DebuggerPanel.sources: HashMap<String /*path*/, SourceEntry>` — per file:
  `lines`, `breakpoints`, folds (`hidden`, `expanded_runs`), last scrolled
  line. Session-level state — the dock, watches, font size, `only_user_code`
  — stays where it is and is **not** touched by a switch (**R4**).
- `app.debug_forms: HashMap<String /*handle*/, DebugForm>` — the generated
  `.cbl` path, the form's `.cfrm`, its user-line set, and the breakpoint set
  last sent to it. Filled on `Attached`, dropped on `Detached`.

**Compatibility.** No on-disk format changes. The `@DBG` protocol is
process-internal between the IDE and a debuggee it spawned itself, and both
ship together; the `target: None` default and the bare-`DebugEvent` fallback
exist so a half-updated pair degrades to single-form debugging rather than
failing.

## 4. Key decisions & alternatives

- **Identity in the router, not the protocol.** *Why:* every `DebugEvent`
  send site in `interpreter.rs` would otherwise need a handle threaded through
  it, and `DebugCmd` would need one on the way in — a change across four
  crates to express something the router already knows. *Rejected:*
  `Sender<(String, DebugEvent)>` in the interpreter.
- **A new `child_debug` field, not `child_interpreter_setup`.** *Why:* the
  existing hook is `Fn(&mut Interpreter)`, `Arc`-shared and callable N times;
  debug wiring is a **one-shot move** of a `Receiver` and needs the handle to
  mint it. *Rejected:* overloading the existing hook, which would hand the
  same `Receiver` to two children — the exact silent-race R10 forbids.
- **Per-debuggee channels, never a shared `Receiver`.** *Why:* R10. An
  `Arc<Mutex<Receiver>>` would let whichever interpreter is blocked in `recv`
  first eat a `Continue` meant for the other, and nothing would report it.
- **One listing that switches (Q3), with per-file state cached.** *Why:* the
  spec's "no new debugger UI" non-goal, and `set_source`'s destructiveness is
  the actual defect — caching fixes it without a tab strip.
- **The IDE resolves handle → file once, on `Attached`.** *Why:* the reverse
  `.cfrm` → `.cbl` map is a linear recompute (`app.rs:6540`, `:6548`); doing
  it per stop would put a scan in the step loop.
- **`PAUSED` stays process-wide.** Operator ruling (Q1). Not revisited here.

## 5. Risks & mitigations

- **Regressing the single-form debugger that works today.** → `target: None`
  = root; a bare `DebugEvent` line still parses; every existing debugger test
  stays green *unchanged* (AC6) and is the gate, not a new test.
- **Two debuggees stopped at once.** Real, not theoretical:
  `event_flows_while_blocked` (`host.rs:1387`) lets `onTick` through while the
  application is paused, so a Timer in form B can reach a breakpoint while
  form A is stopped. → The IDE keeps a set of stopped handles, shows the most
  recent, and when the shown one resumes switches to any other still stopped.
  Without this, a stop nobody is showing looks like a hang.
- **R7 regressing by accident.** Per-form event delivery is what makes the
  wait-state behaviour true, and nothing here should touch it. → A host test
  asserts a click on child B's body reaches B's channel and not the root's —
  a guard for behaviour that exists, so the change cannot quietly break it.
- **A handle reused after a form closes.** → `Detached` drops the entry; the
  supervisor mints monotonic handles (`W1`, `W2`, …), so reuse is not a case.
- **Breakpoint traffic per keystroke.** → Send a handle's set only when *that*
  handle's set changed, keyed off the per-form `sent` snapshot already in
  `debug_sent_breakpoints`.
- **A stop in a form with no open designer and no generated file on disk.** →
  Resolve through the project's forms list as `form_for_generated` does;
  failing that, report the stop with the handle and the form name and keep the
  listing put, rather than showing the wrong file's line.

## 6. Test strategy

**`cobolt-form-host` (the router — unit, no process, no stdio):** construct a
router over in-memory pipes; register `W0` and `W1`; assert
(a) a command targeted at `W1` arrives on `W1`'s receiver and **nothing**
arrives on `W0`'s; (b) `target: None` goes to the root; (c) events from each
debuggee come out stamped with their handle; (d) each `register` yields a
**distinct** `Breakpoints` — inserting 42 into `W1`'s leaves `W0`'s empty;
(e) `unregister` emits `Detached`.

**`cobolt-form-host` (the guard):** a click forwarded to child B's body
reaches B's `ev_tx` and not the root's (protects R7).

**`cobolt-ide` panel:** `show_source` switches listings and back, preserving
each file's breakpoints and folds, and leaving the dock and watch list intact;
a `Stopped` for a handle whose file is not the one displayed does **not**
write its line into the displayed listing.

**`cobolt-ide` app:** handle → generated `.cbl` resolution; the per-handle
breakpoint payload contains only that file's lines; `Detached` drops the entry.

**Manual (operator), against PowerDemo3's `call-form-demo` / `called-form-demo`:**
AC1–AC5 and AC8 as written in the spec — most importantly AC2 (same line number
in both forms, only the owner stops) and AC4 (Continue out of the child lands
back in the caller with the panel following).

**Sweeps:** `cobolt-form-host`, `cobolt-runtime`, `cobolt-ide --bin`, and
`cobolt-forms --features render`, all `--no-fail-fast`, reading every
`test result:` line.

## 7. Steering compliance

- [ ] i18n: any new UI string in all 6 languages — expected to be **none**
      (the breadcrumb already names the file); a "stopped in another form"
      notice, if added, is a `Tr` key ×6.
- [ ] Generated-code banner + regenerate-on-action contract preserved — this
      changes who watches generated code run, never what is generated.
- [ ] English dev guide updated (§19), its five translations deleted.
- [ ] Fix → `z` bumps per change, `CHANGELOG.md` entry each time.
- [ ] No "cobolt" in user-facing text; COBOL identifiers, paragraph names and
      generated source stay English.

## 8. Steps, in order

Each step is independently buildable and testable; each is its own commit.

1. **S1 — the router.** `DebugWire`, `RemoteDebugMsg`, `DebugRouter` in
   `debug_link.rs`, with `stdio_debug_wiring()` reimplemented on top of it as
   the single-debuggee shorthand. Router tests (§6). Nothing else changes yet;
   the session behaves exactly as before.
2. **S2 — children can be debugged.** `FormHostConfig::child_debug`;
   `build_form_instance` attaches when it is `Some`. All existing call sites
   pass `None`. The R7 guard test.
3. **S3 — `rcrun run-form --debug` uses the router** and passes `child_debug`.
   From here a child form's interpreter is attached — but the IDE still shows
   one listing, so a stop in a child reports against the wrong file. Not
   shippable alone; S3+S5+S6+S7 land together or S3 waits.
4. **S4 — the compiled binary** does the same (AC7).
5. **S5 — the IDE reads the envelope**, per external run, and stops merging
   every debugged run's events into one panel.
6. **S6 — the panel switches listings** without losing anything
   (`show_source`, per-source state).
7. **S7 — the IDE addresses commands**, breakpoints and user scope by handle,
   and follows a stop into a new form.
8. **S8 — more than one stopped debuggee** (the `onTick` case).
9. **S9 — docs**: guide §19, translations deleted.

**Shipping order note.** S1, S2 and S4 are inert on their own and can land
singly. S3 and S5–S7 are one behavioural change and must land together, or the
debugger reports stops against the wrong listing between them.
