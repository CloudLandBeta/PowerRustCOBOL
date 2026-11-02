# WASM-016 — Differences between the player and the desktop form host (T17; R6, R14; Q4; AC9)

What the player does differently from the desktop form host, each with its reason and its status. The measurement is
`crates/cobolt-project-tools/tests/desktop_comparison_chrome.rs` (WASM-004 T33): one reference form, the desktop's boxes, colours and
events read from `cobolt_forms::render`, the page's read from Chrome. The test fails on a difference that is not in its `DOCUMENTED` list,
and this file is that list with the rulings behind it.

Written 2026-11-02 (2.0.160). The operator ruled on the twelve differences T33 found; the rulings are applied below.

## Not carried to the browser (Q4, operator 2026-10-22)

Entrance and exit effects, window states and window positions. A verb that asks for one is accepted and does nothing.

## Found by T33, with the ruling

| # | Control | Difference | Ruling (2026-11-02) | Status |
|---|---|---|---|---|
| 1 | CH-BAR | No chart face on the page; the desktop paints navy [15,20,45] | Fix the player | **Fixed** (2.0.160): an uncoloured chart takes the desktop's face in CSS; measured equal, max 0 levels |
| 2 | CH-PIE | No face; the first slice read another colour | Fix the player | **Fixed**: the face comes with row 1; the slices read in the middle of each of the three slices, max 16 levels (allowance 24). The old sample sat where the slices meet |
| 3 | KNB-B | Dial face and ink differed ([66,66,66]/[40,40,40] against [43,46,53] with a blue centre) | Fix the player | **Fixed**: the dial is drawn with the painter's proportions and colours; max 0 levels. The old sample sat on the one-pixel rim |
| 4 | BTN-A and every class A control | The page raises `onGotFocus` on a click, and the previous control's `onLostFocus`; headless `render_form` reports none | Accept; a measurement artefact | **Listed.** To be re-measured with the real host (the lifecycle harness, open) |
| 5 | TXT-A | A typed key raises `onChange` / `onTextChanged` after `onKeyPress` on the page, before `onKeyDown` on the desktop | Accept | **Listed** — the browser's order |
| 6 | CHK-A | No `onMouseDown` / `onMouseUp` on the page; the desktop's list repeats `TXT-A onLostFocus` | Fix the mouse events; accept the repeat | **Fixed** (the press and release on the label are the control's own); the repeat is **listed** (headless `render_form`) |
| 7 | RAD-A | No `onMouseDown` / `onMouseUp` on the page | Fix the player | **Fixed**; the row is gone |
| 8 | SWT-A | `onCheckedChanged` before `onClick` on the page, after on the desktop | Accept | **Listed**; the state is already changed in both |
| 9 | LST-A | The desktop selects on the press, the page on the click | Investigate | **Investigated, nothing duplicated**: each raises the selection change twice in the script (the click, then the Down arrow). The page selects on the click, after `onMouseUp`, as a browser does. **Listed** |
| 10 | CMB-A | A second `onMouseUp` after `onClick` (the `<select>` picker) | Accept | **Listed** |
| 11 | NUD-A | No `onKeyPress` for the Down arrow on the page (a non-character key) | Accept | **Listed** |
| 12 | SLD-A | The page's range input moves on the press and on the Down arrow; the desktop's does not | Investigate | **Open**: probably the headless click not landing on the slider's handle. To be re-measured with the real host |

Measured after the fixes: 14 controls, **8 differences**, all listed: rows 5, 6 (the repeat), 8, 9, 10, 11 and 12 once each, and row 4 on the
class A controls it touches (the test lists it per control). Geometry equal to the pixel on all 14 controls; colours max 0 (bar chart, knob) and 16
(pie) levels.

## The lifecycle (T17, second half; measured 2026-11-02, 2.0.163)

`crates/cobolt-project-tools/tests/lifecycle_comparison_chrome.rs`: one form whose form-level handlers DISPLAY every lifecycle event, run on the real host
(`rcrun run-form --headless`, through the `run_form` tool) and in Chrome (the real Build, served).

| Check | Desktop | Page | Status |
|---|---|---|---|
| Start-up | `onLoad`, `onShow`, `onActivate`, once each, in that order | `onLoad` only | **Fixed** (2.0.163): the player queues `onShow` then `onActivate` behind the form's start-up when the form binds them; measured equal |
| `onClose` (a Button whose handler is `INVOKE ME::Close()`) | once, after `onActivate` | once, after `onActivate` | **Equal** (2.0.164): the whole event list and its order |
| `onRestore` | raised when the OS window leaves minimized or maximized | never | **Listed, by design** (Q4: window states are not carried to the browser) |
| `onCloseRejected` | a form waiting on a modal child vetoes the OS close request | never | **Listed, by design**: a page has no close request to veto |
| Rows 4, 6 (repeat) and 12 of the table above | to re-measure with the real host | | **Open**: the host's click on a control needs a headless step that clicks it |
