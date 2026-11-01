<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec 109 — T0.1: control-storage dependency inventory

**Date:** 2026-11-01. **Generator:** `cobolt-codegen` at 2.0.115 (`3d4bc6a7`). **Task:** T0.1 (R29, Q9).

## How to re-derive it

```bash
cargo run -p cobolt-codegen --example spec109_measure -- inventory
```

The example loads every `.cfrm` under `examples/<project>/forms/`, generates each form twice — in full and
with `compact-storage` — and reads the two programs. A group is **kept** when some line other than its own
declaration names it as a whole word beginning with `WS-<ID>`; it is **omittable** when nothing does. Nothing
is written and nothing is changed.

## What was searched, and where nothing depends on a group

(The spec of 2026-10-26 counted 214 groups and 911 declaration lines for PowerAnalytics; today's count is 216 groups, the two extra being controls added to the form since.)

Searched in every crate (`crates/*/src`) for the suffixes `-TEXT`, `-VISIBLE`, `-ENABLED` and for a name built
as `WS-<id>` from a control id: only `cobolt-codegen` (which writes them), `cobolt-compiler` (documentation
text) and two `cobolt-ide` files (an agent prompt and the empty-block check) mention them. The interpreter,
the form host, the WebAssembly generator and the debugger never build such a name from a control's id, so a
group no authored line names is read by nothing at run time.

## Frozen baseline (re-verified)

| Item | Value |
|---|---|
| `examples/PowerAnalytics/forms/main-form.cfrm` | git blob `9086c77b86e3a05fed2b3106429424db304bee0d`, SHA-256 `bdad6bcc4e3258f33c8ceaca8de117fba89bed956f98ee8b506d8e57ce18908c` |
| `examples/PowerAnalytics/generated/main-form.cbl` | 8,688 lines, 387,801 bytes, SHA-256 `281c6767a8d33754a1f395b2af475d2db7eea109e9e5ee03c5020557b3c99d4a` |
| Generator that wrote it | the 2026-10-26 build (2.0.80); today's generator (2.0.115) writes the **same** 8,688 lines and 387,801 bytes from the same form with `compact-storage` off |

So the figures in spec §2 hold: no later slice changed the default output.

## Groups per form

| Project | Form | Controls with a group | Kept (named elsewhere) | Omittable | Declaration lines the omission saves |
|---|---|---|---|---|---|
| PowerAnalytics | main-form.cfrm | 216 | 12 | 204 | 1075 |
| PowerAnalytics | main-form.recovered.cfrm | 216 | 11 | 205 | 1080 |
| PowerChat | agents-form.cfrm | 12 | 0 | 12 | 63 |
| PowerChat | chat-form.cfrm | 18 | 0 | 18 | 91 |
| PowerChat | confirm-form.cfrm | 3 | 0 | 3 | 15 |
| PowerChat | documents-form.cfrm | 21 | 1 | 20 | 104 |
| PowerChat | files-form.cfrm | 18 | 2 | 16 | 82 |
| PowerChat | kb-folder-form.cfrm | 9 | 0 | 9 | 46 |
| PowerChat | model-form.cfrm | 13 | 0 | 13 | 69 |
| PowerChat | pick-form.cfrm | 4 | 0 | 4 | 21 |
| PowerChat | preview-form.cfrm | 3 | 0 | 3 | 15 |
| PowerChat | prompts-form.cfrm | 15 | 2 | 13 | 66 |
| PowerChat | providers-form.cfrm | 20 | 1 | 19 | 99 |
| PowerChat | settings-form.cfrm | 15 | 1 | 14 | 71 |
| PowerChat | topics-form.cfrm | 16 | 2 | 14 | 72 |
| PowerChat | welcome-form.cfrm | 8 | 1 | 7 | 35 |
| PowerCloud | confirm-form.cfrm | 3 | 0 | 3 | 15 |
| PowerCloud | main-form.cfrm | 90 | 7 | 83 | 432 |
| PowerCloud | pet-form.cfrm | 13 | 0 | 13 | 70 |
| PowerCloud | view-form.cfrm | 5 | 0 | 5 | 25 |
| PowerCloud | working-form.cfrm | 3 | 1 | 2 | 10 |
| PowerDemo3 | Charts/chart-3d.cfrm | 9 | 6 | 3 | 20 |
| PowerDemo3 | Charts/charts-form.cfrm | 19 | 6 | 13 | 65 |
| PowerDemo3 | Common/buttons-form.cfrm | 12 | 0 | 12 | 60 |
| PowerDemo3 | Common/checkbox-form.cfrm | 13 | 0 | 13 | 70 |
| PowerDemo3 | Common/combobox-form.cfrm | 14 | 0 | 14 | 74 |
| PowerDemo3 | Common/datetimepicker-form.cfrm | 14 | 0 | 14 | 71 |
| PowerDemo3 | Common/filedropzone-form.cfrm | 11 | 0 | 11 | 60 |
| PowerDemo3 | Common/gauge-form.cfrm | 12 | 0 | 12 | 73 |
| PowerDemo3 | Common/knob-form.cfrm | 14 | 0 | 14 | 86 |
| PowerDemo3 | Common/labels-form.cfrm | 31 | 0 | 31 | 155 |
| PowerDemo3 | Common/listbox-form.cfrm | 13 | 0 | 13 | 68 |
| PowerDemo3 | Common/maps-demo.cfrm | 19 | 0 | 19 | 99 |
| PowerDemo3 | Common/numericupdown-form.cfrm | 15 | 0 | 15 | 76 |
| PowerDemo3 | Common/radiobuttons-form.cfrm | 18 | 0 | 18 | 91 |
| PowerDemo3 | Common/switch-form.cfrm | 15 | 0 | 15 | 79 |
| PowerDemo3 | Common/textboxes-form.cfrm | 18 | 0 | 18 | 105 |
| PowerDemo3 | Common/viewer-form.cfrm | 50 | 1 | 49 | 255 |
| PowerDemo3 | Containers/groupbox-form.cfrm | 20 | 0 | 20 | 104 |
| PowerDemo3 | Containers/panel-form.cfrm | 22 | 0 | 22 | 111 |
| PowerDemo3 | Containers/splitter-form.cfrm | 15 | 0 | 15 | 78 |
| PowerDemo3 | Containers/tabcontrol-form.cfrm | 17 | 0 | 17 | 90 |
| PowerDemo3 | Data/datagrid-form.cfrm | 9 | 1 | 8 | 40 |
| PowerDemo3 | Data/treeview-form.cfrm | 12 | 0 | 12 | 62 |
| PowerDemo3 | General/call-form-demo.cfrm | 5 | 0 | 5 | 25 |
| PowerDemo3 | General/called-form-demo.cfrm | 4 | 0 | 4 | 20 |
| PowerDemo3 | General/inner-form1.cfrm | 18 | 1 | 17 | 109 |
| PowerDemo3 | General/inner-form2.cfrm | 8 | 2 | 6 | 42 |
| PowerDemo3 | General/layers-form.cfrm | 29 | 1 | 28 | 145 |
| PowerDemo3 | General/modern-form.cfrm | 14 | 1 | 13 | 69 |
| PowerDemo3 | General/props-child-form.cfrm | 18 | 0 | 18 | 95 |
| PowerDemo3 | General/props-parent-form.cfrm | 15 | 0 | 15 | 78 |
| PowerDemo3 | Graphics/animator-form.cfrm | 13 | 0 | 13 | 66 |
| PowerDemo3 | Graphics/line-form.cfrm | 19 | 0 | 19 | 96 |
| PowerDemo3 | Graphics/picturebox-form.cfrm | 16 | 0 | 16 | 81 |
| PowerDemo3 | Graphics/progressbar-form.cfrm | 16 | 1 | 15 | 76 |
| PowerDemo3 | Graphics/shape-form.cfrm | 16 | 0 | 16 | 81 |
| PowerDemo3 | Graphics/slider-form.cfrm | 14 | 0 | 14 | 79 |
| PowerDemo3 | Menus & Bars/menubar-form.cfrm | 12 | 0 | 12 | 61 |
| PowerDemo3 | Menus & Bars/sidebar-demo-form.cfrm | 11 | 0 | 11 | 56 |
| PowerDemo3 | Menus & Bars/statusbar-form.cfrm | 13 | 1 | 12 | 61 |
| PowerDemo3 | Menus & Bars/toolbar-form.cfrm | 11 | 0 | 11 | 56 |
| PowerDemo3 | Non-Visual/agent-form.cfrm | 28 | 0 | 28 | 145 |
| PowerDemo3 | Non-Visual/indexedfile-form.cfrm | 20 | 1 | 19 | 99 |
| PowerDemo3 | Non-Visual/restapi-form.cfrm | 17 | 1 | 16 | 82 |
| PowerDemo3 | Non-Visual/snackbar-form.cfrm | 19 | 0 | 19 | 95 |
| PowerDemo3 | Non-Visual/sqldatabase-form.cfrm | 16 | 1 | 15 | 78 |
| PowerDemo3 | Non-Visual/timer-form.cfrm | 17 | 2 | 15 | 76 |
| PowerDemo3 | Non-Visual/websearch-form.cfrm | 16 | 1 | 15 | 78 |
| PowerDemo3 | Responsive Layout/responsive-anchors-form.cfrm | 31 | 0 | 31 | 162 |
| PowerDemo3 | Responsive Layout/responsive-breakpoints-form.cfrm | 36 | 0 | 36 | 180 |
| PowerDemo3 | Responsive Layout/responsive-collide-form.cfrm | 12 | 0 | 12 | 63 |
| PowerDemo3 | Responsive Layout/responsive-dashboard-form.cfrm | 40 | 4 | 36 | 182 |
| PowerDemo3 | Responsive Layout/responsive-dock-form.cfrm | 22 | 1 | 21 | 107 |
| PowerDemo3 | Responsive Layout/responsive-flex-form.cfrm | 185 | 0 | 185 | 925 |
| PowerDemo3 | Responsive Layout/responsive-flow-form.cfrm | 86 | 0 | 86 | 430 |
| PowerDemo3 | Responsive Layout/responsive-fonts-form.cfrm | 23 | 0 | 23 | 116 |
| PowerDemo3 | Responsive Layout/responsive-grid-form.cfrm | 108 | 0 | 108 | 547 |
| PowerDemo3 | Responsive Layout/responsive-nested-form.cfrm | 45 | 0 | 45 | 227 |
| PowerDemo3 | Responsive Layout/responsive-obsolete-form.cfrm | 30 | 0 | 30 | 158 |
| PowerDemo3 | Responsive Layout/responsive-runtime-form.cfrm | 27 | 0 | 27 | 135 |
| PowerDemo3 | Rust/ferris-says-form.cfrm | 8 | 0 | 8 | 41 |
| PowerDemo3 | sidebar-form.cfrm | 59 | 1 | 58 | 313 |
| PowerSpatial | main-form.cfrm | 107 | 4 | 103 | 542 |
| PowerSpatial | rail-form.cfrm | 8 | 1 | 7 | 35 |
| PowerSpatial | rooms-form.cfrm | 8 | 1 | 7 | 35 |
| PowerSpatial | settings-form.cfrm | 6 | 1 | 5 | 26 |
| PowerSpatial | toolbar-form.cfrm | 11 | 1 | 10 | 50 |
| **Total** | | **2350** | **82** | **2268** | |

### Groups kept, and the line that names each

| Project | Form | Group | First line outside its declaration |
|---|---|---|---|
| PowerAnalytics | main-form.cfrm | `WS-SPK` | line 1485: `COBOL::"CHART-ADD-POINT" ( "SPK" WS-SPK-SELECTED-LBL WS-SPK-SELECTED-V` |
| PowerAnalytics | main-form.cfrm | `WS-CH-BAR` | line 1509: `COBOL::"CHART-ADD-POINT" ( "CH-BAR" WS-CH-BAR-SELECTED-LBL WS-CH-BAR-S` |
| PowerAnalytics | main-form.cfrm | `WS-CH-RADAR` | line 1533: `COBOL::"CHART-ADD-POINT" ( "CH-RADAR" WS-CH-RADAR-SELECTED-LBL WS-CH-R` |
| PowerAnalytics | main-form.cfrm | `WS-CH-DONUT1` | line 1557: `COBOL::"CHART-ADD-POINT" ( "CH-DONUT1" WS-CH-DONUT1-SELECTED-LBL WS-CH` |
| PowerAnalytics | main-form.cfrm | `WS-CH-DONUT2` | line 1581: `COBOL::"CHART-ADD-POINT" ( "CH-DONUT2" WS-CH-DONUT2-SELECTED-LBL WS-CH` |
| PowerAnalytics | main-form.cfrm | `WS-CH-LINE` | line 1605: `COBOL::"CHART-ADD-POINT" ( "CH-LINE" WS-CH-LINE-SELECTED-LBL WS-CH-LIN` |
| PowerAnalytics | main-form.cfrm | `WS-GRD-FUNDOS` | line 524: `*>    Set WS-GRD-FUNDOS-CSV-PATH to the desired output file path befor` |
| PowerAnalytics | main-form.cfrm | `WS-DB-1` | line 588: `*>  Connection string is in WS-DB-1-CONN-STRING.` |
| PowerAnalytics | main-form.cfrm | `WS-TMR-STAGE` | no line outside the declarations (kept by a longer name) |
| PowerAnalytics | main-form.cfrm | `WS-RESTCLIENT-1` | no line outside the declarations (kept by a longer name) |
| PowerAnalytics | main-form.cfrm | `WS-RV-TAB` | no line outside the declarations (kept by a longer name) |
| PowerAnalytics | main-form.cfrm | `WS-RN-TAB` | no line outside the declarations (kept by a longer name) |
| PowerAnalytics | main-form.recovered.cfrm | `WS-SPK` | line 1407: `COBOL::"CHART-ADD-POINT" ( "SPK" WS-SPK-SELECTED-LBL WS-SPK-SELECTED-V` |
| PowerAnalytics | main-form.recovered.cfrm | `WS-CH-BAR` | line 1431: `COBOL::"CHART-ADD-POINT" ( "CH-BAR" WS-CH-BAR-SELECTED-LBL WS-CH-BAR-S` |
| PowerAnalytics | main-form.recovered.cfrm | `WS-CH-RADAR` | line 1455: `COBOL::"CHART-ADD-POINT" ( "CH-RADAR" WS-CH-RADAR-SELECTED-LBL WS-CH-R` |
| PowerAnalytics | main-form.recovered.cfrm | `WS-CH-DONUT1` | line 1479: `COBOL::"CHART-ADD-POINT" ( "CH-DONUT1" WS-CH-DONUT1-SELECTED-LBL WS-CH` |
| PowerAnalytics | main-form.recovered.cfrm | `WS-CH-DONUT2` | line 1503: `COBOL::"CHART-ADD-POINT" ( "CH-DONUT2" WS-CH-DONUT2-SELECTED-LBL WS-CH` |
| PowerAnalytics | main-form.recovered.cfrm | `WS-CH-LINE` | line 1527: `COBOL::"CHART-ADD-POINT" ( "CH-LINE" WS-CH-LINE-SELECTED-LBL WS-CH-LIN` |
| PowerAnalytics | main-form.recovered.cfrm | `WS-GRD-FUNDOS` | line 495: `*>    Set WS-GRD-FUNDOS-CSV-PATH to the desired output file path befor` |
| PowerAnalytics | main-form.recovered.cfrm | `WS-DB-1` | line 510: `*>  Connection string is in WS-DB-1-CONN-STRING.` |
| PowerAnalytics | main-form.recovered.cfrm | `WS-TMR-STAGE` | no line outside the declarations (kept by a longer name) |
| PowerAnalytics | main-form.recovered.cfrm | `WS-RV-TAB` | no line outside the declarations (kept by a longer name) |
| PowerAnalytics | main-form.recovered.cfrm | `WS-RN-TAB` | no line outside the declarations (kept by a longer name) |
| PowerChat | documents-form.cfrm | `WS-TMR-LANG` | no line outside the declarations (kept by a longer name) |
| PowerChat | files-form.cfrm | `WS-TMR-LANG` | no line outside the declarations (kept by a longer name) |
| PowerChat | files-form.cfrm | `WS-DG-LIST` | line 590: `*>    Set WS-Dg-List-CSV-PATH to the desired output file path before c` |
| PowerChat | prompts-form.cfrm | `WS-TMR-LANG` | no line outside the declarations (kept by a longer name) |
| PowerChat | prompts-form.cfrm | `WS-DG-LIST` | line 664: `*>    Set WS-Dg-List-CSV-PATH to the desired output file path before c` |
| PowerChat | providers-form.cfrm | `WS-DG-LIST` | no line outside the declarations (kept by a longer name) |
| PowerChat | settings-form.cfrm | `WS-TMR-LANG` | no line outside the declarations (kept by a longer name) |
| PowerChat | topics-form.cfrm | `WS-TMR-LANG` | no line outside the declarations (kept by a longer name) |
| PowerChat | topics-form.cfrm | `WS-DG-LIST` | line 662: `*>    Set WS-Dg-List-CSV-PATH to the desired output file path before c` |
| PowerChat | welcome-form.cfrm | `WS-TMR-LANG` | no line outside the declarations (kept by a longer name) |
| PowerCloud | main-form.cfrm | `WS-TMR-STACK` | no line outside the declarations (kept by a longer name) |
| PowerCloud | main-form.cfrm | `WS-TMR-THUMBS` | no line outside the declarations (kept by a longer name) |
| PowerCloud | main-form.cfrm | `WS-DG-PETS` | no line outside the declarations (kept by a longer name) |
| PowerCloud | main-form.cfrm | `WS-DG-S3` | no line outside the declarations (kept by a longer name) |
| PowerCloud | main-form.cfrm | `WS-TMR-PLAYER` | no line outside the declarations (kept by a longer name) |
| PowerCloud | main-form.cfrm | `WS-DG-EC2` | no line outside the declarations (kept by a longer name) |
| PowerCloud | main-form.cfrm | `WS-TMR-EC2` | no line outside the declarations (kept by a longer name) |
| PowerCloud | working-form.cfrm | `WS-TMR-WORKING` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | Charts/chart-3d.cfrm | `WS-BARCHART-1` | line 251: `COBOL::"CHART-ADD-POINT" ( "BarChart-1" WS-BarChart-1-SELECTED-LBL WS-` |
| PowerDemo3 | Charts/chart-3d.cfrm | `WS-PIECHART-1` | line 275: `COBOL::"CHART-ADD-POINT" ( "PieChart-1" WS-PieChart-1-SELECTED-LBL WS-` |
| PowerDemo3 | Charts/chart-3d.cfrm | `WS-RADARCHART-1` | line 299: `COBOL::"CHART-ADD-POINT" ( "RadarChart-1" WS-RadarChart-1-SELECTED-LBL` |
| PowerDemo3 | Charts/chart-3d.cfrm | `WS-LINECHART-1` | line 323: `COBOL::"CHART-ADD-POINT" ( "LineChart-1" WS-LineChart-1-SELECTED-LBL W` |
| PowerDemo3 | Charts/chart-3d.cfrm | `WS-TIMER-1` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | Charts/chart-3d.cfrm | `WS-MS-TAB` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | Charts/charts-form.cfrm | `WS-BARCHART-1` | line 246: `COBOL::"CHART-ADD-POINT" ( "BarChart-1" WS-BarChart-1-SELECTED-LBL WS-` |
| PowerDemo3 | Charts/charts-form.cfrm | `WS-LINECHART-1` | line 270: `COBOL::"CHART-ADD-POINT" ( "LineChart-1" WS-LineChart-1-SELECTED-LBL W` |
| PowerDemo3 | Charts/charts-form.cfrm | `WS-PIECHART-1` | line 294: `COBOL::"CHART-ADD-POINT" ( "PieChart-1" WS-PieChart-1-SELECTED-LBL WS-` |
| PowerDemo3 | Charts/charts-form.cfrm | `WS-AREACHART-1` | line 318: `COBOL::"CHART-ADD-POINT" ( "AreaChart-1" WS-AreaChart-1-SELECTED-LBL W` |
| PowerDemo3 | Charts/charts-form.cfrm | `WS-SCATTERCHART-1` | line 342: `COBOL::"CHART-ADD-POINT" ( "ScatterChart-1" WS-ScatterChart-1-SELECTED` |
| PowerDemo3 | Charts/charts-form.cfrm | `WS-DONUTCHART-1` | line 366: `COBOL::"CHART-ADD-POINT" ( "DonutChart-1" WS-DonutChart-1-SELECTED-LBL` |
| PowerDemo3 | Common/viewer-form.cfrm | `WS-TMR-STREAM` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | Data/datagrid-form.cfrm | `WS-DATAGRID-1` | line 144: `*>    Set WS-DataGrid-1-CSV-PATH to the desired output file path befor` |
| PowerDemo3 | General/inner-form1.cfrm | `WS-TIMER-1` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | General/inner-form2.cfrm | `WS-LINECHART-1` | line 170: `COBOL::"CHART-ADD-POINT" ( "LineChart-1" WS-LineChart-1-SELECTED-LBL W` |
| PowerDemo3 | General/inner-form2.cfrm | `WS-TIMER-1` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | General/layers-form.cfrm | `WS-TMR-WORK` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | General/modern-form.cfrm | `WS-GRD-LIST` | line 115: `*>    Set WS-GRD-LIST-CSV-PATH to the desired output file path before ` |
| PowerDemo3 | Graphics/progressbar-form.cfrm | `WS-TMR-COPY` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | Menus & Bars/statusbar-form.cfrm | `WS-TMR-CLOCK` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | Non-Visual/indexedfile-form.cfrm | `WS-IDX-MENU` | line 170: `IF WS-Idx-Menu-IS-OPEN = 0` |
| PowerDemo3 | Non-Visual/restapi-form.cfrm | `WS-RESTCLIENT-1` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | Non-Visual/sqldatabase-form.cfrm | `WS-DB-LOCAL` | line 174: `*>  Connection string is in WS-Db-Local-CONN-STRING.` |
| PowerDemo3 | Non-Visual/timer-form.cfrm | `WS-TMR-FAST` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | Non-Visual/timer-form.cfrm | `WS-TMR-SLOW` | no line outside the declarations (kept by a longer name) |
| PowerDemo3 | Non-Visual/websearch-form.cfrm | `WS-WEB-FIND` | line 169: `*>    Google Custom Search via Web-Find — MOVE your query text to WS-W` |
| PowerDemo3 | Responsive Layout/responsive-dashboard-form.cfrm | `WS-DB-BAR` | line 190: `COBOL::"CHART-ADD-POINT" ( "DB-BAR" WS-DB-BAR-SELECTED-LBL WS-DB-BAR-S` |
| PowerDemo3 | Responsive Layout/responsive-dashboard-form.cfrm | `WS-DB-LINE` | line 214: `COBOL::"CHART-ADD-POINT" ( "DB-LINE" WS-DB-LINE-SELECTED-LBL WS-DB-LIN` |
| PowerDemo3 | Responsive Layout/responsive-dashboard-form.cfrm | `WS-DB-ORDERS` | line 166: `*>    Set WS-DB-ORDERS-CSV-PATH to the desired output file path before` |
| PowerDemo3 | Responsive Layout/responsive-dashboard-form.cfrm | `WS-DB-PIE` | line 238: `COBOL::"CHART-ADD-POINT" ( "DB-PIE" WS-DB-PIE-SELECTED-LBL WS-DB-PIE-S` |
| PowerDemo3 | Responsive Layout/responsive-dock-form.cfrm | `WS-MAIN-GRID` | line 148: `*>    Set WS-MAIN-GRID-CSV-PATH to the desired output file path before` |
| PowerDemo3 | sidebar-form.cfrm | `WS-TIMER-1` | no line outside the declarations (kept by a longer name) |
| PowerSpatial | main-form.cfrm | `WS-CHT-WAVE` | line 602: `COBOL::"CHART-ADD-POINT" ( "Cht-Wave" WS-Cht-Wave-SELECTED-LBL WS-Cht-` |
| PowerSpatial | main-form.cfrm | `WS-TMR-WAVE` | no line outside the declarations (kept by a longer name) |
| PowerSpatial | main-form.cfrm | `WS-TMR-CLOCK` | no line outside the declarations (kept by a longer name) |
| PowerSpatial | main-form.cfrm | `WS-TMR-LANG` | no line outside the declarations (kept by a longer name) |
| PowerSpatial | rail-form.cfrm | `WS-TMR-LANG` | no line outside the declarations (kept by a longer name) |
| PowerSpatial | rooms-form.cfrm | `WS-TMR-LANG` | no line outside the declarations (kept by a longer name) |
| PowerSpatial | settings-form.cfrm | `WS-TMR-LANG` | no line outside the declarations (kept by a longer name) |
| PowerSpatial | toolbar-form.cfrm | `WS-TMR-LANG` | no line outside the declarations (kept by a longer name) |

## What this does not cover

- **Common Code and other forms.** A group is judged by the program of its own form. Common Code reaches a
  form's data only through `CALL … USING` of items the form names, so such an item is a name in the form.
- **Debugger watches.** A watch typed at run time names an item by hand; an omitted group cannot be watched.
  That is why `compact-storage` is opt-in.
- **Dynamic references.** COBOL cannot build a data-name at run time, so none exists in these forms; the one
  late-bound spelling is the debugger watch above.

## Generated helper paragraphs (T5.2)

```bash
cargo run -p cobolt-codegen --example spec109_measure -- helpers
```

Every paragraph of a form's outer program, by what it is for, with how many lines it takes and how many of them
the developer's own code names (`PERFORM <name>` or `CALL "<name>"` inside a handler or procedure body).
`Named by authored code` is **0 for every kind in every example project**.

| Project | Kind of paragraph | Paragraphs | Lines | Named by authored code |
|---|---|---|---|---|
| PowerAnalytics | animation dispatch | 2 | 336 | 0 |
| PowerAnalytics | animation trigger | 160 | 482 | 0 |
| PowerAnalytics | chart wrapper | 24 | 147 | 0 |
| PowerAnalytics | csv export | 1 | 13 | 0 |
| PowerAnalytics | database wrapper | 8 | 79 | 0 |
| PowerAnalytics | event loop | 1 | 151 | 0 |
| PowerAnalytics | handler or procedure | 4 | 45 | 0 |
| PowerAnalytics | other runtime paragraph | 2 | 16 | 0 |
| PowerChat | animation dispatch | 2 | 8 | 0 |
| PowerChat | animation trigger | 1 | 6 | 0 |
| PowerChat | csv export | 3 | 54 | 0 |
| PowerChat | database wrapper | 4 | 22 | 0 |
| PowerChat | event loop | 14 | 722 | 0 |
| PowerChat | handler or procedure | 8 | 48 | 0 |
| PowerChat | other runtime paragraph | 20 | 181 | 0 |
| PowerCloud | event loop | 5 | 255 | 0 |
| PowerCloud | other runtime paragraph | 7 | 75 | 0 |
| PowerDemo3 | animation dispatch | 10 | 116 | 0 |
| PowerDemo3 | animation trigger | 24 | 85 | 0 |
| PowerDemo3 | chart wrapper | 56 | 348 | 0 |
| PowerDemo3 | csv export | 4 | 75 | 0 |
| PowerDemo3 | database wrapper | 11 | 112 | 0 |
| PowerDemo3 | event loop | 62 | 3373 | 0 |
| PowerDemo3 | handler or procedure | 20 | 219 | 0 |
| PowerDemo3 | other runtime paragraph | 71 | 634 | 0 |
| PowerSpatial | chart wrapper | 4 | 27 | 0 |
| PowerSpatial | event loop | 5 | 230 | 0 |
| PowerSpatial | other runtime paragraph | 10 | 121 | 0 |
| **All** | **animation dispatch** | **14** | **460** | **0** |
| **All** | **animation trigger** | **185** | **573** | **0** |
| **All** | **chart wrapper** | **84** | **522** | **0** |
| **All** | **csv export** | **8** | **142** | **0** |
| **All** | **database wrapper** | **23** | **213** | **0** |
| **All** | **event loop** | **87** | **4731** | **0** |
| **All** | **handler or procedure** | **32** | **312** | **0** |
| **All** | **other runtime paragraph** | **110** | **1027** | **0** |

What this says, and what was done with it:

- **Nothing looks a helper paragraph up by name.** Searched in `cobolt-runtime`, `cobolt-form-host`,
  `cobolt-wasm` and `cobolt-web-player` for the paragraph names and for the suffixes they are built from
  (`-SET-TABLE`, `-ADD-POINT`, `-REFRESH`, `-EXPORT-CSV`, `-CONNECT`, `COBOL-PLAY-ANIMATION`, …): no match. The
  generated event loop is what runs handlers, so a helper is reachable only by a line of the program.
- **Left out when nothing names them (2.0.117, opt-in with `compact-storage`):** the animation dispatch
  (`COBOL-PLAY-ANIMATION`, `COBOL-STOP-ANIMATION`), the `<ctrl>-PLAY-<animation>` triggers, a chart's
  `-SET-TABLE` / `-ADD-POINT` / `-CLEAR` / `-REFRESH`, a database's `-CONNECT` / `-EXEC` / `-FETCH-ALL` /
  `-CLOSE` and a grid's `-EXPORT-CSV` (or its `CSVParagraph`). A paragraph that any other line names stays, comments
  included; removing one can leave another unnamed (the dispatch, once its triggers are gone), so the pass repeats.
- **Kept always:** the event loop, `COBOL-MAIN`, `COBOL-START-TIMERS`, the binding paragraphs, a database's
  `-ON-CONNECT` / `-ON-QUERY-DONE` / `-ON-ERROR` (event handler stubs, not helpers) and every handler.
- **Not replaced by a runtime service.** The spec's wording for T5.2 is "runtime services replace generator
  plumbing". The one candidate with weight, the animation dispatch (336 lines on PowerAnalytics), cannot be
  swapped for a call the runtime answers without the browser generator learning the form's animations: it
  compiles this very paragraph and its `FormSpec` carries the controls' names but not their animations. Leaving
  the paragraph out when nothing performs it gives the same saving, needs no new call and breaks no host.
