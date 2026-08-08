<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — External Crates for EXEC RUST

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-08-07
- **Prototype:** ./prototype/ — validated end-to-end against crates.io on
  2026-08-07 (interactive egui 0.36 dialog + CLI over the same modules,
  12 unit tests, live probe of the real 665-package generated-program graph).
  This plan is largely a transplant map for it.

## 1. Approach

Three layers, matching the prototype's architecture (dialog → action layer →
modules), with the shared pieces homed in `cobolt-compiler` because both the
IDE and `rcrun` already depend on it.

**Compiler (build side — no network).** A new `cobolt-compiler` module
`external_crates.rs` owns everything the build needs:

- `ExternalCrate { name, requirement, version, features, url }` — the pin
  record (spec R8/R10), a `#[serde(default)] crates: Vec<ExternalCrate>`
  field on the compiler's `CoboltProject` (parsed from `cobolt.toml` at
  `lib.rs:458`) so `rcrun build` honours pins with no IDE involved.
- `DIRECT_LINKED` — the single source of truth for the crates every generated
  program links directly (today duplicated between `generate_cargo_toml` and
  `cobolt-semantic::LINKED_CRATES`); semantic and the IDE consume it from
  here (R12, R20).
- `generate_cargo_toml` grows `pins: &[ExternalCrate]`: each pin becomes an
  exact-versioned dependency (`csv = { version = "=1.4.0", features=[…] }`)
  plus a `[patch.crates-io]` entry pointing at the project-local
  `crates/<name>-<version>/` — the prototype-proven mechanism that makes the
  vendored source the one cargo uses *and* guarantees one copy per unified
  version (R7, R10, R15). Two prototype-found defects are baked in: every
  `path =` is **absolute** (cargo resolves paths against the staged
  manifest), and the manifest carries an empty `[workspace]` table so a
  project living under someone else's cargo workspace cannot be adopted.
- `probe_manifest(...) -> String` — the same generator invoked with a
  candidate, for the add/update-time resolver probe (R11–R15). The IDE runs
  `cargo metadata` over it on a worker thread; **cargo's resolver is the
  conflict oracle**, we never re-implement resolution. Baseline-vs-candidate
  diff yields the R14 coexistence warning; the duplicate-(name,version) guard
  enforces R15; a resolver error *is* the R13 refusal, verbatim.
- `write_rust_manifest(dest, project, pins)` — called from `build_core`
  step 11c (the delivery step that already places binary, assets, and license
  notices in `dist/`): writes `rust_manifest.md` (name / exact version / URL,
  generated-by banner) when pins exist, deletes a stale one when none (R24–R26).

**Semantic (diagnostics).** `analyze(program)` has ten call sites across five
crates, so it stays; a new `analyze_with(program, &AnalyzeOptions)` carries
`external_crates: Option<Vec<String>>` (lib names, `-`→`_`):

- `Some(list)` = project context: `unlinked_crates()` accepts the listed
  names (R17/R20) and the refusal message becomes "add it under External
  Crates" (R21).
- `None` = single-file context: refusal says external crates require a
  project (R22). `analyze()` delegates with `None`, preserving today's
  behaviour for untouched callers.
- The compiler passes the pins' lib names; the IDE's Check/Run/Debug/Build
  paths (`runner.rs`, `form_runtime.rs`, `app.rs`, `agent_lint.rs`,
  `designer.rs`) pass them from the open project; `rcrun` passes them when a
  `cobolt.toml` is in play.

**IDE (interactive side — the only place with network).**

- `project_model.rs`: IDE-side `CoboltProject` gains the same
  `#[serde(default)] crates: Vec<ExternalCrate>` (type imported from
  `cobolt-compiler`); `Category::ExternalCrates` joins `Category` — `TOP`
  becomes 7 entries ordered after `Generated`, `root_subdir()` = `"crates"`,
  `is_addable()` = true; the exhaustive matches (`list_mut`,
  `all_lists_mut`, `of_kind`, …) get explicit arms — external crates are
  **not** a file list, so `list_mut`/`of_path` never route to it.
- `panels/project.rs`: the category renders one row per pin — `name version`
  (R2) — with a new hand-drawn package/cube `tree_icon` (Q6). The `[+]` and
  row activation emit new `ProjectPanelEvent` variants that open the dialog;
  no per-row tree context menus (the dialog owns update/remove).
