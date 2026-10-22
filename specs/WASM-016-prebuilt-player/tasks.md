<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The prebuilt player (WASM-016)

- **Status:** in progress. T1–T13 were done against the plan of 2026-10-20 and stay checked with their evidence. T14–T29 are
  this revision's work, against the plan of 2026-10-22 (Leptos adopted, JSON description, pinning, layers, `"s"`, `nightly-2026-10-22`).
- **Plan:** ./plan.md   **Date:** 2026-10-22

`[x]` was done and verified; `[ ]` is still to do. The commit that introduced each done task is on `features-2x`. A done task that
the new plan replaces names the task that replaces it (**superseded by …**); its evidence stays true of the code it described.

Paths under `src/`, `tests/`, `examples/` without a crate are `crates/cobolt-web-player/…` (its own Cargo workspace,
`crates/cobolt-web-player/Cargo.toml:29`). `PLAYER` below stands for `--manifest-path crates/cobolt-web-player/Cargo.toml`.

**Freshness rule for every open task.** `fingerprint::INPUTS` (`src/fingerprint.rs:33`) includes `crates/cobolt-forms/src`,
`crates/cobolt-wasm-abi/src`, `crates/cobolt-ast/src`, `crates/cobolt-graphics/src` and the player's own sources. A task that
changes any of them ends with `cargo run PLAYER --example build_player -- --out assets/web-player` and
`cargo test PLAYER --test freshness` green, so the tree stays green after every task.

## Done (plan of 2026-10-20)

