# Handoff — ToolBar: what shipped, what is still open

Rewritten 2026-08-17 at the end of the session that finished the three items the
previous handoff left. Read `CLAUDE.md` first; this only covers what is unfinished.

## State of the tree

`main` is at **1.61.74**, three commits and two merges **ahead of `origin/main`**
— **nothing is pushed**, because the São Paulo window (Mon–Fri 09:00–18:00) was
open the whole session (GOLDEN RULE #1). Push when it closes:

```bash
git push origin main
```

Full sweep green:

```bash
cargo test -q --workspace --exclude cobolt-bench --features cobolt-forms/render \
  --no-fail-fast -- --skip generated_binary_source_actually_compiles
```

**109 suites, 2032 passed, 0 failed.** Then the skipped one on its own:

```bash
cargo test -p cobolt-compiler --lib generated_binary_source_actually_compiles
```

It is skipped in the sweep because it points a nested `cargo build` at the
workspace's own `target/` via `CARGO_TARGET_DIR`, pruning artefacts the outer run
is still using. A spawned task is open to fix it; until then, skip-then-run-alone.

## What shipped this session

| Version | Kind | What |
|---|---|---|
| 1.61.72 | fix | A toolbar button did nothing in **Preview**. `toolbar_actions` moved from `cobolt-form-host` to `cobolt_forms::toolbar_actions` (behind `render`, beside `toolbar_paint`), so the IDE can use it without promoting the dev-dependency the manifest says not to promote. Preview carries out `print`, `run-app`, `open-terminal`, `copy`, `cut`, `paste`, and reports each result in the Output pane. `screenshot`/`share` are refused with a reason — Preview is a pane inside the IDE window. `preview_press` holds the whole rule; its test pins all 11 verbs. |
| 1.61.73 | fix | `procedure:` and `open-modal:` **reached nothing**. A button is named by a derived id, `<toolbar>-<group>-<button>` upper-cased, from `cobolt_forms::toolbar::button_control_id` — one function, so the renderer's event and codegen's `WHEN` cannot drift. `procedure:` → `CALL "<NAME>"`; `open-modal:` → `INVOKE ME::"OpenFormSync"("<FORM>")` (one-arg = modal). A derived id over 64 chars (`COBOL-CONTROL-ID` is `PIC X(64)`), or an empty target, gets a **comment** in the generated source, written OUTSIDE the `EVALUATE` so a form never emits one with no `WHEN`. |
| 1.61.74 | feature | **First-class buttons.** `ToolbarButton.events` (a `ButtonEvent{event,code}`, not an `EventBinding`), round-tripping through `ToolbarLayout`. Toolbar Editor → button → **Events** → **Edit code** closes the editor keeping the toolbar and opens the COBOL editor. Codegen emits the handler as an `IS COMMON` nested program and CALLs it. Run order: toolbar `onClick` → button `onClick` → action. COBOL may write 7 properties (`Tooltip` + 6 colours); **everything else is a runtime error** naming the property and the allowed set, through all three doors. `build_known_controls` lists buttons, so the editor flags a refused property as it is typed. |

Key symbols added: `cobolt_forms::toolbar::{button_control_id, MAX_BUTTON_CONTROL_ID,
find_button, ButtonRef, ButtonEvent, BUTTON_EVENTS, BUTTON_WRITABLE,
button_writable, apply_button_write, write_into_layout, WriteRefused,
refusal_message, ToolbarDef::from_json, ToolbarDef::button_with_group}`;
`cobolt_forms::toolbar_actions` (moved); `cobolt_ide::app::{preview_press,
PreviewPress}`; `toolbar_editor::EditorOutcome::SaveAndEditEvent`.

## Open items

| Item | Status |
|---|---|
| **Push + forum announcements** | `main` is 1.61.74 and unpushed. After pushing: **f=97** (fixes) needs the two fixes above added to the existing draft — it now covers seven fixes; **f=96** (features, prefix `[Noticia]`, thread "Nuevas funcionalidades de PowerRustCOBOL") needs first-class buttons. cobolforo.es sat behind a Cloudflare bot check that did not clear, so the **operator must drive the browser**. Text must be re-derived for these versions. |
| **`Enabled` is not writable on a button** | Deliberate, and the most likely thing to be asked for next. Greying a button out until a record is loaded is idiomatic, but `Enabled` is neither a colour nor a tooltip, and the operator's constraint was exactly "colors and tooltips". Adding it is ~3 lines: `BUTTON_WRITABLE` + a branch in `apply_button_write`. Needs their word. |
| **Gauge / DataGrid / TreeView have no event to announce a change** | Unchanged from the last handoff. `Gauge.Value`, `DataGrid.Rows`, `TreeView.Items` declare no observer event, so a write to them can tell the form nothing. Adding one ADDS a capability — operator's call. |
| **Linear gauge readout** | Draws *above* its bar; the operator described it as always *under*. One line, never flipped, needs their word. |
| **Duplicate `Drop Shadow` label** | **Could not reproduce.** `toolbar_editor.rs` has exactly one "Drop shadow:" and no `sec_shadow` header; the `tr.sec_shadow` header is in `properties.rs`, a different pane. Either it was fixed already or the last handoff misread it. |
| **`cobolt-runtime` → `cobolt-forms` dependency** | Still standing (added 1.61.66 so `FDZ::CommitFiles()` could use `dropzone`'s copy rules). `rcrun` links the form model even for console programs. This session leaned on it again — the interpreter's button-write guard calls `cobolt_forms::toolbar`. |

## Gotchas that cost real time here

- **`ColorImage::as_raw` is behind epaint's `bytemuck` feature**, which `eframe`
  turns on and a bare `egui` dependency does not. Moving code that touches a
  capture out of an eframe-linked crate breaks on it; `Color32::to_srgba_unmultiplied`
  needs no feature and is what epaint would have you use anyway.
- **`MOVE … TO X::Prop` arrives UPPER-CASED; `CALL "COBOL-SET-PROPERTY"` and
  `INVOKE X "SetProperty"` keep the case they were given.** Any property allow-list
  or router must compare case-insensitively. A test that compared exactly caught
  this, which is the only reason it is written down.
- **An `EVALUATE` with no `WHEN` is not COBOL.** Anything conditional that emits
  `WHEN` branches has to decide whether it will emit ANY before opening the
  `EVALUATE`. Refusal comments belong outside it.
- **`exec_method` returns `CobolValue`, not `Result`** — a guard that must raise
  cannot live there. The three developer-facing property-write paths that CAN raise
  are `assign_member`, the `CALL "COBOL-SET-PROPERTY"` arm of `exec_call`, and the
  `Stmt::Invoke` site (which is where a method on a receiver can be refused before
  `exec_method` sees it).
- **`validate_handler_semantics` silently passes a handler it cannot splice.** It
  regenerates a probe form with the candidate written in; if the target is not a
  control it found, nothing is spliced and the gate validates the OLD program. Any
  new kind of handler target has to be added there as well as to `save_event_handler`.
- **`cobolt-forms` needs `--features render`** or its own tests fail to *compile*.
  **`cobolt-ide` has no lib target** — use `--bin cobolt-ide`.
- **Never verdict a sweep from a `head`-truncated pipe** (`head` SIGPIPEs cargo).
  Redirect to a file and count every `test result` line.
- **Check `df` before believing a build error** — `No space left on device` surfaces
  as `could not compile <innocent crate>`. 50 GB free at the end of this session.
- **egui: `Window::max_size` bounds the CONTENT, not the window** (the title bar is
  ~38 px on top), and a `Window` sizes itself from its content, so a ceiling looser
  than the opening size invites inflation.
- **`f32::clamp` panics when min > max**, and propagates NaN.
- **The elegance shape baseline** (`paint.rs::elegance_baseline_reports_untouched_paths`)
  must only be re-blessed with intent, documenting which control moved and by how much.
- **Changing compiler KB doc constants requires rebuilding `chunked.data`**:
  `cargo run --release -p cobolt-ide --example build_chunked_kb`. A red
  `prebuilt_chunked_kb_matches_the_published_documentation` is a real failure.
