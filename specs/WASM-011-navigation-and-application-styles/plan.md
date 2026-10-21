<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Navigation and application styles (WASM-011)

- **Status:** routes, route parameters, the SPA shell, standalone windows, `super::` across windows and modal dialogs are implemented in the player and run in Chrome 152; **not-found / sign-in outcomes (R6), focus and announcement on navigation (R7), unsaved-changes guard (R8), lifecycle events of a closed window (R12 in part) and the mobile stack (R13) are not done**; the project setting and the Build check (R1, R3) are the IDE side
- **Spec:** ./spec.md   **Date:** 2026-10-20

## 1. Approach

The opening verbs (`OpenFormSync`, `OpenFormAsync`, `Close`) are the host's (`host.rs::form_verb`); what a verb does depends on `HostOptions.standalone`, which the bundle sets from the application style (`config.style`: `spa` or `standalone`).

- **R2, SPA.** An open is a **form instance inside the page**: `OpenMode::Embedded` (the ContentPane of the shell), `Panel` (a window-like panel), `Modal` (a `<dialog>`). `OpenFormSync` suspends the caller (ABI amendment A5: tasks of different instances interleave) until the child closes.
- **R2, standalone.** The same verbs emit `Effect::OpenWindow` and the page opens a **tab or window of its own** (`ops.rs::open_window`): `?pr-form=Name&pr-opener=H1&pr-param-<name>=<value>...&pr-super=<token>`. Each window runs its own instance of the program (R9): its data is its own, as on the desktop.
- **R4, R5 — routes.** `view::shell::Route` is `#/form-name?key=value&key2=value2` (a hash works on any static host and never touches the host page's path, Q2; `to_hash` / `parse`, percent-encoded). The shell **pushes** a route when it shows a form (`sync_address`, no push when the address already means the same, the root form has no route), and **follows** the address on Back / Forward / a bookmark (`follow_address`, `watch_address`): the player's own `pushState` does not fire `popstate`, so there are no loops. A route whose parameters differ from the shown instance's re-opens it. An empty hash or `#` closes everything above the main form. A hash that is not a route (`#section`) is the page's own and none of ours (R5).
- **Route parameters** (R4, Q1: *the form and its opening parameters, not its in-memory data*): `OpenFormSync(form, "name=value", ...)` and `OpenFormAsync` take `name=value` arguments after the form's name; the opened form reads them back with `GetParam(name)` (empty when absent) and `GetParams()` (`name=value` per line), **already in `onLoad`** (the tree exists before the handler runs). They go in the address in the SPA and in `pr-param-*` in a standalone window; what comes from an address is the user's, so a bad one (a name that is not `[A-Za-z0-9._-]`, a duplicate, an over-long value) is left out, never trusted, and there are at most 16 of at most 64 / 512 characters. A refused one in a *program's* open refuses the open with the reason. (A7 in `WASM-ABI-v1.md`.)
- **R10 — popups.** A standalone open from a user's gesture opens the window; when the browser's popup blocker refuses it, the application shows a notice saying how to allow pop-ups and **releases the suspended caller with a failure** (`Inbox::ChildClosed`), never leaving it hanging. (Raising a developer-handled event for it is not done.)
- **R11 — `super::` across windows.** In a window another window opened, `super::Prop` is the form that opened it. The two windows talk over a **`BroadcastChannel` that only they share**: its name carries a 128-bit random token the opener generates and puts in the new window's address (`pr-super=`), so no other page of the origin can join it. The new window keeps a **replica** of the opener form's properties, because the program's calls are synchronous and a round trip to another window is not: a read is answered from the replica; a write updates the replica at once and is forwarded; a `CALL` is forwarded and answers nothing. The opener publishes its form's properties after every change and on `hello`, applies the child's `set` / `call` to its own form (with the same events a local child would raise), and announces `closed` when its window goes away (`pagehide`) or the instance closes; the child then fails `super::` with the refusal `ON EXCEPTION` handles. The loader waits at most 1.5 s for the first state so the first handler already reads it (`player.attach_opener` before `start`). Messages are JSON text (`superlink.rs` header has the table). **Differences from the spec's wording, stated plainly:** a read sees the opener's state as of its last message (one message of lag); a `CALL` is forwarded without waiting for its answer (so a value-returning call to the opener's method answers empty), the suspension the spec describes ("the caller is suspended meanwhile") is not implemented. A same-origin, same-browser-profile limit applies (Q3).
- **R14 — modal dialogs.** A modal is a `<dialog>` opened with `showModal()`: the browser makes what is behind inert, traps the focus, closes it on Escape (`cancel` is routed to the form's close), and stacks them in the order opened.

## 2. Affected crates / files

- `crates/cobolt-web-player/src/host.rs` (verbs, `Params`, `RemoteOpener`, `Effect::{OpenWindow, SuperSet, SuperCall}`), `src/host/{params,super}_tests.rs`, `src/superlink.rs`, `src/view/shell.rs` (`Route`, the SideMenu rail, breadcrumb), `src/dom/{ops,instance,mod}.rs` (address, windows, dialogs), `src/api.rs` (`attach_opener`, `PlayerConfig.start_params`).
- `crates/cobolt-web-bundle/assets/loader.js` (`pr-form`, `pr-opener`, `pr-param-*`, `pr-super`; `attach_opener` before `start`), `src/lib.rs` (`AppStyle`, `config.style`).
- `specs/WASM-ABI-v1.md` A7 (`OpenForm` parameters, `GetParam`, `GetParams`).
- Not here: the project setting and the New Project choice (R1), `FormFormat`'s build-time check (R3), the mobile shell (R13).

## 3. Data / model changes

No `.cfrm` change. The address grammar above is public once shipped (Q2); parameter limits are `host::{MAX_PARAMS, MAX_PARAM_NAME, MAX_PARAM_VALUE}`.

## 4. Key decisions & alternatives

- **Decision: a hash, not a path** (Q2). Works on any static host without server configuration (R5).
- **Decision: the replica model for `super::`** over a blocking round trip: the module cannot block on another window, and suspending the task (the way `OpenFormSync` does) needs a reply protocol with timeouts that this first version does not have. Rejected for now: a suspended `super::` call with a reply message.
- **Decision: a private channel per opener-and-child pair**, not one channel per application: a page of the same origin that is not the child cannot listen.
- **Decision: refuse, with the reason, never guess** for parameters from a program; **leave out** those from an address.

## 5. Risks & mitigations

- Risk: a stale replica read. → The opener publishes after every change; the child's own writes update the replica at once.
- Risk: the opener closed. → `closed` on `pagehide` and when the instance closes; the child's first state wait times out after 1.5 s and marks the opener gone.
- Risk: a hash that is not a route. → Left to the page.

## 6. Test strategy

- Native: `host/params_tests.rs` (the parameter grammar and limits, GetParam in `onLoad`, an address's bad parameters left out), `host/super_tests.rs` (6 tests: no opener, read from the replica with case-insensitive names, write seen at once and forwarded, call forwarded, refused when the opener is gone, the opener applies set / call and says when it cannot), the shell `Route` tests, `tests/abi_host.rs`.
- Chrome 152, standalone bundle, **two tabs** (the Browser pane navigates the same tab on `window.open`, so the opener's `window.open` was replaced by a recorder and the child opened in a second tab at the recorded address): the child's click set `super::Caption`, read it back and showed it (`set by the child`); the opener, asked over the channel, published `caption = set by the child`; after the opener's tab left, the child's next `super::` was refused (the fixture module has no `ON EXCEPTION`, so the refusal surfaced as a module trap in its banner).
- Chrome 152, shell bundle: a rail click pushed `#/child-form`, Back closed the child and left an empty hash, Forward re-opened it. (The fixture's Home rail item has no action, so it does nothing.)
- Not run: a popup blocker refusal, a bookmark / deep link of a standalone page in a fresh browser profile, the mobile stack.

## 7. Steering compliance

- [x] `README.md` and `docs/` untouched.
- [x] `WASM-ABI-v1.md` A7 updated for the new verbs.
- [ ] R1, R3 (IDE and Build), R6-R8, R12 lifecycle, R13: see tasks.md.
