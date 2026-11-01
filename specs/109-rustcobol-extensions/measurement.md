<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec 109 — T-M: measurement of the generated programs

**Date:** 2026-11-01. **Generator:** 2.0.115 (`3d4bc6a7`). **Task:** T-M (R36, AC11, AC20).

## How to re-derive it

```bash
cargo run -p cobolt-codegen --example spec109_measure -- measure
```

Every `.cfrm` under `examples/<project>/forms/` is generated twice by the current generator: as it always was
(`compact-storage` off) and with the S5 opt-in on. Counts are of the generated COBOL as written — **no comment
or blank line is stripped from any count**.

| Column | Meaning |
|---|---|
| Total lines / Bytes | every line / every byte of the generated program |
| Non-blank | lines with something on them |
| Executable | non-blank, non-comment lines from `PROCEDURE DIVISION` on |
| User-typed | lines inside an event handler's or a procedure's body (the developer's verbatim code) |
| Generated-only | total lines less user-typed lines |
| (compact) | the same form generated with `compact-storage` on |
| Lines saved | total lines less total lines (compact) |

## Result

| Project | Form | Total lines | Non-blank | Executable | User-typed | Generated-only | Bytes | Total lines (compact) | Bytes (compact) | Lines saved |
|---|---|---|---|---|---|---|---|---|---|---|
| PowerAnalytics | main-form.cfrm | 8688 | 6948 | 4224 | 5198 | 3490 | 387801 | 6575 | 295377 | 2113 |
| PowerAnalytics | main-form.recovered.cfrm | 5990 | 5171 | 3888 | 2581 | 3409 | 235827 | 3869 | 143143 | 2121 |
| **PowerAnalytics** | all forms | **14678** | **12119** | **8112** | **7779** | **6899** | **623628** | **10444** | **438520** | **4234** |
| PowerChat | agents-form.cfrm | 1142 | 924 | 458 | 667 | 475 | 52695 | 1079 | 49965 | 63 |
| PowerChat | chat-form.cfrm | 7074 | 5791 | 2861 | 5247 | 1827 | 362747 | 6983 | 359111 | 91 |
| PowerChat | confirm-form.cfrm | 405 | 332 | 133 | 141 | 264 | 17225 | 390 | 16624 | 15 |
| PowerChat | documents-form.cfrm | 3720 | 3063 | 1337 | 2455 | 1265 | 202526 | 3616 | 198188 | 104 |
| PowerChat | files-form.cfrm | 1828 | 1487 | 645 | 908 | 920 | 90969 | 1730 | 86854 | 98 |
| PowerChat | kb-folder-form.cfrm | 726 | 577 | 253 | 295 | 431 | 33210 | 680 | 31210 | 46 |
| PowerChat | model-form.cfrm | 1340 | 1102 | 534 | 731 | 609 | 65008 | 1271 | 62097 | 69 |
| PowerChat | pick-form.cfrm | 445 | 367 | 157 | 170 | 275 | 19048 | 424 | 18167 | 21 |
| PowerChat | preview-form.cfrm | 193 | 162 | 50 | 26 | 167 | 7764 | 178 | 7174 | 15 |
| PowerChat | prompts-form.cfrm | 2280 | 1878 | 868 | 1302 | 978 | 115873 | 2195 | 112308 | 85 |
| PowerChat | providers-form.cfrm | 2241 | 1835 | 813 | 1305 | 936 | 115674 | 2142 | 111556 | 99 |
| PowerChat | settings-form.cfrm | 2409 | 1981 | 889 | 1627 | 782 | 127073 | 2338 | 124003 | 71 |
| PowerChat | topics-form.cfrm | 2576 | 2106 | 969 | 1538 | 1038 | 131393 | 2485 | 127621 | 91 |
| PowerChat | welcome-form.cfrm | 490 | 400 | 136 | 157 | 333 | 24748 | 441 | 22410 | 49 |
| **PowerChat** | all forms | **26869** | **22005** | **10103** | **16569** | **10300** | **1365953** | **25952** | **1327288** | **917** |
| PowerCloud | confirm-form.cfrm | 239 | 198 | 70 | 36 | 203 | 9138 | 224 | 8537 | 15 |
| PowerCloud | main-form.cfrm | 3758 | 3160 | 1274 | 1418 | 2340 | 181935 | 3326 | 161623 | 432 |
| PowerCloud | pet-form.cfrm | 406 | 347 | 108 | 70 | 336 | 17663 | 336 | 14700 | 70 |
| PowerCloud | view-form.cfrm | 232 | 196 | 60 | 32 | 200 | 9340 | 207 | 8357 | 25 |
| PowerCloud | working-form.cfrm | 197 | 163 | 53 | 20 | 177 | 7664 | 187 | 7233 | 10 |
| **PowerCloud** | all forms | **4832** | **4064** | **1565** | **1576** | **3256** | **225740** | **4280** | **200450** | **552** |
| PowerDemo3 | Charts/chart-3d.cfrm | 558 | 456 | 178 | 165 | 393 | 25368 | 426 | 20013 | 132 |
| PowerDemo3 | Charts/charts-form.cfrm | 1076 | 847 | 547 | 515 | 561 | 43669 | 864 | 34411 | 212 |
| PowerDemo3 | Common/buttons-form.cfrm | 523 | 399 | 251 | 105 | 418 | 17669 | 390 | 12137 | 133 |
| PowerDemo3 | Common/checkbox-form.cfrm | 597 | 469 | 195 | 288 | 309 | 28442 | 527 | 25224 | 70 |
| PowerDemo3 | Common/combobox-form.cfrm | 532 | 417 | 169 | 235 | 297 | 24680 | 458 | 21348 | 74 |
| PowerDemo3 | Common/datetimepicker-form.cfrm | 457 | 361 | 134 | 186 | 271 | 21265 | 386 | 18091 | 71 |
| PowerDemo3 | Common/filedropzone-form.cfrm | 466 | 371 | 139 | 209 | 257 | 22343 | 406 | 19485 | 60 |
| PowerDemo3 | Common/gauge-form.cfrm | 463 | 374 | 124 | 203 | 260 | 22868 | 390 | 19455 | 73 |
| PowerDemo3 | Common/knob-form.cfrm | 558 | 439 | 171 | 246 | 312 | 26043 | 472 | 22074 | 86 |
| PowerDemo3 | Common/labels-form.cfrm | 1468 | 999 | 784 | 480 | 988 | 40841 | 1313 | 34929 | 155 |
| PowerDemo3 | Common/listbox-form.cfrm | 535 | 420 | 171 | 244 | 291 | 24999 | 467 | 21878 | 68 |
| PowerDemo3 | Common/maps-demo.cfrm | 775 | 582 | 290 | 395 | 380 | 31658 | 676 | 27033 | 99 |
| PowerDemo3 | Common/numericupdown-form.cfrm | 486 | 381 | 143 | 197 | 289 | 22111 | 410 | 18684 | 76 |
| PowerDemo3 | Common/radiobuttons-form.cfrm | 782 | 618 | 256 | 426 | 356 | 39386 | 691 | 35321 | 91 |
| PowerDemo3 | Common/switch-form.cfrm | 531 | 415 | 169 | 226 | 305 | 24337 | 452 | 20794 | 79 |
| PowerDemo3 | Common/textboxes-form.cfrm | 672 | 514 | 335 | 216 | 456 | 22645 | 567 | 18377 | 105 |
| PowerDemo3 | Common/viewer-form.cfrm | 1663 | 1247 | 893 | 730 | 933 | 59605 | 1408 | 48305 | 255 |
| PowerDemo3 | Containers/groupbox-form.cfrm | 469 | 370 | 126 | 165 | 304 | 20909 | 365 | 16238 | 104 |
| PowerDemo3 | Containers/panel-form.cfrm | 459 | 360 | 123 | 148 | 311 | 19767 | 348 | 14848 | 111 |
| PowerDemo3 | Containers/splitter-form.cfrm | 423 | 338 | 115 | 158 | 265 | 19685 | 345 | 16004 | 78 |
| PowerDemo3 | Containers/tabcontrol-form.cfrm | 457 | 362 | 129 | 167 | 290 | 20591 | 367 | 16547 | 90 |
| PowerDemo3 | Data/datagrid-form.cfrm | 438 | 336 | 184 | 150 | 288 | 15975 | 379 | 13181 | 59 |
| PowerDemo3 | Data/treeview-form.cfrm | 607 | 477 | 206 | 314 | 293 | 28842 | 545 | 25977 | 62 |
| PowerDemo3 | General/call-form-demo.cfrm | 194 | 161 | 48 | 13 | 181 | 7477 | 169 | 6241 | 25 |
| PowerDemo3 | General/called-form-demo.cfrm | 214 | 173 | 62 | 25 | 189 | 7787 | 194 | 6861 | 20 |
| PowerDemo3 | General/inner-form1.cfrm | 328 | 267 | 76 | 42 | 286 | 12770 | 219 | 7801 | 109 |
| PowerDemo3 | General/inner-form2.cfrm | 342 | 271 | 108 | 64 | 278 | 12914 | 260 | 9395 | 82 |
| PowerDemo3 | General/layers-form.cfrm | 540 | 414 | 159 | 145 | 395 | 22187 | 395 | 14784 | 145 |
| PowerDemo3 | General/modern-form.cfrm | 232 | 198 | 40 | 0 | 232 | 9405 | 144 | 5735 | 88 |
| PowerDemo3 | General/props-child-form.cfrm | 423 | 342 | 118 | 124 | 299 | 17974 | 328 | 14018 | 95 |
| PowerDemo3 | General/props-parent-form.cfrm | 374 | 308 | 101 | 118 | 256 | 16621 | 296 | 13172 | 78 |
| PowerDemo3 | Graphics/animator-form.cfrm | 532 | 409 | 173 | 233 | 299 | 23508 | 466 | 20434 | 66 |
| PowerDemo3 | Graphics/line-form.cfrm | 454 | 356 | 126 | 158 | 296 | 19777 | 358 | 15563 | 96 |
| PowerDemo3 | Graphics/picturebox-form.cfrm | 586 | 457 | 192 | 266 | 320 | 27043 | 505 | 23390 | 81 |
| PowerDemo3 | Graphics/progressbar-form.cfrm | 570 | 446 | 167 | 252 | 318 | 26649 | 494 | 23261 | 76 |
| PowerDemo3 | Graphics/shape-form.cfrm | 475 | 373 | 143 | 194 | 281 | 21629 | 394 | 17878 | 81 |
| PowerDemo3 | Graphics/slider-form.cfrm | 525 | 410 | 161 | 220 | 305 | 23698 | 446 | 20112 | 79 |
| PowerDemo3 | Menus & Bars/menubar-form.cfrm | 468 | 365 | 155 | 197 | 271 | 20848 | 407 | 17775 | 61 |
| PowerDemo3 | Menus & Bars/sidebar-demo-form.cfrm | 392 | 311 | 116 | 149 | 243 | 17914 | 336 | 15123 | 56 |
| PowerDemo3 | Menus & Bars/statusbar-form.cfrm | 447 | 352 | 135 | 170 | 277 | 20046 | 386 | 17145 | 61 |
| PowerDemo3 | Menus & Bars/toolbar-form.cfrm | 549 | 449 | 189 | 254 | 295 | 26488 | 493 | 23678 | 56 |
| PowerDemo3 | Non-Visual/agent-form.cfrm | 797 | 603 | 297 | 340 | 457 | 33759 | 652 | 27087 | 145 |
| PowerDemo3 | Non-Visual/indexedfile-form.cfrm | 786 | 655 | 285 | 316 | 470 | 38954 | 675 | 34055 | 111 |
| PowerDemo3 | Non-Visual/restapi-form.cfrm | 853 | 628 | 414 | 416 | 437 | 31739 | 771 | 27686 | 82 |
| PowerDemo3 | Non-Visual/snackbar-form.cfrm | 1016 | 736 | 494 | 564 | 452 | 38591 | 921 | 33338 | 95 |
| PowerDemo3 | Non-Visual/sqldatabase-form.cfrm | 769 | 621 | 253 | 356 | 413 | 37640 | 631 | 31744 | 138 |
| PowerDemo3 | Non-Visual/timer-form.cfrm | 575 | 452 | 172 | 246 | 329 | 27107 | 499 | 23533 | 76 |
| PowerDemo3 | Non-Visual/websearch-form.cfrm | 679 | 539 | 226 | 302 | 377 | 32382 | 601 | 28708 | 78 |
| PowerDemo3 | Responsive Layout/responsive-anchors-form.cfrm | 613 | 488 | 189 | 202 | 411 | 26176 | 451 | 19032 | 162 |
| PowerDemo3 | Responsive Layout/responsive-breakpoints-form.cfrm | 616 | 481 | 201 | 155 | 461 | 24405 | 436 | 17038 | 180 |
| PowerDemo3 | Responsive Layout/responsive-collide-form.cfrm | 312 | 255 | 87 | 75 | 237 | 13657 | 249 | 10615 | 63 |
| PowerDemo3 | Responsive Layout/responsive-dashboard-form.cfrm | 606 | 494 | 155 | 112 | 494 | 25294 | 331 | 14498 | 275 |
| PowerDemo3 | Responsive Layout/responsive-dock-form.cfrm | 468 | 378 | 157 | 125 | 343 | 19454 | 342 | 14013 | 126 |
| PowerDemo3 | Responsive Layout/responsive-flex-form.cfrm | 1410 | 1108 | 251 | 155 | 1255 | 53697 | 485 | 17015 | 925 |
| PowerDemo3 | Responsive Layout/responsive-flow-form.cfrm | 850 | 665 | 204 | 129 | 721 | 31737 | 420 | 15047 | 430 |
| PowerDemo3 | Responsive Layout/responsive-fonts-form.cfrm | 546 | 425 | 202 | 152 | 394 | 21403 | 430 | 16352 | 116 |
| PowerDemo3 | Responsive Layout/responsive-grid-form.cfrm | 796 | 643 | 87 | 75 | 721 | 32929 | 249 | 10567 | 547 |
| PowerDemo3 | Responsive Layout/responsive-nested-form.cfrm | 573 | 463 | 156 | 130 | 443 | 23442 | 346 | 14390 | 227 |
| PowerDemo3 | Responsive Layout/responsive-obsolete-form.cfrm | 570 | 447 | 199 | 134 | 436 | 21883 | 412 | 15296 | 158 |
| PowerDemo3 | Responsive Layout/responsive-runtime-form.cfrm | 735 | 562 | 329 | 214 | 521 | 26929 | 600 | 21411 | 135 |
| PowerDemo3 | Rust/ferris-says-form.cfrm | 278 | 227 | 83 | 33 | 245 | 10398 | 206 | 7538 | 72 |
| PowerDemo3 | sidebar-form.cfrm | 911 | 697 | 333 | 184 | 727 | 32948 | 527 | 15821 | 384 |
| **PowerDemo3** | all forms | **37399** | **29181** | **12875** | **13207** | **24192** | **1562957** | **29176** | **1207504** | **8223** |
| PowerSpatial | main-form.cfrm | 2227 | 1704 | 595 | 769 | 1458 | 99124 | 1658 | 74598 | 569 |
| PowerSpatial | rail-form.cfrm | 574 | 442 | 201 | 204 | 370 | 23006 | 539 | 21591 | 35 |
| PowerSpatial | rooms-form.cfrm | 545 | 419 | 186 | 188 | 357 | 21932 | 510 | 20483 | 35 |
| PowerSpatial | settings-form.cfrm | 450 | 353 | 126 | 168 | 282 | 19887 | 424 | 18756 | 26 |
| PowerSpatial | toolbar-form.cfrm | 415 | 326 | 97 | 95 | 320 | 17871 | 365 | 15875 | 50 |
| **PowerSpatial** | all forms | **4211** | **3244** | **1205** | **1424** | **2787** | **181820** | **3496** | **151303** | **715** |
| **All** | | **87989** | **70613** | **33860** | **40555** | **47434** | **3960098** | **73348** | **3325065** | **14641** |

`main-form.recovered.cfrm` is a recovered copy that sits in the same folder; its row is counted because the
example reads the folder, and it is not part of the PowerAnalytics baseline.

The "(compact)" columns include both parts of S5: the control groups nothing names (T5.1) and the helper
paragraphs nothing names (T5.2). Measured separately on PowerAnalytics `main-form`: groups alone 8,688 → 7,613
lines (−1,075), groups and helper paragraphs 6,575 (−2,113, so the helper paragraphs are −1,038).

## Against the frozen baseline

| | Lines | Bytes |
|---|---|---|
| PowerAnalytics `main-form` — baseline (2026-10-26) | 8,688 | 387,801 |
| Same form, today, `compact-storage` off | 8,688 | 387,801 |
| Same form, today, groups left out (2.0.112) | 7,613 | 343,659 |
| Same form, today, groups and helper paragraphs left out (2.0.117) | 6,575 | 295,377 |
| Saved by S5 in all | 2,113 (24.3 %) | 92,424 (23.8 %) |
| The study's target (spec §1) | at most 4,344 | |

Across the five example projects: 14,641 of 87,989 lines (16.6 %) and 635,033 of 3,960,098 bytes (16.0 %).
Over the 87 forms without the recovered copies, compact generation parses and checks no worse than full
generation for every one, and the developer's lines are the same lines at their new numbers
(`compact_storage_109.rs::every_example_form_compact_parses_and_checks_no_worse_than_full`: 81,999 → 69,479
lines, 2,063 groups and 300 helper labels left out, 0 forms worse).

## Honest gap

- **S5 is the only slice that shrinks the generated file by itself.** It takes `main-form` from 8,688 to 6,575
  lines. That is **2,231 lines (34 %) above the 4,344 target**: the target is not met and this report does not
  claim it.
- **What is left of the generated file** is 1,377 lines nobody typed (6,575 less the 5,198 user-typed ones): the
  event loop (151 lines), the data-division scaffolding and the handler programs' headers. The event loop is the
  largest single piece and no runtime service replaces it today.
- **S1–S4 and S8 change what a developer *writes*, not what the generator emits**, so they cannot move these
  numbers until PowerAnalytics' authored handlers are rewritten with user functions, collections, formatting,
  `Descendants` and `SetProperties`. That rewrite has not been done, so there is **no per-construct figure**:
  the 5,198 user-typed lines of `main-form` are the pool such a rewrite would draw on, and they are now 79 % of
  the compact file — the target cannot be reached without it.
- **S6 and S7** replace code a developer would otherwise write (a timer paragraph such as `STAGE-TICK`, 270
  lines, or a `SET` list per breakpoint); like the rest they need the authored code rewritten before they show
  here. Their own behaviour is measured by their tests (`layout_script`, `sequence`), not by line counts.
