<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Handoff — Project's Crates (Beta) / spec 044

**Written:** 2026-08-08, end of the implementation session.
**For:** whoever (agent or operator) picks this up next.
**Read this first**, then `tasks.md` in this folder for the exhaustive
per-task record (files touched, exact test counts, timings).

## TL;DR

The feature is **fully implemented, tested, and green** on the `features`
branch — but **nothing is committed**. `git status` shows only working-tree
changes; `features` HEAD is still `0a14917`, identical to `main`. The next
session's job is almost entirely git mechanics (split → commit → push →
announce), plus one open design question the operator raised that has
**no code written yet**.

## 1. Exact repo state

```
branch:        features (HEAD 0a14917, == main — no local commits this session)
version.rs:    1.60.47
last sweep:    cargo test --workspace --no-fail-fast → 98 binaries,
               1691 passed, 0 failed, 8 ignored (all pre-existing, annotated)
release build: target/release/cobolt-ide, built at 1.60.47, verified
               "Project's Crates (Beta)" is embedded (strings check)
```

Modified/untracked files (nothing staged):

```
 M CHANGELOG.md
 M Cargo.lock
 M assets/knowledge/chunked.data
 M crates/cobolt-compiler/Cargo.toml
 M crates/cobolt-compiler/src/lib.rs
 M crates/cobolt-ide/Cargo.toml
 M crates/cobolt-ide/src/{app,form_runtime,i18n,main,runner,version}.rs
 M crates/cobolt-ide/src/panels/{designer,doc_viewer,editor,md_render,mod,project}.rs
 M crates/cobolt-ide/src/project_model.rs
 M crates/cobolt-semantic/src/{exec_rust,lib}.rs
 M docs/cobol85-supported-syntax.md
 M docs/developers-guide-en.md
?? crates/cobolt-compiler/src/external_crates.rs
?? crates/cobolt-compiler/tests/          (test_external_crates_e2e.rs)
?? crates/cobolt-ide/src/external_crates_service.rs
?? crates/cobolt-ide/src/panels/external_crates.rs
?? crates/cobolt-semantic/tests/test_external_crates.rs
?? specs/044-external-crates/             (this folder — spec/plan/tasks/prototype)
```

## 2. What shipped (one paragraph; `tasks.md` has the receipts)

A project can register third-party Rust crates and use them from
`EXEC RUST` blocks with a plain `use` line. Tree category **Project's
Crates (Beta)** (after Generated Code) → dialog searches the configured
registry (pluggable, IDE-wide setting, crates.io default) → results render
as a paged markdown table (50/page, content-tight resizable columns,
description-only wrapping) → pick, optional version requirement + features,
Add → resolve → conflict-check (cargo's own resolver, via a probe manifest
identical to the real build) → vendor into `crates/` → pin in `cobolt.toml`.
Builds link pins via exact version + `[patch.crates-io]` (one copy per
crate, even when the base tree already uses it — proven for serde).
`Update`/`Update All`/`Remove` with confirmation. Every build emits
`dist/rust_manifest.md`. All localized ×6. All 15 spec acceptance criteria
verified (`spec.md` §5), most with both a unit test and a live/e2e proof
against real crates.io.

## 3. A real, pre-existing bug got fixed along the way

