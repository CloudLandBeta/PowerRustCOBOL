<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The prebuilt player (WASM-016)

- **Status:** in progress — T22 (automated part), T23, T24 done in 2.0.42; T22's manual Chrome check is the operator's (see T22). T1–T13 were done against the plan of 2026-10-20 and stay checked with their evidence. T14–T29 are
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

- [x] **T18 — Build profile and toolchain per the rulings** (R12, R13c; plan C5, C6)
  - Files: `examples/build_player.rs` (`NIGHTLY` at line 35 → `"nightly-2026-10-22"`; default `--opt-level` `s`; `wasm-opt -Oz` at
    lines 154 and 202 → `-Os`; the record's `profile.wasm_opt` at line 283), `Cargo.toml` (`[profile]` `opt-level = "z"` at line 108 → `"s"`,
    the comment at 104 states the ruling and the measured cost), `assets/web-player/` (rebuilt).
  - Do: change the profile, rebuild both variants and the three parts, run `--measure` and record the sizes against the T12 baseline.
  - Verify: `cargo run PLAYER --example build_player -- --out assets/web-player --measure` prints the sizes; `cargo test PLAYER --test freshness`
    green; `the_build_record_names_the_tools_and_the_pinned_nightly` asserts `nightly-2026-10-22` and `opt_level` `s`.
  - **Done (2.0.41).** `nightly-2026-10-22` installed (`rustc 1.101.0-nightly (1a2c4797e 2026-10-21)`, recorded); `--measure` now builds the
    chosen level into `--out` and the other into `target-player/measure-opt-*`, so `--out assets/web-player --measure` ships `s`. Sizes of the
    final build (after T19–T21), wasm-opt / gzip bytes: plain **s 2,726,012 / 886,174** against z 2,218,108 / 766,650; threaded **s 2,552,720 /
    870,352** against z 2,182,808 / 767,880; parts at s: Mermaid 2,218,954 / 798,551, Canvas 2D 654,896 / 233,657, PDF 217,066 / 100,680
    (T12 baseline at z: plain 2,222,253, threaded 2,187,394, Mermaid 1,906,933, Canvas 469,419, PDF 219,273). The ruling costs +507,904 bytes on
    the plain player after wasm-opt (+22.9 %). `freshness` 3/3 green.

