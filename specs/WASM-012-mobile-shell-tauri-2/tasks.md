<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The mobile shell (Tauri 2): Android and iOS builds, devices, signing and permissions (WASM-012)

- **Status:** draft. Written 2026-10-22 from `plan.md` (draft of 2026-10-22). No shell code exists yet. Every Tauri, Android and iOS statement in the plan is
  marked **[verify]** and is unconfirmed; T1 confirms them before any other task starts.
- **Plan:** ./plan.md   **Date:** 2026-10-22

Ordered, small, independently-verifiable tasks. Each names the files it touches, the requirement(s) it satisfies, and how to verify it. Check off as completed.

**2.x.x overrides for this list.** There is **no documentation task**. `README.md` and everything under `docs/` are never touched on 2.x.x. What 2.x.x
says about itself goes in `specs/`: here, the ABI amendment A10 (T6), the plan's V0 record (T1) and the `WASM-PROGRESS.md` row (T23). The user-facing
description of the mobile build lives in the System KB text (T21). The finalize task (T25) bumps `z` once and writes one CHANGELOG entry. No task
commits, pushes or switches branches.

**Grounding read for this list (2026-10-22, `features-2x` at 2.0.30).**
- **Manifest model.** `crates/cobolt-compiler/src/project_kind.rs`:
  - `ProjectKind` `:26`, `MobilePlatform` `:81`, `DeviceClass` `:98`, `Orientation` `:115`, `WebSettings` `:273`;
  - `MobileSettings` `:320` has the fields `platform`, `device`, `orientation`, `min_os` (default `"auto"` through `fn auto` `:341`), `style`,
    `multi_memory` and `backend_url`;
  - `MobileSettings::new` `:348`, `module_variants` `:366`; the tests start at `:459`.
- **Compiler.**
  - `crates/cobolt-compiler/src/lib.rs`: `default_build_root` `:144`, `build_staging_dir` `:222`, `stage_sdk` `:3757`;
    `CompilerError::UnsupportedTarget` `:760`, `NotADesktopProject` `:770`, refused at `:1810`.
  - The module lines are at `:57-63`.
  - The KB section "Project kinds: Desktop, Web and Mobile" is at `:6172`. Its Mobile row `:6187` says the package step "needs the mobile shell,
    which the product does not include yet".
  - The `COBOL::"NAME"( args )` = `CALL "COBOL-NAME"` rule is stated for the KB at `:5037`.
  - `project_kind_build_tests` is at `:14238`.
  - `crates/cobolt-compiler/src/backend_build.rs`: `BackendBuildOptions` `:37`, `BackendBuildResult` `:51`, `build_backend` `:92`, which uses
    `build_staging_dir` at `:158`. This is the pattern `mobile_build.rs` follows.
- **Web build.** `crates/cobolt-project-tools/src/web/mod.rs`:
  - `BUNDLE_SUBDIR = "dist/web"` `:41`;
  - `WebBuildRequest::from_manifest` `:71` sets `ios_webview`/`android_webview` `:88-89` and `Isolation::Off` `:95`;
  - `WebBuildError::Refused` `:200`, the `WebBuildBackend` trait `:225`, `link` `:279`, `backend` `:289`;
  - `devserver.rs` lives beside it.

  `crates/cobolt-project-tools/src/tools/build.rs` `:126-135` answers a built Mobile project with `"package": {"built": false, …}`, and the reason
  says the Tauri 2 mobile shell "this program does not include yet". T18 replaces that answer.
- **Generator.** `crates/cobolt-wasm/src/lower_form.rs` `builtin_call` `:261` lowers every `CALL "COBOL-NAME"` that is not special onto the
  application object (`CTRL_APP`, `:316`). It refuses names outside `crate::support::browser_builtin` (`crates/cobolt-wasm/src/support.rs:148`, charts
  and audio only) with "runs on the desktop or the backend, not in a browser module" (`:313`).
- **Interpreter.**
  - `crates/cobolt-runtime/src/builtins.rs`: `BUILTINS` `:35` (each entry is a name without `COBOL-`, its parameters and a description) and `builtin()`
    `:107`.
  - Built-in calls are dispatched by name in `crates/cobolt-runtime/src/interpreter.rs`, for example `"COBOL-APPEND-FILE"` at `:11553`. This is the
    "dispatcher to be located" of plan §2.
- **Player.** `crates/cobolt-web-player/` is a separate workspace (root `Cargo.toml:45` `exclude`).
  - `src/host.rs` `verb` `:1566` is the application object's verbs: `NAVIGATE`/`OPENURL` `:1601`, `COPYTOCLIPBOARD` `:1609`.
  - `src/net.rs` reconnects with back-off (`backoff_start_ms`/`backoff_max_ms` `:75-76`).
  - The shipped player is rebuilt with `cargo run --manifest-path crates/cobolt-web-player/Cargo.toml --example build_player -- --out
    assets/web-player`. The freshness check is `--test freshness` (`specs/WASM-016-prebuilt-player/tasks.md:51`).
