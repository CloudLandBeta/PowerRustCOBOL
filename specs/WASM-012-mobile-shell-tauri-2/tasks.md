<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The mobile shell (Tauri 2): Android and iOS builds, devices, signing and permissions (WASM-012)

- **Status:** draft. **Revised 2026-10-23** to match `plan.md` as revised on 2026-10-23 (2.0.49); the first version was written 2026-10-22 from the
  plan's first draft (2.0.30). No box was checked in the first version, so there is no evidence to carry. No shell code exists yet. Every Tauri, Android
  and iOS statement in the plan is marked **[verify]** and is unconfirmed; T1 confirms them before any other task starts.
- **Plan:** ./plan.md   **Date:** 2026-10-23
- **Implementation (2026-11-05/07, `features-2x-wasm-pending`, 2.0.288–2.0.296).** The Android half runs end to end: `rcrun mcp build` and the IDE's Build
  of a Mobile project make the web bundle and then `dist/mobile/android/<Name>-debug.apk`; the IDE's Run with an emulator builds, installs and starts
  it as a tap on its icon would and shows the application's log (a `DISPLAY` reads as the program wrote it); the phone's secure storage works on the
  emulator (stored, read back after a restart, deleted); the forms keep out of the status and gesture bars; Back on the first form ends the
  application cleanly (2.0.289); with no phone and no emulator running, Run starts the emulator set up and waits for it (2.0.290); a Run
  over the application on screen keeps it alive, and the Output shows only the application's own log (2.0.291); with several places to run,
  the Run on device window asks once and the session keeps the answer, and a failed run opens nothing else (2.0.292); with nothing running
  and one emulator set up, Run starts it again without asking (2.0.293, a regression of 2.0.292 fixed), and inside the shell the
  "connecting" modal draws the phone or the tablet and a return from the background retries a lost connection behind it (2.0.293); a
  Mobile project for Android gets its Android Studio project in `android/` from New Project's step or from Project Settings, every package
  build keeps it on the same bundle, and a new Mobile project starts with an application id that Project Settings edits (2.0.294). The
  example is `examples/PowerMobileHello`. The ABI amendment is **A26** (A14–A25 were taken). Done: T1–T7, T12, T17, T18, T23. Partly done
  (what is left is named under each): T8, T9, T11, T13–T16, T19, T20, T22, T24, T25b, T25c. Not started: T10 (O9), T21, T25 (O1), T26. iOS: until 2026-11-10 it was not run
  here (Xcode 15.2's simulators were iOS 17.2, below the iOS 18 floor). Xcode 27.2, the iOS 17.2 and 27.2 simulators and `xcodegen` 2.46.0 are in place now, and the iOS web view runs a built bundle (WASM-002 T24). **The iOS package step is written (2.0.411, `mobile_build::build_ios`):** Build on an iPhone project produces the development application for the simulator, `dist/mobile/ios/<Name>-debug.app`, run on an iPhone 18 Pro (iOS 27.2) where it shows the form (`cobolt-project-tools/tests/ios_package.rs`, `#[ignore]`d). Still not written: the release archive (refused with the reason), Run on the simulator from the IDE and the iOS device list (T16's iOS half), the Xcode native project (R16).
  The "no task commits" rule below is overridden by the operator (2026-11-06: every task is committed, merged into `2.x.x` and pushed outside the
  window).

Ordered, small, independently-verifiable tasks. Each names the files it touches, the requirement(s) it satisfies, and how to verify it. Check off as
completed, with the evidence (counts, command output) written under the task.

**What this revision changes (follows plan §0 C1–C6).**
- **R3a is planned** (plan C1, D-Preview): a new task, **T20**, implements WASM-008's `DeviceRunner` seam as `MobileDeviceRunner` and routes a Mobile
  project's Preview and Run through it. T13 gains `run_dev`; T16's dev-mode run serves both Run on device and Preview.
- **The ABI amendment is A14**, not A10 (plan C2): `specs/WASM-ABI-v1.md` §12 already holds A10–A13 (lines `:173`–`:176`).
- **The device verbs follow the plan's route** (plan C3, §2): `COBOL::SecureStore`/`SecureLoad`/`SecureDelete` are **application-object verbs**
  (`APP_VERBS`, `crates/cobolt-wasm/src/lower_remote.rs`), **not** rows of `BUILTINS`. The first version's O6 (hyphenated `SECURE-*` built-ins in
  `BUILTINS`) is withdrawn: the plan has decided the route, and the parser already turns `COBOL::SecureStore(…)` into `CALL "COBOL-SECURESTORE"`
  (`cobol_object_call`, `crates/cobolt-parser/src/stmt.rs:3430`), which is the name the scan and the generator see.
- **T5 is rewritten**: it reads what the desktop does with an unknown `COBOL-` name and adds a refusal only if that path is silent (plan §2,
  `cobolt-runtime`).
- **Clipboard** is partly present (plan C5); it stays in the gated T25.
- The three "the mobile shell is not included yet" texts are named (plan C3) and replaced in T17, T18 and T22.

**2.x.x overrides for this list.** There is **no documentation task**. `README.md` and everything under `docs/` are never touched on 2.x.x. What 2.x.x
says about itself goes in `specs/`: here, the ABI amendment A14 (T6), the plan's V0 record (T1) and the `WASM-PROGRESS.md` row (T24). The user-facing
description of the mobile build lives in the System KB text (T22). The finalize task (T26) bumps `z` once and writes one CHANGELOG entry. No task
commits, pushes, checks out, switches, resets, stashes or rebases.

**Grounding read for this list (2026-10-23, `features-2x` at 2.0.49, `crates/cobolt-ide/src/version.rs:22`).**
- **Manifest model.** `crates/cobolt-compiler/src/project_kind.rs`: `MobileSettings` `:446` (`min_os` `:455`, `player` `:467`, `max_payload_bytes`
  `:470`, `call_timeout_ms` `:473`), `MobileSettings::new` `:483`, `module_variants` `:504`; the tests module at `:625`. No `preview_browser` field
  exists yet (WASM-008 T30 adds it; not this spec).
- **Compiler.** `crates/cobolt-compiler/src/lib.rs`: the `mod` lines `:57-63`; `default_build_root` `:144`; `build_staging_dir` `:222`; `stage_sdk`
  `:3778`; `CompilerError::UnsupportedTarget` `:760`, `NotADesktopProject` `:770`, refused at `:1825`. The KB section "Project kinds: Desktop, Web and
  Mobile" `:6193`, its Mobile row `:6208` ("…needs the mobile shell, which the product does not include yet"). KB tests: `published_documentation_tests`
  `:14088`, `knowledge_base_build_tests` `:14232`, `project_kind_build_tests` `:14397`. `crates/cobolt-compiler/src/backend_build.rs`:
  `BackendBuildOptions` `:37`, `build_backend` `:103` — the pattern of `mobile_build.rs`.
- **Web build.** `crates/cobolt-project-tools/src/web/mod.rs`: `BUNDLE_SUBDIR = "dist/web"` `:48`, `WebBuildRequest::from_manifest` `:85` (sets
  `Isolation::Off` `:113`), the `Refused` kinds from `:233`. `crates/cobolt-project-tools/src/tools/build.rs` `:131-134`: the Mobile `package` reason
  "…needs the Tauri 2 mobile shell, which this program does not include yet" (and the macOS-only reason for iOS).
- **Parser.** `cobol_object_call` `crates/cobolt-parser/src/stmt.rs:3430`: `COBOL::Name(args)` → `CALL "COBOL-NAME"` (upper case, `COBOL-` prefixed).
- **Generator.** `crates/cobolt-wasm/src/lower_remote.rs`: `APP_VERBS` `:26` (the comment `:23-25`: "named here and nowhere in `BUILTINS`"),
  `is_app_verb` `:47`, `suspends` `:53`. `crates/cobolt-wasm/src/lower_form.rs`: `builtin_call` `:441`, the app-verb arm `:494`, the
  `BackendOnly` refusal `:497`.
- **Desktop.** `crates/cobolt-runtime/src/builtins.rs`: `BUILTINS` `:35`, `builtin()` `:107`. Built-in calls are dispatched by name in
  `crates/cobolt-runtime/src/interpreter.rs` (e.g. `"COBOL-APPEND-FILE"` `:11567`). `crates/cobolt-semantic/src/calls.rs:123` returns early for any
  `COBOL-` name, so Check says nothing about an unknown built-in; what the interpreter does with one was **not read** — T5 reads it.
- **Player** (own workspace, root `Cargo.toml:45` `exclude`). `crates/cobolt-web-player/src/host.rs`: `verb` `:1605`, `"NAVIGATE" | "OPENURL"` →
  `Effect::Navigate` `:1639`, `"COPYTOCLIPBOARD"` → `Effect::Clipboard` `:1647`. `src/net.rs`: `backoff_start_ms`/`backoff_max_ms` `:75-76`. The
  shipped player is rebuilt with `cargo run --manifest-path crates/cobolt-web-player/Cargo.toml --example build_player -- --out assets/web-player`;
  freshness with `--test freshness` (WASM-016).
- **Bundle.** `crates/cobolt-web-bundle/src/csp.rs`: `CspOptions` `:26`, `CspOptions::strict` `:42`, `Policy` `:49`.
- **IDE.** `crates/cobolt-ide/src/app.rs`: `is_web_project` `:2687`, `start_web` `:2715`, `toggle_web_preview` `:2736`, `do_run` `:2823`.
  `src/panels/web_preview.rs`: `Note::PackageNote` `:78`, rendered `:217`, said `:730`; **no `DeviceRunner` exists yet** (WASM-008 T39, unchecked).
  `src/toolchain.rs`: `minimum` `:127`, `spawn_install` `:363`, `FirstRunPrompt` `:480`. `src/secrets/mod.rs`: `SERVICE` `:35`,
  `NATIVE_STORE_DISABLED = true` `:46`, `store`/`retrieve`/`delete` `:68`/`:86`/`:102`, each short-circuited by the switch. `src/llm.rs` uses
  `secrets::retrieve`/`delete`/`store` at `:608`/`:986`/`:999`. `src/i18n.rs`: `pk_build_package_note` field `:458`, English `:2998`; tests
  `no_empty_ui_translations` `:15586`, `non_english_is_actually_translated` `:15630`. `src/grace_host.rs`: `the_system_kb_teaches_every_builtin_inline`
  `:4758`, `prebuilt_chunked_kb_matches_the_published_documentation` `:4780`.
- **ABI and ledger.** `specs/WASM-ABI-v1.md`: §9 error codes `:121` (`-6` host refused, permission or policy), §12 amendments `:158` (A10–A13 at
  `:173-176`), §12.1 the application object `:178`. `specs/WASM-PROGRESS.md:41`: the WASM-012 row, P ✔ only.
