<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# PowerRustCOBOL — per-control test examples

One self-contained PowerRustCOBOL project per toolbox control. Each project
demonstrates that the control's **events** fire and its **properties** can be
driven from COBOL at runtime. See `specs/004-control-test-examples/` for the
spec, plan, and tasks.

> Status: **work in progress** — projects are being authored control by control.

## What each project contains

```
examples/<control>/
  cobolt.toml          project manifest
  forms/<control>.cfrm form: the subject control + a button column
  src/main.cbl         entry program
  README.md            what it demonstrates + Build/fix-errors steps
  assets/              sample image/data, where the control needs it
```

Each project:

1. **Events** — one console `DISPLAY "<Event> working"` per supported event
   (e.g. `MouseEnter working`). Non-pointer events (Timer `onTick`, RestClient
   `onResponseReceived`, …) are fired by a small trigger button.
2. **Colours** — a button changes the control's fore/back colour from COBOL via
   `INVOKE <Subject> "SetProperty" USING "ForegroundColor" "#0066CC"`.
3. **Other properties** — a button per property that changes it at runtime so the
   effect is visually (or, for non-visual, console-) confirmable.

## The control surface (authoritative metadata)

`CONTROL-METADATA.md` lists every control's supported events and property keys.
Regenerate it any time with:

```sh
cargo run -p cobolt-forms --example list_controls > examples/CONTROL-METADATA.md
```

## Build a project

```sh
rcrun build examples/<control>/cobolt.toml      # CLI build
```

…or open `examples/<control>/cobolt.toml` in the IDE and use **Build**. If the
build reports an error, read it, fix the handler/form, and rebuild — every
project must build with zero errors.

## Index

Every project demonstrates the same thing for its control — a `DISPLAY` per
supported event and a button per property — so the table lists only the folder
and any external service the control needs to run.

| Control | Folder | Service required |
|---------|--------|------------------|
| Button | `button/` | — |
| Label | `label/` | — |
| TextBox | `text-box/` | — |
| CheckBox | `check-box/` | — |
| RadioButton | `radio-button/` | — |
| ComboBox | `combo-box/` | — |
| ListBox | `list-box/` | — |
| NumericUpDown | `numeric-up-down/` | — |
| DateTimePicker | `date-time-picker/` | — |
| GroupBox | `group-box/` | — |
| Panel | `panel/` | — |
| TabControl | `tab-control/` | — |
| Splitter | `splitter/` | — |
| DataGrid | `data-grid/` | — |
| TreeView | `tree-view/` | — |
| PictureBox | `picture-box/` | — |
| Animator | `animator/` | — |
| ProgressBar | `progress-bar/` | — |
| Slider | `slider/` | — |
| Line | `line/` | — |
| Shape | `shape/` | — |
| MenuBar | `menu-bar/` | — |
| ToolBar | `tool-bar/` | — |
| StatusBar | `status-bar/` | — |
| Timer | `timer/` | — |
| AgentObject | `agent-object/` | local LLM endpoint |
| RestClient | `rest-client/` | HTTP endpoint |
| SqlDatabase | `sql-database/` | database connection |
| BarChart | `bar-chart/` | — |
| LineChart | `line-chart/` | — |
| PieChart | `pie-chart/` | — |
| AreaChart | `area-chart/` | — |
| ScatterChart | `scatter-chart/` | — |
| DonutChart | `donut-chart/` | — |

Build them all at once: `examples/build-all.sh` (must report `0 failed`).
Verify coverage: `cargo run -p cobolt-codegen --example check_examples`.
