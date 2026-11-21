<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The WebAssembly module as the portable program format: native builds without a Rust toolchain per program (DESKTOP-WASM-001)

- **Status:** implementing since 2026-11-14. F1–F3 and T1–T12 are done (2.0.483–2.0.497, with the fix 2.0.492 found by T8): every task of M0, and the first four of M1. The CI dispatches of T4 and T7 are owed until their workflows are on `main`.
- **Plan:** ./plan.md   **Spec:** ./spec.md   **Date:** 2026-11-14
- **Branch:** `features-2x-desktop-wasm` (feature), cut from `main` at 2.0.484. The spec commits were copied onto `main` (2.0.478–2.0.482), and `features-2x-wasm-program-format` is kept as it was. The two fixes F1 and F2 went on `fixes-2x-wasm-emit-order`, cut from `main`, and are in `main`.

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

- [x] **F3 — Land the fixes, and bring them to this branch**
  - *Done 2026-11-14:*
    - `main` could not take the spec branch as a fast-forward, because it held a commit of its own (README). So the operator chose to copy the spec commits onto `main` and continue on a new branch.
    - `main` now holds the spec commits (2.0.478–2.0.482), then F1 (`f72c3d03`) and F2 (`dd88944e`), all fast-forwards.
    - `features-2x-desktop-wasm` was cut from that `main`, so it carries both fixes, and `compile_determinism` is green on it.
  - Do:
    - Commit F1 and F2 separately on `fixes-2x-wasm-emit-order`. Each takes the next `z` not used on any open branch: this branch holds 2.0.478 onwards.
    - Push outside the window, and ask the operator to merge them into `main`.
    - Bring them to this branch as the operator decided before F1.
  - Verify: `git log main --oneline` shows both fixes; `cargo test -p cobolt-wasm --test compile_determinism` green on this branch before T22 starts.

## Phase M0 — The baseline, the deterministic mode and the golden hashes (the WebAssembly path is off)

- [x] **T1 — The Web module's hash, recorded before anything changes** (R24; AC16)
  - *Done 2026-11-14 (2.0.485):*
    - **Pinned:** three files, not two. The single-memory module is pinned too, because the Build writes both variants.
      - `program.wasm`: 729 228 bytes, `f7fec25a…`;
      - `program.single.wasm`: 729 244 bytes, `289e75c5…`;
      - the manifest: 27 320 bytes, `d3b8c806…`.
    - **Runs:** 3 of 3 match in four runs: one recording, two checks, and one from a clean checkout at 2.0.484. The uncommitted edits to the example's project file do not reach the module.
    - **Timing:** the Build takes 9.6 s. `UPDATE_PIN=1` re-records the pins.
  - Files: new `crates/cobolt-web-tests/tests/desktop_wasm_unchanged.rs`; its fixture `crates/cobolt-web-tests/tests/fixtures/power-analytics-web.sha256`.
  - Do: build the Web module of `examples/PowerAnalyticsWeb` (from a copy, never in place), and pin the sha256 of `program.wasm` and of its manifest.
  - Verify: `cargo test -p cobolt-web-tests --test desktop_wasm_unchanged -- --nocapture` prints both hashes and passes. It stays green through every later task.

- [x] **T2 — The Build reports its phases and its peak memory** (R18, R20; AC7 harness)
  - *Done 2026-11-14 (2.0.486):*
    - **Phase order:** the enum is declared in the order a Build runs its phases. Regenerate comes before front end, because the forms' COBOL is written before it is parsed. The toolchain check counts as `Cargo`; T16 moves it after classification (C8).
    - **What the peak covers:** the Build's own process plus every process it started after the Build began. On the Cargo path that is where the memory goes, in `rustc`.
    - **The new `path` field** has one variant, `Cargo`. T16 adds the WebAssembly one.
    - **Measured by `build_reports_phases_and_peak`,** a one-form project on the shared cache: regenerate 2.2 ms, front end 7.6 ms, cargo 7 498.8 ms, install 1.5 ms. The sum is 7 510.1 ms against 7 524.5 ms of wall time, the peak 860.2 MB, and 6 of 6 checks passed.
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

- [x] **T3 — The ten-times reference project** (R26; AC18 generation)
  - *Done 2026-11-14 (2.0.487):*
    - **COBOL lines** means the non-blank lines of every program a Build compiles, counted after regenerating a copy as a Build does.
    - **The five references:** PowerGraphics is the largest in forms (9) and in lines (6 655); none has an indexed file. So the target is 90 forms, 66 550 lines and 10 indexed files.
    - **The generated project hits the target exactly.** The Common Code is sized so that the total comes out right. It is byte-identical across two generations (`232c5720…`), and `check` reports 0 errors and 0 warnings, in 137 s.
    - **The test passed 5 of 5 checks,** with the same numbers from a clean checkout.
    - **Why the targets are computed, not written down:** they come from the examples each time the test runs, as R17 defines them. T4 records the targets and the hash with each run.
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

- [x] **T4 — `cobolt-bench build-goal`, and the baseline** (R17, R19, R20; AC6 and AC7 harness, AC18 hash)
  - *Done 2026-11-14 (2.0.488):*
    - **This machine's baseline** is recorded in `specs/WASM-BUILD-GOAL.json` (`macos-aarch64`): Apple M3 Pro, 11 cores, 18 GB, **not a reference box**. 30 of 30 Builds succeeded in 89.3 min, all on the Cargo path, and all six projects are over the goal.
      - **Slowest Builds:** PowerCallOfDuty 190.7 s, PowerCloud 156.4 s, PowerCobolCraft 198.5 s, PowerGraphics 198.7 s, PowerSpatial 163.3 s, TenTimes 191.8 s.
      - **Where the time goes:** Cargo takes 156–199 s, because with no cache it compiles the whole dependency tree. Regeneration takes under 40 ms and install under 52 ms.
      - **Peak memory:** 4 086–5 754 MB.
    - **The front end** takes 20–61 ms on the five examples and **30 409.5 ms on TenTimes**, alone over the goal. Not profiled. The likely place is the per-form loop at `cobolt-compiler/src/lib.rs:2777–2830`, which links the whole Common Code into each of the 89 other forms' programs, then serializes the program and compresses it at the best level. It must be dealt with before the goal is judged at M2, since the WebAssembly path runs the same front end.
    - **The generator** takes the project as the WebAssembly path will: one module, each form with its controls and bound events. Until T16 gives Desktop its own form descriptions, the bench builds them as the Web Build's `form_specs` does, without the Web lowering.
      - **Accepted:** PowerCallOfDuty (5.1 ms) and PowerGraphics (9 forms, 45.8 ms).
      - **Refused**, each at the developer's line:
        - PowerCloud at `COBOL::"AWS-SET-CREDENTIALS"`, and 37 more;
        - PowerCobolCraft at a statement that reads more than four properties, and 18 more;
        - PowerSpatial at `ACCEPT FROM Environment`, and 10 more;
        - TenTimes at an INDEXED file on disk, and 7 more.
      - **A first version compiled each program alone.** That gave false refusals ("SCN-1 is not defined"), because a form's controls are unknown without its form description. It was corrected and the whole baseline run again; only the second run is recorded.
    - **The ten-times hash** is `232c5720…`, equal to T3's.
    - **Owed: the CI dispatch.** `build-goal.yml` is written for the four platforms, but GitHub dispatches only from the default branch, and the workflow is not on `main`. The four result blocks and AC18's four equal hashes are recorded at the first dispatch after the workflow reaches `main`, or at the first `v*` tag. The macOS arm64 job runs on the hosted runner, and its record says "not a reference box" (C9).
    - **Flags:** `--runs 0` times the generator alone and builds nothing; `--record` refuses it.
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

- [x] **T5 — The run clock and the random seed, pinned for tests** (R28, the program half)
  - *Done 2026-11-14 (2.0.489):*
    - **`cobolt-runtime/src/test_pins.rs`** reads `PRC_TEST_CLOCK` (RFC 3339) and `PRC_TEST_SEED` (u64) once.
      - The offset written in the pinned value is the program's time zone, so a pinned run shows the same date on every machine.
      - A value that cannot be read is named on stderr and pins nothing.
      - `set_for_test` and `clear_for_test` pin for the whole process without touching the environment, and restart the generator.
    - **The audit:** every clock or random read a program can see now goes through the pins:
      - `ACCEPT … FROM DATE`, `TIME`, `DAY` and `DAY-OF-WEEK`;
      - `FUNCTION CURRENT-DATE` and `WHEN-COMPILED`;
      - `YEAR-TO-YYYY`, which keeps its UTC year;
      - the first state of `FUNCTION RANDOM`, which a pinned seed sets as `FUNCTION RANDOM(seed)` would.
    - **The other reads never give a program a value,** and are listed in the module's documentation:
      - timeouts and progress of asynchronous operations;
      - the waits of file locks, `WS-PAUSE` and the AWS pool;
      - the frame clock of animation sequences, which is T8's;
      - AWS signing and credential expiry, which need the real time;
      - indexed-file log stamps, key-file nonces and temporary names;
      - measurements inside tests.
      - `cobolt-stdlib`'s intrinsic table has its own `CURRENT-DATE`, but no code calls the table, so it is left as it is.
    - **`pinned_clock_and_seed`:** 7 of 7 checks.
      - The clock, pinned at `2026-01-02T03:04:05.67-03:00`, gives CURRENT-DATE `2026010203040567-0300`, DATE `260102` (YYMMDD in a PIC 9(6)), DAY `26002`, DAY-OF-WEEK `5` and TIME `03040567`, the same in two runs.
      - RANDOM, pinned at seed 20 261 114, gives the same 1 000 values in two runs (1 000 distinct, about 72 000 values/s), equal to what `FUNCTION RANDOM(20261114)` gives with nothing pinned.
      - Unpinned, the first value is still 0.485917337.
      - `test_local_clock` (unpinned, 3 of 3) is unchanged.
    - **Interpreter-binary parity:** the new compiler test `a_built_program_takes_the_clock_and_seed_pins` builds a console fixture with Cargo and runs it with both variables set. It prints `2026010203040567-0300` and `0198382315`, the interpreter's first value for that seed (2 of 2, 41.4 s).
    - **`cargo test -p cobolt-runtime`:** all 210 targets ok, 1 674 passed, 0 failed, 29 ignored. It was run one target at a time, freeing each binary after its run: each of the 208 test binaries takes about 228 MB, about 47 GB if all are linked at once.
    - **The pinned clock does not move yet.** T8 makes it follow the host's virtual time.
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

- [x] **T6 — The deterministic headless loop, and the steps `shot`, `size` and `answer`** (R28, the headless half)
  - *Done 2026-11-14 (2.0.490):*
    - **Deterministic mode:** with `PRC_TEST_CLOCK` set (`test_pins::clock_pinned`), frames are drawn at a virtual time counted in 1/60 s ticks.
      - **Refinement:** only a wait adds ticks (`wait_ms` and the opening wait). A frame that only waits for the program adds none. Otherwise the number of those frames, which depends on how fast the interpreter thread runs, would move the time, and two runs would draw at different times.
      - **Deviation until T8:** a wait still lets the same real time pass. `host.rs`'s timers, its 450 ms opening hold and `onShow` read the real clock, and skipping the real time would change what a script sees. T8 removes the sleep when `HostClock` follows the virtual time.
    - **The steps:**
      - `{"shot": "<name>"}` writes the frame to `<folder>/<name>.png` with the CPU rasteriser; a name with a folder in it is refused;
      - `{"size": [w, h]}` resizes, each side from 1 to 8 192, then settles;
      - `{"answer": {"path": …}}` and `{"answer": {"cancel": true}}` queue the answer for the next file dialog.
    - **The API:** `run_headless_with` takes the folder for pictures. `run_headless` keeps its signature, so its callers are unchanged. `run_script_file` takes the folder, and the generated `main.rs` passes `PRC_HEADLESS_SHOTS`.
    - **The report** adds `virtual_ms` in deterministic mode and `answers_unused` when an answer was left; otherwise it is what it was.
    - **Correction: a Desktop form has no message box.** `MessageBox`, `Alert`, `Confirm` and `InputBox` exist only in the Web player, so `answer` answers the dialogs a Desktop program can raise:
      - the program's `OPEN-FILE-DIALOG`, `SAVE-FILE-DIALOG` and `FOLDER-DIALOG` (`file_dialog::answer_program`);
      - the host's own panels, a Viewer's Save As and a grid's export (`file_dialog::begin`).
    - **The agents' `run_form` script** (`cobolt-project-tools/src/script.rs`) does not offer the three steps. They serve the parity harness, and `shot` needs a folder `run_form` does not give.
    - **`headless_shots`:** 11 of 11 checks, 12 steps in 2 117 ms (virtual 600 ms).
      - Shots at 420 × 200 (6 523 bytes), 640 × 360 (9 930 bytes) and 320 × 240 (6 231 bytes).
      - The open dialog took its scripted path, the save dialog its scripted cancel, and a host Save As panel its queued answer.
      - `../outside` was refused; no answer was left unused.
    - `headless_dump` and `headless_pointer_click` are unchanged and green.
    - **Sweep of `cobolt-form-host`** (debug, `--no-fail-fast`, every `test result:` read):
      - the library: 189 passed, 1 failed;
      - the integration targets, each green: `backend_events` 1, `headless_dump` 1, `headless_pointer_click` 1, `headless_shots` 1, `progress_cancel` 1, `remote_call_pending` 1, `snackbar_stack` 21;
      - the built binary, `side_menu_rows_hosts --ignored`: 1 passed in 60.5 s.
      - **The one red is not T6's:** `host::parity::corpus_golden` differs on `PowerChat/chat-form.cfrm` (SideMenu-1 at 594 px high, golden 792). It is the known red already named in the 2.x merge sweep. T6 touches neither `host.rs` nor the render path.
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