- **Bundle.** `crates/cobolt-web-bundle/src/csp.rs`: `CspOptions` `:26`, `CspOptions::strict` `:42`, `Policy` `:49`.
- **IDE.**
  - `crates/cobolt-ide/src/app.rs` `start_web` `:2710`.
  - `src/toolchain.rs`: `minimum` `:127`, `spawn_install` `:363`, `FirstRunPrompt` `:480`.
  - `src/panels/new_project.rs`: the `orientation`/`min_os` options `:516-534`.
  - `src/secrets/mod.rs`: `SERVICE = "PowerRustCOBOL"` `:35`, `NATIVE_STORE_DISABLED = true` `:46`, and `store`/`retrieve`/`delete` at `:68`/`:86`/`:102`,
    each short-circuited by the switch.
  - `src/llm.rs` uses `secrets::retrieve`/`store`/`delete` at `:608`/`:986`/`:999`.
  - `src/i18n.rs` tests: `no_empty_ui_translations` `:15273`, `non_english_is_actually_translated` `:15286`.
  - `src/grace_host.rs`: `the_system_kb_teaches_every_builtin_inline` `:4758`, `prebuilt_chunked_kb_matches_the_published_documentation` `:4780`.
    The store is regenerated by `examples/build_chunked_kb.rs`.
  - `src/version.rs` `VERSION = "2.0.30"` `:22`.
- **ABI and ledger.**
  - `specs/WASM-ABI-v1.md`: §9 error codes `:123` (`-6` "host refused (permission or policy)"); A7 `:170`, which says an unknown verb answers `-4`;
    A8 `:171`, A9 `:172`.
  - §12.1 `:174`: `Navigate`/`OpenUrl` `:185`, and `GetParam`/`GetParams` "also from a deep link" `:192`.
  - `crates/cobolt-wasm-abi/src/` has `codes.rs` and `manifest.rs`.
  - `specs/WASM-PROGRESS.md:41` has the WASM-012 row, with P ✔ only.
- **Nothing exists yet** for the shell, any `tauri` dependency, a mobile toolchain/devices/features module, or the signing or device windows. This was
  confirmed by the plan and by no match in the tree.

**Binding decisions (operator, 2026-10-22).**
- Q2: no remote builds. iOS needs a Mac, and iOS is offered as unavailable elsewhere, with the reason.
- Q3: the minimum OS is the last version released in 2024, raised one year per major version (D-MinOS).
- Q4: one shell. Phone and tablet differ only by the responsive breakpoints (D-Tablet).
- Q5: Tauri and its plugins track the IDE (D-Pin).
- Q6: certificate pinning is optional and belongs to WASM-013. Nothing here.

**Open — working assumptions, never decided here.**
- **O1 — spec Q1, "yes", *to confirm*.** Do camera, location, notifications, biometric unlock and barcode scanning enter 2.0? Working assumption: none
  is implemented until the operator confirms. T24 is written as a gated template and stays unchecked.
- **O2 — PQ1, one platform per project.** Working assumption: one platform per project, `MobileSettings::platform` as today. R1's "each platform the
  project selected" is read as that one platform.
- **O3 — PQ2, may the IDE install the Tauri CLI?** Working assumption: it is offered with consent, like `rustup target add`. Xcode, the SDK, the NDK
  and the JDK are never installed.
- **O4 — PQ3, the first prompting feature.** Working assumption: AC6's device half is proved on the first Q1 feature the operator confirms. Its logic
  half (a refusal arrives as `-6`) is proved now (T15).
- **O5 — PQ4, secure storage without native code.** This is raised only if T1 finds no qualifying published plugin or crate. If T1 raises it, T8 and
  T15's secure-storage part **stop** and wait for the operator. The rest of the list proceeds.
- **O6 — the spelling of the device verbs.** The plan writes `COBOL::SecureStore`/`SecureLoad`/`SecureDelete`. The tree has two routes: built-ins
  (`CALL "COBOL-NAME"`, hyphenated, `builtins.rs`/`builtin_call`) and the player's application-object verbs (`host.rs` `verb`). The `COBOL::"NAME"`
  form is the built-in route (compiler `lib.rs:5037`), and the generator already lowers it onto ctrl 2. Working assumption: `COBOL::"SECURE-STORE"`,
  `"SECURE-LOAD"` and `"SECURE-DELETE"`, as built-ins. The plan's §3/R9 names are updated to match in T1's write-back. An operator preference for the
  CamelCase verb form changes T6, T7 and the scan's verb list only.
- **O7 — WASM-017 Q5 (not answered).** Isolation in the shell. Working assumption: `Isolation::Off`, as `from_manifest` already sets.
- **O8 — D-Store** bypasses the secret store's kill switch for `mobile-signing/` accounts only. The plan calls this out for the operator's review at
  `/analyze`. T10 is not started before that review.