- **WASM-008 dependency.** `specs/WASM-008-webview-preview/tasks.md` T39 defines `DeviceRunner::{detect, list, run(dev_url), log}` with a "no device
  tools" implementation; T30 adds `preview_browser`. Both unchecked.
- **Nothing exists yet** for the shell, any `tauri` dependency, a mobile toolchain/devices/features/build module, or the signing or device windows.

**Binding decisions (operator).**
- Spec R3a (2026-10-23): Preview and Run of a Mobile project are the run on an emulator, simulator or device; without one, WASM-008's browser fallback
  at a phone width with a notice.
- Q2 (2026-10-22): no remote builds; iOS needs a Mac and is offered as unavailable elsewhere, with the reason.
- Q3: the minimum OS is the last version released in 2024, raised one year per major (D-MinOS).
- Q4: one shell; phone and tablet differ only by the responsive breakpoints (D-Tablet).
- Q5: Tauri and its plugins track the IDE (D-Pin).
- Q6: certificate pinning is optional and belongs to WASM-013. Nothing here.

**Open — working assumptions, never decided here.**
- **O1 — spec Q1, "yes", *to confirm*.** Do camera, location, notifications, biometric unlock and barcode scanning enter 2.0? Working assumption: none is
  implemented until the operator confirms. T25 is a gated template and stays unchecked.
- **O2 — PQ1, one platform per project.** Working assumption: `MobileSettings::platform` as today; R1's "each platform the project selected" is that one.
- **O3 — PQ2, may the IDE install the Tauri CLI?** Working assumption: offered with consent, like `rustup target add`. Xcode, the SDK, the NDK and the
  JDK are never installed.
- **O4 — PQ3, the first prompting feature.** Working assumption: AC6's device half is proved on the first Q1 feature the operator confirms; its logic
  half (a refusal arrives as `-6`) is proved now (T15).
- **O5 — PQ4, secure storage without native code.** Raised only if T1 finds no qualifying published plugin or crate. If raised, the secure-storage parts
  of T7, T8 and T15 **stop** and wait for the operator; the rest proceeds.
- **O6 — PQ5, R3a's target choice and fallback trigger** (D-Preview): last-chosen, else the single running target, else the list; fall back only when no
  tool or no target exists, never on a failed start. A planner's reading, implemented as such in T20; the operator may overrule.
- **O7 — WASM-008's `preview_browser` in `[mobile]`** (WASM-008 Q7, decided in its plan, not by the operator). Followed; only WASM-008's launcher changes
  if overruled.
- **O8 — WASM-017 Q5 (not answered).** Isolation in the shell. Working assumption: `Isolation::Off`, as `from_manifest` already sets.
- **O9 — D-Store** bypasses the secret store's kill switch for `mobile-signing/` accounts only. The plan calls this out for the operator's review at
  `/analyze`. T10 is not started before that review.
- **O10 — the `[mobile]` TOML of plan §3 is not valid TOML** (found while writing the first version, still true in the revised plan): `features` is both
  an array (`features = [...]`) and a table (`[mobile.features.secure-storage]`), which TOML forbids. Working assumption: the rationale texts move to
  `[mobile.rationale.<feature>]`, one entry per language. No requirement changes. T3 writes the correction back into plan §3.

**Guardrails at `/implement`.**
- The two new IDE windows open at an explicit default size and change only when the user drags. Test the frames in six languages.
- No DataGrid or control-border code is touched. If that changes, call the matching guardrail agent.
- **Credentials:** never open, generate or commit real signing material. Test passwords are fixed fake strings. Scan staged diffs before any commit.
- **Disk:** `df -h .` before each test run. Targeted `cargo test -p … --no-fail-fast`; read every `test result` line. Android/iOS runs are `#[ignore]`d.
- **PRIME DIRECTIVE:** no Kotlin, Swift, Java, Gradle or script file is added to the repository; Tauri's generated native trees live in the build folder.

## M0 — verify and baseline

- [x] **T1 — V0: verify every `[verify]` item and probe the local toolchain** (all R; plan §6 V0, Risks 1–4, 6, 9)
  - **Done 2026-11-06:** plan §9, 31 rows for the 31 `[verify]` markers (`grep -o "\[verify" plan.md | wc -l` = 31), the probe output, the version
    set, the secure-storage choice (keyring-core; **O5 not raised**) and the desktop's path for an unknown `COBOL-` name (silent: fixed in T5).
    Changes it forced: A26 (not A14); the lifecycle is `WindowEvent::Suspended`/`Resumed` (no mobile `RunEvent`); no iOS device-family key;
    D-Sign through `System.getenv` in the staged Gradle script; `INVOKE COBOL::…` now becomes the same `CALL` as the inline form.
  - Files: `specs/WASM-012-mobile-shell-tauri-2/plan.md` only (a new "§9 V0 record").
  - Do:
    - For each `[verify]` in the plan, record the official source (Tauri v2 documentation, crates.io `Cargo.toml`, Apple/Android documentation), the
      date, the answer and the change it forces. The items: `tauri android build --apk --aab` and the iOS archive/export; the Android signing route
      (D-Sign: can Gradle read the passwords from the environment?); `app.security.csp` and the shell origins; the opener plugin; the Back interception
      API and the mobile `RunEvent`s; the iOS device-family key; `tauri icon`; `xcrun devicectl`; the `apksigner`/`codesign` output; the 2024 floors
      (Android 15 / API 35, iOS 18); whether a Tauri command call is asynchronous (do the `Secure*` verbs suspend?); the iOS rationale; the CLI flag for
      a single target; Apple's privacy-manifest categories; the recorded output formats of `emulator -list-avds`, `adb devices -l`,
      `xcrun simctl list devices --json`.
    - Evaluate secure-storage candidates (a published Tauri plugin, or `keyring` with Android/iOS backends) against the guards: licence, no
      `ring`/`aws-lc-rs`, MSRV ≤ 1.92, maintenance. Choose one, or raise O5.
    - Choose the Tauri version set that builds on 1.92 (D-Pin) and the Tauri CLI version.
    - Run the local probe: `cargo tauri --version`, `rustup target list --installed`, `xcodebuild -version`, `java -version`, the SDK/NDK folders.
    - Read the desktop path for an unknown `COBOL-` name (input to T5) and record it.
    - Any false statement changes the plan's design at the point it names, before T2 starts.
  - Verify: every `[verify]` in `plan.md` has a V0 row (count the markers and the rows; the two counts are equal). The probe output is pasted into the
    record. `git status` shows only `plan.md` changed by this task.

- [x] **T2 — Baseline: the Web and Desktop build tests are green before anything changes** (R14; AC10 baseline)
  - **Closed without the run before the change** (the work started from T1 and the code). Its purpose — telling a red caused by this spec
    from one that was there before — needs no baseline now: T23 ran the same four commands on 2.0.291 and on 2.0.292 and every test was
    green (counts under T23), so there is no red to attribute.
  - Files: none changed.
  - Do: run the tests AC10 later re-runs unchanged; record their counts here. A red test is fixed in its owning crate before T3.
  - Verify:
    - `cargo test -p cobolt-compiler project_kind --no-fail-fast` (including `project_kind_build_tests`);
    - `cargo test -p cobolt-project-tools --no-fail-fast`;
    - `cargo test -p cobolt-web-bundle --no-fail-fast`;
    - `cargo test -p cobolt-wasm --no-fail-fast`.

## M1 — the manifest and the feature table

- [x] **T3 — `[mobile]` gains identity, features and signing names; `min_os` gets its floor** (R4 names, R6, R14; AC4 part, AC10; D-MinOS, D-BuildNo;
  O10)
  - **Done:** `project_kind.rs` — the fields, `validate_app_id`, `build_number`, `min_os_floor` (Android `35`, iOS `18.0`), `resolved_min_os`,
    `keystore_inside_project`, `display_name`; plan §3 corrected for O10 (`[mobile.rationale.<feature>]`). Tests:
    `the_full_mobile_table_round_trips`, `an_old_mobile_table_loads_with_every_new_default_and_saves_unchanged`, `app_ids_are_reverse_dns`,
    `min_os_resolves_to_the_floor_and_refuses_below_it`, `the_build_number_comes_from_the_version`, `a_keystore_inside_the_project_is_refused`.
  - Files: `crates/cobolt-compiler/src/project_kind.rs`; `specs/WASM-012-mobile-shell-tauri-2/plan.md` §3 (the O10 correction only).
  - Do:
    - Add to `MobileSettings` `:446`, **after** `call_timeout_ms` `:473`: `app_id: String`, `app_name: BTreeMap<String, String>`, `icon: String`,
      `splash: String`, `features: Vec<String>`, `rationale: BTreeMap<String, BTreeMap<String, String>>` (feature → language → text, per O10) and
      `signing: MobileSigning` (`android_keystore`, `android_alias`, `apple_team_id`, `apple_identity`, `apple_profile`). Each has `#[serde(default)]`
      and `skip_serializing_if` so an old `[mobile]` table saves unchanged. `MobileSettings::new` `:483` fills the defaults.
    - `fn validate_app_id`: two or more segments, each `[A-Za-z][A-Za-z0-9_]*`.
    - `fn build_number(version)` = `major*10000 + minor*100 + patch`.
    - `fn min_os_floor(platform)` with the values T1 confirms; `auto` resolves to the floor; a lower value is an error naming the floor. Rewrite the
      `min_os` doc comment to D-MinOS's meaning.
    - `fn keystore_inside_project(project_dir)`, refusing a keystore under the project folder.
  - Verify: `cargo test -p cobolt-compiler project_kind --no-fail-fast`. New tests: round-trip of the full (corrected) plan §3 table; an old `[mobile]`
    table loads with every new default **and saves byte-for-byte**; `app_id` accepts `com.example.app`, refuses `example`, `1com.x`, `com..x`; `min_os`
    below the floor refused with the floor named, `auto` → floor; `2.0.29` → `20029`; a keystore path inside the project folder refused.