- New `panels/external_crates.rs` — the prototype's `ui.rs` transplanted:
  same layout (registry field / search-and-pick / crate+requirement+features
  row / registered list with Update / Update All / Remove / manifest note /
  log pane), same worker-thread + `mpsc` state machine, buttons disabled +
  spinner while busy, R19 confirm modal. Literals become `Tr` keys ×6; glass
  theme comes for free from `apply_glass_visuals`. Manifest writing is not a
  dialog button in the IDE (it is a build step, R24); the dialog shows where
  the manifest will land.
- New `external_crates_service.rs` — the prototype's `registry.rs` + `ops.rs`
  transplanted: blocking `ureq` 2 + explicit `native_tls` connector (the
  `http_runtime.rs` stack; new IDE deps `ureq`/`native-tls`, same versions as
  `cobolt-runtime`), crates.io-compatible endpoints (search / resolve /
  download+unpack), and `add`/`update`/`remove` orchestration with a progress
  sink. Vendor into `<project>/crates/`; save `cobolt.toml` through the
  existing project-save path. The probe finds the workspace via the same
  resolution `build_core` uses (`BuildOptions.workspace_root` /
  `find_workspace_root`, exported from the compiler).
- Registry setting (R4, IDE-wide): `ExternalCratesSettings { registry: String }`
  persisted at `llm::base_dir().join("external_crates.toml")` — the
  `debug_settings.rs` load/save pattern exactly. Edited in the dialog's
  header field; defaults to `https://crates.io`.

**Docs & KB.** `docs/developers-guide-en.md` gains the External Crates
section (search, add, pin, update, remove, conflicts in plain words, the
manifest, network-at-add-time); `docs/cobol85-supported-syntax.md`'s EXEC
RUST crate rule is updated from "linked crates only" to "linked + registered
External Crates"; these feed the chunked KB, so `build_chunked_kb` is re-run
and `assets/knowledge/chunked.data` committed in the same change.

## 2. Affected crates / files

- `crates/cobolt-compiler/src/external_crates.rs` — **new**: pin type,
  `DIRECT_LINKED`, name-collision check, probe-manifest generator, manifest
  writer (transplants prototype `project.rs`/`conflict.rs`/`manifest.rs`).
- `crates/cobolt-compiler/src/lib.rs` — `CoboltProject.crates` field;
  `generate_cargo_toml(pins)`; step 11c manifest emit/cleanup; export
  `find_workspace_root`; `BuildOptions` untouched.
- `crates/cobolt-semantic/src/lib.rs` + `src/exec_rust.rs` —
  `AnalyzeOptions`/`analyze_with`; allowlist + R21/R22 message variants;
  `LINKED_CRATES` re-exported from / aligned with compiler's `DIRECT_LINKED`
  (semantic must not depend on compiler — see §4 decision d).
- `crates/cobolt-cli/src/main.rs` — pass project pins (or `None`) into
  `analyze_with` for `check`; build path needs no change beyond the compiler.
- `crates/cobolt-ide/src/project_model.rs` — `crates` field,
  `Category::ExternalCrates`, exhaustive-match arms.
- `crates/cobolt-ide/src/panels/project.rs` — category rendering, rows,
  icon, `ProjectPanelEvent::{OpenExternalCrates}` routing.
- `crates/cobolt-ide/src/panels/external_crates.rs` — **new**: the dialog
  (transplants prototype `ui.rs`).
- `crates/cobolt-ide/src/external_crates_service.rs` — **new**: registry
  client + action layer (transplants prototype `registry.rs`/`ops.rs`).
- `crates/cobolt-ide/src/app.rs` — dialog state on `CoboltApp`, event
  routing, `analyze_with` plumbing on Check/Run/Debug/Build paths.
- `crates/cobolt-ide/src/{runner.rs, form_runtime.rs, agent_lint.rs,
  panels/designer.rs}` — swap `analyze` → `analyze_with(project pins)`.
- `crates/cobolt-ide/src/i18n.rs` — ~22 new `Tr` keys ×6 (category label,
  dialog, refusals/warnings, update summary, confirm, registry label).
- `crates/cobolt-ide/Cargo.toml` — add `ureq`, `native-tls` (runtime's
  versions/features).
