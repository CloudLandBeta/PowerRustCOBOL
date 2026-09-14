---
name: fix
description: The workflow for a direct "fix this" request in PowerRustCOBOL — classify, branch, change, gate, verify, version, commit. Use whenever the user asks to fix, correct, repair or debug something and no spec is in flight. The counterpart to /implement, which serves the spec-driven feature path.
---

# /fix — the fix path, end to end

`/implement` serves the feature path. **This serves the fix path**, which had no
skill and lived only in CLAUDE.md prose. Work the steps in order; each one has
bitten this project at least once.

---

## 1. Classify before touching a file

**Fix** — a defect, or **technical debt**: a COBOL-85 construct that *should
already work*, a declared clause the runtime ignores, baseline behaviour every
comparable widget has, an incomplete catalogue, or **withdrawing a requirement
that should never have applied**.

**Feature** — a capability *beyond* the COBOL-85 standard or the IDE's existing
scope: a new widget, panel, non-standard extension, new platform target.

Read **Classification precedents** in `CLAUDE.md` first — several arguments are
already settled and should not be had twice. Ambiguous? **Ask.** The answer
decides the branch, and mixing the two in one commit breaks GOLDEN RULE #5.

## 2. Branch, and sync from `main` *before* the first edit

```bash
git checkout fixes && git rebase main
```

**Rebase, never plain `git merge main`** (operator ruling 2026-09-14): the remote
refuses merge commits on a working branch, and a rebase cannot create one.

⚠️ **`git rebase` refuses a dirty tree, and this tree is shared.** When another
session's edits are sitting unstaged, use this instead — equivalent whenever
`main` is an ancestor, and `--ff-only` can never make a merge commit:

```bash
git merge --ff-only main
```

Rebase only **before the first commit**, which is where this step sits. Once the
branch is pushed, a rebase rewrites published history and needs a force-push,
which on a shared branch can destroy another session's work.

`main` is never a workbench — a `PreToolUse` hook in `.claude/settings.local.json`
refuses `git commit` there. The sync comes **before** editing, not after.

⚠️ **The working tree is shared with other sessions.** Expect files you did not
touch to be modified. Never `git commit -a`; never stage a path you did not edit.

⚠️ **In a worktree checkout** you cannot `git checkout fixes`. Fast-forward it in,
commit, then `git push origin HEAD:fixes`.

## 3. Make the change

- **Surgical.** Only what the request requires. Don't improve neighbouring code.
- **User code is sacred** — never delete what the developer wrote, however
  orphaned or unparseable. Report it.
- **A window may never resize itself.** Own an explicit `size: Vec2`,
  `.resizable(false).fixed_size(...)`, one grip as the field's only writer.
- **Runtime fix?** It must reach **all three hosts** that construct the
  interpreter — `rcrun run-form`, embedded child forms (`host.rs`), and
  `run_form_app` in `cobolt-compiler` (the compiled binary, the one that gets
  forgotten and the one the developer ships). Read the
  `interpreter-binary-parity` skill.
- **Corner / painting fix?** Read `rounded-corners` before touching anything that
  paints, masks, clips or strokes a rounded corner. A visual fix applied to one
  surface is invisible on the other — check both `paint.rs` (designer) and
  `render.rs` (run form).

## 4. The two gates

**System KB.** Did you change a behaviour, control, property, method or event?
Then update the `cobolt-compiler` doc tables **and** regenerate the store in the
same change:

```bash
cargo run -p cobolt-ide --example build_chunked_kb
```

Commit `assets/knowledge/chunked.data`. A red
`prebuilt_chunked_kb_matches_the_published_documentation` is a **real failure**.
An unchanged `chunked.data` plus a green freshness test means you edited the
wrong file — the KB's source is Rust constants, not `docs/*.md`.

**Documentation.** Did you change anything a developer would observe? Update
`docs/developers-guide-en.md` in the same change.

While you are in that document, **check that what it already says is still
true** — a fix frequently invalidates a neighbouring claim, and a fix that closes
a gap leaves the page still apologising for it. `/doc-audit` is the procedure:
trace each ✅ to a **runtime** path (parsed is not implemented), re-check each ❌
against what has since landed. **Report what you find; do not quietly rewrite
it.**

