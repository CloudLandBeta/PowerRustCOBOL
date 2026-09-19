# Spec — Debugging an application, not a form

- **Status:** draft → **ready for `/plan`** (every question settled — see §7)
- **Folder:** specs/061-multi-form-debugging/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-19

## 1. Overview

Since spec 051 a PowerRustCOBOL application is **several forms** — a main form
that opens child windows, modal children and SideMenu pane occupants, each
running its own generated program in its own interpreter. The **debugger is
still a single-form debugger**: `rcrun run-form --debug` attaches the debug
channels to the *root* interpreter and to nothing else, so the moment execution
crosses into a second form it leaves the debugger's world entirely. A
breakpoint in a called form is never reached, its handlers cannot be stepped,
and the panel goes on showing the first form's listing with a line number that
now means nothing.

This spec describes what it takes for the debugger to follow the **application**
— to stop wherever the developer set a breakpoint, show the source of the form
that actually stopped, and let the developer drive whichever window they are
working in (operator, 2026-09-19: *"The debugger must jump to the code of the
other form and continue the debugging"*).

It is classified as a **fix** (operator ruling, 2026-09-19): the debugger
already claims to debug the developer's application, and an application has
been multi-form since 051 — a verb doing half of what it says, the same shape
as the cross-process-locking precedent in `CLAUDE.md`.

## 2. Goals / Non-goals

**Goals**

- A breakpoint set in **any** form's generated `.cbl` is reached, and the
  session stops there.
- When a stop lands in a form other than the one on screen, the panel **shows
  that form's listing**, positioned on the stopping line, without losing the
  session.
- Breakpoints belong to a **file**, so the same line number in two forms'
  programs does not cross-fire.
- The developer's debugger controls — Continue, the three steps, Pause — act on
  the form the panel is showing.
- Whichever form the developer is working in is the one producing events; the
  others sit in their wait state until focus returns.
- All three interpreter hosts behave identically (`interpreter-binary-parity`):
  `rcrun run-form --debug`, the in-IDE `DebugRunner`, and the **compiled
  binary**.

**Non-goals**

- **Debugging two forms at once.** One form is *current* at any moment; this is
  not concurrent multi-threaded debugging with N stopped contexts side by side.
- **A new debugger UI.** The panel, its dock, watches and gutter stay as they
  are; what changes is *which* listing they are pointed at.
- **Remote / cross-process debuggees.** The `@DBG` line protocol already
  anticipates adb/ssh; nothing here is about that.
- **Changing what a stop shows.** Frames, scopes, variables and the query
  service are unchanged.
- **Making `Only my code` cleverer.** Its rule is unchanged; only the question
  of *whose* user lines it means (see Q4).

## 3. User stories

- As a COBOL developer, I set a breakpoint inside the *called* form's button
  handler, run the caller under Debug, click the button — and the debugger
  stops on my line, showing the called form's code.
- As a COBOL developer, with the modal child stopped at my breakpoint, I press
  Step Over and step through **that** form's handler, then Continue and watch
  the caller pick up the result.
- As a COBOL developer, I click back to the caller window; it is the caller
  that now produces events, and when its own handler hits a breakpoint the
  debugger is showing the caller's code again — I did not have to tell it
  which form I moved to.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The system shall attach debug channels to every
  interpreter in a debugged run — the root form's and every form it opens
  (child window, modal child, SideMenu pane occupant).
- **R2 (ubiquitous):** Every `DebugEvent` the IDE receives shall identify which
  form emitted it, and every command the IDE sends shall identify which form it
  is for.
- **R3 (event):** When a program stops, the debugger panel shall display the
  generated source of the form that stopped, positioned on the stopping line.
- **R4 (event):** When that source is not the one already displayed, the panel
  shall switch listings while preserving the session's investigation dock,
  watches, fold state and the breakpoints of **both** files.
- **R5 (ubiquitous):** A breakpoint shall belong to the generated `.cbl` it was
  set in; a line number shall never stop a different form's program.
- **R6 (event):** When the developer presses Continue, a step or Pause, the
  command shall be delivered to the form the panel is currently showing, and to
  no other.
- **R7 (state):** While the developer works in form B, form B shall be the form
  producing events; form A shall remain in its wait state, receiving none, and
  shall resume when the developer returns to it. **This already holds** — see
  §7 Q2 — so R7 is a *preservation* requirement: the change must not break it.
- **R8 (state):** While any form is stopped, the **whole application** shall be
  stopped: every window stays visible and legible and takes no operator input,
  as it does today (operator ruling, 2026-09-19 — §7 Q1).
- **R9 (constraint):** The system shall not change single-form debugging: a
  project whose main form opens nothing behaves exactly as it does today.
- **R10 (constraint):** The system shall not leave any form's interpreter
  waiting on a command that was meant for another — a misrouted command must be
  impossible by construction, not merely unlikely.

## 5. Acceptance criteria

Measured against **PowerDemo3**, whose `call-form-demo` / `called-form-demo`
pair is exactly this shape.

- [ ] **AC1** — A breakpoint in `called-form-demo`'s generated `.cbl` is
      reached when the caller opens it; the panel shows
      `called-form-demo.cbl`, with the stop line highlighted.
- [ ] **AC2** — With a breakpoint on the *same line number* in both forms'
      generated programs, only the form that owns it stops.
- [ ] **AC3** — From that stop, Step Over advances inside the called form's
      handler; the variables pane shows the called form's data items.
- [ ] **AC4** — Continue from the called form's stop returns the result to the
      caller; a breakpoint in the caller's `onClick` after `OpenFormSync`
      returns is then reached, and the panel is showing the caller again.
- [ ] **AC5** — Pause, pressed while both windows are open and idle, stops the
      form that has focus, at that form's last executed line (per 1.70.89).
- [ ] **AC6** — A project with one form debugs exactly as it does today:
      the existing debugger tests stay green unchanged.
- [ ] **AC7** — The same session driven against a **compiled binary** behaves
      identically (parity with `rcrun run-form --debug`).
- [ ] **AC8** — Closing a form that is currently stopped ends that form's
      debuggee cleanly and leaves the session on the form that remains.

## 6. Constraints & steering check

- **i18n (6 languages):** any new user-facing string — a "showing form X"
  breadcrumb, a status line naming the current debuggee — is a `Tr` field in
  all six tables. COBOL identifiers, paragraph names and the generated source
  stay English.
- **Generated-code / regenerate contract:** unchanged. This does not alter what
  `cobolt-codegen` emits; it changes who is *watching* it run. The `.cfrm` →
  `.cbl` map is already computed (`app.rs:6558` `generated_cbl_path`) — it needs
  to become a real index rather than a linear recompute (`app.rs:6540`, `:6548`).
- **Docs (English guide):** §19 *Debugging* gains a passage on debugging an
  application of several forms; its five translations are deleted per GOLDEN
  RULE #8, and regenerated at the next minor.
- **System KB:** no control, property, method or event changes → no
  `chunked.data` rebuild.
- **Fix vs feature:** **fix** (operator ruling, 2026-09-19) — `fixes` branch,
  `z` bump, f=97 if ever announced.
- **`interpreter-binary-parity`:** three hosts wire debug independently and all
  three must land together —
  `cobolt-cli/src/form_gui.rs:402`, `cobolt-compiler/src/lib.rs:3240`,
  `cobolt-ide/src/runner.rs:673`.

## 7. Questions — all settled

- **Q1 — While form B is stopped, what does form A's window do with input?**
  **SETTLED (operator, 2026-09-19): the application is stopped, all of it —
  keep the current behaviour.** `debug_link::PAUSED` stays one process-wide
  `AtomicBool` (`debug_link.rs:59`), consulted at the same four render sites
  (`host.rs:3044`, `:3226`, `:3503`, `:4541`); every window goes inert on any
  stop, as it does today. Nothing in this change makes pause per-form, and
  §8's obstacle 6 is therefore **not** an obstacle — it is the specified
  behaviour.

- **Q2 — What is "the focused form", and how does focus reach the debug
  link?** **SETTLED: it does not have to, because the routing already exists.**
  Each form body owns its **own** event channel — `FormBody::ev_tx`
  (`host.rs:721`), created per instance in `build_form_instance`
  (`host.rs:3358`) and sent to only by that body's `forward_interaction`
  (`host.rs:1213`). A click on form B therefore reaches B's interpreter and no
  other, while A's interpreter stays blocked inside `COBOL-WAIT-EVENT` — which
  is precisely the wait state the requirement describes. R7 is satisfied by
  construction today.

  So the debugger does **not** track OS focus. It follows **whichever
  interpreter stops**, which is the same thing from the developer's chair: the
  form they are working in is the one running handlers, so it is the one that
  hits their breakpoints. A SideMenu pane occupant needs no special answer for
  the same reason — it has its own body and its own channel, whatever the OS
  thinks the focused window is. §8's obstacle 10 leaves the critical path
  entirely.

- **Q3 — One listing that switches, or a tab per form?** **SETTLED: one
  listing that switches**, per R4 and the "no new debugger UI" non-goal. The
  breadcrumb already names the file being shown, so which form is current stays
  visible; a tab strip can be revisited if reading two forms at once turns out
  to matter.

- **Q4 — Is `Only my code` per form?** **SETTLED: yes.** The toggle stays one
  switch the developer flips, but the *user-line set* it resolves against is
  the stopped form's own — computed from that form's source map, exactly as
  `app.rs:2388-2392` computes it for the launched form today. Anything else
  would make stepping in form B obey form A's line numbers, which is the same
  collision R5 removes from breakpoints.

- **Q5 — Do watches survive a switch?** **SETTLED: one watch list per session,
  and a watch that does not resolve in the current form shows as
  unavailable.** The panel already carries a per-watch error slot
  (`Watch::error`, cleared at every stop, `debugger.rs:1243-1246`), so this
  needs no new UI. Dropping watches on a switch would lose the developer's
  work; a second list per form would multiply state for no stated benefit.

- **Q6 — What identifies a form on the wire?** **SETTLED: the supervisor
  handle travels; the IDE resolves it once.** The handle (`ROOT_HANDLE` and
  the child handles) is what the host, the supervisor and `set_form_host`
  already key on, so it is free and unambiguous at the runtime end. It means
  nothing to the IDE, so each debuggee announces itself once as it attaches —
  handle plus form object name — and the IDE maps that to the form's generated
  `.cbl` through the existing `.cfrm` → `.cbl` rule (`app.rs:6558`), cached in
  the index §8's obstacle 9 calls for. Commands carry the handle back.

- **Q7 — Is the compiled binary in this change?** **SETTLED: yes**, per AC7 and
  the `interpreter-binary-parity` rule (operator ruling, 2026-09-07). Three
  hosts wire debug independently — `cobolt-cli/src/form_gui.rs:402`,
  `cobolt-compiler/src/lib.rs:3240`, `cobolt-ide/src/runner.rs:673` — and the
  binary is the one that ships. All three land together.

## 8. What the code says today

Verified 2026-09-19. These are the obstacles the plan must answer, not
suggestions for how to answer them.

1. **No identity anywhere in the protocol.** `DebugCmd` (`debugger.rs:29-55`),
   `DebugEvent` (`:183-225`) and `RemoteDebugCmd` (`:154-168`) carry no program,
   form or interpreter id. The only program-ish field is `DebugFrame::program`
   (`debug_session.rs:78-93`), a stack frame's `PROGRAM-ID`, read-only payload
   on a stop. R2 is therefore a protocol change on both directions of the wire.
2. **Child interpreters have no debug plumbing at all.** Both the child-window
   path (`host.rs:3280` `spawn_child`) and the pane-occupant path
   (`host.rs:2977` `ensure_occupant`) build through `build_form_instance`, whose
   interpreter thread (`host.rs:3387-3417`) calls `set_control_ids`,
   `set_input_channel`, `set_event_counter`, `set_form_host`, `set_super_form`,
   `seed_objects` — and never `attach_debug_channels`.
3. **The command receiver is moved, not shared.** `attach_debug_channels`
   takes `mpsc::Receiver<DebugCmd>` by value (`interpreter.rs:3377-3383`), and
   a `Receiver` is single-consumer and not `Clone`. Two debuggees need either a
   channel each plus a **router**, or a shared mutex receiver — and the latter
   silently races a `Continue` meant for form B into whichever interpreter is
   blocked in `recv` first. R10 exists because of this.
4. **stdin is claimed once.** `stdio_debug_wiring` locks it for the process
   (`debug_link.rs:108`), so the fan-out has to happen after that single reader,
   inside the process.
5. **Breakpoints and user scope are one flat shared set.**
   `Breakpoints = Arc<Mutex<HashSet<u32>>>` (`debugger.rs:276`) and
   `DebugUserScope` (`:335`) are process-wide bare line numbers — with several
   forms in one process, identical line numbers collide. R5 is a data-model
   change, not a filter.
6. **`PAUSED` is process-wide** (`debug_link.rs:59`) — and **stays that way**
   by ruling (Q1). Listed here because it is load-bearing, not because it is
   an obstacle: a stop freezes every window, which is the specified behaviour.
7. **The panel assumes one source for the whole session.** `set_source`
   (`debugger.rs:1145`) clears the dock, the folds and the breakpoint set, and
   is documented "at session start"; `apply_event` (`:1217`) writes any stop's
   line into `current_line` against whatever listing is loaded, with no check of
   whose stop it was. R4 needs a non-destructive switch that `set_source` cannot
   currently express.
8. **The IDE already merges every debug child's events into one panel.**
   `app.rs:14652` routes all external runs into one `dbg_events` vec and
   `:14688` feeds them all to `self.debugger.apply_event(ev)` with no filter on
   which run they came from. That is a latent defect today — reachable the
   moment two debugged runs exist — and R2 is what makes it fixable.
9. **The `.cfrm` ↔ `.cbl` reverse map is a linear recompute.**
   `designer_idx_for_generated` (`app.rs:6540`) and `form_for_generated`
   (`:6548`) scan and compare on every call; a per-stop lookup wants an index.
10. **Focus exists in the host but reaches nothing — and need not.**
    Per-viewport focus is readable (`host.rs:3539`, `:4111`), the root body
    tracks `focused_actual` to raise COBOL `onGotFocus`/`onLostFocus`
    (`host.rs:4111-4114`), and the interpreter can *command* focus
    (`FormHost::FocusWindow`, `form_host.rs:145`) but never reads it; nothing
    in `debug_link.rs` mentions focus, handles or viewports. **Off the critical
    path** (Q2): event delivery is already per form body
    (`host.rs:721`, `:1213`, `:3358`), so the debugger follows the interpreter
    that *stops* rather than the window that has focus. Recorded so a later
    reader does not go looking for a focus signal the design deliberately does
    not use.
