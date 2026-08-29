---
name: nist-grind
description: Drive the NIST CCVS85 conformance suite toward 100% on both axes, one work item at a time, across as many sessions as it takes. Reads and updates the committed ledger NIST/progress.json so every session starts identically without re-deriving state. Use when the user runs /nist-grind, asks to continue or automate the NIST work, or when resuming NIST conformance after any break.
---

# /nist-grind — the NIST conformance loop

This is a **long-running, multi-session grind**. It is designed so that any
session, and any subagent, can pick it up cold and continue without a
hand-written handoff.

**The ledger is `NIST/progress.json`. It is the state.** Read it first, update
it last, commit it with the change. Do not write a `HANDOFF-*.md` — those were
re-derived by hand each session and drifted out of date. If you learn something
durable, it belongs in the ledger (`dead_ends`, `work_queue`, `history`) or in
`CLAUDE.md`, not in a scratch document.

---

## The invariants — these outrank speed

1. **One module at a time, in order** (GOLDEN RULE #9). Finish the current
   module on **both** axes before starting the next. A tempting fix in another
   module goes in `parked` and is not acted on.
2. **The protected baselines must hold after every change.** They are in the
   ledger under `protected_baselines`. Today: NC 95/95 on both axes with
   4614 PASS / 0 FAIL, SQ 85/85 with 624 PASS / 0 FAIL, whole-suite compile
   422/434. **If a change breaks one, revert that commit and record a dead
   end.** Do not chase the regression forward.
3. **Never conflate the two axes.** Compile is the strictly weaker claim.
   Always report both numbers.
4. **A deleted test is not a passing one.** `***** ****TEST DELETED****` means
   the program failed its own setup and skipped the case. Watch the `DELETED`
   count in the run summary — it can move hugely while the clean-program count
   sits still, and that is real progress.
5. **Never invent grammar to make a test pass.** If a member needs syntax that
   is genuinely not implemented, that is a feature gap: record it in the ledger
   (`blocked_on` or `parked`) and move to the next item. Fixing a bug in
   already-supported syntax is in scope; extending the language is not.
6. **Never merge to `main` and never post to the forum** without an explicit
   ask in the current conversation. Committing and pushing the working branch
   is pre-authorised; those two are not.

---

## The loop

Repeat until the current module is 100/100 on both axes, then advance
`current_module` to the next entry in `module_order` and keep going.

### 1. Orient (cheap — do not skip)

```bash
git branch --show-current          # must be `fixes`; if not, checkout and merge main
cat NIST/progress.json             # the state
```

If the branch is not `fixes`, `git checkout fixes` **and merge `main` into it
before the first edit** (GOLDEN RULE #5).

### 2. Build both binaries — separately

```bash
cargo build --release -p cobolt-cli
```

```bash
cargo build --release -p cobolt-semantic --example nist_conformance
```

⚠️ **Two commands, never one.** Combining `-p cobolt-cli` with `--example`
leaves `rcrun` stale, and the census then silently measures the previous
version.

### 3. Pick the top work item

Take the highest-priority entry in `work_queue` for `current_module`. If the
queue is empty or stale, rebuild it:

```bash
./target/release/examples/nist_conformance run   IX > /tmp/run-ix.txt
./target/release/examples/nist_conformance fails IX > /tmp/fails-ix.txt
```

Bucket the failures by their message to find shared causes — one program's
failure is an anecdote, a bucket across ten programs is a root cause.

**Check `dead_ends` before starting.** If the approach you are about to take is
listed there, read why it failed and take the recorded alternative instead.

### 4. Diagnose — parallel is allowed here, and only here

Diagnosis is read-only, so it is safe to fan out. Spawn one agent per failure
bucket (Explore or general-purpose), each returning: the failing programs, the
COBOL construct involved, the observed vs expected status or value, and the
narrowest hypothesis. **Do not let a diagnosing agent edit anything.**

Then **verify the hypothesis by hand before writing a fix.** Extract the member
and run it:

```bash
mkdir -p /tmp/ix && cd /tmp/ix
/path/to/nist_conformance extract IX214A > IX214A.cbl
/path/to/rcrun run IX214A.cbl --source-format fixed --switch XXXXX051=ON --switch XXXXX052=OFF
```

The CCVS report is `XXXXX055` in that directory — read it with `grep -a`, it is
binary-ish. For a member that declares a producer (`inherits_from` in the
harness), run the producer in the same directory first.

Better still, **reduce to a minimal COBOL repro** in `/tmp`. Three of this
suite's biggest fixes were found that way, and a repro that fails before and
passes after is what the regression test is built from.

### 5. Fix — sequentially, one item at a time

Never apply two independent fixes before measuring. They share one runtime, and
a regression from either becomes untraceable.

### 6. Test, then gate

```bash
cargo test --release -p cobolt-runtime --no-fail-fast
```

Read **every** `test result:` line. Never verdict a sweep from a grep for
failures.

Then the **full regression** — all of it, every time:

```bash
./target/release/examples/nist_conformance strict    > /tmp/strict-all.txt
./target/release/examples/nist_conformance run   NC  > /tmp/run-nc.txt
./target/release/examples/nist_conformance run   SQ  > /tmp/run-sq.txt
./target/release/examples/nist_conformance run   IX  > /tmp/run-ix.txt
```

**The gate:** every `protected_baselines` figure must match exactly, and the
current module must have improved or held. If a baseline moved down, revert and
record a dead end.

### 7. Add a regression test

Every fix gets one, in the crate it belongs to — usually
`crates/cobolt-runtime/tests/test_indexed.rs` or a `#[cfg(test)]` module in the
source. The test must fail before the fix and pass after. Say in the test's doc
comment **which CCVS85 member** motivated it and what went wrong.

### 8. Land it

- Bump `VERSION` in `crates/cobolt-ide/src/version.rs` — the **fix number `z`**,
  always, feature or fix. Only the operator raises `x` or `y`.
- Add a `CHANGELOG.md` entry at the top, dated absolutely, carrying the before
  and after numbers on both axes.
- Update `NIST/progress.json`: the module's figures, `measured_at_version`,
  `history`, and the `work_queue` (remove what is done, re-rank the rest).
- Update `docs/developers-guide-en.md` **if a developer would observe the
  change** (GOLDEN RULE #3). English canonical only — GOLDEN RULE #8 is
  suspended, so touch no translation file.
- Commit, then push `fixes`.

```bash
git push origin fixes
```

⚠️ **The push window (GOLDEN RULE #1).** Never push between **09:00 and 18:00
São Paulo, Monday–Friday**. Check first:

```bash
TZ=America/Sao_Paulo date '+%A %H:%M'
```

Inside the window: **commit anyway, skip the push**, note it in the ledger, and
keep working. The embargo delays publication, never progress.

### 9. Loop

Go back to step 3. Do not stop to summarise unless the user asked, the module
finished, or you hit a genuine blocker.

---

## When a module finishes

Both axes at 100%. Then, in one change:

1. Set its `state` to `finished` and `protected` to `true` in the ledger.
2. Add its figures to `protected_baselines` — from now on every future change
   must preserve them.
3. Advance `current_module` to the next entry in `module_order`.
4. Baseline the new module (`run <MOD>`) and seed its `work_queue`.

---

## Reporting

Per module, always as **two separate numbers**, and vertically — one row per
module, names spelled out (`NC (Nucleus)`, not bare `NC`):

| Module | Compile | Execution | Assertions |
|---|---:|---:|---|

Report only figures the tools actually produced. Never estimate, never round up
a measurement, and never present a compile score as if it meant the programs
work.

---

## Known traps

- **zsh does not word-split unquoted variables.** Putting `rcrun`'s flags in a
  shell variable passes them as a single argument and the program silently does
  nothing, creating no output files. Write the flags out in full.
- **Do not run `cargo` while a `cargo test --workspace` sweep is running.** The
  compiler tests shell out to `cargo build`, and the package-cache lock makes
  one fail with a message that looks like a real failure.
- **Check `df` before believing a build error.** Disk exhaustion masquerades as
  "could not compile <some innocent crate>"; `target/` runs to tens of GB.
- **A harness limitation looks exactly like a runtime regression.** Two of this
  session's findings were the measurement being wrong, not the compiler. Run
  the program by hand before blaming the runtime.
- **RL (Relative I/O) is a feature build, and it is authorised.** There is no
  RELATIVE engine at all — the parser accepts the clause and the runtime never
  matches it, so such a program parses and then misbehaves silently. The
  operator ruled on 2026-08-29 to **implement it as the NIST suite expects**.
  It is last in `module_order` on purpose: do not start it until IX is 100/100
  on both axes, and run it through the spec pipeline (`/specify` → `/plan` →
  `/tasks` → `/implement`) since it is a new capability rather than a
  correction. The scope notes are in the ledger's `RL` entry.
  **This is the one authorised exception** to "never invent capability to make
  a test pass" — everywhere else a genuine feature gap is still recorded and
  parked.
- **This project is Rust only.** Never write a shell, Python or Node script to
  inspect, count, generate or bulk-edit anything — not even as a throwaway in
  `/tmp`. Use the editing tools, or write it in Rust as a test or an example
  binary. Invoking `cargo`, `git`, `grep`, `ls` is fine; stringing them into a
  script is not.