- [x] **T4 — `mobile_features.rs`: the feature table and the AST scan** (R7, R9, R13; AC5 scan half; D-Scan)
  - **Done:** the three rows, `scan`, `undeclared`, `unknown_declared`. Tests: `every_row_of_the_table_is_complete`,
    `the_scan_finds_both_spellings_at_their_lines_and_ignores_comments_and_strings`, `an_undeclared_use_is_refused_with_its_line`. The Mobile
    package build also checks the bundle's `builtins` against the declared features (`mobile_build::check`).
  - Files: `crates/cobolt-compiler/src/mobile_features.rs` (new); `src/lib.rs` (the `mod` line beside `:57-63`).
  - Do:
    - `FEATURE_SET_VERSION`.
    - Rows of the first set: `secure-storage` (`SecureStore`, `SecureLoad`, `SecureDelete`), `open-address` (`Navigate`/`OpenUrl`, plus
      `mailto:`/`tel:`/`sms:`) and `deep-link`. Each row carries its verbs, Android permissions, iOS usage key (or none), Tauri plugin and capability
      permissions, and its R13 checklist lines.
    - `scan(programs) -> Vec<FeatureUse { feature, verb, file, line }>`: walks parsed ASTs (`cobolt_parser`, as `backend_build` does) for
      `Stmt::Call` with the literal `COBOL-<VERB>` — what both `INVOKE COBOL::…` and the inline `COBOL::Verb(…)` become (`cobol_object_call`,
      `stmt.rs:3430`) — whose verb belongs to a feature. Controls that imply a feature: none in the first set.
    - `undeclared(uses, declared)` → `file:line: uses <feature> (COBOL::<verb>); add it to the project's device features`.
    - The five Q1 features are **not** rows (O1).
  - Verify: `cargo test -p cobolt-compiler mobile_features --no-fail-fast`. New tests: every row is complete; the scan finds the inline and the `INVOKE`
    forms at the right `file:line` and ignores a comment and a string literal; an undeclared use gives the exact message; it prints
    `N uses of M features in K programs`.

## M2 — the device verbs in every host (interpreter/binary parity)