- `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` — z bump + entry.
- `docs/developers-guide-en.md`, `docs/cobol85-supported-syntax.md`,
  `assets/knowledge/chunked.data` (rebuilt) — docs/KB (translations untouched).

## 3. Data / model changes

- **`cobolt.toml`** gains an array of tables (absent = empty, old projects
  load unchanged in both parsers):

  ```toml
  [[crates]]
  name        = "csv"
  requirement = ""            # developer's words; "" = newest stable
  version     = "1.4.0"       # the exact pin every build uses
  features    = []
  url         = "https://crates.io/crates/csv"
  ```

- **`<project>/crates/<name>-<version>/`** — vendored, read-only third-party
  source; created on add, replaced on update, deleted on remove.
- **Generated `Cargo.toml`** (build artefact) — pins + `[patch.crates-io]`
  with absolute paths + empty `[workspace]`.
- **`<dest>/rust_manifest.md`** (build artefact) — banner + `| Crate |
  Version | URL |` table; absent when no pins (stale copy deleted).
- **`<llm base dir>/external_crates.toml`** — IDE-wide `registry` string.
- No `.cfrm`, AST, or runtime format changes; no new COBOL syntax (spec
  non-goal).

## 4. Key decisions & alternatives

- **(a) Exact pin + `[patch.crates-io]`** for vendored source — Why:
  deterministic builds from project-local source with cargo-guaranteed
  single-copy unification (R10/R15); prototype proved serde unifies (graph
  665 → 665). — Rejected: plain path-dependencies (duplicates the crate
  beside the registry copy — the R15 trap); full `cargo vendor` of the
  closure (hundreds of MB per project, unasked-for).
- **(b) `cargo metadata` as the conflict oracle** at add/update time — Why:
  resolution, `links` collisions, and feature unification are cargo's own
  semantics; the probe manifest is the *same text* the build uses, so the
  verdict cannot drift (R11–R15). Prototype caught a real R14 live (itoa
  1.0.10 vs 1.0.18). — Rejected: re-implementing semver/links resolution
  (wrong the day cargo changes), index-only heuristics (blind to features).
- **(c) Shared code lives in `cobolt-compiler`** — Why: the IDE and `rcrun`
  both depend on it; the build must work IDE-less. — Rejected: a new
  workspace crate (more surface for one module's worth of code).
- **(d) Semantic does *not* depend on the compiler** (dependency direction:
  compiler → semantic). The linked-crate names stay in `cobolt-semantic` as
  today's `LINKED_CRATES`; a compiler unit test asserts it equals
  `DIRECT_LINKED`'s lib names, so the two cannot drift silently. — Rejected:
  reversing or splitting crates for one constant.
- **(e) Network client is IDE-only, blocking `ureq`** (runtime's stack,
  explicit TLS connector) — Why: add/update are dialog worker-thread
  actions; builds never need the network for the crate itself (R10).
  — Rejected: the IDE's existing async `reqwest` (wrong shape for a blocking
  worker; heavier plumbing for four endpoints).
- **(f) Dialog-centric UX** — tree `[+]`/row-activate open one modal that
  owns search/add/update/remove (prototype layout, operator-approved
  "perfect"); tree rows stay passive displays (R2). — Rejected: per-row tree
  context menus and inline editing (new tree machinery, more i18n surface,
  no added capability).
- **(g) `analyze_with` + options struct** — Why: ten `analyze` call sites;
  additive API keeps untouched callers compiling and behaviour-stable.
  — Rejected: changing `analyze`'s signature everywhere.
- **(h) Registry field lives in the External Crates dialog, persisted
  IDE-wide** via the `debug_settings.rs` file pattern (R4/Q7) — Why: the
  one place it matters, discoverable, IDE-scoped storage. — Rejected: the
  Appearance dialog (per-project by design, contradicts R4).
- **(i) Spec assumptions adopted as-is:** Q2 (extra-features field yes,
  default-features toggle deferred), Q3 (manifest = registered crates only,
  no license column), Q4 (`=` pins report "current" on update), Q5 (position
  after Generated), Q6 (package/cube icon), Q8 (anonymous registries only).
  Each is a small, isolated change if the operator overrules.

## 5. Risks & mitigations

- **Probe latency** (seconds warm, minutes on a cold index) → worker thread
  + spinner + narrated log lines (prototype pattern); the dialog never
  blocks the UI thread; progress text explains the pause (041 Q8 precedent).