**Guardrails at `/implement`.**
- The two new IDE windows open at an explicit default size and change only when the user drags (self-resizing-window rule). Test the frames in six
  languages.
- No DataGrid or control-border code is touched. If that changes, call the matching guardrail agent.
- **Credentials:** never open, generate or commit real signing material. Test passwords are fixed fake strings. Scan staged diffs before any commit
  (the pipeline does the commit).
- **Disk:** run `df -h .` before each test run in this worktree. Use targeted `cargo test -p … --no-fail-fast`, and read every `test result` line.
  Android/iOS builds are `#[ignore]`d.

## M0 — verify and baseline

- [ ] **T1 — V0: verify every `[verify]` item and probe the local toolchain** (all R; plan §6 V0, Risks 1–4, 8)
  - Files: `specs/WASM-012-mobile-shell-tauri-2/plan.md` only (a new "§9 V0 record").
  - Do:
    - For each `[verify]` in the plan, record the official source (Tauri v2 documentation, crates.io `Cargo.toml`, Apple/Android documentation), the
      date, the answer, and the change it forces.
    - The items:
      - `tauri android build --apk --aab` and the iOS archive/export;
      - the Android signing configuration route (D-Sign: can Gradle read the passwords from the environment?);
      - `app.security.csp` and the shell origins;
      - the opener plugin;
      - the Back interception API and the mobile `RunEvent`s;
      - the iOS device-family key;
      - `tauri icon`;
      - `xcrun devicectl`, and the `apksigner`/`codesign` output;
      - the 2024 floors (Android 15 / API 35, iOS 18);
      - whether a Tauri command call is asynchronous (does `SECURE-*` suspend?);
      - the iOS rationale;
      - the CLI flag for a single target;
      - Apple's privacy-manifest categories.
    - Evaluate secure-storage candidates (a published Tauri plugin, or `keyring` with Android/iOS backends) against the guards: licence, no
      `ring`/`aws-lc-rs`, MSRV ≤ 1.92, maintenance. Choose one, or raise O5.
    - Choose the Tauri version set that builds on 1.92 (D-Pin).
    - Run the local probe: `cargo tauri --version`, `rustup target list --installed`, `xcodebuild -version`, `java -version`, and the SDK/NDK folders.
    - Write back O6's verb spelling.
    - Any false statement changes the plan's design at the point it names, before T2 starts.
  - Verify: every `[verify]` in `plan.md` has a V0 row (count the markers and the rows; the two counts are equal). The probe output is pasted into the
    record. Nothing else in the tree changes (`git status` shows only `plan.md`).

- [ ] **T2 — Baseline: the Web and Desktop build tests are green before anything changes** (R14; AC10 baseline)
  - Files: none changed.
  - Do: run the tests that AC10 later re-runs unchanged. Record their counts in this task. A red test is fixed in its owning crate before T3.
  - Verify:
    - `cargo test -p cobolt-compiler project_kind --no-fail-fast` (including `project_kind_build_tests`);
    - `cargo test -p cobolt-project-tools --no-fail-fast`;
    - `cargo test -p cobolt-web-bundle --no-fail-fast`.

## M1 — the manifest and the feature table

