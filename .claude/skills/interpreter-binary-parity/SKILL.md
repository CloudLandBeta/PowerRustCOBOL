---
name: interpreter-binary-parity
description: A runtime change is not done until it lands in EVERY host that runs a form — rcrun run-form, embedded child forms, AND the compiled binary. Read before fixing anything in cobolt-runtime's interpreter, or anything a form's behaviour depends on at run time. The compiled binary is the one that gets forgotten, and it is the one the developer ships.
---

# The interpreter runs in more than one place

A fix to the interpreter is usually a fix to **one** call site — and there are
three that construct a form interpreter. Wire two and the third keeps the bug,
silently, until someone runs the surface you missed.

**The one that gets forgotten is the compiled binary**, and it is the one the
developer ships. The two live surfaces are what an agent tests, so the bug looks
fixed right up until a build goes out.

## The three hosts

| Host | Where | What runs there |
|---|---|---|
| `rcrun run-form` | `crates/cobolt-cli/src/form_gui.rs` | The IDE's **Run Form** / **Debug Form** button |
| Embedded child forms | `crates/cobolt-form-host/src/host.rs` | A form opened by another form (spec 051) |
| **The compiled binary** | `run_form_app` in `crates/cobolt-compiler/src/lib.rs` | Everything `rcrun build` produces |

`crates/cobolt-ide/src/form_runtime.rs` also constructs interpreters, but only
in `#[cfg(test)]` — check it when a signature changes, not for behaviour.

Find them all before editing:

```bash
grep -rn "new_with_channels\|new_with_channels_and_bridge" crates/ --include='*.rs' | grep -v "^crates/cobolt-runtime/"
```

## What this rule covers

Anything the interpreter is *told* rather than *deduces*, and anything that
changes what a running form does:

- setup calls on the interpreter (`set_control_ids`, `set_input_channel`,
  `set_event_counter`, `set_form_host`, `register_exec_rust_blocks`, …);
- new capabilities a control gains at run time;
- event queueing and dispatch;
- anything read off a control's properties during a run.

## How it goes wrong — the worked case (1.65.59)

`AgentObject::Ask` queued `onResponse` after a successful reply. The verbose log
said *"onResponse will fire"*. It never did, on any surface.

A COBOL word reaches the interpreter **upper-cased**, so a member call on
`Agent-Helper` arrives as `AGENT-HELPER`. The generated event loop compares
against the literal the designer wrote:

```cobol
           EVALUATE COBOL-CONTROL-ID
               WHEN "Agent-Helper"
```

`"AGENT-HELPER"` ≠ `"Agent-Helper"`, so `EVALUATE` fell through. A **UI** event
never showed it — those carry the host's spelling and match. Only events the
interpreter queued for itself were affected: `onResponse`, `onError`,
`onComplete`, `onCancelled`, `onTimeout`, on every async control.

The fix hands the interpreter the form's spelling. Two hosts were wired; the
**compiled binary was not**, and only the operator asking "make sure a standalone
binary would work as well" caught it. A built application would have shipped
with the bug after both live surfaces were verified.

## Prefer the host over codegen

When a fix could go in the generated COBOL **or** in the host that feeds the
interpreter, prefer the host:

- a codegen change needs every `.cbl` regenerated before it takes effect, so
  existing forms stay broken until someone rebuilds them;
- a host change repairs already-generated forms on the next run.

The 1.65.59 fix could have made the generated `EVALUATE` case-insensitive. It
did not, for exactly this reason.

## Before calling it done

1. Every host in the table changed, or a stated reason why one does not apply.
2. `cargo test -p cobolt-runtime -p cobolt-form-host -p cobolt-compiler -p cobolt-cli`
   — all four, because the sites live in four crates and a miss shows up as a
   crate that was never rebuilt.
3. A test on the runtime behaviour itself, not on one host's wiring: the
   behaviour is shared, the wiring is repeated.
4. Say in the commit **which** hosts were touched. "Fixed in the interpreter" is
   the sentence that hides a missed one.

## Not this skill

- **Designer-canvas vs running-form RENDERING** parity (spec 017, one engine in
  `cobolt-forms`, guarded by `engine_reference_form_parity_static_vs_faces`) —
  a different parity, about paint. See `rounded-corners` for corners and
  `egui-paint-regressions` for the rest.
- **Property tables** — `test_nonvisual_property_readers` already asserts that a
  property declared runtime-read really is read. Update it in the same change;
  it is the guard that catches a setting which reaches nothing.