- [x] **T7 — The pixel-parity harness, Cargo only; the base scripts; the first golden hashes** (R27 harness; AC19 second guard, AC20 per form; AC1 check scripts)
  - *Done 2026-11-14 (2.0.491):*
    - **`crates/cobolt-bench/src/pixel_parity.rs`**, the `pixel-parity` mode, with `--write-scripts`, `--cargo`, `--record` and `--project <name>`.
    - **Refinements and decisions:**
      - **The projects are taken as committed** (`git ls-tree` and `git show` of `HEAD`), never from the working tree, so a golden recorded on one machine is the one a clean checkout compares with. The operator's working tree differs from `HEAD` in some of them (for example `war-form.cfrm` binds 40 events there, 2 at `HEAD`).
      - **No product change.** A built program already opens at any of its embedded forms with `COBOLT_DESIGNER_FORM`, and the headless variables still apply, so each form's script runs against the project's own binary.
      - **The network is cut off rather than stubbed.** The copy's AWS connections are routed to a server that does not exist. Each run gets an empty home folder, no AWS variables, and `HTTP(S)_PROXY`/`ALL_PROXY` set to a port that refuses connections. Handlers that would reach a model, a web service, a database or AWS are not raised, and the script's comment names them.
      - **The script format:** `{"comment", "form", "steps"}`. A `shot` once the form is open and after every step. A dialog handler gets `{"answer": {"cancel": true}}` before it.
      - **A handler that opens a form modally** (`OpenFormSync`) is checked before one that ends the form, because a handler can do both (PowerCobolCraft's BTN-PLAY opens the world and then closes the title). It is raised without waiting (`"wait": false`), then given 500 ms. Once one has run, the ending handlers are raised without waiting too; otherwise the run waits on a program the modal form holds up, until the time limit.
      - **Each size is its own run of the program**, with `{"size": [w, h]}` first for 800 and 480 wide, so a run at one size cannot leave state for the next. `data/` is restored from a pristine copy before every run.
      - **Three runs to record, two to check.** In the first full recording two runs of PowerGraphics' bridge form matched by chance, and the check then found 6 of its frames different. With three runs, a form that fails to repeat is far less likely to be recorded.
      - **Audio plays during a run** (the reference projects play sounds); nothing in a frame depends on it.
    - **Measured** (macOS arm64, release build):
      - **`--cargo --record`:** 5 projects, 22 forms, 22 scripts, 263 steps, 369 frames, three runs each, in 18.5 min.
        - Per project, forms / steps / frames / identical in all three runs / build s / runs s:
          - PowerCallOfDuty 1 / 5 / 9 / 0 / 273.2 / 15.4;
          - PowerCloud 5 / 39 / 57 / 25 / 104.6 / 74.7;
          - PowerCobolCraft 2 / 41 / 60 / 4 / 7.2 / 78.6;
          - PowerGraphics 9 / 95 / 120 / 108 / 137.8 / 174.7;
          - PowerSpatial 5 / 83 / 123 / 56 / 69.7 / 170.0.
        - 193 of 369 frames identical; **6 of 22 forms repeatable** (PowerGraphics: charts, imagelab, pitfall, pool2d, pool3d, viewer3d), **75 golden frames** written.
      - **The second run without `--record`** (PowerGraphics, twice, two runs each): **75 of 75 golden frames matched both times**, 0 differing (6.1 and 5.6 min, exit 0). The golden frames are all PowerGraphics', so the other projects had nothing to match.
      - **The unit test** `cargo test -p cobolt-bench pixel_parity` (`base_script_orders_and_names_the_handlers`): 9 of 9 checks. A fixture of 7 controls and 8 handlers gives the 14 expected steps, and the comment names its modal, ending, dialog and network handlers. It also checks a form's file stem, the CALLs a handler makes, and a frame's hash. (Found while writing it: an AgentObject written as a self-closed `<Control …/>` was not among the loaded form's controls, and no error said so. The reference forms never write one. Noted for the operator, not changed here.)
    - **Not repeatable before `HostClock`, 16 forms** (frames that differ between runs): PowerCallOfDuty war-form 9 of 9; PowerCloud main-form 2 of 27, pet-form 12 of 12, view-form 3 of 3, confirm-form 9 of 9, working-form 6 of 6; PowerCobolCraft world-form 38 of 42, title-form 18 of 18; PowerGraphics game-form 9 of 33, bridge-form 2 of 6, runner-form 1 of 6; PowerSpatial main-form 7 of 60, toolbar-form 9 of 9, rail-form 22 of 24, rooms-form 21 of 21, settings-form 8 of 9. The causes, read from the code: the entrance and exit effects (MatrixRain is seeded from the form's name and size, but its progress reads the real clock), snackbars, timers, the 450 ms opening hold, and canvas `onFrame` events handled while the next frame is drawn. T8 removes them.
    - **Owed:** the CI dispatch on four platforms, and the three other platforms' golden files from its artefacts. GitHub runs `workflow_dispatch` only from the default branch, and `main` is not touched while the feature is on its branch.
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

- [x] **T8 — `HostClock`, then the rest of the golden hashes** (R28; AC20)
  - *Done 2026-11-14 (2.0.493; the fix 2.0.492 before it):*
    - **`crates/cobolt-form-host/src/clock.rs`:** `now()` and `since()`. It is the real clock unless a deterministic run has started the virtual time; then it is a fixed instant plus that time.
    - **The 16 reads in `host.rs` that reach a pixel** now read it:
      - the form's start, which gates the 450 ms hold and `onShow` (root and child bodies);
      - the entrance effect, the exit effect (two places) and the window effect;
      - the snackbars (raise and tick);
      - the animation step (two places);
      - the parked form's timers.
    - **Left on the real clock**, as nothing they do reaches a pixel: placing a group of windows on the screen (`group_fit_started`), the OS blur's retry, time limits, and the measurements in tests. The plan's "37 calls" counted those and the tests. `snackbar_stack.rs` reads no clock (every method takes `now`), so it is unchanged.
    - **The virtual time lives in `cobolt_runtime::test_pins`** (`start_virtual_time`, `set_virtual_time`, `stop_virtual_time`, `virtual_elapsed`), so the program's clock follows the same time: the pinned instant plus the virtual time while it runs.
    - **The headless loop** (`headless.rs`), with the clock pinned:
      - starts the virtual time before it builds the form, and ends it with the run;
      - no longer sleeps during a wait; each frame is one 1/60 s tick;
      - **refinement (`quiesce`):** before time moves on, and at the end of every step that waits for the program, the form on the pane finishes everything it was sent. When anything was sent since it was last at rest, a wake goes behind it (`FormHost::script_barrier`), and the loop draws frames at the same virtual time until the program has handled both. The frames that let the writes reach the state go round again if they raise an event (at most 16 rounds).
        - Why the wake: the program takes an event off its queue before handling it, so the pending count alone says nothing about a handler still running. The wake is taken only after the handler returns.
        - Why before time moves on: the first version waited only for events raised while the form was idle. That leaves the opening wait racing a long `onLoad` (war-form's builds a city): canvas frames raised meanwhile queue up, or are coalesced away past 8, in a number set by how fast the program ran. Whether that race ever bit here was not isolated, because the pixels war-form drew differently turned out to be the rasteriser's (below).
        - A form that a modal form holds up (`root_modal_blocked`), or whose program has ended, is not waited for. Events sent to a child window were not waited for either, until 2.0.574 (`fixes-2x-headless-children`): the open child windows now get the same wake (`FormHost::script_children_barrier`) and are waited for until each has finished its start-up and all it was sent (`script_children_pending`), unless its program has ended or a modal child of its own holds it up. A window opened after the wakes went out (the click's handler works a while before `OpenFormSync`) starts another round (`host::windows_opened()`). Before that, a form opened modally by a click was pictured before its `onLoad` had finished (PowerCobolCraft's title form, R28).
      - `host::events_sent()` counts the events the host sends, so the loop can tell whether anything was sent since the form was last at rest.
    - **`cobolt-bench pixel-parity`:**
      - a check now fails when a form's runs differ (AC20), as a differing golden does; a recording only leaves the form out. Its wording and the golden file's header say so;
      - **the entrance wait:** under virtual time an event moves no clock. In the four projects that open their forms with MatrixRain (2000 ms), every picture after the first was therefore taken 600 ms in, mid-effect, and showed the effect rather than the form's answer (PowerCloud's confirm-form drew the same hash for every step). In such a project (`[forms] entrance-effect`, read as the product reads it), the base script now waits `MATRIX_HARD_MAX_MS` (6000 ms, longer than any effect) after `00-open` and takes `00-ready`. PowerGraphics opens without an effect, and its scripts are unchanged.
    - **A fix found on the way, landed first (2.0.492, `fixes-2x-raster-atlas`, picked onto this branch):**
      - war-form still drew 2 to 4 text pixels differently between runs, one step of 255 apart, at the designed size.
      - The CPU rasteriser summed a glyph's texture coordinate from absolute atlas coordinates in f32, so its rounding depended on where egui had put the glyph, which depends on the order text was first laid out in.
      - It now samples exactly in f64, relative to the first vertex. Its own test went from 5 of 12 differing pictures to 12 of 12 identical.
    - **Measured** (macOS arm64, release build):
      - **The refactor moved no pixel:**
        - the first version of this task, checked against T7's goldens, matched 75 of 75 golden, with 369 of 369 frames identical in two runs (a three-run recording after it found war-form differing in 3 of 9 frames; see the fix above);
        - the final code with the sampling before 2.0.492 matched PowerGraphics 120 of 120 frames identical, 9 of 9 forms repeatable, 75 of 75 golden (3.9 min).
      - **`--write-scripts --cargo --record`:** 5 projects, 22 forms, 289 steps, 408 frames, three runs each, 18.2 min.
        - Per project, forms / steps / frames / identical in all three runs / build s / runs s:
          - PowerCallOfDuty 1 / 7 / 12 / 12 / 205.8 / 19.4;
          - PowerCloud 5 / 49 / 72 / 72 / 82.8 / 116.0;
          - PowerCobolCraft 2 / 45 / 66 / 66 / 6.0 / 119.9;
          - PowerGraphics 9 / 95 / 120 / 120 / 102.5 / 58.2;
          - PowerSpatial 5 / 93 / 138 / 138 / 65.7 / 311.5.
        - **408 of 408 frames identical; 22 of 22 forms repeatable** (6 of 22 at T7); 408 golden frames written, `not_repeatable` empty.
        - Against T7's 75 frames, 42 are unchanged. The other 33 (every frame of PowerGraphics' pitfall, pool3d and viewer3d forms) moved, by 2.0.492's sampling alone, as the check above shows.
      - **The check without `--record`** (two runs each): **408 of 408 golden frames matched**, 22 of 22 forms repeatable, exit 0, 14.6 min.
      - **Tests:**
        - `deterministic_mode_repeats_frames`: 10 of 10 checks, 4 of 4 pictures identical, 41 ticks and 5 700 ms of virtual time per run;
        - `pinned_clock_and_seed`: 9 of 9; `base_script_orders_and_names_the_handlers`: 11 of 11;
        - `cobolt-form-host`: 189 of 190 in the library and 28 of 28 in 8 targets. The red, `corpus_golden`, is a layout golden that fails on PowerChat's chat-form, and was red before this task;
        - `cobolt-forms --features render`: 1 877 of 1 878, `engine_reference_form_parity_static_vs_faces` green. The red, `example_corpus_golden` (3 PowerDemo3 forms), is a shape golden, red on `main` without this work too;
        - the rcrun host tests (`powercloud_hosts`, `side_menu_hosted_controls`, `side_menu_rows_hosts`, `layers_hosts_091`, `perspective_hosts_096`, `model_list_hosts`, `partiql_hosts`, `aws_hosts`): 22 of 22, `side_menu_rows_hosts` with its built-binary test; the 5 other built-binary tests stay ignored, as by default.
    - **Owed:** the CI dispatch of `pixel-parity.yml` on four platforms and the other three platforms' golden files, as for T7 (the workflow must be on `main` first).
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

- [x] **T9 — The manifest's capabilities, the ABI range and the bundler's refusal** (R3, R4 web half, R5; AC5 web half, AC16)
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
  - *Done 2026-11-14 (2.0.494):*
    - **`codes.rs`:**
      - `cap`: the ten capability names of the plan, and `cap::ALL`;
      - `NATIVE_IMPORT_MODULE = "prn"`, and `NATIVE_IMPORTS` with the sets M1 uses: `console` → `con_read`, `env` → `env_get`, `exit`. Only those two, because a set is fixed once a release uses it (C2): the dialog, file, sort, SQL and HTTP sets are left to the tasks that implement them (T32, T38, T39, T41, T42), each amending A27;
      - `CONFORMANCE_IMPORTS` gains `switch_state`, which the conformance build has always imported (C10).
    - **`manifest.rs`:**
      - `capabilities: Vec<String>`, `serde(default, skip_serializing_if = "Vec::is_empty")`;
      - `validate` accepts `1..=ABI_VERSION`. The refusal now says what to do: for 0, rebuild the application; for a newer ABI, run it with a current release. "Rebuild with the current IDE" was wrong advice for a module newer than the host.
      - Struct literals: the field was added to the 20 places that build a manifest by hand (the ABI crate, `cobolt-wasm`'s `build_manifest`, the player's `tree.rs`, 15 player test files and `parity_page.rs`). `build_manifest` writes none: T12 fills it under `native`.
    - **`scan.rs` (`bundle_problems`):**
      - each capability the embedded manifest lists is a problem that names it: one only the desktop runner provides, or, for an unknown name, one this release does not know;
      - an import from `prn` is named as a runner import, no longer as "not allowed by §3".
      - The page's own loader (`hostpage.rs`) still accepts ABI 1 exactly, which is every ABI there is. It is left as it is, so the Web bundle stays byte-identical (AC16).
    - **`WASM-ABI-v1.md`:**
      - §10 gains the `capabilities` member;
      - §11 item 1 reads "refuse an ABI it does not speak";
      - §8 is rewritten to the code: the `adv` argument, modes 2 = I-O and 3 = EXTEND with the organisation in bits 8–15, end of file −10, failures as the FILE STATUS negated, handles from 1 with 0 as the console, and `switch_state`;
      - amendment rows **A27** (capabilities, `prn`, the ABI range) and **A28** (the §8 correction).
    - **Measured:**
      - `cargo test -p cobolt-wasm-abi`: 21 of 21; with `--features fixture`, 26 of 26. New:
        - `validate_accepts_every_abi_from_1_to_the_current_one`: 3 of 3 (1 accepted; 0 and 2 refused with what to do);
        - `capabilities_are_written_only_when_there_are_some`: 4 of 4 (568 bytes, as before A27; with two they round-trip in order);
        - `a_manifest_without_capabilities_is_the_bytes_it_was`: 2 of 2 (the fixture modules' embedded manifests, 2 165 and 2 166 bytes, have the sha256 taken from `write_fixtures` before the change);
        - `bundle_problems_names_a_capability_only_the_runner_provides`: 8 of 8 (both variants: `files.indexed` named, an unknown name named, none accepted).
      - `desktop_wasm_unchanged` (T1, AC16): 3 of 3 artefacts match the pin, regenerated and built in 9.2 s.
      - `cargo test -p cobolt-web-bundle`: 146 of 146 in 60 targets.
      - The player: `--lib` 474 of 474, `abi_host` 28 of 28.
      - **The shipped player is rebuilt** (`build_player --out assets/web-player`). Its freshness test asked for it, and was red before this task too: the player's inputs changed at 2.0.470 (Canvas3D in `onLoad`) and at the fix 2.0.492 without a rebuild, the last being 2.0.447.
        - plain 4 129 791 → 4 130 450 bytes, threaded 3 944 236 → 3 944 904, the graphics3d part 8 471 405 → 8 471 516; the other parts unchanged.
        - `freshness` 6 of 6; `cobolt-web-bundle` 146 of 146 again with the new player.

- [x] **T10 — The native column of the subset table** (R12; AC10)
  - Files:
    - `crates/cobolt-wasm/src/support.rs`: every entry of `STATEMENT_TABLE` and `BUILTIN_TABLE` gains `native: Native { Module, Host(Cap), Cargo(&'static str) }`;
    - `crates/cobolt-wasm/tests/subset_table.rs`;
    - `crates/cobolt-compiler/src/lib.rs`: a Desktop column in `browser_subset_doc`;
    - `assets/knowledge/chunked.data`.
  - Do: classify each entry for the Desktop kind as it stands at M1. What the WebAssembly path cannot do yet is `Cargo("<reason>")`.
  - Verify:
    - `every_statement_and_builtin_has_a_native_class` and a planted-unknown twin green.
    - The KB is regenerated, and `prebuilt_chunked_kb_matches_the_published_documentation` is green.
  - *Done 2026-11-14 (2.0.495):*
    - **`support.rs`:**
      - `Native { Module, Host(&'static str), Cargo(&'static str) }`; `Host` carries a name of `cap` (A27, T9);
      - `STATEMENT_TABLE` is now `(name, Support, Native)`, and `BuiltinClass` gains `native`; `statement_native(name)` reads a statement's class, as `builtin_entry(name).native` reads a built-in's;
      - one reason per group, each naming the milestone that moves it: forms, controls and the application object (M2); dialogs (M2); files on disk, the file and image built-ins, SORT with USING or GIVING, MERGE, COMMIT, ROLLBACK and UNLOCK (M3); SQL and HTTP (M4); pointers (M5); the non-visual controls, keys, model lists, audio, animation sequences, layouts, the phone's secure storage, remote calls and identity (M6); EXEC RUST never moves.
    - **The classes at M1:**
      - 53 statements: 31 in the module; 3 through the runner (`ACCEPT` → `console`, `STOP` and `GOBACK` → `env`); 19 on the Cargo path. *(T24: 29 and 21, as `TRY` and `THROW` move to the Cargo path, M5: no generator compiles them; WASM-PARITY #58.)*
      - 83 built-ins, all on the Cargo path. Each one drives a form, a file, a database, a model, a key or the network, so a console program needs none.
    - **Decisions:**
      - **The remote calls name M6.** R11's end state leaves only EXEC RUST, crates and Debug on the Cargo path, and the plan's milestone table does not list them elsewhere.
      - **`SECURESTORE`, `SECURELOAD` and `SECUREDELETE` have their own reason.** They are Mobile built-ins, at which a desktop program stops (`builtins.rs`). That stop is what the WebAssembly path must reproduce at M6.
      - **For T21:** a file verb's class is `Cargo`, with a reason that says an INDEXED file declared `STORAGE IS MEMORY` is compiled into the module, and so is a SORT with an INPUT and an OUTPUT PROCEDURE (RELEASE and RETURN are `Module`). A row names a statement, not its file, so the Cargo class of `OPEN`, `READ`, `WRITE`, `REWRITE`, `DELETE`, `START`, `CLOSE` and `SORT` applies to a file on disk. Whether a file is in memory is the generator's own check, the one that writes `MemoryFiles` for a browser module. T21 should take the refusal from there, not from the row, or M1 would send a program with only in-memory files to the Cargo path. *(T24 changed this: on the desktop a memory file is read from its disk file, as the interpreter reads it, so a desktop module refuses it too, and the reason says so; WASM-PARITY #15. The SORT work file is still compiled into the module.)*
    - **`browser_subset_doc`:** both tables gain **On the Desktop** and **Why**, with a paragraph saying that every Desktop Build still takes the Cargo path today. The KB is regenerated: 4 389 records from 8 documents, 15 372 288 → 15 544 320 bytes.
    - **Measured:**
      - `cargo test -p cobolt-wasm --test subset_table`: 8 of 8. New:
        - `every_statement_and_builtin_has_a_native_class`: 136 entries (53 + 83), 0 faults, 0 Cargo reasons without a milestone, 5 of 5 fixed points of M1 (EXEC RUST, ACCEPT, STOP, GOBACK, MOVE), 1.3 ms;
        - `a_planted_entry_with_no_native_class_is_caught`: 4 of 4 planted faults caught (two unknown names, a capability A27 does not list, an empty reason), and the sound control passed, 1.1 ms.
      - `prebuilt_chunked_kb_matches_the_published_documentation`: green. The compiler's `published_documentation` tests: 5 of 5, including `the_browser_subset_section_names_every_builtin_and_statement`.
      - `cargo test -p cobolt-wasm --no-fail-fast`: 27 of 29 targets green. Two reds, both from before this task:
        - `corpus`: the program `conditionals/test-arith-relation-after-and.cbl` prints its own timing (460 ms in the module, 970 ms in the interpreter);
        - `dependency_guard`.
      - No browser class moved: the 136 rows, with the Desktop column taken out, are the same as at 2.0.494.

- [x] **T11 — WHEN-COMPILED and paths as Build inputs** (R2; C3)
  - Files: `crates/cobolt-wasm/src/lib.rs` (`Options.compiled_at`), the WHEN-COMPILED lowering, and the `prlines` writer.
  - Do:
    - `compiled_at` comes from `SOURCE_DATE_EPOCH` when it is set, and from the Build's clock otherwise.
    - Every path in `prlines` is written relative and with `/`.
  - Verify:
    - `compile_determinism` gains a case: with the same `SOURCE_DATE_EPOCH`, a program using WHEN-COMPILED gives identical bytes, built from two folders with different separators.
    - T1's test green.
  - *Done 2026-11-14 (2.0.496):*
    - **The field:** the option already existed as `Options.when_compiled: Option<String>` (`module.rs`). It keeps that name rather than becoming `compiled_at`, so that the six test files that name it are untouched. The WHEN-COMPILED lowering (`lower_more.rs`) is unchanged: it takes the text the module builder hands it.
    - **`build_time(source_date_epoch)`** (`module.rs`, public) gives the 21 characters when the options name none:
      - from `SOURCE_DATE_EPOCH` when it is set and not empty;
      - otherwise from the Build's clock, as before.
      - A value that is not a whole number of seconds, or is past 9999-12-31 23:59:59 UTC (the last second a four-digit year spells), is refused. The message names the variable and its value, and says to set it to a number or unset it, as reproducible-builds.org asks for a malformed value.
      - Every caller that leaves the option empty now honours the variable: the Web and Mobile generator today, the Desktop `wasm_build` from T21.
    - **The `prlines` writer** (`line_site`):
      - Today no file path reaches the line table. Its sites are form display paths (`MAIN ▸ BTN-OK ▸ onClick`) and form names, because codegen's source map has no Common Code spans. A Common Code file's display path is its bare `rel_path`, and `resolve_display_path` already treats a site with no `▸` as a file.
      - The writer uses that same rule. A file site is written with `/`; an absolute one (`/…` or a drive letter) is refused as an internal error, because it would put the building machine's folders into the module's bytes.
      - The fallback site (the form's own name, for a line the source map does not resolve) is left as it is.
    - **Measured:**
      - `source_date_epoch_and_separators_are_build_inputs` (new in `compile_determinism`): 8 of 8 checks, 358 ms. The fixture moves FUNCTION WHEN-COMPILED, with its three statements mapped to a Common Code file. The checks:
        - `src\common\stamp.cbl` and `src/common/stamp.cbl` give the same bytes in both variants (85 374 and 85 385 bytes);
        - 12 of 12 `prlines` rows naming the file are written with `/`;
        - the module answers `2025111400000000+0000` for `SOURCE_DATE_EPOCH=1763078400`;
        - the next day gives other bytes;
        - `tomorrow` is refused, naming the variable and its value;
        - `/Users/dev/…` and `C:\work\…` are refused;
        - unset, the clock gives 21 characters;
        - the last second of 9999 is spelled `9999123123595900+0000`, and the next second is refused.
      - `same_input_same_bytes` (F1): 126 variants, 0 differing; 2 520 compiles in 23.5 s (107 compiles/s).
      - `desktop_wasm_unchanged` (T1, AC16): 3 of 3 artefacts match the pin, 9.9 s.
      - `cargo test -p cobolt-wasm --no-fail-fast`: 27 of 29 targets green. The same two reds as before this task:
        - `corpus`: the program `conditionals/test-arith-relation-after-and.cbl` prints its own timing (450 ms in the module, 970 ms in the interpreter);
        - `dependency_guard`.

- [x] **T12 — `Options.native` and the console capabilities in the generator** (R3, R10, R11 console part)
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
  - *Done 2026-11-15 (2.0.497):*
    - **The option:** `Options.native` (default off) is in `module.rs`, where `Options` is defined; `lib.rs` only re-exports it. The Web and Mobile generator leaves it off.
    - **What compiles to the runner's imports** (ABI **A29**, new):
      - `ACCEPT` from the console (plain, or from a console mnemonic) → `prn.con_read`. A conformance build keeps reading handle 0, as before.
      - `ACCEPT … FROM ENVIRONMENT "name"`, `ENVIRONMENT-VALUE`, `COMMAND-LINE`, `ARGUMENT-NUMBER` and `ARGUMENT-VALUE` → `prn.env_get`.
      - `DISPLAY … UPON ENVIRONMENT-NAME` and `… UPON ARGUMENT-NUMBER` set two hidden registers of the region (a 255-character name, an eight-byte pointer) and show nothing, as on the desktop.
      - `ACCEPT … FROM ESCAPE KEY` and `CRT STATUS` move `"00"` and `"0000"`, as the desktop does: in the module, with no capability.
    - **The exit code** (operator, 2026-11-14: "Match Cargo now"):
      - The task asked for `RETURN-CODE` at `STOP RUN` to reach `exit`. The source says otherwise: the Cargo-built program and `rcrun run` end with 0 after `STOP RUN` whatever `RETURN-CODE` holds. `RETURN-CODE` is not in the generator at all, so a program naming it takes the Cargo path.
      - So `STOP RUN` and `GOBACK` end the run inside the module, the runner will end with 0, and `exit` stays reserved in A29 until `RETURN-CODE` reaches both paths. That gap is a fix of its own, for both paths, proposed separately.
      - *Later (2.0.498, 2.0.499):* the fix landed on `fixes-2x-return-code` and was picked here. The Cargo-built program and `rcrun run` now end with the code `RETURN-CODE` holds. The generator still refuses `RETURN-CODE` (`native_console` checks it), so such a program takes the Cargo path, and a module, which cannot name it, ends with 0. `RETURN-CODE` in the generator, and `exit` with it, fall to T15.
      - `STOP` and `GOBACK` are now *in the module* in the Desktop column: `support.rs`, the KB's paragraph and `subset_table`'s fixed points. The KB is regenerated.
    - **The manifest:** `capabilities` lists `console` and `env` as the module's `prn` imports reach them, in A27's order. `imports` names them `prn.con_read` and `prn.env_get`. A module that reaches neither writes no `capabilities`, and its bytes are the browser build's.
    - **The routine blob** (`rt.wasm`) gains `rt_accept_con` and `rt_accept_env`. T1's pin failed at each step until four rules held, and they are written above the new code in `accept.rs`:
      - **Order.** The routines are compiled in symbol-name order, and each is placed where it is first named. The new functions are named to sort after `accept_clock` (`accept_native_console`, `accept_native_environment`). A separate module, `native_accept`, sorted first and pulled `put` and `locate_sized` ahead of their old places.
      - **Signatures.** They use only signatures the blob already had, because the generator copies the blob's types in order. An `i64` argument, or a six-argument import, shifted every type index. So `env_get` writes its answer over the name it reads, and the argument pointer travels as an operand.
      - **No checks.** They index nothing and divide by nothing that can be zero. Each such check adds a 16-byte panic location to the read-only data and moves every static after it.
      - **Lines.** They stand below every line of the existing code, because a panic location records its line.
    - **The test host** (`host.rs`) gains `NativeInput` (the console's lines, the environment, the arguments) and `run_native`. Its `prn` imports answer only from that list.
    - **Measured:**
      - **`native_console`** (new): 85 of 85 checks, 5.4 s. 8 programs × 2 memory variants, 66 lines compared, compile 114 ms in all:
        - `console` (`console`): `[Ada Lovelace]`, `[036]`, `[long]`, `[   ]`, `[     ]`, `[007]`. These are the lines `rcrun run` printed with the same four lines on its standard input: the line end is removed, a long line is cut, an empty line is spaces, and at the end of the input an alphanumeric item gets spaces while a numeric one keeps its value.
        - `environment`, `arguments`, `no-arguments` (`env`), `escape-key-crt-status` (none) and `sort-procedures` (none): equal to the interpreter run in the same process with the same variables and arguments;
        - `console-and-environment` lists `["console", "env"]`; `in-memory-indexed` (none) reads its three records back in key order;
        - each capability-free module equals the browser build byte for byte; each module that needs the runner is refused by the bundler, which names its `prn` import.
        - Without `native`: the environment, the arguments, both `DISPLAY UPON` registers and ESCAPE KEY are refused as before, and the console's six `ACCEPT`s are ignored with six warnings.
      - **`desktop_wasm_unchanged`** (T1, AC16): 3 of 3 artefacts match the pin.
      - **`compile_determinism`:** 2 of 2. `same_input_same_bytes` covers 126 variants, 0 differing; 2 520 compiles in 23.5 s (107 compiles/s).
      - **`subset_table`:** 8 of 8. 33 entries in the module, 1 through the runner (`console`), 102 on the Cargo path; 5 of 5 fixed points.
      - **`rt_blob`:** 2 of 2.
      - **The KB:** regenerated (4 388 records). `prebuilt_chunked_kb_matches_the_published_documentation` and `the_browser_subset_section_names_every_builtin_and_statement` are green.
      - **`cargo test -p cobolt-wasm --no-fail-fast`:** 28 of 30 targets green. The same two reds as before this task:
        - `corpus`: `conditionals/test-arith-relation-after-and.cbl` prints its own timing (430 ms in the module, 920 ms in the interpreter);
        - `dependency_guard`.

- [x] **T13 — `cobolt-runner-engine`: one wasmtime setup, machine code and its cache** (R6, R15, R16; AC8 engine half)
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
  - *Done 2026-11-15 (2.0.511):*
    - **The crate** `crates/cobolt-runner-engine` (a workspace member): wasmtime `=44.0.3` with `cranelift`, `runtime`, `std` and `parallel-compilation`; `target-lexicon` 0.13 for the host's triple; `sha2`.
    - **One `Config`** (`config(target)`): multi-memory, NaN canonicalisation, parallel compilation, and an explicit target with its CPU flags named, so nothing of the compiling machine leaks into the code.
      - **`Target::baseline(triple)`** is what the Build makes code for: x86-64-v2 on x86-64 (`has_sse3`, `has_ssse3`, `has_sse41`, `has_sse42`, `has_popcnt`, `has_cmpxchg16b`), the Apple M1's level on aarch64 (`has_lse`, `has_pauth`, `has_fp16`).
      - **`Target::host()`** is what the runner and the tests run on: this machine's triple at the same baseline, less any flag this CPU lacks. Reading the source showed why: wasmtime refuses to compile *at load* for an engine whose flags the CPU lacks (`Engine::_check_compatible_with_native_host`), so a fixed baseline engine would leave an older aarch64 Linux machine (no LSE) with no way to run. On every Apple silicon Mac and every x86-64-v2 PC the two are equal, and the Build's code loads.
    - **`precompile(module, target)`** returns `MachineCode { bytes, sha256, compat }`: wasmtime's serialised module, its sha256, and the sha256 of the engine's `precompile_compatibility_hash`.
    - **`load(engine, module, code, cache)`** goes in the task's order and returns the module, its `LoadPath` (`MachineCode`, `Cache`, `Compiled`), why the code was passed by (`Skipped`: `Absent`, `Damaged`, `Foreign`, `Refused` with wasmtime's words) and the ms to a runnable module.
      - The order is: the code's sha256 → the compatibility hash → `deserialize` → the local cache → compile at load, and then keep the result in the cache.
      - **The cache:** one `<key>.cwasm` per module and engine, keyed by the sha256 of both. Each entry holds its own sha256 followed by the code. Entries are written whole and renamed into place. A damaged entry is removed and the module compiled again. Past 512 MiB (`CACHE_LIMIT`) the least recently used go first. A cache that cannot be read or written never stops a program.
    - **`cobolt_wasm::host::engine()`** is now `cobolt_runner_engine::engine()`. The generator's tests run modules on the runner's own engine (feature `host`).
    - **The M1 spike** (`spike_baseline_code_in_a_host_detecting_engine`, Apple M3 Pro, aarch64-apple-darwin): **yes.**
      - The baseline `.cwasm` loads in an engine that detects the host's CPU, with the same settings, and answers the model's value.
      - **It also loads in an engine with wasmtime's default settings**, which has no NaN canonicalisation. wasmtime's own load check (`Metadata::check_compatible`) compares the triple, a fixed list of code-generation flags, the tunables and the wasm features. It checks the CPU flags against the CPU. `enable_nan_canonicalization` is on its list of settings whose value "shouldn't matter" (`Engine::check_compatible_with_shared_flag`), so it is **not** checked.
      - **The settings it forces:** none beyond the triple. So the compatibility-hash step is what keeps code made with other settings out, and it stays ahead of `deserialize`. The two engines' hashes differ (`812d5a0c…` for ours, `6786c619…` for the defaults).
      - The runner therefore keeps the explicit-target engine (`Target::host()`). A host-detecting engine would load the Build's code but hash differently, so `load` would compile instead.
    - **Measured** (macOS arm64, debug build): `cargo test -p cobolt-runner-engine -- --nocapture`, 5 of 5.
      - **`every_load_path_runs_the_same_program`:** 22 of 22 checks. The module has 300 functions × 64 steps, is 594 680 bytes, uses two memories, and folds in a canonical NaN every 16 steps. Every path answers the Rust model's −1672998543259288415. The machine code is 1 068 488 bytes and took 1 140 ms to make. Load times:
        - the Build's machine code: `MachineCode`, 77.6 ms;
        - a foreign compatibility hash: `Compiled` (`Foreign`), 642.6 ms;
        - truncated code: `Compiled` (`Damaged`), 723.0 ms;
        - absent code: `Compiled` (`Absent`), 643.2 ms;
        - an empty cache: `Compiled`, which fills it, 891.4 ms;
        - **a cache hit on the second load:** `Cache`, 61.8 ms;
        - truncated code with a cache hit: `Cache` (`Damaged`), 80.8 ms;
        - a damaged cache entry: `Compiled` again, 399.7 ms.
      - **`precompile_targets_are_explicit_and_repeatable`:** aarch64 for Linux, Windows and macOS. Each target's machine code is the same bytes twice, and the three differ from one another (1 199 560, 1 199 624 and 1 068 488 bytes). x86-64 needs wasmtime's `all-arch` and is not built on an aarch64 machine: the Build makes code for its own platform.
      - **`the_compatibility_hash_follows_the_settings`:** two engines with these settings hash equal, and wasmtime's defaults hash differently.
      - **`the_cache_keeps_the_most_recently_used_within_its_limit`:** with a limit of two and a half entries, the least recently used entry goes and the touched one stays. The loads go `Compiled, Compiled`; `Cache`; `Compiled`; `Cache, Compiled`, and 2 entries are left.
      - **`cargo tree -d -p cobolt-runner-engine`:** one `wasmtime`, v44.0.3. Its duplicates are wasmtime's own (`hashbrown` 0.16/0.17, `syn` 2/3) and the test-only `wasm-encoder` 0.244 beside wasmtime's 0.246.
      - **`cargo test -p cobolt-wasm --no-fail-fast`** on the new engine: 118 tests in 32 targets, 0 failed. That includes `dependency_guard` (3 of 3, with `rayon`, `target-lexicon` and `sha2` now in the host's tree), `corpus`, `compile_determinism`, `native_console` and `native_return_code`.
      - **`desktop_wasm_unchanged`** (`cobolt-web-tests`): 3 of 3 artefacts match the pin.
      - **`dependency_guards_063`** (`cobolt-compiler`, over `Cargo.lock`): 3 of 3.

- [x] **T14 — `cobolt-program-package`: the payload inside one executable** (R1, R8)
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
  - *Done 2026-11-15 (2.0.512):*
    - **The crate** `crates/cobolt-program-package` (a workspace member). Its only dependencies are `sha2` and `serde`.
    - **The payload** (`encode`, `decode`), format 1, little-endian:
      - a 56-byte header (`PRPAYLD\0`, the version, the part count, the total length, and the table of contents' own sha256);
      - one 56-byte entry per part (`{kind, offset, len, sha256}`);
      - the parts, each at a 16-byte boundary.
      - **Kinds:** `MODULE` (1), `MACHINE_CODE` (2) and **`MACHINE_CODE_ENGINE` (3)**. The third is new: it carries T13's compatibility hash, which the table's four fields had no place for. A kind the reader does not know is kept and passed by, so an older runner reads a newer Build's payload as far as it can. A newer format version is refused, naming it.
      - **Checks:** a damaged table of contents is refused before any part is read. Each part is checked when it is taken: `Payload::module()` refuses a damaged module. Damaged machine code is only reported (`Part::verified`), so T13's `load` compiles at load instead of refusing the program.
    - **The carriers** (`embed`, `locate`, `format_of`):
      - **ELF and PE:** the payload after the image at a 16-byte boundary, then the 32-byte `PRLOC` trailer: offset, length, version, and `PRLOC\0\0\0` as the file's last bytes. A file without the trailer (a bare runner, or one a packager stripped) is named `NoPayload`.
      - **Mach-O:** a read-only `__PRPAYLOAD` segment (72 bytes, no sections) inserted before `__LINKEDIT`'s load command. The payload takes `__LINKEDIT`'s old place, padded to a 16 KiB page. `__LINKEDIT` moves up by that much in the file and in memory, with every file offset into it: `LC_SYMTAB`, `LC_DYSYMTAB`, `LC_DYLD_INFO(_ONLY)`, the nine `linkedit_data_command`s (chained fixups and the exports trie included) and `LC_NOTE`. The old signature is dropped and a new ad hoc one written (`codesign.rs`, pure Rust): a SuperBlob with a CodeDirectory v0x20400 (SHA-256, 16 KiB pages on Apple silicon and 4 KiB on Intel, as `codesign` chooses; two special slots; `__TEXT` as the executable segment), an empty requirement set and an empty signature wrapper. The reserved space is rounded up to 16 bytes, as `codesign` reserves it.
      - **Header room:** a fat binary or a non-executable is refused. A runner without header room for one more load command is refused, naming the bytes free and needed. A Rust test binary had 48 bytes free, so the runner (T15) must be linked with `-Wl,-headerpad,0x1000`, as this crate's `build.rs` links its tests.
      - **`write_executable`** writes a new file and renames it into place, executable on Unix, so macOS checks its new signature afresh rather than a cached one.
    - **`RunnerSettings`** (the type only, `serde`, every field defaulted): the values `generate_main_rs_for` and the `bake_*` functions put into `main.rs`. These are the name, version, main form, theme, entrance and exit effects, connection catalogue, file memory limit, focus ring, indexed engine, locale, currency, link schemes and `remote`.
    - **libsui evaluated** (0.16.4, MIT, the Deno authors, used by `deno compile`). The choice is **the hand-written carriers**:
      - **libsui's carriers:**
        - on Apple silicon, a `__SUI` segment with one section before `__LINKEDIT`, re-signed ad hoc, as here;
        - on Intel Macs, no segment (`intel_mac.rs`): a second path;
        - on ELF, a `PT_NOTE` program header, rewriting the header table;
        - on PE, a resource (RCDATA).
        - It finds them at run time through FFI: `getsectdata`, `dl_iterate_phdr`, `FindResource`.
      - **Against it:**
        - it would add to every Build's tree a second `object` (an exact pin `=0.36.3` beside the workspace's 0.39.1, which would also block that crate's updates), `zerocopy` 0.7 beside 0.8, and `windows` 0.58 beside 0.57, 0.61 and 0.62;
        - it declares no `rust-version`;
        - its Intel-Mac path differs from its Apple-silicon one, where ours is one path for both;
        - its code is 3 589 lines, against 763 here for the payload, the carriers and the signer together (the settings and the tests apart).
      - **What it does better, noted for later:** its ELF note survives `strip`, where our overlay does not. The runner names the missing payload, as the plan's risk table says.
    - **Measured** (macOS arm64, Apple M3 Pro): `cargo test -p cobolt-program-package -- --nocapture`, 6 of 6.
      - **`a_copy_of_this_test_carries_a_program_and_still_runs`**, 13 of 13: the test's executable (1 657 272 bytes) carries a 600 346-byte payload, giving 2 255 136 bytes. The payload has four parts: a module of 180 008 bytes, machine code of 420 000, the engine hash of 32, and an unknown kind of 10. Timings: embed 83.9 ms, write 0.6 ms, read back and decode 0.4 ms. Checks:
        - **`codesign --verify --strict`:** valid on disk, satisfies its Designated Requirement;
        - **`codesign -s -` of the same file writes the same file, byte for byte** (the same CDHash: v=20400, 138+2 hashes);
        - the copy **runs** (287.2 ms) and reads its own four parts back through `current_exe`, each matching its sha256;
        - the bare executable has no payload, and a packaged one is not packaged again.
      - **`a_damaged_table_of_contents_is_refused`**, 13 of 13: a changed offset, length, kind, recorded sha256, count, total length or table checksum; cut short; cut inside the table; no magic; a part outside the payload with the checksum recomputed (`Damaged`); a whole but newer format (`NewerFormat(2)`).
      - **`a_part_whose_sha256_does_not_match_is_refused`**, 4 of 4: a damaged module is refused naming `MODULE`, also inside an executable; damaged machine code is refused naming `MACHINE_CODE`, and the module beside it still reads.
      - **`the_overlay_carrier_round_trips_on_any_system`**, 12 of 12, on ELF and PE files made in the test: the image kept, the payload read back, not packaged twice, a bare file without one, a trailer pointing elsewhere refused, a stripped file named `NoPayload`.
      - **`the_macho_carrier_moves_linkedit_and_signs_again`**, 12 of 12, on a Mach-O made in the test: one command more, `__PRPAYLOAD` where `__LINKEDIT` began, `__LINKEDIT` and its tables moved by the payload's pages with their bytes where the offsets say, the new signature ending the file and `__LINKEDIT`, the text unchanged, 40 bytes of room refused (`NoHeaderRoom { free: 40, needed: 72 }`).
    - **The other carriers:** `build-all-platforms.yml` gains "Test the program package on this system's carrier" in each of the four builds (Linux and Windows overlay, both Macs' segment). It runs at the workflow's next dispatch and **is not yet measured**. The Intel Mac's 4 KiB pages are the one choice made without a machine to check it on.

- [x] **T15 — `prc-runner` for console programs** (R4 runner half, R5, R6, R10, R16, R28 runner half; AC5 runner half)
  - Files: new `crates/cobolt-runner/` (a workspace member; binary `prc-runner`).
  - Do:
    - **Loading:** read the payload through `PRLOC`, then validate the manifest's range.
    - **Capabilities:** refuse any it does not provide, naming it. Messages come in six languages, chosen by the system locale, and never say "cobolt".
    - **Running:** load the machine code through T13, link `pr` and the console part of `prn` with `linker_for(abi, imports)`, run, and exit with the program's code. That needs `RETURN-CODE` in the generator and the `exit` import's signature (A29), left open by T12 and 2.0.498.
    - **Test pins:** `now` and `random` come from `PRC_TEST_CLOCK` and `PRC_TEST_SEED`.
  - Verify:
    - `cargo test -p cobolt-runner -- --nocapture`:
      - a console fixture module, packaged with T14 into the built `prc-runner`, gives under the pins the same output as `rcrun run` of the same program, byte for byte;
      - a module that declares `sql` is refused, naming `sql`;
      - a module of ABI 1 runs;
      - the refusal text exists in six languages and contains no "cobolt".
  - **Generator half done (2.0.507, 2026-11-15)**; the runner half waits for T13 and T14 (both done: 2.0.511, 2.0.512). Link `prc-runner` with `-Wl,-headerpad,0x1000` on macOS, or T14 refuses it (`NoHeaderRoom`).
    - **`RETURN-CODE` in a desktop module** is a hidden whole-number register (`BINARY`, 18 digits, signed), zero when the run starts. It compiles in `MOVE`, `ADD`, `SUBTRACT`, `MULTIPLY`, `COMPUTE`, `DISPLAY` and comparisons with a number, `EVALUATE` included.
    - **Why only whole numbers:** the interpreter holds `RETURN-CODE` as a register of its own whose type follows the last value moved in (a fraction or characters stay as they are). The module cannot follow that, so it compiles only what it can prove is a whole number and refuses the rest, naming why. The program then takes the Cargo path, which ends as the interpreter does. Refused: a value that may carry a fraction or characters (a literal, an item with decimals, a division), `DIVIDE`, `ON SIZE ERROR` with it as a receiver, and any other use (`STRING`, a move into characters, a comparison with characters). A browser build still refuses it as undefined.
    - **`prn.exit(code)`** (ABI A30, capability `env`): where the run ends (after `STOP RUN`'s flush, `GOBACK` or `EXIT PROGRAM` from the main program, past its last statement), the module hands `RETURN-CODE` over, held to the 32-bit range as the interpreter's exit code is. The test host records it in `Outcome.exit_code`. A program that never names `RETURN-CODE` imports nothing new, and its module is the browser module byte for byte.
    - **Tests:** `tests/native_return_code.rs`: 14 programs × 2 memory variants against the interpreter (lines and exit code), 9 refusals, 4 programs past 18 digits, the browser build and a module that never names it; 142 of 142 checks. `native_console` now expects `RETURN-CODE` to compile natively.
    - **Past 18 digits** the module keeps the low 18 digits; the interpreter keeps more: WASM-PARITY #52. The operator decided on 2026-11-15 to keep the truncation (2.0.510).
    - **Found in passing, each a fix of its own** (fixed in 2.0.508 and 2.0.509 on `fixes-2x-run-end-flush` and `fixes-2x-exit-code-wrap`, picked here; 2.0.510 lets the "last statement" case of `native_return_code` DISPLAY again):
      - `pr_run` does not flush when the main program goes past its last statement, so on the test host the DISPLAY output of such a run is lost. That is older than T15.
      - The interpreter's exit code wraps past 64 bits (`CobolValue::as_i64` casts its `i128`): 2^64 ends with 0, −(2^64)+5 with 5.
  - *Runner half done 2026-11-15 (2.0.513):*
    - **The crate** `crates/cobolt-runner` (a workspace member, binary `prc-runner`), linked on macOS with `-Wl,-headerpad,0x1000` by its `build.rs` (`cargo:rustc-link-arg-bins`). Its dependencies are T13's engine, T14's package, `cobolt-wasm-abi` (`scan`), `wasmtime` 44.0.3, `chrono` and `serde_json`; it does not link the generator or the interpreter, and adds no third-party package to `Cargo.lock` (the generator and the parser are test-only dependencies).
    - **Start** (`run_self`): the runner reads its own file. Then:
      1. It finds the payload (`locate`, `decode`). A file with none is `NoProgram`; anything damaged is `Damaged`, with the package's words.
      2. It reads the manifest from `prmanifest` before loading anything. An ABI outside 1 to `ABI_VERSION` is refused, naming both numbers.
      3. It refuses every capability outside `PROVIDED_CAPABILITIES` (`console`, `env`), naming them, and every import outside `PROVIDED_IMPORTS`.
      4. It loads through T13's `load` with the `MACHINE_CODE` and `MACHINE_CODE_ENGINE` parts. The cache is `PRC_RUNNER_CACHE`, or the user's cache folder under `PowerRustCOBOL/runner`. `PRC_RUNNER_TRACE` names the load path on stderr.
      5. It instantiates the module, checks that `pr_abi` equals the manifest's ABI, calls `pr_run`, and exits with the code `prn.exit` received (0 without one).
    - **`linker_for(engine, abi, imports)`** links exactly the imports the module declares:
      - `PROVIDED_IMPORTS` lists `(ABI, module, name)` rows that are never removed. ABI 1 has `pr.flush`, `abort`, `now` and `random`, and `prn.con_read`, `env_get` and `exit`.
      - `pr.query`, `pr.remote` and the conformance imports are not provided yet. They are named when a module has them.
    - **The imports:**
      - **`flush`:** each `DISPLAY` command is one line on stdout, and an `ERROR` goes to stderr.
      - **`abort`:** the output so far, then the error in the system's language and exit 1.
      - **`con_read`:** stdout is flushed first, then one line of stdin is read, its line end removed; −10 at the end of the input.
      - **`env_get`:** this process's variables and arguments, with the test host's kinds 0–3.
    - **Messages** (`messages.rs`): eight messages in en, es, pt, fr, ja and zh, chosen by `LC_ALL`, then `LC_MESSAGES`, then `LANG`. `C`, `POSIX` and an unknown language get English. The runner is "the PowerRustCOBOL runner"; no text names the crate.
    - **Pins** (`pins.rs`): `PRC_TEST_CLOCK` and `PRC_TEST_SEED` are read as `cobolt-runtime::test_pins` reads them, and an unreadable value is named on stderr in the interpreter's words.
      - **The clock is the local one.** `now` answers the local date and time (the instant plus the zone's offset) as milliseconds since 1970, because the routines read it as a calendar. So `ACCEPT … FROM DATE/TIME/DAY/DAY-OF-WEEK` and the first 16 characters of `CURRENT-DATE` are the desktop's, as the interpreter shows them, in any zone.
      - **Open:** `CURRENT-DATE`'s last five characters still read `+0000` (**WASM-PARITY #53**, new). The module needs the offset from the runner before AC19 judges a form that shows `CURRENT-DATE` whole.
      - **`random`** answers the interpreter's generator from `PRC_TEST_SEED`.
    - **Found and settled: `FUNCTION RANDOM` never asks the host.** The module keeps its own generator (`cobolt-wasm-rt` `func.rs`), with the interpreter's algorithm and first state. A `random` import therefore cannot pin it.
      - **The generator side:** a desktop program module that calls `RANDOM` now starts with `rt_random_pin`. That reads `PRC_TEST_SEED` through the `env_get` the runner already has (capability `env`), and starts the generator as `FUNCTION RANDOM(seed)` does. That is what the Cargo binary does with its own environment. A value that is not a whole number from 0 to 2^64−1 pins nothing, as in the interpreter.
      - **No new import and no ABI change.** A program without `RANDOM`, a browser module and a form's module are unchanged. A form opens more than once, so its pin waits for the desktop form host (M2).
      - **The routine blob was rebuilt** (234 427 → 235 145 bytes). The new routine is appended at the end of `accept.rs` and `lib.rs`, writes `mix_seed`'s arithmetic out so that function keeps its one caller, and indexes nothing, so the routines a browser module links keep their bytes.
    - **Measured** (macOS arm64, Apple M3 Pro, debug build):
      - **`cargo test -p cobolt-runner -- --nocapture --test-threads=1`:** 5 of 5 (2 unit, 3 integration).
        - **`a_packaged_console_program_matches_rcrun_run_byte_for_byte`, 19 of 19:**
          - The program: console `ACCEPT` ×2, `ENVIRONMENT` ×2 (set, not set), `ARGUMENT-NUMBER`, `COMMAND-LINE`, `ARGUMENT-VALUE`, `DATE`, `TIME`, `DAY`, `DAY-OF-WEEK`, `CURRENT-DATE` (its 16 characters), `RANDOM` ×3, and `RETURN-CODE` 7.
          - It compiles to an ABI 1 module of 136 835 bytes (imports `prn.con_read`, `env_get`, `exit`; capabilities `console`, `env`) in 12.7 ms. Its machine code is 1 465 944 bytes, made in 207.8 ms. Packaged into the built debug `prc-runner` it is 46 399 024 bytes, written in 1 940 ms.
          - Under `PRC_TEST_CLOCK=2026-01-02T03:04:05.67-03:00` with seeds 20261114 and 7, its stdout equals that of `rcrun run`, built fresh from the same source, byte for byte: 294 bytes in 10 lines each. Both exit with 7.
          - The runner loads the Build's machine code (`loaded by MachineCode`).
          - The pins are not a no-op: the lines show `date 260102 time 03040567 day 26002 dow 5` and `stamp 2026010203040567`, and the two seeds give different `RANDOM` lines.
          - Start-up: 67.5 ms for a warm second run, against `rcrun run`'s 22.0 ms. The first run of a newly written file is 1.5 s on macOS, while the system checks it.
        - **`a_capability_the_runner_lacks_is_refused_by_name_in_six_languages`, 34 of 34:**
          - A module whose manifest asks for `console`, `env` and `sql` exits 1 under `LC_ALL` en_US, es_ES, pt_BR, fr_FR, ja_JP and zh_CN. In each, stderr is exactly that language's text and names `sql`, and stdout is empty.
          - A manifest naming ABI 2 is refused as "ABI 2 … ABI 1 to 1".
          - A bare `prc-runner` says, in Portuguese, that it carries no program.
        - **`every_message_exists_in_six_languages_without_the_crate_name`, 113 of 113:** 8 messages × 6 languages; every text is distinct, has no "cobolt", and names every capability; 8 locale names read.
        - **Unit tests:** the pinned clock answers its local wall time (1 767 323 045 670 ms), and unreadable pins are named and pin nothing.
      - **`native_random_pin`** (`cobolt-wasm`), 40 of 40: 11 values of `PRC_TEST_SEED` (none, 0, 7, 20261114, 2^64−1, `"  42\t"`, `"+42"`, `"abc"`, `"-1"`, 2^64, empty) each give the interpreter's 5 lines (55 in all). A desktop program without `RANDOM` is the browser module byte for byte.
      - **`cargo test -p cobolt-wasm --no-fail-fast`** with the rebuilt blob: 118 tests in 31 targets, 0 failed (`corpus`, `player_fixture`, `rt_blob`, `compile_determinism`, `native_console`, `native_return_code` included).
      - **`desktop_wasm_unchanged`:** 3 of 3 PowerAnalyticsWeb artefacts match their pins. **`web_golden`:** 10 match.
      - **`cargo test -p cobolt-wasm-rt`:** 27 of 27 (9 unit, 18 integration).
    - **Not done here:** the Build does not yet make this file (T16), and the runner is not yet built or signed in CI. Its release size is measured in M1 (C12).

- [x] **T16 — The Build classifies and takes the WebAssembly path** (R1, R7, R8, R13, R14, R21, R24, R25; AC1 local half, AC11, AC12, AC13, AC15, AC17 compiler half; C8, C11)
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
  - *Done 2026-11-15 (2.0.514):*
    - **The order of a Build** (`build_core`): regenerate, front end, **classify**, then the WebAssembly path (generate, machine code, assemble, install) or the Cargo path (the toolchain check, then today's steps), then `finish_install`, which both paths share (the old steps 11b to the External Crates, factored out unchanged). The phase clock now starts at the front end, so a Cargo Build reports `Regenerate, FrontEnd, Classify, Cargo, Install`.
    - **`classify`** checks, in this order, and the first that holds is the reason: Debug → `DebugBuild`; `BuildOptions.force_cargo` or `[desktop] build_path = "cargo"` → `Chosen`; `[[crates]]` → `ExternalCrate`; an `EXEC RUST` block in the main program or a form's program (`cobolt_semantic::exec_rust`) → `ExecRust { file, line }`; forms → `Refused`, naming the main form and its `.cfrm` at line 0 with the forms reason (M2); no runner → `RunnerUnavailable`; then the generator's first refusal → `Refused { construct, reason, file, line }`.
      - **The runner** is `BuildOptions.runner` when that is a file, else `PRC_RUNNER`, else `prc-runner[.exe]` beside the running program. So `rcrun` and the IDE in `target/debug` find `target/debug/prc-runner`, and a test executable in `target/debug/deps` finds none: the existing compiler tests stay on the Cargo path, with `RunnerUnavailable`.
      - **The file of a refusal:** the generator's error now carries its program unit (`GenError.program`). A PROGRAM-ID that a Common Code file declares names that file; anything else names the main program's file.
    - **The WebAssembly path** (`wasm_build`): `cobolt_wasm::compile_program` with `native: true`; machine code for `Target::baseline` of this machine's triple (a failure is logged and the program is packaged without it; skipped by `no_machine_code` or `[desktop] machine_code = false`); the payload (module, machine code, its engine's compatibility hash) embedded into a copy of the runner; staged at `<build folder>/cobolt-build-<name>/wasm/<exe>` and renamed into `bin/`.
    - **The toolchain check** moved after `classify` (C8). Its failure is `CompilerError::CargoNeedsToolchain { reason, tool, detail }`; the text still contains "Rust toolchain is required", which the IDE matches until T18.
    - **In the generator** (desktop modules only; browser bytes unchanged): files on disk (subject `file NAME`), `SORT … USING/GIVING` (an INPUT and OUTPUT PROCEDURE still compiles), a built-in whose Desktop class is `Cargo(_)` (subject `COBOL::"NAME"`), and members of controls, forms and the application object, `INVOKE` included, are refused with the reason of their Desktop class (T10).
    - **Settled while building:**
      - The named tests are **sections of one test**, `desktop_wasm_build::the_build_classifies_and_takes_the_webassembly_path`: AC1 empties `PATH`, which is process-wide, so the test that does so must be the only one in its file. The runner is built inside it first (`$CARGO build -p cobolt-runner --bin prc-runner`), before `PATH` is emptied.
      - `BuildResult.build_dir` stays a top-level field (the WebAssembly path stages there too). `Refused` carries the `reason` as well as the construct, file and line, so the message says why.
      - **`DesktopControl { form, control }` is not added yet.** Until M2 a project with forms is refused as a whole, naming its main form; the per-control reason comes with M2's form support.
    - **Measured** (macOS arm64, Apple M3 Pro, debug build):
      - **`desktop_wasm_build`, 28 of 28,** with `PATH` an empty folder and `CARGO_HOME`, `RUSTUP_HOME` empty folders. The debug runner is 43.0 MB.
        - **AC1, AC13, AC15:** a console project (main program + Common Code) builds on the WebAssembly path in 2 015.5 ms into a 46 211 504-byte file: regenerate 0.1, front end 3.9, classify 0.0, generate 8.4, machine code 140.9, assemble 1 827.6, install 2.4 ms. `bin/<name>` runs in 598.1 ms and prints `TOTAL 001415 COUNT 0003`; `bin/` holds that one executable, `assets/logo.txt` and the three notices; `dist/` holds the same file, `assets/`, `data/` and the notices, and runs too. None of the 7 statement lines is in the built file.
        - **AC12:** `EXEC RUST` at `src/main.cbl` line 5 is named in 0.8 ms; nothing is compiled or staged.
        - **AC11:** `file LOG-FILE` at `src/files.cbl` line 13 (a Common Code file) in 2.3 ms, and `COBOL::"HTTP-GET"` at `src/main.cbl` line 9 in 3.6 ms, each with its reason.
        - **AC17:** `build_path = "cargo"` with no toolchain stops in 1.8 ms with `CargoNeedsToolchain` and reason `Chosen`; `bin/`'s tree hash is unchanged.
        - A Debug Build gives `DebugBuild`; no runner, or a runner path that is not a file, gives `RunnerUnavailable`; the chosen build folder, the typed toolchain error and the current-code twins (`FRESH-T16-BOOK` compiled, `STALE-T16-BOOK` kept with regeneration off) all hold.
      - **`native_desktop_refusals`** (`cobolt-wasm`), 10 of 10, 10 programs in 23.1 ms:
        - refused as a desktop module, each at its line with its reason: `file LOG-F` (M3, unit `T16G`), `SORT … USING/GIVING` (M3), `COBOL::"HTTP-GET"` (M4), `ME::TITLE` read and written (M2; the parser gives the name in capitals) and `INVOKE` (M2);
        - a refusal inside the nested program `INNER` names `INNER`;
        - the same disk file in a browser module keeps the browser's refusal;
        - a SORT with an INPUT and an OUTPUT PROCEDURE, and a plain console program, compile.
      - **`cargo test -p cobolt-compiler --no-fail-fast`:** 18 targets, 298 passed, 1 failed, 9 ignored. The one red is `example_calls_name_only_what_exists`, red since PowerOficina came in (2.0.390): its 127 unresolved CALLs are all PowerOficina's test programs calling the programs it lists under `[files] backend`, which `common_code_names` does not read. No T16 change reaches it. Fixed in 2.0.515 (`fixes-2x-oficina-calls`, picked here): `cobolt-compiler` is 299 passed, 0 failed.
      - **`cargo test -p cobolt-wasm --no-fail-fast`:** 34 targets, 120 passed, 0 failed. **`desktop_wasm_unchanged`:** 3 of 3 PowerAnalyticsWeb artefacts match their pins. **`web_golden`:** 10 match.
    - **Owed:** the release runner's size (C12, M1). Assembling takes 1.8 s with the 43 MB debug runner, most of the Build.

- [x] **T17 — `rcrun build --cargo`, `--no-machine-code`, and the install hint** (R15, R24, R25; A4; AC8 first half, AC17 CLI half)
  - Files: `crates/cobolt-cli/src/main.rs`.
  - Verify: new `cobolt-cli` tests, with `rcrun` built first:
    - `--cargo` gives path Cargo, reason `Chosen`;
    - `--no-machine-code` gives a file whose table of contents has no `MACHINE_CODE` entry, while the default Build's has one;
    - with no toolchain, an `EXEC RUST` project prints how to install Rust 1.95.0, exits non-zero, and leaves `bin/` untouched.
  - *Done 2026-11-15 (2.0.516):*
    - **`rcrun build --cargo`** sets `BuildOptions.force_cargo`, the same as `[desktop] build_path = "cargo"`; **`--no-machine-code`** sets `BuildOptions.no_machine_code`. The help text, the module doc and the unknown-flag message name both.
    - **A successful Build prints its path:** `Path   : WebAssembly`, or `Path   : Cargo, because <reason>`. The `Chosen` reason now names both ways to choose it: "the Cargo path is chosen (the project's `[desktop] build_path = "cargo"`, or `rcrun build --cargo`)".
    - **The install hint (R25, A4):** when the Build fails with `CargoNeedsToolchain` or `Toolchain`, `rcrun build` prints the error, then how to install Rust 1.95.0 (the release PowerRustCOBOL is built with) with the official rustup.rs command for this platform, and exits with 1. It installs nothing. Any other failure (a missing manifest, a compile error) prints no hint.
    - **One install command:** `cobolt_compiler::rust_install_command()` is the text; the IDE's `toolchain::install_command()` now returns it (its three constants are gone), so the IDE's installer and `rcrun` cannot drift apart. `install_argv` is unchanged.
    - **Settled while building:** the test checks the Cargo path through the stop it makes with no toolchain (the reason `Chosen`, the hint, `bin/` unchanged) rather than through a full Cargo Build, which the compiler's own tests already cover. `cobolt-program-package` is a dev-dependency of `cobolt-cli`, so the test reads the built file's table of contents.
    - **Measured** (macOS arm64, Apple M3 Pro, debug build):
      - **`build_path_flags`, 16 of 16** (one test; the runner is built beside `rcrun` first, in 668 ms; the no-toolchain cases run `rcrun` with `PATH`, `CARGO_HOME` and `RUSTUP_HOME` set to empty folders):
        - the default Build takes 5 522.4 ms and makes a 46 195 088-byte file with parts `Module, MachineCode, MachineCodeEngine`; it prints `Path   : WebAssembly`, and the program prints `T17 TOTAL 000042`;
        - `--no-machine-code` takes 1 686.9 ms and makes a 44 898 224-byte file with the part `Module` only; its first run, which makes the machine code at load, takes 674.2 ms and prints the same line (AC8 first half);
        - `--cargo` with no toolchain stops in 16.2 ms, exits non-zero, names the Cargo path and the reason `Chosen`, prints the install hint, and leaves `bin/` as it was;
        - `EXEC RUST` at `src/main.cbl` line 5 with no toolchain stops in 14.1 ms, exits with 1, names the line, prints the hint with the command in full, and leaves `bin/` (`keep.txt` only) as it was (AC17 CLI half);
        - a manifest that is not there exits non-zero with no hint;
        - the command printed on macOS: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.95.0`.
      - **`cargo test -p cobolt-cli --no-fail-fast`:** 26 targets, 68 passed, 0 failed, 6 ignored (the six run with `--ignored`).
      - **`cargo test -p cobolt-ide --bin cobolt-ide toolchain`:** 21 of 21, including `the_command_shown_is_the_command_run` (the command shown is the one `install_argv` runs). **`cargo test -p cobolt-compiler --lib missing_toolchain`:** 2 of 2.

- [x] **T18 — The IDE: the Build's path, its phases, the install offer, and the Desktop settings** (R18, R24, R25; AC17 IDE half; i18n)
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
  - *Done 2026-11-15 (2.0.517):*
    - **New module `crates/cobolt-ide/src/build_path_report.rs`:** `BuildFailure` (the message, `needs_rust`, the `CargoReason`), made from the `CompilerError` on the build thread; `phase_name`, `reason_text`, `path_line`, `phases_line` and `report_lines` in the IDE's language; `rust_offer`, the decision to offer.
    - **`app.rs`:**
      - The build channel carries `BuildFailure`. The Run-intent note that matched "Rust toolchain is required" now reads `needs_rust`, so no text is matched any more.
      - After a Build, the Output gets the path line and the phases line (also in the Build log as details), and the Building dialog's summary ends with the path line.
      - `offer_rust_install(&BuildFailure)` opens the first-run offer at Build time, unless an offer is already on screen. The Building dialog and the details window step aside.
      - `show_toolchain_prompt`: a Build's offer has its own title, says why the Build needs Cargo, leaves out the first-run "why", and does not record the first-run answer.
    - **`toolchain.rs`:** `FirstRunPrompt.at_build`, `FirstRunPrompt::at_build(status, reason)`. A status the IDE's probe finds usable is shown as missing, since the Build could not run it. At Build time one decline is final.
    - **Settings:**
      - `SettingsDraft.desktop` is filled from `[desktop]` or the defaults. `apply_kind` writes it only when it differs from what the project says.
      - `web_settings::show` draws a Desktop project's section (Build path, machine code, a note under each), with a shared `section_heading`.
      - The New Project options `build_path` (`wasm`, `cargo`) and `machine_code` live in `cobolt-project-tools/src/templates.rs` and are written by `create.rs`, so the dialog and the `create_project` tool offer them alike. `new_project.rs` translates both.
    - **i18n:**
      - 27 new `Tr` keys in six languages: the section, the two settings, their two choices and two notes; eight phases; six reasons plus the at-line form; the path and phases lines; the offer's title and text. The offer's buttons are the first-run ones, so no button key is new.
      - `rust_check_why` and `rust_check_lost` are rewritten in six languages: without Rust, only Cargo-path Builds stop.
    - **Settled while building:**
      - **Six reasons, not seven:** `DesktopControl` comes with M2 (T33), and its `Tr` key with it.
      - **The choice labels are short** (`WebAssembly (no Rust needed)`), because the long ones made the New Project window wider than its fixed size in Portuguese (`the_window_does_not_grow_while_nobody_touches_it`).
      - **`a_desktop_project_draws_nothing` became `a_desktop_project_draws_only_its_build_rows`:** the old rule (a Desktop project has no section) is what T18 changes. The test now checks that the Desktop rows are drawn, that no Web or Mobile row is, and that drawing leaves the draft as it was.
    - **Measured** (macOS arm64, Apple M3 Pro, debug build):
      - **`cargo test -p cobolt-ide --bin cobolt-ide build_path_report`:** 3 of 3, 108 of 108 checks in 5.0 ms:
        - **six languages:** 8 phase names, 6 reasons and 24 report lines, each filled and keeping its file, line and construct;
        - **the offer:** a typed failure opens it with the reason; at Build time one decline is final, while the first-run offer still asks twice; any other failure opens nothing;
        - **the settings:** the defaults are read, and no `[desktop]` table is written for a project without one.
      - **`cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`:** 2 028 passed, 3 failed, 6 ignored.
        - `docs_embed` is the known red.
        - The two T18 reds are fixed (above), and the re-run of the 82 `new_project`, `web_settings`, `build_path_report`, `i18n` and `toolchain` tests is green. `no_empty_ui_translations` and `non_english_is_actually_translated` are among them.
      - **`cargo test -p cobolt-project-tools --no-fail-fast`:** 32 targets, 265 passed, 0 failed, 2 ignored, including `the_options_belong_to_their_templates` and `the_kind_and_its_options_pick_the_template_and_shape_the_project` (9 refusals).
      - **Owed to the operator:** the manual IDE Build on a machine with no toolchain (plan §6).

- [x] **T19 — The project tools' `build` reports the path, the reason and the timings** (R13, R18)
  - Files: `crates/cobolt-project-tools/src/tools/` (the `build` tool), and its schema text.
  - Verify: `cargo test -p cobolt-project-tools build` green. On fixtures, the tool's result names:
    - the path;
    - the reason, with the file and line for `ExecRust` and `Refused`;
    - each phase in ms;
    - the peak.
  - *Done 2026-11-16 (2.0.518):*
    - **`crates/cobolt-project-tools/src/tools/build.rs`:**
      - `BuildOutcome` gains `report` (a Desktop Build's report; `None` for a Mobile package).
      - The builder's failure is `BuildFailure` (the message plus the detail of a toolchain failure) instead of a string.
      - `report_json` writes `path`, `reason` on the Cargo path, `phases` (`phase`, `ms` to a tenth), `peak_bytes` and, on Cargo, `crates_compiled`.
      - `reason_json` writes `kind` and `text`, plus `file` and `line` for `ExecRust` and `Refused` (no `line` for a whole file), and `construct` and `why` for `Refused`.
      - `failure_json` turns `CargoNeedsToolchain` into `needs_rust`, `tool`, `path` and `reason` (a bare `Toolchain` into `needs_rust` and `tool`).
      - `wait` merges the report into the `built` answer and the detail into the `failed` one.
    - **`tools/mod.rs`:** the `build` tool's description names the new fields.
    - **Settled while building:**
      - **The answer carries no install command.** An agent that reads `needs_rust` tells the developer. Installing Rust is the developer's decision, made in the IDE's offer (T18) or by hand.
      - **The fields sit at the top of the answer,** beside `binary` and `bytes`, not under a sub-object, so a `built` answer reads the same with or without them.
    - **Measured** (macOS arm64, Apple M3 Pro, debug build):
      - **`build_answer_names_the_path_the_reason_the_phases_and_the_peak`** (unit, hand-made Build results and the build slot): 22 of 22 checks in 16.2 ms.
      - **New `tests/build_report.rs`, `the_build_tool_names_the_path_the_reason_the_phases_and_the_peak`** (real Builds, no Rust on `PATH`, `CARGO_HOME` or `RUSTUP_HOME`): 15 of 15 checks, 3 fixtures.
        - The console project took the WebAssembly path in 2 027 ms, in one call. Phases: regenerate 0.0, front end 0.7, classify 0.0, generate 7.8, machine code 150.6, assemble 1 844.3, install 1.5 ms. Peak 144 752 640 bytes.
        - The `EXEC RUST` program stopped with `needs_rust`, naming `src/main.cbl` line 5.
        - The disk-file program stopped naming `src/files.cbl` line 13 (`LOG-FILE`, files on disk at M3).
      - **`cargo test -p cobolt-project-tools build`:** 32 targets, 39 passed, 0 failed, 2 ignored.
      - **`cargo test -p cobolt-project-tools --no-fail-fast`:** 33 targets, 267 passed, 0 failed, 2 ignored.

- [x] **T20 — `prc-runner` ships with the IDE** (R6; AC13)
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
  - *Done 2026-11-16 (2.0.519):*
    - **`.github/workflows/build-all-platforms.yml`:**
      - The build step also builds `cobolt-runner`.
      - Staging copies `prc-runner` beside the IDE and `rcrun`: into the package folder on Linux and Windows, into `Contents/MacOS` on macOS.
      - On macOS it is signed ad hoc before the application, so `codesign --verify --strict` still covers the whole bundle.
      - The dependency check (`ldd`, `otool -L`) includes it.
      - A new step, *Check the packaged runner starts*, runs the staged runner on each platform and expects its "carries no program" message and exit code 1.
      - The `.deb` and `.rpm` content checks look for `/opt/powerrustcobol/prc-runner`.
      - The MSI and the `.dmg` are built from the staged folder (WiX `heat` harvests it; the `.dmg` wraps the signed application), so they carry it with no list to edit.
    - **`crates/cobolt-compiler/src/lib.rs`:** `runner_path` now asks the new `runner_near(exe)`. That looks beside the running program and, when the program was started through a link, beside the file the link points to. Two links ship: the loose `rcrun` of the macOS archive points into the application, and `/usr/bin/rcrun` points into `/opt/powerrustcobol`.
    - **`THIRD_PARTY_NOTICES.md`:** wasmtime, Cranelift, Pulley and regalloc2 were already listed (Apache-2.0 WITH LLVM-exception). Their "Used by" now names `cobolt-runner-engine` and `prc-runner`.
      - `cargo tree -p cobolt-runner -e normal` lists 92 crates. Outside MIT, Apache-2.0, BSD, ISC, Zlib and the Unlicense, they hold only the LLVM-exception family already listed, plus `unicode-ident`.
      - `unicode-ident` (Unicode-3.0) is reached only through procedural macros, so it is used while compiling and is not in the shipped binary.
    - **Decided: the Platform SDK is unchanged.**
      - `SDK_CRATES` is the source closure the Cargo path compiles against (`stage_sdk` and `prebuild_libs` are its only readers, both run by the release workflow).
      - The WebAssembly path compiles nothing from source. It uses the generator already linked into the IDE and `rcrun`, and the prebuilt runner that ships beside them.
      - So no crate is added to `SDK_CRATES`, and a Platform SDK installation's IDE builds Desktop projects on the WebAssembly path through the `prc-runner` beside it.
    - **Settled while building:**
      - **A test or an example of a checkout finds no runner** (`target/<profile>/deps`, `…/examples`). It names its runner (`PRC_RUNNER` or `BuildOptions::runner`), so T16's "no runner → `RunnerUnavailable`" check keeps meaning what it says.
      - **The IDE in a checkout finds `target/<profile>/prc-runner`** once `cargo build -p cobolt-runner` has put it there; until then its Builds take Cargo, naming the reason.
    - **Measured** (macOS arm64, Apple M3 Pro, debug build):
      - **New `cargo test -p cobolt-compiler --test runner_locator`, `the_runner_is_found_beside_the_ide_and_in_a_checkout`:** 8 of 8 checks in 5.0 ms. Layouts: a Linux or Windows package (IDE and `rcrun`), the macOS application, the archive's linked `rcrun`, a linked `/usr/bin/rcrun`, a checkout's `target/debug`, a checkout's test (none found), and a bare IDE (none found).
      - **The macOS staging, rehearsed on a stand-in bundle:** `rcrun`, `prc-runner` and the application signed ad hoc in the workflow's order; `codesign --verify --strict` accepted it. The runner start check passed (exit 1, "carries no program").
        - Through the archive-style link to the bundle's `rcrun`, with no `PRC_RUNNER`, `rcrun build` of a console project found the runner in `Contents/MacOS` and built on the WebAssembly path in 3 986 ms. The program printed its line and exited 0.
      - **Re-run green:** `desktop_wasm_build` (T16, 28 of 28 checks, "no runner → `RunnerUnavailable`" among them) and `cobolt-cli --test build_path_flags`.
      - **Owed to the operator:** the release workflow's dispatch. `gh` is not installed and no token is set, so an agent cannot dispatch it. Run **Actions → "Build PowerRustCOBOL (all platforms)"** on `features-2x-desktop-wasm` with *publish* off. Its new step shows the runner starting on each of the four platforms, and the `.deb`, `.rpm` and macOS signature checks cover the packages.

- [x] **T21 — The module corpus, kept for ever** (R5, R23; AC4)
  - Files:
    - new `tests/module-corpus/`: an index of every entry ever added, and its first entry, the console fixtures built by this release, with their expected outputs;
    - new `crates/cobolt-runner/tests/module_corpus.rs`.
  - Do: each release adds one entry within 256 KiB. The index is append-only.
  - Verify: `cargo test -p cobolt-runner --test module_corpus -- --nocapture`:
    - **`corpus_runs_on_this_runner`:** every entry runs and gives its recorded output. The result block counts releases, modules and identical outputs.
    - **`corpus_entries_are_never_deleted`:** an entry in the index with its files missing fails, and a planted case proves it.
  - *Done 2026-11-16 (2.0.520):*
    - **New `tests/module-corpus/`:**
      - `fixtures/console/` (the console, the environment, the arguments, the pinned clock, `RANDOM`, `RETURN-CODE` 7) and `fixtures/ledger/` (main and Common Code: arithmetic and `ROUNDED`, `DIVIDE … REMAINDER`, edited pictures, `STRING`/`UNSTRING`/`INSPECT`, `SEARCH`, `CALL`, an in-memory `INDEXED` file with `WRITE`, keyed `READ`, `REWRITE`, `DELETE`, `START` and `READ NEXT`, `SORT` with input and output procedures, `EVALUATE`, `RETURN-CODE` 3). Each has a `run.toml` with its input, pins, exit code and output.
      - `releases/2.0.520/`: the two modules this release's Build made, compressed (`.wasm.gz`), each with its `run.toml` as it is now.
      - `index.toml`: the first entry.
      - `README.md`: the rules and how a release adds its entry.
      - `.gitattributes` (`* -text`): no line-ending conversion, which would change the recorded sha256 on a Windows checkout.
    - **New `crates/cobolt-runner/tests/support/corpus.rs`:** the format (index, entries, run descriptions), the chain, `problems` (the whole-corpus check), gzip, and `run_module`, which puts a module alone into a copy of the runner and runs it under the pins. The test and the tool share it; the tool includes it by path.
    - **New `crates/cobolt-compiler/examples/module_corpus_add.rs`:** adds a release's entry. It builds each fixture through `build_project` (it must take the WebAssembly path), takes the module out of the built executable, and runs it alone on this checkout's runner. Unless the output and exit code are exactly the fixture's, it refuses. Otherwise it writes the files, checks the budget, appends the entry with its chain, and prints the `KNOWN` line.
    - **New `crates/cobolt-runner/tests/module_corpus.rs`:** `corpus_runs_on_this_runner` and `corpus_entries_are_never_deleted`. The runner crate gains the dev-dependencies `serde`, `toml`, `sha2` and `flate2` (workspace versions).
    - **Settled while building:**
      - **The budget is met by compressing.** A console module is about 137 KB, so two raw modules would pass 256 KiB; gzip brings each to about 67 KB.
      - **The modules are packaged alone, with no machine code.** Machine code belongs to one runner's engine (R16), so the corpus keeps only what lasts: the module. Each run also proves the runner makes the machine code itself.
      - **Append-only is proved three ways:**
        - each file's sha256 is in the index;
        - each entry's `chain` covers the previous chain, so a middle entry taken out breaks the next;
        - the test's `KNOWN` list names every entry, so even the last cannot go unnoticed.
      - **The recorded outputs were checked against the interpreter.** `rcrun run` printed them byte for byte with the same exit codes. The ledger was run as one source file holding both programs, with its file declared `WITH PERSISTENCE`, because the interpreter does not write to a `STORAGE IS MEMORY` file without it (see `cobolt-wasm/tests/memory_files.rs`).
      - **Line endings:** the test compares with a Windows console's CR LF read as LF.
    - **Measured** (macOS arm64, Apple M3 Pro, debug build):
      - **`module_corpus_add`:** console 137 161 bytes (66 586 compressed), ledger 147 516 bytes (69 114 compressed). Each ran in about 0.75 s with identical output. 135 700 of 262 144 bytes used.
      - **`cargo test -p cobolt-runner --test module_corpus -- --nocapture`:**
        - **`corpus_runs_on_this_runner`:** releases 1 (2.0.520), modules 2, identical output and exit code 2 of 2, in 4 630 ms (console 726 ms, ledger 704 ms, machine code made by the runner).
        - **`corpus_entries_are_never_deleted`:** 7 of 7 checks in 31.3 ms. Planted: a deleted module, a changed byte, the last entry taken out, and a middle entry taken out of a three-entry corpus.
      - **`cargo test -p cobolt-runner --no-fail-fast`:** 5 targets, 7 passed, 0 failed.

- [x] **T22 — CI: no toolchain, Rust releases, and across platforms** (R2, R5; AC1, AC2, AC3 console)
  - Files: new `.github/workflows/desktop-wasm.yml`, on tags and on dispatch.
  - Do:
    - **`no-toolchain`, on the four platforms:** a clean runner with Rust removed from `PATH`, `~/.cargo` and `~/.rustup`. It installs the release's IDE package, builds every project that takes the WebAssembly path at this milestone, and runs its check script.
    - **`determinism`:** generators built by Rust 1.95.0 and by `stable` compile the corpus and the reference projects with the same `SOURCE_DATE_EPOCH`, then compare the bytes. **Waits for F3.**
    - **`cross-platform`:** modules built on each platform run under the other three platforms' runners, compared as AC3 says.
  - Verify: dispatched once, outside the window; each job's result block is recorded in `WASM-PROGRESS.md`. AC1 is met only once every reference project takes the WebAssembly path. Until then the job lists those still on the Cargo path, with their reasons.
  - *Done 2026-11-16 (2.0.521):*
    - **New `.github/workflows/desktop-wasm.yml`** (tags `v*` and dispatch), five jobs:
      - **`tools`** (four platforms): `rcrun`, `prc-runner` and the checks' helper, built in release and side by side (signed ad hoc on macOS); the console fixtures built by that platform's `rcrun`, their modules kept for `cross-platform`. The tools travel as a tar, which keeps the executable bits an artifact's zip loses.
      - **`no-toolchain`** (four platforms): removes every folder holding `rustc`, `cargo` or `rustup`, with its home, from the machine and from `PATH`, plus `~/.cargo` and `~/.rustup`, and checks that none is left. Then `rcrun` builds and checks the fixtures, and builds the five reference projects, listing those on the Cargo path with the reason.
      - **`determinism`** (Rust 1.95.0 and `stable`, both on `ubuntu-22.04`): `module_hashes` with `SOURCE_DATE_EPOCH=1767225600`.
      - **`determinism-compare`**: the two lists must be identical.
      - **`cross-platform`** (four platforms): every platform's modules on this platform's runner.
    - **New `crates/cobolt-runner/examples/desktop_wasm_ci.rs`:** the helper (`fixtures`, `reference`, `cross`). It drives the real `rcrun build` as a separate program, recognises the path from `rcrun`'s `Path : WebAssembly` line and the reason from its `Cargo path:` line, and prints one result block for each check.
    - **New `crates/cobolt-compiler/examples/module_hashes.rs`:** one line per program, sorted. It covers the corpus fixtures and the reference projects (built as a Desktop Build builds them, `PATH` emptied so the Cargo path stops at its toolchain check and gives its reason) and every program of `tests/cobol` compiled alone as a desktop module (its sha256, or the generator's refusal).
    - **Settled while building:**
      - **The deviation from "installs the release's IDE package":** the jobs use the tools this run built, laid out as the package lays them out (`rcrun` finds `prc-runner` beside it, T20). A package is made by `build-all-platforms.yml`, a separate workflow whose artifacts this one cannot reach without a token. The layout is the same, and so is the code.
      - **"Every project that takes the WebAssembly path at this milestone"** is the module corpus's console fixtures: at M1 no reference project does, because all five have forms (M2).
      - **The check script of a fixture** is its `run.toml`: its input and pins, and the output and exit code it must give.
    - **Measured** (rehearsed on macOS arm64, Apple M3 Pro, debug build; `PATH`, `CARGO_HOME` and `RUSTUP_HOME` emptied):
      - **`fixtures`:** console built on the WebAssembly path in 5 215 ms and ledger in 2 003 ms; each ran with identical output and exit code (7, 3). 2 of 2.
      - **`reference`:** 0 on the WebAssembly path, 5 on the Cargo path, each naming its main form ("forms … are not on the WebAssembly path yet (DESKTOP-WASM-001 M2)"), 0 failed.
      - **`cross`** (this platform's modules and a copy standing in for a second platform): 4 of 4 identical, each fixture's module the same file on both.
      - **`module_hashes`, run twice with the same `SOURCE_DATE_EPOCH`:** 69 lines identical. They cover 19 modules (2 fixtures and 17 of the 62 `tests/cobol` programs; the other 45 are refused, each named), and the 5 reference projects with their reasons.
      - **Owed to the operator:** one dispatch of **Actions → "Desktop WebAssembly path (DESKTOP-WASM-001)"**, once the file is on the default branch (GitHub dispatches from there), or a `v*` tag. `gh` is not on this machine. The four platforms' result blocks then go into `WASM-PROGRESS.md`.

- [x] **T23 — AC9 at M1, and the parity list** (R11, R21, R22; AC9)
  - Files: new `crates/cobolt-runner/examples/nist_runner_run.rs` (the `nist_wasm_run` pattern, with the module run by `prc-runner`); `specs/WASM-PARITY.md`.
  - Do: measure on both axes (GOLDEN RULE #9):
    - the NIST modules the interpreter has finished, in the native mode;
    - `tests/cobol` against the interpreter's output.

    Each divergence found goes into `WASM-PARITY.md`, with who is right.
  - Verify: the compile and execution counts per module, reported separately, plus the corpus's identical-output count, are written to `WASM-PROGRESS.md`. Nothing gates on them (R21).
  - *Done 2026-11-16 (2.0.522):*
    - **New `crates/cobolt-runner/examples/nist_runner_run.rs`**, two modes:
      - **`run`**, what the NIST harness passes `rcrun`: the generator compiles the program in the native mode (copybooks expanded, free form unless `--source-format` says otherwise), the module is packaged alone into a copy of `prc-runner`, and that executable runs it in the working directory. A refusal exits 1 and is written to `NIST_RUNNER_LOG`, which is how the compile axis is counted. `--switch` is accepted and ignored (WASM-PARITY #55).
      - **`compare --rcrun <rcrun> [folder]`**: every program of `tests/cobol` (or the folder), run under `rcrun run` and under the runner, each in its own copy of the program's folder, with no input, the pins and 20 s each. Output and exit code must be equal; one result block.
    - **Measured** (macOS arm64, Apple M3 Pro, debug build):
      - **NC (Nucleus):** compile 4 of 95, execution 4 of 95 with 0 failures (NC110M, NC302M, NC303M, NC401M). 53 PASS, 0 FAIL, the same 53 the interpreter reports for them.
      - **SQ (Sequential I/O):** compile 3 of 85, execution 3 of 85 with 0 failures (SQ302M, SQ303M, SQ401M). 24 PASS, 0 FAIL, the interpreter's 24.
      - **Why so few:** every other CCVS program uses a file on disk, and files on disk come at M3 (T40 measures SQ again). Each refusal names the first such file: all 91 in NC name the report file `PRINT-FILE`; in SQ, 62 name `RAW-DATA` (the installation's data file) and 20 name `PRINT-FILE`.
      - **`tests/cobol`:** 62 programs, 18 compiled in the native mode, **16 of 18 identical**. The other 2 (`call/test-call-exception.cbl`, `call/test-call-returning.cbl`) are identical except for one line, `rcrun`'s own warning `CALL to unknown program '…'`, which `rcrun` writes to standard output (WASM-PARITY #54).
      - **The 44 refused, by reason:** 19 disk files; 11 `EXEC SQL`; 5 value-method extensions (#56); and one each of MERGE, `SET` pointer, `AUDIO-PLAY`, `IMAGE-INFO`, national literals, `DISPLAY-OF`, `ULENGTH` and a user-defined function (`FUNCTION-ID`). One more is `copy-replace/tcpyrep.cbl`: read as free form, its comment line `COPY and COPY REPLACING` is taken for a `COPY` statement, by the interpreter too, so it is refused on both.
    - **New parity rows:**
      - **#54:** `rcrun` writes its log to standard output. The runner is right; this is a 1.x.x fix, reported to the operator.
      - **#55:** external switches are OFF in a built program on both paths (the Cargo-built `main.rs` never sets them either). By design at M1.
      - **#56:** the value-method extensions the generator refuses and the interpreter runs.
    - **Settled while building:**
      - **The comparison reads free form,** as `rcrun run` and the Desktop Build do. Detecting the format would compare two different readings of a source.
      - **The NIST harness is unchanged:** its 20 s limit and its scoring apply to the runner as they do to `rcrun`.

- [x] **T24 — Console parity, byte for byte** (R27 console part)
  - Files: new `tests/pixel-parity/console/` (console fixtures using each M1 construct); `crates/cobolt-bench/src/pixel_parity.rs` (`--console`).
  - Verify: `cargo run -p cobolt-bench --release -- pixel-parity --console` builds each fixture on both paths, runs both under the pins, and prints "programs N, identical output N of N". Any difference is a failure.
  - *Done 2026-11-16 (2.0.523):*
    - **New `tests/pixel-parity/console/`**, three fixtures, each a folder with its sources and a `run.toml` (`sources`, then what the program is given: `stdin`, `args`, `env`):
      - **`statements`:** the arithmetic verbs with GIVING, ROUNDED, REMAINDER and SIZE ERROR; COMPUTE; edited pictures; JUSTIFIED; the CORRESPONDING forms; INITIALIZE with REPLACING; STRING with POINTER and OVERFLOW; UNSTRING with COUNT and TALLYING; INSPECT TALLYING, REPLACING and CONVERTING; class and sign conditions; 88-levels; NEXT SENTENCE; EVALUATE with ALSO and THRU; the PERFORM forms; SET, SEARCH and SEARCH ALL; intrinsic functions; GO TO DEPENDING; ALTER. 47 lines.
      - **`console`:** two lines of input and one past its end; a variable set and one not set; three arguments, one with a space and one not ASCII, walked by `DISPLAY n UPON ARGUMENT-NUMBER`; the command line; DATE, DAY, TIME, DAY-OF-WEEK and all 21 characters of CURRENT-DATE; RANDOM with and without a seed; RETURN-CODE 9 as the exit code. 16 lines.
      - **`calls`:** CALL BY REFERENCE and BY CONTENT into Common Code (one program per file, as the Build links Common Code); a counter that keeps its state until CANCEL; a contained program; sections; SORT with an INPUT and an OUTPUT PROCEDURE (RELEASE, RETURN); EXIT PROGRAM and GOBACK. 12 lines.
    - **`cobolt-bench pixel-parity --console`** (new `crates/cobolt-bench/src/console_parity.rs`; `pixel_parity.rs` takes the flag and hands over):
      - each fixture is copied twice with a project file and built with `force_cargo` and by default, sharing one build folder; the default Build must take the WebAssembly path, or the fixture fails with the Cargo reason, since both programs would then be the same;
      - both programs run in an empty folder under the form runs' pins (`PRC_TEST_CLOCK`, `PRC_TEST_SEED`) with the fixture's input, arguments and variables, 60 s each; standard output must be identical byte for byte and the exit codes equal. A difference keeps both outputs under `target/pixel-parity/console/<name>/` and names the first line.
    - **Measured** (macOS arm64, Apple M3 Pro, release): **programs 3, identical output 3 of 3**; 75 lines compared; Cargo builds 62.7, 2.7 and 2.6 s (the first compiles the shared crates), WebAssembly builds 0.1 s each; runs 212–339 ms on either path; 1.2 min in all.
    - **Closed on the way:**
      - **#53:** the module asks the runner for the zone's offset after each CURRENT-DATE (`prn.tz_offset`, ABI A31, a capability of `env`) and writes it over the `+0000`. `cobolt-runner` `tests/console.rs` now compares all 21 characters with `rcrun run` in two zones (`-0300`, `+0530`).
      - **#15, on the desktop:** a desktop module refuses an INDEXED file declared `STORAGE IS MEMORY`, as every other file, because the desktop reads it from its disk file (status 37 on OUTPUT, the operator's rule of 2026-10-01); such a program takes the Cargo path until M3. The browser module is unchanged (`desktop_wasm_unchanged` passes). The module corpus's `ledger` fixture, which used one, keeps its entries in a table instead.
      - **The Desktop column of TRY and THROW:** it said both are compiled into the module; no generator compiles them. Both now take the Cargo path with their reason, M5 (#58).
    - **New parity rows:** #57 (a numeric item moved into a reference-modified item: the interpreter moves the item's displayed value, the runner its digits as COBOL-85 says; a 1.x.x fix, reported) and #58.
    - **Settled while building:**
      - **The fixtures use only what the WebAssembly path builds at M1:** a construct it refuses would send the default Build to the Cargo path and compare a program with itself. The harness checks the path, not the fixture's author.
      - **A Common Code file holds one program:** the Build links each file's first program, so a second one in the same file is not found (`CALL "…": no program … has that name`).
      - **Left out, each recorded:** WHEN-COMPILED (#5, the Build's input), the reference-modified MOVE of a numeric item (#57), TRY and THROW (#58), and the COBOL 2002 forms `ACCEPT … FROM DATE YYYYMMDD` and `DAY YYYYDDD`.
      - **Seen, the same on both engines:** `ACCEPT … FROM ARGUMENT-VALUE` answers the argument the pointer names and does not move it, so three ACCEPTs in a row give the first argument three times. The fixture sets the pointer each time. Moving to the next argument, as X/Open describes, would be an interpreter change, reported.

- [x] **T25 — The Developer's Guide and the System KB for M1** (GOLDEN RULE #3, #8; plan §7)
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
  - *Done 2026-11-16 (2.0.524), the System KB half; the Guide half waits for the merge into `main`:*
    - **Why the Guide waits:** this work lives on the 2.x.x line, where `README.md` and `docs/` are never touched (CLAUDE.md, "Version 2.x.x"), and the operator ruled on 2026-10-30 that a 2.x-only feature's Guide text is written when 2.x.x merges into `main`, afresh, not kept on a branch. So `docs/developers-guide-en.md` and its five translations are unchanged here. **Owed at the merge into `main`:** §18 (the default path, the machine code and its setting, choosing Cargo, what still takes Cargo and why, a mermaid diagram of the two paths, a Caveat on `WASM-PARITY.md`'s known divergences, no Rust toolchain), §17 (`rcrun build --cargo`, `--no-machine-code`), then the five translations deleted (#8).
    - **The System KB** (`cobolt-compiler` `system_documentation`, `project_model_and_settings.md`):
      - a new section, **`[desktop]` — how a Desktop Build makes the program:** the two paths and how the Build names them; when it takes Cargo (chosen by `build_path = "cargo"` or `rcrun build --cargo`, Debug, External Crates, `EXEC RUST`, a construct the path does not build yet with its file and line, no runner); `machine_code` and `rcrun build --no-machine-code`; settings written only when they differ from the default; what happens with no Rust toolchain (the stop, `bin/` untouched, the IDE's offer, `rcrun`'s command);
      - "What runs in a browser module" no longer says that every Desktop Build takes the Cargo path: its *On the Desktop* column is the default path's.
    - **Regenerated:** 4 397 records from 8 documents, 15 552 512 → 15 581 184 bytes. `prebuilt_chunked_kb_matches_the_published_documentation` green; the compiler's 14 documentation tests green.

- [x] **T26 — M1 close**
  - *Done 2026-11-16 (2.0.525), on macOS arm64 (Apple M3 Pro, 11 cores, 18 GB; not a reference box):*
    - **Tests:** the six crates passed 460 tests, with 0 failed and 9 ignored, in 66 test binaries.
    - **`cobolt-ide --bin`:** 2 030 passed, 1 failed, 6 ignored. The red is the 2.x line's known `docs_embed::every_document_ships_in_every_language`. The System KB check is green, so T25's KB is current.
    - **T1's test:** `desktop_wasm_unchanged` 3 of 3 artefacts match their pins.
    - **`build-goal --runs 1`:** 6 of 6 Builds took the Cargo path, and each names its reason: its first form, "not on the WebAssembly path yet (DESKTOP-WASM-001 M2)".
      - The slowest Builds: PowerCallOfDuty 178.6 s, PowerCloud 144.6 s, PowerCobolCraft 177.6 s, PowerGraphics 173.5 s, PowerSpatial 139.3 s, TenTimes 185.4 s.
      - Peak memory: 4 783–5 549 MB.
      - All six are over the 30 s goal, which M2 is the first to judge.
      - The generator alone accepts PowerCallOfDuty (5.8 ms) and PowerGraphics (39.8 ms).
      - `build-goal` now prints the reason for every Cargo Build and records it as `cargo_reason`. One run before that change measured the same paths, 141.3–185.5 s.
    - **`pixel-parity --cargo`:** 408 of 408 golden frames match, and 22 of 22 forms are repeatable (14.8 min). No project moves at M1, so AC19 has no WebAssembly-built form to compare yet.
    - **AC9 (NIST through the runner):**
      - NC (Nucleus): compile 4 of 95, execution 4 of 95 with 0 failures.
      - SQ (Sequential I/O): compile 3 of 85, execution 3 of 85 with 0 failures.
      - The other 91 and 82 programs are refused for a file on disk (M3), as at T23.
      - `tests/cobol`: 16 of 18 identical, with the 2 others differing only by `rcrun`'s warning line (#54).
    - **The runner's size (C12):** the release `prc-runner` is 10 336 576 bytes (9.9 MiB) for macOS arm64 and needs only the system libraries (libSystem, CoreFoundation, libiconv). It links neither the interpreter (`cobolt-runtime`) nor the generator, so C12's expected "links `cobolt-runtime`, unused" did not happen. Measured on the tree carrying T27 and T28, whose `cobolt-forms` (without painting) is in the runner's tree but unused until T30; the build before them was 10 336 000 bytes.
    - **Owed at the merge into `main`:** the Developer's Guide's list of what takes the Cargo path (with T25's §17 and §18).
  - Do: the milestone close (preamble).
  - Verify:
    - `cargo test -p cobolt-wasm -p cobolt-wasm-abi -p cobolt-runner-engine -p cobolt-program-package -p cobolt-runner -p cobolt-compiler --no-fail-fast`, every `test result:` line read.
    - `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`: only the known reds of the 2.x line.
    - T1's test green.
    - `build-goal` names each reference project's fallback ("forms").
    - The runner's size goes into `WASM-PROGRESS.md` (C12).

## Phase M2 — Forms

- [x] **T27 — `cobolt-abi-host`, extracted from the player unchanged** (R9; plan §4.3)
  - *Done 2026-11-16 (2.0.526):*
    - **The move:** 35 files moved with their history (`git mv`) into the new `crates/cobolt-abi-host`, a member of the main workspace. With its new `lib.rs` and `menus.rs`, the crate has 37 `.rs` files and 19 021 lines.
      - It holds `host` (with its tests and `streams`), `tree`, `methods`, the menus the bundle delivers (`MenuStore`, out of the player's `view`), and their closure: `auth`, `hyperlink`, `kinds`, `net`, `grid`, `par`, `value`, `upload`, `css`, `base_css`, `layout_css`, `zoom`, `motion`, `metrics`.
      - The player depends on it, re-exports each module under its old path, and lists the crate in `fingerprint::INPUTS`.
      - The test helpers (`tree::testing`, `host::testing`) are behind the new crate's `testing` feature, which only the player's tests turn on.
    - **Code changes:** only paths and visibility. Two source-reading tests (`net`'s storage test, the player's `wire_codes` and `sinks`) now also read the new crate's `src/`. The session token's `token()` is now public, because the player's transport (`api.rs`, wasm32 only) is in another crate; the native tests could not see that, the wasm32 build did.
    - **Test counts:** the player's source had 474 `#[test]` functions before the move. After it, the player `--lib` passes 213 and `cobolt-abi-host` 261, so 474 of 474 pass.
    - **The player's other tests:** `abi_host` 28, `exposed` 2, `freshness` 6 (after the rebuild), `grid_calls` 1, `no_secret_in_console` 3, `sinks` 3, `wire_codes` 1. `dependency_guard` is 6 of 7: its Rust 1.92 check, red since 2.0.469 (the workspace needs 1.95), is to be reported as a 2.x fix.
    - **The wasm32 build:** it succeeds, and the shipped player is rebuilt. Plain is now 4 159 011 bytes (+28 561, +0.7%; gzip 1 441 557) and threaded 3 970 623 (+25 719). The growth is most likely generic code instantiated in both crates: the profile is already `lto = true` with one codegen unit.
    - **T1's test:** 3 of 3 artefacts match.
  - Files:
    - new `crates/cobolt-abi-host/`: the player's pure `Host`, `FormTree` (`tree.rs`), `methods.rs` and their closure, moved with their tests;
    - `crates/cobolt-web-player/Cargo.toml`: a path dependency, plus a re-export;
    - `crates/cobolt-web-player/src/fingerprint.rs`: `INPUTS` (:33) gains the new crate.
  - Verify:
    - The player `--lib` gives the same pass count as before the move, and is green.
    - `cargo test -p cobolt-abi-host` green.
    - The player's `wasm32` build succeeds. The shipped player is rebuilt if the freshness test asks.
    - T1's test green.

- [x] **T28 — The forms, menus, settings and art inside the module** (R1; C1; AC15, AC16)
  - *Done 2026-11-16 (2.0.527):*
    - **`cobolt-program-package::sections`:** four custom sections, each with its own format number (1).
      - `prsettings`, always: JSON, the `RunnerSettings`.
      - `prforms`: JSON, each form's code-free `FormDescription`, the main form first.
      - `prmenus`: JSON, each menu's key and its YAML.
      - `prart`: binary little-endian, each theme pack's id, manifest and files.
      - A section that is missing reads as empty, or as the defaults. A reader takes every format up to its own and refuses a newer one by name: "run the program with a later PowerRustCOBOL runner".
      - Recorded as ABI A32.
    - **`cobolt_wasm::module::append_custom_sections`:** the Desktop Build appends the sections after the generator has finished. A Web module's bytes therefore cannot change: T1's test still matches 3 of 3.
    - **`cobolt-compiler`:**
      - `gather_forms` is step 5 of `build_core`, taken out so the two paths share it.
      - `form_spec` is the FormSpec builder the Web generator now uses too. Its browser-only lowering stays its own.
      - `runner_settings` is the one function that makes the settings. The Cargo path bakes them into `main.rs` through `main_rs_from_settings`, and the WebAssembly path writes them into `prsettings`.
      - `desktop_sections` describes the forms, the menus and the theme art. `find_theme_packs` is now shared with `stage_theme_packs`.
    - **`desktop_sections_tests`:**
      - The settings test: for each of the five reference projects, with remote calls off and on, the settings read back from the generated `main.rs` equal those decoded from a module's `prsettings`. 10 of 10 (170 fields).
      - The forms test (AC15 for a form project): 22 forms are described (509 486 bytes of `prforms`), every section round-trips (5 of 5), and none of the projects' 993 handler statement lines is in them, raw or JSON-escaped.
      - The reference projects have no menus and no theme packs, so `prmenus` and `prart` are covered by the package's own round-trip tests (4 of 4).
    - **No test named `no_project_source_in_built_file` exists.** AC15's check of the built file is in `desktop_wasm_build`, and a form project's built file waits for M2: until then its forms take the Cargo path.
    - **Other suites:** `cobolt-wasm` 121 passed, 0 failed; `cobolt-compiler` 302 passed, 0 failed, 9 ignored (19 test binaries); `cobolt-program-package` 10 passed; `cobolt-project-tools` 267 passed, 0 failed.
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

- [x] **T29 — `ProgramEngine` in the form host; today's path becomes `InterpreterEngine`** (R9; C5)
  - *Done 2026-11-16 (2.0.528):*
    - **`cobolt-form-host::engine` (new):**
      - `ProgramRun` carries everything a form instance's program is given: its handle, its form's object, the control ids, the event, state, display and input channels, the closed signal, the request channel, the pending and finished flags, the caller for `super::`, the seeded objects, the designed menus and the debug wiring.
      - `ProgramStart` is the one-shot start of a program with that run.
      - `trait ProgramEngine` has `form(id)` (the design alone) and `open(id)` (the design and a start).
      - `InterpreterEngine { setup, indexed_engine, bridge }` holds the interpreter-thread code that was in `build_form_instance_as`, moved unchanged.
    - **`host.rs`:**
      - `HostHooks::program_engine()` gives the host its engine once, when it is built. The default is `None`, the interpreter, with the programs `form_source` names, so no existing host changes.
      - `form_design` and `open_program` ask the engine when there is one, else the form source. `form_side_menu_id` and `form_has_side_menu` read the design through `form_design`.
      - `build_form_instance_as` registers the debug router, then calls the start with the instance's own channels.
    - **New test `a_child_form_runs_on_the_program_engine_the_glue_gave`:** a host with no form source, given a fake engine, answers a form's design (3 asks) and opens a child window. The start receives the instance's display channel, its handle `W1`, its object `DETAIL` and its caller `W0`. The engine's "finished" releases the window, and the close reaches the fan-out.
    - **Verify:**
      - `cargo test -p cobolt-form-host`: 219 passed, 0 failed (218 before, plus the new test).
      - The `rcrun` host tests (`aws_hosts`, `layers_hosts_091`, `model_list_hosts`, `partiql_hosts`, `perspective_hosts_096`, `powercloud_hosts`, `side_menu_hosted_controls`, `side_menu_rows_hosts`): 21 passed, 0 failed, 6 ignored.
      - `engine_reference_form_parity_static_vs_faces`: 1 passed.
      - `pixel-parity --cargo`: 408 frames compared, 408 golden matched, 0 differ, 22 of 22 forms repeatable, 13.8 min. The refactor moved no pixel.
  - Files: `crates/cobolt-form-host/src/host.rs`: `build_form_instance_as` (:5314), and the interpreter thread (:5379–5447) behind the trait.
  - Verify:
    - `cargo test -p cobolt-form-host` green.
    - Every `rcrun` host test green.
    - `pixel-parity --cargo` matches every golden: the refactor moved no pixel.
    - `engine_reference_form_parity_static_vs_faces` green.

- [x] **T30 — `WasmEngine`: one form** (R9, R10, R28 runner half)
  - *Done 2026-11-16 (2.0.529):*
    - **`cobolt-runner::forms` (new):**
      - `WasmEngine` is the runner's `ProgramEngine`. It answers a form's design from the module's `prforms` section, and starts an instance by handing its channels to the one program thread.
      - The program thread owns the module's instance and the ABI host (`HostOptions { observer_events: false }`). It performs the host's calls as the browser's loader does (`pr_open_form`, `pr_event`, `pr_resume`, `pr_close_form`, `pr_cancel`).
      - **Events in:** before each event, the user's edits are stored without an echo (`ui_set`). An event leaves `pending` when it is taken, as the interpreter takes it. An input wake only drains the input. A closed window ends its form's program.
      - **Changes out:** each property write is sent as a `StateUpdate` spelled as the program spelled it.
      - **`DISPLAY` and run-time errors** go to the window's display.
      - **Timers:** `Sleep` timers run on the form host's clock. A Timer control is run by the form host, as for the interpreter.
      - `now` and `tz_offset` answer the interpreter's clock (`test_pins::now()`), which follows the headless run's virtual time.
    - **`cobolt-runner::app` (new):**
      - The glue mirrors `run_form_app`: the assets anchor, the connections, locale, currency, link schemes, focus ring and menus from `prsettings` and `prmenus`, the theme packs from `prart`, the window effects, the shell decision, and the main form's instance.
      - `HostHooks::program_engine` gives the host the `WasmEngine`.
      - `PRC_HEADLESS_SCRIPT`, `PRC_HEADLESS_LIMIT` and `PRC_HEADLESS_SHOTS` run the script as a Cargo-built program does.
      - `run_module` takes this path when the module has forms. The runner now provides `pr.query`.
    - **`cobolt-abi-host`:**
      - `HostOptions::observer_events` (on by default; the player sets it on). The form host raises a write's observer events itself, so a handler runs once.
      - `FormTree::spelled` records how the program spelled each property it last wrote. The form host keys a write by that spelling: `CAPTION` from COBOL, `Items` from `AddItem`, `Interval` from `SetInterval`. A toggle is sent under its control's one name (`Checked` or `Selected`), as the interpreter sends it.
      - The player is rebuilt: plain 4 163 331 bytes, threaded 3 974 495.
    - **The Build (`cobolt-compiler`):**
      - A form project is compiled as one module: the main form's program, then each form whose program the Build has (`cobolt_wasm::compile_project`, now re-exported).
      - **The temporary switch `PRC_WASM_FORMS=1`** lets a form project take that path. Every other form project stays on the Cargo path, as at M1, until T33 replaces the switch with the controls check.
      - The generator's member and statement gates let a module with forms reach its controls and the application object. A built-in of a later milestone is still refused.
    - **`rcrun run-form --headless`** honours `PRC_HEADLESS_SHOTS`, so both engines take the same pictures.
    - **Deviation:** the test is `crates/cobolt-cli/tests/runner_one_form.rs`, not a `cobolt-runner` test. It needs `rcrun` for both the interpreter's run and the Build, and only `cobolt-cli`'s tests have it built.
    - **`interpreter_and_wasm_engine_agree_on_one_form`:**
      - **The fixture:** one form with a Label ×4, a TextBox, a CheckBox, a ListBox, a Timer and four Buttons. The handlers do `MOVE` from a property, an `IF` on `Checked`, `AddItem`, `GetCount`, and a Timer started by a click whose `onTick` stops it.
      - **The run:** headless under `PRC_TEST_CLOCK` and `PRC_TEST_SEED`, under `rcrun run-form` and in the program `rcrun build` made on the WebAssembly path.
      - **Result:** 15 of 15 checks. All 5 dumps are equal, and all 5 pictures are equal in every pixel (640×420 each, 1 344 000 pixels compared, 0 differing). The program's results are seen in the runner (Ann; checked; 2; ticked).
      - **Timings:** Build 6.5 s; interpreter run 7.1 s; runner run 5.5 s.
    - **`desktop_wasm_build` (AC15):** the bare runner now holds the text `STOP RUN.` itself (the runtime and form crates it links). The check now names a line the bare runner already holds and leaves it out: 7 lines searched, 1 in the runner itself, 0 of the other 6 in the built file; 28 of 28.
    - **Size:** the debug runner is 172.1 MB, against 46.2 MB with the console only (T16). The release size is measured at M2's close (T36), with C12.
    - **Tests:**
      - `cargo test --no-fail-fast` over the eight crates T30 touches, with every `test result:` line read: 950 passed, 1 failed (`desktop_wasm_build`, corrected above, then 28 of 28), 9 ignored.
      - Per crate: `cobolt-abi-host` 261; `cobolt-compiler` 302 with the correction, 9 ignored, 18 binaries; `cobolt-form-host` 219; `cobolt-wasm` 121; `cobolt-wasm-abi` 26; `cobolt-program-package` 10; `cobolt-runner` 7; `cobolt-runner-engine` 5.
      - Then the player's `freshness` (6), the T1 gate `desktop_wasm_unchanged` (1) and the whole `cobolt-cli` suite (its 27 binaries, the `rcrun` host tests and `runner_one_form` among them): 76 passed, 0 failed, 6 ignored.
  - Files: `crates/cobolt-runner/src/` (`WasmEngine`, one program thread, the ABI host from T27).
  - Do:
    - Run one form: its controls, properties, methods, events and timers.
    - The runner honours `PRC_HEADLESS_SCRIPT`, `PRC_HEADLESS_LIMIT` and `PRC_HEADLESS_SHOTS`, as `main.rs` does (:4873–4874).
  - Verify:
    - A new `cobolt-runner` test, `interpreter_and_wasm_engine_agree_on_one_form` (`-- --nocapture`), runs a fixture form headless under both engines and compares their dumps after every step (AC14 part).
    - The fixture's `shot` frames are equal in every pixel.

- [x] **T31 — `WasmEngine`: several forms, the shell and the SideMenu** (R9, R10)
  - *Done 2026-11-16 (2.0.530):*
    - **`cobolt-runner::forms`:**
      - Each form instance is a link with its window handle, its supervisor's request channel and its own event queue. An event is taken only when the ABI host has no task waiting on that form (`Host::busy`): the interpreter takes no event inside a handler, a modal wait, a sleep or a dialog. `QUIT` bypasses the queue.
      - A program's `OpenFormSync` or `OpenFormAsync` asks the form host's supervisor for the form's window (`FormRequest::OpenForm`, with the window state, position, size and modality the program gave), and the module opens the form only when the window comes (`Host::open_held`), so its program starts in its window, as the interpreter's does. Opening it in the module first, as a first draft did, let a form that closes itself in its `onLoad` close before its window ever drew; the window left on top then differed, and the several-forms twin caught it (11 778 pixels of a title bar).
      - A `Closed` effect sends `CloseSelf` for a form the program closed (`me::Close`).
      - **The main form is one window** (spec 037 R10): a program that opens it again while it runs starts no second instance in the module, and a Sync open does not wait. The runner asks the supervisor, which focuses the running window, as it does for the interpreter. Without this, a Sync open of the main form would have waited for a window that never comes.
      - The window methods (`SetWindowState`, `Focus`, `SetFocus`, `SetTitleVisible`, `SetBreadcrumbDetail`, `ClearBreadcrumbDetail`) go to the supervisor as `HandleMethod`, as the interpreter sends them.
      - A program's write to another form's object (`super::SetProperty`) goes to that form's window as `HandleMethod SETPROPERTY`, never echoed into the writer's window.
      - A layer write is sent to the window as a `StateUpdate` on the layer's name.
      - The designed SideMenus of each form are handed to the ABI host, as the form host hands them to the interpreter (spec 066).
    - **`cobolt-abi-host`:** `HostOptions::desktop` replaces T30's `observer_events`. On the desktop the ABI host raises no observer events and no `onShow`/`onActivate` (the form host raises them after its warm-up), records layer writes, keeps `OpenForm`'s trailing arguments as a `Placement`, writes `ActivateItem` as the interpreter's `ActivateItemRequest` token, collects cross-form writes, and answers an `OpenForm` of the running main form with its handle. New: `Host::busy`, `take_window_asks`, `open_held` and `drop_held` (`WindowAsk`), `take_form_writes`, `take_refocused`, `FormTree::forget_change`; `nav_tests_on_the_desktop_a_form_a_program_opens_waits_for_its_window` proves the hold.
    - **The SideMenu's row methods are shared (C5):** the interpreter's dispatch for `AddItem`, `AddSection`, `RemoveItem`, `SetItemLabel`, `SetItemIcon`, `SetItemBadge`, `SetItemEnabled`, `SetItemControl`, `SetItemAction`, `Clear`, `GetCount` and `HasItem` moved unchanged into `cobolt_forms::menu::runtime::row_method`, which the interpreter and the ABI host now both call. Before, the ABI host took `AddItem` for a list's and answered `GetCount` from the items. The player's rows now match the interpreter's (`side_menu_rows_are_the_interpreters_on_every_surface`, 2 of 2 surfaces); `side_menu_rows_hosts` proves the interpreter unchanged. The player is rebuilt.
    - **Deviation:** the twins are one test file, `crates/cobolt-cli/tests/runner_shell_hosts.rs`, for the reason T30 gives. A fifth twin covers several forms, since none of the four named tests opens a second form.
    - **The five twins** (headless, `PRC_TEST_CLOCK` and `PRC_TEST_SEED`; `rcrun run-form` against the program `rcrun build` made on the WebAssembly path; dumps and pictures after every step):
      - `the_rail_and_its_controls_look_the_same_open_and_folded` (twin of `side_menu_hosted_controls`): 8 of 8, 1 036 800 pixels.
      - `a_program_makes_the_same_side_menu_rows` (twin of `side_menu_rows_hosts`): 9 of 9, 1 555 200 pixels; the runner's program counts 4, 4, 3.
      - `a_layer_answers_the_same_in_the_shell` (twin of `layers_hosts_091`): 11 of 11, 2 160 000 pixels; the runner reads what the `rcrun` test wants.
      - `a_tilted_chart_answers_the_same_in_the_shell` (twin of `perspective_hosts_096`): 9 of 9, 1 620 000 pixels.
      - `several_forms_open_wait_and_reach_their_opener`: a form opened beside writes to its opener with `super::SetProperty`; a modal form answers and closes itself, and its caller goes on and reads the answer; the main form opened again with `OpenFormSync` neither starts again nor waits. 15 of 15, 768 000 pixels; each `onLoad` (the main form's too) ran once under both engines.
      - **Result:** 5 passed, 52 of 52 checks, 15 pictures, 7 140 000 pixels compared, 0 differing; 31.5 s.
    - **Known differences, left for later tasks:** the window handle a program reads is `H{n}` in the ABI host and `W{n}` in the interpreter; a non-window `Change::Command` is only traced; the dialogs are T32; `PRC_WASM_FORMS` stays until T33.
    - **Tests:** `cobolt-abi-host` 263 passed (with `nav_tests_on_the_desktop_a_form_a_program_opens_waits_for_its_window`); `cobolt-runner` 7 passed; `runner_shell_hosts` 5 of 5; the interpreter's host tests (`side_menu_rows_hosts`, `side_menu_hosted_controls`, `side_menu_own_toggle`, `layers_hosts_091`, `perspective_hosts_096`) and T30's `runner_one_form` 7 passed, 3 ignored; `cobolt-runtime` 566 passed, 3 ignored; `cobolt-form-host` 219 passed; `cobolt-forms --features render` 1 877 passed and 1 failed that predates T31: `every_example_form_renders_as_its_golden` differs on the three PowerDemo3 forms that hold a PieChart or DonutChart, whose goldens were not re-recorded when 2.0.413 centred the pie (the forms crate's only change here is the new `row_method`). The player is rebuilt, `freshness` 6 of 6.
  - Do: `OpenFormSync`, `OpenFormAsync`, `super::`, the application shell with its SideMenu, ContentPane and breadcrumb.
  - Verify: runner twins of the `rcrun` host tests `side_menu_hosted_controls`, `side_menu_rows_hosts`, `layers_hosts_091` and `perspective_hosts_096`, equal in their dumps and frames.

- [x] **T32 — `WasmEngine`: dialogs, file dialogs and the scripted answers** (R10)
  - *Done 2026-11-16 (2.0.531):*
    - **Correction to T6, so to this task's "message boxes":** a desktop form has no message box. Neither the interpreter nor the form host dispatches `COBOL::"MESSAGEBOX"`, `"ALERT"`, `"CONFIRM"`, `"INPUTBOX"` or any other verb of a web page's application object (checked name by name); they exist only in the browser (`lower_remote::APP_VERBS`: "not built-ins of the desktop runtime"). On the desktop, opening a form is the `me::OpenFormSync`/`me::OpenFormAsync` method (T31), not a `COBOL::` verb. So there is no message box for the runner to show, and the `answer` step drives the file dialogs only.
    - **The page's verbs are refused in a desktop module:** before, `cobolt-wasm` lowered them in a native module as on the web, and the runner would have met an effect it does not serve. Now a native form module refuses each of the 17 (every `APP_VERBS` entry except the 3 mobile ones), naming the verb and its line, so the Build takes the Cargo path, as for any construct the module does not carry.
    - **The three file dialogs** (`OPEN-FILE-DIALOG`, `SAVE-FILE-DIALOG`, `FOLDER-DIALOG`) are desktop-only app verbs (ABI A33) that suspend the task as `remote` does. The ABI host keeps a `FileDialogAsk` (title, filters, folder, file name) and waits; the runner sends it to the form host's supervisor as `FormRequest::FileDialog`, the same request the interpreter sends, and gives the answer back (`ToProgram::Answered`), so the last data item receives the path chosen, or spaces when cancelled. While the dialog is open that form takes no event (`Host::busy`). The module's manifest lists the capability `dialogs`; the runner provides it.
    - **Shared with the interpreter (C5):** the ask's shape and its parsing (`FileDialogAsk`, `file_dialog_kind`, `file_dialog_ask`, `dialog_filters`) moved unchanged into `cobolt-runtime::form_host`, which the interpreter and the runner both call.
    - **A dialog outside a form's handler** (in a console program) is refused, as before: there is no window to own it.
    - **Headless:** the `answer` step answers a dialog under either engine, and the headless report now lists what each dialog asked (`dialogs_asked`: kind, title, filters, folder, file name).
    - **The KB's Desktop column** now reads "yes, with the runner's `dialogs`" for the three dialogs, and the System KB is regenerated.
    - **The twin** `file_dialogs_are_asked_and_answered_alike` in `crates/cobolt-cli/tests/runner_shell_hosts.rs` (the file of T30 and T31): an open dialog with a title, a filter and a start folder; a save dialog with a file name; a folder dialog; and an open dialog the operator cancels. Each is answered by the script (`answer`: a file, a save path, a folder, a cancel). Under the pins, both engines asked the form host the same four dialogs (kind, title, filters, folder, file name), the program received the same four answers (for the cancel, spaces, which the handler shows as "cancelled"), the dumps are equal, the picture is equal in every pixel (256 000 pixels), and the module's manifest lists `dialogs`: 8 of 8 checks; the Build took 9.3 s.
    - **Tests:** `runner_shell_hosts` 6 of 6 (60 of 60 checks, 16 pictures, 7 396 000 pixels, 0 differing) and T30's `runner_one_form` 1 of 1 (15 of 15, 1 344 000 pixels, 0 differing); `cobolt-wasm` 121 passed in 34 targets, with `native_desktop_refusals` 14 of 14 checks over 30 programs (the 17 page verbs refused, a dialog in a handler compiled, outside one refused); `cobolt-abi-host` 263 passed; `cobolt-runtime` 1 674 passed, 29 ignored, in 210 targets (`test_file_dialog_calls` among them); `cobolt-form-host` 219 passed; `cobolt-runner` 7 passed; the player rebuilt, `freshness` 6 of 6; the System KB regenerated (4 397 records), `prebuilt_chunked_kb_matches_the_published_documentation` green. The runtime's 209 test binaries (about 200 MB each) were run 30 at a time, each batch deleted after it ran, as the disk could not hold them all.
  - Do: message boxes and file dialogs run under the runner, and the `answer` step of T6 drives them headless.
  - Verify: a runner test where a fixture opens each dialog kind, answered by script, gives equal dumps and frames under both engines.

- [x] **T33 — Desktop controls the ABI host lacks fall back by name** (R13; plan §4.3)
  - *Done 2026-11-16 (2.0.532):*
    - **Forms take the WebAssembly path by default.** The switch `PRC_WASM_FORMS` (T29–T32's opt-in) is gone: `classify` no longer refuses a project for having forms. In its place, plan §4.7's "desktop control" step, after `EXEC RUST` and before the runner lookup.
    - **`CargoReason::DesktopControl { form, file, control, control_type, reason }`** — the plan named `{form, control}`; the file (as the project lists it) and the control's type are added so the IDE, `rcrun` and the agent can point at the `.cfrm` and say what the control is. Display: "forms/main.cfrm: the form MAIN holds the control MAP-CITY (Maps): …".
    - **Which controls the runner serves (decision):** the web player's classes A and B (HTML and drawn, `cobolt_forms::web_class`), whose semantics the ABI host carries, plus the DataGrid with in-memory `Rows` (web class A for that use). Everything else falls back with a reason naming its milestone:
      - the 24 service controls of class C (AgentObject, RestClient, SqlDatabase, IndexedFile, KnowledgeBase, FileDropZone, WebSearch, Authentication and the 16 AWS controls): "M3, M4 and M6";
      - the 7 canvases and graphics objects (Canvas2D, Canvas3D, ImageBuffer, Plot, Scene3D, Physics2D, Physics3D): M6;
      - Maps and the Viewer: M6;
      - a type the path does not know (class D, a plug-in): M6.
      - So 38 of the 71 control types are served and 33 fall back. AC19 (T34) is the judge of the served set: a served control that draws or behaves differently is caught there and moves back to the list.
    - **Data bindings fall back too (decision, not in the task text):** a binding is filled by the interpreter's runtime and the form host's seeding, which a module does not carry; the ABI host fills only the web's remote grids. So any control bound to data takes the Cargo path, whatever its type, with the source's milestone: an indexed file M3; a SQL query and a REST endpoint M4; a COBOL table, an agent's output and a PartiQL query M6 (`DESKTOP_BINDING_GAPS`). No task names the bindings; they move with T38 (files), T41/T42 (SQL, HTTP) and T47/T48 (agents, PartiQL).
    - **The ToolBar needs no exception:** its four web exceptions (`run-app`, `open-terminal`, `print`, `share`) are performed by the form host itself (`toolbar_runner`), under either engine.
    - **The native column (T10):** a new class `Native::Form` for the 13 entries a form's program is served (the built-ins CHART-ADD-POINT, CHART-CLEAR, CHART-REFRESH, CHART-SET-TABLE, GET-PROPERTY, INIT-FORM, RAISE-EVENT, SET-PROPERTY, WAIT-EVENT; the statements INVOKE, control assignment, member call, window operation). Before, they were `Cargo(D_FORMS)`. A program with no form still sends them to the Cargo path (`desktop_refusal(stmt, in_form)`), with D_FORMS reworded. The KB's column reads "yes, in a form's program".
    - **Search order:** the forms in the project's order, each form's controls in document order with containers searched depth-first, then that form's first binding. The first finding is the reason.
    - **The KB:** `[desktop]` now says the path builds form programs, and adds the bullet for an unserved or bound control. A new subsection "Controls on the Desktop" is written from `desktop_control_gap` and `DESKTOP_BINDING_GAPS`, so it lists exactly what the Build refuses. The System KB is regenerated: 4 410 records.
    - **Where the reason shows:**
      - the IDE's Build report (`build_reason_desktop_control`, six languages);
      - `rcrun build` and the toolchain error (Display);
      - the agent's `build` tool: kind `desktop_control` with `form`, `file`, `control`, `control_type` and `why`.
    - **Tests and benches that relied on forms falling back:** `build_reports_phases_and_peak` now asks for the Cargo path (`force_cargo`, reason `Chosen`), and `cobolt-bench pixel-parity`'s Cargo recording sets `force_cargo`. The two runner twins drop the switch, so they now build forms on the WebAssembly path by default.
    - **Tests:**
      - `desktop_controls_the_runner_lacks_are_named` (compiler unit): 5 of 5 — served controls pass; Maps nested in a Panel is named with its form, file and type; a Canvas2D on the second form names that form; a Label bound to an indexed file names M3.
      - `desktop_wasm_build`: 32 of 32 checks with no Rust toolchain. Section 7 is new: a form of a Button and a Label built on the WebAssembly path in 7 267.6 ms; a form with Maps in a Panel stopped on the Cargo path in 10.9 ms, naming MAIN, `forms/main.cfrm`, MAP-CITY and Maps, with nothing compiled.
      - `subset_table`: 9 of 9. The new `every_control_has_a_desktop_class` gives 71 types = 38 served + 33 on the Cargo path, 8 of 8 fixed points, 6 binding sources each with its row, and 0 reasons without a milestone. AC10: 31 in the module, 4 through the runner, 13 in a form's program and 88 on the Cargo path, 7 of 7 fixed points, 0 faults.
      - The KB subset test: 33 controls listed with their reason, 38 served and not listed, 6 binding sources.
      - `cobolt-wasm`: 122 passed in 34 targets. `native_desktop_refusals`: 14 of 14; its three checks of a program with no form (`ME::Title` read and written, `INVOKE`) now expect the reworded reason.
      - The runner twins, now built without the switch: `runner_one_form` 15 of 15 (1 344 000 pixels, 0 differing; `rcrun build` took the WebAssembly path by default in 6 780 ms); `runner_shell_hosts` 6 of 6 (60 of 60 checks, 16 pictures, 7 396 000 pixels, 0 differing).
      - Project tools `build`: 11 passed. IDE `build_path_report`: 3 passed. IDE `i18n`: 22 passed. `prebuilt_chunked_kb`: green.
      - `cargo test -p cobolt-compiler`: 303 passed, 0 failed, 9 ignored, in 19 targets.
    - **No reference project moves yet** (T34 measures it). PowerSpatial holds only served controls, but its Common Code opens an ORGANIZATION IS INDEXED disk file (M3). The other four hold a Canvas, a Scene3D, an ImageBuffer, an Aws control or a FileDropZone.
  - Files: `crates/cobolt-compiler/src/lib.rs` (`classify`: `DesktopControl { form, control }`), and the native column (T10) updated for what M2 covers.
  - Verify: a compiler test with a form holding one control the ABI host lacks. It builds on the Cargo path, and the message names the form and the control. The AC10 test stays green.

- [x] **T34 — Pixel parity for forms** (R27; AC19)
  - *Done 2026-11-16 (2.0.533):*
    - **The pairs are the mode with no flag** (`cobolt-bench pixel-parity`). `--cargo` (T7's golden check), `--console` (T24) and `--write-scripts` keep their meaning.
    - **How a project's path is learnt (decision):** a new `BuildOptions::wasm_only`. A Build that would take the Cargo path ends with `CompilerError::WasmOnly { reason }` right after classification, before the toolchain probe or anything compiled for Cargo, and `bin/` is left as it was. So the bench learns a project's path, with the Build's own reason, without a Cargo build. The IDE, `rcrun` and the agent's `build` tool keep it off.
    - **Per project:**
      - two copies are laid out (each with its pristine `data/`);
      - the first is built with `wasm_only` into the runner beside the bench (`runner_near`);
      - a project that stops has not moved, and its reason is printed;
      - otherwise the second copy is built with `force_cargo`, and every script runs once under each program, at the three sizes, under the T7 pins.
      - Compared: every frame by hash (a differing one pixel by pixel, a frame only one program drew counting all its pixels), every `read` and `dump` in order, the Cargo-built frames against the goldens (reference projects), and a fixture's expected reads under both programs.
      - Moved means: the WebAssembly path, 0 differing pixels, every read and dump equal, the goldens met, and the expected reads met.
    - **The form fixtures (decision, not in the task text):** at M2 no reference project takes the WebAssembly path, so the task's "for every reference project the WebAssembly path builds" would compare nothing. A form fixture, written in code like the runner twins (`crates/cobolt-bench/src/form_fixtures.rs`), gives the mode at least one pair from now on:
      - `ParityControls`: one form, 720 by 460, of 20 controls the runner draws: three labels, a text box, a check box, a switch, two radio buttons, a progress bar, a slider, a gauge, a knob, a list box, a bar chart, a timer and five buttons. Its handlers copy text, toggle the switch from the check box, set four values, add to the list and count it, and start a timer that stops itself.
      - Its script has 27 steps with 6 pictures, 6 dumps and 3 reads.
      - **Expected reads (`expect`):** two runs that both did nothing would be equal. So the fixture says what its reads must see, under both programs: LBL-TITLE "Ann", LBL-COUNT "2", LBL-TICKS "ticked".
    - **CI:** `pixel-parity.yml` builds `prc-runner` (release) after the golden step and runs the pairs on the four platforms. Its dispatch is owed to the operator (from the default branch, as T7's).
    - **Result** (`cargo run -p cobolt-bench --release -- pixel-parity`, macOS aarch64, commit cb32e979 plus this change):
      - ParityControls: WebAssembly path. 18 frames compared, 18 identical, 0 differing pixels. 27 of 27 reads and dumps equal. 18 of 18 expected reads met. Its WebAssembly Build took 0.2 s, its Cargo Build 152.7 s, and the runs 7.2 s; the whole mode 2.7 min.
      - Not moved, each with the Build's reason:
        - PowerCallOfDuty: CNV-3D (Canvas3D), M6;
        - PowerCloud: Aws-Mcp (AwsMcp), M3, M4 and M6;
        - PowerCobolCraft: CNV-DIORAMA (Canvas3D), M6;
        - PowerGraphics: CNV-GAME (Canvas2D), M6;
        - PowerSpatial: `src/ps-setting-get.cbl` line 46, SETTINGS-FILE on disk, M3.
      - moved: 1 of 6.
    - **Tests:**
      - `desktop_wasm_build`: 38 of 38. Its section 8 is new: `wasm_only` stops the Maps form in 11.3 ms with `WasmOnly { DesktopControl MAP-CITY }` and nothing compiled; it stops `build_path = "cargo"` with `Chosen` and `bin/` unchanged; and it builds the Button and Label form on the WebAssembly path as before (6 676.6 ms).
      - `cobolt-bench` unit tests: 3 passed. `a_pair_counts_every_differing_pixel_and_read` 7 of 7: 4 frames, 235 differing pixels, 3 reads with 1 equal, 3 expected reads with 1 met.
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

- [x] **T35 — The same form on every surface and on every platform** (R2, R5, R9; AC3 forms, AC14)
  - *Done 2026-11-16 (2.0.535):*
    - **AC14, the same form on every surface:** a new test, `runner_surfaces` (`cobolt-cli`), pictures the guard's reference form as it opens (a Panel holding an AreaChart, a PictureBox and a TextBox, and a Label, 400 by 300, no handler), three ways:
      - by the designer's render path, `snapshot::render_form_image` (the picture of the designer's preview and of the agent's `render_form`);
      - under `rcrun run-form` (the interpreter), headless with T30's pins;
      - in the program `rcrun build` makes on the WebAssembly path (the runner), the same.
      - Result: 6 of 6 checks. 0 differing pixels in each of the three pairs, 360 000 pixels compared (119 913 inked). The designer's picture took 650 ms, the interpreter run 4 805 ms, the runner run 2 703 ms, and the Build 7 976 ms.
      - `engine_reference_form_parity_static_vs_faces` (the geometry half: the canvas entry `render_faces` against `render_form`) is green.
      - **A fix found on the way (2.0.534, `fixes-2x-snapshot-visuals`, pushed, then picked into this branch):** the designer's picture differed from both runs by 1 486 pixels, at the edge of every glyph and anti-aliased line. Run Form's host draws on light visuals; the picture was made on a fresh context, in dark mode, whose text coverage curve inks darker. The picture now starts from the host's light visuals. Its own test, `designer_picture_is_run_form` (form host), compares the picture with the real host and interpreter: 138 differing pixels before, 0 after. With the fix, `cobolt-forms --features render` keeps 101 targets green (1 877 tests). Its one red, `example_corpus_golden`, is the three PowerDemo3 goldens recorded before this task, and that test does not call the snapshot. `desktop_comparison_chrome`, `page_border_look` and the designer's `designer_css_effective` stay green.
    - **AC3, forms on every platform (decision, not in the task text):** at M2 no reference project has moved (T34), so "the form projects that moved" are none. The `cross-platform` job gains T34's form fixture instead, as T34's pairs mode did. A reference project joins the same way once it moves.
      - `desktop_wasm_ci` includes `cobolt-bench/src/form_fixtures.rs` by path, so the fixture has one definition. The fixture's two read checks (`reads_of`, `missed_expectations`) moved into that file from `pixel_parity.rs`, unchanged.
      - `fixtures` (the `tools` and `no-toolchain` jobs) builds each form fixture with `rcrun`, which must take the WebAssembly path. It then runs each script headless with the pins, and requires every picture and the expected reads. `--modules-out` keeps the module.
      - `cross` runs every platform's module of the fixture headless on this platform's runner. Each must take every picture and meet the expected reads, and all must draw the same pictures (the PNG files' sha256) and give the same reads and dumps here.
      - `desktop-wasm.yml`: only its header changes, since the jobs already run `fixtures` and `cross`.
      - **Result on this Mac** (debug `rcrun`, `prc-runner` and helper):
        - `fixtures`: console 2 of 2; ParityControls 1 of 1, built on the WebAssembly path in 13.2 s, 27 steps, 6 of 6 pictures, 9 reads and dumps, 3 of 3 expected reads, 11.1 s.
        - `cross`, with this Mac's modules laid out twice as two platforms: console 4 of 4; form 2 of 2; the same on this runner 1 of 1. Each module is the same file both times.
      - The dispatch on the four platforms is owed to the operator, with T22's.
    - **Tests:** `cobolt-bench` 3 passed, 0 failed, among them the pairs test, which uses the moved checks.
  - Verify:
    - **AC14:** a test renders a reference form in the designer's render path, under `rcrun run-form` and under the runner, and gets equal pixels. `engine_reference_form_parity_static_vs_faces` green.
    - **AC3:** the `cross-platform` job of T22 gains the form projects that moved.

- [x] **T36 — M2 close**
  - *Done 2026-11-16 (2.0.540), on macOS arm64 (Apple M3 Pro, 11 cores, 18 GB; not a reference box), at commit d27ca2d7 (2.0.536):*
    - **Per project at M2 (the record the close asks for):**

      | Project | Path | Reason for the Cargo path | AC19 | Slowest Build (30 s goal) |
      |---|---|---|---|---|
      | PowerCallOfDuty | Cargo | WAR-FORM's CNV-3D (Canvas3D), M6 | not comparable (no WebAssembly-built form) | 272.8 s, over |
      | PowerCloud | Cargo | MAIN-FORM's Aws-Mcp (AwsMcp), M3, M4 and M6 | not comparable | 215.6 s, over |
      | PowerCobolCraft | Cargo | TITLE-FORM's CNV-DIORAMA (Canvas3D), M6 | not comparable | 278.5 s, over |
      | PowerGraphics | Cargo | GAME-FORM's CNV-GAME (Canvas2D), M6 | not comparable | 295.1 s, over |
      | PowerSpatial | Cargo | `src/ps-setting-get.cbl` line 46, SETTINGS-FILE on disk, M3 | not comparable | 235.5 s, over |
      | TenTimes | Cargo | `generated/form-001.cbl` line 268, IDX-01-FILE on disk, M3 | — | 364.2 s, over |

      - The generator alone accepts PowerCallOfDuty (6.1 ms) and PowerGraphics (45.9 ms), as at T26. Behind the first reason, the others also hold statements the generator refuses: PowerCloud `AWS-SET-CREDENTIALS`, PowerCobolCraft a statement reading more than four properties, PowerSpatial `ACCEPT FROM Environment`, TenTimes its indexed file.
    - **The first 30 s judgement (AC6):** no reference project takes the WebAssembly path at M2, so there is no reference Build on that path to judge. The one WebAssembly-path Build measured is T34's form fixture `ParityControls`: 0.3 s, against 503.0 s on the Cargo path.
      - The reference projects' Cargo Builds (`build-goal --runs 1`, each from an empty build folder) took 215.6–364.2 s, peak memory 3 636–4 619 MB.
      - That is slower than T26's run on this Mac (139.3–185.4 s); I did not find out why.
    - **`pixel-parity` (pairs, AC19):**
      - moved 1 of 6: ParityControls on the WebAssembly path, 18 of 18 frames identical, 0 differing pixels, 27 of 27 reads and dumps equal, 18 of 18 expected reads met (8.7 min);
      - the five reference projects stop before compiling, each with the reason in the table.
    - **`pixel-parity --cargo`:** 408 of 408 golden frames match and 22 of 22 forms are repeatable (22.7 min). The two fixes carried since T35 (2.0.534, the designer's picture; 2.0.536, the zoom of a form larger than the screen) moved no pixel on the Cargo path.
    - **AC9 (NIST through the runner):**
      - NC (Nucleus): compile 4 of 95, execution 4 of 95 with 0 failures (53 assertions passed);
      - SQ (Sequential I/O): compile 3 of 85, execution 3 of 85 with 0 failures (24 assertions passed);
      - the other 91 and 82 programs are refused for a file on disk (M3), as at T26;
      - `tests/cobol`: 16 of 18 identical (interpreter 15.6 s, runner 38.6 s); the 2 others differ only by the interpreter's warning line for a `CALL` of an unknown program (#54).
    - **The runner's size (C12):** the release `prc-runner` is 48 141 296 bytes (45.9 MiB), macOS arm64, against 10 336 576 at M1.
      - It now draws forms, so it carries the window, the GPU drawing and the form engine. It needs only system frameworks (AppKit, Metal, QuartzCore, CoreGraphics, Foundation, Security and others) and libiconv.
      - It now links `cobolt-runtime` (default features off), through the form host and for the test pins, the stack size and the settings it publishes. That is what C12 expected; at M1 it did not.
    - **Tests:**
      - the six crates of T26: 468 passed, 0 failed, 9 ignored (66 test binaries);
      - `cobolt-form-host` and `cobolt-abi-host`: 484 passed, 0 failed (13);
      - `cobolt-forms --features render`: 1 877 passed, 1 failed, 2 ignored (102). The red is `example_corpus_golden`'s three PowerDemo3 forms (`chart-3d`, `charts-form`, `responsive-dashboard-form`), recorded before this task;
      - `cobolt-ide --bin`: 2 030 passed, 1 failed (the known `docs_embed`), 6 ignored;
      - T1's test: 3 of 3 artefacts match their pins.
    - **Owed at the merge into `main`:** the Developer's Guide's list of what takes the Cargo path, with T25's §17 and §18 (the 2.x line does not touch `docs/` here). The table above is that list at M2.
    - **Operator, 2026-11-16:** "unless there is a technical reason, no project should rely on cargo build". So from M3 on, the order favours the tasks that remove a reference project's reason. Files (M3) unblock PowerSpatial and TenTimes. The canvases (M6) unblock PowerCallOfDuty, PowerCobolCraft and PowerGraphics.
  - Do: the milestone close (preamble).
  - Verify:
    - The first 30 s judgement on the WebAssembly path, for each reference project that moved (AC6).
    - The guide's fallback list is updated.
    - The sweeps of T26, plus `cargo test -p cobolt-form-host -p cobolt-abi-host` and `cargo test -p cobolt-forms --features render`.

## Phase M3 — Disk files, and `SORT` and `MERGE` with files

- [x] **T37 — `SequentialFile` and the sort core, extracted from the interpreter unchanged** (C5)
  - *Done 2026-11-16 (2.0.541):*
    - **`crates/cobolt-runtime/src/seqfile.rs`:** `SequentialFile` holds a sequential or line-sequential file open in any mode. Its `open`, `read`, `write`, `rewrite` and `close` are the interpreter's former `Writer`/`Reader` arms, and it decides each status the same way: `refuses_absent` and `opened_status` (35, 05, 00), `failed_status` (35, 30), `SeqRead::status` (00, 10, 30, 47), and the rewrite's 49, 43, 44 and 00. `RecordPlace` (the interpreter's `LastRead`) and `VARYING_LEN_PREFIX` moved with it. Three unit tests cover the open statuses, the record round trip (fixed, variable with its four-byte prefix, line) and each rewrite refusal.
    - **`crates/cobolt-runtime/src/sort_core.rs`:** `compare_keys` and `sort_keyed`, the comparison `SORT` and `MERGE` used: the collating sequence for two alphanumeric keys, `cob_ordering` (now `pub(crate)`) otherwise, and descending keys reversed.
    - **The interpreter:** `OpenFile::Sequential(SequentialFile)` replaces `Writer` and `Reader`. `exec_open`, `exec_close`, `exec_read`, `exec_write`, `exec_rewrite`, `read_all_records`, `write_all_records` and `sort_records` call the two modules. 310 lines out, 67 in.
    - **`cargo test -p cobolt-runtime`:** 211 test programs (the library, 209 integration targets, the doc test), 1 678 passed, 0 failed, 29 ignored. Run in batches of 30 targets, each batch's binaries deleted after it ran: the 209 targets together need about 47 GB.
    - **NIST, execution on the interpreter (GOLDEN RULE #9, every finished module):** each count equals the ledger's and the run before this change, all with 0 failures. NC (Nucleus) 95 of 95, 4 614 assertions passed; SQ (Sequential I/O) 85 of 85, 624; IX (Indexed I/O) 41 of 41, 574; RL (Relative I/O) 34 of 34, 354; ST (Sort/Merge) 39 of 39, 735; IF (Intrinsic Functions) 45 of 45, 841; IC (Inter-program Communication) 25 of 25, 309; SM (Source Text Manipulation) 16 of 16, 311.
  - Files: new `crates/cobolt-runtime/src/seqfile.rs`, the sort core beside it, and the interpreter's file and `SORT` code calling them.
  - Verify:
    - `cargo test -p cobolt-runtime` green.
    - The NIST execution runs of SQ, IX, RL and ST give the counts recorded for the interpreter (GOLDEN RULE #9: every finished module's execution pass).

- [x] **T38 — Files of every organisation as capabilities** (R10, R11)
  - Files:
    - `crates/cobolt-wasm/src/` (the file verbs lowered to the `prn` `f_*` imports under `native`);
    - `crates/cobolt-runner/src/` (the `files.*` capabilities over `SequentialFile`, `RelativeFile` and `IndexedStore`);
    - the native column.
  - Do: records cross by `(ptr, len)` in memory 0. `FILE STATUS` values are the interpreter's.
  - Verify: one runner test per organisation and verb. Its file contents and `FILE STATUS` values are equal to the interpreter's.
  - *Done 2026-11-16 (2.0.542):*
    - **ABI A34** (`specs/WASM-ABI-v1.md`, `cobolt-wasm-abi/src/files.rs`, `codes.rs`): the `prfiles` custom section (JSON, format 1: each file's organisation, access mode, record lengths, keys, engine, `STORAGE`/`WITH PERSISTENCE`, a RELATIVE KEY's digits and an INDEXED file's definition text) and seven `prn` imports, `f_open`, `f_close`, `f_read`, `f_write`, `f_rewrite`, `f_delete` and `f_start`. Each answers the `FILE STATUS` as a number; records and keys cross by `(ptr, len)` in memory 0. The capabilities `files.sequential`, `files.line-sequential`, `files.relative` and `files.indexed`.
    - **The generator** (`lower_nfile.rs`, `module.rs`, `lower.rs`): under `native`, every file but a SORT work file is a runner file, one declared `STORAGE IS MEMORY` too (on the desktop it is read from its file, and opens for INPUT only without `WITH PERSISTENCE`). The verbs call the runtime's `rt_nf_*` routines (`cobolt-wasm-rt/src/nfile.rs`), which do the program's half of each statement as the interpreter does: the ASSIGN path, the key, the record number, a short record padded with spaces, `RECORD … DEPENDING ON`, the RELATIVE KEY and the FILE STATUS item. A `LINAGE` file is refused with its line.
    - **The runner** (`cobolt-runner/src/files.rs`): files keyed by their slot, opened through `cobolt_runtime::file_core` (new, the interpreter's open and verb code shared, C5) with the engine the interpreter would choose: the project's `[ide] indexed_engine` (`prsettings`), else `COBOL_INDEXED_ENGINE`, else the default. A form's files belong to its window and close with it.
    - **Shared with the interpreter, unchanged:** `cobolt_ast::fd_text::file_definition` and `item_extents` (an INDEXED file's definition text) and `cobolt_ast::data::pic_integer_digits` moved out of `cobolt-runtime`, which now calls them.
    - **Decision:** a DataGrid bound to an indexed file (`_BindingKind=IndexedFile`) stays on the Cargo path, relabelled M6: it needs the grid's binding machinery (refresh, ledger, write-back, specs 094/095), not just the file verbs.
    - **Web modules (AC16):** the runtime blob's new imports renumber its function types, and the blob's edits moved four bytes of panic line numbers, so a browser module's bytes moved with no instruction or data changed (checked on the player fixtures: equal code and data sections, the WAT equal but for type numbers). `nfile.rs` is written without a panicking call, so it adds nothing to the read-only data every module carries. The PowerAnalyticsWeb pin is re-recorded, the player fixtures regenerated and the shipped player rebuilt (`freshness` 6 of 6).
    - **Found:** the interpreter drops leading zeros when an unsigned integer is moved into a reference-modified alphanumeric item (`MOVE WS-I TO WS-REC(5:2)`, `WS-I` = 1, gives `"1 "`). A COBOL-85 fix of its own; the twin test avoids it.
    - **Measured:**
      - `cobolt-runner --test files`, one program per organisation under the runner and `rcrun run`, lines and files compared byte for byte: sequential 33 lines and 4 files (193 bytes), relative 34 lines and 2 files (387 bytes), indexed 26 lines and 4 files (45 056 bytes); 50 of 50 checks.
      - `native_desktop_refusals` 15 of 15 (a disk file compiles and names its capability; `LINAGE` refused with its line). `native_console` 2 of 2.
      - `cargo test` of `cobolt-wasm`, `cobolt-wasm-abi`, `cobolt-wasm-rt`, `cobolt-ast` and `cobolt-runner`: green. `cobolt-runtime`: the library and the six file targets, 638 passed. The compiler's subset and runner-gap tests green.
      - NIST on the interpreter, all eight finished modules, 0 failures: NC 95/95, SQ 85/85, IX 41/41, RL 34/34, ST 39/39, IF 45/45, IC 25/25, SM 16/16.
      - The System KB regenerated (4 412 records); `prebuilt_chunked_kb_matches_the_published_documentation` green.

- [x] **T39 — `SORT` and `MERGE` with `USING`/`GIVING`; `COMMIT`, `ROLLBACK`, `UNLOCK`** (R11)
  - Verify: runner tests equal to the interpreter's output and files. The native column moves these entries to `Host(sort)` or `Host(files.*)`.
  - *Done 2026-11-16 (2.0.543):*
    - **ABI A35** (`specs/WASM-ABI-v1.md`, `cobolt-wasm-abi/src/files.rs`, `codes.rs`): one `prn` import, `f_tx(op, id, slot)`, with `verb::TX_UNLOCK`, `TX_COMMIT`, `TX_ROLLBACK`. It joins the `files.indexed` set (`codes::INDEXED_FILE_IMPORTS`: A34's seven verbs plus `f_tx`).
    - **The generator** (`lower_idx.rs`, `lower_nfile.rs`, `module.rs`): under `native`, `SORT … USING` and `MERGE … USING` read each file through `rt_nf_sort_using` (`cobolt-wasm-rt/src/nfile.rs`: `f_open` INPUT, `f_read` NEXT until a status other than `00`, `f_close`), which overlays each record on the sort record area and keeps it at its own length, as the interpreter does. `GIVING` writes through `rt_nf_sort_giving`: at each record's own length after `USING`, at the sort record's length after an `INPUT PROCEDURE`. The in-module sort (A27) is unchanged. `UNLOCK` (on an INDEXED file), `COMMIT` and `ROLLBACK` call `rt_nf_tx`. A relative or indexed `USING`/`GIVING` file and `COLLATING SEQUENCE` are refused with their line. The browser path is unchanged: there, `SORT` with files and these statements are still refused.
    - **The runner** (`cobolt-runner/src/files.rs`, `lib.rs`): `f_tx` unlocks one indexed file, and commits or rolls back every indexed file of the calling instance, as each window's interpreter does.
    - **The native column:** `SORT`, `MERGE`, `UNLOCK`, `COMMIT` and `ROLLBACK` become `Native::Files`; the `D_SORT` and `D_FILES_M3` reasons are gone.
    - **Known edge differences:** `GIVING` on the runner applies a variable-length file's record range, as `WRITE` does (status 44, record not written), where the interpreter's raw write does not. A short record's fields past its end after `RETURN` hold whatever was there before, and that may differ between the paths.
    - **Found:**
      - `UNLOCK` is not a reserved word (`stmt.rs` parses it by name), so a `DISPLAY` or `OPEN` operand list swallows the word. The twin test ends those statements with a period. A fix of its own.
      - The interpreter's `SORT … USING` reads only sequential and line-sequential files: a relative or indexed `USING` file gives no records. A COBOL-85 fix of its own; the WebAssembly path refuses such a file until both paths read it.
    - **Web modules (AC16):** the runtime's new routines renumber the blob's function types again; every instruction and data byte of the player fixtures is otherwise equal (checked on the WAT). The PowerAnalyticsWeb pin is re-recorded, the player fixtures regenerated and the shipped player rebuilt (`freshness` 6 of 6).
    - **Measured:**
      - `cobolt-runner --test files`: sorting 44 lines and 7 files (439 bytes), transactions 12 lines and 3 files (20 480 bytes), under the runner and `rcrun run`, compared byte for byte; with T38's programs, 88 of 88 checks. `ROLLBACK` undid record 3 on both paths.
      - `native_desktop_refusals` 18 of 18.
      - `cargo test -p cobolt-wasm`: 34 test programs, 122 passed, 0 failed. `cobolt-runner`: 6, 8 passed. `cobolt-wasm-abi` and `cobolt-wasm-rt`: green. The compiler's subset and runner-gap tests green. The player's `freshness` 6 of 6.
      - The interpreter is not changed (no `cobolt-runtime` edit), so its NIST counts stand as at T38.
      - The System KB regenerated (4 414 records); `prebuilt_chunked_kb_matches_the_published_documentation` green.

- [x] **T39b — `LINAGE` and `SAME [RECORD] AREA`** (R11; added 2026-11-16)
  - Why added: the generator's own refusals named both as M3 work ("LINAGE and SAME AREA are not on the WebAssembly path yet (DESKTOP-WASM-001 M3)"). The first census through the runner found them in 13 NIST programs: SQ114A, SQ201M, SQ206A, SQ208M, SQ209M, SQ210M, IX105A, IX107A, IX205A, RL106A, ST131A, ST134A and ST135A. M3 cannot close with them refused.
  - Verify: runner twins equal to the interpreter's printed lines and files. `native_desktop_refusals` names what still takes the Cargo path. AC16 unchanged.
  - *Done 2026-11-16 (2.0.547):*
    - **No ABI change.** Both are kept in the module, as the interpreter keeps them (`advance_linage`, and the peer copy in `exec_read`). The runner and `cobolt-wasm-rt` are unchanged.
    - **`LINAGE`** (`lower_nfile.rs`: `LinageMeta`, `linage_open`, `linage_write`, `page_size`, `read_at_least`; `lower.rs`: `LINAGE-COUNTER`):
      - the page lives in 24 hidden bytes per file: the counter, the lines to advance and a scratch slot. A page measure in a data item is read at each `WRITE`, and a value of 0 or less means the stated number.
      - `LINAGE-COUNTER` is an 18-digit register the module sets after a successful `OPEN` (1) and after each `WRITE`. It is read wherever `RETURN-CODE` is.
      - writing it (`MOVE`, arithmetic) is refused naming `LINAGE-COUNTER`, and so is `ADVANCING` a mnemonic-name (named `WRITE ADVANCING`). Both are at their line, so the Build takes the Cargo path for them.
    - **`SAME [RECORD] AREA`** (`lower_nfile.rs`: `same_area_copy`, `area_extent`):
      - after a `READ` that returns 00, the record area of each file joined to the one read takes the record: `memory.copy` of the shorter length, then spaces to the end of a longer area. This happens before `INTO` and the phrases, at the same point as in the interpreter.
      - a peer can be a file the runner keeps or a `SORT` work file.
    - **Docs:** the Desktop column's note on files (`cobolt-compiler`) and the module doc of `lower_nfile.rs`. The System KB was regenerated (4 420 records), and `prebuilt_chunked_kb_matches_the_published_documentation` passes.
    - **Measured:**
      - `cobolt-runner --test files`: linage 23 lines and 2 files (96 bytes), same area 12 lines and 5 files (20 576 bytes), equal under the runner and `rcrun run`. With the earlier programs, 132 of 132 checks.
      - `native_desktop_refusals`: 20 of 20.
      - `cargo test -p cobolt-wasm`: 34 test programs, 124 passed, 0 failed. `cobolt-runner`: 8 passed, 0 failed.
      - The PowerAnalyticsWeb pin and the player fixtures are unchanged.
      - Generator NIST, unchanged: IC 25/25 (309/0/4), NC 95/95 (4 612/0/5), IF 45/45 (841/0/0).

- [x] **T39c — What M3 still sends to the Cargo path** (R11; added 2026-11-17)
  - Why added: T40's measurement found refusals the generator still names as M3 work. M3 cannot close while they stand:
    - **The file and image built-ins** ("the file and image built-ins are not on the WebAssembly path yet"): `COBOL::"APPEND-FILE"`, `"WRITE-FILE"`, `"FILE-STATUS"`, `"IMAGE-FIT"` and `"IMAGE-INFO"`. PowerCloud calls `IMAGE-FIT` and `IMAGE-INFO` three times each, and PowerChat calls `APPEND-FILE` and `WRITE-FILE`.
    - **The file controls**, inside "the service controls (files, …)": the IndexedFile control and FileDropZone. PowerCloud holds a FileDropZone.
    - **`SORT` and `MERGE` with a relative or indexed `USING` or `GIVING` file**, left by T39. The interpreter's own `SORT` reads nothing from such a `USING` file, a COBOL-85 fix of its own that lands first.
    - **`OPEN … WITH REGISTERED USER`.**
  - What the built-ins do under the interpreter (the rules to keep):
    - `APPEND-FILE` and `WRITE-FILE` (path, text, [status]): the text without its trailing spaces, as one line, added to or replacing the file; the status is spaces or why it failed.
    - `FILE-STATUS` (file, status): the last I-O status of the named file.
    - `IMAGE-FIT` (source, target, width, height, status) and `IMAGE-INFO` (path, width, height, format, status): through the runtime's image code.
    - A relative path starts at the application's folder, as an `ASSIGN` does.
  - Verify: runner twins equal to `rcrun run` in printed lines and in the files written. `native_desktop_refusals` and `desktop_controls_the_runner_lacks_are_named` no longer name them. No refusal names M3. AC16 unchanged.
  - *The file and image built-ins done 2026-11-17 (2.0.576); FILE-STATUS, FileDropZone, the IndexedFile control, `SORT`/`MERGE` with a relative or indexed file and `OPEN … WITH REGISTERED USER` are still open, so the box stays empty:*
    - **What runs (ABI A39, capabilities `files.text` and `images`):** `COBOL::"APPEND-FILE"`, `"WRITE-FILE"`, `"IMAGE-FIT"` and `"IMAGE-INFO"` compile to three `prn` imports, `t_write`, `img_fit` and `img_info`, in a form's handler and in a program with no form alike.
      - The module hands each path and text whole, as the interpreter's value of the argument (a number as its text, a figurative constant as its character), and a width or a height as its whole number (`deskio.rs`).
      - The runner does the work with the interpreter's own code: the path trimmed and anchored at the application's folder, the line's trailing spaces dropped, `std::fs` with the same options; `cobolt_runtime::image_runtime` for the pictures (the runner's runtime gains the `images` feature).
      - Each answer goes into its receiver as the interpreter sets it: a word as `set_str` does; a width or a height as `set_i64` does, which puts a number into an item of characters right-justified, keeping the digits' left end when they do not fit.
    - **FILE-STATUS** stays on the Cargo path, with a reason of its own: its answer is the last status of a file the IndexedFile control keeps, so it comes with that control.
    - **Tests:**
      - the runner twin `text_files_and_pictures_match_the_interpreter`: 15 lines printed and 6 files (1 490 bytes: two text files and the PNG and JPEG pictures IMAGE-FIT writes) equal byte for byte to `rcrun run`, 27 of 27 checks. It covers a padded path, a value method (`::Trim()`), numeric, signed and figurative texts, a missing folder's status, a decimal width and a text height, the refusals `BADSIZE`, `BADEXT`, `NOFILE` and `BADIMAGE`, and numeric, alphanumeric and too-short receivers;
      - the manifest of a program that only appends asks for `files.text` and imports `t_write` alone.
      - The twin found that `IMAGE-INFO` must put a number into an alphanumeric receiver right-justified (`    40`, not `40    `), as the interpreter's `set_i64` does.
    - **AC16, named:** the task asks for AC16 unchanged, and a browser module's bytes did move, as at 2.0.542 (T38). The generator copies the runtime's whole type section and data segment into every module: the two new import signatures renumber its function types, and the panic messages naming `deskio.rs` move later data by 64 bytes. With type numbers and data addresses masked, every instruction of the player's fixture module is the same, so nothing a browser module does changed. The pin was re-recorded, the player's fixtures regenerated and the web player rebuilt. Keeping a browser module's bytes still across runtime changes would need the generator to copy only the types and data its closure uses: a change of its own, not made here.
    - **PowerCloud's** only reason left is its FileDropZone.
  - *FileDropZone done 2026-11-17 (2.0.577); the IndexedFile control with FILE-STATUS, `SORT`/`MERGE` with a relative or indexed file and `OPEN … WITH REGISTERED USER` are still open:*
    - **What runs:**
      - Taking files in (a drop or the picker) was already the form host's, shared by both engines: it judges the files by the zone's rules, copies or stages them, and raises `onFilesDropped` and `onFilesRejected`. The runner gives the zone's new values to the ABI host before the event, as for any edit.
      - `CommitFiles()` is the runner's. The ABI host hands the zone's `StagedFiles`, its review list's ticked rows and its `DestinationFolder` to the runner (`DropZonePort`), which runs `cobolt_forms::dropzone::commit_staged`, the interpreter's own code, moved out of the interpreter unchanged.
      - The answers go back in the interpreter's order: the list's `Items` and `CheckedItems`, then the zone's `DroppedFiles` and `CommitSummary`, which is also the method's answer. In a page the call is refused as before.
    - **No ABI change:** `CommitFiles()` is an ordinary control call.
    - **Tests:** the runner twin `file_drop_zones_take_stage_and_commit_alike` (four zones fed by the picker: staged and committed, staged and unticked, copied at once with a refusal, and staged for a review list the form does not hold, committed twice): 20 readings, one dump and 2 pictures (0 differing pixels in 972 000) equal to `rcrun run-form`, the same 4 files (2 845 bytes) copied, 10 of 10 checks.
    - **The twin found** that the interpreter keeps what a drop writes to a review list the form does not hold and reads it back at `CommitFiles()` (the staged rows count as ticked; after a commit only the committed ones do, so a second commit copies nothing, as with a real list). The ABI host now keeps the values of such an absent control too, and hands `CommitFiles()`'s writes to it back to the window in the interpreter's order.
    - **The twin's project has `assets/`,** as every project: a built program anchors a relative folder at the folder above it that holds `assets/`, as a Cargo-built program does (else at `bin/`).
    - **PowerCloud** no longer takes the Cargo path for its FileDropZone (`cobolt-bench pixel-parity --project PowerCloud`, 2026-11-17). Its Build now stops at the generator: its main form reads `WS-OUTPUTS::Split( … )(2)::Split( … )(1)`, a value method's answer indexed and then another method on that part, which the WebAssembly path does not take yet (the interpreter does). Next in T39c.
  - *`::Split( … )( i )` done 2026-11-17 (2.0.579), with the fix it needed (2.0.578):*
    - **What runs:**
      - `X::Split( sep )( i )` is part `i` of the text, counted from 1, as the interpreter reads it:
        - an index of 0 or less reads as 1;
        - a decimal is truncated;
        - a text that spells no number reads as 1;
        - a part past the last is empty.
      - A method may follow the part, and a chain may hold two Splits. The part can be moved, displayed, compared or used in arithmetic.
    - **The fix it needed (2.0.578, WASM-PARITY #73):** a value method's answer used in arithmetic read as zero on the module, unless the method was `::Len`. It now reads as the number its text spells, as in the interpreter.
    - **Tests:**
      - `cobolt-wasm` `split_with_an_index_matches_the_interpreter`: 19 rows, equal on both engines and both memory variants.
      - `a_value_methods_answer_in_arithmetic_is_the_number_it_spells`: 8 rows.
    - **AC16, named:** the routine that answers a value method is in every module, so the PowerAnalyticsWeb pin and the player's generated fixtures were re-recorded at 2.0.578 and again at 2.0.579.
    - **PowerCloud** (`cobolt-bench pixel-parity --project PowerCloud`, 2026-11-17) now stops at `INVOKE WS-WORK::Close()` in `TMR-EC2`'s `onTick`. That is a method called through a windowHandler, which the WebAssembly path does not take yet: `WS-WORK` is a `USAGE OBJECT` item bound by `ME::OpenFormAsync( "WORKING-FORM" ) RETURNING WS-WORK`. Next in T39c.
  - *windowHandlers done 2026-11-17 (2.0.580, ABI A40):*
    - **What runs:**
      - A `USAGE OBJECT` item with no class, filled by `OpenFormSync` / `OpenFormAsync … RETURNING`, is a windowHandler, as in the interpreter (spec 037 R20, R23, R24).
      - A call or a read through it (`INVOKE WS-WORK::Close()`, `SetProperty`, `GetProperty`, the window methods `super::` reaches) goes to the form of the window it names.
      - Once that window has closed, a call through it is refused in the interpreter's words (`windowHandler WS-WORK is NULL — the form it referred to is closed`, now one text in `cobolt_forms::exceptions` for both engines). On the desktop that is the handler's unhandled exception: the handler stops there.
    - **Known differences, recorded in A40:**
      - the item keeps the handle after its window closes (the interpreter's becomes empty);
      - a desktop window the runner has not made yet (the instant after an `OpenFormAsync`) answers as a closed one;
      - an item never filled is refused as NULL, where the interpreter tries the call as an ordinary object's.
    - **Tests:**
      - `cobolt-abi-host` `a_call_through_a_window_handle_reaches_the_form_of_that_window_and_a_closed_one_is_refused`: 10 of 10 checks.
      - Runner twin `a_window_handler_reaches_its_window_and_is_null_once_it_closes`: 3 readings, 3 dumps and 3 pictures (768 000 pixels, 0 differing) equal to `rcrun run-form`, 11 of 11 checks.
    - **PowerCloud** (`cobolt-bench pixel-parity --project PowerCloud`, 2026-11-17) gets past its windowHandler. Its Build now stops at the generator on `generated/main-form.cbl` line 2888: the concatenation operator `&` in a method's argument (`'{"StackName":"' & TXT-STACK::Text::Trim() & '"}'`), which the WebAssembly path does not take yet. Next.
  - *The concatenation operator `&` done 2026-11-17 (2.0.583), with the fix it needed (2.0.582):*
    - **What runs:** `a & b & c` is the texts of its parts joined, each as the interpreter's value of it: an item with its spaces, a method's answer, a number spelled from its value (`n=42`, `n=-7`, `n=1.50`). The interpreter's `&` and its `FUNCTION CONCATENATE` are one rule, so the generator makes `&` that function's call. The result can be moved, displayed, compared, passed as a method's argument, or used in arithmetic.
    - **The fix it needed (2.0.582, WASM-PARITY #74):** the module's `CONCATENATE` spelled a numeric argument by its picture (`n=0042`) where the interpreter spells its value (`n=42`).
    - **Tests:** `cobolt-wasm` `the_concatenation_operator_matches_the_interpreter`, 17 rows; `function_concatenate_spells_a_number_as_the_interpreter`, 9 rows; each equal on both engines and both memory variants.
    - **PowerCloud takes the WebAssembly path** (`cobolt-bench pixel-parity --project PowerCloud`, 2026-11-17): Build 0.4 s (Cargo 187.8 s); 5 forms, 49 steps at three sizes, 72 frame pairs identical, 0 differing pixels; the Cargo frames match the goldens, 72 of 72. All five reference projects now take the WebAssembly path.
  - *`SORT`/`MERGE` with a relative or indexed file done 2026-11-17 (2.0.586), after the interpreter's own fix (2.0.585):*
    - **The fix it needed first (2.0.585):** the interpreter's sort read and wrote only sequential files. From a relative or indexed `USING` file it took no record, and it wrote none to such a `GIVING` file. Both now go through the verbs `READ` and `WRITE` use. NIST ST execution is unchanged: 39 of 39 programs, 735 PASS, 0 FAIL.
    - **What runs:** a `USING` or `GIVING` file of any organisation.
      - The generator no longer refuses one.
      - The runner already read and wrote every organisation with the interpreter's file verbs.
      - The runtime's `GIVING` gives a relative file's records slots 1, 2, 3, … and leaves the `RELATIVE KEY` item on the last one, as the interpreter does.
    - **Tests:**
      - the runner twin `files_of_every_organisation_match_the_interpreter` gains `sort keyed`: 28 lines printed and 9 files (41 404 bytes) equal to `rcrun run`; 189 of 189 checks across the twin's ten programs. It covers:
        - an indexed `USING` file in key order;
        - a relative one past its empty slots;
        - relative `GIVING` in the sequential and random access modes (the `RELATIVE KEY` left on 3);
        - indexed `GIVING` in the sequential access mode;
        - a `MERGE` of two indexed files into a relative file (the key left on 9);
        - a record number wider than the `RELATIVE KEY` ending the reading (14);
        - every `FILE STATUS` the sorts leave.
      - `native_desktop_refusals`: an indexed `USING` file and a relative `GIVING` file compile; 21 of 21 checks.
    - **AC16 unchanged:** the pin and the player's fixtures pass without re-recording.
  - *`OPEN … WITH REGISTERED USER` done 2026-11-17 (2.0.587):*
    - **What runs:** the `OPEN` itself, as without the phrase.
      - The interpreter writes the user only into an INDEXED file's transaction log, which `rcrun` keeps when it is given `--indexed-log` or `COBOL_INDEXED_LOG`.
      - A built program keeps none on either path: the Cargo-built one never turns the log on. So the module leaves the phrase out.
      - The IndexedFile control's generated `OPEN` paragraph uses the phrase (`OPEN I-O MENU REGISTERED USER 'operator'`).
    - **Tests:**
      - the runner twin gains `registered`: 8 lines printed and 3 files (20 487 bytes) equal to `rcrun run`; 206 of 206 checks across its eleven programs. It covers a literal and a data item; an INDEXED file `OUTPUT`, `I-O` and `INPUT`; a LINE SEQUENTIAL file; and two files in one `OPEN`;
      - `native_desktop_refusals` 22 of 22.
    - The System Knowledge Base's Desktop column says so, and `chunked.data` is regenerated.
  - *The IndexedFile control and `FILE-STATUS` done 2026-11-17 (2.0.588). T39c is complete:*
    - **What runs:**
      - `COBOL::"FILE-STATUS" ( "file" item )` reads, in the module, the status the file's last verb left: every file verb on the runner's files keeps it beside the file's handle (a third word in the handle's place, 12 bytes now). It needs no import and no capability.
      - As the interpreter answers it: `00` before any verb and for a name no program declares; the name in any case, with spaces around it; a SORT work file `00` (`SORT`, `RELEASE` and `RETURN` set no status under its name); a file other programs of the module declare, read where its verbs left it (the module keeps one file per name, as the interpreter does, so programs that declare one name share it, and its status is the last verb's on any of them). The receiver is filled as the interpreter fills it: a group, an alphanumeric item (padded or cut), a numeric item.
      - The IndexedFile control is served. Without PartiQL it is a set of properties over the paragraphs the form generates (`OPEN … REGISTERED USER`, `START`, `READ`, `COMMIT`, `ROLLBACK`, `CLOSE`, each followed by `FILE-STATUS`), which use the program's own verbs. `AutoOpen` opens the file as the form starts.
      - Its PartiQL members (`Execute`, `ExecuteTransaction`, `Fetch`, `Get`, `ColumnNames`, `ColumnCount`, `RefreshBinding`, and the properties `ResultCount`, `StatusCode`, `LastError`, `BindingQuery`) are refused where they are used, naming the member and its line (T48, M6). A control bound to a PartiQL query was already refused (Data Binding).
      - Left to T45 (M5), refused at the call: `FILE-STATUS` with the file's name in a data item.
    - **Tests:**
      - the runner twin gains `file status`: 18 lines printed and 5 files (20 537 bytes) equal to `rcrun run`, across INDEXED, RELATIVE, LINE SEQUENTIAL and SEQUENTIAL files with and without a FILE STATUS clause, a SORT, a nested program and a file two nested programs declare; 0 statuses differ; all runner file twins 235 of 235 checks;
      - the form twin `an_indexed_file_control_opens_writes_and_reads_alike`: an IndexedFile control with `AutoOpen` on an OPTIONAL file, handlers that write, read and close; 8 readings, the dump and one picture (268 800 pixels, 0 differing) equal to `rcrun run-form`, the data files equal byte for byte; 6 of 6 checks;
      - `native_desktop_refusals` (30 of 30 checks, 49 programs compiled), `every_control_has_a_desktop_class` (63 served, 8 on the Cargo path), `desktop_controls_the_runner_lacks_are_named`, the System Knowledge Base test;
      - no refusal names M3; AC16 unchanged; the sweep of the five WebAssembly crates and the runner: 60 test binaries, 213 tests passed (the two reds: this refusal case, then corrected and rerun, and `cobolt-web-tests` `examples`, red from untracked operator files).
    - **Found on the way:** the end of a headless run is a race on both engines (WASM-PARITY #75), a fix of its own; and the IndexedFile paragraphs name the `.cidx` stem, not the form's `SELECT` name, when the two differ (PowerDemo3's `MENU` against `SELECT MENU-FILE`), a fix of its own.

- [x] **T40 — M3 close** (AC9 for SQ, IX, RL, ST)
  - Verify:
    - The NIST SQ, IX, RL and ST counts on both axes through the generator and the runner, reported separately.
    - The ten-times project's path and time.
    - The milestone close.
  - *Measured 2026-11-17 (2.0.566). The box stays empty until T39c is done and PowerCobolCraft and PowerSpatial draw every frame alike:*
    - **NIST through the runner (AC9), on both axes:** each program built on the WebAssembly path by `nist_runner_run` and run by `prc-runner`.
      - SQ: 85 of 85 compiled and clean, 624 PASS, 937 s.
      - IX: 41 of 41, 574 PASS. RL: 34 of 34, 354 PASS. ST: 39 of 39, 735 PASS.
      - NC, for comparison: 95 of 95, 4 615 PASS.
    - **The reference projects' paths** (`cobolt-bench build-goal`, which now times the generator in its desktop mode, as the Build runs it):
      - PowerCallOfDuty, PowerCobolCraft, PowerGraphics and PowerSpatial take the WebAssembly path, each built in under a second.
      - PowerCloud takes the Cargo path (160.6 s), for T48's AWS controls and T39c's image built-ins and FileDropZone.
      - The ten-times project takes the WebAssembly path in 55.8 s (front end 30.7 s, generator 22.1 s), over the 30 s goal.
    - **Pixel parity (AC19), the full run:** 354 frame pairs, 303 identical; reads and dumps 27 of 27 equal.
      - Moved: PowerCallOfDuty (12 of 12), PowerGraphics (120 of 120) and the ParityControls fixture (18 of 18).
      - Not yet: PowerCobolCraft (21 of 66) and PowerSpatial (132 of 138), 51 frames. There a form opened on its own calls `super::`: the interpreter ends that handler with its critical-exception notice, and the runner goes on. A fix of its own.
      - The Cargo-built frames match the goldens, 336 of 336.
    - **The corpus** (`nist_runner_run compare`): 62 programs, 33 compiled, 24 identical.
      - 7 `fileiot*` programs print the same lines under both engines and reach the 20 s limit under both.
      - 2 differ only by the interpreter's WARN line.
      - 29 are refused, with the construct named.
    - The release runner is 78.4 MiB.
  - *Pixel parity re-measured 2026-11-17 (2.0.575), after the fixes the 2.0.566 run found (2.0.570–2.0.575):*
    - **The fixes:** a desktop handler that fails stops there and the form shows the interpreter's notice (2.0.570); an out-of-range reference modification takes the interpreter's positions (2.0.571); a subscript outside its table reads the interpreter's blank and writes nowhere (2.0.572); a canvas drawn once just before a call to another graphics object is shown (2.0.573); a headless run with the clock pinned waits for a window the program opened (2.0.574); `Show`, `Hide`, `Enable`, `Disable` and `SetFocus` work on the graphics controls under the interpreter too (2.0.575).
    - **The full run** (`cobolt-bench pixel-parity`, 13.7 min): 354 frame pairs, **354 identical, 0 differing pixels**; reads and dumps 27 of 27 equal; expected reads 18 of 18.
      - Moved: ParityControls (18 of 18), PowerCallOfDuty (12 of 12), PowerCobolCraft (66 of 66), PowerGraphics (120 of 120) and PowerSpatial (138 of 138). The Cargo-built frames match their goldens, 336 of 336.
      - PowerCloud still takes the Cargo path, for its FileDropZone (T39c).
  - *Closed 2026-11-17 (2.0.592), after T39c (2.0.588) and the fixes 2.0.589–2.0.591:*
    - **NIST through the runner (AC9), on both axes:** each program built on the WebAssembly path by `nist_runner_run` and run by `prc-runner`, one module at a time; compiled and executed clean are the same count in every module.
      - SQ: 85 of 85 compiled and clean, 624 PASS, 0 FAIL, 965 s.
      - IX: 41 of 41, 574 PASS, 620 s.
      - RL: 34 of 34, 354 PASS, 511 s.
      - ST: 39 of 39, 735 PASS, 683 s.
      - RL and ST were run a second time, alone. The first run shared the machine with Cargo Builds and a test sweep: 17 ST programs and RL207A passed the 20 s limit, and the programs that read their files (ST114M, ST117A, RL208A) reported FAILs. Run alone, every one is clean. The limit is the harness's; nothing changed in the programs.
    - **The reference projects' paths** (`cobolt-bench build-goal`, 5 Builds each, a fresh build folder each): all six take the WebAssembly path, 30 of 30 Builds.
      - PowerCallOfDuty 0.4 s, PowerCloud 0.5 s (on the WebAssembly path since T39c), PowerCobolCraft 0.8 s, PowerGraphics 0.6 s, PowerSpatial 0.4 s.
      - The ten-times project: 70.9 s beside the NIST runs, 61.4 s alone (front end 29.4 s, generator 33.3 s, peak 6 745 MB), over the 30 s goal, which is reported, not enforced (D24; plan §5). The generator took 22.1 s at 2.0.566; the 11 s it has grown since is not yet explained.
    - **Pixel parity (AC19), the full run** (`cobolt-bench pixel-parity`, 18.9 min): 6 projects, 426 frame pairs, 425 identical; reads and dumps 27 of 27 equal; expected reads 18 of 18. The Cargo-built frames match their goldens, 407 of 408 (ParityControls has none).
      - PowerCloud moved: 72 of 72 frames, on the WebAssembly path.
      - The one frame apart, PowerCobolCraft `title-form/medium/05-BTN-QUIT-onClick` (67 pixels), was drawn while NIST ran beside it; its Cargo-built frame missed its golden the same way. Run again with the machine to itself (2.0.591): 66 of 66 identical, 66 of 66 goldens.
    - **The corpus** (`nist_runner_run compare`): 62 programs, 34 compiled, 28 refused with the construct named; 25 of 34 identical.
      - 7 `fileiot*` programs print the same lines under both engines and reach the 20 s limit under both.
      - 2 differ only by the interpreter's WARN line.
    - The release runner is 68.6 MiB (71 923 792 bytes).

## Phase M4 — SQL and HTTP

- [x] **T41 — `EXEC SQL`, the SQL object and the SqlDatabase control** (R10, R11)
  - Files: the `EXEC SQL` binding, extracted from `cobolt-runtime` unchanged (C5); `sql_exec`, with the statements in a `prsql` section; the runner's `sql` capability.
  - Verify: runner twins of the interpreter's `EXEC SQL` and backend tests give equal results. `cargo test -p cobolt-runtime` green.
  - *Design, settled 2026-11-17 after a survey of the interpreter's SQL code:*
    - **Three parts, each in its own commit:**
      - **T41** proper: `EXEC SQL` in programs and handlers.
      - **T41b:** the six SQL built-ins (`OPEN-DB`, `EXEC-SQL`, `FETCH-ROW`, `NEXT-ROW`, `ROW-COUNT`, `CLOSE-DB`).
      - **T41c:** the SqlDatabase control, its Async mode included; `COBOL::SqlDatabase` was split from it as **T41d**.
    - **Who does the work:**
      - The runner runs every statement with the interpreter's own SQL code (`cobolt_runtime::esql`: the three backends, cursors, positioned updates, prepared statements, the catalog and the run unit). So the databases, the SQLSTATEs and the transactions are the same on both paths.
      - The module keeps the host variables. It hands the runner each input's value and stores what the runner hands back.
    - **The boundary is a value, not a record:** the interpreter's environment holds values, not the record's bytes (`display_bytes` renders a COMP-3 item as characters), so the module's memory cannot simply be copied into it.
      - An input crosses as the SQL value the interpreter reads from the item (`sql_read_item`):
        - a decimal for a numeric item, de-edited for a numeric-edited one;
        - a float;
        - otherwise the text without its trailing spaces;
        - NULL when its indicator is negative.
      - A target crosses back as the value already converted for its item, with its indicator.
    - **As built (2026-11-17), simpler than the extraction first planned:** no SQL code moves. The runner keeps a small interpreter per program and form instance, built from the program's declarations (its data and cursors, without its procedure), and runs each statement with the interpreter's own `exec_sql`. `sql_convert`, `sql_read_item` and `sql_set_status` therefore stay exactly as they are.
      - Before the statement the module sends the value of every **cell** (each elementary item the statement may read or write: host variables and the leaves of host structures, indicators, and the status items the program can see), in the form the interpreter holds it: a decimal, a float, or bytes.
      - The runner sets them in that interpreter, runs the statement, and answers the `WHENEVER` condition and the cells whose value changed. The module stores those and branches.
      - `cobolt-runtime` gains three public doors and nothing else: `exec_sql_outcome` (the statement's condition, with `WHENEVER` left to the caller; `exec_sql` now calls it), `sql_end` made public, and `sql_instance`/`set_sql_instance`, so the programs of one form instance share its prepared statements as the form's single interpreter does.
      - **Handlers:** a form's handlers are programs contained in the form's program. Each handler's declarations carry the `GLOBAL` records of the programs that contain it (the nearest first) and their cursors, as the interpreter sees them. A host variable in a containing program's file record still takes the Cargo path.
    - **ABI A41:**
      - The capability `sql`, with one `prn` import, `sql_exec`.
      - A `prsql` section holding each program's declarations, each statement as the parser left it (`cobolt_ast::sql::ExecSql`, which serializes), and its cells.
      - The interpreter sets the status items (SQLSTATE, SQLCODE, SQLMSG, the SQLCA fields), which cross back as cells; the generated code follows `WHENEVER`.
    - **The run unit:**
      - One per run, shared by every window, as the form host shares one.
      - The main program or main form ends it: a commit after a normal end, a rollback otherwise.
      - The catalog is the built program's `sql-connections.toml`, read as the Cargo-built program reads it.
    - **Left on the Cargo path:** `DESCRIBE` and the SQLDA, which need pointers (T45).
  - *Done 2026-11-17 (2.0.593):*
    - **Programs** (`cobolt-runner` `esql`): 10 programs of the interpreter's own embedded-SQL suite (`tests/cobol/esql`), each built on the WebAssembly path and run by the runner, print the same lines as under `rcrun run`, and every one reports no failure: 122 statements, 61 of 61 checks. `ac11_dynamic.cbl` is sent down the Cargo path, the descriptor area named among its reasons.
    - **A form** (`cobolt-cli` `runner_sql_form`): 8 handlers of one form (CONNECT, CREATE, INSERT from a TextBox, COUNT and SUM INTO, a cursor the form's WORKING-STORAGE declares, PREPARE in one handler and EXECUTE in another, a failing DELETE), two scripts, the second after the first has ended: 5 dumps equal, 5 pictures with 0 differing pixels of 1 344 000, 19 of 19 checks.
    - **The deployment** (`cobolt-compiler` `test_esql_build`, the new ungated twin `a_webassembly_built_program_reads_its_sql_connections_file`): the Build takes the WebAssembly path in 10.3 s; the `sql-connections.toml` beside the program, a missing database (08001), the edited file, the commit at `STOP RUN`, the rollback after a runtime error, `CRM_SQL_SALES_URL`, and a password in the file refused (28000) give the answers the gated Cargo-built test asserts; 15 of 15 checks. Its runtime error is a `CALL` of a program that is not there: `super::` sends a program with no form down the Cargo path.
    - **The module corpus:** a new fixture, `sql`, and the entry 2.0.593 (console, ledger, sql; 189 824 bytes compressed of 262 144), each run alone on this runner with its output identical.
    - **Unchanged:** `cobolt-runtime`'s SQL tests (9 test binaries, 30 tests passed, 18 ignored for want of a MySQL server or the samples) and its library tests (576); the whole `cobolt-runtime` suite is not run on this machine. The sweeps: `cobolt-wasm` 36 test binaries, 136 passed; `cobolt-runner` 8, 14 passed; the runner form tests of `cobolt-cli` 18 of 18; `cobolt-wasm-abi` and `cobolt-wasm-rt` 53 passed; 0 failed. The System KB is rebuilt for the native column of `EXEC SQL`, and its test passes.
    - **AC16, on purpose:** the routines `rt.wasm` gained are linked into every module, so the PowerAnalyticsWeb pin is re-recorded (the manifest's bytes unchanged) and the player's generated fixtures with them; the shipped player is rebuilt, its bytes unchanged.
    - **Reported, not changed (GOLDEN RULE #2):** the interpreter reads an indicator only when it follows its host variable with no space (`:HOST:IND`); `:HOST :IND` and `:HOST INDICATOR :IND` give SQLSTATE 42601 on both paths.
    - **On a live MySQL (2.0.598, 2026-11-17):** `cobolt-runner` `embedded_sql_matches_the_interpreter_on_mysql` points 9 of the programs' `CONNECT TO ':memory:'` at a MySQL 8.4 server (ignored without `PRC_TEST_MYSQL_URL`): both paths print the same lines, every tally FAIL 000; 27 of 27 checks. The named-connection program is left out (a module the test packages carries no application name for the override); the SqlDatabase twin on MySQL (2.0.597) covers named connections on a server.

- [x] **T41b — The SQL built-ins** (R10, R11; added 2026-11-17)
  - Do: `OPEN-DB`, `EXEC-SQL`, `FETCH-ROW`, `NEXT-ROW`, `ROW-COUNT` and `CLOSE-DB` on the runner, over the interpreter's `DbRegistry`.
  - Verify: runner twins of `test_sql.rs` give the interpreter's output.
  - *Done 2026-11-17 (2.0.594):*
    - **One implementation (C5):** the interpreter's six arms now call `DbRegistry::builtin`, which takes the built-in, its arguments and a way to open a connection, and answers the values for its receivers. The runner calls the same function, so a handle, a row, an error text and a count are the interpreter's on both paths. Its library tests (576) and SQL tests (9 test binaries, 30 passed, 18 ignored) are unchanged.
    - **ABI A42:** one more `prn` import in the `sql` capability, `sql_db(op, handle, number, text, buf)`. The runner answers each receiver as a number or a text, and the module stores it as the interpreter stores a value of that kind in that item. The runner keeps one set of connections per form instance (and one for a console program), shared by that instance's handlers and the procedures they `CALL`, as the form's single interpreter shares them. Closing the form closes them, and so does the end of the run.
    - **Programs** (`cobolt-runner` `sql_builtins`): the Database Runtime guide's example, a program that `CALL`s a procedure holding the connection, and an edge program (bad SQL, a column out of range, a closed handle, a floating-point receiver, a nested program) print the same 30 lines under the runner and under `rcrun run`; an `OPEN-DB` with too few arguments is refused by the generator, naming what it takes, so its Build takes the Cargo path; 21 of 21 checks.
    - **A form** (`cobolt-cli` `runner_sql_form`): two more handlers, `OPEN-DB` on `:memory:` and `ROW-COUNT` into a Label's caption. The form now has 10 handlers. Result: 7 pictures, 0 differing pixels of 1 881 600; 25 of 25 checks.
    - **The deployment** (`cobolt-compiler` `test_esql_build`): `OPEN-DB 'sql-connection:SALES'` reaches the connection the program's `sql-connections.toml` names, as the Cargo-built program does; 16 of 16 checks.
    - **The module corpus:** the `sql` fixture gains a paragraph of built-ins, and the entry 2.0.594 (console, ledger, sql; 194 599 bytes compressed of 262 144).
    - **Sweeps:** `cobolt-wasm` 36 test binaries, 136 passed; `cobolt-runner` 9, 15 passed; the runner tests of `cobolt-cli` 4, 19 passed; `cobolt-wasm-abi` 24 and `cobolt-wasm-rt` 31 passed (two new: trimming and decimal parsing as the interpreter does them); 0 failed. The System KB is rebuilt and its test passes.
    - **AC16, on purpose:** `rt.wasm` grew by 2 416 bytes (251 390 to 253 806; written without the formatting machinery that slicing text pulls in, which first cost 12.6 KB). The PowerAnalyticsWeb pin is re-recorded; its manifest moves too, because it records the linked routines' sizes. The player's generated fixtures move with it; the shipped player is rebuilt, its bytes unchanged.
    - **Reported, not changed (GOLDEN RULE #2):** in the interpreter, a `COMP-2` item with a `VALUE` clause behaves as a fixed-point item with the value's decimals (`VALUE 1.5`, then `MOVE 12.25` gives 12.2). Without a `VALUE`, it stays floating point, and the runner matches that. A separate fix.

- [x] **T41c — The SqlDatabase control** (R10, R11; added 2026-11-17)
  - Do: the methods, properties and events, including Async mode. The interpreter's method code is moved where both engines call it, unchanged.
  - Verify: runner twins of the interpreter's SqlDatabase tests; a form twin with a SqlDatabase on `:memory:`.
  - *Split, 2026-11-17 (decision, not in the task text):* `COBOL::SqlDatabase`, the program-level database, is not a control on a form: the parser makes `COBOL::SqlDatabase` one word, which the generator refuses as the Backend's (`SQL_DATABASE_OBJECT`), so serving it needs a lowering of its own, its own `prn` operation and its own state in the runner. It became **T41d**, so that this task's commit holds one thing.
  - *Done 2026-11-17 (2.0.595):*
    - **One implementation (C5):** the interpreter's SqlDatabase arms moved, unchanged, into `DbRegistry::control`. It takes the method, its arguments, a way to read the control's properties and a way to open a connection, and answers what the method writes (the properties in order, the `ConnectionDataItem` or `ResultSetDataItem` value, the events, the answer) or that the statement runs asynchronously. `DbRegistry::control_delivered` gives an asynchronous result's effects, and `async_op::timed_out` the timeout's text. The interpreter and the runner both call them.
    - **ABI A43:** no new command and no new import. A method on a SqlDatabase is an ordinary `CALL` on the control. The ABI host hands it to a `SqlPort` when it has one (the runner's), writes the properties, raises only the bound events and answers; a host with no port (a page) answers as before. `Cancel` asks the port and, when a statement was running, clears `Busy` and raises `onCancelled`. The runner runs the method on the connections of the window that calls, the same that window's SQL built-ins use (T41b), and runs an Async statement on a thread with the connection lent to it. Its result, or its timeout, comes back to the program thread as news, and the host carries it out.
    - **Build gates:** the capability `sql` is listed when a desktop form holds a SqlDatabase, and the runner then reads the program's `sql-connections.toml`. A SqlDatabase whose design sets `ConnectionDataItem` or `ResultSetDataItem`, and a handler that uses one of those members or `SetProperty` on a SqlDatabase, takes the Cargo path, because the interpreter looks those data items up by name at run time (`D_SQL_ITEMS`, M6).
    - **A form** (`cobolt-cli` `runner_sqldatabase_form`): three SqlDatabase controls (`:memory:` Sync, `:memory:` Async, `SqlConnection` SALES), 12 buttons, 9 event handlers; a pinned sync script and a real-time async script (busy answer, second statement ignored, `ResultCount`, `TimeoutMs` 50 and `onTimeout`, `Cancel()` and `onCancelled`, the connection serving again): 8 dumps equal, 8 pictures with 0 differing pixels of 2 880 000; 36 of 36 checks.
    - **The ABI host** (`cobolt-abi-host` `sql_tests`): 15 of 15 checks; its suite 274 passed.
    - **Sweeps:** `cobolt-runtime`'s library tests 576 passed and its 12 SQL and SqlDatabase test binaries 94 passed (18 ignored for want of a server); `cobolt-wasm` 36 test binaries, 136 passed; `cobolt-runner` 9, 15 passed; the other runner tests of `cobolt-cli` 4, 19 passed; `cobolt-abi-host` 274 passed; `cobolt-wasm-abi` and `cobolt-wasm-rt` 55 passed; `cobolt-compiler`'s library 261 passed and `desktop_wasm_build` 38 of 38; 0 failed. The System KB is rebuilt (4 413 records) and its test passes.
    - **Corrected on the way:** `desktop_wasm_build` was red since T38: its AC11 case expected a disk file in Common Code to be refused, and the WebAssembly path serves those since then. The case now uses a pointer (`SET … TO ADDRESS OF`), which stays on the Cargo path until M5. `rt.sources.hash` was stale since T41b (a test added to `cobolt-wasm-rt` after the blob was built): `rt.wasm` rebuilt with identical bytes, the hash corrected.
    - **AC16:** the PowerAnalyticsWeb pin and the player's generated fixtures are unchanged. The shipped player is rebuilt because its ABI host gained the port it never holds (1 414 bytes more, 1 291 in the threaded build).
    - **On a live MySQL** (8.4, a local container, 2026-11-17): the interpreter's `EXEC SQL` live tests for MySQL, 8 run against the server and passed (2 more need the samples folder), the 11 AC programs among them with FAIL 000. `DbRegistry`'s own round trip finds a multi-row `INSERT` counted as 0 rows, so `Execute` and `EXEC-SQL` answer 0 on MySQL on both paths: fixed separately (2.0.597).

    - **Reported, fixed separately (2.0.596):** the interpreter keeps a let-go statement's connection lent until the wait after the next handler, so that handler's first statement on it is refused as busy; the Guide says the connection serves again the moment the worker ends. The twin clicks an idle button before such a statement.

- [x] **T41d — `COBOL::SqlDatabase`** (R10, R11; split from T41c 2026-11-17)
  - Do: the program-level database's members on the runner, over `DbRegistry::control`, with its properties kept per program and form instance as the interpreter keeps them.
  - Verify: a runner twin of `test_sql_database_object.rs` gives the interpreter's output.
  - *Done 2026-11-17 (2.0.599):*
    - **ABI A44:** one new `prn` import of the capability `sql`, `sql_obj(op, member, member_len, text, text_len, buf, cap) -> i32`, with `op` a verb, a property read or a property write (`cobolt_wasm_abi::sql::obj`). It is served in every desktop module, a console program's included, since the object is on no form. A form-tree target in the ABI host was considered and rejected: a console program has no ABI host.
    - **The module:** one new routine, `rt_sql_obj`. It sends the member's name and the argument's or written value's text as a control's path argument and `SET` text are sent, and leaves the answer in a character temporary as a property read leaves its answer. The generator lowers `COBOL::SqlDatabase::Member` as a value, as a statement (`Native::Form` no longer refuses it outside a form) and as a receiving field, with the value methods after the member applied to its answer in the module.
    - **The runner:** `Builtins::object` keeps the object's properties per form instance, beside the connections the SQL built-ins and the SqlDatabase controls use, and runs each verb with `DbRegistry::control`; the events are dropped, as the interpreter drops them. The properties move with the connections when a starting form's instance becomes known, and go when its window closes.
    - **What takes the Cargo path, with its reason:** a write to `Mode` (`D_SQL_OBJECT_ASYNC`: an asynchronous statement's result would arrive between events with no handler to hear it); `ConnectionDataItem`, `ResultSetDataItem` and `SetProperty` (`D_SQL_ITEMS`); and any member other than the verbs (`Open`, `Execute`, `Exec`, `Query`, `BeginTransaction`, `Commit`, `Rollback`, `IsInTransaction`, `Fetch`, `FetchAll`, `ColumnNames`, `ColumnCount`, `ColumnName`, `Close`, `Cancel`, `IsBusy`) and the properties (`ConnectionString`, `SqlConnection`, `Mode`, `TimeoutMs`, `LastError`, `StatusCode`, `ResultCount`, `Busy`), a chain of two members, a value method after a member that receives a value, or `STRING … INTO` a property (`D_SQL_OBJECT_MEMBER`). A browser module still refuses the object as the Backend's.
    - **The twin** (`cobolt-runner` `sql_object`): the interpreter's three programs and one more (`SqlConnection` and `sql-connection:SALES`, a `STRING`-built statement, a verb in `IF` and as a statement, value methods after a verb and after a property, `ColumnName` of a data item, written properties read back, `IsBusy`, `Cancel`, a verb after `Close`) print the same lines under the runner and `rcrun run`, and those lines are what the interpreter's own test expects: 33 expectations; 6 refusals each name their reason; 59 of 59 checks. On a live MySQL (`…_on_mysql`, ignored without `PRC_TEST_MYSQL_URL`): the three programs, 9 of 9.
    - **The module corpus:** the `sql` fixture calls the object too, checked against `rcrun run`; entry 2.0.599 records it, so every later runner is checked for `sql_obj`.
    - **AC16:** the rebuilt routine blob moves 79 bytes of each browser fixture (routine order), at the same size and with the same imports; the PowerAnalyticsWeb pin and the player's generated fixtures are re-recorded. The shipped player's files are byte-identical; only its recorded source hash moved.
    - **Sweeps:** `cobolt-wasm` 136 passed in 36 test binaries; `cobolt-wasm-rt` and `cobolt-wasm-abi` 55 in 7; `cobolt-runner` 16 in 10 (2 ignored without a live MySQL); `cobolt-abi-host` 274 in 2; `cobolt-compiler` library 261 (1 ignored) and `desktop_wasm_build` 1; `cobolt-cli`'s five runner tests 20 (1 ignored); the module corpus 11 of 11 modules in 4 releases with identical output; the player's freshness test 6; 0 failed.

- [x] **T42 — HTTP and RestClient** (R10, R11)
  - Do: `http_call` suspends and resumes the module as `remote` does.
  - Verify: a runner test against a local stub server gives the interpreter's results. No reference script calls the network.
  - *The HTTP built-ins done 2026-11-18 (2.0.601); the RestClient control followed as T42b (2.0.602):*
    - **Decision, against the task text:** `http_call` **blocks** the program's thread until the answer comes, as the SQL statements do (T41), instead of suspending the module as `remote` does. Suspending would send the answer back through the application object, whose text is capped at 64 KiB, and a console program has no host loop to resume it, yet it needs the import too. The cost: while a call waits, the program's other windows run no handler (each form's interpreter has its own thread). Recorded as WASM-PARITY row 76. The RestClient control (T42b) is to run its requests on worker threads, as the SqlDatabase control's `Async` mode does, so a program that must stay responsive has that choice.
    - **ABI A45:** one `prn` import of the capability `http`, `http_call(op, a, a_len, b, b_len, buf, cap) -> i64` (`cobolt_wasm_abi::http`): the operation, the two texts sent, and the answer's text written at `buf`; the status code in bits 32 to 47 of the result and the text's whole length in bits 0 to 31. `TAIL` asks for the text's last `cap` bytes after its trailing spaces, for a `JUSTIFIED` receiver.
    - **The module:** one new routine, `rt_http`. A body received into an alphanumeric item or a group is written in place (so a 40 000-byte body needs no scratch area), any other receiver through `put_text`, and the status code through `put_number`. The generator lowers the seven built-ins and their `CALL "COBOL-…"` form in a `native` build; fewer arguments than a built-in takes are refused, naming what it takes. `support.rs` classes them `Native::Host(cap::HTTP)`.
    - **The runner:** `http::Clients` keeps one `cobolt_runtime::HttpClient` per form instance and one for a console program, moved and dropped with the window as the SQL connections are, and runs each call with it; the file of `HTTP-DOWNLOAD` is anchored as the interpreter anchors it.
    - **Found on the way, fixed first (2.0.600, on its own branch):** the interpreter stored a built-in's text received into a group in a slot nothing reads, so `FETCH-ROW` and `HTTP-GET` into a group left it as it was. The twin's one difference until then.
    - **The twin** (`cobolt-runner` `http_builtins`): two programs against a stub server on 127.0.0.1, under the runner and `rcrun run`: every verb and receiver, and the interpreter's own download test's cases. The same 37 lines, the same 42 requests and the same files on both paths; 3 refusals; 46 of 46 checks. No reference script calls the network.
    - **The module corpus:** the `console` fixture also sets and clears a header, has a download refused for its URL and a GET whose URL cannot be read, offline; entry 2.0.601 records it, so every later runner is checked for `http_call` (a fourth fixture would have passed the 256 KiB budget).
    - **AC16:** each browser fixture grows 171 bytes (one type, and the runtime's data), with the same imports and code size; the PowerAnalyticsWeb pin and the player's generated fixtures are re-recorded; the shipped player's files are byte-identical.
    - **Sweeps:** `cobolt-wasm` 137 passed in 36 test binaries; `cobolt-wasm-rt` and `cobolt-wasm-abi` 56 in 7; `cobolt-runner` 19 in 11 (2 ignored without a live MySQL); `cobolt-abi-host` 274 in 2; `cobolt-compiler` library 261 (1 ignored) and `desktop_wasm_build` 1 (39 of 39 checks); `cobolt-cli`'s five runner tests 20 (1 ignored). Three tests whose expectation T42 changes now say what is true: the desktop refusals (`HTTP-GET` compiles, with the capability `http`; one given two arguments is refused, naming what it takes; `MODEL-LIST` stands for a built-in still refused; 32 of 32), the build-path test (`MODEL-LIST` refused at its line; the HTTP program builds on the WebAssembly path) and the runner's refusal of a capability it lacks, which now asks for a name no release provides, since the runner now provides every capability the ABI names but `sort`.
  - *The RestClient control done 2026-11-18 (2.0.602, T42b), so the box is checked:*
    - **One set of rules (C5):** the RestClient's rules left the interpreter, unchanged, for `cobolt_runtime::rest_door`: whether a verb runs now or on its own thread (`Mode`), the timeout (`TimeoutMs`, else `TimeoutSeconds`), the address (`BaseURL` and the argument), the headers and credentials (`DefaultHeaders`, `AuthType`/`AuthToken`), the redirect and TLS policy, the request each of `Get`, `Post`, `Put`, `Delete`, `Call`, `SetTimeout`, `SetHeader` and `ClearHeaders` asks for, and what an answer, a failure or a timeout writes and raises. The interpreter's verb arms and helpers now call it; its REST tests are unchanged and pass (the library's 578, and the 21 of the five REST, HTTP and event test binaries).
    - **ABI A46:** no new command and no new import. A method on a RestClient is an ordinary `CALL` on the control; the ABI host hands it to a `RestPort` when it has one (the runner's), writes the properties, raises only the bound events and answers; a host with no port (a page) answers as before. `Cancel` asks the port and, when a request was running, clears `Busy` and raises `onCancelled`. The host's way of carrying out a port's effects is now one function for the SqlDatabase and the RestClient.
    - **The runner:** `rest::Port` sends with the HTTP client of the window that calls, the one its HTTP built-ins use (T42), so `SetHeader` on a RestClient reaches `COBOL::"HTTP-GET"`. A `Sync` request runs on the program's thread; any other on a thread of its own with a copy of the client, with a second thread waiting out its `TimeoutMs`, and comes back to the program thread as news, checked by generation, as the SqlDatabase's statements do. Row 76's way out is therefore real: a program that must stay responsive while it waits uses an asynchronous RestClient.
    - **Found on the way:** the runner never applied a project's named REST connection (`Configuration`) or a control's credential from the environment (`COBOLT_CONTROL_KEY_<FORM>__<CONTROL>`) to its forms, which the interpreter's seed does for every form; a form file never holds a credential, so an `AuthToken` would have been empty on the WebAssembly path. `cobolt_form_host::seeding::resolve_connections` is now public and the runner applies it to each form, its contained controls included, before the form opens.
    - **Build gates:** the RestClient is served (`desktop_control_gap`); the service controls still refused are the AI, search and sign-in ones (M6). A RestClient whose design sets `RequestDataItem`, `ResponseDataItem` or `StatusDataItem`, and a handler that uses one of those members or `SetProperty` on a RestClient, takes the Cargo path (`D_REST_ITEMS`, M6): the interpreter reads and writes those data items by name at run time.
    - **A form** (`cobolt-cli` `runner_restclient_form`): a stub server on 127.0.0.1 echoing the verb, the path, four headers and the body; four RestClients (Sync with Bearer and `DefaultHeaders`, asynchronous with APIKey and `DefaultMethod = post`, and one of each where nothing listens), 12 buttons, 16 event handlers; a pinned Sync script (`Get`, `Post`, `Call` with a lower-case verb and a `?query`, `Delete` of `BaseURL`, `SetHeader` shared with `HTTP-GET`, `ClearHeaders`, a 404, a transport failure, `SetTimeout`) and a real-time Async script (nothing answered at once and busy, a second request ignored, `onComplete` with `ResponseBody`/`StatusCode`/`IsBusy`, `Call` with `DefaultMethod`, `TimeoutMs` 300 and `onTimeout` with the late answer dropped, `Cancel()` and `onCancelled`, `onError` with `StatusCode` 0): 8 dumps equal, 8 pictures with 0 differing pixels of 3 404 800; 35 of 35 checks.
    - **The ABI host** (`cobolt-abi-host` `rest_tests`): 2 tests, 16 checks: the port asked with the control, the method and the arguments; the properties written; only bound events raised; an asynchronous start, `IsBusy`, delivered endings; `Cancel` idle and running; release; no port.
    - **AC16:** a browser module is unchanged (the PowerAnalyticsWeb pin holds). The shipped player is rebuilt because its ABI host gained the port it never holds (1 287 bytes more, 1 217 in the threaded build); its freshness test passes.
    - **Sweeps:** `cobolt-runtime`: the library 578 passed, 3 ignored; the REST, HTTP and event targets 21. `cobolt-abi-host` 276. `cobolt-wasm` 137 in 36 targets; `cobolt-wasm-rt` and `cobolt-wasm-abi` 56. `cobolt-form-host` 193. `cobolt-compiler`: the library 261 (1 ignored), `desktop_wasm_build` and `web_remote_lowering` 9. The six `cobolt-cli` runner twins 21 (1 ignored, the live MySQL one). `desktop_wasm_unchanged` 1; `prebuilt_chunked_kb_matches_the_published_documentation` 1. `cobolt-runner` 18 passed, 1 failed, 2 ignored: T42's `http_builtins` failed in 2 of 5 runs. In the failing runs the two paths sent the headers set with `HTTP-SET-HEADER` in different orders: the HTTP built-ins have always sent them in a hash map's order, which changes from run to run. Not this change's; fixed on its own in 2.0.603.

- [x] **T43 — M4 close**
  - Do: the milestone close (preamble).
  - *Closed 2026-11-18 (2.0.605), run one after another. Pixel parity and the Build times were measured at `fe7558bc` (2.0.603) and had the machine to themselves. NIST and the corpus used binaries built 25 minutes later, while the fix of 2.0.604 was being written (it changes only programs with national data, which no NIST program has), and ran while other crates were built and tested beside them, which changes their times but not their counts:*
    - **Pixel parity (AC19), the full run** (`cobolt-bench pixel-parity`, 15.4 min): 6 projects, all 6 on the WebAssembly path; 426 frame pairs, **426 identical, 0 differing pixels**. Reads and dumps 27 of 27 equal; expected reads 18 of 18. The Cargo-built frames match their goldens, 408 of 408 (ParityControls has none).
      - ParityControls 18 of 18 frames, PowerCallOfDuty 12 of 12, PowerCloud 72 of 72, PowerCobolCraft 66 of 66, PowerGraphics 120 of 120, PowerSpatial 138 of 138.
    - **The reference projects' paths** (`cobolt-bench build-goal`, 5 Builds each, a fresh build folder each, 5.0 min): all six take the WebAssembly path, 30 of 30 Builds.
      - PowerCallOfDuty 0.4 s, PowerCloud 0.5 s, PowerCobolCraft 0.8 s, PowerGraphics 0.6 s, PowerSpatial 0.5 s.
      - The ten-times project (90 forms, 66 550 lines, 10 indexed files): 57.9 s (front end 29.5 s, generator 17.6 s, peak 8 314 MB). It is over the 30 s goal, which is reported, not enforced (D24; plan §5). The generator took 33.3 s at the M3 close and 17.6 s now.
      - None of the reference projects uses SQL, HTTP or a RestClient, so M4 moved none of them; they had all moved at M3.
    - **NIST through the runner (AC9), on both axes:**

      | Module | Compiled (of in scope) | Ran clean, 0 failures | PASS markers | Census |
      |---|---|---|---|---|
      | NC (Nucleus) | 95 of 95 | 95 | 4 615 | 1 215 s |
      | SQ (Sequential I/O) | 85 of 85 | 85 | 624 | 1 061 s |
      | IX (Indexed I/O) | 41 of 41 | 41 | 574 | 644 s |
      | RL (Relative I/O) | 34 of 34 | 34 | 354 | 546 s |
      | ST (Sort-Merge) | 39 of 39 | 39 | 735 | 734 s |
      | IF (Intrinsic Functions) | 45 of 45 | 45 | 841 | 510 s |
      | IC (Inter-Program Communication) | 25 of 25 | 25 | 309 | 295 s |
      | SM (Source Text Manipulation) | 16 of 16 | 16 | 311 | 233 s |

      - 380 of 380 programs, 8 363 PASS markers, 0 FAIL. NC and SQ match the M3 close exactly (4 615 and 624). The census ran while other crates were built and tested beside it: six programs (NC124A, NC236A, NC237A, NC252A, SQ102A, SQ117A) passed the harness's 20 s limit there, and each ran clean when run again alone (174, 11, 14, 78, 12 and 9 PASS), with the same binaries.
      - Run by the harness (`nist_conformance run`) with `RCRUN` set to `nist_runner_run`, which builds each program on the WebAssembly path and runs it with `prc-runner`; no line was refused.

    - **The corpus** (`nist_runner_run compare`): 62 programs, 45 compiled, 17 refused with the construct named (34 and 28 at the M3 close); 34 of 45 identical. 7 `fileiot*` programs print the same lines under both engines and reach the 20 s limit under both. The other 4 differ only by the interpreter's own log lines on standard output (WASM-PARITY row 54) and, in two, by the times they measure: run again by hand, `call/test-call-exception` (20 lines), `call/test-call-returning` (57), `media/test-image-builtins` (28) and `esql/ac8_units_of_work` are otherwise line for line the same. `ac8_units_of_work` reports one FAIL on both engines, as its `@DIR@` folder is not filled in by the corpus; the runner's own `esql` test fills it in and passes.
    - The release runner is 73.6 MiB (77 176 496 bytes); it was 68.6 MiB (71 923 792 bytes) at the M3 close. M4 added the runner's SQL and HTTP services; their share of the 5.0 MiB was not measured.
    - **The list of what still takes the Cargo path:** the Build's documentation (`cobolt-compiler`, "The Build" in the subset document, in the System KB) no longer names files, `EXEC SQL` or HTTP, which M3 and M4 moved. The Developer's Guide's copy is owed at the merge into `main`, as at M2 and M3.
    - **Relabelled:** the six `BINDING-…` built-ins, and a control bound to a SQL query or a REST endpoint, said they waited for M4. They drive a Data Binding, which moves with the other bound controls, so they now name M6, as the indexed-file binding has since T38.
    - **Found:** a Desktop program with national or UTF-8 data went to the WebAssembly path, where the module holds such data as plain bytes (WASM-PARITY 17), and gave other answers than the interpreter. Fixed on its own in 2.0.604: it takes the Cargo path until T44.

## Phase M5 — National and UTF-8 data, and the rest of the table

- [x] **T44 — National and UTF-8 data, computed in the module** (R11; C4)
  - Files: the national code extracted from `cobolt-runtime` unchanged; its lowering in `cobolt-wasm` and in `cobolt-wasm-rt`; the blob rebuilt with `cargo run -p cobolt-wasm --example build_rt`.
  - Verify: twins of the interpreter's national tests give identical bytes. `rt_blob` green.
  - *Done 2026-11-18 (2.0.606):*
    - **Decision, against the task text:** the interpreter's national rules are **mirrored**, not extracted unchanged. The module's runtime (`cobolt-wasm-rt`) is built for `wasm32` with no allocator. The interpreter's `national.rs` builds `String`s and `Vec`s at every step (`fit_class`, `class_image`, `class_text`). `cobolt_wasm_rt::nat` follows the same rules over borrowed bytes and character iterators. The twins below are the proof that the two give the same characters and the same bytes; the mirror names each interpreter function it follows.
    - **Decision, the scope:** a desktop module computes national and UTF-8 data where the reference projects and the interpreter's own tests use it:
      - `MOVE` (into and out of such an item);
      - `DISPLAY`;
      - comparisons (`IF`, `EVALUATE`, `SEARCH`);
      - `INITIALIZE` with `REPLACING NATIONAL`/`UTF-8 DATA`;
      - `FUNCTION LENGTH`, `BYTE-LENGTH`, `LENGTH-AN`;
      - `VALUE` clauses.
      Every other use is refused at its line, naming the item, and takes the Cargo path (`D_TEXT_USE`, M5): reference modification, `INSPECT`, `STRING`, `UNSTRING`, a function's argument, `ACCEPT`, `SET`, arithmetic, a `CALL` parameter, a control's property, an SQL host variable, a `SORT` key, a condition-name, a number moved in or out, a JSON group holding such a field. So is any such item in a program with a `PROGRAM COLLATING SEQUENCE` (`D_TEXT_COLLATING`), since the module compares code points only. A `LINE SEQUENTIAL` record holding one stays refused (`D_LINE_TEXT`, T39). The refusals are not entries of the native column: they are uses of data, not statements. T45's "every other `Cargo(_)` entry" does not cover them; they remain for a later task.
    - **The module:** two descriptor kinds, `K_NATIONAL16` (12; `aux` = characters) and `K_UTF8` (13; `aux` = characters, 0 for `BYTE-LENGTH`), and the literal flag `F_CLASSED` for `N"…"`/`U"…"`. Only a desktop module's layout describes items with them (`Layout::native`); a browser module's is unchanged.
      - `nat::write` is `class_image(fit_class(…))`: whole characters, and `JUSTIFIED RIGHT` keeps the rightmost significant ones.
      - `nat::relation` is `compare_values` for classed operands: by code point, padded with spaces, with the `TRUE`/`FALSE` word gate.
      - `nat::store` and `nat::into_alnum` are the moves into such an item and from one into an alphanumeric item.
      - `MOVE`, `INITIALIZE`, `DISPLAY` and the comparisons route to them by kind.
      - The image of a `VALUE` follows the interpreter's reading of the clause: a nonnumeric literal is text, a figurative constant fills the class, any other literal is no text, and `JUSTIFIED` applies.
    - **The generator:** `Lower::text_ok` admits a classed operand once, for the operand of a statement that computes such data. It is set by `text_operand` only for a direct reference, after the statement's pairs are checked (`check_text_move`, `check_text_compare`, `text_replacement`). The funnel `item_operand_bounded` refuses a classed item reached any other way, so a use nobody admitted takes the Cargo path instead of computing bytes. `LENGTH` and `BYTE-LENGTH` of a classed item are constants of its description.
    - **The twins** (`cobolt-wasm` `native_national`): 16 cases, each the interpreter's own national test or a rule it relies on: VALUE with JUSTIFIED and the figurative constants, characters that do not fit whole, EVALUATE and SEARCH, the boolean words, groups, edited and multiple receivers, a nested program. Each runs under the interpreter and as a desktop module in both memory layouts. The 94 DISPLAY lines and the 860 bytes of the item images are identical. `native_desktop_refusals` now checks that the four declarations of 2.0.604 compile when moved, shown, compared, initialised and measured, and that seven other uses and a collating sequence are refused at their line (13 of 13).
    - **AC16:** `rt.wasm` grew by 9 244 bytes (255 717 to 264 961): the national routines. The move, compare and display routines reach them by kind, so they are linked into every module, a browser module's too, where no item is of either kind: each of the player's generated fixtures grows by 8 355 to 8 784 bytes (`program.wasm` 114 765 to 123 524). The PowerAnalyticsWeb pin is re-recorded (its manifest moves too, as it records the linked routines' sizes) and then matches, 3 of 3; the player's generated fixtures are re-recorded. No input of the shipped player's fingerprint changed, so the player is not rebuilt.
    - **Sweeps:** `cobolt-wasm` with its runtime and the ABI: 199 passed in 44 targets (`cobolt-wasm` 139 in 37; `cobolt-wasm-rt` and `cobolt-wasm-abi` 60 in 7). `cobolt-runner`: 19 passed in 10 targets, 2 ignored. `cobolt-compiler` `--lib` and `desktop_wasm_build`: 262 passed, 1 ignored. The player: `abi_host` 28 of 28, `freshness` 6 of 6. `cobolt-web-tests` `desktop_wasm_unchanged`: 3 of 3 artefacts match the pin. The System KB freshness test passes (4 418 records). The runner twins of `cobolt-cli`: `runner_one_form`, `runner_restclient_form`, `runner_sql_form`, `runner_sqldatabase_form` and `runner_surfaces` pass; `runner_shell_hosts` passes 16 of 16, 15 run four at a time and `canvases_and_graphics_objects_draw_and_answer_alike` alone (16 of 16 checks, 1 536 000 pixels, 0 differing). Run all at once beside the other sweeps, with 16 builds of 327 s each, six of them reached the headless 60 s limit on both engines; alone, the graphics twin takes 55.7 s under the interpreter and 57.3 s under the runner, close to that limit.

- [x] **T45 — `SET` pointer, and every other `Cargo(_)` entry but `EXEC RUST`** (R11, R12; AC10)
  - Do: lower `SET` pointer (reclassified by F2) and each remaining entry the native column sends to Cargo.
  - Also (from T39c, 2.0.588): `COBOL::"FILE-STATUS"` with the file's name in a data item.
  - Verify: the AC10 test gains an assertion that `EXEC RUST` is the only `Cargo(_)` entry left. Each entry has a runner test equal to the interpreter.
  - *T45a done 2026-11-19 (2.0.614): `TRY … CATCH … FINALLY … END-TRY` and `THROW`. `SET` pointer (T45b, done the same day) and `FILE-STATUS` with the file's name in a data item (T45c) were still open, so the box stays empty:*
    - **What runs:** both statements, in a desktop module and in a browser one (the Web column already listed them as compiled; no generator lowered them, WASM-PARITY #58). The native column moves `THROW` and `TRY` from `Cargo(D_EXCEPTIONS)` to `Module`.
    - **How (`lower_try.rs`):**
      - a `TRY` pushes a frame on the `PERFORM` stack whose end word is `TRY_MARK` (−16, no block's index) and whose return word is its landing block;
      - a `THROW` keeps where its text is (`rt_throw_set`), drops frames down to the nearest `TRY` frame and lands there with the resume slot holding `THROWN`; with none open, `rt_throw_uncaught` ends the run with error 112 (1112 on the wire, ABI A47) and the interpreter's words;
      - every `TRY` takes a `THROW`, as the interpreter's does (`Err(UserException)` is matched whatever the clauses). `CATCH item` binds by an alphanumeric move (`rt_catch_bind`); `CATCH RUST-EXCEPTION` is not lowered, since a module has no `EXEC RUST`;
      - jumps the generator sees (`GO TO`, `EXIT PARAGRAPH`, `EXIT SECTION`, `EXIT PERFORM [CYCLE]`) drop the frames of the `TRY`s they leave and run their `FINALLY`s, innermost first (`escape`; `LoopCtx.tries` counts the `TRY`s open where an inline loop starts);
      - when some `TRY` of the module has a `FINALLY` (`has_finally`, found in phase A over every unit), `STOP RUN`, the main program's `GOBACK`/`EXIT PROGRAM` (`leave_open_tries_first`) and a called program's exit (`unit_exit_code`) stop at each open `TRY` frame, store the block to come back to in the resume slot and land there; the landing runs `FINALLY` and returns to it. A module with no `FINALLY` gets none of this code;
      - `FINALLY` is lowered once; every way into it stores, in the `TRY`'s own slot, the block it goes on to. The `CATCH` and `FINALLY` statements are lowered outside the `TRY`'s context, so a jump or a `THROW` from them skips that `FINALLY`, as the interpreter's `?` does.
    - **Refused at their line** (the Build takes the Cargo path): a `THROW` of anything but alphanumeric text (`D_THROW_OPERAND`); a `CATCH` item that is not alphanumeric or a group (`D_CATCH_ITEM`); inside a `TRY`, an altered `GO TO`, `GO TO … DEPENDING ON` and `NEXT SENTENCE` (`D_TRY_JUMP`).
    - **Differences recorded:** WASM-PARITY #77 (a routine error inside a `TRY` ends the module's run before `FINALLY`), #78 (an uncaught message over 231 bytes is cut in the error text) and #79 (found while measuring: the interpreter drops a sender's trailing spaces before a `JUSTIFIED RIGHT` store; an interpreter defect, for the operator).
    - **Tests:** `native_exceptions` 21 of 21 programs in 63 module runs (204 DISPLAY lines, 3 run-time errors identical), 8 of 8 refusals, #77 measured. The generator, routines and ABI: 202 passed in 45 targets. AC10: 34 in the module, 42 through the runner, 13 in a form's program, 47 on the Cargo path. AC16: every module is 28 bytes larger (the routines' state); the player's fixtures and the PowerAnalyticsWeb pin are re-recorded, the pin having first matched with the previous routines.
    - **Decision (AC10):** "`EXEC RUST` is the only `Cargo(_)` entry left" cannot hold at M5: the M6 entries (the non-visual controls, Data Binding, media, remote verbs) stay `Cargo(_)` until T47 to T51. The assertion moves to T51, the M6 close.
  - *T45b done 2026-11-19 (2.0.615): `USAGE POINTER`, `SET ptr … TO ADDRESS OF item`, `SET ADDRESS OF item TO {ADDRESS OF item | ptr | NULL}`. `FILE-STATUS` with the file's name in a data item (T45c) is still open, so the box stays empty:*
    - **What runs:** the interpreter's pointer test (`cobolt-runtime/tests/test_pointers.rs`) and the rules it relies on, in a desktop module. The native column moves `SET (pointer)` from `Cargo(D_POINTERS)` to `Module`; a browser module still refuses it (`BackendOnly`).
    - **How (`lower_ptr.rs`, `cobolt-wasm-rt` `ptr.rs`):**
      - the interpreter's pointer is a number, 1, 2, … in the order items are first named in `ADDRESS OF`, and its `SET ADDRESS OF` aliases a name to another. The module keeps the number in a table the generator lays out (it knows every item whose address the program takes): `[count][address × k][number × k]`, zero until written;
      - `SET ADDRESS OF` stores the address in the LINKAGE item's slot (`rt_ptr_point`, `rt_ptr_bind`, `rt_ptr_unbind`); the two readings agree when the LINKAGE item and every item whose address can reach it are described alike, which phase A checks;
      - `SET ptr TO NULL` and `SET ptr TO ptr-2` are `MOVE`s for both engines (the parser makes them so). Two changes in the routines make them agree: `NULL` against a number is the number 0 (`cmp.rs`), and a pointer's `DISPLAY` is its number without padding (`AUX_POINTER`, `disp.rs`).
    - **Refused at their line** (the Build takes the Cargo path): pointers in a called program (`D_PTR_UNIT`); `SET ADDRESS OF` anything but an elementary LINKAGE 01/77 with a slot of its own (`D_PTR_BASED`); `ADDRESS OF` a LINKAGE item, a table's item, a subscript or a part, a 66 or 88 (`D_PTR_ITEM`); a pointer that is not a `USAGE POINTER` item outside LINKAGE (`D_PTR_POINTER`); an item that can reach a LINKAGE item described otherwise (`D_PTR_SHAPE`).
    - **Differences recorded:** WASM-PARITY #80 (a pointer never set displays as `0`, not nothing), #81 (a LINKAGE item with no address is error 1107, not its own storage) and #82 (found while measuring: a `USAGE INDEX` item displays nine digits in every module; a fix, queued).
    - **Tests:** `native_pointers` 5 of 5 programs in 10 module runs (40 DISPLAY lines identical), 5 of 5 refusals, #80 and #81 measured. The generator, routines and ABI: 204 passed in 46 targets. AC10: 35 in the module, 42 through the runner, 13 in a form's program, 46 on the Cargo path. AC16: every module is 59 to 79 bytes larger (the comparison and `DISPLAY` checks); the player's fixtures and the PowerAnalyticsWeb pin are re-recorded, the pin having first matched with the previous routines.
  - *T45c done 2026-11-19 (2.0.616): `COBOL::"FILE-STATUS"` with the file's name in a data item. With T45a and T45b, every entry T45 names now runs in a desktop module; the box is ticked below, after the runner twins:*
    - **What runs:** the name read at run time from an item, a group, a group's field, a table element or a part of an item, trimmed of the spaces around it and in any case, as the interpreter reads it (`last_file_status` by the name in capitals). A name that is no file of the module, and a blank one, answer `00`.
    - **How (`lower_form.rs` `file_status_named`, `cobolt-wasm-rt` `nfile.rs` `rt_nf_last_status_named`):** the generator interns a table of every file name of the module (`[count][address, length, place]…`), each with the place its status is kept, chosen by `status_place` as for a literal of that name (the literal form now shares it). The routine trims the item's text, compares it in capitals and answers through `rt_nf_last_status`; nothing crosses to the host.
    - **Refused at its line** (the Build takes the Cargo path): a name in a numeric item, or in anything but alphanumeric text.
    - **Tests:** `native_desktop_refusals` 13 of 13 and 33 of 33 checks (the name in an item now compiles; in a `PIC 9(8)` item it is refused); the runner twin `files_of_every_organisation_match_the_interpreter` gains `file status by a name in an item`: 7 lines, 2 files equal; 250 of 250 checks. The generator, routines and ABI: 204 passed in 46 targets with the routine's first build, whose one failure (the player's fixture) is the next point. AC16: the first build indexed a slice, which put a 48-byte panic location into the routines' data, which every module carries; with `get` instead, `rt.wasm` grows by 540 bytes (266 625 to 267 165), all of it the routine, and the player's 8 fixtures and the PowerAnalyticsWeb pin match as committed.
  - *T45 done 2026-11-19 (2.0.616), with T45a, T45b and T45c:*
    - **Runner twins, one per entry:** `TRY`/`THROW` and `SET` pointer in the new `cobolt-runner` `exceptions_and_pointers_match_the_interpreter` (4 of 4 programs identical to `rcrun run`, byte for byte and in exit code, 61 checks); `FILE-STATUS` with the name in a data item in `files_of_every_organisation_match_the_interpreter`. A `THROW` nobody catches is left out of the twin: the runner words a run-time error "Runtime error 1112 at line …" and `rcrun run` "Runtime error: …", a difference older than T45; the generator's own test compares that error's text with the interpreter's.
    - **AC10:** the assertion that `EXEC RUST` is the only `Cargo(_)` entry left moved to T51 (decision recorded under T45a).

- [x] **T46 — M5 close**
  - Do: the milestone close (preamble).
  - *Closed 2026-11-19 (2.0.617), run one after another at normal priority, at `c97bf9d2` (2.0.616) with this task's relabels in the tree (they change only the texts of four refusals). The operator was using the machine during the NIST census, the corpus and the generator comparison (load average 3 to 6 during the last), which changes their times but not their counts:*
    - **Pixel parity (AC19), the full run** (`cobolt-bench pixel-parity`, 15.6 min): 6 projects, all 6 on the WebAssembly path; 426 frame pairs, **426 identical, 0 differing pixels**. Reads and dumps 27 of 27 equal; expected reads 18 of 18. The Cargo-built frames match their goldens, 408 of 408 (ParityControls has none).
      - ParityControls 18 of 18 frames, PowerCallOfDuty 12 of 12, PowerCloud 72 of 72, PowerCobolCraft 66 of 66, PowerGraphics 120 of 120, PowerSpatial 138 of 138.
    - **The reference projects' paths** (`cobolt-bench build-goal`, 5 Builds each, a fresh build folder each, 5.6 min): all six take the WebAssembly path, 30 of 30 Builds.
      - PowerCallOfDuty 0.4 s, PowerCloud 0.5 s, PowerCobolCraft 0.7 s, PowerGraphics 0.6 s, PowerSpatial 0.4 s.
      - The ten-times project (90 forms, 66 550 lines, 10 indexed files; the same tree hash as at M4): 61.6 s (front end 29.3 s, generator 31.9 s, peak 7 198 MB). It is over the 30 s goal, which is reported, not enforced (D24; plan §5).
      - **The generator's 31.9 s, against 17.6 s at the M4 close, is the machine, not the code.** The generator alone (`build-goal --runs 0`) on the same ten-times project, M4's commit `8b272b03` and this one taken in turn: M4 26.7 s and 25.1 s, this one 26.5 s and 27.1 s. Two runs of this one just before gave 32.2 s and 23.3 s. The spread within one commit is wider than any difference between the two.
      - None of the five reference projects uses national or UTF-8 data, `TRY`, `THROW`, pointers or an SQL descriptor (their forms, Common Code and indexed files searched), so M5 moved none of them; they had all moved at M3.
    - **NIST through the runner (AC9), on both axes:**

      | Module | Compiled (of in scope) | Ran clean, 0 failures | PASS markers | Census |
      |---|---|---|---|---|
      | NC (Nucleus) | 95 of 95 | 95 | 4 615 | 1 120 s |
      | SQ (Sequential I/O) | 85 of 85 | 85 | 624 | 993 s |
      | IX (Indexed I/O) | 41 of 41 | 41 | 574 | 646 s |
      | RL (Relative I/O) | 34 of 34 | 34 | 354 | 540 s |
      | ST (Sort-Merge) | 39 of 39 | 39 | 735 | 716 s |
      | IF (Intrinsic Functions) | 45 of 45 | 45 | 841 | 514 s |
      | IC (Inter-Program Communication) | 25 of 25 | 25 | 309 | 298 s |
      | SM (Source Text Manipulation) | 16 of 16 | 16 | 311 | 235 s |

      - 380 of 380 programs, 8 363 PASS markers, 0 FAIL, none over the harness's 20 s limit. Every module's PASS count equals the M4 close's.
      - Run by the harness (`nist_conformance run`) with `RCRUN` set to `nist_runner_run`, which builds each program on the WebAssembly path and runs it with `prc-runner`; no line was refused.
    - **The corpus** (`nist_runner_run compare`, 14.3 min): 64 programs, 47 compiled, 17 refused with the construct named (62, 45 and 17 at the M4 close); **36 of 47 identical** (interpreter 153.2 s, runner 229.0 s).
      - The two programs new since M4, `call/test-goback-in-perform.cbl` and `esql/status_in_called_program.cbl` (they came with the fixes 2.0.607 and 2.0.608), compile and are identical.
      - The 17 refused are the M4 close's 17 programs. T45 moved `TRY`, `THROW` and pointers, yet none of the 17 left: `esql/ac11_dynamic.cbl` now stops at `SET SQLDA-DATA (1) TO ADDRESS OF WS-CODE`, a pointer in the descriptor's table, and `esql/ac18_mixed.cbl` at the descriptor area itself; both wait for T50a's descriptor (below).
      - The 11 that differ are the M4 close's 11: 7 `fileiot*` programs print the same lines under both engines and reach the 20 s limit under both; `call/test-call-exception`, `call/test-call-returning`, `esql/ac8_units_of_work` and `media/test-image-builtins` differ by the interpreter's own log lines on standard output (WASM-PARITY row 54).
    - The release runner is 73.6 MiB (77 176 944 bytes), 448 bytes more than at the M4 close.
    - **The list of what still takes the Cargo path:** the Build's documentation (`cobolt-compiler`, "The Build" in the subset document, and the System KB, rebuilt) still said pointers, `TRY` and `THROW` take the Cargo path, which T45 moved; its statement table had been right since T45. It now names `TRY`, `CATCH`, `FINALLY`, `THROW` and pointers in the main program as built, and what M5 left: a pointer in a called program, an SQL descriptor area, and national or UTF-8 data in any use but those T44 computes. The Developer's Guide's copy is owed at the merge into `main`, as at M2, M3 and M4.
    - **Relabelled, and a task added:** four refusals said they waited for M5 and are still refused after it: other uses of national or UTF-8 data (`D_TEXT_USE`), such data in a program with a `PROGRAM COLLATING SEQUENCE` (`D_TEXT_COLLATING`), national or UTF-8 text in a `LINE SEQUENTIAL` record (`D_LINE_TEXT`), and the SQL descriptor area (`D_DESCRIPTOR`). No task took them: they are uses of data, not entries of the native column that T45 went through. They now name M6, and **T50a** (new) holds them, so that R11's end state at T51 includes them. `native_desktop_refusals` checks the new texts, 2 of 2.
    - **Decision, the SQL descriptor area:** the interpreter fills a descriptor's entries by their names (`DESCRIBE`) and reaches the host variables through the pointers in its table (`SQLDA-DATA(i)`). A module's pointers are whole items outside any table (T45b), so the descriptor cannot be served by T45b's pointers; T50a gives it a design of its own.

## Phase M6 — The non-visual controls

Each task here moves its controls out of the `DesktopControl` fallback. It is verified by runner twins of the `rcrun` host tests for those controls, and by pixel parity on every reference project that uses them.

- [x] **T47 — AgentObject, KnowledgeBase and WebSearch** (R11)
  - Verify: twins of `model_list_hosts` and the agent and KB host tests, against local stubs.
  - *WebSearch done 2026-11-19 (2.0.618, "T47a"):*
    - **Why WebSearch came first:** it is a RestClient request whose address, headers and body come from the control's properties, so it rides on T42b's port with no new import. KnowledgeBase needs the runtime's `kb` feature in the runner and its own events, and AgentObject a tool loop, streaming and MCP: each is a step of its own.
    - **One implementation of the rules:** the interpreter's `Search` and its accessors moved into a module both hosts call, `cobolt_runtime::search_door`, with no change of behaviour (the 24 search unit tests and its own test, 20 checks). It decides:
      - the key: the control's `ApiKey`, else the project's;
      - the "not configured" refusal, with nothing sent;
      - the request, through `search_runtime`, with RestClient's `Mode`, `TimeoutMs` and backstop;
      - `onResultsReceived` before `onComplete`;
      - `ResultCount`, `TopTitle`, `TopSnippet`, `TopLink` and `GetResult(n)` over the last answer;
      - the masked lines `Verbose` narrates. The masking functions moved with it.
    - **What runs (ABI A48, no new import):**
      - The ABI host sends a WebSearch's methods down the RestClient's port, as for a `RestClient` (`rest_call`). The port's effects gained the lines to narrate, written where `DISPLAY` goes (`Effect::Display`).
      - The runner's port follows `search_door`. `Cancel`, `IsBusy` and `GetResult()` with no argument stay the host's own.
      - Before a form opens, the runner gives a WebSearch the project's search key from the environment, as the interpreter's seed does.
      - In the generator, `WebSearch` is served (`desktop_control_gap`). A WebSearch that names a data item takes the Cargo path with the RestClient's reason (`D_REST_ITEMS`, now worded for both).
    - **Tests:**
      - **The runner twin `the_websearch_control_agrees_on_both_engines`, against a local SearXNG stub:** 34 of 34 checks. 6 dumps are equal; 6 pictures, 2,553,600 pixels, 0 differing; 42 narrated lines are the same on both engines. It covers Sync and Async, `onResultsReceived` then `onComplete`, the accessors, a Google control with no key, `TimeoutMs`, `Cancel()` and a transport failure. Both engines run with the project's search key variable removed, so no run reaches a real provider.
      - `desktop_controls_the_runner_lacks_are_named`: 10 of 10, with a WebSearch served, a WebSearch naming a data item refused, and a KnowledgeBase still refused.
      - `subset_table`: control types 71 = 66 served + 5 on the Cargo path; 25 of 25 fixed points.
  - *KnowledgeBase done 2026-11-19 (2.0.619, "T47b"). AgentObject (T47c) is still open, so the box stays empty:*
    - **One implementation of the rules:** the interpreter's KnowledgeBase methods and the mapping of its workers' reports moved into a module both hosts call, `cobolt_runtime::kb_door`, with no change of behaviour (the interpreter's KnowledgeBase tests, `test_knowledge_base` 7 tests, `test_nonvisual_property_readers` 5, `test_class_c_catalogue` 2 and `test_model_list` 3, with and without the runtime's default features). It decides:
      - which methods answer at once (collections, documents, the last search's results), and what they write;
      - which start a background operation, one per control, and the refusals before one starts: one already running, a `ModelEntry` not handed over, or one with no key its API needs;
      - the worker's settings, read from the control's properties;
      - what each report raises (`onProgress`, then `onIndexed`, `onSearchComplete`, `onBusy` or `onError`) and the values it carries.
      The methods need the runtime's `kb` feature; the report mapping is compiled in every build, as the interpreter's drain is.
    - **What runs (ABI A49, a new capability `kb`, no new import):**
      - A desktop module with a KnowledgeBase requires `kb`, so a runner that does not provide it refuses to start and names it.
      - The ABI host routes a KnowledgeBase's methods to the runner's port first (`kb_call`), as the interpreter routes them by class: `Search`, `Refresh` and `Cancel` mean something else on other controls. A host with no port answers every method `"0"` with `LastError`, as an interpreter built without the Knowledge Base.
      - `Cancel` is the host's uniform one: the port lets a running operation go, `Busy` is cleared and `onCancelled` raised; the answer is `"1"`.
      - Each report becomes an event queued with its values (A38's `Work::Apply`), written as it leaves the queue, bound or not; a search's results travel with `onSearchComplete` as JSON rows and become the control's then.
      - The runner's port (`crates/cobolt-runner/src/kb.rs`) runs each operation on a thread of its own with the interpreter's workers; a report from an operation that was cancelled, or whose form closed, is dropped. A form that closes does not stop its operation, as under the interpreter.
      - The runner links the runtime's `kb` feature, without the model (no `candle` or `tokenizers` in its tree, checked with `cargo tree`). The release `prc-runner` is 79 869 776 bytes (76.2 MiB) for macOS arm64, against 77 176 944 before T47: the document importers and the index add 2 692 832 bytes (3.5 %) to every program built on the WebAssembly path.
    - **Decision (the built-in semantic model):** a project that sets `[rag] embedder = "builtin"` and has a KnowledgeBase takes the Cargo path (`kb_semantic_reason`, `D_KB_SEMANTIC`). The runner carries the Knowledge Base without the model: linking candle and tokenizers into the runner would make every program built on the WebAssembly path larger, for the few projects that use the model; `cobolt-kb` keeps the model behind a feature of its own because tokenizers' regex engine compiles C. This is a technical reason under the operator's rule of 2026-11-16 ("no project should rely on cargo build" unless there is one). A second runner that carries the model would remove it; that is left for after M6. Without the setting, an `Embedder` of `Builtin` searches lexically on both paths and `SearchModeReason` says why.
    - **Tests:**
      - **The runner twin `the_knowledgebase_control_agrees_on_both_engines`:** 26 of 26 checks. 6 dumps are equal; 6 pictures, 2 553 600 pixels, 0 differing. Two KnowledgeBase controls, 9 buttons and 14 event handlers cover the collection methods; `AddDocument` busying the control, and a second operation refused with `LastError`; `onProgress` and `onIndexed` reading their own values; `UpdateDocument`, `ImportDocument`, `ListDocuments`, `GetDocument`; a `Search` with a maximum of 2, whose `onSearchComplete` reads the best result first; `DeleteDocument`; `onError` with no collection; and `Refresh` then `Cancel()`, with `onCancelled` and the refresh's reports dropped. Each engine starts from an empty folder, in real time: the interpreter's run 54.5 s, the runner's 54.8 s. The `onError` check asks only that `LastError` names the missing collection: the message arrives wrapped as an invalid collection name on both engines (`kb_runtime`, since 1.70), a fix of its own.
      - **The ABI host's `kb_tests`:** 2 tests, 16 checks: a method's properties and answer, the reports' values queued and written on leaving, an unbound event skipped, the results handed back; `Cancel` running and idle, the general dispatch, a host with no Knowledge Base, an instance's end.
      - `desktop_controls_the_runner_lacks_are_named`: 13 of 13, with a KnowledgeBase served on its own and inside a Panel, an AgentObject beside it still refused, and `[rag] embedder = "builtin"` sending a KnowledgeBase to the Cargo path with the semantic model's reason (another embedder, or no KnowledgeBase, does not).
      - `subset_table`: control types 71 = 67 served + 4 on the Cargo path; 26 of 26 fixed points.
    - **Left for this task:** AgentObject (T47c), and the twins of `model_list_hosts` and the agent host tests.
  - *AgentObject, first part, done 2026-11-19 (2.0.621, "T47c"). Its file and Knowledge Base tools and the `model_list_hosts` twins are still open, so the box stays empty:*
    - **One implementation of the rules:** the interpreter's AgentObject conversation moved into a module both hosts call, `cobolt_runtime::agent_door`, with no change of behaviour. It holds:
      - the declared tools (`AddTool`, `AddToolParameter`, `RemoveTool`) and the methods the runner answers (`Ask`, `SetToolResult`, `SetPrompt`, `SetModel`, `GetResult()`);
      - the plain conversation and its continuations (`MaximumContinuations`), and the tool loop (`MaximumToolRounds`, `ToolProtocol`);
      - the streaming worker (`Job`), with the start limit and the silence limit, and what each report writes and raises (`onPartialReply`, `onResponse`, `onError`, `onToolCall`);
      - the lines `Verbose` narrates.
      The interpreter reaches it through `AgentSite` (`interpreter/agent_loop.rs`), which keeps its own tools (MCP files, Knowledge Base collections). Its tests pass unchanged: `test_agent_runtime` 9, `test_agent_tool_calling` 12, `test_async_rest` 5, `test_knowledge_base` 7, `test_mcp_tool_parity` 3, `test_model_list` 3, `test_model_list_file` 1, `test_registered_files` 8 (1 ignored), `test_nonvisual_property_readers` 5 (it now scans `agent_door.rs`), and the 15 queued-event unit tests.
    - **What runs (ABI A50, a new capability `agent`, no new import):**
      - A desktop module with an AgentObject requires `agent`.
      - The ABI host routes an AgentObject's methods to the runner's port (`agent_call`), after the KnowledgeBase's; `Cancel` is the host's uniform one. The narration goes where `DISPLAY` goes, the properties are written at once, and bound events are queued.
      - **`onToolCall` is followed past the queue** (`Work::ToolCall`): its handler runs, and once the instance is free again the port takes the result and sends the next round, before the program takes another event; with no handler bound it goes on at once. This is where the interpreter takes the handler's `SetToolResult`: at its next wait.
      - The runner's port (`crates/cobolt-runner/src/agent.rs`) sends each request on a thread of its own with a copy of the calling window's HTTP client, polls every 40 ms while one is out, and lets a request go at its time limits (`PendingOp::expired`, shared with the interpreter's sweep) with `onTimeout`. A form that closes ends its conversations; a report for a cancelled or ended one is dropped.
      - In the generator, `AgentObject` is served (`desktop_control_gap`).
    - **What still takes the Cargo path, and why:**
      - An AgentObject whose `ResponseDataItem` is set, or a program that uses that member or `SetProperty` on one (`D_AGENT_ITEMS`): the interpreter looks the data item up by name at run time.
      - A program that uses `AllowFile`, `DenyFile`, `RegisterFile`, `UnregisterFile`, `AllowKnowledgeBase` or `DenyKnowledgeBase` (`D_AGENT_TOOLS`): the tools the runtime answers itself (indexed files, Knowledge Base collections) are the second part of T47c.
    - **Tests:**
      - **The runner twin `the_agentobject_agrees_on_both_engines`, against a local stub model server:** 24 of 24 checks. 5 dumps are equal; 5 pictures, 1 672 000 pixels, 0 differing; the same 11 narrated lines, and the same 8 requests on the stub from both engines. Seven AgentObjects, 7 buttons and 15 event handlers cover a plain `Ask` with `Verbose`; `StreamReply` and `onPartialReply`; a continuation; a declared tool, `onToolCall` and `SetToolResult`; `Ask` then `Cancel()`, with the late answer dropped; `StartTimeoutSeconds` and `onTimeout`; and a refused connection with `onError`. The stub answers by what each request asks, so the order the requests arrive in does not matter. In real time: the interpreter's run 35.7 s, the runner's 43.4 s.
      - **The ABI host's `agent_tests`:** 2 tests, 29 checks: a method's narration, properties and answer; a piece and an answer written at once, only bound events queued; a stray report dropped; the general dispatch; `onToolCall` bound (after its handler returns, before the next event; a yielded handler still waits) and unbound (at once); `Cancel` in flight and idle; an instance's end; a host with no port.
      - `native_desktop_refusals`: 35 of 35 checks, with an AgentObject's `AddTool`, `Ask`, `SetToolResult` and `LastReply` compiling and 4 of 4 members refused at their line.
      - `desktop_controls_the_runner_lacks_are_named`: 14 of 14, with an AgentObject served beside a KnowledgeBase, the sign-in control beside them refused, and an AgentObject naming `ResponseDataItem` refused.
      - `subset_table`: control types 71 = 68 served + 3 on the Cargo path; 29 of 29 fixed points.
    - **Left for this task:** the AgentObject's file and Knowledge Base tools (T47c's second part), and the twins of `model_list_hosts`.
  - *AgentObject's indexed-file tools done 2026-11-19 (2.0.622, "T47c", second part, a). Its Knowledge Base tools and the `model_list_hosts` twins are still open, so the box stays empty:*
    - **One implementation of the rules:** what the interpreter did for `AllowFile`, `DenyFile`, `RegisterFile` and `UnregisterFile` moved, unchanged, into functions both hosts call: `mcp_tool::allow_file` (the description from the named `.cidx`, or the one found by the file's name under `indexed/`), `find_description`, `agent_tool_specs`, `agent_tool_answer` and `FileAccess::of_layout`; `registered_file::register_for_agent` (the five `Register…` properties and the memory limit). The record layout of a file declared in a form comes from its FD text through `files::layout_from_definition`, the same parse the `.cidx` import uses (`indexed_import::definition_record`).
    - **What runs (ABI A51, a new capability `agent.files`, no new import):**
      - A desktop module whose AgentObject uses one of the four file members requires `agent.files`.
      - `AllowFile("NAME" …)` carries two hidden arguments: the file's entry in the module's file table (`prfiles`, which holds the FD text and the keys of an INDEXED file) and its ASSIGN path, the data item's value when the ASSIGN names one. The runner rebuilds the layout and the primary key from that entry, anchors the path as the program's other files are, and offers `search_<file>` to the model; the search reads the file on disk, as the interpreter's does.
      - `DenyFile`, `RegisterFile` and `UnregisterFile` are answered by the runner's port with the shared functions; `RegisterFile` writes `RegisterResult`, `RegisterMessage`, `RegisteredName`, `RegisterFileBytes` and `RegisterLimitBytes`.
    - **What still takes the Cargo path, and why:**
      - `AllowFile` whose file name is not a literal (`D_AGENT_FILE_NAME`): the module must know at Build time which file's layout to carry.
      - `AllowFile` naming a file that is not INDEXED (`D_AGENT_FILE_ORG`): the module carries the layout of indexed files only.
      - `AllowKnowledgeBase` and `DenyKnowledgeBase` (`D_AGENT_TOOLS`): the second half of T47c's second part.
    - **Tests:**
      - **The runner twin `the_agentobject_file_tools_agree_on_both_engines`, against a local stub model server:** 14 of 14 checks. One form with an INDEXED file of 3 orders (its FD in the form, its `.cidx` under `indexed/`), 2 AgentObjects, 6 buttons and 9 event handlers cover `AllowFile` with its definition, found by the file's name, and naming no file of the program; a search the model asks for, which finds ACME's two orders; `RegisterFile` of the data under another name and of a missing file; an `Ask` offering both tools; `UnregisterFile` twice; `DenyFile` and an `Ask` offering none. Three dumps are equal, three pictures (684,000 pixels) have 0 differing, and the stub receives the same 4 requests from both engines. The project has an `assets/` folder, so the built program anchors `data/` and `indexed/` at the project, as Run Form does.
      - `native_desktop_refusals`: 38 of 38 checks, 57 programs compiled: the four file members compile and require `agent.files`, a module without them does not, and an `AllowFile` naming its file in a data item, or naming a LINE SEQUENTIAL file, is refused at its line.
      - `subset_table`: 71 control types = 68 served + 3 on the Cargo path, 29 of 29 fixed points; the file members are served, the Knowledge Base members are not.
    - **Left for this task:** the AgentObject's Knowledge Base tools, and the twins of `model_list_hosts`.
  - *AgentObject's Knowledge Base tools done 2026-11-19 (2.0.623, "T47c", second part, b). The `model_list_hosts` twins are still open, so the box stays empty:*
    - **One implementation of the rules:** what the interpreter did for `AllowKnowledgeBase` and `DenyKnowledgeBase` moved, unchanged, into `kb_door`, which both hosts call: `AgentKbTool` (one collection handed to one agent), `allow_for_agent`, `deny_for_agent`, `agent_tool_is`, `agent_tool_specs` and `agent_search`. Each host gives these functions its own way to read a KnowledgeBase's properties: the interpreter reads its controls, and the runner reads the form's (`AgentControl::knowledge_base`, `Host::knowledge_base_property`).
    - **What runs (ABI A52, a new capability `agent.kb`, no new import, no hidden argument):**
      - A desktop module whose AgentObject uses either member requires `agent.kb`. The KnowledgeBase is named as the program wrote it, by a literal or by a data item's value, and the runner finds it in the form.
      - The runner keeps one list per form instance and lets it go when the form closes. Each collection belongs to the agent it was handed to; the tool is `kb_<control>_<collection>`.
      - The model's call is answered with the KnowledgeBase's settings as they are at that moment. A lexical search answers at once. A search that needs the embedding server runs on a thread of its own, and its text returns as `agent_door::Report::ToolResult`, which `agent_door::reported` hands to `tool_worker_delivered`, where the interpreter's worker result already went.
    - **What still takes the Cargo path:** of the AgentObject's members, only an `AllowFile` that names its file in a data item (`D_AGENT_FILE_NAME`), or names a file that is not INDEXED (`D_AGENT_FILE_ORG`), as at 2.0.622. `D_AGENT_TOOLS` is gone.
    - **Tests:**
      - **The runner twin `the_agentobject_knowledge_base_tools_agree_on_both_engines`, against a local stub that serves both the model and the embedding server:** 14 of 14 checks. One form with two KnowledgeBases (KB-1 lexical, KB-2 with its embeddings from the stub), 2 AgentObjects, 5 buttons and 12 event handlers. It covers:
        - `AllowKnowledgeBase` of a control's own collection, of another collection, of a Label, and of a name no collection may have;
        - a request whose model searches both collections, one at once and one on a worker, and answers with both results;
        - the other agent's request, offering no collection;
        - `DenyKnowledgeBase` of KB-2, then a request offering KB-1's collection only.
        Three dumps are equal, three pictures (684,000 pixels) have 0 differing, and the stub receives the same 7 requests from both engines, 2 of them to the embedding server.
      - `native_desktop_refusals`: 39 of 39 checks, 57 programs compiled: a form whose AgentObject uses `AllowKnowledgeBase` (by a literal, with no collection, and by a data item) and `DenyKnowledgeBase` compiles and requires `agent` and `agent.kb`; a form whose AgentObject uses none of the members requires `agent` alone. The other test: 13 of 13.
      - `subset_table`: 9 of 9; 71 control types = 68 served + 3 on the Cargo path, 29 of 29 fixed points; 136 entries classified (53 statements, 83 built-ins), 0 faults; the Knowledge Base members are served.
    - **Left for this task:** the twins of `model_list_hosts`.
  - *The `model_list_hosts` twin done 2026-11-20 (2.0.624), with T48's model lists; T47 is done:*
    - **The twin:** `the_model_list_keys_and_providers_agree_on_both_engines` runs `model_list_hosts`' order (an entry beats the Configuration, which beats the agent's own settings) on both engines, with the built-ins that make the entry and its key: 18 of 18 checks (T48 below).
    - **T47's verification, all against local stubs:** the twins of the agent and Knowledge Base host tests are `runner_agent_form` (24 of 24 checks), `runner_agent_files_form` (14 of 14), `runner_agent_kb_form` (14 of 14), `runner_knowledgebase_form` (26 of 26) and `runner_websearch_form` (34 of 34); `model_list_hosts` itself still passes (1 passed; its built-binary test, a Debug Build on the Cargo path, stays ignored as by default).

- [x] **T48 — Maps, the Aws controls, model lists and MCP** (R11)
  - Verify: twins of `aws_hosts`, `partiql_hosts` and `powercloud_hosts`, against local stubs. No twin sends a request signed with the live keys some example `.cfrm` files carry.
  - *AWS done 2026-11-17 (2.0.567); Maps, the model lists and MCP are still open, so the box stays empty:*
    - **Why AWS came first:** the operator's ruling of 2026-11-16 ("no project should rely on cargo build") puts first the work that removes a reference project's reason, and the AWS controls and keys stood first in PowerCloud's.
    - **What runs (ABI A38, capability `aws`):**
      - a method of an `Aws…` control is an ordinary call on the control. The runner decides it with the interpreter's own rules, now one module both call (`cobolt_runtime::aws::door`): whether the method is the control's, the connection, the refusals, the request, the budgets, and what a result writes;
      - a `Sync` operation answers at once; an `Async` one answers `1`, sets `Busy` and runs on a thread of the runner. Its event and `onComplete` (or `onError`, `onTimeout`) are queued behind what the instance is running, each with its values written as it leaves the queue, bound or not (the ABI host's `Work::Apply`);
      - `Cancel` drops a running operation's result, clears `Busy` and raises `onCancelled`; `GetRow` and `GetField` read the last row set;
      - `COBOL::"AWS-SET-CREDENTIALS"`, `"AWS-CLEAR-CREDENTIALS"` and `"AWS-CREDENTIALS-SET"` in a form's handler are calls on the application object answered through `AwsStatus` and `AwsFlag`. No host records their arguments. A program with no form takes the Cargo path for them.
    - **Tests:**
      - the runner twin `the_demos_pass_on_the_webassembly_path_and_leave_no_server_behind`: the 16 AWS demo forms, built on the WebAssembly path against the local fake servers, give 16 of 16 tallies equal to Run Form's; 17 servers started, none left running;
      - the ABI host's `aws_tests`: 9 of 9 and 13 of 13 checks (order, values written as the queue reaches them, `Cancel`, rows, the keys never recorded, a host without AWS); the crate 267 of 267;
      - the runtime: 37 AWS unit tests and 72 AWS integration tests (16 files, the live one left out), and the narration test;
      - `desktop_controls_the_runner_lacks_are_named` 8 of 8, with an AWS form served; the runner twins 13 of 13; `cobolt-wasm` and `cobolt-wasm-abi` 154 of 154; the PowerAnalyticsWeb pin 3 of 3.
    - **PowerCloud** still takes the Cargo path, for the reasons T39c lists (the image built-ins and its FileDropZone).
    - **Left for this task:** Maps, the model lists and MCP, and the `partiql_hosts` and `powercloud_hosts` twins. `PARTIQL` is still refused, and PowerCloud cannot be twinned while it takes the Cargo path.
  - *The model lists done 2026-11-20 (2.0.624); Maps and MCP are still open, so the box stays empty:*
    - **One implementation of the rules:** the interpreter's twelve arms moved, unchanged, into `cobolt_runtime::model_door`, which both hosts call. `VERBS` gives each built-in's fewest arguments and the arguments it writes. `run` reads an argument only when the built-in needs it, in the order it needs it, and takes the key of a `MODEL-LIST` or `MODEL-TEST` with a blank key from the store, when a sixth argument names an entry.
    - **What runs (ABI A53, a new capability `models`, no new import):**
      - A form's handler that calls one of the twelve makes the module require `models`. The call goes to the application object with every argument as text; the runner answers with `model_door`, and the generated code moves each answer from the application's `BuiltinAnswer<n>` into argument `n`.
      - The model list and the key store are the application's files, as under the interpreter. The models a provider last listed belong to the form instance that asked.
      - `MODEL-LIST` and `MODEL-TEST` use the calling window's HTTP client and wait for the answer, as the interpreter's do.
    - **What still takes the Cargo path:** a call with fewer arguments than the interpreter runs it with, and one in a program with no form, as AWS's keys. `D_CONTROLS` now names the MCP search and PartiQL only.
    - **Tests:**
      - **The runner twin `the_model_list_keys_and_providers_agree_on_both_engines`, against four local stubs (the entry's server, the Configuration's, the agent's own and a provider):** 18 of 18 checks. One form with 2 AgentObjects, 8 buttons and 12 event handlers covers all twelve built-ins:
        - the providers' catalogue;
        - an entry, and one with no name; the count into a numeric and an alphanumeric item; the entry read back;
        - `model_list_hosts`' order: the entry, then the Configuration, then the agent's own settings;
        - a key stored, asked about, removed and asked about again, never in a request;
        - a provider's two models, the second read back, and a connection test;
        - the entry removed, and an Ask naming it failing as spec 076 says.
        Four dumps are equal, four pictures (1,033,600 pixels) have 0 differing, each server receives the same requests from both engines ([1, 1, 1, 2]), and both engines leave the same list in the project's settings folder. In real time: the interpreter 36.6 s, the runner 36.1 s.
      - `model_door`'s own test: 10 of 10 checks. The runner's `the_abi_and_the_rules_name_the_same_arguments`: 12 of 12 rows equal.
      - `native_desktop_refusals`: 43 of 43 checks. The twelve compile in a form's handler and require `models`; a form calling none does not; `MODEL-SET` with 3 arguments, and `MODEL-COUNT` in a program with no form, are refused at their line. `subset_table`: 136 entries, 54 through the runner, 34 on the Cargo path, 0 faults.
      - `desktop_wasm_build`: a `MODEL-LIST` in a program with no form is refused at its line, naming the form's handler. Two of that test's checks expect the pointer refusal's text from before 2.0.615; 2.0.625 corrects them.
      - The interpreter, unchanged: `test_model_list` 3, `test_model_list_file` 1, `test_providers` 1, `test_cobol_object_call` 2, `test_sql_database_object` 4; the library's tests of the model list, the key store and the providers.
    - **Left for this task:** Maps, MCP (`MCP-SEARCH` with it) and PartiQL, and the `partiql_hosts` and `powercloud_hosts` twins.
  - *The MCP search done 2026-11-20 (2.0.626); Maps and PartiQL are still open, so the box stays empty:*
    - **One implementation:** the interpreter's `COBOL::"MCP-SEARCH"` arm moved, unchanged, into `cobolt_runtime::mcp_tool::search_text`, which both hosts call: the tool's name and the arguments trimmed, arguments that are not JSON read as no filters, the result's text parts one per line.
    - **What runs (ABI A54, a new capability `mcp.search`, no new import):** a form's handler that calls the search makes the module require `mcp.search`. The call goes to the application object with every argument as text, as A53's built-ins do; the runner searches the files the instance's AgentObjects allowed (`AllowFile`, `RegisterFile`, less `DenyFile` and `UnregisterFile`), one tool set per form instance as the interpreter keeps one per program, and the generated code moves the application's `BuiltinAnswer3` into the third argument. The lowering is shared with the model lists (`desktop_app_builtin`).
    - **What still takes the Cargo path:** a call with fewer than three arguments, and one in a program with no form. `D_CONTROLS` now names PartiQL only.
    - **Tests:**
      - **The runner twin `the_mcp_search_agrees_on_both_engines`:** 13 of 13 checks. One form with an INDEXED file of 3 orders, 2 AgentObjects that ask no model, 5 buttons and 11 searches: before any file is allowed; a filter in lower case; `limit` 1; arguments that are not JSON; a filter nothing matches; a result longer than its item; a file the other AgentObject registered; after `DenyFile`. Three dumps are equal, three pictures (684,000 pixels) have 0 differing, and no request leaves the machine. In real time: the interpreter 27.6 s, the runner 29.0 s.
      - `native_desktop_refusals`: 47 of 47 checks (the search compiles in a form's handler and requires `mcp.search`; a form not calling it does not; two arguments, and a program with no form, are refused at their line; `PARTIQL` in a program with no form is refused with PartiQL's reason).
    - **Found by the twin, queued as a fix (not this task's):** a value moved into a control property loses its trailing whitespace under the interpreter (`str::trim_end`) but only its trailing spaces in a module (`cobolt-wasm-rt` `cmd::set`), so an item whose text ends in a newline differs, on the desktop and in the browser. The twin closes the whole result with a bar, so it still compares all of it.
    - **Left for this task:** Maps and PartiQL, and the `partiql_hosts` and `powercloud_hosts` twins.
  - *The IndexedFile's PartiQL members done 2026-11-20 (2.0.627); Maps and the `COBOL::"PARTIQL"` built-ins are still open, so the box stays empty:*
    - **One implementation of the rules:** the interpreter's PartiQL arm moved, unchanged in behaviour, into `cobolt_runtime::partiql::door` (`resolve`, `parse`, `prepare`, `create_if_missing`, `open_engine`, `auto_commit`, `declares_file`, `ControlState`), which both hosts call.
    - **What runs (ABI A55, a new capability `partiql`, no new import):**
      - A desktop module requires `partiql` whenever a form holds an IndexedFile. `Execute`, `ExecuteTransaction`, `Fetch`, `Get`, `ColumnNames` and `ColumnCount` are calls on the control; `ResultCount`, `StatusCode`, `LastError` and a column's value are the properties the runner writes back.
      - The arguments go with every character, and after them one more, the kind of each argument after the statement (`N`, `P` or `S`), so the runner types each as the interpreter types it by its declaration (spec 108 R3).
      - As the form starts each IndexedFile is asked `_START` with the program's `SELECT`s, so `AutoOpen` opens a file the program does not declare and `AutoCommit` follows whether it does.
      - The file store is the program's (`cobolt-runner` `files.rs`, shared with the port): a control works on the program's own open file (spec 108 R9), the program's `OPEN` of a file a control opened takes it over, `COMMIT` and `ROLLBACK` reach the controls' files, and a form that closes closes them.
    - **What still takes the Cargo path:** `BindingQuery`, `RefreshBinding`, `StatusDataItem`, `CurrentRecordDataItem` and `SetProperty` (they name a data item the interpreter looks up at run time), a statement on a control whose design names a data item, a control bound to a PartiQL query (Data Binding), and `COBOL::"PARTIQL"` and its two siblings.
    - **Found by the twin, fixed before the commit:** `_START` was queued in the command batch and reached the runner after the program's first `OPEN`, which had already published its FD definition into the file; the control's file was then resolved with the program's record, and a statement naming the `.cidx`'s fields failed. The interpreter resolves at `INIT-FORM`, before the `OPEN`; `_START` is now asked (`CALLR`) so it arrives in program order.
    - **Tests:**
      - **The runner twin `the_indexedfile_partiql_members_agree_on_both_engines` (the `partiql_hosts` twin):** 16 of 16 checks. One form with 5 IndexedFile controls (a PRCIDXD1 file `AutoOpen` makes, redb with `AutoCommit` off, a file in memory, the program's open file, a file the program's `OPEN` takes over), 5 buttons and 30 member calls. Three dumps are equal and three pictures (972,000 pixels) have 0 differing. In real time: the interpreter 19.4 s, the runner 24.7 s.
      - `native_desktop_refusals`: 50 of 50 checks. `subset_table`: 136 entries, 55 through the runner, 33 on the Cargo path, 0 faults.
      - The interpreter, unchanged: the nine `test_partiql_*` files 13 passed, the library's PartiQL tests 31, and `partiql_hosts` 6 of 6 cells (two hosts, three engines).
      - Regressions: the whole `cobolt-runner` suite 21 passed, 2 ignored; `cobolt-wasm` 146, `cobolt-abi-host` 282, `cobolt-wasm-abi` 25; `cobolt-compiler --lib` 262 passed, 1 ignored; `desktop_wasm_build` 39 of 39 checks; `runner_shell_hosts` 16 of 16 (five of them rerun one at a time after running past their script limit in a parallel run, 0 differing pixels); the System KB rebuilt (4,422 records) and its check passing.
      - The program the Cargo path builds, with the moved rules: `partiql_hosts`' `the_same_answers_in_a_built_binary` (run with `--ignored`) 3 of 3 cells, on PRCIDXD1, redb and memory.
    - **Left for this task:** Maps; `COBOL::"PARTIQL"`, `"PARTIQL-FETCH"` and `"PARTIQL-ERROR"`; and the `powercloud_hosts` twin.
  - *The `COBOL::"PARTIQL"` built-ins done 2026-11-20 (2.0.628); Maps is still open, so the box stays empty:*
    - **One implementation of the rules:** what the interpreter's three arms did between reading their arguments and setting their receivers moved, unchanged in behaviour, into `cobolt_runtime::partiql::door` (`declared_schema`, `run_declared`, `FileResults`, `arg_of`), which both hosts call.
    - **What runs (ABI A56, a new capability `partiql.files`, one new import `prn.pq_file`):**
      - `PARTIQL`, `PARTIQL-FETCH` and `PARTIQL-ERROR` in a console program or a form's handler compile as calls to the module's routine (`cobolt-wasm-rt` `pq.rs`). It sends the program's files, the file argument and, for a statement, its text and each argument with its kind (`N` or `S`, by its declaration), and moves the answer into the receivers as the interpreter sets them.
      - The runner runs the statement on the program's own open file (the store T38 shares with the IndexedFile controls), with the keys the module's file descriptions now carry by name (`prfiles` `named_keys`, optional, so an older module still reads).
      - The results stay with the program, one set per file it names; a form instance keeps its own and they go when it closes.
    - **What still takes the Cargo path:** a call with fewer arguments than the built-in takes, and one with more than 256 `?` arguments. A browser module still refuses the three.
    - **A module carries only the types it uses (AC16):** the new routine's import added a type to `rt.wasm` and moved another, which moved every module's bytes. The generator now interns the linked routines' signatures and the types their blocks name, in routine order, and a glue function's types only when it makes that glue. The PowerAnalyticsWeb pin and the player's generated fixture are recorded again once; a desktop routine or import no longer moves a browser module. Guard: `type_section`'s `a_module_carries_no_type_it_does_not_use`.
    - **Tests:**
      - **The console twin `the_partiql_builtins_match_the_interpreter`:** 68 of 68 checks. One program with three files (an INDEXED file with an alternate key, one with a split key, one never opened) runs INSERT (a duplicate, a text longer than its column), SELECT (every item, the alternate key, the split key), UPDATE, DELETE, a numeric item holding letters, a statement that does not parse, a column the file lacks, `COUNT(*)`, an undeclared file, a file not open and a closed file, with arguments of nine kinds and receivers of twelve; the three refusals. Packaged by the runner and run beside `rcrun run`: 48 lines printed the same and 4 files (45,056 bytes) equal. In real time: the runner 3.6 s, `rcrun run` 3.7 s.
      - **Found by the twin, fixed before the commit:** a numeric item holding letters (left there by a group `MOVE`) went as the number decoding made of them, so the statement found nothing (`0000 00`), where the interpreter sends the characters and the statement fails (`0000 94`). The routine now takes such an item by its characters, by the interpreter's rule; the twin holds the line. The runner also listed the import without the capability, and refused the program by name.
      - **The form twin** (`cobolt-cli` `runner_partiql_form`, the IndexedFile twin of 2.0.627): a button's handler also runs the three built-ins on the program's shared file, a file argument in lower case with spaces, a statement that fails and its error: 18 of 18 checks (2 new: one row and `00`, the row, then `10` past it; the error naming the column the file lacks). Three dumps are equal and three pictures (972,000 pixels) have 0 differing. In real time: the interpreter 18.8 s, the runner 20.6 s.
      - `native_desktop_refusals`: 13 of 13 and 52 of 52 checks (the three compile and require `files.indexed` and `partiql.files`, import `pq_file`; `PARTIQL-FETCH` with 2 arguments is refused at its line). `subset_table`: 136 entries, 58 through the runner, 30 on the Cargo path, 0 faults. `type_section`: 84 types in 3 modules, 0 unused, 8 of 8 checks.
      - Regressions: `cobolt-wasm` 148 passed (the player's fixture recorded again once, then equal); the PowerAnalyticsWeb pin recorded again once, then equal; the whole `cobolt-runner` suite 22 passed, 2 ignored; the interpreter, unchanged: the nine `test_partiql_*` files 13 passed and the library's PartiQL tests 31; the desktop Build test (`desktop_wasm_build`) 39 of 39 checks.
    - **Left for this task:** Maps, and the `powercloud_hosts` twin. *(Corrected at 2.0.630: this note said PowerCloud still took the Cargo path for T39c's reasons; it has taken the WebAssembly path since 2.0.583.)*
  - *Maps done 2026-11-20 (2.0.629); the `powercloud_hosts` twin is still open, so the box stays empty:*
    - **One implementation of the rules:** the interpreter's Maps arms moved, unchanged in behaviour, into `cobolt_runtime::maps_door` (`start`, `run`, `is_service`), which both hosts call: the six service methods and the arguments each reads, the key checks before any thread (the project's Google key, `TraceRoad`'s own key), `TimeoutMs`, and the eight collection methods with their rules (a marker appended as given; a route or a region replacing the one with its id; blank lines dropped, except between markers).
    - **What runs (no new import or capability):** a Maps method is a call on the control. The ABI host's `rest_call` now hands a Maps control's to the runner's port, as a RestClient's and a WebSearch's; a page has no port, so a browser's answer is unchanged. A service method runs on a thread of the runner with the RestClient's `Busy`, `TimeoutMs` and endings (`rest_door`), one at a time per control, and a second is ignored before its key is looked at, as under the interpreter; `Cancel` and `IsBusy` are the RestClient's. The runner resolves `_ResolvedMapsApiKey` as it resolves the search key, and links the runtime's `maps` feature.
    - **The tile test folder (`PRC_TEST_MAP_TILES`):** the form host reads each tile from `<folder>/<zoom>/<x>/<y>.png` instead of a tile server, so a test pictures a map with no network; a missing file is a failed tile. Never set by default.
    - **What still takes the Cargo path:** the Viewer (T50) and the sign-in control; `desktop_control_gap` no longer names Maps. The browser's own refusal of the Maps collection methods (the ABI host's `methods.rs`) is queued as a separate fix.
    - **Tests:**
      - **The form twin `the_maps_control_agrees_on_both_engines` (`cobolt-cli` `runner_maps_form`):** 20 of 20 checks. One form with a Maps control over four tiles from a test folder, a ListBox its events write to, 5 buttons and 9 event handlers: the eight collection methods; `Geocode` and `TraceRoad` with no key (each reason in `LastError` at once; the two `onError` handlers run after the click's and both read the last); `TraceRoad` on its own thread (`IsBusy()` 1, `Busy` true) with a `Geocode` ignored while it runs; `Cancel`; a pointer click on the designed marker. Six dumps are equal, three pictures (1,134,000 pixels) have 0 differing, and no request reaches a provider. In real time: the interpreter 30.5 s, the runner 33.7 s.
      - **Found by the twin:** two of its own expectations were wrong, both engines agreeing: the `onError` handlers read `LastError` when they run, after the click's handler, and `Busy` reads as a truth value. The checks now say so.
      - The ABI host's `a_maps_method_is_the_ports_on_the_desktop` (and the crate 283 passed); `maps_door`'s 4 unit tests; the interpreter, unchanged: the library's Maps tests 13 and `test_maps_collections` 8.
      - `subset_table`: 136 entries, 58 through the runner, 30 on the Cargo path, 0 faults. `desktop_controls_the_runner_lacks_are_named`: a Viewer in a Panel is named, a Maps in a Panel is not.
      - Regressions: `cobolt-wasm` 148 passed; the whole `cobolt-runner` suite 22 passed, 2 ignored; `desktop_wasm_build` 39 of 39 checks (its example of a control the runner lacks is now a Viewer); `cobolt-forms --features render` 1,881 passed, 2 ignored; the web player rebuilt and fresh; the System KB rebuilt (4,424 records) and its check passing.
    - **Left for this task:** the `powercloud_hosts` twin. *(Corrected at 2.0.630: this note said PowerCloud still took the Cargo path for T39c's reasons; it has taken the WebAssembly path since 2.0.583.)*
  - *The `powercloud_hosts` twin done 2026-11-20 (2.0.630); T48 is done:*
    - **The twin `powercloud_agrees_on_both_engines` (`cobolt-cli` `runner_powercloud_hosts`):** the example copied into a temporary project whose six AWS connections lead to the fake MCP server, with a local web server for S3's temporary link, built once on the WebAssembly path. The same 16 scenarios run, in the same order, under `rcrun run-form` and in the built program: every tab of `powercloud_hosts`' tab-by-tab test, a photo refused with no stack and no bucket, keys that cannot deploy, the six languages of the main form, a status line written again in a new language, and the popups told the language. Compared per scenario: the values read, the windows and popups, and the requests the fake server received; five pictures pixel for pixel; the photos fitted into the cache byte for byte; and `powercloud_hosts`' expectations, on the built program. Nothing reaches AWS (planted keys, visibly fake and not shaped like AWS's own ids; the test's own `HOME`; no `AWS_*` variable) and no secret may appear in any output.
    - **Found by the twin, in the twin (no product change):**
      - Run Form takes the project's form theme only from the IDE (`--theme-default`); without it the interpreter drew Liquid Glass and the built program Spatial, 166,637 pixels apart in every picture. The twin now passes the project's `[forms] theme`, as the IDE does: 0 differing.
      - A built program's relative paths start at its own folder (1.91.201), so it works in the `data` beside it, which the Build filled. The twin had reset and read the project's `data` for both engines, so the built program kept its photo cache from one scenario to the next, and the requests of two later scenarios differed. Each engine now gets its own folder reset and read.
    - **Corrected:** the notes of 2.0.628 and 2.0.629 said PowerCloud still took the Cargo path for T39c's reasons; it has taken the WebAssembly path since 2.0.583.
    - **Tests:** `powercloud_agrees_on_both_engines`: 78 of 78 checks. 16 scenarios on each engine, 84 hosted calls on each, equal; five 1120 x 740 pictures, 4,144,000 pixels compared, 0 differing; 7 cached photos equal byte for byte; the six languages' 322 reads equal. Real time: Run Form 456.9 s, the built program 405.6 s; `rcrun build` on the WebAssembly path 80.6 s; the whole test 955 s. No product code changed, so no other suite was run again.

- [x] **T49 — AUDIO, ANIMATIONSEQUENCE and LAYOUT** (R11)
  - *Sound done 2026-11-16 (2.0.555); `ANIMATIONSEQUENCE` and `LAYOUT` were still open, so the box stayed empty:*
    - **Why sound came first:** the operator's ruling of 2026-11-16 ("no project should rely on cargo build") puts first the work that removes a reference project's reason. The sound verbs stood in three of them: PowerCallOfDuty (10 statements), PowerCobolCraft (4) and PowerCloud (7). No reference project uses `ANIMATIONSEQUENCE` or `LAYOUT`.
    - **What runs:** `COBOL::"AUDIO-PLAY"`, `"AUDIO-PAUSE"`, `"AUDIO-RESUME"`, `"AUDIO-STOP"` and `"AUDIO-STATE"` in a form's handler compile as calls on the application object, and each receiver takes its answer from the properties `AudioStatus`, `AudioState`, `AudioPosition` and `AudioDuration` (ABI A36, capability `audio`).
      - The runner plays them through the ABI host's new player port with the runtime's own player, one door per form instance, as each form's interpreter has one. A form's door is released when the form closes.
      - A program with no form still takes the Cargo path for them, as for the file dialogs. A browser module is unchanged.
    - **Tests:**
      - the runner twin `sounds_are_played_and_answered_alike` on the null output: 8 readings equal, 1 picture with 0 differing pixels, 9 of 9 checks. With it, the file's 7 twins pass 69 of 69 checks.
    - **Left for this task:** `ANIMATIONSEQUENCE` and `LAYOUT`, after T50's canvases, which hold three reference projects on the Cargo path.
  - Verify: runner twins equal in dumps and frames, with animations under the virtual clock.
  - *`ANIMATIONSEQUENCE` and `LAYOUT` done 2026-11-21 (2.0.631); with the sound of 2.0.555, T49 is done:*
    - **What runs** (ABI A57, capabilities `sequences` and `layouts`, no new import):
      - `COBOL::"ANIMATIONSEQUENCE"` and `COBOL::"LAYOUT"` in a form's handler compile as calls on the application object, with every argument as the program wrote it.
      - The ABI host keeps each form instance's sequences (`host/scripted.rs`) and reads their scripts and the layout scripts with `cobolt_forms::sequence` and `cobolt_forms::layout_script`, the interpreter's own readers. Names are checked against every control of the form, nested ones included.
      - The runner fires the entries that came due between handlers, as the interpreter does while it waits for the next event; a form that closes takes its sequences with it.
      - A control's `PlayAnimation`, `StopAnimation` and `Pause` now reach the window from a built program.
      - A refusal is the interpreter's, word for word, and the form survives it as there: the message goes to the form's `LastException` and a Critical notification.
      - A call with fewer than two arguments, or in a program with no form, is refused when the program is built (the Cargo path). A browser module still refuses both.
    - **Deterministic runs:** under `PRC_TEST_CLOCK` the interpreter's sequence clock is the form host's virtual time, as the runner's is. Without it nothing changes.
    - **Tests:**
      - the runner twin `sequences_animations_and_layout_scripts_agree_on_both_engines` (`cobolt-cli` `runner_motion_form`): every sequence action, `SHOW`, `HIDE` and `PLAY` entries, `PlayAnimation` on a control, `Layout` `Apply` and `Clear` at 760 and 560 px, and a sequence never created. 7 dumps equal, 8 pictures with 0 of 2,499,200 pixels differing, 24 of 24 checks;
      - the ABI host's two new tests, the crate 285 of 285; `native_desktop_refusals` 57 of 57 and 13 of 13; the interpreter's sequence tests 7 of 7;
      - the whole `cobolt-runner` suite 22 passed (2 live MySQL tests ignored); `desktop_wasm_unchanged` 3 of 3 artefacts match the pin.
    - **Found by the twin, in the twin:** it had expected the unknown sequence to end the program; both engines keep the form running, as for every unhandled exception (the operator's ruling of 2026-09-06).
    - **Not measured, and why:** the twin runs in real time, not under the virtual clock this task's Verify line names. Every picture is taken after what it shows has come to rest, so timing cannot move a pixel, but the frames in the middle of a fade are not compared. Under the virtual clock neither engine fires a sequence's entries in step with the script's frames, so a picture taken in the middle of a fade would not compare reliably. The box is ticked on the pictures at rest; motion frame by frame is left to AC19's gate.

- [x] **T50 — Viewer verbs, the window methods and canvas frames** (R11)
  - *Canvas frames done 2026-11-17 (2.0.560); the Viewer's verbs and the window methods are still open, so the box stays empty:*
    - **Why the canvases came first:** three reference projects took the Cargo path for them (PowerCallOfDuty, PowerCobolCraft, PowerGraphics). The operator's ruling of 2026-11-16 puts first the work that removes a project's reason.
    - **What runs:**
      - the seven graphics objects (Canvas2D, Canvas3D, ImageBuffer, Scene3D, Plot, Physics2D, Physics3D) in a desktop form take the WebAssembly path (`desktop_control_gap` serves class E);
      - the runner draws them with the interpreter's `cobolt-graphics` registry, one per form instance (`desktop_registry`): drawing calls and the ABI A23 display lists run in order when the program hands its changes to the windows, and read-backs and the other six objects answer at once through `Host::gfx_sync`;
      - `LastError`, presented frames (`_CanvasFrame`, `_Canvas3DFrame`), `AutoPresent` and `onFrame` behave as under the interpreter, whose argument and answer rules the runner calls (`value_of_text`, `answer_text`).
    - **Tests:**
      - the runner twin `canvases_and_graphics_objects_draw_and_answer_alike`: 6 readings equal, 3 pictures with 0 differing pixels, 12 of 12 checks. With it, the file's 11 twins pass 108 of 108 checks;
      - `desktop_controls_the_runner_lacks_are_named` 6 of 6, with the canvases served.
    - **Not measured:** no twin pictures a Canvas3D; the headless rasteriser has no 3D path.
    - **Left for this task:** the Viewer's documents and verbs, and the window methods the ABI host does not carry out yet.
  - *The Viewer and the window methods done 2026-11-21 (2.0.632); with the canvases of 2.0.560, T50 is done:*
    - **What runs** (ABI A58, capability `viewer`, no new import):
      - **The Viewer.** A desktop form with a Viewer takes the WebAssembly path (`desktop_control_gap` serves it). Its methods, a program's `SetProperty` on it, a write to its member and what the window sends back (the toolbar's Save As, print and share, the find bar, the page) go to the runner's port, which applies the interpreter's own rules:
        - the rules moved, unchanged in behaviour, from the interpreter's dispatch into `cobolt_runtime::viewer_door` (C5), which the interpreter and the runner both call. The interpreter's tests named `viewer` prove it unchanged, 25 of 25;
        - a source written starts a load (`Progress`, `onLoaded` or `onError`), and a load that fails puts back the document still on screen. The window still gets the value the program wrote, in its place among the pending writes, and the program reads the restored one (`FormTree::put_quiet`);
        - `LoadBytes` and the conversation's methods (`AppendMarkdown`, `AppendHtml`, `AppendRaw`, `AppendToMessage`, `ReplaceMessage`) send their arguments with every character in a native module, as the interpreter passes them; the generator used to trim them, and the twin caught a lost space before an emphasis;
        - the runner links the runtime's `pdf` and `office` features, so it opens what the interpreter opens.
      - **The window methods.** `SetFullScreen` goes to the window supervisor, as the other window methods have since T31; the ABI host used to ask a browser for it. `GetFormState` and `FormState` answer `Ready`, or `Waiting` while the program has written `Waiting`, as the supervisor answers (spec 037 R16, R23); a page still answers `Normal`. A form embedded in the ContentPane refuses `SetFullScreen`, `SetTitleVisible`, `SetWindowState`, `Focus` and `SetFocus` in the interpreter's words (`cobolt_forms::exceptions::window_method_needs_a_window`, shared with the supervisor), and the handler stops there; the breadcrumb's methods stay its own. The runner tells the ABI host each instance's supervisor handle, and whether it is embedded, before it asks for the form (`Host::name_window`), because a form's `onLoad` runs inside its open; the form host hands that to the runner as `ProgramRun::embedded`.
      - The ABI host's form verbs now leave no window method unimplemented on the desktop.
    - **Tests:**
      - the runner twin `the_viewer_agrees_on_both_engines` (`cobolt-cli` `runner_viewer_form`, new): three Viewers: a streamed conversation (Markdown, HTML and raw messages appended, extended and replaced, a new conversation, an older one registered and selected), a text file loaded through `SetProperty`, zoomed by a `MOVE` and a load that fails, the find bar opened, stepped and closed, a document given as bytes and saved, the conversation saved as a PDF, a print with nothing to print, and Save As answered with a path and with a cancel. Every message is sent as written, through an `INVOKE` and an inline call. Dumps equal, 8 pictures with 0 of 3,222,400 pixels differing, no dialog answer left unused, 33 of 33 checks;
      - the runner twins `full_screen_and_form_state_are_the_supervisors_alike` and `an_embedded_form_refuses_the_window_methods_alike` (`runner_shell_hosts`): `full_screen_and_form_state_are_the_supervisors_alike` (`SetFullScreen` on and off, then `GetFormState()` through the main window's handle before any write, while `Waiting` and when `Ready` again: 4 readings and 2 dumps equal, 2 pictures with 0 of 512,000 pixels differing, 7 of 7 checks) and `an_embedded_form_refuses_the_window_methods_alike` (a form in the shell's ContentPane: `SetFullScreen` and `SetTitleVisible` refused, read by its `onUnhandledException` naming `W1`, `SetBreadcrumbDetail` kept: 5 readings and 3 dumps equal, 3 pictures with 0 of 1,620,000 pixels differing, 9 of 9 checks). The file's 18 twins pass 173 of 173 checks, 42 pictures with 0 of 15,458,320 pixels differing;
      - the ABI host's `a_viewer_is_the_ports_and_a_quiet_write_keeps_the_windows_value` (18 of 18), `a_desktop_host_with_no_viewer_port_answers_as_a_page` (3 of 3) and `window_methods_go_to_the_supervisor_and_an_embedded_form_refuses_the_ones_that_need_a_window` (20 of 20);
      - `native_desktop_refusals` 59 of 59 and 13 of 13 (a desktop form with a Viewer requires `viewer`, the same form for a browser requires nothing); `subset_table` and `desktop_controls_the_runner_lacks_are_named` now count the Viewer as served (15 of 15); `desktop_wasm_build` 39 of 39 (its example of a control the runner lacks is now the sign-in control);
      - the ABI host crate 288 of 288; `cobolt-wasm` 148 of 148; the interpreter's tests named `viewer` 25 of 25 and `form_host` 14 of 14; `cobolt-form-host` 193 of 193; `cobolt-forms --features render` 1,882 passed (2 ignored);
      - the whole `cobolt-runner` suite 22 passed (2 live MySQL tests ignored), 757 s; the other runner twins `runner_viewer_form` 33 of 33, `runner_one_form` 15 of 15, `runner_surfaces` 6 of 6, `runner_motion_form` 24 of 24;
      - `desktop_wasm_unchanged` 3 of 3 artefacts match the pin; the player rebuilt, `freshness` 6 of 6; the System KB rebuilt, `prebuilt_chunked_kb_matches_the_published_documentation` green.
    - **Found, and queued as fixes (not changed here):**
      - the interpreter refuses `H::FormState`, the property read through a windowHandler (spec 037 R23), as a data item's member: `is_value_item` does not set aside a `USAGE OBJECT` item that holds a window's handle. `INVOKE H::GetFormState()` works; the twin uses it. The built program answers the property read;
      - the interpreter's supervisor mirrors only the main window's `FormState`, so `GetFormState()` through the handle of a child window answers `Ready` even while that form is `Waiting`. The built program answers from the form's own `FormState`; no twin reads a child window's state;
      - Markdown documents: `cobolt_forms::viewer::open_document` answers "Markdown rendering is not yet available" on both engines, so the twin loads a text file.
    - **Not measured:** a full-screen window. The headless form host has no window to make full screen, so the twin checks that the handler goes on and the pictures agree.
  - Verify: runner twins. The ABI host's list of unimplemented window methods is empty.

- [ ] **T50a — The data uses M5 left on the Cargo path** (R11; C4)
  - *Added 2026-11-19 at the M5 close (T46): these refusals named M5, and no task took them. They are uses of data, not entries of the native column, so T45 did not cover them. R11's end state (T51) needs them.*
  - Do:
    - national and UTF-8 data in every use the interpreter computes: reference modification, `INSPECT`, `STRING`, `UNSTRING`, a function's argument, `ACCEPT`, `SET`, arithmetic, a `CALL` parameter, a control's property, an SQL host variable, a `SORT` key, a condition-name, a number moved in or out, a JSON group (`D_TEXT_USE`), and in a program with a `PROGRAM COLLATING SEQUENCE` (`D_TEXT_COLLATING`);
    - national and UTF-8 text in a `LINE SEQUENTIAL` record (`D_LINE_TEXT`);
    - the SQL descriptor area: `DESCRIBE … INTO`, `USING DESCRIPTOR` and `INTO DESCRIPTOR`, whose host variables are reached through the pointers in the descriptor's table (`D_DESCRIPTOR`; a module's pointers are whole items outside any table since T45b).
  - Verify: runner twins against `rcrun run` for each use, and `native_desktop_refusals` without these refusals.
  - *Split 2026-11-21 into five pieces, each one commit with its own version and runner twin: **a1** the eight national functions and a function's argument; **a2** `INSPECT`, `STRING`, `UNSTRING` and reference modification by character; **a3** `ACCEPT`, `SET`, arithmetic, a `CALL` parameter, a control's property, an SQL host variable, a `SORT` key, a condition-name, a number moved in or out and a JSON group (split again 2026-11-21: **a3a** `ACCEPT`, `SET`, a condition-name, a number moved in or out, comparisons and class tests; **a3b** a `CALL` parameter, a control's property, an SQL host variable, a `SORT` key and a JSON group; arithmetic on a national item is a semantic error in the interpreter too and needs nothing); **a4** `PROGRAM COLLATING SEQUENCE` (`D_TEXT_COLLATING`) and a `LINE SEQUENTIAL` record (`D_LINE_TEXT`); **a5** the SQL descriptor area (`D_DESCRIPTOR`). T50a stays open until the fifth is done.*
  - **T50a1 — done 2026-11-21 (2.0.633): the functions over national and UTF-8 data.**
    - What a desktop module now computes: `NATIONAL-OF` and `DISPLAY-OF` (UTF-8, Windows-1252 and ISO-8859-1, by name, by CCSID or from a data item; the substitution character as the interpreter writes it; any other code page ends the run in the interpreter's words), `UPPER-CASE` and `LOWER-CASE` by Unicode's rules for national and UTF-8 data and by ASCII for anything else (the final sigma included), `REVERSE` by character, `ULENGTH`, `UPOS`, `UWIDTH`, `UVALID`, `USUPPLEMENTARY`, `USUBSTR`, and every function that reads the text of its argument (`TRIM`, `CONCATENATE`, `NUMVAL`, `STORED-CHAR-LENGTH`, …) over a national or UTF-8 item or an `N"…"` literal.
    - How: nine new runtime routines in `cobolt-wasm-rt` (`natfn.rs`, `func.rs`); a literal's class travels in the operand (`F_NATIONAL_LIT`); the generator admits a national operand per function argument (`admits_text_arg`) and treats a character-valued function as an alphanumeric sender. A browser module still refuses the new functions.
    - Still refused, now T50a2: a reference-modified national argument (`UPPER-CASE(WS-N(1:2))`) and a national item as the argument of a function that reads a number (`ABS(WS-N)`).
    - Measured: module against interpreter, 22 cases in both memory layouts, 196 DISPLAY lines and 1,494 bytes of item images identical, plus the unknown-code-page error text, 4 of 4; the runner twin `national_functions` (`cobolt-runner`) runs `nat_functions.cbl`, `utf8_basics.cbl` and the four interpreter tests of AC4, AC12, AC14 and the Unicode case rule packaged and under `rcrun run`: 3 of 3 programs identical, 22 of 22 checks; `native_desktop_refusals` 23 of 23 checks.
    - **AC16, on purpose:** the runtime blob grew from 269,921 to 288,161 bytes (+18,240) and every generated module links it, so the player's eight recorded fixtures grew by 2,304 bytes each and the PowerAnalyticsWeb pin moved on all three artefacts; both were re-recorded. The pin matched before this change.
    - Known approximation: `LOWER-CASE` writes a capital sigma as a final sigma by Unicode's rule, with the "case-ignorable" test a small fixed set (apostrophes, full stops, the colon, accents, the combining marks U+0300 to U+036F) instead of the whole Unicode table that Rust's `str::to_lowercase` holds. The twin's Greek case (`ΟΔΥΣΣΕΥΣ ΑΣ`) agrees; a sigma beside a character outside that set may lower differently from the interpreter. To close it, the rule needs the full table in the runtime blob.

  - **T50a2 — done 2026-11-21 (2.0.634): `INSPECT`, `STRING` and `UNSTRING` by character.**
    - What a desktop module now computes: `INSPECT` of a national or UTF-8 item (a reference-modified target counted in characters), `STRING … INTO` such an item and `UNSTRING` of such a source, with every phrase the interpreter reads: `TALLYING`, `REPLACING`, `CONVERTING`, `BEFORE`/`AFTER INITIAL`, `POINTER`, `COUNT IN`, `DELIMITER IN`, `ON OVERFLOW`, `ALL` and several delimiters, the split by receiver sizes. Alphanumeric operands mix with national ones as the interpreter mixes them.
    - How: `INSPECT` and `UNSTRING` in `cobolt-wasm-rt` became generic over the unit they walk (`u8` for bytes, `u32` for code points), so one copy of each rule serves alphanumeric and national data; `STRING` into a classed receiver assembles the senders' bytes and fits the receiver by character. Three new runtime routines (`rt_inspect_text`, `rt_string_text`, `rt_unstring_text`); the generator admits a classed operand per verb (`verb_operand`, `verb_receiver`) and refuses what it cannot mirror.
    - Still refused: a reference-modified national sender or source, a group holding a national item named in one of the verbs, `ALL N"…"` as a pattern or delimiter.
    - Decision: the interpreter's reference modification of a national or UTF-8 item counts UTF-8 bytes, not characters (found while twinning an `INSPECT` target). It is not mirrored: the module counts characters for the one place it admits such an item, and refuses the rest. The interpreter's rule is queued as a separate fix on `fixes-2x-<slug>` from `main`.
    - Limit: the characters of one statement live in the 32 KiB scratch area, so an item of more than about 2,000 positions in these verbs ends the run with error 1105 (scratch overflow).
    - Measured: module against interpreter 34 of 34 cases in both layouts (12 new); the runner twin `national_verbs` 3 of 3 programs identical, 12 of 12 checks; NIST NC on generated modules 95 of 95 execute, 0 failures, 4,612 PASS, 5 DELETED; `native_desktop_refusals` 27 of 27.
    - **AC16, on purpose:** the runtime blob grew from 288,161 to 303,251 bytes (+15,090); the player's eight fixtures +429 bytes each; the PowerAnalyticsWeb pin re-recorded.

  - **T50a3a — done 2026-11-21 (2.0.635): a number meets national and UTF-8 text; `ACCEPT`; condition-names.**
    - What a desktop module now computes: `MOVE` of a numeric item (its digit positions, zero-padded, unsigned, left-justified), of an integer literal (the digits as written) and of a decimal or computed value (its text) into a national or UTF-8 item; `MOVE` of such an item into a numeric item (the number its text spells, else the receiver as it was); `IF` against `ZERO`, an integer literal or an integer numeric item; class tests; condition-names over such a host (values, ranges, `EVALUATE TRUE`, `SET … TO TRUE`, also under a table element); `ACCEPT` from the console, `ENVIRONMENT`, `COMMAND-LINE`, `ARGUMENT-NUMBER` and `ARGUMENT-VALUE`.
    - How: `mv::Sender::Lit` separates a number written in the program or computed from an item's number; `nat::store` writes the number as text by the interpreter's rule (an integer literal is a `Sender` flagged `AUX_DIGITS` and keeps left-justification); `nat::relation` compares text with a number (`ZERO`, integer, integer item); `nat::class_test`; the generator's `check_text_move` and `check_text_compare` admit exactly these pairs and `set_condition` takes the host's subscripts.
    - Decisions: arithmetic on a national item is a semantic error in the interpreter, so it stays refused with the other uses. A text such as `1e3` is not read as 1000 (the interpreter leaks Rust's float syntax there); the module leaves the receiver as it was. The interpreter ignores `SET cond (sub) TO TRUE` for a condition-name under a table element; the module sets the element as COBOL-85 says, and that case is a module-only test. Both interpreter points are queued as separate fixes on `fixes-2x-<slug>` from `main`.
    - Still refused: a decimal item or literal, a negative literal or a computed value against such an item in a comparison; `ACCEPT … FROM DATE`/`TIME` into one; a `VALUE` with a number on such a host; arithmetic.
    - Measured: module against interpreter 43 of 43 cases in both layouts (9 new), 462 DISPLAY lines and 4,316 bytes of item images identical; the runner twin `national_numbers` 2 of 2 programs identical, 23 of 23 checks; NIST NC on generated modules 95 of 95 execute, 0 failures, 4,612 PASS, 5 DELETED; `native_desktop_refusals` 34 of 34.
    - **AC16, on purpose:** the runtime blob grew from 303,251 to 307,525 bytes (+4,274); the player's eight fixtures and the PowerAnalyticsWeb pin re-recorded.

  - **T50a3b — done 2026-11-21 (2.0.637; `main` holds 2.0.636): a `CALL` parameter, a control's property, an SQL host variable, a `SORT` key.**
    - What a desktop module now computes: one elementary national or UTF-8 item passed `BY REFERENCE` or `BY CONTENT` to a parameter described the same way (class, size, `JUSTIFIED`), or `BY VALUE`, or a literal or a value moved into the parameter; such an item written to and read back from a control's property, a method's argument or a path (UTF-8 text on the wire, trailing spaces cut); such an item as an `EXEC SQL` host variable (read, written, with an indicator, in a host structure), crossing as the bytes of its own image; a national or UTF-8 `SORT` key, ascending or descending, by code point.
    - How: `bind_args` admits a classed parameter when the argument's description agrees (`check_text_arg`), and a classed argument takes `text_operand`; a control member read or written by a classed item is `Side::Host` (`moves_into_classed`), and `raw_text_of` reads the item as UTF-8; `sql.rs` in the runtime treats `K_NATIONAL16` and `K_UTF8` as bytes of the item's image, padded by class (`put_image`), and the runner's `put` sends the image where it sent the UTF-8 text before; the `SORT` key lowers its item with the class admitted.
    - Decisions: an argument the interpreter would alias with another class or size, a UTF-8 literal with non-ASCII characters passed by content (the interpreter pads it by bytes) and a group holding a national item stay refused. **The JSON group stays refused:** its only user is the call to a backend over `pr.remote`, which the desktop runner does not provide; it is lifted with that channel. Four interpreter defects found here (a `SELECT INTO` of a longer text into a `PIC N` or `PIC U` host variable cuts by bytes and can leave half a character; `SET cond (sub) TO TRUE` writes nothing; `"1e3"` read as a number; reference modification counts UTF-8 bytes) are queued as separate fixes on `fixes-2x-<slug>` from `main`.
    - Still refused: a parameter that is a group holding one or a table; a non-identical argument description; every use not listed in T44 and T50a1 to T50a3.
    - Measured: module against interpreter 45 of 45 cases in both layouts (2 new), 572 DISPLAY lines and 4,540 bytes of item images identical; control properties 8 checks; the runner twin `embedded_sql_matches_the_interpreter` 11 of 11 programs identical, 67 of 67 checks (`ac12_national.cbl` new); NIST NC on generated modules 95 of 95 execute, 0 failures, 4,612 PASS, 5 DELETED; `native_desktop_refusals` 43 of 43.
    - **AC16, on purpose:** the runtime blob grew from 307,525 to 307,744 bytes (+219); the player's fixtures and the PowerAnalyticsWeb pin re-recorded.

  - **T50a4 — done 2026-11-21 (2.0.638): a `PROGRAM COLLATING SEQUENCE` and a `LINE SEQUENTIAL` record that holds national or UTF-8 fields.**
    - What a desktop module now computes: every `PROGRAM COLLATING SEQUENCE` (`STANDARD-1`, `NATIVE`, a program's own alphabet) beside national and UTF-8 data, which compare by code point while alphanumeric comparison, `SORT` and `INSPECT` follow the sequence; a `LINE SEQUENTIAL` record holding `PIC N`, `PIC U`, `PIC U BYTE-LENGTH` or a table of them, written as the characters' UTF-8 text and read back by class.
    - How: the collating refusal (`D_TEXT_COLLATING`) is gone, the module's comparison already weighed alphanumeric data by the table; the generator lists the record's national and UTF-8 fields (`collect_text_fields`: offset, size, class, each occurrence of a table) in the file's `prfiles` entry, and the section moves to **format 2** only for a module that has such a field; the runner (`record_layout`) rebuilds the runtime's `RecordLayout` from it and the interpreter's own `image_to_line` and `line_to_image` make the line and the record.
    - Decisions: national data under a collating sequence compares by code point (COBOL-85: the sequence orders alphanumeric data); the interpreter weighs it by UTF-8 bytes through the alphabet's table and is not mirrored (queued as a separate fix). A `LINE SEQUENTIAL` file with more than one record description, any of them holding such a field, stays refused at its first verb (`D_LINE_TEXT`): the interpreter reads by the first description and writes by each one's own.
    - Measured: module against interpreter 47 of 47 cases in both layouts (2 new), 620 DISPLAY lines and 4,540 bytes identical; the runner twin `files` 14 of 14 programs identical, 262 of 262 checks (`LINE_TEXT` new: 20 lines, 2 files, 223 bytes equal); `native_desktop_refusals` 59 of 59 checks; the ABI test of the section 2 of 2.
    - The runtime blob is unchanged: no fixture and no pin moved. The ABI crate is an input of the browser player, so the player was rebuilt for `freshness` (`threaded/player_bg.wasm`, `player-build.json`); `desktop_wasm_unchanged` 3 of 3 still matches, `freshness` 6 of 6.

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