- [x] **T1 — ABI v1 host** (R7, R8, R9, R11)
  - Files: `src/host.rs`, `src/host/{auth,params,super}_tests.rs`, `tests/abi_host.rs`
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib --test abi_host` — 219 + 21 tests green; the host
    runs the fixture module in `wasmi` in both memory variants.
- [x] **T2 — Form tree from a description** (R3, R4, R5): `src/tree.rs`; the description is the sanitised `.cfrm` (Q1).
  **Superseded by T19–T20** (Q1 decided: JSON); `src/tree.rs` stays the tree builder, fed by `to_form`.
- [x] **T3 — The view layer as data** (R5, R15): `src/view/*` returns `VNode`; every class A and B control (WASM-004) has a view and an
  interaction function with tests.
- [x] **T4 — DOM patching, events, page state** (R5, R6, R7, R15): `src/dom/*`, `src/api.rs`; `tests/sinks.rs` proves no HTML-string API,
  no string execution and no leaked closure (it caught one in this session: the `pagehide` listener of `superlink.rs`, now owned and freed).
  **Superseded by T22–T24** (the Leptos `Surface`); `dom/{canvas,find,captcha,sheets}.rs` are kept.
- [x] **T5 — Connection to the backend and Authentication** (R9): `src/net.rs`, `src/auth.rs`, `src/dom/captcha.rs`; proved against a mock backend over a
  real WebSocket in Chrome (focus after a refused password, hosted return, reCAPTCHA origins).
- [x] **T6 — Routing, the two application styles, standalone windows** (R10): see WASM-011.
- [x] **T7 — The loader, the parts, the variants** (R2, R13, R13c): `crates/cobolt-web-bundle/assets/loader.js`; parts Mermaid, PDF, Canvas 2D.
  **The hand-written loader is replaced by WASM-002's Rust-emitted loader** (plan C7) — that is WASM-002's task, not this list's.
- [x] **T8 — Versions and refusal of a mismatched module** (R11, AC6): `tests/abi_host.rs`, the bundle's import check.
- [x] **T9 — Two variants, one source** (R13c): plain on stable; threaded on `nightly-2026-10-08` with `-Zbuild-std`, atomics, `wasm-bindgen-rayon`.
  - Verified in Chrome 152: isolated page (`require-corp` and `credentialless`) -> threaded variant on a Worker, a pool of 8 workers;
    non-isolated page -> plain variant on the main thread, threaded files not fetched; 300,000-row DataGrid `Sort` equals a stable JS sort,
    128-138 ms threaded warm against 150-172 ms plain.
  **The nightly moves to `nightly-2026-10-22` in T18** (WASM-017 Q9).
- [x] **T10 — Disposal** (R13b): `unmount()` frees everything the page holds (listeners, channels, timers, the connection); node scenario `disposal`: 100 open/close
  cycles leave one live instance. **Caveat, not fixable here:** the Rayon worker pool cannot be disposed (`wasm-bindgen-rayon` has no API for it).
- [x] **T11 — Source fingerprint and the freshness test** (R12, AC7)
  - Files: `src/fingerprint.rs`, `examples/build_player.rs`, `tests/freshness.rs`, `assets/web-player/`
  - Verify: edit any input source -> `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --test freshness` fails naming the changed file's effect; rebuild with
    `cargo run --manifest-path crates/cobolt-web-player/Cargo.toml --example build_player -- --out assets/web-player` -> passes. (Shown in this session; the inputs are
    listed in `fingerprint::INPUTS`. **After merging other agents' commits that touch `cobolt-forms`, `cobolt-ast`, `cobolt-wasm-abi` or `cobolt-graphics`, rebuild once.**)
- [x] **T12 — Build reports sizes** (R13, AC8): `player-build.json`; plain 2,222,253 bytes (gzip 772,899), threaded 2,187,394 (gzip 773,318), parts Mermaid 1,906,933 / Canvas 2D 469,419 / PDF 219,273.
  These are the **baseline** of plan §4 (at `opt-level = "z"`); T18 re-measures at `"s"`.
- [x] **T13 — Dead code removed, no `allow`** (steering): the 19 warnings are gone by deleting or using the code; five placeholder functions that hid unused imports are gone too.

## To do (plan of 2026-10-22), in execution order

- [ ] **T18 — Build profile and toolchain per the rulings** (R12, R13c; plan C5, C6)
  - Files: `examples/build_player.rs` (`NIGHTLY` at line 35 → `"nightly-2026-10-22"`; default `--opt-level` `s`; `wasm-opt -Oz` at
    lines 154 and 202 → `-Os`; the record's `profile.wasm_opt` at line 283), `Cargo.toml` (`[profile]` `opt-level = "z"` at line 108 → `"s"`,
    the comment at 104 states the ruling and the measured cost), `assets/web-player/` (rebuilt).
  - Do: change the profile, rebuild both variants and the three parts, run `--measure` and record the sizes against the T12 baseline.
  - Verify: `cargo run PLAYER --example build_player -- --out assets/web-player --measure` prints the sizes; `cargo test PLAYER --test freshness`
    green; `the_build_record_names_the_tools_and_the_pinned_nightly` asserts `nightly-2026-10-22` and `opt_level` `s`.

- [ ] **T19 — The form description: a versioned JSON schema in `cobolt-forms`** (R3, R4, R18; Q1; plan §1.3, §1.6, §3)
  - Files: `crates/cobolt-forms/src/description.rs` (new), `crates/cobolt-forms/src/lib.rs` (`pub mod description;`, model-only — not behind
    the `render` feature, since the player uses `default-features = false`, `crates/cobolt-web-player/Cargo.toml:55`).
  - Do: own types (`FormDescription`, `ControlDescription`, `LayerDescription`) with `DESCRIPTION_FORMAT: u32 = 1`; `describe(&Form)`,
    called on a form from `load_form_from_str` (`crates/cobolt-forms/src/xml.rs:595`); `to_form(&FormDescription) -> Result<Form, DescriptionError>`.
    Properties as `PropValue` (already `Serialize`, `crates/cobolt-forms/src/model.rs:54`). Layers in stack order with name and backdrop, never
    `Visible` (`Layer`, `model.rs:8316`); each control's `layer`, absent = `Form`; an empty `layers` is not serialised. No field for handler code,
    `user_ws_source`, `cobol_structure`, `user_procedures` or `deleted_code`. Added fields are `#[serde(default)]`.
  - Verify: `cargo test -p cobolt-forms description` — (a) `to_form(describe(f))` equals the code-free form for **every example form** under
    `examples/` (prints the count); (b) the JSON of a form whose handlers carry code contains none of it (R4); (c) a form with no layers
    serialises no `layers` key, a layered form round-trips stack order and each control's layer (R18); (d) a description with a higher `format`
    is refused with both numbers named. `cargo test -p cobolt-forms --features render` still green.

- [ ] **T20 — The Build and the player read JSON** (R3, R4, R11; plan §1.3)
  - Files: `crates/cobolt-web-bundle/src/lib.rs` (`form_description` at line 322 writes JSON through `describe`; the clearing at 337–340
    removed; `DESCRIPTION_VERSION` at 47 re-exports `DESCRIPTION_FORMAT`; the config's form `url` names a `.json` file), `crates/cobolt-web-bundle/src/devserver.rs`
    (`json` MIME, beside the `cfrm` arm at line 88), `crates/cobolt-web-bundle/src/secrets.rs` (scans the JSON), `src/api.rs`
    (`add_form(json)` at line 821; `versions()` at 1149 reads `DESCRIPTION_FORMAT` instead of the literal `1` at 1152), `src/tree.rs` (built from `to_form`).
  - Do: the loader needs no edit — it fetches `config.forms[i].url` and passes the text to `add_form` (`loader.js:737-739`); the URL is written by Rust.
  - Verify: `cargo test -p cobolt-web-bundle` (assembler, CSP, `e2e_node`, `no_string_execution`) green; a new bundle test asserts every
    description is JSON with `format` = `DESCRIPTION_FORMAT` and that a planted secret in a property fails the scan; `cargo test PLAYER --lib`
    has a test that `add_form` of an unknown `format` returns an error naming both numbers. Player rebuilt (freshness rule).

- [ ] **T21 — Form layers in the player** (R18, R19; plan §1.6; AC12)
  - Files: `src/host.rs` (per-instance `Visible` set, created empty when `pr_open_form` creates the instance, applied on the next frame,
    dropped with the instance), `src/tree.rs` (each control's layer), `src/view/*` (a layer is a stacking context; a hidden layer is `inert`, D20).
  - Depends on: ABI A9 in the generator and `forms[i].layers` in the manifest (`FormEntry`, `crates/cobolt-wasm-abi/src/manifest.rs:67`, has no
    `layers` field today) — WASM-004/010's work. Until it lands, the host tests drive layer ids `3 + n` directly through the fixture.
  - Verify: `cargo test PLAYER --lib layer` — a layered form opened, a layer shown, closed, opened again starts with every layer hidden, both
    times; after release the instance's layer state is gone (no entry for its id); a form with no layers allocates none. Player rebuilt.

- [ ] **T16 — Leptos admitted to the player's workspace** (R1; plan C1, §5 first risk)
  - Files: `Cargo.toml` (`leptos`, client-side rendering only), `Cargo.lock`, `tests/dependency_guard.rs` (Leptos's closure on the allow-list).
  - Do: pick the newest Leptos release whose MSRV is ≤ 1.92 and which accepts `wasm-bindgen = "=0.2.129"` (`Cargo.toml:75`), or move the
    pinned CLI and crate together; record the release, its MSRV and its licence in the `Cargo.toml` comment. **Gate: if no release qualifies,
    stop and report to the operator** — never pin a nightly for the plain variant.
  - Verify: `cargo test PLAYER --test dependency_guard` green (it lists what was admitted); `cargo build PLAYER --target wasm32-unknown-unknown`
    on stable 1.92; the licence check passes. Player rebuilt.

- [ ] **T22 — The Leptos `Surface`, beside `dom/`** (R5, R6, R7, R13b, R15; plan §1.2)
  - Files: a new module under `src/` implementing `pub(crate) trait Surface` (`src/api.rs:285`), a cargo feature selecting it, `src/api.rs`
    (mount on the element the loader hands over).
  - Do: one reactive store per instance (its `VNode` tree, page-only state, the layer `Visible` set of T21); one generic component renders a
    `VNode`, its listeners build the existing `Payload` (`src/dom/payload.rs`) and call `view::interact::on_event`; canvas, Find, captcha and
    stylesheets reached through `NodeRef` using `dom/{canvas,find,captcha,sheets}.rs`; `unmount()` (`src/api.rs:1080`) disposes the Leptos
    owner, then the listed `wasm-bindgen` objects.
  - Verify: `cargo test PLAYER --lib` green (views unchanged); `cargo test -p cobolt-web-bundle --test e2e_node` passes with the player built
    **once with each feature**, every scenario named in the report passing on both (including `disposal`: 100 open/close cycles, one live instance).

- [ ] **T23 — `inner_html` forbidden; the source search covers the Leptos code** (R15; AC10)
  - Files: `tests/sinks.rs` (add `inner_html` to `FORBIDDEN`, line 23; a self-test sample line for it).
  - Verify: `cargo test PLAYER --test sinks` prints the files and lines searched and zero hits; the sample-line test catches `inner_html`.

- [ ] **T24 — `dom/` retired; Leptos is the only browser half** (R1, R5; plan §1.2 "Order of work")
  - Files: remove `src/dom/apply.rs` and the instance code of `src/dom/{mod,instance,ops}.rs`; remove the feature of T22; `src/lib.rs`'s layer table.
  - Verify: `cargo test PLAYER` (lib, `abi_host`, `sinks`, `dependency_guard`, `freshness`) and `cargo test -p cobolt-web-bundle` green with no
    warnings and no `allow`; AC3–AC5 re-proved on the Leptos surface by `e2e_node` with `examples/mock_backend.rs` (click → handler, `SET` in the
    DOM, `CALL` reaches the control, `DISPLAY` on the console; suspension, token on the connection, two queued events in order; back-off and
    `ON EXCEPTION` for the call in flight only). Player rebuilt; sizes compared with T18 and recorded here.

- [ ] **T25 — Version triple and pinning an older player** (R11, R12; Q6; plan §1.4; AC6)
  - Files: `examples/build_player.rs` (the triple `{abi, description, player}` in `player-build.json`), `src/api.rs` (`versions()` answers the
    triple from `cobolt_wasm_abi::ABI_VERSION` (`crates/cobolt-wasm-abi/src/codes.rs:11`), `DESCRIPTION_FORMAT` and the crate version),
    `crates/cobolt-compiler/src/project_kind.rs` (optional `player` in `WebSettings` line 273 and `MobileSettings` line 320; absent = current),
    `crates/cobolt-project-tools/src/web/mod.rs` (the pin in `WebBuildRequest` line 46 via `from_manifest` line 71; a pin variant of
    `WebBuildError` line 194), `crates/cobolt-web-bundle/src/lib.rs` (`assemble_bundle` line 415 copies the current or the pinned player),
    `assets/web-player/archive/` (layout `archive/<version>/` with its own `player-build.json`), `tests/freshness.rs` (checks every archived record).
  - Working assumption (plan **Q-P2, to confirm**): the IDE keeps every player released on a `2.x.0`, plus the last `z` of the previous minor; a pin
    to a version not kept fails the Build listing the kept versions. Not decided here.
  - Verify: `cargo test -p cobolt-compiler project_kind` (old manifests read unchanged; `player = "2.0.0"` reads); `cargo test -p cobolt-project-tools web`
    — a pin whose `abi` or `format` the compiler cannot emit fails naming the pinned version and the oldest it can emit; a pin to an absent
    version lists the kept ones; `cargo test -p cobolt-web-bundle` — a player lacking an import the module needs fails the Build (AC6, first half;
    the module's refusal at start is T8's `abi_host` test, re-run here); `cargo test PLAYER --test freshness` checks the archive.

- [ ] **T26 — Charts, maps and the Viewer as parts on first use** (R13; Q3; plan §1.7; AC8)
  - Files: `crates/cobolt-web-player/parts/` (new parts where they split without a second copy of the core), `examples/build_player.rs`
    (builds and records them), `crates/cobolt-web-bundle/src/lib.rs` (lists them in the config with their integrity).
  - Do: split each one that can be split; for one that cannot, write the reason in this task's evidence rather than a half-split.
  - Verify: `cargo test -p cobolt-web-bundle --test e2e_node` — a form with no chart, map or Viewer fetches none of their parts (the scenario
    lists what was fetched); a form with a chart fetches the chart part once. Player rebuilt; sizes recorded.

- [ ] **T27 — The Build report and the pin error, in six languages** (R13; AC8; i18n)
  - Files: `crates/cobolt-project-tools/src/web/mod.rs` (`BuildReport` line 164 and `PartSize` line 125 carry the player's size and each part's,
    read from `player-build.json`), `crates/cobolt-ide/src/panels/web_preview.rs` (the panel that shows `BuildReport` / `WebBuildError` today),
    `crates/cobolt-ide/src/i18n.rs` (new `Tr` keys for the size lines and the pin error, ×6 languages).
  - Verify: `cargo test -p cobolt-project-tools web` — the report holds the player's and every part's size, equal to `player-build.json`;
    `cargo test -p cobolt-ide --bin cobolt-ide i18n` — no empty translation for the new keys.

- [ ] **T14 — The IDE finds the player beside the executable** (R1, R12; plan §1.4)
  - Files: `crates/cobolt-ide/src/app.rs` (a `web_player_dir()` resolved as `images_dir()` (line 20415) and `CoboltApp::themes_dir()` (line 2442)
    are, never from `CARGO_MANIFEST_DIR`; the test `bundled_directories_resolve_beside_the_executable` (line 21886) extended to it); the call into
    the Web Build passes that directory.
  - Depends on: the IDE Build step of WASM-001/002 (it already calls `assemble_bundle`); the installers copying `assets/web-player/` is the
    packaging step's, named here so it is not forgotten.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide bundled_directories_resolve_beside_the_executable` green.

- [ ] **T28 — No toolchain on the developer's machine** (R1, R2; AC1)
  - Files: `crates/cobolt-web-bundle/tests/e2e_node.rs` (or a new test beside it).
  - Do: build a Web project into a bundle with `PATH` stripped of `cargo`, `rustc` and `rustup`, from the shipped `assets/web-player/`
    (not `support::built_player()`, which builds it).
  - Verify: `cargo test -p cobolt-web-bundle --test e2e_node` — the test lists the bundle's files and asserts exactly: player plain and threaded,
    program multi-memory and single-memory, host page, descriptions, stylesheet, assets; the page runs headless under node. AC10's other halves are
    re-run here: `cargo test -p cobolt-web-bundle --test csp_origins` (the page under the generated CSP) and `cargo test PLAYER --lib net`
    (a connection to an origin outside the configured backend is refused).

- [ ] **T15 — The Preview replaces the module, a description, the stylesheet in place** (R16; AC11)
  - Files: the Preview's path through `cobolt_project_tools::web::preview` (`crates/cobolt-project-tools/src/web/preview.rs`) and
    `crates/cobolt-web-preview/src/main.rs`; the player's entry points already exist (`set_stylesheet` `src/api.rs:1028`, `add_form` 821, `attach` 832).
  - Depends on: WASM-008 (the Preview). `crates/cobolt-web-preview/src/main.rs` carries no description today (no `add_form` or `description` in it).
  - Verify: a `cobolt-web-bundle` node scenario (headless): replacing the module, one description and the stylesheet each changes the DOM, and
    the player instance id is the same before and after (no reload).

- [ ] **T17 — Equality with the desktop form host by measurement** (R6, R14; Q4; AC2, AC9)
  - Files: `specs/WASM-016-prebuilt-player/differences.md` (new: the expected differences — entrance and exit effects, window states, window
    positions, per the operator's 2026-10-22 answer to Q4; a verb that asks for one is accepted and does nothing), the R14 harness (a test that runs
    the WASM-004 R11 reference form and the spec 037/042 lifecycle and event-order scripts on `cobolt-form-host` and, under node, on the player).
  - Depends on: the WASM-004 R11 reference form and property round-trip harness.
  - Verify: the harness prints, per check, desktop result / player result / equal or listed difference; it fails on an unlisted difference.
    AC2: every class A and B control drawn from its description with each property set to a distinct value and read back from the DOM.
    Manual (plan §6): Chrome on 127.0.0.1, the variant choice, the Rayon DataGrid sort, parts on first use; the operator looks at the reference
    form beside the desktop.

- [ ] **T29 — System KB** (steering: System KB rule)
  - Files: `crates/cobolt-compiler/src/lib.rs` (the settings table at line 6202: the optional `player` key under `[web]` and `[mobile]`; a short
    table of the form-description format and its `format` number), `assets/knowledge/chunked.data` (regenerated).
  - Verify: `cargo run -p cobolt-ide --example build_chunked_kb`; `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` green.

- [ ] **T30 — Finalize** (all)
  - Do: bump `z` **once** in `crates/cobolt-ide/src/version.rs` (`VERSION`, line 22; `2.0.29` at the time of writing — bump whatever it is then);
    add **one** `CHANGELOG.md` entry above the previous `## [PowerRustCOBOL …]` heading, dated, marked *Written on features-2x*. No
    documentation task: `README.md` and `docs/` are never touched on 2.x.x.
  - Verify, with `--no-fail-fast`, every crate touched, reading every `test result` line:
    `cargo test PLAYER --no-fail-fast`; `cargo test -p cobolt-forms --no-fail-fast` and `cargo test -p cobolt-forms --features render --no-fail-fast`;
    `cargo test -p cobolt-web-bundle --no-fail-fast`; `cargo test -p cobolt-project-tools --no-fail-fast`; `cargo test -p cobolt-compiler --no-fail-fast`;
    `cargo test -p cobolt-web-preview --no-fail-fast`; `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`. `cargo fmt --check`; no warnings.
    `df -h` before the sweep (each test binary is large).

## Coverage of the acceptance criteria

| AC | Tasks whose verification covers it |
|---|---|
| AC1 | T28 (T14, T18 feed it) |
| AC2 | T19 (description round trip), T17 (every control drawn, property round trip) |
| AC3 | T1, T24 |
| AC4 | T1, T5, T24 |
| AC5 | T5, T24 |
| AC6 | T8, T25 |
| AC7 | T11, T18, T25 (the archive) |
| AC8 | T12, T18, T26, T27 |
| AC9 | T17 |
| AC10 | T4, T23, T28 |
| AC11 | T15 |
| AC12 | T21 |

## Still open (not decided by this list)

- **Q-P1 — `cargo-leptos` for the player (to confirm).** Working assumption: `build_player` keeps driving `cargo` + the pinned `wasm-bindgen` CLI +
  `wasm-opt`, Leptos used as a library; `cargo-leptos` builds the server (WASM-003). If the operator wants `cargo-leptos` here, T18 changes the
  build call and the record names it; no other task changes.
- **Q-P2 — Which older players are kept (to confirm).** Working assumption in T25.
- **WASM-017 Q5 (isolation on the mobile shell) — not answered.** Until it is, the Mobile kind runs the plain player.

## Done criteria

Every AC above is covered by a passing task, the crates touched pass with `--no-fail-fast`, `differences.md` lists every expected difference
of the R14 harness, `README.md` and `docs/` are untouched, and the Leptos gate of T16 either passed or was reported to the operator.