- **Registry flakiness / rate limits** → per-call User-Agent (crates.io
  policy), 30 s timeout, R9 messages state *which* failure; no silent retry.
- **Probe passes but the crate fails to compile at build** (resolution ≠
  compilation — e.g. feature-union breakage like the `zune-jpeg` `log` case
  the prototype hit) → accepted residual risk: surfaces as a normal build
  error with cargo's message; guide documents it; the probe still eliminates
  the whole resolution/links class at add time.
- **Windows paths in generated TOML** (backslashes break TOML strings) →
  normalize `path =` values to forward slashes (cargo accepts them on
  Windows); unit-test the generator with a `\`-bearing input.
- **Older PowerRustCOBOL opening a `[[crates]]` project** → serde ignores
  unknown fields: it builds without the pins and blocks fail semantic with
  the unregistered-crate error — loud, not silent; guide notes the version
  requirement.
- **`crates/` dir name collision with a user's existing folder** → add-time
  check: if `<project>/crates` exists untracked with unexpected content,
  refuse with a clear message instead of writing into it (user code is
  sacred).
- **Update leaves old vendor dir on failure mid-swap** → prototype ordering
  kept: download new → verify → only then delete old → record (R18).

## 6. Test strategy

Prototype's 12 unit tests transplant with their modules. New/expanded, per
crate (quantified reporting per golden rule #7 where measurements exist):

- **cobolt-compiler** (`external_crates` unit + `tests/`): pin TOML
  round-trip incl. absent-field defaults; `generate_cargo_toml` with pins —
  asserts exact-pin lines, `[patch.crates-io]` paths absolute + forward-slashed,
  `[workspace]` present, features rendered; `DIRECT_LINKED` ↔ semantic
  `LINKED_CRATES` parity test (decision d); manifest writer — columns,
  banner, stale-delete; collision verdicts (already-available vs clash vs
  reserved `cobolt-*`). **E2E** (041-style, network + real builds, timings
  reported): build a project with a `csv`-using block → binary runs, output
  proves the parse, `dist/rust_manifest.md` exists with csv's row (AC2/AC3/
  AC11 core); lockfile contains exactly one `serde` when serde is pinned
  (AC5); probe refusal on a `links`-conflicting crate (AC7).
- **cobolt-semantic**: registered name accepted in project context;
  unregistered name → R21 message (project) vs R22 message (single-file);
  `analyze()` unchanged-behaviour regression.
- **cobolt-ide**: i18n completeness (existing harness auto-covers the new
  keys — AC15); `project_model` — `[[crates]]` round-trip through the IDE's
  save path, `Category::TOP` order/arms; tree render behavioural test —
  category shows `csv 1.4.0` row (AC2's tree half); dialog state-machine
  tests driving the panel struct with injected worker messages (no network):
  busy-disables-buttons, refusal renders red, confirm-modal gates removal
  (AC13's UI half).
- **Manual/visual** (operator): open a project → External Crates → search
  "csv" → add → watch probe narration → Build → run the app → open
  `dist/rust_manifest.md`; flip the registry field to a mirror and observe
  the next search hitting it (AC14's live half); all six languages via the
  language switcher.

## 7. Steering compliance

- [ ] i18n: every new UI string a `Tr` field in EN/ES/PT/JA/ZH/FR; no
      literals in the dialog/panel/tree.
- [ ] Generated-code contract: generated `Cargo.toml` and `rust_manifest.md`
      are regenerated build artefacts with banners; vendored source is
      third-party input, read-only, never hand-edited by us.
- [ ] English dev guide updated; `-es/-pt/-jp/-cn` translations untouched.
- [ ] System KB: `cobol85-supported-syntax.md` + guide feed the KB;
      `build_chunked_kb` re-run, `chunked.data` committed same change.
- [ ] Fix vs feature: **feature** → implemented on the features branch
      (merge from main first, GR#5); version bump **z only** in
      `version.rs` + `CHANGELOG.md` (operator owns x/y); f=96 [Noticia]
      announcement only after merge to main, operator-confirmed.
- [ ] No "cobolt" in user-facing text ("PowerRustCOBOL" / "External
      Crates"); COBOL identifiers and generated code stay English.
- [ ] User code sacred: removal = explicit + confirmed; conflict never
      deletes/edits COBOL; `crates/` collision refuses rather than overwrites.