⚠️ **GOLDEN RULE #8 — and it is NOT "never touch the translations".** That was
the pre-2026-08-24 rule and it still appears in older skills. The current ruling:
update the **English canonical only**, then **physically delete that document's
five translations**. They are regenerated whole at the next minor/major, never
patched. **No English file is ever deleted.**

Every document now ships in all six languages, so this deletes five real files
and turns two `docs_embed.rs` guards red until the next minor. That red is the
intended signal — do not re-add their `#[ignore]`.

If that cost is too high for the change at hand, **say so and offer to park it**
rather than paying it silently or skipping the doc.

## 5. Verify — build *and* test every crate you touched

```bash
cargo test -p cobolt-forms --features render    # the render feature is REQUIRED
cargo test -p cobolt-ide --bin cobolt-ide       # the IDE needs --bin
```

- Sweeps use `--no-fail-fast`, and you read **every** `test result:` line. Never
  verdict a sweep from a grep for failures.
- Live-network and `libsqlite3-sys` failures are environmental.
- **Verify-first**: never report a measurement the run did not produce.
- **Never drive the application** to verify. No computer-use, no UI automation.
  Verify through builds and tests; let the operator look at the UI.
- Disk exhaustion masquerades as compiler errors. "could not compile *&lt;innocent
  crate&gt;*"? Check `df` before believing it.

## 6. Version and changelog — every change, no exceptions

Bump the **fix number `z`** in `crates/cobolt-ide/src/version.rs` and add a
top-of-file `CHANGELOG.md` entry with the **absolute** date. Fix or feature, both
bump `z`: **only the operator raises `x` or `y`.**

One bump per *job*, not per file — a multi-file job carries a single bump on the
final commit. The operator reads the version in the window title to confirm they
are running fresh code, so never skip it.

## 7. Quarantine sweep, then stage explicitly

Before every commit, scan for **untracked program source in a language other than
Rust or COBOL** (`.py .sh .bash .zsh .js .mjs .cjs .ts .rb .pl .php .lua .awk .jq
.ps1 .bat .cmd`, or any untracked file with a `#!` or the executable bit):

```bash
git status --porcelain --untracked-files=all | awk '/^\?\?/{sub(/^\?\? /,"");print}' \
  | grep -iE '\.(py|sh|bash|zsh|js|mjs|cjs|ts|rb|pl|php|lua|awk|jq|ps1|bat|cmd)$'
```

Anything found is **MOVED, never deleted**, to
`/Users/emersonlopes/Documents/PowerRustCOBOL-local-settings/quarantine`, and
**every move is reported to the operator by name** — it may be theirs.

Two hard limits: **tracked files are never touched** (`git ls-files` is the
authority), and **data, config, docs and assets are never in scope at any
extension** — `.json .toml .yaml .xml .md .csv .tsv .txt .cfrm .cidx .data
.lock`, fonts, images, fixtures. Ship-blocking foreign code is the target; a file
a crate reads is not. Ambiguous? **Leave it and ask.**

Then stage **by name** and **read every diff** — not just the files that warned.
A clean single `M` can still hide another session's hunk.

## 8. Commit, and mind the push window

End the message with:

```
Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
```

**NEVER `git push` between 09:00 and 18:00 (America/Sao_Paulo), Monday–Friday**,
except on Brazilian national holidays — even if explicitly asked. A user-level
hook enforces it and evaluates at **call time**, so a backgrounded
`sleep && git push` is denied too; schedule the push instead.

```bash
TZ=America/Sao_Paulo date '+%H:%M %A'     # check before pushing
```

Outside the window, pushing needs no further permission — a commit is not a
stopping point. **Merging into `main` is different: only when explicitly asked.**

## 9. Announce nothing

Forum posts are **not** a standing task. A push to `origin/main` triggers nothing;
announcements are batched into the Release Candidate post and written only when
the operator asks. If asked, GOLDEN RULE #4 (fixes → f=97) and #4b (features →
f=96) govern the content, and publishing happens only from `main`.

---

## Report

Say what changed, the **real** test numbers, what you did **not** do and why, and
anything left for the operator to look at. If a gate was expensive and you parked
it, name where it was parked so it is not lost.