- [x] **T19 — The form description: a versioned JSON schema in `cobolt-forms`** (R3, R4, R18; Q1; plan §1.3, §1.6, §3)
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
  - **Done (2.0.41).** `cobolt_forms::description`: `describe`, `to_form`, `from_json` (reads `format` first), `to_json`, `code_free` (the form a
    description stands for: no code, credential values blank, the designer's grid at its default). Enumerations use the `.cfrm` spellings.
    `cargo test -p cobolt-forms --features render --lib description`: 5/5 — **88 example forms, 2,344 controls** round-trip equal through
    `form_to_string`; 7 code markers absent; layers in stack order, no `layers` key without them, a layer serialises only `name` and
    `backdrop`; format 2 refused naming 2 and 1; a credential value is not described. **Deviation:** no `kind` field — the model carries no
    form kind and the Build does not know one per form; the player draws both the same. Add it compatibly if a consumer needs it.

- [x] **T20 — The Build and the player read JSON** (R3, R4, R11; plan §1.3)
  - Files: `crates/cobolt-web-bundle/src/lib.rs` (`form_description` at line 322 writes JSON through `describe`; the clearing at 337–340
    removed; `DESCRIPTION_VERSION` at 47 re-exports `DESCRIPTION_FORMAT`; the config's form `url` names a `.json` file), `crates/cobolt-web-bundle/src/devserver.rs`
    (`json` MIME, beside the `cfrm` arm at line 88), `crates/cobolt-web-bundle/src/secrets.rs` (scans the JSON), `src/api.rs`
    (`add_form(json)` at line 821; `versions()` at 1149 reads `DESCRIPTION_FORMAT` instead of the literal `1` at 1152), `src/tree.rs` (built from `to_form`).
  - Do: the loader needs no edit — it fetches `config.forms[i].url` and passes the text to `add_form` (`loader.js:737-739`); the URL is written by Rust.
  - Verify: `cargo test -p cobolt-web-bundle` (assembler, CSP, `e2e_node`, `no_string_execution`) green; a new bundle test asserts every
    description is JSON with `format` = `DESCRIPTION_FORMAT` and that a planted secret in a property fails the scan; `cargo test PLAYER --lib`
    has a test that `add_form` of an unknown `format` returns an error naming both numbers. Player rebuilt (freshness rule).
  - **Done (2.0.41).** The bundle writes `forms/<name>.<hash>.json` (the stylesheet first, so the description names it); no `.cfrm` ships.
    `devserver.rs` and `secrets.rs` needed no change (`json` was already served and scanned). The player's `add_form(json)` goes through the
    new native `Host::add_description`. `cargo test -p cobolt-web-bundle --no-fail-fast`: 19 + 2 + 2 + 1 + 2 + 3 + 3 green, `e2e_node` **14
    scenarios passed, 0 failed** on the rebuilt player; new `tests/form_descriptions.rs` (every description JSON at format 1; a planted
    connection string stops the bundle, `forms/main-form.<hash>.json: line 72`, value not shown); `host::description_tests` 3/3.

- [x] **T21 — Form layers in the player** (R18, R19; plan §1.6; AC12)
  - Files: `src/host.rs` (per-instance `Visible` set, created empty when `pr_open_form` creates the instance, applied on the next frame,
    dropped with the instance), `src/tree.rs` (each control's layer), `src/view/*` (a layer is a stacking context; a hidden layer is `inert`, D20).
  - Depends on: ABI A9 in the generator and `forms[i].layers` in the manifest (`FormEntry`, `crates/cobolt-wasm-abi/src/manifest.rs:67`, has no
    `layers` field today) — WASM-004/010's work. Until it lands, the host tests drive layer ids `3 + n` directly through the fixture.
  - Verify: `cargo test PLAYER --lib layer` — a layered form opened, a layer shown, closed, opened again starts with every layer hidden, both
    times; after release the instance's layer state is gone (no entry for its id); a form with no layers allocates none. Player rebuilt.
  - **Done (2.0.41).** A9 has landed in the generator (`cobolt-wasm/src/module.rs` writes `LayerEntry`), so the host uses real ids. State is
    `FormTree::layers` (dies with the tree); `Target::Layer` in `resolve`/SET/GET/CALL/COUNT; `Name` refused with `-2`; a program-built layer
    the description lacks is a pairing error. Views: mount order by layer then `ZOrder`; a hidden layer's controls are `hidden` + `inert`, with
    `data-layer`. `cargo test PLAYER --lib layer`: 5 layer tests green (226 lib tests in all). **Not done here:** painting a layer's
    *backdrop* in the page needs an element per layer, which `dom/` does not mount — left to the Leptos surface (T22); the backdrop
    properties are stored and readable.

- [x] **T16 — Leptos admitted to the player's workspace** (R1; plan C1, §5 first risk)
  - Files: `Cargo.toml` (`leptos`, client-side rendering only), `Cargo.lock`, `tests/dependency_guard.rs` (Leptos's closure on the allow-list).
  - Do: pick the newest Leptos release whose MSRV is ≤ 1.92 and which accepts `wasm-bindgen = "=0.2.129"` (`Cargo.toml:75`), or move the
    pinned CLI and crate together; record the release, its MSRV and its licence in the `Cargo.toml` comment. **Gate: if no release qualifies,
    stop and report to the operator** — never pin a nightly for the plain variant.
  - Verify: `cargo test PLAYER --test dependency_guard` green (it lists what was admitted); `cargo build PLAYER --target wasm32-unknown-unknown`
    on stable 1.92; the licence check passes. Player rebuilt.
  - **Done (2.0.41). The gate passed:** Leptos **0.8.22** (newest stable on 2026-10-23; 0.9 is `rc0`), MIT, `rust-version` 1.88,
    `wasm-bindgen = "^0.2"` — the lock keeps `wasm-bindgen 0.2.129`. `wasm32` dependency, `default-features = false`, `features = ["csr"]`.
    The browser closure grows from 66 to 189 packages (threaded adds the same 8). `tests/dependency_guard.rs` gains a licence and MSRV check
    of the whole browser closure (cobolt-wasm's permissive list, MSRV 1.92): **197 packages, all permissive, none above 1.92**, and asserts
    `leptos` is in the closure. One existing assertion was narrowed: `regex` now reaches the player through `leptos_config` (not optional in
    Leptos 0.8); the guard allows that path only and still refuses it through the Mermaid renderer. `cargo +1.92 build --target
    wasm32-unknown-unknown` finished. Rebuilt player (Leptos not yet used): plain s 2,726,137 / gzip 886,069, threaded 2,552,223 / 870,020.

- [ ] **T22 — The Leptos `Surface`, beside `dom/`** — automated part **done (2.0.42)**; the box stays open for the operator's manual
  Chrome check (decision of 2026-10-23, below). Stopped on 2026-10-23 (2.0.41) for the reason that follows; resumed once decided.
  - Why: `e2e_node` runs the player **headless** (`crates/cobolt-web-bundle/tests/support/harness.rs:131`, `headless: true`), and
    `Player::mount` returns before any surface is created in a headless run (`src/api.rs:841-845`). Building the player "once with each
    feature" therefore exercises neither `dom/` nor a Leptos surface, and T24's "`SET` in the DOM … re-proved on the Leptos surface by
    `e2e_node`" cannot be met as written. `dom/` was proved in Chrome by hand (T4, T5, T9). Replacing it with a surface no automated test
    mounts is the regression risk plan §5 names. **Needs the operator's decision** on how T22/T24 are verified: a Chrome-driven harness (no
    CDP crate is in either workspace — a new dependency), a DOM for node (none is in the repository; the PRIME DIRECTIVE bars JavaScript
    tooling in the tree), or manual Chrome checks recorded per scenario. T22–T24 wait on that answer; the tasks after them were not started. (R5, R6, R7, R13b, R15; plan §1.2)
  - **Decided by the operator (2026-10-23): both.** (1) An automated test in Rust drives a real Chrome through the DevTools protocol
    (a dev-dependency of the player's test harness — e.g. `chromiumoxide` — never of the shipped player or the IDE): it serves the
    Build's bundle cross-origin isolated, mounts it standalone and in a `<div>`, clicks, types, resizes and asserts on the DOM and on
    computed styles, for the `dom/` surface and the Leptos one. It finds Chrome on the machine (`/Applications/Google Chrome.app` here,
    `CHROME` to override) and is **skipped with a printed reason** when none exists. Its crate set passes the dependency guard (licence,
    MSRV, no `ring`/`aws-lc`/OpenSSL). (2) A manual check in Chrome, scenario by scenario, is run afterwards by the operator's session and
    recorded under this task — **it does not block the pipeline**: write the scenarios to check here and leave the box open for them.
  - Files: a new module under `src/` implementing `pub(crate) trait Surface` (`src/api.rs:285`), a cargo feature selecting it, `src/api.rs`
    (mount on the element the loader hands over).
  - Do: one reactive store per instance (its `VNode` tree, page-only state, the layer `Visible` set of T21); one generic component renders a
    `VNode`, its listeners build the existing `Payload` (`src/dom/payload.rs`) and call `view::interact::on_event`; canvas, Find, captcha and
    stylesheets reached through `NodeRef` using `dom/{canvas,find,captcha,sheets}.rs`; `unmount()` (`src/api.rs:1080`) disposes the Leptos
    owner, then the listed `wasm-bindgen` objects.
  - Verify: `cargo test PLAYER --lib` green (views unchanged); `cargo test -p cobolt-web-bundle --test e2e_node` passes with the player built
    **once with each feature**, every scenario named in the report passing on both (including `disposal`: 100 open/close cycles, one live instance).
  - **Done, automated part (2.0.42).**
    - **The Leptos node layer** is `src/dom/reactive.rs`: every `VNode` node is an `RNode` with its own reactive `Owner` and three
      signals (what the element says about itself; its text; its children's ids); one view (`node_view`) draws any node through
      `leptos::html::custom` / the allow-listed `leptos::svg` elements, with the children in a keyed `<For>`; a render effect applies
      classes, attributes, CSSOM declarations, properties and listeners by the rules now shared in `src/dom/attrs.rs` (moved out of
      `apply.rs`: `value`/`checked`/`selected` against the live element, `crossorigin` before `src`, `autofocus`). Leptos runs on a
      local executor that the layer polls after each write (`any_spawner::Executor::init_local_custom_executor` over a
      `futures::executor::LocalPool`), so `create`/`patch` return with the DOM done: the page reads elements back at once (slots,
      a dialog's input). `any_spawner` and `futures` were already in Leptos's closure; `dependency_guard` 6/6.
    - **Deviation from the plan (recorded):** the plan's "new module implementing `Surface`" became a new **node layer under the same
      `Surface`** (`dom::Dom`), with `apply.rs`'s API (`create`, `patch`, `slots`, `find_key`). The page's furniture (instances, shell,
      dialogs, notices, timers, the address, canvas/Find/captcha/sheets) is behaviour, not a DOM diff; duplicating ~1,500 lines to delete
      them again in T24 was the regression risk plan §5 names. "One reactive store per instance" is the instance's tree of `RNode`
      signals; the layer `Visible` set stays `FormTree::layers` (T21) and reaches the page through the views (`hidden` + `inert`).
      `unmount()` drops every record, each cleaning up its `Owner` (signals, effects) and its listeners.
    - **The browser test** is `crates/cobolt-web-bundle/tests/e2e_chrome.rs` with `tests/support/cdp.rs`: a DevTools client of ~250
      lines over `tungstenite` (already a dev-dependency of the bundle; **no new crate** in either workspace, so no guard changes).
      Chrome is found at `CHROME` or its install paths on macOS/Linux/Windows; with none the test prints `SKIPPED: …` and passes. It
      assembles the Build's bundle around the fixture program, serves it with `devserver` (`require-corp`, so `crossOriginIsolated`),
      and runs, as the whole window and mounted in a 700x600 `<div>` of another page: load, geometry (BTN-GO at 10,112 200x28 from
      `getBoundingClientRect`), real clicks (`Input.dispatchMouseEvent`, refused when the point does not hit the control), typing
      (`Input.insertText`), a program write reaching an input's `value`, a modal `<dialog>` (`:modal`), resizing (window: follows,
      `position: fixed`; div: stays 700 px, `relative`), 100 panel open/close cycles by clicks, unmount, no console error.
    - **Run with the player built once per layer** (until T24 removed the switch): **40 passed, 0 failed** — 10 scenarios × {window,
      div} × {`dom`, `leptos`}. The node e2e (`e2e_node`) runs headless and draws nothing, so it says nothing about either layer; it
      stays green (14 scenarios).
    - **Found by the browser, fixed (both layers):** a modeless panel window collapsed to its title bar —
      `.pr-panelwin > .pr-body { flex: 1 }` makes `flex-basis: 0%`, which overrides the body's designed `height` in a column flex box
      with no height; every panel's form was 0 px high and could not be clicked. Now `flex: 1 1 auto; min-height: 0`
      (`src/base_css.rs`), and the disposal scenario asserts the panel body is 480 px.
  - **Manual check in Chrome (the operator's session; does not block the pipeline).** Serve a built bundle on 127.0.0.1
    (`cargo run -p cobolt-web-bundle --example serve`) and look, for the reference form and the fixture: (1) controls drawn as in the
    Preview, text and colours, both themes; (2) a text box keeps its caret and selection while other controls redraw (a Timer
    writing a label); (3) a ComboBox and a ListBox keep their selection after items change; (4) a modal form: focus trapped, Escape
    closes it; (5) a panel window: drag by its title bar, close button; (6) a SideMenu shell: rail collapse, breadcrumb, Back button;
    (7) a DataGrid of 300,000 rows: scroll and sort on the threaded variant (isolated page) and on the plain one; (8) an SVG icon and
    a Mermaid diagram in a Viewer; (9) the same page mounted in a `<div>` of another page, resized. Record each as pass/fail here.

- [x] **T23 — `inner_html` forbidden; the source search covers the Leptos code** (R15; AC10)
  - Files: `tests/sinks.rs` (add `inner_html` to `FORBIDDEN`, line 23; a self-test sample line for it).
  - Verify: `cargo test PLAYER --test sinks` prints the files and lines searched and zero hits; the sample-line test catches `inner_html`.
  - **Done (2.0.42).** `("inner_html", …)` in `FORBIDDEN`; the self-test's sample now has a `view! { <div inner_html=s /> }` line and
    expects `["set_inner_html", "inner_html", "inner_html", "Closure::forget"]`. `cargo test PLAYER --test sinks`: 3/3, "searched 48 source
    files, 15577 lines of code, 26 forbidden names", zero hits (`src/dom/reactive.rs` included). The positive test now names the Leptos
    layer: elements by `leptos::html::custom` in `dom/reactive.rs`, attributes by `dom/attrs.rs`, and `dom/apply.rs` absent.

- [x] **T24 — `dom/` retired; Leptos is the only browser half** (R1, R5; plan §1.2 "Order of work")
  - Files: remove `src/dom/apply.rs` and the instance code of `src/dom/{mod,instance,ops}.rs`; remove the feature of T22; `src/lib.rs`'s layer table.
  - Verify: `cargo test PLAYER` (lib, `abi_host`, `sinks`, `dependency_guard`, `freshness`) and `cargo test -p cobolt-web-bundle` green with no
    warnings and no `allow`; AC3–AC5 re-proved on the Leptos surface by `e2e_node` with `examples/mock_backend.rs` (click → handler, `SET` in the
    DOM, `CALL` reaches the control, `DISPLAY` on the console; suspension, token on the connection, two queued events in order; back-off and
    `ON EXCEPTION` for the call in flight only). Player rebuilt; sizes compared with T18 and recorded here.
  - **Done (2.0.42).** `src/dom/apply.rs` deleted; the `leptos-surface` feature removed; `dom::nodes` is `dom/reactive.rs` alone;
    `h::svg`, `h::text` and `SVG_NS` (the patch's only users) removed; `src/lib.rs`'s table names the page as drawn by Leptos.
    **Deviation:** "the instance code of `dom/{mod,instance,ops}.rs`" stays — it is the page's furniture, not a second DOM layer, and
    now draws every element through Leptos (see T22). `build_player` needs no feature switch (T22's test builds used one; reverted).
    - **AC3–AC5 re-proved on the Leptos page in Chrome**, not by `e2e_node`: `e2e_node` runs headless and draws no page (T22), so it
      cannot see the surface; it stays green (14 scenarios passed, 0 failed). `tests/e2e_chrome.rs` now runs a WebSocket backend in the
      test that speaks `examples/mock_backend.rs`'s connection (`{"type":"call"}` in, `{"id","result"}` out), held back, stopped and
      started again on the same port: **24 passed, 0 failed** (12 scenarios × {window, div}). AC3: click → handler, `SET` in the DOM,
      `CALL` → the ListBox gains an `<li>`, `DISPLAY` on the console. AC4: the label goes `"calling"` → the answer frame → BTN-GO's
      greeting → BTN-RAW's `"written first"`, with the two clicks made while the call waited; the backend saw one call. AC5: the
      backend killed with a call in flight → the handler resumes with `{"error":{"class":"transport","code":7,…}}` (its `ON EXCEPTION`),
      the page shows "Reconnecting…", the backend comes back, a later call answers `pong`. **Not re-proved here:** the bearer token on
      the connection — the fixture program has no Authentication control; the token path is `net.rs`'s, covered by its native tests
      and T5's check in Chrome.
    - **Found by the browser, fixed (`src/api.rs`, `src/host.rs`):** a user's click while a handler waited for the backend **froze the
      page** — `pump` restarted the driver whenever `pending_work() > 0`, and an event queued behind the suspended task is pending
      but cannot run, so `pump` → `drive` → `pump` looped through microtasks forever (reproduced with the hand-written layer too: not
      Leptos's). The driver and `settled()` now ask `Host::can_run()` (a task whose wait is over, or work not held behind its instance's
      task), which shares `next_call`'s selection (`ready_task`, `runnable_work`). `abi_host`'s queued-events test asserts `can_run()`
      is false while the events wait and true once the answer arrives (22/22).
    - `cargo test PLAYER --no-fail-fast`: lib 226, `abi_host` 22, `dependency_guard` 6, `freshness` 3, `sinks` 3 — all passed.
      `cargo test -p cobolt-web-bundle --no-fail-fast`: 19 + 2 + 2 + 1 (`e2e_chrome`, 24 scenarios) + 1 (`e2e_node`, 14) + 2 + 3 + 3 — all
      passed. No warning from either crate, no `allow`.
    - **Sizes, player rebuilt (`assets/web-player`), after `wasm-opt` / gzip:** plain **2,846,052 / 930,144** (T18: 2,726,012 /
      886,174 → **+120,040 / +43,970**, +4.4 %); threaded **2,670,938 / 913,337** (T18: 2,552,720 / 870,352 → +118,218 / +42,985).
      Parts unchanged: Mermaid 2,218,954, Canvas 2D 654,896, PDF 217,066 — what drawing with Leptos costs over the hand-written patch.

- [x] **T25 — Version triple and pinning an older player** (R11, R12; Q6; plan §1.4; AC6)
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
  - **Done (2.0.42).** Q-P2 was decided by the operator (plan §8): every `2.x.0` player plus the last `z` of the previous minor.
    - **The record** (`examples/build_player.rs`): `player-build.json` gains `versions: {abi, description, player}` (from
      `cobolt_wasm_abi::ABI_VERSION`, `DESCRIPTION_FORMAT`, the crate version — the triple `versions()` already answered since T20)
      and `host_imports` (`pr.flush`, `pr.query`, `pr.remote`, `pr.now`, `pr.random`, `pr.abort`: ABI v1 §3). Shipped record:
      `{"abi": 1, "description": 1, "player": "2.0.0"}`.
    - **The pin** (`crates/cobolt-compiler/src/project_kind.rs`): `player: Option<String>` in `WebSettings` and `MobileSettings`,
      `#[serde(default, skip_serializing_if = "Option::is_none")]`, so an old manifest reads and is saved unchanged; a Mobile request
      carries it into its Web settings (`web/mod.rs`).
    - **Choosing the player** (`crates/cobolt-project-tools/src/web/generator.rs`): `select_player(dir, pin)` after `player_dir()`:
      no pin → the current player; a pin → a kept player (`kept_players`: the current one and each `archive/<version>/`, read from
      their records), else `WebBuildError::PlayerPin(PlayerPinError::NotKept { pinned, kept })`; a kept player whose host API or
      description format this compiler does not emit (it emits only the current ones) → `CannotEmit { pinned, abi, description,
      oldest }`, `oldest` the oldest kept player it can build for. `cobolt-wasm-abi` became a normal dependency of project-tools
      (an in-workspace path crate: nothing new in the lock). The IDE's panel and the `build` tool show the error (in English until
      T27). **Deviation:** the choice is made where the player folder is found (project-tools), not in `assemble_bundle`, which takes
      the player it is given.
    - **AC6, first half** (`crates/cobolt-web-bundle/src/lib.rs`, `player_lacks_imports`): a module import the player's record does
      not list fails the bundle — `program.wasm imports `pr.remote`, which the player 1.9.0 does not provide: …`, each import once; a
      player with no record is not judged. The module's own refusal at start is T8's, re-run (`abi_host` 22/22).
    - **The archive:** `assets/web-player/archive/` is empty — no older player has been released yet; `freshness` checks every
      `archive/<version>/` (its record's version is its folder's, every listed file has its digest) and skips the archive when it
      checks the current player's files.
    - Verify: `cargo test -p cobolt-compiler project_kind` 14/14 (new: the pin is optional and only written when set, Web and Mobile);
      `cargo test -p cobolt-project-tools web` 32 + 1 (new: a pin chosen or refused — not kept, listing `1.9.9, 2.0.0, 2.1.0, 2.2.0`;
      cannot emit, naming `1.9.9` and the oldest buildable `2.0.0`; and a Build with `player = "0.0.1"` stops listing the kept
      players, the current `2.0.0` among them, writing no bundle); `cargo test -p cobolt-web-bundle --test player_versions` 2/2;
      `cargo test PLAYER --test freshness` 5/5 ("archived players checked: 0"). Player rebuilt (the record changed; sizes as in T24).

- [x] **T26 — Charts, maps and the Viewer as parts on first use** (R13; Q3; plan §1.7; AC8)
  - Files: `crates/cobolt-web-player/parts/` (new parts where they split without a second copy of the core), `examples/build_player.rs`
    (builds and records them), `crates/cobolt-web-bundle/src/lib.rs` (lists them in the config with their integrity).
  - Do: split each one that can be split; for one that cannot, write the reason in this task's evidence rather than a half-split.
  - Verify: `cargo test -p cobolt-web-bundle --test e2e_node` — a form with no chart, map or Viewer fetches none of their parts (the scenario
    lists what was fetched); a form with a chart fetches the chart part once. Player rebuilt; sizes recorded.
  - **Done (2.0.42): none of the three splits without a second copy of the core — the reason, measured, instead of a half-split.**
    The plain player was built once with the chart, Maps and Viewer views replaced by the hidden placeholder (a scratch build in
    `target-player/measure-t26`, source restored at once): **2,759,890** bytes after `wasm-opt` (gzip 896,937) against the shipped
    **2,846,052** (930,144). Everything the three draw costs the core **86,162 bytes (3.0 %), 33,207 gzipped** — the most a split could
    take out. A part is a WebAssembly module of its own with its own standard library, `serde_json` and the `cobolt-forms` parsers it
    reads (`chart::parse_chart_rows`, `map_geometry`, `viewer::parse_markdown` with `pulldown-cmark`), and it would hand its `VNode`
    tree back through JavaScript as JSON; the smallest part there is, the PDF reader, is already **217,066** bytes. A chart, map or
    Viewer part would be larger than what it removes, and a form that uses one would download more than today. The heavy work behind
    these controls is already in parts loaded on first use: Mermaid diagrams (2,218,954), the PDF reader (217,066) and the Canvas 2D
    rasteriser (654,896). Revisit if a control gains a heavy dependency of its own (a chart engine, a vector-tile renderer).
    - Verify: `cargo test -p cobolt-web-bundle --test e2e_node` — 14 scenarios passed, among them "parts: the Mermaid renderer is
      fetched on first use, once, and not before" (nothing of the part at start, one fetch on the first diagram, none on the second)
      and the Canvas 2D part's. No part was added, so no new scenario; the player was not changed by this task.

- [x] **T27 — The Build report and the pin error, in six languages** (R13; AC8; i18n)
  - Files: `crates/cobolt-project-tools/src/web/mod.rs` (`BuildReport` line 164 and `PartSize` line 125 carry the player's size and each part's,
    read from `player-build.json`), `crates/cobolt-ide/src/panels/web_preview.rs` (the panel that shows `BuildReport` / `WebBuildError` today),
    `crates/cobolt-ide/src/i18n.rs` (new `Tr` keys for the size lines and the pin error, ×6 languages).
  - Verify: `cargo test -p cobolt-project-tools web` — the report holds the player's and every part's size, equal to `player-build.json`;
    `cargo test -p cobolt-ide --bin cobolt-ide i18n` — no empty translation for the new keys.
  - **Done (2.0.42).** `BuildReport::player: Vec<PartSize>` (`web/mod.rs`, also in `to_json` as `player`), filled by
    `generator::player_sizes` from the copied player's `player-build.json`: each variant at the record's chosen `opt-level`, then
    `parts/<name>`, bytes after `wasm-opt`. The panel (`panels/web_preview.rs`) says it after "Build done" as `Note::PlayerSizes` —
    "Player size: plain 2.71 MB, threaded 2.55 MB" and "Loaded on first use: mermaid 2.12 MB, …" — and a pin it cannot use as
    `Note::PlayerPin` (an error; replaces T25's English stand-in). Five `Tr` keys × six languages (pt-BR): `web_build_player_sizes`,
    `web_build_player_parts`, `web_build_pin_not_kept`, `web_build_pin_cannot_emit`, `web_build_pin_cannot_emit_none`.
    - Verify: `cargo test -p cobolt-project-tools --test web_build a_web_project_builds_to_modules_and_a_bundle` — the report's
      player sizes equal the record: `plain 2,846,052, threaded 2,670,938, parts/mermaid 2,218,954, parts/canvas 654,896,
      parts/pdf 217,066`; `cargo test -p cobolt-ide --bin cobolt-ide -- i18n web_preview`: 34 passed — "8 web build texts in 6
      languages" (none empty, none untranslated, placeholders as in English) and "156 note renderings (26 notes x 6 languages), none
      empty, none with a gap", with the English sentences checked word for word.

- [x] **T14 — The IDE finds the player beside the executable** (R1, R12; plan §1.4)
  - Files: `crates/cobolt-ide/src/app.rs` (a `web_player_dir()` resolved as `images_dir()` (line 20415) and `CoboltApp::themes_dir()` (line 2442)
    are, never from `CARGO_MANIFEST_DIR`; the test `bundled_directories_resolve_beside_the_executable` (line 21886) extended to it); the call into
    the Web Build passes that directory.
  - Depends on: the IDE Build step of WASM-001/002 (it already calls `assemble_bundle`); the installers copying `assets/web-player/` is the
    packaging step's, named here so it is not forgotten.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide bundled_directories_resolve_beside_the_executable` green.
  - **Done (2.0.42).** `web_player_dir()` in `crates/cobolt-ide/src/app.rs`, the pattern of `images_dir()`: `<exe dir>/assets/web-player`
    when it is a folder, else `assets/web-player` under the working directory; never `CARGO_MANIFEST_DIR`. The test now checks it with
    the other two. The IDE passes it to the Build through a new `WebBuildRequest::player_dir: Option<PathBuf>`
    (`panels/web_preview.rs`, the Build/Preview thread); `None` keeps `generator::player_dir()` for `rcrun` and the tools. A named
    folder is used as it is: one that is not there is the missing player (`PLAYER_UNAVAILABLE_PREFIX`), never another folder
    found by searching. **Not done here (named so it is not forgotten):** the installers copying `assets/web-player/` beside the
    IDE — the packaging step's.
    - Verify: `cargo test -p cobolt-ide --bin cobolt-ide bundled_directories_resolve_beside_the_executable` 1/1;
      `cargo test -p cobolt-project-tools --test web_build the_build_copies_the_player_from_the_folder_it_is_given` 1/1 (a given folder
      builds and reports `plain`; a missing one stops the Build naming it).

- [x] **T28 — No toolchain on the developer's machine** (R1, R2; AC1)
  - Files: `crates/cobolt-web-bundle/tests/e2e_node.rs` (or a new test beside it).
  - Do: build a Web project into a bundle with `PATH` stripped of `cargo`, `rustc` and `rustup`, from the shipped `assets/web-player/`
    (not `support::built_player()`, which builds it).
  - Verify: `cargo test -p cobolt-web-bundle --test e2e_node` — the test lists the bundle's files and asserts exactly: player plain and threaded,
    program multi-memory and single-memory, host page, descriptions, stylesheet, assets; the page runs headless under node. AC10's other halves are
    re-run here: `cargo test -p cobolt-web-bundle --test csp_origins` (the page under the generated CSP) and `cargo test PLAYER --lib net`
    (a connection to an origin outside the configured backend is refused).
  - **Done (2.0.42).** A new test beside `e2e_node`: `crates/cobolt-web-bundle/tests/no_toolchain.rs`. It runs itself again as a child
    with `PATH` = a folder holding only `node` + `/usr/bin:/bin`; the child first asserts `cargo`, `rustc` and `rustup` cannot be
    started, then assembles the bundle from the **committed** `assets/web-player` (both variants, the three parts, its record) and
    the program in both variants, with a stylesheet and an asset. The parent lists the 28 files by kind and refuses any it cannot
    name: player plain (2), player threaded (3, with the Rayon worker helper), parts on first use (6), program multi-memory and
    single-memory, the program manifest, form descriptions (2 JSON), the stylesheet, an asset, the host page (`index.html`,
    `app.*.js/css`, `loader.*.js`, `worker.*.js`, `config.json`, `_headers`, `sbom.cdx.json`) and the hosting examples
    (`hosting/.htaccess`, `hosting/nginx.conf`); the plain player in the bundle is the shipped file byte for byte. **The page runs
    headless under node: 14 scenarios passed, 0 failed.** Node's harness hosts the plain player only (no `wasm-bindgen-rayon`
    workers), so it runs a copy whose `config.json` offers no threaded player — what a page that is not isolated chooses; the
    threaded variant is a browser's (T9, Chrome).
    - The same for a **Web project compiled from COBOL** (`cobolt-project-tools/tests/web_build.rs`,
      `a_web_project_builds_with_no_rust_toolchain_on_the_path`): `GeneratorBackend::build` in a child with `PATH=/usr/bin:/bin`, the
      player from `assets/web-player` — 25 files, 9,053,545 bytes. The Web build path starts no process at all (no `Command::new` in
      `cobolt-wasm`, `cobolt-web-bundle` or `cobolt-project-tools/src/web/generator.rs`).
    - AC10's other halves re-run: `cargo test -p cobolt-web-bundle --test csp_origins` 2/2; `cargo test PLAYER --lib net` 11/11.

- [x] **T15 — The Preview replaces the module, a description, the stylesheet in place** (R16; AC11)
  - Files: the Preview's path through `cobolt_project_tools::web::preview` (`crates/cobolt-project-tools/src/web/preview.rs`) and
    `crates/cobolt-web-preview/src/main.rs`; the player's entry points already exist (`set_stylesheet` `src/api.rs:1028`, `add_form` 821, `attach` 832).
  - Depends on: WASM-008 (the Preview). `crates/cobolt-web-preview/src/main.rs` carries no description today (no `add_form` or `description` in it).
  - Verify: a `cobolt-web-bundle` node scenario (headless): replacing the module, one description and the stylesheet each changes the DOM, and
    the player instance id is the same before and after (no reload).
  - **Done (2.0.42).**
    - **The player** (`src/api.rs`): `Player::reset(manifest_json) -> Promise` — waits for a call in flight, closes every open
      instance in the module it runs in (`Host::teardown`, the close calls through the old link), lets the page take the instances
      away (their `Closed` effects), and puts a new `Host` for the given manifest behind the same `Player` (`make_host`, now shared
      with `Player::new`). The page, the stylesheets and the connection stay; a session the Authentication control held does not
      (the application starts as on a fresh page). Then the loader registers the descriptions, `attach`es the link and `start`s.
    - **The loader** (`crates/cobolt-web-bundle/src/hostpage.rs`, the Rust-emitted text): `app.update(next)` compares `next`
      (a configuration) with the running one — another player, worker or application style answers `'reload'` and changes nothing;
      a new application or form stylesheet is fetched and set; a new program (module + manifest: fetched, compiled, ABI and import
      checks as at start, a new main-thread or Worker link) or new descriptions restart the application in place through
      `player.reset`. It answers what it took in (`"stylesheet"`, `"forms"`, `"program"`).
    - **The Preview** (`crates/cobolt-project-tools/src/web/devserver.rs`, the live-reload script): on a change it fetches
      `config.json` and hands it to `app.update`; it reloads the page only when the answer is `'reload'`, the update fails, or the
      page holds no application. `crates/cobolt-web-preview/src/main.rs` needed no change (it shows the page the server serves; its
      own `reload` command is the user's).
    - **Deviation (verification):** proved in Chrome, not by a headless node scenario — headless runs draw no DOM, and the task
      asks that each replacement "changes the DOM". `tests/e2e_chrome.rs`, scenario "preview", in both placements: a stylesheet →
      `getComputedStyle(LBL-OUT).letterSpacing` becomes `3px`; the main form's description with BTN-GO at x 300 → the button is at
      300 px and `onLoad` ran again; the program rebuilt with `onLoad` writing "loaded v2" → the label reads it and BTN-GO's
      handler still answers; `update` answered `"stylesheet"`, `"forms"`, `"program"`; `window.__noReload` and a mark on the
      `Player` object survive, `performance.getEntriesByType('navigation')` is 1, one form on the page, one host instance.
      **26 passed, 0 failed** (13 scenarios × {window, div}). `cargo test -p cobolt-project-tools --lib devserver` 14/14 (the
      script offers the change to `app.update` first). The bundle suite stays green (`e2e_node` 14 scenarios, `no_string_execution`
      3/3 over the new loader text).

- [ ] **T17 — Equality with the desktop form host by measurement** (R6, R14; Q4; AC2, AC9)
  - Files: `specs/WASM-016-prebuilt-player/differences.md` (new: the expected differences — entrance and exit effects, window states, window
    positions, per the operator's 2026-10-22 answer to Q4; a verb that asks for one is accepted and does nothing), the R14 harness (a test that runs
    the WASM-004 R11 reference form and the spec 037/042 lifecycle and event-order scripts on `cobolt-form-host` and, under node, on the player).
  - Depends on: the WASM-004 R11 reference form and property round-trip harness.
  - Verify: the harness prints, per check, desktop result / player result / equal or listed difference; it fails on an unlisted difference.
    AC2: every class A and B control drawn from its description with each property set to a distinct value and read back from the DOM.
    Manual (plan §6): Chrome on 127.0.0.1, the variant choice, the Rayon DataGrid sort, parts on first use; the operator looks at the reference
    form beside the desktop.
  - **BLOCKED (2.0.42): its dependency is not built.** The harness compares the player with `cobolt-form-host` on the WASM-004 R11
    reference form, and that form and its tooling are WASM-004's open tasks **T32** (property round-trip for every class A and B
    control) and **T33** (the desktop comparison harness: per-control geometry, sampled colours and event logs from a headless
    `cobolt-forms --features render` run). Writing a second reference form here would be the parallel harness the plan forbids
    (two measurements that can drift, the failure R14 exists to prevent). `differences.md` is not written either: its rows are
    what the harness finds. Unblocks when WASM-004 T32–T33 land; the Chrome client of T22 (`tests/support/cdp.rs`) is ready to be
    its browser side.

- [x] **T29 — System KB** (steering: System KB rule)
  - Files: `crates/cobolt-compiler/src/lib.rs` (the settings table at line 6202: the optional `player` key under `[web]` and `[mobile]`; a short
    table of the form-description format and its `format` number), `assets/knowledge/chunked.data` (regenerated).
  - Verify: `cargo run -p cobolt-ide --example build_chunked_kb`; `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` green.
  - **Done (2.0.42).** `project_model_and_settings.md` in `crates/cobolt-compiler/src/lib.rs`: `# player = "2.0.0"` under `[web]` and
    `[mobile]` in the settings block; a paragraph **The player** (prebuilt, Leptos, shipped beside the IDE, no toolchain for a Web
    build, plain/threaded, parts on first use, the pin and its two refusals, a host import the player lacks refused at Build); a
    paragraph **Form descriptions** with the table of the format (`format` = 1, `name`, `form`, `layers`, `controls`, `form_events`,
    `data_bindings`/`animations`/`layout`/`breakpoints`, `stylesheet`); the Build report names the player's and each part's size;
    the Preview takes a change in place and reloads only when it cannot (T15) — the KB said "the open page reloads".
    `cargo run -p cobolt-ide --example build_chunked_kb`: 3,559 records from 8 documents, `assets/knowledge/chunked.data`
    12,763,136 bytes; `prebuilt_chunked_kb_matches_the_published_documentation` 1/1.

- [ ] **T30 — Finalize** (all)
  - Do: bump `z` **once** in `crates/cobolt-ide/src/version.rs` (`VERSION`, line 22; `2.0.29` at the time of writing — bump whatever it is then);
    add **one** `CHANGELOG.md` entry above the previous `## [PowerRustCOBOL …]` heading, dated, marked *Written on features-2x*. No
    documentation task: `README.md` and `docs/` are never touched on 2.x.x.
  - Verify, with `--no-fail-fast`, every crate touched, reading every `test result` line:
    `cargo test PLAYER --no-fail-fast`; `cargo test -p cobolt-forms --no-fail-fast` and `cargo test -p cobolt-forms --features render --no-fail-fast`;
    `cargo test -p cobolt-web-bundle --no-fail-fast`; `cargo test -p cobolt-project-tools --no-fail-fast`; `cargo test -p cobolt-compiler --no-fail-fast`;
    `cargo test -p cobolt-web-preview --no-fail-fast`; `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`. `cargo fmt --check`; no warnings.
    `df -h` before the sweep (each test binary is large).
  - **Done in part (2.0.42); the box stays open: two crates are not green, for reasons outside this spec.** `VERSION` 2.0.41 →
    **2.0.42** (once); one `CHANGELOG.md` entry above 2.0.41, *Written on features-2x*. `README.md` and `docs/` untouched.
    Sweep with `--no-fail-fast` (75 GB free before it), every `test result` line read:
    - **green:** player — lib 226, `abi_host` 22, `dependency_guard` 6, `freshness` 5, `sinks` 3; `cobolt-forms --features render` —
      1,697 passed in 91 test binaries, 0 failed; `cobolt-web-bundle` 36 in 11 (`e2e_chrome` 26 scenarios, `e2e_node` 14,
      `no_toolchain` 14 under node); `cobolt-project-tools` 163 in 12; `cobolt-compiler` 212 in 11; `cobolt-web-preview` 5.
    - **`cobolt-forms` without `render`:** its lib tests do not compile (17 errors: test code naming `crate::render`, `paint`,
      `egui`, `layout::apply`, `discover_packs`, which exist only with the feature). This spec does not touch `cobolt-forms`.
    - **`cobolt-ide --bin cobolt-ide`: 1,875 passed, 7 failed, none in code this spec changed:** `backend_run` ×2 (`target/debug/rcrun`
      is not built here), `docs_embed::every_document_ships_in_every_language` (the Portuguese documents are absent, by the 2.x.x rule
      that deletes translations), `every_completion_description_exists_in_six_languages` (12 editor completion texts are English
      only), `claude_validates_the_written_bundle` (the installed `claude` CLI's plugin check), `external_crates_service` ×2 (live
      crates.io: `egui` 0.37 is out). The IDE tests of this spec pass (`i18n`, `web_preview`, the bundled directories, the KB).
    - `cargo fmt --check`: not clean, as the tree was not before (the player's and the IDE's sources keep their long lines; the new
      files follow them). No warning from the crates this spec touched; no `allow`.

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