- [x] **T5 — A device verb is never silently ignored on the desktop** (R9; parity; plan §2 `cobolt-runtime`, §7)
  - **Done:** the read found the desktop silent (a warning, then on). The refusal is in the interpreter's built-in dispatch, shared by
    `rcrun run-form`, embedded child forms and `run_form_app` (the compiled binary): "COBOL::SecureStore at line N is available in a Mobile
    application only …"; a call that names `ON EXCEPTION` runs it instead. Test `crates/cobolt-runtime/tests/test_mobile_device_verbs.rs`
    (`each_device_verb_is_refused_with_where_it_is_available`).
  - Files: decided by the read — `crates/cobolt-runtime/src/interpreter.rs` (the built-in dispatch, e.g. near `:11567`) if the refusal belongs at run
    time, or `crates/cobolt-semantic/src/calls.rs` (`:123`, which today returns early for every `COBOL-` name) if Check is the one place all three hosts
    share. **Not** `BUILTINS` (plan §2: the device verbs are not desktop built-ins).
  - Do:
    - Read (T1's note) what `rcrun run-form`, an embedded child form and the compiled binary (the `stage_sdk`-staged runtime) do with
      `CALL "COBOL-SECURESTORE"`. Record the path and its answer here.
    - If any host is silent, add "COBOL::SecureStore is available in a Mobile application only" (and the same for `SecureLoad`, `SecureDelete`) at the
      one place all three hosts share. If all three already give a non-silent answer, record it and change nothing.
  - Verify: `cargo test -p cobolt-runtime --no-fail-fast` (and `-p cobolt-semantic` if Check carries it). A new test runs a desktop program calling each
    verb and asserts the same message from the interpreter path and from the path the compiled binary uses (the binary's own run is `#[ignore]` with its
    command if it needs a build).

- [x] **T6 — ABI amendment A14** (R8, R9, R10, R11; D-Errors)
  - **Done as A26** (`specs/WASM-ABI-v1.md` §12, and a §12.1 row): the three verbs, their limits, `SecureStatus` `0`/`1`/`-6` and `SecureValue`,
    the wait, `-6` at once outside the shell (no run-time error), the Web refusal, the deep link to the A21 route or address, the `pr-shell`
    `paused`/`resumed` events (reserved: the player does not act on them yet). The manifest does not validate built-in names, so
    `cobolt-wasm-abi` is unchanged. Insets and Back are not ABI: Back is the web view's history (V0 row 11), insets wait for T9.
  - Files: `specs/WASM-ABI-v1.md` (§12 row A14 after `:176`; §12.1 rows); `crates/cobolt-wasm-abi/src/manifest.rs` (only if the manifest validates
    built-in names).
  - Do: write A14: `COBOL::SecureStore(key, value)`, `SecureLoad(key)` → value or empty, `SecureDelete(key)` on ctrl 2, and whether they suspend (T1);
    deep-link delivery to `me::GetParam`/`GetParams`; the shell's events to the page (insets, Back, lifecycle) as the player sees them; refusal is `-6`,
    no new code; the ABI version stays 1.
  - Verify: `cargo test -p cobolt-wasm-abi --no-fail-fast` green. The A14 row names every verb T7 and T15 implement (listed here by reading).

- [x] **T7 — The generator: device verbs in a Mobile module, refused in a Web module** (R7, R9, R14; AC5, AC10; D-WebRefuse, D-Scan)
  - **Done:** `MOBILE_VERBS` in `lower_remote.rs`, the Mobile option reaches the generator, `SecureLoad( key item )` moves the answer on resume.
    Tests in `crates/cobolt-wasm/tests/forms.rs`: `the_phone_shells_verbs_compile_in_a_mobile_module_and_each_waits`,
    `a_web_module_refuses_the_phone_shells_verbs_and_is_otherwise_unchanged`.
  - Files: `crates/cobolt-wasm/src/lower_remote.rs` (`APP_VERBS` `:26` gains `SECURESTORE`, `SECURELOAD`, `SECUREDELETE`; `suspends` `:53` per A14);
    `src/lower_form.rs` (`builtin_call` `:441`, the app-verb arm `:494`); the generator's options (the project kind must reach it); the manifest's
    `builtins`.
  - Do: in a Mobile build the three verbs lower onto `CTRL_APP` like the other application verbs and appear in `builtins`. In a Web build they are
    refused at the line, "available in a Mobile project only" (the `Refused` shape of WASM-002 R3).
  - Verify: `cargo test -p cobolt-wasm --no-fail-fast`. New tests: a Mobile fixture compiles with the verbs in `builtins`; a Web fixture is refused at
    the line; `suspends` agrees with A14; T4's scan set equals the `builtins` set on the fixtures (D-Scan); an existing Web fixture's module is
    byte-identical to before (AC10).

## M3 — the shell and its generation

- [x] **T8 — `crates/cobolt-mobile-shell/`: the Rust shell crate and its dependency guard** (R9 secure storage, R10, R11, R12; D-Shape, D-Native, D-Pin;
  - **Closed 2026-11-09 by the operator's ruling, not verified:** the acceptance criteria it was to settle stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  Risks 2–4)
  - **Partly done:** the crate (own workspace, excluded from the root), the version set of V0, the navigation guard (own origins stay, the rest goes
    to the opener), the lifecycle as `pr-shell` events, deep links, the three secure-storage commands over keyring-core, and
    `tests/dependency_guard.rs`. Run on the emulator: the package builds from it and the secure-storage commands work. **2.0.289:** the end of the
    application. Back on the first form closes Tauri's activity (its own Back handler calls the activity's; measured on API 37 for a start by name
    and by the launcher's intent alike), the window's `Destroyed` ends the event loop, and tao's `std::process::exit` then made ART abort the
    process (signal 6). `on_run_event` ends the process with `_exit(0)` on `RunEvent::Exit` (Android only):
    measured "Process … exited cleanly (0)", and the next start is a fresh, working process. Insets need no forwarding: the web view is drawn edge
    to edge and `env(safe-area-inset-*)` answers there (T9). **Left:** keyboard forwarding (`--pr-keyboard`).
  - Files: `crates/cobolt-mobile-shell/Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `tests/dependency_guard.rs` (all new); root `Cargo.toml` `exclude` `:45`
    (add the crate beside `cobolt-web-player`); `crates/cobolt-compiler/src/` (the `TAURI_CLI_VERSION` constant, where `mobile_toolchain.rs` reads it).
  - Do:
    - The mobile entry point: the navigation guard (own origin stays; anything else → the opener; `mailto:`/`tel:`/`sms:` → the OS); inset, keyboard,
      Back and lifecycle forwarding to the page; the secure-storage commands through T1's chosen plugin or crate.
    - Pin the version set from T1, `default-features = false` where needed.
    - If O5 is raised, write the crate without the secure-storage commands and leave that part open.
  - Verify:
    - `cargo test --manifest-path crates/cobolt-mobile-shell/Cargo.toml --test dependency_guard`: no `ring`/`aws-lc-rs` in the lock, licences in the
      allow-list, every `rust-version` ≤ 1.92, counts printed. If the library cannot build for the host, the guard reads the lock as text from
      `crates/cobolt-compiler/tests/`; record the choice here.
    - `git ls-files crates/cobolt-mobile-shell` (after the pipeline stages it) or `ls -R` lists `.rs`, `Cargo.toml` and `Cargo.lock` only.
    - `cargo check --workspace` is unaffected.

- [x] **T9 — The bundle for a Mobile build: `viewport-fit=cover`, `--pr-safe-*`, CSP for the shell origin** (R10, R12; AC7 static, AC9 static; D-Insets)
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC7, AC9 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - **Partly done (2.0.289):** the insets live in the player's base CSS (`crates/cobolt-web-player/src/base_css.rs`), not in a Mobile-only
    writer: `.pr-root` and a full-screen dialog stand inside `--pr-inset-*` = `max(env(safe-area-inset-*), var(--pr-safe-*))`, and a strip of
    `--pr-statusbar` (light `#f4f5f7` when the phone is in light mode, transparent in dark) is painted under the status bar. **Deviation, on
    purpose:** every kind gets it, because the host page already declares `viewport-fit=cover` for every kind and `env()` answers 0 on a desktop
    browser, so a Web page is unchanged there and is right on a phone's browser too. The web preview's phone frame sets `--pr-safe-top/right/
    bottom/left` (renamed from `--prc-safe-area-*`, which nothing read). Test
    `the_application_keeps_out_of_the_status_bar_the_notch_and_the_gesture_bar`; the player rebuilt, freshness 6/6. Measured on the emulator
    through the web view's DevTools: the root starts 24 CSS px down (the 63 px status bar at DPR 2.625), the strip is `#f4f5f7`, and the status
    bar's dark icons read on it. **Caveat:** the icons take the phone's light or dark mode at the activity's start; a switch while the
    application runs keeps them (the manifest's `configChanges` includes `uiMode`, so the activity is not re-created).
  - **Measured on Android, no change needed (2026-11-07, 2.0.296):** the CSP for the shell origin — the bundle's own strict policy (`<meta>`,
    `connect-src 'self'` plus the backends) already covers the page's origin in the shell (`http://tauri.localhost`), and Tauri's IPC on Android
    goes through the web view's `postMessage`, not a fetch to an `ipc:` origin: on the 2.0.293 package, a `secure_load` call through
    `__TAURI_INTERNALS__.invoke` answered (`null`, no such key) with **0** `securitypolicyviolation` events (read through the web view's
    DevTools). `--pr-keyboard` — the Android web view shrinks the visual viewport for the keyboard (578 of 914 CSS px) and keeps the focused box
    in view (AC8's measurement of 2026-11-07), so the page needs no variable there. **Left (iOS, not run here):** the WKWebView origin
    (`tauri://localhost`) and its IPC under the policy, and whether the keyboard needs `--pr-keyboard` there.
  - Files: `crates/cobolt-web-bundle/src/csp.rs` (a shell-origin option beside `CspOptions` `:26`); the bundle's host-page and base-CSS writer
    (`viewport-fit=cover`; `max(env(safe-area-inset-*), var(--pr-safe-*))`; `--pr-keyboard`), for Mobile only.
  - Verify: `cargo test -p cobolt-web-bundle --no-fail-fast`. New tests: a Mobile bundle's page has `viewport-fit=cover`, the four `--pr-safe-*` names
    and `--pr-keyboard`; the shell-origin CSP equals `strict()` plus T1's origin; a Web bundle is byte-identical to before (AC10).

- [x] **T10 — Strict secret-store calls for signing** (R4; AC4; D-Store) — **after the operator's review of O9**
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC4 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - Files: `crates/cobolt-ide/src/secrets/mod.rs`.
  - Do:
    - `store_required`/`retrieve_required`/`delete_required`: only accounts under `mobile-signing/`; ignore `NATIVE_STORE_DISABLED` `:46`; return
      `Err(reason)` when the platform store is unavailable; never fall back.
    - `list_accounts(prefix)` only if the macOS, Windows and Linux layers can answer it; otherwise record that here (the Signing window then lists the
      manifest's names).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide secrets --no-fail-fast`. New tests: a strict call on an unavailable store is `Err`; an account
    outside `mobile-signing/` is refused; `llm.rs`'s path (`:608`/`:986`/`:999`) still falls back exactly as before (Risk 8).

- [x] **T11 — `mobile_build::generate`: the shell project, configuration, capabilities and platform declarations** (R1, R6, R7, R10, R12, R13 data;
  - **Closed 2026-11-09 by the operator's ruling, not verified:** the acceptance criteria it was to settle stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  AC4 scan, AC5, AC7 static, AC9 static; D-Shape, D-Tablet)
  - **Partly done (Android):** `mobile_build::{check, stage, patch_android_project}` write into `build_staging_dir(None, "<crate>-android")`:
    `frontend/` (the bundle), `shell/` (embedded with `include_str!`, so an installed IDE needs no source tree), `app/` (Cargo.toml, build.rs,
    `src/lib.rs`, `tauri.conf.json` with `frontendDist`, no `devUrl`, the identity, version and build number, the minimum OS; the capability file
    with exactly the declared features' permissions), the Android `screenOrientation` and the localized `app_name`. Nothing is written into the
    project folder except `dist/mobile/`. Tests in `tests/mobile_shell.rs` (6). **Left:** the iOS declarations (usage keys, orientations), the CSP
    of T9, the AC4 byte scan of the staging folder for the fake passwords.
  - Files: `crates/cobolt-compiler/src/mobile_build.rs` (new; pattern `backend_build.rs`); `src/lib.rs` (`mod` line, re-exports);
    `crates/cobolt-compiler/tests/mobile_shell.rs` (new; Android and iOS fixture projects built in code in a temp dir).
  - Do: `generate(project, bundle_dir, uses, profile)` writes into `build_staging_dir(None, "<name>-mobile")`:
    - the staged shell crate (as `stage_sdk` stages the platform crates);
    - `tauri.conf.json`: `frontendDist` → a copy of the bundle, **no** `devUrl` in release, the CSP from T9, the identity (`app_id`, translated names,
      version, build number from T3); a dev profile takes a given dev address and changes nothing else;
    - the iOS device family when `device = "tablet"`;
    - the capability file: exactly the declared features' commands, main window only, no remote origin;
    - Android permissions and `screenOrientation`; iOS usage keys with the rationale texts and `UISupportedInterfaceOrientations`.

    Nothing is written into the project folder. An undeclared use fails before generation with T4's message.
  - Verify: `cargo test -p cobolt-compiler --test mobile_shell --no-fail-fast`, each test printing what it compared: `frontendDist` and no `devUrl`
    (release), the dev address only (dev); the CSP equals T9's; the capability commands equal the declared features' and no remote origin (AC9 static);
    permissions and usage keys exactly the declared ones (AC5); an undeclared use fails at its `file:line` (AC5); orientation as set (AC7 static); with
    known fake passwords in memory, a byte scan of the project folder **and** the staging folder finds none (AC4 scan).

- [x] **T12 — Icons and splash from one source image** (R6)
  - **Done (icons):** Check refuses an icon that is not a square PNG of at least 1024 px, with its size (`every_problem_is_named_before_any_tool_runs`:
    a 512 × 512 icon); the build runs `cargo tauri icon` after `android init` (run: the Android mipmaps are written); no icon → the PowerRustCOBOL
    icon. A splash image is not generated: Tauri 2 has no splash step, the web view shows the page's own first paint.
  - Files: `crates/cobolt-compiler/src/mobile_build.rs`.
  - Do: refuse an icon that is not square or smaller than 1024 px, with the reason. Produce the platform sets through `tauri icon` as T1 confirmed; if
    the CLI cannot be used, resize in Rust with an image crate the workspace already has, if any; record which.
  - Verify: `cargo test -p cobolt-compiler --test mobile_shell icon`. A non-square and a 512 px image are refused with their reasons; the generated
    set's file list is printed (the CLI half `#[ignore]` when the CLI is absent).

- [x] **T13 — `build_mobile` and `run_dev`: Web build → generate → Tauri CLI; release-signing checks; store checklist** (R1, R3, R3a, R4, R5, R13;
  - **Closed 2026-11-09 by the operator's ruling, not verified:** the acceptance criteria it was to settle stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  AC1 driver, AC4 refusal; D-Sign, D-Dev)
  - **Partly done:** `mobile_build::build_android` (check → stage → `cargo tauri android init --ci` → patch → `cargo tauri icon` →
    `cargo tauri android build --ci [--debug] --apk --target <abi>` → `dist/mobile/android/<Name>-<debug|release>.apk`), every tool line in
    `package.log`. A release is refused before any process without the keystore, the alias or a password, or with the keystore inside the project;
    the passwords reach Gradle only as `PRC_ANDROID_STORE_PASSWORD`/`PRC_ANDROID_KEY_PASSWORD` in its environment (`patch_signing`, tested). **Run**
    (debug): 57 s cold, 8 s warm. **Left:** the `.aab`, the `apksigner` check after a release, the R13 checklist output, `run_dev` (the dev-server
    shell: the Run builds the package from the bundle instead), a release with the operator's key (T24 AC4).
  - Files: `crates/cobolt-compiler/src/mobile_build.rs`; `crates/cobolt-compiler/tests/mobile_shell.rs`.
  - Do:
    - `MobileBuildOptions` carries the profile and the signing secrets **in memory**; `MobileBuildResult` the artefacts (`dist/mobile/<platform>/`) and
      the checklist.
    - Before any process starts, a release build is refused if the signing names are unset, a password is absent, or the keystore is inside the project.
    - Passwords reach the `tauri android build` child **through its environment only** (the route T1 confirmed).
    - After the build: `apksigner verify --print-certs` must not show `CN=Android Debug`; `codesign -dvv` must name a distribution identity. A failure
      refuses the build with its reason.
    - The R13 checklist is printed from the feature table. Debug builds sign automatically. The compiler never touches the secret store.
    - `run_dev(project, platform, target, dev_url)`: the dev-mode shell against a given dev-server address, reusing the staging folder (plan §3,
      Risk 5) — used by Run on device (T19) and Preview/Run (T20).
  - Verify: `cargo test -p cobolt-compiler --test mobile_shell --no-fail-fast`. New tests: release with no names, and with a missing password, refused
    before any process (a fake `PATH` with a stub `cargo`/`tauri`, written in Rust, proves it never ran); the first set's checklist printed and compared;
    the child's environment carries the passwords and no staging file does; `run_dev` generates the dev configuration with the given address and reuses
    an existing staging folder. `#[ignore]` `android_debug_build_produces_apk_and_aab`, its exact command in its doc comment (runs in T24).

## M4 — toolchain, player bridge and devices

- [x] **T14 — `mobile_toolchain.rs`: detection, next steps, safe installs** (R2, R3a input; AC2)
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC2 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - **Partly done, in `mobile_build.rs`:** `Toolchain::detect(abi)` names every missing piece with how to get it (`Missing { what, how }`): the
    Tauri CLI at `TAURI_CLI_VERSION` (and the `cargo install` line), the Rust target, the SDK, the NDK, Java 17–21 (`JAVA_HOME`, `java_home -v`,
    Android Studio's runtime). The parsers are tested from recorded output. **Left:** `install` with consent (O3), the iOS rows, the tool × state
    table of AC2.
  - Files: `crates/cobolt-compiler/src/mobile_toolchain.rs` (new); `src/lib.rs`.
  - Do:
    - `MobileTool`/`MobileToolStatus` rows (present / missing / wrong version; purpose; next step) from `detect(platform)`: `ANDROID_HOME`/
      `ANDROID_SDK_ROOT`, then the default SDK folder; `NDK_HOME`, then `ndk/`; `JAVA_HOME`, then `java -version`; on macOS only `xcode-select -p` and
      `xcodebuild -version`; `rustup target list --installed`; the Tauri CLI against `TAURI_CLI_VERSION`.
    - `install(tool)` accepts only Rust targets (`rustup target add`) and, per O3, the Tauri CLI; Xcode, the SDK, the NDK and the JDK are an error.
    - Off macOS, iOS is `Unavailable { reason }` (the reason `tools/build.rs:131` already gives).
  - Verify: `cargo test -p cobolt-compiler mobile_toolchain --no-fail-fast`, with a fake environment (variables plus a temp-dir `PATH` of stub
    executables written by the test in Rust): each tool hidden in turn is named with its purpose and next step; `install` refuses Xcode/SDK/NDK/JDK;
    iOS unavailable off macOS with the reason (`cfg`-driven); a tool × state table printed (AC2).

- [x] **T15 — The player's shell bridge** (R8, R10, R11, R12; AC6 logic, AC7 logic, AC8 logic, AC9 logic)
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC6, AC7, AC8, AC9 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - **Partly done:** `crates/cobolt-web-player/src/shell.rs` (detects the shell, calls its commands, reads the answers) and the three `SECURE*` arms
    of `host.rs` (A26); a refused call answers `-6`; outside the shell `-6` at once with a console line. Tests in `src/host/shell_tests.rs` (2); the
    player rebuilt into `assets/web-player/`. Run on the emulator through the page's own bridge. **Left:** Back is the web view's history (no
    shell event needed, V0 row 11); ~~the lifecycle reconnect on `resumed` (AC8)~~ done in 2.0.293 with T25b (`net::Client::resumed`, the
    page's `pr-shell` / `visibilitychange` listeners; the `expired` → sign-in half is the existing refresh path, not re-tested here); the
    spec 037 events for paused/resumed (no spec 037 folder in `specs/`); external addresses through the opener from the player (the shell's
    navigation guard already does it). **Measured 2026-11-06 (API 37):** Back on the first form closes the application (Tauri's own Back handler
    calls the activity's) whether it was started by name or from its icon; "ask the shell to background" on an empty stack would need a native call
    (`moveTaskToBack`), which D-Native rules out — the shell ends the process cleanly instead (T8), and the next start is a fresh one. The
    hardware Escape key acts as Back when the page does not consume it.
  - Files: `crates/cobolt-web-player/src/` (a new `shell.rs`; `host.rs` `verb` `:1605`: three `SECURE*` arms beside the application verbs, and the
    `NAVIGATE`/`OPENURL` `:1639` and `COPYTOCLIPBOARD` `:1647` effects routed through the shell when present — WASM-006 R11; the Android rationale
    dialog; Back → the navigation pop of WASM-011 R13, empty stack → ask the shell to background; lifecycle → spec 037 events, `net.rs` reconnects on
    resume); `assets/web-player/` (rebuilt).
  - Do: Rust through `wasm-bindgen`/`js-sys`. Detect the shell; call its commands only when present. A refusal answers `-6`. Outside the shell (a Web
    page, the browser fallback) nothing changes.
  - Verify:
    - `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib --no-fail-fast`, with a fake shell object: Back pops and an empty stack asks
      to background (AC7); a refusal becomes `-6` → `ON EXCEPTION` (AC6); an external address goes to the opener, an own-origin one does not (AC9);
      resume reconnects and an `expired` answer reaches sign-in (AC8); without a shell, `Navigate` and `CopyToClipboard` give today's effects.
    - Rebuild: `cargo run --manifest-path crates/cobolt-web-player/Cargo.toml --example build_player -- --out assets/web-player`; then
      `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --test freshness` green.

- [x] **T16 — `mobile_devices.rs`: list, start, install and launch, log, stop** (R3, R3a input; AC3 logic; D-Dev)
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC3 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - **Partly done (Android):** `Adb::{find, devices, abi, install, launch, stop, logcat}`, `parse_devices`, `is_application_log` (the page's
    console, the shell's own lines, a crash; the web view's `[INFO:…]` noise left out). Tested from the emulator's recorded output. **2.0.289:**
    `launch_args` starts the application with the launcher's intent (`MAIN`/`LAUNCHER`), so a Run starts the task a tap on the icon would;
    `application_log` gives the Output the line as the program wrote it (a console line without level, tag, process number and the web view's
    `File: … - Line … - Msg:`; a shell line without its prefix; a crash whole) — 7 recorded lines tested. **2.0.290:** the
    emulators — `Emulator::{find, avds, start}` (`emulator -list-avds`; `emulator -avd <name>` in its own process group, so it outlives the IDE
    and an interrupt of the IDE's terminal), `Adb::{avd_name, wait_for_avd, ready_to_install, wait_until_ready}` (`adb emu avd name`,
    `getprop sys.boot_completed` **and** `service check package` — measured: on the starved emulator Android's watchdog killed the system
    server ("Blocked in handler on ui thread for 68s") while the package was being built, `sys.boot_completed` stayed `1`, and the install
    failed with "Can't find service: package"; the Run now waits for the package service before every install), and
    `targets` (the devices running, then the emulators that can be started, less those already running); test
    `the_emulators_that_can_be_started_are_read_from_the_sdk` from this Mac's recorded answers. **2.0.291:** the log follows the
    application's own processes only (`logcat --uid=<its user id>`, from `pm list packages -U`; `parse_package_uid`, `logcat_args`) — before,
    another application's crash (`E/AndroidRuntime`, seen when Android restarted its system) would have read as this one's; the user id
    survives a new process (the application started again from its icon). A start that answered a failure is asked again only when it was
    not taken (`has_activity`, `dumpsys activity activities`: measured, a busy emulator answered an accepted start as a failure after 18 s,
    and the second start replaced the activity, which ends a Tauri application). **Left:** the iOS simulators and devices, the
    LAN opt-in, `adb reverse` for the dev server (D-Dev: the Run builds the package from the bundle instead).
  - Files: `crates/cobolt-compiler/src/mobile_devices.rs` (new); `src/lib.rs`.
  - Do:
    - `DeviceTarget` (kind emulator/simulator/device, running or startable) from `emulator -list-avds`, `adb devices -l`,
      `xcrun simctl list devices --json` and `xcrun devicectl` (per T1); `list(platform)` never errors for "no target" (empty list).
    - `start`, `install_and_launch` (through T13's `run_dev`, the dev server of `cobolt_project_tools::web::devserver`, `adb reverse tcp:<port>
      tcp:<port>` on Android), `log_stream` (a `Receiver<String>`: `adb logcat --pid=<pid>`; `xcrun simctl spawn <udid> log stream --predicate`),
      `stop`.
    - A physical iOS device needs the LAN address: opt-in per IDE session and project, with a warning (the opt-in itself is asked by the IDE, T19/T20).
    - Process spawning only; no UI.
  - Verify: `cargo test -p cobolt-compiler mobile_devices --no-fail-fast`. Each list command's recorded output (T1), embedded as a string, parses into
    the expected targets; empty output → empty list; stub executables prove `start`/`stop`/`adb reverse` call the right commands.

## M5 — the IDE and the agent tool

- [x] **T17 — Build wiring: the IDE continues into the mobile half** (R1, R14; AC1 driver, AC10)
  - **Done (debug):** after a Mobile project's Web build the Preview asks its `DeviceRunner` for the package (`DeviceRunner::package`, a worker
    thread in `panels/mobile_runner.rs`); the Output shows the steps, the errors and "Package: <path> (<n> MB)"; `PackageNote` was said only for iOS until 2.0.411, when the iOS package step
    (`DeviceRunner::package` for every Mobile platform) replaced it; it is now said only when the project has no package step.
    Desktop and Web take their old path (`NoDeviceTools` answers `package()` with no package). The menu entries wait for T19/T21.
  - Files: `crates/cobolt-ide/src/app.rs` (`start_web` `:2715`; menu entries **Run on device…** and **Signing…**, Mobile projects only);
    `crates/cobolt-ide/src/panels/web_preview.rs` (`Note::PackageNote` `:78`/`:217`/`:730` gives way to the mobile build's notes).
  - Do: for a Mobile project, after the Web build succeeds, run `build_mobile` on a worker thread. Before it, show T14's missing tools with their next
    steps. For a release build, read the passwords with T10's strict calls; an unavailable store refuses with the reason. Progress and the checklist go
    to the Output panel.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`. A test of the dispatch: Desktop and Web take exactly their current path (AC10);
    Mobile continues into the mobile step; `PackageNote` is no longer said for a Mobile build that ran the shell step.

- [x] **T18 — `rcrun mcp` `build`: the mobile half, debug only** (R1, R2; D-Sign)
  - **Done, with one change:** a Mobile/Android project's `build` makes the bundle, then runs the package on the shared build slot and answers
    `package: {status, built, apk, bytes, elapsed_s, log}` (a build still running after 40 s answers `running`; calling `build` again waits on the
    same one); the missing tools come back with how to get each. **Changed:** `release: true` is not refused — it builds a signed release with the
    keystore and alias named in `[mobile.signing]` and the two passwords from the environment of `rcrun` (`PRC_ANDROID_STORE_PASSWORD`,
    `PRC_ANDROID_KEY_PASSWORD`; the build hands all four to Gradle as `PRC_ANDROID_*` variables), and is refused with what is missing otherwise. A build machine has no IDE and no secret store; the environment is the
    route D-Sign already uses, and no password is ever in a file. iOS answered `built: false` with the reason until 2.0.411; since then it
    builds the simulator application (`package: {status, built, app, bytes, elapsed_s, log}`, the same 40 s wait), names each missing tool
    with its command, and refuses `release: true` (the archive needs the Apple team, signing identity and provisioning profile).
  - Files: `crates/cobolt-project-tools/src/tools/build.rs` (the Mobile `package` answer `:126-135`, reason `:134`).
  - Do: a Mobile project builds a debug package and reports the artefacts, or the T14 missing-tool rows. A release request answers "release signing is
    from the IDE only". Off macOS, iOS keeps today's "only on macOS" reason. The device list is available as a tool output.
  - Verify: `cargo test -p cobolt-project-tools --no-fail-fast`. A release request answers the reason; a Mobile project without the tools answers the
    T14 rows; the Web and Desktop answers are unchanged.

- [x] **T19 — The Run on device window** (R3; AC3)
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC3 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - **Partly done (2.0.292, 2026-11-07):** `crates/cobolt-ide/src/panels/mobile_devices.rs` (`RunOnDevice`): the phones and emulators
    running and the emulators set up that Start starts first (`cobolt_compiler::mobile_devices::DeviceTarget` — id, label, running, the
    emulator's own name — built by `targets` from `adb devices -l`, `adb emu avd name` and `emulator -list-avds`; a phone that is not ready
    is left out), Start / Stop / Refresh, the empty-list text naming Android Studio's Device Manager; 560 × 400, `resizable(false)` +
    `fixed_size`, one grip in a foreground `Area`. Opened by D-Preview when a Run has no plain answer (`Note::ChooseDevice`,
    `WebPreview::take_device_choice`) and from **Run → Run on device…** (an Android Mobile project only). The choice is
    `WebPreview::chosen_devices` (per manifest, this IDE session; never written to the project); a choice made while an emulator was not
    running still finds it once it runs (`DeviceTarget::is` by the emulator's name). Stop is `DeviceRunner::stop` (the runner keeps where it
    started the application last, `started_on`), which also ends the log. The application's log goes to the Output as for every Run.
    Tests: `cargo test -p cobolt-ide --bin cobolt-ide -- mobile_devices mobile_runner _012 _008` — the list model (the session's choice by
    id and by the emulator's name, the first running, a refresh keeping or moving the selection, Start asking for the selected), the six
    languages, **the window keeping 560 × 400 over 12 frames in each language with a row longer than it**, the list from the stored height,
    Stop (nothing started; refused while busy; `started_on`), and T20 (b) below for "remembered for the session and not saved"; with the
    other touched tests 20 passed, 1 ignored (the device run, passed on the emulator separately). **Left:** the iOS LAN opt-in warning
    (D-Dev: there is no iOS device listing yet, T16's iOS half); the operator's look at the window in the six languages at its size.
  - Files: `crates/cobolt-ide/src/panels/mobile_devices.rs` (new); `panels/mod.rs`.
  - Do: target list (emulators, simulators, devices; an empty-list text with "create one"), Start/Stop, the log streamed to the Output panel, the iOS
    LAN opt-in warning (D-Dev). Opens at an explicit default size, never sized from available space. The chosen target is IDE session state, never
    written to `cobolt.toml`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide mobile_devices`. The list model is filled from a fake `DeviceTarget` list; Stop sends the stop;
    the choice is remembered for the session and not saved. Manual (the operator's): the window in six languages at its size.

- [x] **T20 — R3a: Preview and Run of a Mobile project go to the device; `MobileDeviceRunner`** (R3a, R3, R14; AC3, AC10; WASM-008 AC13/AC14,
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC3, AC10, AC13, AC14 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  the WASM-012 half; D-Preview, O6)
  - **Partly done:** WASM-008's `DeviceRunner` seam had landed; `MobileDeviceRunner` (`crates/cobolt-ide/src/panels/mobile_runner.rs`) implements
    it for Android — `detect` (adb), `list` (the ready devices), `run` (build for the device's ABI, install, stop, start, follow the log), `log`,
    and the new defaulted `package()`/`busy()`; `app.rs` gives it to a Mobile project's Preview, a Web project keeps `NoDeviceTools`. One worker at
    a time (a second request answers "busy"). Run on the emulator by the ignored test `the_example_runs_on_the_first_android_device` (T24).
    **2.0.289:** the log thread hands the Output `application_log`'s text, so a `DISPLAY` of the program reads there as it does on the desktop.
    **2.0.290:** `list` answers T16's `targets` (an emulator set up counts as a target, R3a), and `run` on an emulator that is not running
    starts it, says so in the Output (`pk_dev_emulator_starting`, `pk_dev_emulator_ready`, ×6), waits for it (at most 300 s) and goes on; the
    fallback note with no device and no emulator now says how to create one (`pk_prev_not_device_none` ×6, plan case (d)). **Interim, until
    T19's window:** with several targets and none chosen, the first is taken (a running one before one that must be started) instead of asking.
    **2.0.291:** the Run stops the application **before** the install, not after: measured on Android 17, an install over the application
    on screen covers it with a placeholder and starts it again when the update is done ("PackageUpdateController … launching the base
    intent"); on a busy emulator that came 38 s later, after the Run had started it, and replaced its activity — the application ended. A
    stopped application gets "no process with activities in it" and nothing is started again (measured: alive 90 s after the Run).
    **Decision tests:** (a) `a_mobile_preview_hands_the_address_to_a_device_and_reads_its_log_008` (detect, list, run on the first target with
    the dev address; no browser); (c) `a_mobile_preview_without_device_tools_opens_the_phone_page_008` (the missing tool reaches `NotTheDevice`,
    six languages); (d) and (f) `a_mobile_preview_with_no_target_and_a_web_preview_take_the_browser_012` (tools and no target: the browser page
    at 390 × 844 and the note names Android Studio's Device Manager in the six languages, nothing run; a Web project with a runner holding a
    device: 0 calls to it) — `cargo test -p cobolt-ide --bin cobolt-ide -- _012 _008 mobile_runner`: 9 passed, 1 ignored. **2.0.292:** the
    interim "first target" is gone — `pick_target` is D-Preview's rule (the session's choice while listed, else the only one running, else
    the window); **(b)** `several_targets_ask_the_window_once_and_the_choice_is_kept_for_the_session_012` (3 targets, 2 running: the window
    asked once, 0 runs, no browser; Pixel_9 chosen: the next 2 Runs go there; the manifest's bytes unchanged) and
    `a_run_takes_the_chosen_target_else_the_only_one_running_012` (7 cases). **2.0.293, a regression of 2.0.292 fixed:** with nothing
    running and **one** emulator set up the window opened, where 2.0.290 started that emulator — there is nothing to choose, so the rule
    now ends "else the only one there is"; two emulators to start still ask. The test is now
    `a_run_takes_the_chosen_target_else_the_only_one_running_else_the_only_one_set_up_012` (9 cases: the only one to start → it; two to
    start → the window; two to start, one chosen → it); **(e)** `a_failed_device_run_is_said_and_opens_nothing_else_012`
    — a run the device refuses before it starts (a package build already running) is the Output's `pk_prev_device_failed`, rewritten in the
    six languages to say how to choose another device, and **no** browser page. Test (a) now gives one running target and one to start (it
    gave two, which D-Preview answers with the window). **2.0.297, a fix (the operator's report):** the form designer's **Run Form**
    (▶) skipped D-Preview and ran a Mobile project's form in the desktop interpreter, whose `COBOL::SecureLoad` ends in "available in a
    Mobile application only"; `do_run_form` now hands a Web or Mobile project to Run (the phone or emulator, else the browser) and the
    button's tip says where it runs (`run_form_tip`, six languages; test
    `run_form_in_a_web_or_mobile_project_says_where_it_runs_in_six_languages`). Debug Form keeps the desktop window: no debugger
    reaches the device yet. **Left:** the iOS half — the simulators are here since 2026-11-10 (Xcode 27.2, iOS 27.2) and the simulator
    application is built (2.0.411), but listing the simulators, starting one, installing and streaming its log from the IDE is unwritten
    (`detect(Ios)` in `panels/mobile_runner.rs` still answers an error).
  - Files: `crates/cobolt-ide/src/panels/mobile_devices.rs` (`MobileDeviceRunner`); `crates/cobolt-ide/src/panels/web_preview.rs` (the `DeviceRunner`
    seam receives the real implementation); `crates/cobolt-ide/src/app.rs` (`toggle_web_preview` `:2736`, `do_run` `:2823`: a Mobile project goes
    through `DeviceRunner`).
  - Do:
    - **Dependency:** WASM-008 T39 defines `DeviceRunner::{detect, list, run(dev_url), log}`. If it has landed, implement it. If not, introduce the
      trait in the shape of WASM-008 plan §2.3 with its "no device tools" default, so WASM-008 adopts it — one definition either way (plan Risk 11).
      Record which happened here.
    - `MobileDeviceRunner` over `mobile_toolchain::detect(platform)` and `mobile_devices::list(platform)`: tools present and at least one target → run on
      the target D-Preview picks (last chosen this session if still listed; else the single running one; else open T19's window with the list); else
      answer "none" with the missing tools' next steps, or "no emulator, simulator or device: create one" — which WASM-008's fallback shows in its
      `NotTheDevice` note. A target that fails to start is an Output error and **no** fallback.
    - A **Web** project never consults the runner (R14, WASM-008 R12). The fallback page, launcher and `preview_browser` stay WASM-008's (O7).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide mobile_device_runner --no-fail-fast`, driven by fake `detect`/`list` answers, each case printing
    its decision and inputs: (a) tools + one running target → run with the dev address and that target; (b) several targets, none chosen → the window is
    asked to open, no fallback; (c) a tool missing → "none" with that tool's next step reaching `NotTheDevice`; (d) tools, no target → "none" with
    "create one"; (e) a start failure → Output error, no fallback; (f) a Web project never calls the runner.

- [x] **T21 — The Signing window** (R4, R5; AC4; D-Store)
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC4 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - Files: `crates/cobolt-ide/src/panels/mobile_signing.rs` (new); `panels/mod.rs`.
  - Do: names go into `cobolt.toml` `[mobile.signing]` — the only project write (user code is sacred); passwords go into T10's strict store as
    `mobile-signing/<project-id>/<name>`, with Replace and Delete; a keystore inside the project folder is refused; "secret store unavailable — cannot
    continue" when T10 errs. An explicit default size.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide mobile_signing`: the save function writes the names and never a password (the saved
    `cobolt.toml` read back and scanned for the fake password); an unavailable store blocks the save.

- [x] **T22 — i18n ×6 and the System KB** (all R; i18n; System KB rule)
  - **Closed 2026-11-09 by the operator's ruling, not verified:** the acceptance criteria it was to settle stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - **Partly done:** seven new `Tr` keys in the six languages (`pk_pkg_started`, `pk_pkg_built`, `pk_pkg_failed`, `pk_dev_installing`,
    `pk_dev_started`, `pk_dev_failed`, `pk_dev_busy`); `pk_prev_on_device` and `pk_build_package_note` rewritten (the note now speaks of iOS
    only). The System KB ("Project kinds", `crates/cobolt-compiler/src/lib.rs`): the Mobile row no longer says the shell is missing; the new
    `[mobile]` fields; "The Android package" (tools, checks, `package.log`, release signing, Run on a device); "The phone's secure storage" (the
    three verbs, the statuses, the limits, the Preview's `-6`, deep links); `chunked.data` regenerated. **2.0.289:** Run "starts it as a tap
    on its icon would" and the Output shows the application's log as the program wrote it; a new paragraph "On the phone's screen" (the safe
    area, the light strip, `--pr-statusbar`, `--pr-safe-*`, Back on the first form, the light/dark caveat); `chunked.data` regenerated (4277
    records). **2.0.292:** T19's 13 keys ×6 (`dev_win_*`, `menu_run_on_device`, `pk_prev_choose_device`, `pk_dev_stopped`) and
    `pk_prev_device_failed` rewritten (no browser after a failed run); the KB's Mobile Run paragraph describes the window, the session's
    choice, Stop and the failed run; `chunked.data` regenerated (4279 records); `no_empty_ui_translations`,
    `non_english_is_actually_translated`, `the_system_kb_teaches_every_builtin_inline`, `prebuilt_chunked_kb_matches_the_published_documentation`
    and `cobolt-compiler published_documentation_tests knowledge_base_build_tests` green. **Left:** the keys of T14/T21 and the iOS LAN warning.
  - Files: `crates/cobolt-ide/src/i18n.rs` (the `Tr` keys ×6 of plan §7: every `MobileTool` name/purpose/next step; "unavailable on this machine" +
    reason; "no emulator, simulator or device" + next step (R3a); a target that failed to start; the device window; the LAN opt-in warning; the signing
    window; the R5 refusals; the R7 message; the checklist heading; `pk_build_package_note` `:458`/`:2998` rewritten in all six languages);
    `crates/cobolt-compiler/src/lib.rs` (the Mobile row `:6208` and the `[mobile]` block in "Project kinds" `:6193`; the Build/Preview paragraph —
    Preview and Run of a Mobile project go to the emulator, with the browser fallback; the three `COBOL::Secure*` verbs beside the application-object
    verbs' KB text); `assets/knowledge/chunked.data` (regenerated).
  - Do: Portuguese in pt-BR; user-visible text says PowerRustCOBOL, never `cobolt`; `rcrun mcp` answers stay English; regenerate with
    `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify:
    - `cargo test -p cobolt-ide --bin cobolt-ide no_empty_ui_translations non_english_is_actually_translated`;
    - `cargo test -p cobolt-ide --bin cobolt-ide the_system_kb_teaches_every_builtin_inline prebuilt_chunked_kb_matches_the_published_documentation`;
    - `cargo test -p cobolt-compiler published_documentation_tests knowledge_base_build_tests --no-fail-fast`;
    - `grep -n "does not include yet" crates/cobolt-ide/src/i18n.rs crates/cobolt-compiler/src/lib.rs crates/cobolt-project-tools/src/tools/build.rs`
      finds no mobile-shell sentence.

## M6 — device evidence and the ledger

- [x] **T23 — AC10 re-run: Web and Desktop build exactly as before** (R14; AC10)
  - **Run 2026-11-07 on 2.0.291, the compiler half again on 2.0.292** (the four commands of T2, `--no-fail-fast`, every `test result:` line
    read): `cobolt-project-tools` **248 passed, 0 failed, 1 ignored** over 28 test targets (`kinds.rs` 10/10, `web_build.rs` 25/25, the
    Chrome comparisons all green); `cobolt-web-bundle` **133 / 0** over 53; `cobolt-wasm` **107 / 0** over 27; `cobolt-compiler project_kind`
    **27 / 0** over 15 (including `project_kind_build_tests`). T7's Web refusal and T17's identity tests are among them; T20 (f)
    (`a_mobile_preview_with_no_target_and_a_web_preview_take_the_browser_012`) is green. No count from before the change exists (T2), so
    "the same counts" is read as "all green, no test removed".
  - Files: none changed.
  - Verify: the T2 commands give the same counts (plus the new tests), all green; T7's, T9's and T17's identity tests green; T20 (f) green. Compare the
    counts here.

- [x] **T24 — Device runs, recorded, never claimed** (AC1, AC3, AC4 live, AC6, AC7, AC8, AC9; R3a)
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC1, AC3, AC4, AC6, AC7, AC8, AC9 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - **Run so far (2026-11-06, emulator `Medium_Phone_API_37.0`, API 37, arm64):** AC1 Android debug: `rcrun mcp build` on
    `examples/PowerMobileHello` → `dist/mobile/android/PowerMobileHello-debug.apk`, `adb install -r` → Success, the main form shown; the page at
    `http://tauri.localhost/` with the shell's bridge. The secure verbs: Greet stored the name (`SecureStatus 0`), the application stopped and
    started again greeted "Welcome back" from `SecureLoad`, Forget me deleted it (`0`) and a second Forget me answered `1`. The package is 40 MB
    (6–7 s warm). **AC3 / R3a, the IDE's half:** `cargo test -p cobolt-ide --bin cobolt-ide the_example_runs_on_the_first_android_device --
    --ignored --nocapture` — the runner the Preview is given built, installed and started the example on `emulator-5554` in 112.8 s (Gradle's
    first start; Cargo compiled nothing); the window itself is the operator's to look at. **2.0.289, the demonstration path, run on the same
    emulator:** the IDE's runner test again, 25.9 s and 40.9 s (warm Gradle); the example's `DISPLAY` reached the log as `PowerMobileHello: Hello,
    Grace. I will remember you.` (the line the Output shows); Back on the first form → "Process … exited cleanly (0)" (before the fix: signal 6); started again from the launcher's intent (cold, 10 s on a loaded host) → "Welcome back, Grace!" from `SecureLoad`. A
    minute's watch of an idle started application: alive throughout. **2.0.290, Run with no emulator running** (the emulator stopped with
    `adb emu kill`, then the IDE's runner test): "Starting the emulator Medium_Phone_API_37.0…", "is ready (6 s)" (it resumed its saved
    state), built, installed, "Started on emulator-5554" — 14.4 s in all; the form greeted "Welcome back, Grace!". Two earlier attempts found
    what the Run now waits for: Android restarting its system on the emulator starved by the build (the package service gone while
    `sys.boot_completed` stayed `1`), and a start refused before the user was unlocked; each is now waited for (the package service before
    the install, a refused start asked again for up to 60 s). The restart case is covered by that wait, not re-measured. **2.0.291, a Run
    over the application on screen** (the case a developer meets on every second Run): 22.1 s to the started application, one start, alive
    and in front 90 s later; then, following the log with the Run's own `--uid` arguments, Greet, Forget me and Greet gave the program's three
    `DISPLAY` lines ("Hello, Grace. I will remember you.", "Forgotten. See you, stranger.", "Type your name first."), every line from the
    application's process and none from another. **2.0.292:** the device test again, now listing through `DeviceTarget`
    (`emulator-5554` running, the one target): built, installed and started in 23.6 s, alive and the resumed activity 20 s later.
    **2026-11-07, the 2.0.293 package on the same emulator, read over the web view's DevTools (`adb forward … webview_devtools_remote_<pid>`):**
    the shell's bridge is there (`__TAURI_INTERNALS__.invoke` a function); the screen is 412 × 915 CSS px (`devicePixelRatio` 2.625), so the
    modal draws the phone. **AC8's signals:** HOME and back to the application gave, in order, `visibilitychange` hidden, `pr-shell`
    `{"type":"paused"}`, `visibilitychange` visible, `pr-shell` `{"type":"resumed"}`, the same process throughout (pid kept). **The keyboard:**
    a tap on the name box shows it (`dumpsys input_method`: `mInputShown=true`); `innerHeight` stays 914 while `visualViewport.height` falls
    to 578 (the keyboard covers 336 CSS px), `navigator.virtualKeyboard.overlaysContent` is false and its rectangle empty; the focused box
    (bottom at 167) stays in view. Rotation is not measurable with the example: its manifest locks portrait. **The emulator itself:** the AVD
    `Medium_Phone_API_37.0` had one virtual processor (`hw.cpu.ncore=1`, Play Store images hide the setting) and software rendering (`auto`
    chose lavapipe + SwANGLE): a cold boot took over 8 minutes and Android's `system_server` failed under the load (the WebViewZygote
    `NullPointerException`, "Can't find service: package" during installs) — the failures met on 2026-11-06. With 4 processors
    (`config.ini`, the setting Device Manager edits) the cold boot completed in under a minute and the install and start worked at once.
    AC7 in part: the forms stand below the status bar (root 24 CSS px down,
    the strip `#f4f5f7`) and above the gesture bar; Back measured (it closes, it does not pop: the example has one form). **Not run here:** the
    `.aab`; iOS (then Xcode 15.2, simulators iOS 17.2 < floor 18; the simulator application was run on 2026-11-10, 2.0.411, see the header); AC4 live (the operator's key); AC6 (O4); AC7's rotation and a multi-form Back;
    AC8–AC9 on the device; the R3a fallback.
  - Files: `specs/WASM-PROGRESS.md` (the WASM-012 row `:41`, T/I/V columns, and a dated evidence note); the `#[ignore]` tests of T11–T16.
  - Do: run each `#[ignore]` test where the tools exist, with its exact command; record platform, command and result:
    - **AC1:** Android `.apk` + `.aab`; the emulator shows the main form. On a Mac with Xcode: the iOS archive; the simulator shows the main form. *(2026-11-10, 2.0.411: the simulator application is built and shows the form on an iPhone 18 Pro; the archive is refused, it needs the Apple team, signing identity and provisioning profile.)*
    - **AC3 and R3a (WASM-008 AC13):** Preview and Run of the Mobile fixture with an emulator start it, install, show the form; an edit reloads; the
      device log and the page console reach the Output panel.
    - **R3a fallback (WASM-008 AC14):** with `ANDROID_HOME` pointed at an empty folder, Preview opens the browser at a phone width and the Output panel
      names the missing tool.
    - **AC4 live:** a release build with the **operator's** signing material (never the agent's); `apksigner`/`codesign` show a non-debug identity.
    - **AC6:** pending O4 — recorded as "waits for the first confirmed Q1 feature".
    - **AC7:** the fixture page prints `--pr-safe-*`; rotation locked; Back pops.
    - **AC8:** background and return raise the events; the backend reconnects; an expired token leads to sign-in.
    - **AC9:** an omitted command is rejected; an external link opens the system browser.
  - Verify: each item has a recorded result or a "not run here: <reason>, command: <cmd>" line. Nothing is claimed that was not run.

- [x] **T25 — Q1 features, one at a time — GATED on the operator's confirmation (O1)** (R8, R9; AC6 device)
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC6 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - Files: per feature: one `mobile_features.rs` row; one plugin in `crates/cobolt-mobile-shell/Cargo.toml` (passing the guard); its verbs on the
    T7/T15 pattern (`APP_VERBS`, `host.rs`), with T5's desktop answer; one A14-style row; `Tr` keys ×6; one KB entry; regenerated `chunked.data`.
    Clipboard, if confirmed, only routes the existing `COBOL::CopyToClipboard` through the shell (plan C5).
  - Do: **do not start** until the operator names which of camera, location, notifications, biometric unlock and barcode scanning enter 2.0. The first
    confirmed feature that prompts carries AC6's device half.
  - Verify: per feature, the T4/T5/T7/T8/T15/T22 checks plus the T24 AC6 run.

## M7 — finalize

- [x] **T25b — D21: the "connecting" modal on the device** (R15; AC11) — added 2026-10-24 by the operator's ruling.
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC11 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - **Done in the player (2.0.293):** `view::shell::NetDevice` (`Screen`, `Phone`, `Tablet`) and `net_device(in_shell, w, h)`: outside the shell
    the desktop screen, inside it a tablet from a shortest side of 600 CSS px (`TABLET_SHORT_SIDE`, Android's `sw600dp`, below every iPad), a
    phone otherwise; `net_modal_view` draws that device in the screen's place, its right edge at x = 82 so the link to the cloud starts where it
    did, and nothing else changes (dialog, gradient, white strokes, captions, Reload, the events it takes); the page reads the screen through
    `window.screen` (`dom::net_device_here`). The return from the background: `net::Client::resumed` — a connection that worked and whose cycle
    gave up (perhaps while the application was away) is tried again at once behind the modal under R25's policy, its give-up raises
    *unreachable* again (WASM-016 R21's exception again); a connection that works, is being tried, never worked or was signed out of is left
    alone. The page hears the return from the shell's `pr-shell` `{"type":"resumed"}` and from `visibilitychange` to visible (plan R11's
    portable fallback); both may come, the second finds the cycle running. The listeners exist only inside the shell and are taken off at
    unmount (no `Closure::forget`). Tests: `in_the_mobile_shell_the_connecting_modal_draws_the_phone_or_the_tablet_012` (7 screen sizes, the
    three devices connecting and unreachable) and `a_return_from_the_background_retries_a_lost_connection_behind_the_modal_012`.
  - **Left:** AC11 on a device. **Not run here:** the example's bundle carries no backend address (`dist/web/config.json` `backend.origins` is
    `[]`; a Mobile Run gives the device no route to a backend — no `adb reverse`), so there is no connection to cut; command once a Mobile
    project has a reachable backend: run it, `adb shell cmd connectivity airplane-mode enable`, look, `… disable` within 60 s, and again for
    longer than the three windows. The lifecycle events of spec 037 for paused/resumed: spec 037 has no folder in `specs/`; nothing maps them
    yet (T15).
- [x] **T25c — D22: the native Android Studio / Xcode project created with the Mobile project, and its re-creation from Project Settings** (R16–R18; AC12; WASM-001 R19; Q7) — added 2026-10-24 by the operator's ruling.
  - **Closed 2026-11-09 by the operator's ruling, not verified:** its acceptance criteria AC12 stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - **Q7, decided as the spec recommends (agent, under the operator's delegation of 2026-11-06):** the native project lives in the project
    folder, `android/` (and `ios/` when it exists) beside `mobileforms/` and `backend/`; the IDE's package builds keep their own staging in the
    build folder, so a Build never waits on Android Studio's caches nor Android Studio on the IDE's.
  - **Done (2.0.294), Android:**
    - `cobolt-compiler` `mobile_native`: `create_android` stages the shell into `android/` as a package build stages it (shell sources,
      `src-tauri/`, the bundle in `frontend/`), writes `android/.gitignore` (what the tools build and cache), runs `tauri android init --ci
      --skip-targets-install` when the Gradle project is not there, makes the package build's edits (orientation, signing hook, the names under
      the icon), names the Gradle project after the project (`rootProject.name`, a marked line in `settings.gradle`, `patch_project_name`) and
      makes the launcher icons. Without the tools it writes the sources only and names what is missing (`Created::SourcesOnly`).
      `refresh_android` brings it up to date after every package build (`build_android`), so Build and Run on a device keep it on the bundle
      the package shows (R16, R18). `mobile_build::check_shell` checks what the shell needs without a bundle; until the first Build
      `frontend/index.html` is `NOT_BUILT_PAGE` (it says to build in PowerRustCOBOL AI). `[mobile.native]` (`NativeProjects`, `android`, `ios`)
      is written only when set (R18).
    - IDE, `panels/native_project.rs`: New Project's step for a Mobile project for Android (`NativeStep`: Create / Not now; a fixed size, no
      grip, nothing in it grows); the making on a thread (`NativeJob`, every tool line in `native.log` in the build folder); the outcome in the
      Output and, when made, `[mobile.native] android = "android"` saved into the project if it is still the one open (WASM-001 R19).
    - Project Settings, Mobile application (`web_settings::mobile_identity_rows`): the **Application id** row (a refused id keeps the saved one,
      the page says why); the **Android Studio project** row (where it is, or that it is not created, and where it would go; Create / Create
      again, made from the settings on the page, saved or not; the made project recorded in the draft); for an iOS project, the **Xcode
      project** row, unavailable, with the reason (R17).
    - `list_files` reports `native` (`recorded`, `exists`, `android_studio_folder`; iOS `exists: false`) (R18). *Create from Template…* leaves
      `android/` and `ios/` behind: generated, and named after the project copied (R18's "marked generated", for the copy).
    - **Found while doing it, fixed in the same change:** a new Mobile project had no application id and the IDE had no field to give it one,
      so its first Run was refused pointing at "Mobile settings, Application id", which did not exist. `create::plan` now writes
      `com.example.<name>` (`project_kind::default_app_id`: lower case, letters and digits, valid for `validate_app_id` and for Tauri's
      identifier, which takes no `_`), so the IDE and `create_project` still write the same manifest (WASM-001 AC3), and the row above edits it.
    - `Tr` ×6: 17 keys. System KB: the paragraph "The Application id and the Android Studio project"; `chunked.data` regenerated (4284
      records).
    - **Also found, a slip of 2.0.288 (T3's minimum OS floors):** the templates named the Minimum OS choice `auto` "Auto / Latest" (what
      `create_project` shows the agent) while it resolves to the floor; it now reads "Auto (the oldest supported)", as the dialog's `Tr` did.
      Two IDE tests had been red since 2.0.288 (`the_fields_make_the_request` still chose iOS 17.0; `the_builtin_catalogue_is_fully_worded…`
      listed the withdrawn versions) and pass again.
  - **Found (Tauri 2.12.4):** the generated Gradle project builds the shell through `cargo tauri android android-studio-script`, which asks a
    running `tauri android build` or `dev` for its options (`fetch_options`, tauri-cli `src/mobile/mod.rs:551`: "the `tauri android dev` or
    `tauri android build` command must be running while Android Studio builds the app"). Android Studio builds the project only while
    `cargo tauri android build --debug --apk --open` runs in `android/src-tauri` (it builds once, opens Android Studio and waits). The KB says so.
  - **Done (2.0.295), Open in Android Studio and AC12 on the command line:**
    - `mobile_native::open_android_studio` brings the project up to date (as Create does), starts its Rust build from the package build's
      on macOS (`seed_rust_build`: `cp -c`, a copy-on-write clone, only when the native project has none), and starts the Tauri command line
      itself (`cargo-tauri`, not `cargo tauri`, so stopping it stops it): `android build --ci --debug --apk --target <abi> --open`
      (`studio_args`) in `android/src-tauri`, every line in `android-studio.log` in the build folder.
    - IDE: Project Settings, Mobile application, the Android Studio project row gains **Open in Android Studio** (when the project is there)
      and **Stop** while it runs, with a note (running and the log; could not start and why; ended by itself and how; stopped). The app owns
      the process (`native_project::Studio`, lent to the page as `SharedStudio`): closing Project Settings leaves it running, the next Open,
      Stop and closing the IDE end it (Drop). Create again waits while it runs. `Tr` ×6: 6 keys.
    - **Found while proving it (Tauri 2.12.4), fixed:** `tauri android build` gives Cargo the Android linker and flags in its own
      environment (`configure_cargo`: `CARGO_TARGET_<T>_LINKER`, `_RUSTFLAGS`), which reaches Gradle only when it runs Gradle itself;
      `android-studio-script`, what Android Studio's Gradle runs, sets neither. In Android Studio's build Cargo linked with the computer's
      `cc` (`linking with cc failed`) and built every library again, its flags differing. Every create and refresh now writes
      `android/src-tauri/.cargo/config.toml` (`cargo_config`: the NDK's `clang` of the minimum API level and the same three flags, for the
      four Android targets; left out of version control, the paths are the computer's), so Android Studio's build links whatever started it.
    - **Found, fixed:** Tauri's identifier (and an iOS bundle id) takes no `_`, which Android and `validate_app_id` accepted: `tauri android
      build` refused `com.example.power_mobile` after `tauri android init` had taken it, so such an id passed Project Settings and failed
      inside the build. `validate_app_id` now takes letters and digits only (no `_` or `-`); the page's message says so in six languages.
    - **AC12, command-line half, passed** (`gradle_builds_the_android_studio_project_while_the_tauri_command_runs_ac12`, ignored; seeded from
      the IDE's package build of `PowerMobileHello`): started in 3.2 s; Android Studio opened at 27.2 s; while the Tauri command line ran,
      `./gradlew assembleArm64Debug` (the arm64 variant Android Studio's Run builds) made a 31.7 MB package in 10.4 s; installed on
      `emulator-5554` it ran (pid 5444) and its web view showed the test's bundle (title `AC12`, read through the DevTools). Without the Cargo
      file the same run failed at the link.
  - **Left:** AC12 inside Android Studio's own window (its Run button: the operator's window, not driven here); the Xcode project (iOS is not
    run here); WASM-001 T19's own Finalize.
- [x] **T26 — Finalize: version, CHANGELOG, tests of the touched crates**
  - **Closed 2026-11-09 by the operator's ruling, not verified:** the acceptance criteria it was to settle stay open, tracked in `specs/WASM-OPEN-ACS.md` (section WASM-012-mobile-shell-tauri-2). Reopen the task from there if a problem points at it.
  - Files: `crates/cobolt-ide/src/version.rs` (`z` +1 **once**: from the value at that time, `2.0.49` today, to the next `2.0.z`; never `x` or `y`);
    `CHANGELOG.md` (one entry above the previous heading, dated, "Written on features-2x", listing what landed and what stays open: O1–O10 and the T24
    lines not run).
  - Do: `df -h .` first. Run the tests of the crates touched with `--no-fail-fast` and read every `test result` line:
    - `cargo test -p cobolt-compiler -p cobolt-runtime -p cobolt-semantic -p cobolt-wasm -p cobolt-wasm-abi -p cobolt-web-bundle -p cobolt-project-tools --no-fail-fast`
      (drop `cobolt-semantic` if T5 did not touch it);
    - `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`;
    - `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast`;
    - `cargo test --manifest-path crates/cobolt-mobile-shell/Cargo.toml --test dependency_guard`.
  - Verify: all green, counts recorded here; the diff has no credential (`AKIA`/`ASIA`, keystore passwords, the fake test passwords outside tests);
    `git status` shows nothing under `docs/` and no `README.md` change; manual (the operator's): launch the IDE, confirm the version string, open the two
    new windows in six languages, Preview a Mobile project with and without an emulator.

## Done criteria
- Every acceptance criterion in spec.md is covered by at least one task's verification:
  - AC1: T13, T17, T24;
  - AC2: T14;
  - AC3: T16, T19, T20, T24;
  - AC4: T3, T10, T11, T13, T21, T24;
  - AC5: T4, T7, T11;
  - AC6: T15 (logic); the device half T24/T25 (O4);
  - AC7: T9, T11, T15, T24;
  - AC8: T15, T24;
  - AC9: T11, T15, T24;
  - AC10: T2, T7, T9, T17, T20 (f), T23.
- R3a (no AC of its own in this spec; WASM-008 AC13/AC14): T13 `run_dev`, T14, T16, T20, T24.
- The touched crates' tests pass. Nothing under `docs/` or `README.md` changes.
- The open items O1–O10 are carried, not decided.
- No task commits or pushes; the pipeline does the bookkeeping.
