<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The WebAssembly module as the portable program format: native builds without a Rust toolchain per program (DESKTOP-WASM-001)

- **Status:** tasked, 2026-11-14 (2.0.482). No task started.
- **Plan:** ./plan.md   **Spec:** ./spec.md   **Date:** 2026-11-14
- **Branch:** `features-2x-wasm-program-format` (feature). The two fixes F1 and F2 go on `fixes-2x-wasm-emit-order`, cut from `main`.

These are small, ordered tasks that can each be checked on their own. Each one names:
- the files it touches;
- the requirements it satisfies;
- how it is verified.

Every test that measures prints one result block (GOLDEN RULE #7): the cases it ran, the counts, and for timed ones the milliseconds and the throughput. Real measurements only. Any throwaway tooling stays outside the repository (PRIME DIRECTIVE).

**Order.** The two fixes come first (#5). Then the milestones of plan §1.3, M0 to M6, then closing. The workspace builds and its tests stay green after every task. Inside M0 the order matters: the frames of today's path are recorded before anything in the form host changes (see "Refinements" below).

**Each task as it lands:**
- bumps `z` in `crates/cobolt-ide/src/version.rs`;
- adds a top `CHANGELOG.md` entry;
- updates this spec's row in `specs/WASM-PROGRESS.md`;
- is committed on its branch and pushed outside the forbidden window (GOLDEN RULE #1).

Tasks committed together share one bump. Nothing merges into `main` unless the operator asks (operator, 2026-11-13: "do not merge into main, push it in its own branch for now").

**Each milestone closes with** its "close" task, which:
- runs `cobolt-bench pixel-parity` and `cobolt-bench build-goal` over the reference projects;
- runs the AC9 measurement (NIST on both axes, then the `tests/cobol` corpus);
- updates the Developer's Guide's list of what still takes the Cargo path, deletes its five translations (#8) and regenerates the System KB;
- records in `WASM-PROGRESS.md`, per project: the path taken, the fallback reason, the AC19 result and the slowest Build against the 30 s goal.

**Before every long build:** `df -h /` shows at least 10 GB free. Reclaim space from stale `target/` folders and merged branches, never from uncommitted work.

### Refinements found while tasking

The code says five things the plan did not settle. Each is applied in the tasks below.

1. **The golden hashes need repeatable frames.** Today's frames repeat only where nothing on screen depends on time, because `host.rs` reads the real clock. So M0 proceeds in four steps:
   1. It adds the runtime pins (T5) and a deterministic loop in `headless.rs` alone (T6), with no change to `host.rs`.
   2. It records goldens for every form whose frames already repeat, checked by running each script twice (T7).
   3. Then it adds `HostClock` (T8).
   4. Last, it checks those goldens still match and records the rest. Each recording names its forms in its CHANGELOG entry.

   AC19's second guard therefore holds every form that could be measured before the refactor.
2. **The reference projects have no check script.** None of the five has a headless script (checked: no `.json` outside `agentic_ai/`). AC1's "the project's own check script" is therefore the base script of its main form, written in T7.
3. **The console part of R27 needs console programs.** The five reference projects all have forms. So at M1, R27's console part is judged on console fixtures kept in `tests/pixel-parity/console/` (T24). The `tests/cobol` corpus is measured too (AC9).
4. **The 30 s goal is first judged at M2.** No reference project takes the WebAssembly path at M1 (each has forms). The ten-times project has at least 10 indexed files, so it moves at M3 at the earliest. Until a project moves, `build-goal` reports its fallback reason in place of a WebAssembly time.
5. **The Developer's Guide applies now.** `main` has carried the 2.x line since 2026-11-11 (`WASM-PROGRESS.md`, operating rules). So GOLDEN RULE #3 holds: the guide changes in the same task as behaviour a developer sees, and its translations are deleted (#8).
   - The headless script format is not in the guide (checked: no `PRC_HEADLESS` there). The steps `shot` and `size` are documented in `headless.rs`'s step table, beside the other steps.
   - This corrects plan §7, which put them in the guide.

**Commands used below:**

| What | Command |
|---|---|
| generator | `cargo test -p cobolt-wasm <filter>` |
| ABI | `cargo test -p cobolt-wasm-abi` |
| Web bundle | `cargo test -p cobolt-web-bundle` |
| Web fixtures | `cargo test -p cobolt-web-tests <filter>` |
| runtime | `cargo test -p cobolt-runtime <filter>` |
| form host | `cargo test -p cobolt-form-host <filter>` |
| forms | `cargo test -p cobolt-forms --features render` |
| compiler | `cargo test -p cobolt-compiler <filter>` |
| new crates | `cargo test -p cobolt-runner-engine`, `-p cobolt-program-package`, `-p cobolt-runner`, `-p cobolt-abi-host` |
| player (outside the workspace) | `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib` |
| `rcrun` | `cargo build -p cobolt-cli --bin rcrun` |
| `rcrun` host tests | `cargo test -p cobolt-cli --test <name>_hosts` |
| IDE | `cargo test -p cobolt-ide --bin cobolt-ide <filter>` |
| project tools | `cargo test -p cobolt-project-tools <filter>` |
| bench | `cargo run -p cobolt-bench --release -- build-goal …` and `… -- pixel-parity …` |
| NIST on the generator | `cargo run -p cobolt-wasm --example nist_wasm -- census <MOD>` (compile axis); `nist_wasm_run` as `RCRUN` for `nist_conformance run <MOD>` (execution axis) |
| System KB | `cargo run -p cobolt-ide --example build_chunked_kb` |

**The three window sizes of AC19:** the form's designed size; **Medium**, 800 px wide; **Compact**, 480 px wide. Medium and Compact keep the designed height. The widths fall inside the default breakpoints (`cobolt-forms/src/layout/breakpoints.rs`: Compact [0, 600), Medium [600, 1024)).

**Out of scope:** the stale worktree `fixes-2x-motion-lockstep` (another session's staged fix at 2.0.454). It is not touched.

---

## Phase F — The two fixes, landed first (`fixes-2x-wasm-emit-order`)

**Before F1, one question for the operator: how the fixes reach this branch.**
- **The constraint:** this branch is pushed, so CLAUDE.md allows it only `--ff-only`. A fix branch cut from today's `main` can never be fast-forwarded into it, because this branch carries the spec commits.
- **Recommended: every step a fast-forward.**
  1. The operator merges this branch's spec commits (2.0.478–2.0.482: the spec documents, CHANGELOG entries and version number only) into `main`.
  2. The fix branch is cut from that `main`.
  3. Once the fixes are on `main`, this branch follows with `git merge --ff-only main`.
- **Otherwise:** the fixes wait on their branch. Only T22 (determinism across Rust releases) needs them; every other task can go ahead.

- [x] **F1 — The generator emits in a fixed order** (R2; plan §4.9)
  - *Done 2026-11-14 (2.0.483):*
    - **Fixed:** `alter_slots` is a `BTreeMap`.
    - **The audit:** about 50 maps and sets, of which 13 iteration sites were read. One other order-dependent walk was found: `sort_file_names`' `records.iter().find` (lower_idx.rs:101). It is unreachable, because a record name shared by two files is refused at `build_files` ("has no record description"), and the second fixture pins that.
    - **The test:** `compile_determinism` failed on the unfixed tree (the ALTER fixture differed at compile 2 of 20, in both variants). After the fix, 126 of 126 variants give the same outcome every time (38 identical modules, 88 identical refusals; 2 520 compiles in 24.9 s).
    - **Unchanged:** the 18 corpus modules are byte-identical to before. The census gives NC 95/95, IF 45/45, IC 47/47 members.
    - **Reds outside the fix:** the crate's other tests are green except two. `corpus` fails on a timing line its clock filter misses (`conditionals/test-arith-relation-after-and.cbl`). `dependency_guard` expects Rust 1.92 at most, while the workspace has declared 1.95 since 2.0.469.
  - Files:
    - `crates/cobolt-wasm/src/lower.rs`: `alter_slots` becomes a `BTreeMap` (:259), walked by `init_alter_slots` (:1084);
    - every other `HashMap` or `HashSet` walked while emitting, in `crates/cobolt-wasm/src/*.rs`;
    - new `crates/cobolt-wasm/tests/compile_determinism.rs`.
  - Do:
    - **The audit:** list every `HashMap` and `HashSet` iteration in the generator. Each one in the evidence is either changed to an ordered walk or shown to have no effect on the emitted bytes.
    - **The test `same_input_same_bytes`** compiles 20 times each:
      - an ALTER fixture with at least eight `ALTER` targets in three paragraphs;
      - every program of `tests/cobol/` the generator accepts.
    - **Before the change,** record the hash of every module the corpus produces. Modules with no `ALTER` must keep their bytes.
  - Verify:
    - `cargo test -p cobolt-wasm --test compile_determinism -- --nocapture` prints "programs N, compiles 20 each, identical N of N".
    - The ALTER fixture fails this test on the unfixed tree. Run it once before the change and keep its output in the evidence.
    - `cargo test -p cobolt-wasm` green.
    - Every corpus module without `ALTER` is byte-identical to its hash from before the change.
    - The `nist_wasm` census for NC, IF and IC gives the counts recorded in `WASM-PROGRESS.md`.

- [x] **F2 — `SET` pointer is classified as it is compiled** (C13)
  - *Done 2026-11-14 (2.0.484):*
    - **Before:** a program using it was refused at its line with the generic "not yet supported by the WebAssembly generator" (the `stmt` fallback arm), while the table said `Module`. Nothing was miscompiled.
    - **Now:** `BackendOnly(POINTERS)` in both `support()` and `STATEMENT_TABLE`. `subset_table` gives 7 of 7 refusals, the KB is regenerated (4 348 records), and `prebuilt_chunked_kb_matches_the_published_documentation` is green.
  - Files: `crates/cobolt-wasm/src/support.rs` (the `SET` pointer entry), `crates/cobolt-wasm/tests/subset_table.rs`.
  - Do:
    - Record what a program using `SET` pointer does today under the generator: compiled, silently skipped, or wrong code.
    - Reclassify the entry as refused, with the construct named at its line and the alternative, as the other refusals are. Lowering it is the feature's job (T45).
  - Verify:
    - `constructs_outside_the_subset_are_refused_at_their_line_with_the_alternative` gains a `SET` pointer case and is green.
    - `cargo test -p cobolt-wasm --test subset_table` green.
    - The System KB text that reads the table is regenerated, and `prebuilt_chunked_kb_matches_the_published_documentation` is green.

- [ ] **F3 — Land the fixes, and bring them to this branch**
  - Do:
    - Commit F1 and F2 separately on `fixes-2x-wasm-emit-order`. Each takes the next `z` not used on any open branch: this branch holds 2.0.478 onwards.
    - Push outside the window, and ask the operator to merge them into `main`.
    - Bring them to this branch as the operator decided before F1.
  - Verify: `git log main --oneline` shows both fixes; `cargo test -p cobolt-wasm --test compile_determinism` green on this branch before T22 starts.

## Phase M0 — The baseline, the deterministic mode and the golden hashes (the WebAssembly path is off)

- [ ] **T1 — The Web module's hash, recorded before anything changes** (R24; AC16)
  - Files: new `crates/cobolt-web-tests/tests/desktop_wasm_unchanged.rs`; its fixture `crates/cobolt-web-tests/tests/fixtures/power-analytics-web.sha256`.
  - Do: build the Web module of `examples/PowerAnalyticsWeb` (from a copy, never in place), and pin the sha256 of `program.wasm` and of its manifest.
  - Verify: `cargo test -p cobolt-web-tests --test desktop_wasm_unchanged -- --nocapture` prints both hashes and passes. It stays green through every later task.

- [ ] **T2 — The Build reports its phases and its peak memory** (R18, R20; AC7 harness)
  - Files:
    - `crates/cobolt-compiler/src/lib.rs`:
      - `BuildProgress` (:1834) gains `phase: BuildPhase` and `elapsed`;
      - `BuildResult` (:1929) gains `phases: Vec<(BuildPhase, Duration)>`, `peak_rss: Option<u64>` and `path: BuildPath`, which is always `Cargo` for now;
      - a sampler thread reads resident memory every 25 ms with `sysinfo` (already in `Cargo.lock`), added to `cobolt-compiler`'s `Cargo.toml`.
    - The readers of `BuildProgress` in `cobolt-ide` and `cobolt-project-tools` compile against the new fields, unchanged in what they show.
  - Do: the phases are `FrontEnd`, `Regenerate`, `Classify`, `Generate`, `MachineCode`, `Assemble`, `Cargo` and `Install`. A phase that did not run is absent.
  - Verify:
    - A new compiler test, `build_reports_phases_and_peak` (`-- --nocapture`), builds a one-form fixture on the Cargo path. Its result block prints each phase in ms, their sum against the wall time, and the peak in MB. The sum is ≤ the wall time, and the peak is > 0.
    - The three existing build tests of plan §6 are green.
    - `cargo build -p cobolt-ide -p cobolt-project-tools -p cobolt-cli` succeeds.

- [ ] **T3 — The ten-times reference project** (R26; AC18 generation)
  - Files: new `crates/cobolt-bench/src/ten_times.rs`, with its tests in the same file.
  - Do:
    - **Measure the five reference projects** (C7): forms, COBOL lines and indexed files.
    - **Write a project** with ten times the largest of each dimension, and at least 10 indexed files.
    - **Keep it deterministic:** ordered maps only, a fixed seed, `/` in every path, and no timestamp in any file.
    - **Use only names and constructs the reference** (`kb_lookup`) lists for a Desktop project.
  - Verify:
    - `cargo test -p cobolt-bench ten_times -- --nocapture` prints:
      - each project's dimensions;
      - the target dimensions;
      - the generated dimensions;
      - the sha256 of the whole tree, equal across two generations.
    - `check` on the generated project reports no error.

- [ ] **T4 — `cobolt-bench build-goal`, and the baseline** (R17, R19, R20; AC6 and AC7 harness, AC18 hash)
  - Files:
    - `crates/cobolt-bench/src/main.rs`: a `build-goal` mode, dispatched before the existing filter;
    - new `crates/cobolt-bench/src/build_goal.rs`;
    - `crates/cobolt-bench/Cargo.toml`: depends on `cobolt-compiler`;
    - new `specs/WASM-BUILD-GOAL.json`, written by `--record`;
    - new `.github/workflows/build-goal.yml`: the four platforms, on tags and on dispatch.
  - Do:
    - **Each run:** copy every reference project, the five examples and the ten-times project, into a temporary folder, never building an example in place. Build each one five times with no cache: a fresh target folder and the shared build cache off. Time the generator alone where it accepts the project, and name the construct where it refuses.
    - **Flags:** `--runs N` shortens a local run; the default is 5.
    - **The result block,** per platform and project, gives:
      - the path taken;
      - the slowest Build against 30 s;
      - each phase in ms;
      - the peak memory;
      - the generator's time or its refusal;
      - the ten-times project's hash.
    - **The macOS arm64 job** runs on a larger or self-hosted runner. Otherwise it records the deviation (C9).
  - Verify:
    - `cargo run -p cobolt-bench --release -- build-goal --record` prints the result block and writes the JSON.
    - The numbers from this machine go into `WASM-PROGRESS.md` with its cores and memory, named as "not a reference box" where it is not one.
    - The CI job is dispatched once, outside the window. Its four result blocks are recorded, and the four ten-times hashes are equal (AC18).

- [ ] **T5 — The run clock and the random seed, pinned for tests** (R28, the program half)
  - Files:
    - new `crates/cobolt-runtime/src/test_pins.rs`: read once from `PRC_TEST_CLOCK` (RFC 3339) and `PRC_TEST_SEED` (u64), with a `set_for_test` for tests, which never touch the environment;
    - `crates/cobolt-runtime/src/interpreter.rs`: the clock sites at :19016 and :21041–21174;
    - `crates/cobolt-stdlib/src/intrinsics.rs`: `RANDOM`'s seed at :170, and `CURRENT-DATE`.
  - Do:
    - **The audit:** every read of the system clock or of a random source in the two crates goes through the pins, or is listed in the evidence as never reaching a program.
    - **When pinned,** the clock starts at the pinned instant and moves only when the form host advances virtual time (T8). A console program sees the pinned instant.
    - **When the variables are unset,** nothing changes.
  - Verify:
    - The new `cargo test -p cobolt-runtime --test pinned_clock_and_seed -- --nocapture` checks, with pins, that these are equal across two runs and to the expected values:
      - `FUNCTION CURRENT-DATE`;
      - `ACCEPT … FROM DATE`, `DAY`, `DAY-OF-WEEK` and `TIME`;
      - 1 000 values of `FUNCTION RANDOM`.
    - `cargo test -p cobolt-runtime` green.
    - The compiled binary takes the pins through the same crate. A Cargo-built fixture run with `PRC_TEST_CLOCK` set prints the pinned date (interpreter-binary parity).

- [ ] **T6 — The deterministic headless loop, and the steps `shot`, `size` and `answer`** (R28, the headless half)
  - Files:
    - `crates/cobolt-form-host/src/headless.rs`: the loop at :130–162 and the step table at :20–27;
    - `crates/cobolt-form-host/src/file_dialog.rs` and the message-box path: scripted answers;
    - `crates/cobolt-compiler/src/lib.rs`: the generated `main.rs` at :4873 passes `PRC_HEADLESS_SHOTS`;
    - new `crates/cobolt-form-host/tests/headless_shots.rs`.
  - Do:
    - **With `PRC_TEST_CLOCK` set,** each frame advances virtual `t` by 1/60 s, nothing sleeps, and `wait_ms` advances virtual time. `host.rs` is not touched yet.
    - **`{"shot": "<name>"}`** writes the current frame to `PRC_HEADLESS_SHOTS/<name>.png` with the CPU rasteriser (`cobolt_forms::raster`).
    - **`{"size": [w, h]}`** resizes the window.
    - **`{"answer": {…}}`** queues the answer for the next message box or file dialog, so no run waits on a person.
    - **The step table** in the module comment gains the three steps.
  - Verify:
    - `cargo test -p cobolt-form-host --test headless_shots -- --nocapture` (`shot_and_size_steps_write_pictures`) checks that:
      - three shots at two sizes give PNGs of those sizes;
      - a queued answer closes a message box;
      - a file dialog takes its scripted path.
    - With the variables unset, `headless_dump` and `headless_pointer_click` are green and unchanged.

- [ ] **T7 — The pixel-parity harness, Cargo only; the base scripts; the first golden hashes** (R27 harness; AC19 second guard, AC20 per form; AC1 check scripts)
  - Files:
    - new `crates/cobolt-bench/src/pixel_parity.rs` (a `pixel-parity` mode);
    - new `tests/pixel-parity/scripts/<project>/<form>.json` for the five reference projects;
    - new `tests/pixel-parity/golden/macos-aarch64.toml`;
    - new `.github/workflows/pixel-parity.yml`: the four platforms, on tags and on dispatch, with `--record` on dispatch only.
  - Do:
    - **`pixel-parity --write-scripts`** makes each form's base script:
      - a `shot` after every step;
      - every bound event raised once, in document order;
      - a handler that ends the form or the program goes last, and is named in the script's comment field;
      - a dialog gets an `answer` step.
    - **The script runs three times:** at the designed size, then after `{"size": [800, h]}`, then after `{"size": [480, h]}`.
    - **`pixel-parity --cargo`** builds each project on the Cargo path (from a copy) and runs every script twice under the pins. Two outcomes per form:
      - frames that repeat: their sha256 (of the RGBA buffer) is recorded with `--record`;
      - frames that do not: the form is named as "not repeatable before `HostClock`".
    - **On a mismatch,** the diff image and both frames go under `target/pixel-parity/`.
  - Verify:
    - `cargo run -p cobolt-bench --release -- pixel-parity --cargo --record` prints, per project:
      - the forms, steps and sizes;
      - frames compared, and frames identical across the two runs;
      - the forms not yet repeatable.
    - The golden file is written. A second run without `--record` matches every golden.
    - The CI job is dispatched once, outside the window, and the three other platforms' goldens are committed from its artefacts.

- [ ] **T8 — `HostClock`, then the rest of the golden hashes** (R28; AC20)
  - Files:
    - new `crates/cobolt-form-host/src/clock.rs`;
    - `crates/cobolt-form-host/src/host.rs`: its 37 `Instant::now` calls (timers, animations, entrance and exit effects, snackbars);
    - `crates/cobolt-form-host/src/snackbar_stack.rs`, where it reads the time;
    - new `crates/cobolt-form-host/tests/deterministic_mode.rs`.
  - Do:
    - **`HostClock`** is real by default, and virtual when `PRC_TEST_CLOCK` is set. The headless loop of T6 drives it, and the runtime clock of T5 follows its virtual time.
    - **The refactor changes nothing** with the variables unset.
  - Verify:
    - `cargo test -p cobolt-form-host --test deterministic_mode -- --nocapture` (`deterministic_mode_repeats_frames`). A fixture form has a timer, an entrance animation and a snackbar. Run twice, it prints "frames N, identical N".
    - `cargo test -p cobolt-form-host` and `cargo test -p cobolt-forms --features render` green. `engine_reference_form_parity_static_vs_faces` green.
    - `pixel-parity --cargo` without `--record` matches every golden from T7: the refactor moved no pixel.
    - Then, with `--record`, it adds the forms that now repeat. Their names are listed in this task's CHANGELOG entry.
    - The `rcrun` host tests stay green (`side_menu_hosted_controls`, `powercloud_hosts` and the others).

## Phase M1 — Console programs on the WebAssembly path, on by default

- [ ] **T9 — The manifest's capabilities, the ABI range and the bundler's refusal** (R3, R4 web half, R5; AC5 web half, AC16)
  - Files:
    - `crates/cobolt-wasm-abi/src/manifest.rs`: `capabilities`, serialised only when not empty; `validate` (:241) accepts `1 ≤ abi ≤ ABI_VERSION` (C2);
    - `crates/cobolt-wasm-abi/src/codes.rs`: `NATIVE_IMPORTS`, namespace `prn`;
    - `crates/cobolt-wasm-abi/src/scan.rs`: `bundle_problems` names a native-only capability;
    - `specs/WASM-ABI-v1.md`: the §10 amendment, and §8 corrected to the code (C10).
  - Verify:
    - `cargo test -p cobolt-wasm-abi` green, with new tests:
      - a manifest with no capabilities is byte-identical to before (fixture bytes);
      - `validate` accepts 1 and `ABI_VERSION`, and refuses 0 and `ABI_VERSION + 1`;
      - `bundle_problems` names `files.indexed` in a module that declares it.
    - `cargo test -p cobolt-web-bundle` and T1's test green.
    - The player `--lib` is green. The shipped player is rebuilt if its freshness test asks for it, in the same commit.

- [ ] **T10 — The native column of the subset table** (R12; AC10)
  - Files:
    - `crates/cobolt-wasm/src/support.rs`: every entry of `STATEMENT_TABLE` and `BUILTIN_TABLE` gains `native: Native { Module, Host(Cap), Cargo(&'static str) }`;
    - `crates/cobolt-wasm/tests/subset_table.rs`;
    - `crates/cobolt-compiler/src/lib.rs`: a Desktop column in `browser_subset_doc`;
    - `assets/knowledge/chunked.data`.
  - Do: classify each entry for the Desktop kind as it stands at M1. What the WebAssembly path cannot do yet is `Cargo("<reason>")`.
  - Verify:
    - `every_statement_and_builtin_has_a_native_class` and a planted-unknown twin green.
    - The KB is regenerated, and `prebuilt_chunked_kb_matches_the_published_documentation` is green.

- [ ] **T11 — WHEN-COMPILED and paths as Build inputs** (R2; C3)
  - Files: `crates/cobolt-wasm/src/lib.rs` (`Options.compiled_at`), the WHEN-COMPILED lowering, and the `prlines` writer.
  - Do:
    - `compiled_at` comes from `SOURCE_DATE_EPOCH` when it is set, and from the Build's clock otherwise.
    - Every path in `prlines` is written relative and with `/`.
  - Verify:
    - `compile_determinism` gains a case: with the same `SOURCE_DATE_EPOCH`, a program using WHEN-COMPILED gives identical bytes, built from two folders with different separators.
    - T1's test green.

- [ ] **T12 — `Options.native` and the console capabilities in the generator** (R3, R10, R11 console part)
  - Files:
    - `crates/cobolt-wasm/src/lib.rs` (`Options.native`), the `lower*.rs` files it reaches, and `module.rs` (the manifest's capabilities);
    - new `crates/cobolt-wasm/tests/native_console.rs`.
  - Do:
    - With `native` set, these are lowered to `prn` imports:
      - `ACCEPT` from the console → `con_read`;
      - the environment and the arguments → `env_get`;
      - the exit code and `RETURN-CODE` at `STOP RUN` → `exit`.
    - In-memory INDEXED and `SORT` stay in the module.
    - The capabilities list names exactly what the program reaches.
    - With `native` unset, the generator refuses these as it does today.
  - Verify:
    - `cargo test -p cobolt-wasm --test native_console -- --nocapture`: one program per construct compiles, and its manifest lists exactly its capabilities. The same program without `native` gives today's refusal.
    - `cargo test -p cobolt-wasm`, `compile_determinism` and T1's test green.

- [ ] **T13 — `cobolt-runner-engine`: one wasmtime setup, machine code and its cache** (R6, R15, R16; AC8 engine half)
  - Files:
    - new `crates/cobolt-runner-engine/` (a workspace member);
    - `crates/cobolt-wasm/src/host.rs`: `engine()` delegates to it.
  - Do:
    - **One `Config`** for the tests and the runner: multi-memory, NaN canonicalisation, an explicit target, parallel compilation.
    - **`precompile(module, target)`** targets x86-64-v2, and aarch64 at the Apple M1 level.
    - **`load(module, code, cache)`** goes in this order: sha256 → compatibility hash → `deserialize` → the local cache (least recently used, 512 MiB) → compile at load. It returns the path it took.
    - **The M1 spike:** does a baseline `.cwasm` load in an engine that detects the host's CPU? The answer, and the settings it forces, go in the evidence.
  - Verify:
    - `cargo test -p cobolt-runner-engine -- --nocapture`. Each of these runs, gives the same result and names its path:
      - a foreign compatibility hash;
      - truncated code;
      - absent code;
      - a cache hit on the second load.

      The result block gives each path's ms.
    - `cargo test -p cobolt-wasm` green.
    - `cargo tree -d -p cobolt-runner-engine` shows one `wasmtime`, `=44.0.3`.

- [ ] **T14 — `cobolt-program-package`: the payload inside one executable** (R1, R8)
  - Files: new `crates/cobolt-program-package/` (a workspace member).
  - Do:
    - **The table of contents:** `{kind, offset, len, sha256}` entries, kinds `MODULE` and `MACHINE_CODE`.
    - **The carriers:** an overlay on ELF and PE, found through `PRLOC`; a `__PRPAYLOAD` segment on Mach-O, signed again ad hoc in pure Rust.
    - **`RunnerSettings`:** the type only; T28 fills it from the project.
    - **Evaluate `libsui`** against the hand-written carriers, and record the choice in the evidence.
  - Verify:
    - `cargo test -p cobolt-program-package -- --nocapture`:
      - a round trip on a copy of the test's own executable, on this platform's carrier;
      - a damaged table of contents is refused;
      - a payload whose sha256 does not match is refused.
    - On macOS, `codesign --verify --strict` passes on the result.
    - The other carriers run in `build-all-platforms.yml`'s test step on their systems.

- [ ] **T15 — `prc-runner` for console programs** (R4 runner half, R5, R6, R10, R16, R28 runner half; AC5 runner half)
  - Files: new `crates/cobolt-runner/` (a workspace member; binary `prc-runner`).
  - Do:
    - **Loading:** read the payload through `PRLOC`, then validate the manifest's range.
    - **Capabilities:** refuse any it does not provide, naming it. Messages come in six languages, chosen by the system locale, and never say "cobolt".
    - **Running:** load the machine code through T13, link `pr` and the console part of `prn` with `linker_for(abi, imports)`, run, and exit with the program's code.
    - **Test pins:** `now` and `random` come from `PRC_TEST_CLOCK` and `PRC_TEST_SEED`.
  - Verify:
    - `cargo test -p cobolt-runner -- --nocapture`:
      - a console fixture module, packaged with T14 into the built `prc-runner`, gives under the pins the same output as `rcrun run` of the same program, byte for byte;
      - a module that declares `sql` is refused, naming `sql`;
      - a module of ABI 1 runs;
      - the refusal text exists in six languages and contains no "cobolt".

- [ ] **T16 — The Build classifies and takes the WebAssembly path** (R1, R7, R8, R13, R14, R21, R24, R25; AC1 local half, AC11, AC12, AC13, AC15, AC17 compiler half; C8, C11)
  - Files:
    - `crates/cobolt-compiler/src/lib.rs`:
      - `classify`, in plan §4.7's order;
      - `wasm_build`;
      - `finish_install`, factored out of :3244–3338;
      - `CargoReason`;
      - `CompilerError::CargoNeedsToolchain`;
      - the toolchain check, moved after `classify` (:2219–2223);
      - `BuildResult.cargo: Option<…>`;
      - the runner locator, built like `cobolt-project-tools/src/web/preview.rs::helper_path` (:212);
    - `crates/cobolt-compiler/src/project_kind.rs`: `DesktopSettings` (:1059) gains `build_path` (default `Wasm`) and `machine_code` (default `true`).
  - Do:
    - **The reasons:** Debug → `DebugBuild`; `build_path = "cargo"` → `Chosen`; `[[crates]]` → `ExternalCrate`; `EXEC RUST` → `ExecRust { file, line }`; a generator refusal → `Refused { construct, file, line }`; no runner → `RunnerUnavailable`.
    - **Not yet covered:** a construct whose native class (T10) is `Cargo(_)` is a `Refused` that carries that class's reason. At M1 this is how every form project falls back.
  - Verify: `cargo test -p cobolt-compiler` green, with these new tests:
    - **AC1 (local half):** a console project builds with `PATH` holding no `cargo` or `rustc`, `CARGO_HOME` and `RUSTUP_HOME` pointing at empty folders, and `bin/<name>` runs.
    - **`exec_rust_falls_back_before_compiling` (AC12):** the message names the block's file and line before any compile.
    - **`refused_construct_falls_back_naming_file_and_line` (AC11).**
    - **`no_toolchain_stops_and_leaves_bin_untouched` (AC17):** `bin/`'s tree hash is equal before and after.
    - **The Debug Build** takes Cargo with `DebugBuild`; the chosen setting gives `Chosen`; a missing runner gives `RunnerUnavailable`.
    - **The three twins:** lands in the chosen folder; a missing toolchain is named, now a typed error; current code, not a stale generated file.
    - **`no_project_source_in_built_file` (AC15):** no `PROCEDURE DIVISION` statement line of the project is found in the built file. Literals, data names and `EXEC SQL` text are excluded (C1).
    - **AC13:** `bin/` holds one executable plus the assets and licences the Cargo path writes, and `dist/` is copied as today.
    - `cargo build -p cobolt-ide -p cobolt-project-tools -p cobolt-cli` succeeds.

- [ ] **T17 — `rcrun build --cargo`, `--no-machine-code`, and the install hint** (R15, R24, R25; A4; AC8 first half, AC17 CLI half)
  - Files: `crates/cobolt-cli/src/main.rs`.
  - Verify: new `cobolt-cli` tests, with `rcrun` built first:
    - `--cargo` gives path Cargo, reason `Chosen`;
    - `--no-machine-code` gives a file whose table of contents has no `MACHINE_CODE` entry, while the default Build's has one;
    - with no toolchain, an `EXEC RUST` project prints how to install Rust 1.95.0, exits non-zero, and leaves `bin/` untouched.

- [ ] **T18 — The IDE: the Build's path, its phases, the install offer, and the Desktop settings** (R18, R24, R25; AC17 IDE half; i18n)
  - Files:
    - `crates/cobolt-ide/src/app.rs`:
      - the toolchain failure, today matched by its text (:17237), becomes the typed error;
      - `offer_rust_install(reason)` reuses the first-run offer (:2616) at Build time;
      - the Output shows the path, the reason and the phase times;
    - `crates/cobolt-ide/src/panels/new_project.rs`: the Desktop option rows (:508–527, `pk_o_*` and `pk_c_*`);
    - Project Settings' Desktop section, found from the readers of `DesktopSettings`;
    - `crates/cobolt-ide/src/i18n.rs`.
  - Do: add new `Tr` keys in all six languages for:
    - the two settings and their choices;
    - the eight phase names;
    - the seven reasons;
    - the path line;
    - the offer's title, text and button.
  - Verify:
    - The new `cargo test -p cobolt-ide --bin cobolt-ide build_path_report`:
      - a typed failure opens the offer;
      - the report's lines come out in six languages;
      - a project without the new settings reads the defaults and is not rewritten.
    - `no_empty_ui_translations` and `non_english_is_actually_translated` green.
    - The manual check (plan §6) is owed to the operator: an IDE Build on a machine with no toolchain. The window is never driven by an agent; the MCP `build` tool shows the same report (T19).

- [ ] **T19 — The project tools' `build` reports the path, the reason and the timings** (R13, R18)
  - Files: `crates/cobolt-project-tools/src/tools/` (the `build` tool), and its schema text.
  - Verify: `cargo test -p cobolt-project-tools build` green. On fixtures, the tool's result names:
    - the path;
    - the reason, with the file and line for `ExecRust` and `Refused`;
    - each phase in ms;
    - the peak.

- [ ] **T20 — `prc-runner` ships with the IDE** (R6; AC13)
  - Files:
    - `.github/workflows/build-all-platforms.yml`:
      - build `cobolt-runner`;
      - stage `prc-runner` beside the IDE, as `prc-web-preview` is at :235 and :302;
      - sign it ad hoc on macOS, as at :351;
      - list it in the installers' file lists;
    - `THIRD_PARTY_NOTICES.md`: wasmtime and its dependencies;
    - `crates/cobolt-compiler/src/lib.rs`: `SDK_CRATES` (:3833) and `stage_sdk`, if the Platform SDK's IDE builds Desktop projects (decided from `stage_sdk`'s readers, and recorded).
  - Verify:
    - The runner locator finds `prc-runner` next to the IDE binary and, in a checkout, in `target/<profile>/`. This is a compiler test.
    - The release workflow is dispatched once, outside the window. Each of the four packages contains `prc-runner`, and on macOS `codesign --verify --strict` passes.
    - The package step and the installers are otherwise unchanged (AC13).

- [ ] **T21 — The module corpus, kept for ever** (R5, R23; AC4)
  - Files:
    - new `tests/module-corpus/`: an index of every entry ever added, and its first entry, the console fixtures built by this release, with their expected outputs;
    - new `crates/cobolt-runner/tests/module_corpus.rs`.
  - Do: each release adds one entry within 256 KiB. The index is append-only.
  - Verify: `cargo test -p cobolt-runner --test module_corpus -- --nocapture`:
    - **`corpus_runs_on_this_runner`:** every entry runs and gives its recorded output. The result block counts releases, modules and identical outputs.
    - **`corpus_entries_are_never_deleted`:** an entry in the index with its files missing fails, and a planted case proves it.

- [ ] **T22 — CI: no toolchain, Rust releases, and across platforms** (R2, R5; AC1, AC2, AC3 console)
  - Files: new `.github/workflows/desktop-wasm.yml`, on tags and on dispatch.
  - Do:
    - **`no-toolchain`, on the four platforms:** a clean runner with Rust removed from `PATH`, `~/.cargo` and `~/.rustup`. It installs the release's IDE package, builds every project that takes the WebAssembly path at this milestone, and runs its check script.
    - **`determinism`:** generators built by Rust 1.95.0 and by `stable` compile the corpus and the reference projects with the same `SOURCE_DATE_EPOCH`, then compare the bytes. **Waits for F3.**
    - **`cross-platform`:** modules built on each platform run under the other three platforms' runners, compared as AC3 says.
  - Verify: dispatched once, outside the window; each job's result block is recorded in `WASM-PROGRESS.md`. AC1 is met only once every reference project takes the WebAssembly path. Until then the job lists those still on the Cargo path, with their reasons.

- [ ] **T23 — AC9 at M1, and the parity list** (R11, R21, R22; AC9)
  - Files: new `crates/cobolt-runner/examples/nist_runner_run.rs` (the `nist_wasm_run` pattern, with the module run by `prc-runner`); `specs/WASM-PARITY.md`.
  - Do: measure on both axes (GOLDEN RULE #9):
    - the NIST modules the interpreter has finished, in the native mode;
    - `tests/cobol` against the interpreter's output.

    Each divergence found goes into `WASM-PARITY.md`, with who is right.
  - Verify: the compile and execution counts per module, reported separately, plus the corpus's identical-output count, are written to `WASM-PROGRESS.md`. Nothing gates on them (R21).

- [ ] **T24 — Console parity, byte for byte** (R27 console part)
  - Files: new `tests/pixel-parity/console/` (console fixtures using each M1 construct); `crates/cobolt-bench/src/pixel_parity.rs` (`--console`).
  - Verify: `cargo run -p cobolt-bench --release -- pixel-parity --console` builds each fixture on both paths, runs both under the pins, and prints "programs N, identical output N of N". Any difference is a failure.

- [ ] **T25 — The Developer's Guide and the System KB for M1** (GOLDEN RULE #3, #8; plan §7)
  - Files:
    - `docs/developers-guide-en.md` §18, "Building a distributable binary":
      - the Desktop Build's default path;
      - the machine code and the setting that turns it off;
      - choosing the Cargo path (Project Settings, `rcrun build --cargo`);
      - what still takes the Cargo path and why;
      - what happens with no Rust toolchain;
    - §17: the new `rcrun build` options;
    - delete the guide's five translations;
    - the compiler's doc tables for the settings and options;
    - `assets/knowledge/chunked.data`.
  - Do: write it in the guide's voice, for COBOL developers, with no Rust. Add a mermaid diagram of the two paths and a Caveat on the known divergences (`WASM-PARITY.md`).
  - Verify:
    - `prebuilt_chunked_kb_matches_the_published_documentation` green.
    - `git status` shows the five translations deleted, never the English file.

- [ ] **T26 — M1 close**
  - Do: the milestone close (preamble).
  - Verify:
    - `cargo test -p cobolt-wasm -p cobolt-wasm-abi -p cobolt-runner-engine -p cobolt-program-package -p cobolt-runner -p cobolt-compiler --no-fail-fast`, every `test result:` line read.
    - `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`: only the known reds of the 2.x line.
    - T1's test green.
    - `build-goal` names each reference project's fallback ("forms").
    - The runner's size goes into `WASM-PROGRESS.md` (C12).

## Phase M2 — Forms

- [ ] **T27 — `cobolt-abi-host`, extracted from the player unchanged** (R9; plan §4.3)
  - Files:
    - new `crates/cobolt-abi-host/`: the player's pure `Host`, `FormTree` (`tree.rs`), `methods.rs` and their closure, moved with their tests;
    - `crates/cobolt-web-player/Cargo.toml`: a path dependency, plus a re-export;
    - `crates/cobolt-web-player/src/fingerprint.rs`: `INPUTS` (:33) gains the new crate.
  - Verify:
    - The player `--lib` gives the same pass count as before the move, and is green.
    - `cargo test -p cobolt-abi-host` green.
    - The player's `wasm32` build succeeds. The shipped player is rebuilt if the freshness test asks.
    - T1's test green.

- [ ] **T28 — The forms, menus, settings and art inside the module** (R1; C1; AC15, AC16)
  - Files:
    - `crates/cobolt-compiler/src/lib.rs`:
      - `desktop_form_specs`;
      - `RunnerSettings` made by the one function that also feeds the settings baked into `main.rs` (:3059–3084);
    - `crates/cobolt-project-tools/src/web/generator.rs` (:149–295) uses the shared builder; its browser-only lowering (:251–268) stays its own;
    - `crates/cobolt-wasm/src/module.rs`: the sections `prforms`, `prmenus`, `prsettings` and `prart`, each with its own version.
  - Verify:
    - T1's test green: the Web module's bytes are unchanged.
    - A compiler test: for each reference project, the settings baked on the Cargo path equal the `RunnerSettings` in the module.
    - `no_project_source_in_built_file` covers a form project.
    - `cargo test -p cobolt-project-tools` green.

- [ ] **T29 — `ProgramEngine` in the form host; today's path becomes `InterpreterEngine`** (R9; C5)
  - Files: `crates/cobolt-form-host/src/host.rs`: `build_form_instance_as` (:5314), and the interpreter thread (:5379–5447) behind the trait.
  - Verify:
    - `cargo test -p cobolt-form-host` green.
    - Every `rcrun` host test green.
    - `pixel-parity --cargo` matches every golden: the refactor moved no pixel.
    - `engine_reference_form_parity_static_vs_faces` green.

- [ ] **T30 — `WasmEngine`: one form** (R9, R10, R28 runner half)
  - Files: `crates/cobolt-runner/src/` (`WasmEngine`, one program thread, the ABI host from T27).
  - Do:
    - Run one form: its controls, properties, methods, events and timers.
    - The runner honours `PRC_HEADLESS_SCRIPT`, `PRC_HEADLESS_LIMIT` and `PRC_HEADLESS_SHOTS`, as `main.rs` does (:4873–4874).
  - Verify:
    - A new `cobolt-runner` test, `interpreter_and_wasm_engine_agree_on_one_form` (`-- --nocapture`), runs a fixture form headless under both engines and compares their dumps after every step (AC14 part).
    - The fixture's `shot` frames are equal in every pixel.

- [ ] **T31 — `WasmEngine`: several forms, the shell and the SideMenu** (R9, R10)
  - Do: `OpenFormSync`, `OpenFormAsync`, `super::`, the application shell with its SideMenu, ContentPane and breadcrumb.
  - Verify: runner twins of the `rcrun` host tests `side_menu_hosted_controls`, `side_menu_rows_hosts`, `layers_hosts_091` and `perspective_hosts_096`, equal in their dumps and frames.

- [ ] **T32 — `WasmEngine`: dialogs, file dialogs and the scripted answers** (R10)
  - Do: message boxes and file dialogs run under the runner, and the `answer` step of T6 drives them headless.
  - Verify: a runner test where a fixture opens each dialog kind, answered by script, gives equal dumps and frames under both engines.

- [ ] **T33 — Desktop controls the ABI host lacks fall back by name** (R13; plan §4.3)
  - Files: `crates/cobolt-compiler/src/lib.rs` (`classify`: `DesktopControl { form, control }`), and the native column (T10) updated for what M2 covers.
  - Verify: a compiler test with a form holding one control the ABI host lacks. It builds on the Cargo path, and the message names the form and the control. The AC10 test stays green.

- [ ] **T34 — Pixel parity for forms** (R27; AC19)
  - Files: `crates/cobolt-bench/src/pixel_parity.rs` (the pairs mode, the default), and `.github/workflows/pixel-parity.yml`.
  - Do: for every reference project the WebAssembly path builds:
    - build it both ways;
    - run every script under both binaries, at the three sizes, with the same pins;
    - compare every pair exactly, and the reads and dumps.
  - Verify:
    - `cargo run -p cobolt-bench --release -- pixel-parity` prints, per project:
      - the path taken;
      - forms, steps and sizes;
      - frames compared;
      - differing pixels, which must be 0;
      - golden matches.
    - A project counts as moved only when it shows 0 differing pixels.
    - The CI job is dispatched once on the four platforms, outside the window, and its blocks are recorded.

- [ ] **T35 — The same form on every surface and on every platform** (R2, R5, R9; AC3 forms, AC14)
  - Verify:
    - **AC14:** a test renders a reference form in the designer's render path, under `rcrun run-form` and under the runner, and gets equal pixels. `engine_reference_form_parity_static_vs_faces` green.
    - **AC3:** the `cross-platform` job of T22 gains the form projects that moved.

- [ ] **T36 — M2 close**
  - Do: the milestone close (preamble).
  - Verify:
    - The first 30 s judgement on the WebAssembly path, for each reference project that moved (AC6).
    - The guide's fallback list is updated.
    - The sweeps of T26, plus `cargo test -p cobolt-form-host -p cobolt-abi-host` and `cargo test -p cobolt-forms --features render`.

## Phase M3 — Disk files, and `SORT` and `MERGE` with files

- [ ] **T37 — `SequentialFile` and the sort core, extracted from the interpreter unchanged** (C5)
  - Files: new `crates/cobolt-runtime/src/seqfile.rs`, the sort core beside it, and the interpreter's file and `SORT` code calling them.
  - Verify:
    - `cargo test -p cobolt-runtime` green.
    - The NIST execution runs of SQ, IX, RL and ST give the counts recorded for the interpreter (GOLDEN RULE #9: every finished module's execution pass).

- [ ] **T38 — Files of every organisation as capabilities** (R10, R11)
  - Files:
    - `crates/cobolt-wasm/src/` (the file verbs lowered to the `prn` `f_*` imports under `native`);
    - `crates/cobolt-runner/src/` (the `files.*` capabilities over `SequentialFile`, `RelativeFile` and `IndexedStore`);
    - the native column.
  - Do: records cross by `(ptr, len)` in memory 0. `FILE STATUS` values are the interpreter's.
  - Verify: one runner test per organisation and verb. Its file contents and `FILE STATUS` values are equal to the interpreter's.

- [ ] **T39 — `SORT` and `MERGE` with `USING`/`GIVING`; `COMMIT`, `ROLLBACK`, `UNLOCK`** (R11)
  - Verify: runner tests equal to the interpreter's output and files. The native column moves these entries to `Host(sort)` or `Host(files.*)`.

- [ ] **T40 — M3 close** (AC9 for SQ, IX, RL, ST)
  - Verify:
    - The NIST SQ, IX, RL and ST counts on both axes through the generator and the runner, reported separately.
    - The ten-times project's path and time.
    - The milestone close.

## Phase M4 — SQL and HTTP

- [ ] **T41 — `EXEC SQL`, the SQL object and the SqlDatabase control** (R10, R11)
  - Files: the `EXEC SQL` binding, extracted from `cobolt-runtime` unchanged (C5); `sql_exec`, with the statements in a `prsql` section; the runner's `sql` capability.
  - Verify: runner twins of the interpreter's `EXEC SQL` and backend tests give equal results. `cargo test -p cobolt-runtime` green.

- [ ] **T42 — HTTP and RestClient** (R10, R11)
  - Do: `http_call` suspends and resumes the module as `remote` does.
  - Verify: a runner test against a local stub server gives the interpreter's results. No reference script calls the network.

- [ ] **T43 — M4 close**
  - Do: the milestone close (preamble).

## Phase M5 — National and UTF-8 data, and the rest of the table

- [ ] **T44 — National and UTF-8 data, computed in the module** (R11; C4)
  - Files: the national code extracted from `cobolt-runtime` unchanged; its lowering in `cobolt-wasm` and in `cobolt-wasm-rt`; the blob rebuilt with `cargo run -p cobolt-wasm --example build_rt`.
  - Verify: twins of the interpreter's national tests give identical bytes. `rt_blob` green.

- [ ] **T45 — `SET` pointer, and every other `Cargo(_)` entry but `EXEC RUST`** (R11, R12; AC10)
  - Do: lower `SET` pointer (reclassified by F2) and each remaining entry the native column sends to Cargo.
  - Verify: the AC10 test gains an assertion that `EXEC RUST` is the only `Cargo(_)` entry left. Each entry has a runner test equal to the interpreter.

- [ ] **T46 — M5 close**
  - Do: the milestone close (preamble).

## Phase M6 — The non-visual controls

Each task here moves its controls out of the `DesktopControl` fallback. It is verified by runner twins of the `rcrun` host tests for those controls, and by pixel parity on every reference project that uses them.

- [ ] **T47 — AgentObject, KnowledgeBase and WebSearch** (R11)
  - Verify: twins of `model_list_hosts` and the agent and KB host tests, against local stubs.

- [ ] **T48 — Maps, the Aws controls, model lists and MCP** (R11)
  - Verify: twins of `aws_hosts`, `partiql_hosts` and `powercloud_hosts`, against local stubs. No twin sends a request signed with the live keys some example `.cfrm` files carry.

- [ ] **T49 — AUDIO, ANIMATIONSEQUENCE and LAYOUT** (R11)
  - Verify: runner twins equal in dumps and frames, with animations under the virtual clock.

- [ ] **T50 — Viewer verbs, the window methods and canvas frames** (R11)
  - Verify: runner twins. The ABI host's list of unimplemented window methods is empty.

- [ ] **T51 — M6 close** (R11's end state)
  - Verify:
    - The `DesktopControl` fallback is reached by no reference project.
    - Every reference project takes the WebAssembly path and passes AC19, so AC1 is met.
    - The milestone close.

## Phase Z — Closing

- [ ] **T52 — i18n audit** (spec §6)
  - Verify:
    - Every `Tr` key added by T18 and later exists in EN ES PT JA ZH FR.
    - `no_empty_ui_translations` and `non_english_is_actually_translated` green.
    - A `grep` of user-facing text, the runner's messages included, finds no "cobolt".

- [ ] **T53 — The System KB and the Developer's Guide, final** (GOLDEN RULE #3, #8)
  - Verify:
    - The guide's §17 and §18 describe the finished Build. The five translations are deleted.
    - `build_chunked_kb` is run, and `prebuilt_chunked_kb_matches_the_published_documentation` is green.

- [ ] **T54 — Finalize**
  - Do:
    - Check off the ACs in `spec.md` with the evidence named in `WASM-PROGRESS.md`. Set the status to done here and in the spec.
    - Before the commit, quarantine untracked program files in other languages and run the credential scan.
  - Verify:
    - `cargo test --workspace --no-fail-fast`, every `test result:` line read: only the known reds.
    - The player `--lib` green.
    - `rcrun` builds.
    - The four CI workflows (`build-goal`, `pixel-parity`, `desktop-wasm`, `build-all-platforms`) dispatched once and recorded.

## Acceptance criteria → tasks

| AC | Requirements | Verified in |
|---|---|---|
| AC1 | R7 | T16 (local), T22 (CI, as projects move), T51 (all moved) |
| AC2 | R2 | F1, T11, T22 (`determinism`) |
| AC3 | R2, R5 | T22 (console), T35 (forms) |
| AC4 | R5, R23 | T21 |
| AC5 | R3, R4 | T9 (bundler), T15 (runner) |
| AC6 | R17, R18 | T4 (harness, baseline), T36 (first judgement), each close, T54 |
| AC7 | R20 | T2, T4 |
| AC8 | R15, R16 | T13 (engine), T14 (payload), T17 (the setting) |
| AC9 | R11, R21, R22 | T23, T40, each close |
| AC10 | R12 | T10, T45 |
| AC11 | R13 | T16 |
| AC12 | R14 | T16 |
| AC13 | R8 | T16 (the `bin/` layout), T20 (packages, installers, SDK) |
| AC14 | R9 | T30, T35 |
| AC15 | R1 | T16 (console), T28 (forms) |
| AC16 | R24, WASM-010 | T1, held by T9, T11, T12, T27, T28 |
| AC17 | R25 | T16, T17, T18 |
| AC18 | R26 | T3 (generation), T4 (hashes on four platforms) |
| AC19 | R27 | T7 (goldens), T8 (goldens after `HostClock`), T29 (refactor guard), T34 (pairs), each close |
| AC20 | R28 | T7 (per form), T8 (`deterministic_mode_repeats_frames`) |

**Requirements with no AC of their own:**
- **R6, R10:** T13, T15, T20, and T30–T32.
- **R19:** T4.
- **R21:** T16, the default.
- **R22:** T23, `WASM-PARITY.md`.
- **R24's opt-out:** T16–T18.

## Done criteria

This spec is done when:
- every acceptance criterion in `spec.md` is checked, with its evidence named in `WASM-PROGRESS.md`;
- the sweeps of T54 are green apart from the known reds;
- the System KB is regenerated and the Developer's Guide describes the finished Build, with its translations deleted;
- F1 and F2 landed as fix commits on their own branch, and every feature task as a feature commit on `features-2x-wasm-program-format`, each with its own `z` and CHANGELOG entry, pushed outside the forbidden window;
- nothing was merged into `main` without the operator asking.
