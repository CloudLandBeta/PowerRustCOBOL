<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Hyperlink control (WASM-006)

- **Status:** the player half (rendering, the link policy, the click plan, the methods) is implemented, tested natively and run in Chrome 152; **Cancel and the before-navigate event (R6, R13), fragment scrolling (R10) and the mobile rule (R11) are not done**; the model type, toolbox, Designer face and Properties panel are the IDE side
- **Spec:** ./spec.md   **Date:** 2026-10-20

## 1. Approach

Hyperlink reaches the player as a `ControlType::Custom` named `Hyperlink` (the model does not carry the type yet), recognised by `kinds::player_kind`. Its rules are **pure functions in `crates/cobolt-web-player/src/weblink.rs`** — nothing there knows the DOM — so they are tested without a browser; the view and the interaction layer only ask `plan()` what to do.

- **Properties read** (the names this file's header lists, the contract the model must match): `Url`, `Target` (`Self`, `NewTab`, `NewWindow`, `Parent`, `Top`), `WindowFeatures`, `Rel`, `Download`, `HrefLang`, `MimeType`, `ReferrerPolicy`, `Underline`, `AccessibleLabel`, the colours `LinkColor`, `HoverColor`, `ActiveColor`, `VisitedColor`; `Cancel` is read as the flag a handler sets.
- **R2, R4, R9 — an anchor.** The control is an `<a>` showing its Caption; it is focusable, Enter activates it natively, the browser's context menu stays. The accessible name is the Caption unless `AccessibleLabel` overrides it.
- **R3 — every `<a>` capability but `ping`.** Operator ruling, 2026-10-20: `ping` is **not** offered, because it is the one attribute that makes the browser send a request to an address the page never fetched. The player never writes it (checked in Chrome).
- **R7 — what it accepts.** `weblink::check(url, allow_data, &Policy)`: `http`, `https`, `mailto`, `tel`, `sms`, an in-page fragment, a relative address, and the **extra schemes the project allows** (`Policy`, carried on every form tree). `javascript:` and the like are refused however spelled (the URL is normalised first, as the browser's parser does: leading and trailing C0 controls and spaces stripped, tabs and newlines removed anywhere), and the project's extra schemes cannot widen that list. `data:` is allowed only for a `Download` link.
- **R8 — `rel`.** For `NewTab` / `NewWindow` with `Rel` empty the plan writes `noopener noreferrer`; `ReferrerPolicy` is carried through as the property says.
- **R5 — click.** The click raises `onClick` (argument: the modifier text) and the **browser performs the navigation**: a plain click is not `preventDefault`ed, so middle-click, Ctrl/Cmd-click, "open in new tab" and the context menu keep their native meaning; Alt-click downloads, named from the path; a `NewWindow` link that carries `WindowFeatures` is the one case the player opens itself (`window.open` with the features and `noopener`).
- **R12 — methods.** `SetUrl` (refused with the reason when the address fails `check`, the control left unchanged), `GetUrl`, `Open`, `OpenIn(target)`; the last two are page commands that perform the navigation.

## 2. Affected crates / files

- `crates/cobolt-web-player/src/weblink.rs` (policy, `plan`, `method`), `src/view/controls.rs::weblink`, `src/view/interact.rs::weblink`, `src/dom/ops.rs` (`navigate`, `open window`), `src/methods.rs` (dispatch), `src/tree.rs` (`FormTree.policy`).
- `crates/cobolt-web-bundle` — the project's extra link schemes reach every check (the bundle's `config.json`).
- Not here: `ControlType`, `Control::new` defaults, the Designer face, the Properties panel, the toolbox icon (R1, R14, R15), `tests/controls/`.

## 3. Data / model changes

None in the player. The model's Hyperlink must carry the properties above under those names.

## 4. Key decisions & alternatives

- **Decision: do not `preventDefault` a plain click.** The browser's navigation is the most compatible one. **Consequence, stated plainly: a handler cannot cancel it** — R6 (Cancel) and R13 (the before-navigate event) are **not met**. Meeting them means preventing the default, running the handler, then navigating from script (`location` / `window.open`), which loses the modifier-key and context-menu conventions R5 asks to keep for the plain case. The spec's Q3 left the choice open; the clean design is: prevent the default only when a handler is bound to the before-navigate event, so a link nobody intercepts stays fully native.
- **Decision: one policy function** used by the view, the interaction layer and `SetUrl`.
- **Decision: refuse, never rewrite.** A refused address leaves the control inert and says why; nothing is silently corrected.

## 5. Risks & mitigations

- Risk: an address built from data (`SetUrl` from a field) is a script. → `check` runs on every write and every click; the tests cover the dangerous schemes however they are spelled, and that a project cannot widen them.
- Risk: `target=_blank` without `rel` hands the new page `window.opener`. → `noopener noreferrer` by default (R8).

## 6. Test strategy

- `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib weblink` — 7 tests: dangerous schemes refused; a project widens the list but not for them; a plain click follows the target and new contexts get `noopener noreferrer`; modifier keys follow the browser's conventions; download, window features and referrer policy are carried through; an empty address does nothing and a refused one is reported; `SetUrl` refuses `javascript:` and leaves the control unchanged.
- Chrome 152 (gallery bundle, 127.0.0.1): `<a href target=_blank rel="noopener noreferrer">`, no `ping`, `cursor: pointer`, `javascript:` refused by `SetUrl` with the reason, `mailto:` accepted, a plain click not default-prevented.
- Not run: the mobile shell, the Designer, the `.cfrm` round trip.

## 7. Steering compliance

- [x] No invented model property: the names are the spec's R3 list.
- [x] `README.md` and `docs/` untouched.
- [ ] Model, Designer, Properties, toolbox, `tests/controls/`, System KB, six-language labels: the IDE side.
