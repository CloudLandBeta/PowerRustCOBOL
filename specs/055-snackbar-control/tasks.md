# Tasks — Snackbar control

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-09-01

Ordered so the tree stays green after every task. The sequence is deliberate:
**T1–T7 land a Snackbar that shows and expires**; stacking, overflow and the
full icon set follow. Each stage is independently useful and independently
verifiable — plan §5 flags scope as the main risk, and this is the mitigation.

> **Branch:** `features`, its own commit, never sharing with a fix (GOLDEN
> RULE #5). Do not commit or push unless the operator asks.

---

## Stage A — the control exists

- [x] **T1 — Add `ControlType::Snackbar`** (R1)
  - Files: `crates/cobolt-forms/src/model.rs`
  - Do: the variant, `as_str()`/`from_str()` round-trip, `ControlType::ALL`.
        **Add it last** in the enum — appended, never inserted.
  - Verify: `cargo build --workspace` fails ONLY with non-exhaustive-match
        errors; **write that list down — it is the checklist for T2.**
        `cargo test -p cobolt-forms` shows `every_control_type_has_an_icon`
        failing (expected until T3).

- [x] **T2 — Satisfy every match the compiler named** (R1)
  - Files: whatever T1's build listed — expect `render.rs`, `anim.rs`,
        `host.rs`, `toolbox.rs`, `properties.rs`, `designer.rs`,
        `codegen/src/lib.rs`, `compiler/src/lib.rs`, `examples/list_controls.rs`
  - Do: the minimum arm each site needs. No behaviour yet — a Snackbar that
        compiles and draws nothing.
  - Verify: `cargo build --workspace --all-targets` clean.

- [x] **T3 — The `control-snackbar` icon** (R2, AC1)
  - Files: `crates/cobolt-forms/src/icons.rs`
  - Do: one icon, existing treatment — 24-unit grid, single 1.5-unit stroke.
  - Verify: `cargo test -p cobolt-forms --features render every_control_type_has_an_icon`
        green. **AC1 covered.**

- [x] **T4 — Non-visual, and seeded properties** (R1, R3, AC2)
  - Files: `crates/cobolt-forms/src/model.rs`
  - Do: add to `is_non_visual()`; seed all 35 properties from spec §6 with the
        documented defaults. Colours/icon/timeout default **empty/0 = "the
        category decides"**, never a concrete value.
  - Verify: `cargo test -p cobolt-forms` — new test asserts the property set and
        that every default is the documented one, **printing the table**.
        Round-trip a Snackbar through `save_form`/`load_form` and confirm
        `ControlType` is keyed by NAME (plan §5 risk — verify, do not assume).
        **AC2 covered** (tray placement is T13's visual check).

## Stage B — one notification, shown and expiring

- [x] **T5 — `snackbar.rs`: the pure parts** (R18, R19, R22, AC8, AC14)
  - Files: `crates/cobolt-forms/src/snackbar.rs` (new), `src/lib.rs`
  - Do: size classes → dimensions; `Category` → defaults; `Buttons` parsing;
        content layout (icon → gap → text → flex → buttons) with vertical
        centring and the per-class line budget + ellipsis. **No clock, no state.**
  - Verify: `cargo test -p cobolt-forms` — dimensions table printed; vertical
        centring asserted for all three classes (**AC8**); over-long text
        ellipsized (**AC14**); `Buttons` parses with omitted trailing fields;
        a 4th button yields a diagnostic, not a silent drop (spec Q5).

- [x] **T6 — `paint::draw_snackbar`** (R20, R21, R23, R24)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: paint order background colour → background image → content. Reuse
        `draw_glass_auto*`, `paint_background_gradient`, **`face_color`** (the
        1.62.140 fix — the face owes its own transparency) and `readable_ink_on`.
  - Verify: `cargo test -p cobolt-forms --features render` — a test asserting
        paint order by shape index, and that the background image never moves
        content (**R21**). Category defaults applied when unset, overridden when
        set (**AC10**).

- [x] **T7 — `SnackbarStack`: raise and expire** (R4, R5, R6, R7)
  - Files: `crates/cobolt-form-host/src/snackbar_stack.rs` (new), `src/lib.rs`,
        `src/host.rs` (own one per `FormBody`, tick + draw per frame)
  - Do: `raise()`, `tick(now: Instant, pointer: Option<Pos2>)`, timeout,
        `Timeout = 0` never expires, hover pause/resume.
        **The clock is a parameter — nothing sleeps.**
  - Verify: `cargo test -p cobolt-form-host` — driven by a fabricated clock:
        expiry (**AC6**), `Timeout = 0` stays, hover holds and leaving resumes,
        each **reporting the remaining ms** it measured.

## Stage C — COBOL reaches it

- [x] **T8 — `Show()` / `DismissAll()`** (R4, R9, AC3)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (method table ~10949 and
        the known-method list ~13730), `src/form_host.rs` (the request)
  - Do: dispatch both; `Show()` mints from the control's CURRENT property values.
  - Verify: `cargo test -p cobolt-runtime` — `Show()` twice yields two live
        notifications (**AC3**); `DismissAll()` clears with reason
        `Programmatic`. **Every property read defaults — no `unwrap()`** (plan §5).

- [x] **T9 — Events** (R8, AC7)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`,
        `crates/cobolt-forms/src/model.rs` (`SUPPORTED_EVENTS`)
  - Do: `onShown`, `onClosing`, `onClosed`, `onTimeout`, `onButtonClick`
        (id + index); `DismissOnClick` dismisses with reason `Action`.
  - Verify: `cargo test -p cobolt-runtime` — asserts **order**:
        `onButtonClick` then `onClosed` (**AC7**); `DismissOnClick = false`
        leaves it up; all five reasons reported verbatim.

## Stage D — the stack

- [x] **T10 — Stacking and growth direction** (R11, R12, R13, AC4)
  - Files: `crates/cobolt-forms/src/snackbar.rs`
  - Do: `stack_layout()` — vertical only, `StackSpacing` apart; Top grows down,
        Bottom grows up, newest nearest the anchor; Centre defaults
        newest-first, `StackOrder` overrides.
  - Verify: `cargo test -p cobolt-forms` — for each of the nine anchors, print
        the y-order produced and assert the direction (**AC4**).

- [x] **T11 — Reflow and overflow** (R14, R15, AC5, AC11)
  - Files: `crates/cobolt-form-host/src/snackbar_stack.rs`
  - Do: recompute on dismiss; `MaximumVisible` + `OverflowBehavior`
        (Queue / DiscardOldest / DiscardNewest).
  - Verify: `cargo test -p cobolt-form-host` — dismissing the middle of three
        closes the gap, printing before/after rects (**AC5**); each overflow
        policy reports **which ids survived and what was dropped** — no silent
        truncation (**AC11**).

- [x] **T12 — Anchored to the form's SURFACE** (R16, R17, R26, AC9, AC13)
  - Files: `crates/cobolt-form-host/src/host.rs`
  - Do: resolve `Anchor` + `Margin` against the occupant's pane rect for an
        Embedded form, the window for a standalone one (D3).
  - Verify: `cargo test -p cobolt-form-host` — headless host over an Embedded
        form asserts the notification rect lies inside the ContentPane and does
        not overlap rail or breadcrumb (**AC9**); the surface size is unchanged
        by a raise (**AC13**).

## Stage E — surfaces, icons, docs

- [x] **T13 — IDE: toolbox, tray, property editors** (R1, R3, AC2)
  - Files: `crates/cobolt-ide/src/panels/toolbox.rs`, `properties.rs`,
        `designer.rs`
  - Do: toolbox entry; non-visual tray rendering; grouped editors, with a
        line-per-button editor for `Buttons` rather than a raw text box.
  - Verify: `cargo test -p cobolt-ide --bins`; **manual:** drop a Snackbar —
        it appears in the tray and paints nothing on the canvas (**AC2**).

- [x] **T14 — Category + button icons** (R24)
  - Files: `crates/cobolt-forms/src/icons.rs`
  - Do: 5 category icons + ~11 button icons in the **existing treatment** —
        24-unit grid, single 1.5-unit stroke, fills as accents (operator,
        2026-09-01, spec Q4). The hand-drawn quality comes from the PATH SHAPES
        — an octagon deliberately not regular, a circle that does not quite
        close — never from a second stroke style.
  - Verify: `cargo test -p cobolt-forms --features render` — names unique, all
        drawable, every shape on the 24-unit grid (the existing icon suite
        already asserts all three).

- [x] **T15 — Codegen** (R1)
  - Files: `crates/cobolt-codegen/src/lib.rs`
  - Do: emit the declaration and `onButtonClick` handler stubs; banner intact.
  - Verify: `cargo test -p cobolt-codegen`; generated `.cbl` parses under
        `cobolt-parser` and checks clean under `cobolt-semantic`.

- [x] **T16 — Engine parity** (R25, AC12)
  - Files: `crates/cobolt-forms/tests/`
  - Do: a parity test in the shape of
        `engine_reference_form_parity_static_vs_faces`.
  - Verify: `cargo test -p cobolt-forms --features render` — the same shapes and
        fills on both paths (**AC12**). *Both* paths — this is the split that
        cost a day on the caption shadow.

- [x] **T17 — System KB** (steering: hard constraint)
  - Files: `crates/cobolt-compiler/src/lib.rs`, `assets/knowledge/chunked.data`
  - Do: property/method/event doc tables for the Snackbar, then
        `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify: `chunked.data` **actually changed** (an unchanged file means the
        wrong source was edited); `cargo test -p cobolt-ide --bins
        prebuilt_chunked_kb` green.

- [x] **T18 — Docs & i18n**
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`
  - Do: a Snackbar section for a PowerCOBOL/isCOBOL reader — COBOL examples
        only, Notes + ⚠️ Caveats, no host-language explanations. Every new IDE
        string a `Tr` field in **all six** languages; COBOL property/method/event
        names and `Buttons` field values stay **English in every language**.
        Translations of the guide untouched.
  - Verify: `cargo test -p cobolt-ide i18n`; guide passes
        `iconv -f UTF-8 -t UTF-8` with zero double-encoded bytes.

- [ ] **T19 — Finalize**
  - Do: bump `z` in `version.rs` + `CHANGELOG.md` entry.
  - Verify: `cargo test -p cobolt-forms --features render --no-fail-fast`,
        `-p cobolt-form-host`, `-p cobolt-runtime`, `-p cobolt-ide --bins`,
        `-p cobolt-codegen`, `-p cobolt-compiler` — **read every
        `test result:` line**, never grep for failures.
        NIST is **not** required: nothing here touches the interpreter's
        language behaviour. If T8/T9 grow beyond method dispatch, it becomes
        required — both axes.
        **Manual** (operator, not agent — the app is never driven from here):
        raise three notifications; confirm stacking, growth direction per
        anchor, reflow on dismiss, hover-pause, and that an Embedded form's
        notifications land inside its ContentPane.

## Done criteria

All 14 acceptance criteria in `spec.md` are checked, every suite green, docs and
KB updated, and the change sits in its own **feature** commit on `features`
(do **not** commit or push unless the operator asks).

**Coverage map** — AC1 T3 · AC2 T4/T13 · AC3 T8 · AC4 T10 · AC5 T11 · AC6 T7 ·
AC7 T9 · AC8 T5 · AC9 T12 · AC10 T6 · AC11 T11 · AC12 T16 · AC13 T12 · AC14 T5.