- [ ] **T3 — `[mobile]` gains identity, features and signing names; `min_os` gets its floor** (R4 names, R6, R14; AC4 part, AC10; D-MinOS, D-BuildNo)
  - Files: `crates/cobolt-compiler/src/project_kind.rs`.
  - Do:
    - Add to `MobileSettings` `:320` the fields `app_id: String`, `app_name: BTreeMap<String, String>`, `icon: String`, `splash: String`,
      `features: Vec<String>`, `rationale: BTreeMap<String, BTreeMap<String, String>>` (feature → language → text) and
      `signing: MobileSigning` (`android_keystore`, `android_alias`, `apple_team_id`, `apple_identity`, `apple_profile`). Every one has
      `#[serde(default)]`.
    - **Found while writing this list:** the plan's §3 TOML is not valid TOML. It uses `features` as an array
      (`features = ["secure-storage", …]`) and also as a table (`[mobile.features.secure-storage]`), and TOML forbids one key being both. Working
      assumption: the rationale texts move to `[mobile.rationale.<feature>]` with one entry per language (`en = "…"`). This changes no requirement.
      Write the correction back into plan §3 and say so here.
    - Add `fn validate_app_id`: two or more segments, each `[A-Za-z][A-Za-z0-9_]*`.
    - Add `fn build_number(version)` = `major*10000 + minor*100 + patch`.
    - Add `fn min_os_floor(platform)`, with the values T1 confirms. `auto` resolves to the floor; a lower value is an error that names the floor.
    - Add `fn keystore_inside_project(project_dir)`, which refuses a keystore under the project folder.
  - Verify: `cargo test -p cobolt-compiler project_kind --no-fail-fast`. New tests:
    - round-trip of the full plan §3 table;
    - an old `[mobile]` table (today's fields only) loads with every new default;
    - `app_id` accepts `com.example.app` and refuses `example`, `1com.x` and `com..x`;
    - `min_os` below the floor is refused with the floor named, and `auto` resolves to the floor;
    - `2.0.29` gives `20029`;
    - a keystore path inside the project folder is refused.

- [ ] **T4 — `mobile_features.rs`: the feature table and the AST scan** (R7, R9, R13; AC5 scan half; D-Scan)
  - Files: `crates/cobolt-compiler/src/mobile_features.rs` (new); `src/lib.rs` (the `mod` line beside `:57-63`).
  - Do:
    - Write `FEATURE_SET_VERSION`.
    - Write the rows of the first set: `secure-storage` (the verbs of O6), `open-address` (`Navigate`/`OpenUrl` and `mailto:`/`tel:`/`sms:`), and
      `deep-link`. Each row carries its verbs, its Android permissions, its iOS usage key (or none), its Tauri plugin and capability permissions, and
      its R13 checklist lines.
    - Write `scan(programs) -> Vec<FeatureUse { feature, verb, file, line }>`. It walks parsed ASTs (`cobolt_parser`) for `CALL "COBOL-…"`, which is
      what `COBOL::"…"( … )` parses to, and for the application-object verbs.
    - Write `undeclared(uses, declared)`, which returns `file:line: uses <feature> (COBOL::<verb>); add it to the project's device features`.
    - The five Q1 features are **not** rows (O1).
  - Verify: `cargo test -p cobolt-compiler mobile_features --no-fail-fast`. New tests:
    - every row is complete;
    - the scan finds the inline and the `CALL` forms at the right `file:line`, and ignores a comment and a string literal;
    - an undeclared use gives the exact message;
    - it prints `N uses of M features in K programs`.

## M2 — the device verbs in every host (interpreter/binary parity)

- [ ] **T5 — The interpreter refuses device verbs outside a Mobile application** (R9; parity; plan §7)
  - Files: `crates/cobolt-runtime/src/builtins.rs` (rows `SECURE-DELETE`, `SECURE-LOAD`, `SECURE-STORE` in `BUILTINS` `:35`, in alphabetical order);
    `crates/cobolt-runtime/src/interpreter.rs` (the built-in dispatch near `:11553`).
  - Do:
    - The three names answer a run-time error, "available in a Mobile application only". They are not an unknown-built-in path and not a silent
      no-op.
    - `rcrun run-form`, embedded child forms and the compiled binary all go through this dispatcher. Confirm this by reading the binary's
      entry (`stage_sdk`-staged runtime) and record it here.
  - Verify:
    - `cargo test -p cobolt-runtime --no-fail-fast`. A new test runs a program calling each verb and asserts the error text.
    - `cargo test -p cobolt-ide --bin cobolt-ide the_system_kb_teaches_every_builtin_inline` is expected **red** until T21. Record it here; T21
      makes it green.

- [ ] **T6 — ABI amendment A10** (R8, R9, R10, R11; D-Errors)
  - Files: `specs/WASM-ABI-v1.md` (§12 table row A10; §12.1 rows); `crates/cobolt-wasm-abi/src/manifest.rs` (the built-in names, if the manifest
    validates them).
  - Do: write A10 with these parts:
    - the three secure-storage verbs on ctrl 2, their arguments, and whether they suspend (from T1);
    - deep-link delivery to `me::GetParam`/`GetParams`;
    - the shell's events to the page (insets, Back, lifecycle) as the player sees them;
    - refusal is `-6`, with no new code;
    - the ABI version stays 1.
  - Verify: `cargo test -p cobolt-wasm-abi --no-fail-fast` is green. The A10 row names every verb that T5, T7 and T15 implement (a check by reading;
    list the names in this task).

- [ ] **T7 — The generator: device verbs in a Mobile module, refused in a Web module** (R7, R9, R14; AC5, AC10; D-WebRefuse, D-Scan)
  - Files: `crates/cobolt-wasm/src/support.rs` (`browser_builtin` `:148`, or a sibling `mobile_builtin`); `src/lower_form.rs` (`builtin_call`
    `:261`); the generator's options (the project kind reaches it); the manifest's `builtins`.
  - Do:
    - In a Mobile build the three verbs lower onto `CTRL_APP` like any page built-in, and appear in `builtins`.
    - In a Web build they are refused at the line, "available in a Mobile project only". This is the `WebBuildError::Refused` shape.
  - Verify: `cargo test -p cobolt-wasm --no-fail-fast`. New tests:
    - a Mobile fixture compiles, with the verbs in `builtins`;
    - a Web fixture is refused at the line;
    - T4's scan set equals the `builtins` set on the fixtures (D-Scan);
    - an existing Web fixture's module is byte-identical to before (AC10).

## M3 — the shell and its generation

- [ ] **T8 — `crates/cobolt-mobile-shell/`: the Rust shell crate and its dependency guard** (R10, R11, R12, R9 secure storage; D-Shape, D-Native, D-Pin;
  Risks 2–4)
  - Files: `crates/cobolt-mobile-shell/Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `tests/dependency_guard.rs` (all new); root `Cargo.toml` `exclude`
    `:45` (add the crate beside `cobolt-web-player`).
  - Do:
    - Write the mobile entry point. It does these things:
      - a navigation guard: own origin stays, anything else goes to the opener, and `mailto:`/`tel:`/`sms:` go to the OS;
      - inset, keyboard, Back and lifecycle forwarding to the page;
      - the secure-storage commands, through T1's chosen plugin or crate.
    - Pin the version set from T1, with `default-features = false` where needed.
    - Also pin a `TAURI_CLI_VERSION` constant. It lives where `mobile_toolchain.rs` can read it (`cobolt-compiler`).
    - Add **no** Kotlin, Swift, Java or Gradle file.
    - If O5 is raised, write the crate without the secure-storage commands and leave that box open.
  - Verify:
    - `cargo test --manifest-path crates/cobolt-mobile-shell/Cargo.toml --test dependency_guard`: no `ring`/`aws-lc-rs` in the lock, licences in the
      allow-list, every `rust-version` ≤ 1.92, and the counts printed.
    - Working assumption: if the crate's library cannot build for the host, the guard reads the lock as text from `cobolt-compiler/tests/`, and this
      choice is recorded here.
    - `git ls-files crates/cobolt-mobile-shell` lists `.rs`, `Cargo.toml` and `Cargo.lock` only (PRIME DIRECTIVE).
    - `cargo check --workspace` is unaffected.

- [ ] **T9 — The bundle for a Mobile build: `viewport-fit=cover`, `--pr-safe-*`, CSP for the shell origin** (R10, R12; AC7 static, AC9 static;
  D-Insets)
  - Files: `crates/cobolt-web-bundle/src/csp.rs` (an option for the shell origin, beside `CspOptions` `:26`); the host-page and base-CSS writer of
    the bundle (`viewport-fit=cover` and `max(env(safe-area-inset-*), var(--pr-safe-*))`, `--pr-keyboard`), for Mobile only.
  - Do: a Mobile bundle gets the viewport and the CSS fallbacks. A Web bundle gets nothing new.
  - Verify: `cargo test -p cobolt-web-bundle --no-fail-fast`. New tests:
    - a Mobile bundle's page has `viewport-fit=cover` and the four `--pr-safe-*` names plus `--pr-keyboard`;
    - the shell-origin CSP equals `strict()` plus the origin T1 confirmed;
    - a Web bundle is byte-identical to before (AC10).

- [ ] **T10 — Strict secret-store calls for signing** (R4; AC4; D-Store) — **after the operator's review of O8**
  - Files: `crates/cobolt-ide/src/secrets/mod.rs`.
  - Do:
    - Add `store_required`/`retrieve_required`/`delete_required`. They act only on accounts under `mobile-signing/`, ignore `NATIVE_STORE_DISABLED`
      `:46`, and return `Err(reason)` when the platform store is unavailable. They never fall back.
    - Add `list_accounts(prefix)` only if the macOS, Windows and Linux layers can answer it. If they cannot, record that here; the Signing window then
      lists the manifest's names.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide secrets --no-fail-fast`. New tests:
    - a strict call on an unavailable store is `Err`;
    - an account outside `mobile-signing/` is refused;
    - `llm.rs`'s path (`secrets::retrieve`/`store`, `:608`/`:999`) still falls back exactly as before (Risk 7).

- [ ] **T11 — `mobile_build::generate`: the shell project, configuration, capabilities and platform declarations** (R1, R6, R7, R10, R12, R13 data;
  AC5, AC7 static, AC9 static, AC4 scan; D-Shape, D-Tablet)
  - Files: `crates/cobolt-compiler/src/mobile_build.rs` (new; pattern `backend_build.rs`); `src/lib.rs` (the `mod` line, re-exports);
    `crates/cobolt-compiler/tests/mobile_shell.rs` (new); fixture projects for Android and iOS under the test's temp dir, built in code.
  - Do: `generate(project, bundle_dir, uses, profile)` writes into `build_staging_dir(None, "<name>-mobile")`:
    - the staged shell crate (the way `stage_sdk` stages the platform crates);
    - `tauri.conf.json`: `frontendDist` set to a copy of the bundle, **no** `devUrl` in release, the CSP from T9, the identity (`app_id`, the
      translated names, the version, and the build number from T3);
    - the iOS device family when `device = "tablet"`;
    - the capability file, which lists exactly the declared features' commands, for the main window only, with no remote origin;
    - Android permissions and `screenOrientation`;
    - iOS usage keys with the rationale texts and `UISupportedInterfaceOrientations`.

    It writes nothing into the project folder. An undeclared use fails before generation, with T4's message.
  - Verify: `cargo test -p cobolt-compiler --test mobile_shell --no-fail-fast`. Each test prints what it compared:
    - `frontendDist` and no `devUrl`;
    - the CSP equals T9's;
    - the capability commands equal the declared features' commands, and there is no remote origin (AC9 static);
    - the permissions and usage keys are exactly the declared ones (AC5);
    - an undeclared use fails at its `file:line` (AC5);
    - the orientation is written as set (AC7 static);
    - with known fake passwords supplied in memory, a byte scan of the project folder **and** the staging folder finds none of them (AC4 scan).

- [ ] **T12 — Icons and splash from one source image** (R6)
  - Files: `crates/cobolt-compiler/src/mobile_build.rs`.
  - Do: refuse an icon that is not square or is smaller than 1024 px, and give the reason. Produce the platform sets through `tauri icon`, as T1
    confirmed. If T1 shows the CLI cannot be used here, resize in Rust with the image crate the workspace already has, if it has one; record which.
  - Verify: `cargo test -p cobolt-compiler --test mobile_shell icon`. A non-square image and a 512 px image are refused with their reasons. The
    generated set's file list is printed (the CLI half is `#[ignore]` when the CLI is absent).

- [ ] **T13 — `build_mobile`: Web build → generate → Tauri CLI → `dist/mobile/<platform>/`; release-signing checks; store checklist** (R1, R4, R5,
  R13; AC1 driver, AC4 refusal; D-Sign)
  - Files: `crates/cobolt-compiler/src/mobile_build.rs`; `crates/cobolt-compiler/tests/mobile_shell.rs`.
  - Do:
    - `MobileBuildOptions` carries the profile (debug/release) and the signing secrets **in memory**. `MobileBuildResult` carries the artefacts and
      the checklist.
    - Before any process starts, a release build is refused if the signing names are unset, a password is absent, or the keystore is inside the
      project.
    - Passwords reach the `tauri android build` child **through its environment only** (D-Sign, the route confirmed in T1).
    - After the build, the artefact is checked: `apksigner verify --print-certs` must not show `CN=Android Debug`, and `codesign -dvv` must name a
      distribution identity. A failure refuses the build with its reason.
    - The R13 checklist is printed from the feature table.
    - Debug builds sign automatically.
    - The compiler never touches the secret store (the IDE passes the secrets in).
  - Verify: `cargo test -p cobolt-compiler --test mobile_shell --no-fail-fast`. New tests:
    - release with no names, and with a missing password, is refused before any process (a fake `PATH` with a stub `cargo`/`tauri` proves it never
      ran);
    - the checklist for the first set is printed and compared;
    - the child's environment carries the passwords, and no file in the staging folder does.

    `#[ignore]` `android_debug_build_produces_apk_and_aab` gives the exact command in its doc comment (it runs in T23).

## M4 — toolchain and devices

- [ ] **T14 — `mobile_toolchain.rs`: detection, next steps, safe installs** (R2; AC2)
  - Files: `crates/cobolt-compiler/src/mobile_toolchain.rs` (new); `src/lib.rs`.
  - Do:
    - `MobileTool`/`MobileToolStatus` rows (present / missing / wrong version; purpose; next step), from `detect(platform)`. It checks:
      - `ANDROID_HOME`/`ANDROID_SDK_ROOT`, then the default SDK folder;
      - `NDK_HOME`, then `ndk/`;
      - `JAVA_HOME`, then `java -version`;
      - on macOS only, `xcode-select -p` and `xcodebuild -version`;
      - `rustup target list --installed`;
      - the Tauri CLI against `TAURI_CLI_VERSION`.
    - `install(tool)` accepts only Rust targets (`rustup target add`) and, per O3, the Tauri CLI. It returns an error for Xcode, the SDK, the NDK and
      the JDK.
    - Off macOS, iOS is `Unavailable { reason }`.
  - Verify: `cargo test -p cobolt-compiler mobile_toolchain --no-fail-fast`, with a fake environment: variables plus a temp-dir `PATH` of stub
    executables, written by the test in Rust.
    - Each tool hidden in turn is named with its purpose and next step.
    - `install` refuses Xcode, the SDK, the NDK and the JDK.
    - iOS is unavailable off macOS, with the reason (a `cfg`-driven test).
    - A tool × state table is printed (AC2).

- [ ] **T15 — The player's shell bridge** (R8, R10, R11, R12; AC6 logic, AC7 logic, AC8 logic, AC9 logic)
  - Files: `crates/cobolt-web-player/src/` (a new `shell.rs`; `host.rs` `verb` `:1566` for `SECURE-*`, and `NAVIGATE`/`OPENURL` `:1601` routed to
    the opener inside the shell; the Android rationale dialog; Back → navigation pop of WASM-011 R13, and on an empty stack the shell is asked to go
    to the background; lifecycle → spec 037 events, and `net.rs` reconnects on resume); `assets/web-player/` (rebuilt).
  - Do: write it in Rust through `wasm-bindgen`/`js-sys`. Detect the shell, and call its commands only when the shell is present. A refusal answers
    `-6`.
  - Verify:
    - `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib --no-fail-fast`, with a fake shell object:
      - Back pops, and on an empty stack the shell is asked to go to the background (AC7);
      - a refusal becomes `-6` → `ON EXCEPTION` (AC6);
      - an external address goes to the opener and an own-origin address does not (AC9);
      - resume reconnects, and an `expired` answer reaches sign-in (AC8).
    - Rebuild: `cargo run --manifest-path crates/cobolt-web-player/Cargo.toml --example build_player -- --out assets/web-player`. Then
      `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --test freshness` is green.

- [ ] **T16 — `mobile_devices.rs`: list, start, install and launch, log, stop** (R3; AC3 logic; D-Dev)
  - Files: `crates/cobolt-compiler/src/mobile_devices.rs` (new); `src/lib.rs`.
  - Do:
    - `DeviceTarget` comes from `emulator -list-avds`, `adb devices -l`, `xcrun simctl list devices --json`, and `xcrun devicectl` (per T1).
    - Write `start`, `install_and_launch` (the shell in development mode against `cobolt_project_tools::web::devserver`, with `adb reverse` on
      Android), `log_stream` (a `Receiver<String>`) and `stop`.
    - On a physical iOS device, LAN exposure is opt-in per run, with a warning.
    - Process spawning only; no UI.
  - Verify: `cargo test -p cobolt-compiler mobile_devices --no-fail-fast`. Each list command's recorded sample output, embedded in the test as a
    string, parses into the expected targets. Stub executables prove that `start`/`stop` call the right commands.

## M5 — the IDE and the agent tool

- [ ] **T17 — Build wiring: the IDE continues into the mobile half** (R1, R14; AC1 driver, AC10)
  - Files: `crates/cobolt-ide/src/app.rs` (`start_web` `:2710`; the menu entries **Run on device…** and **Signing…**, shown for a Mobile project
    only).
  - Do:
    - For a Mobile project, after the Web build succeeds, run `build_mobile` on a worker thread.
    - Before the build, show T14's missing tools with their next steps. For a release build, read the passwords with T10's strict calls; if the
      store is unavailable, refuse with the reason.
    - Write progress and the checklist to the Output panel.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`. A test of the dispatch function: Desktop and Web take exactly their current
    path (AC10), and Mobile continues into the mobile step.

- [ ] **T18 — `rcrun mcp` `build`: the mobile half, debug only** (R1, R2; D-Sign)
  - Files: `crates/cobolt-project-tools/src/tools/build.rs` (replace the `"package": {"built": false …}` answer at `:126-135`).
  - Do:
    - A Mobile project builds a debug package and reports the artefacts, or the missing tools from T14.
    - A release request is answered with the reason: release signing is from the IDE only.
    - Off macOS, iOS keeps today's "only on macOS" reason.
  - Verify: `cargo test -p cobolt-project-tools --no-fail-fast`. A release request answers the reason. A Mobile project on a machine without the tools
    answers the T14 rows. The Web and Desktop answers are unchanged.

- [ ] **T19 — The Run on device window** (R3; AC3)
  - Files: `crates/cobolt-ide/src/panels/mobile_devices.rs` (new); `panels/mod.rs`.
  - Do:
    - The window has the target list (emulators, simulators, devices, and an empty-list text), Start/Stop, and the log streamed to the Output
      panel.
    - It opens at an explicit default size and is never sized from available space.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide mobile_devices`. The list model is filled from a fake `DeviceTarget` list, and Stop sends the
    stop. Manual (the operator's): the window in six languages at its size.

- [ ] **T20 — The Signing window** (R4, R5; AC4; D-Store)
  - Files: `crates/cobolt-ide/src/panels/mobile_signing.rs` (new); `panels/mod.rs`.
  - Do:
    - Names go into `cobolt.toml` `[mobile.signing]`; that is the only project write, and user code is sacred.
    - Passwords go into T10's strict store as `mobile-signing/<project-id>/<name>`. The window offers Replace and Delete.
    - A keystore inside the project folder is refused.
    - "Secret store unavailable — cannot continue" is shown when T10 errs.
    - An explicit default size.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide mobile_signing`:
    - the save function writes the names and never a password (the saved `cobolt.toml` is read back and scanned for the fake password);
    - an unavailable store blocks the save.

- [ ] **T21 — i18n ×6 and the System KB** (all R; i18n; System KB rule)
  - Files: `crates/cobolt-ide/src/i18n.rs` (the `Tr` keys ×6 of plan §7: every `MobileTool` name/purpose/next step, "unavailable on this machine",
    the device window, the signing window, the R5 refusals, the R7 message, the checklist heading); `crates/cobolt-compiler/src/lib.rs`
    (the Mobile row `:6187` and the `[mobile]` block in "Project kinds" `:6172`; the Build/Preview paragraph; the three `COBOL::"SECURE-…"` built-ins
    in the built-in table near `:5045`); `assets/knowledge/chunked.data` (regenerated).
  - Do:
    - Write Portuguese in pt-BR.
    - Text the user sees says PowerRustCOBOL, never `cobolt`.
    - `rcrun mcp` answers stay in English.
    - Regenerate the store with `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify:
    - `cargo test -p cobolt-ide --bin cobolt-ide no_empty_ui_translations non_english_is_actually_translated`;
    - `cargo test -p cobolt-ide --bin cobolt-ide the_system_kb_teaches_every_builtin_inline prebuilt_chunked_kb_matches_the_published_documentation`
      (T5's red test is now green);
    - the KB tests in `cobolt-compiler` (`published_documentation_tests`, `knowledge_base_build_tests`).

## M6 — device evidence and the ledger

- [ ] **T22 — AC10 re-run: Web and Desktop build exactly as before** (R14; AC10)
  - Files: none changed.
  - Verify: the T2 commands give the same counts, all green. T7's and T9's byte-identity tests are green. Compare the counts in this task.

- [ ] **T23 — Device runs, recorded, never claimed** (AC1, AC3, AC4 live, AC6, AC7, AC8, AC9)
  - Files: `specs/WASM-PROGRESS.md` (the WASM-012 row `:41`, and a dated evidence note); the `#[ignore]` tests from T11–T16.
  - Do: run each `#[ignore]` test where the tools exist, using its exact command. Record the platform, the command and the result for each:
    - **AC1:** Android `.apk` + `.aab`; the emulator shows the main form. On a Mac with Xcode: the iOS archive, and the simulator shows the main form.
    - **AC3:** the list shows targets, the application installs, an edit reloads, and the log appears in the Output panel.
    - **AC4 live:** a release build with the **operator's** signing material (never the agent's). `apksigner`/`codesign` show a non-debug identity.
    - **AC6:** pending O4. Recorded as "waits for the first confirmed Q1 feature".
    - **AC7:** the fixture page prints `--pr-safe-*`, rotation is locked, and Back pops.
    - **AC8:** background and return raise the events; the backend reconnects; an expired token leads to sign-in.
    - **AC9:** an omitted command is rejected, and an external link opens the system browser.
  - Verify: each AC has either a recorded result or a "not run here: <reason>, command: <cmd>" line. Nothing is claimed that was not run (WASM-PLAN
    §5).

- [ ] **T24 — Q1 features, one at a time — GATED on the operator's confirmation (O1)** (R8, R9; AC6 device)
  - Files: per feature: one `mobile_features.rs` row, one plugin in `crates/cobolt-mobile-shell/Cargo.toml` (passing the dependency guard), its
    `SECURE-*`-style built-ins (T5/T7/T15 pattern), one A10 row, `Tr` keys ×6, one KB entry, and a regenerated `chunked.data`.
  - Do: **do not start** until the operator names which of camera, location, notifications, biometric unlock and barcode scanning enter 2.0. The
    first confirmed feature that prompts carries AC6's device half.
  - Verify: per feature, the T4/T7/T8/T15/T21 checks, plus the T23 AC6 run.

## M7 — finalize

- [ ] **T25 — Finalize: version, CHANGELOG, tests of the touched crates**
  - Files: `crates/cobolt-ide/src/version.rs` (`z` +1 once: `2.0.30` → the next free `2.0.z` at that time; never `x` or `y`); `CHANGELOG.md` (one
    entry above the previous heading, dated, "Written on features-2x", listing what landed and what stays open: O1–O8, and the T23 lines not run).
  - Do: run `df -h .` first. Run the tests of the crates touched, with `--no-fail-fast`, and read every `test result` line:
    - `cargo test -p cobolt-compiler -p cobolt-runtime -p cobolt-wasm -p cobolt-wasm-abi -p cobolt-web-bundle -p cobolt-project-tools --no-fail-fast`;
    - `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`;
    - `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast`;
    - `cargo test --manifest-path crates/cobolt-mobile-shell/Cargo.toml --test dependency_guard`.
  - Verify:
    - all of the above are green, and the counts are recorded here;
    - the staged diff has no credential (`AKIA`/`ASIA`, keystore passwords, the fake test passwords outside tests);
    - `git status` shows nothing under `docs/` and no `README.md` change;
    - manual (the operator's): launch the IDE and confirm the version string; open the two new windows in six languages.

## Done criteria
- Every acceptance criterion in spec.md is covered by at least one task's verification:
  - AC1: T13, T17, T23;
  - AC2: T14;
  - AC3: T16, T19, T23;
  - AC4: T3, T10, T11, T13, T20, T23;
  - AC5: T4, T7, T11;
  - AC6: T15 logic; the device half is T23/T24 (O4);
  - AC7: T9, T11, T15, T23;
  - AC8: T15, T23;
  - AC9: T11, T15, T23;
  - AC10: T2, T7, T9, T17, T22.
- The touched crates' tests pass.
- Nothing under `docs/` or `README.md` changes.
- The open items O1–O8 are carried, not decided.
- Do **not** commit or push; the pipeline does the bookkeeping.