**This must ship as a SEPARATE commit from the feature (operator's GR#5).**

`base_dependency_block` in `cobolt-compiler/src/lib.rs` unions the
`zune-jpeg` `log` feature back in. Without it, **every generated GUI
program fails to build on any fresh dependency lock** — `zune-jpeg` ≥
0.5.15 (pulled transitively via the eframe image stack) ships with `log`
off, and its logging macros then expand to nothing where an expression is
required. This was verified broken on a clean `main` checkout *before* any
spec-044 code touched it (bisected by stashing). It is a fix, not a
feature, per the operator's CLAUDE.md rule that missing/broken standard
behaviour counts as tech debt → fix.

## 4. What next session needs to DO (in order)

### 4a. Split the CHANGELOG

`CHANGELOG.md`'s `## [PowerRustCOBOL 1.60.47]` heading currently has
**both** the feature and the fix under one entry (`### Added` +
`### Fixed`). Per GR#5 these need to ship as separate commits. The content
is already written and correct — it just needs to be split into two
headings/entries before committing. Decide with the operator whether the
fix gets its own earlier version number (the feature went through several
z-bumps this session — 1.60.44 → 1.60.47 — purely for its own iterations;
the fix could reasonably claim one z of its own, e.g. land as 1.60.44-fix
conceptually, or the operator may prefer both changes simply share the
final z since only they raise version numbers per
`no-minor-bumps-without-permission` — **ask, don't assume**).

### 4b. Stage and commit the fix

Files: just the `zune-jpeg` union line and its comment in
`crates/cobolt-compiler/src/lib.rs` (inside `base_dependency_block`). If
git-splitting a single-file, multi-hunk change is awkward, `git add -p` is
the tool. Commit message: describe the zune-jpeg breakage and its fix, no
mention of Project's Crates.

### 4c. Stage and commit the feature

Everything else in the modified/untracked list above. Commit message:
Project's Crates (Beta) — point at the CHANGELOG entry.

### 4d. Push — respect the work-hour rule (GR#1)

No `git push` Mon–Fri 09:00–18:00 America/Sao_Paulo (Brazilian holidays
excepted). Check the current time before pushing; don't assume the window
from when this was written.

### 4e. Announce — only after merge to **main**, and only with a go-ahead

- Fix commit → forum **f=97** (no prefix), in Spanish, vBulletin BBCode,
  signed "Anthropic Claude Codex Agent". Title ≤ 50 chars.
- Feature commit → forum **f=96**, prefix **[Noticia]**, thread "Nuevas
  funcionalidades de PowerRustCOBOL" (reply on its last page, or create it
  if missing).
- **Show the exact post text and wait for explicit approval before
  submitting** — both are standing rules, not optional.
- Do this only after the operator has actually merged `features` → `main`.
  It is not merged as of this handoff.

### 4f. Manual verification (operator, not automatable)

Open the release IDE → a project → Project's Crates (Beta) → search
something broad ("maps", "json") → confirm the table paginates and columns
resize by dragging → Add a crate → watch the log narrate the probe → Build
→ run the app → open `dist/rust_manifest.md` → flip the registry setting to
a mirror and search again → walk all six languages.

## 5. Open design question — operator asked, NOT implemented

The operator asked whether crate-name collisions with PowerRustCOBOL's own
linked crates (egui, eframe, …) could be resolved by giving the colliding
crate a fixed Cargo `package = "…"` rename instead of refusing the add
outright. **This was investigated experimentally (see the conversation),
not implemented.** Findings, for whoever picks this up:

- **Works for genuinely incompatible versions.** `prj_egui = { package =
  "egui", version = "0.29" }` alongside the platform's `egui = "0.36"`
  resolves both, and both compile and are independently usable.
- **Cannot apply when versions are semver-compatible** — cargo unifies
  them and then *rejects* the alias ("depends on crate serde multiple
  times with different names"). So this only ever helps the
  already-incompatible case — which is exactly the case R12 refuses today.
- **No interop across the alias boundary.** A value from the aliased copy
  cannot be passed to platform APIs expecting the real one (proven:
  `expected egui::Color32, found a different egui::Color32`). A block using
  an aliased egui could not hand anything to `cobolt_windows::open` or the
  form host.
- **Does not touch the COBOL↔Rust bridge at all.** Everything that crosses
  into COBOL (the 48 shipped `CLASS RUST-*` types) is `std` or lives in
  `cobolt-runtime`, which is reserved by name (`cobolt-*` prefix) and can
  never be the thing being aliased. Bound items are unaffected either way.
- **Real risk if made the default instead of an exception:** ecosystem
  crates whose types cross crate boundaries (serde + serde_json, tokio +
  tokio-util) would silently duplicate if a project's own copy diverged
  from what a *different* added crate expects, producing confusing
  "trait not satisfied" errors that don't exist today under
  `[patch.crates-io]` unification (which forces exactly one copy —
  that's what AC5 proves).
- **Recommendation surfaced to the operator, not yet decided:** keep plain
  names + unification as the default (today's behaviour); only when a
  candidate collides with a platform-linked crate at an incompatible
  version, offer the alias instead of a hard refusal — generate
  `prj_<name> = { package = "<name>", version = "…" }`, show the alias in
  the dialog/tree, document that the block then writes
  `use prj_<name>::…`. This is additive to the existing R12 refusal path
  in `crates/cobolt-compiler/src/external_crates.rs`
  (`name_collision`/`CollisionRefusal`) — no architectural rework needed,
  but it **is unstarted**: no spec requirement, no plan, no code.
- If the operator wants this pursued, it likely wants its own small spec
  addendum (new R-numbers under 044, or a 044-followup) rather than being
  folded silently into what's already marked done — the existing AC6/AC9
  tests assert the *current* refusal behaviour and would need to change.

## 6. Known non-blocking follow-ups

- `docs/developers-guide-en.md` has one screenshot placeholder still open:
  `📷 external-crates-dialog.png` — capture after Add, in whatever tree
  position the operator settles on. Candidate for the `doc-shots` skill.
- Translations (`docs/developers-guide-{es,pt,jp,cn}.md`, and `-fr` which
  doesn't exist) are deliberately untouched — user-maintained, per
  standing rule. The English guide is current.
- Internal identifiers keep the name `ExternalCrates`
  (`Category::ExternalCrates`, `external_crates.rs`, the `044-` spec
  folder slug) while every user-facing surface says "Project's Crates
  (Beta)". This was a deliberate call (internal names sit behind the
  brand, same as `cobolt-*` crate prefixes) — flagged to the operator, not
  reversed. Revisit only if asked.
- The prototype at `specs/044-external-crates/prototype/` is a standalone,
  detached cargo project (its own `[workspace]`) used to validate the
  approach before implementation. It is not part of the shipped product
  and needs no further attention; leave it as historical record unless
  the operator wants it removed.

## 7. Orientation map

- `specs/044-external-crates/{spec,plan,tasks}.md` — the full spec-driven
  trail; `tasks.md` has T1–T17 each with files touched and verification
  numbers actually observed (not asserted).
- `crates/cobolt-compiler/src/external_crates.rs` — build-side model: pin
  type, reserved-name collision check (`name_collision`), probe-manifest
  generator, `rust_manifest.md` writer. No network.
- `crates/cobolt-ide/src/external_crates_service.rs` — IDE-side registry
  client (search/resolve/download), the resolver probe (`cargo metadata`
  over the compiler's probe manifest), add/update/remove actions, and the
  `flow_tests` module (mock registry + live crates.io verdicts).
- `crates/cobolt-ide/src/panels/external_crates.rs` — the dialog.
- `crates/cobolt-ide/src/panels/md_render.rs` — gained `TableLayout`
  (`Equal` vs `TightResizable`) and table-cell link support
  (`RenderOutput::clicked_link`) as part of this work; both are shared
  renderer changes, worth knowing about if touching the docs viewer later.
