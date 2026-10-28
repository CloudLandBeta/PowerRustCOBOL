// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The player's base stylesheet: the lowest layer of the cascade (WASM-007 R8 — "the theme's
//! defaults"). It gives every control type a plain, readable default look; everything a form's
//! properties say comes after it (`@layer pr-props`), and everything the developer writes comes after
//! that, unlayered, so it wins whatever its specificity.
//!
//! The class names (`pr-button`, `pr-textbox`, …) and the custom properties (`--pr-accent`, …) are a
//! public contract once shipped (WASM-007 Q4); `view/contract.rs` pins them. A host page sets the
//! custom properties on the mount element and they inherit into the shadow root (WASM-007 R9).

pub const BASE_CSS: &str = r#"
:host { display: block; contain: layout; width: 100%; height: 100%; }
*, *::before, *::after { box-sizing: border-box; }
:host {
  --pr-accent: #4e8cff; --pr-surface: #2e3138; --pr-surface-2: #3a3e47; --pr-text: #ffffff;
  --pr-text-dim: #b8bcc6; --pr-border: #565b66; --pr-radius: 6px; --pr-danger: #e5484d;
  --pr-font: "PR Ubuntu Light", system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; --pr-focus: #8ab4ff; --pr-field: #f4f5f7; --pr-field-text: #14161a; --pr-field-border: #aab0ba;
}
.pr-root { width: 100%; height: 100%; position: relative; color: var(--pr-text); font: 11px var(--pr-font); line-height: 13px; overflow: hidden;
  /* The desktop's text is the sum of the font's advances and its pair kerning, with no hinting and no synthetic bold (egui). */
  font-kerning: normal; font-synthesis-weight: none; text-rendering: geometricPrecision;
  /* The desktop draws a glyph with a coverage boost a browser does not: the same text carries about 20 % less ink here. A quarter of a pixel of outline (measured on 9 to 28 px text) brings the two to within 5 %. */
  -webkit-text-stroke: 0.25px; }
[hidden] { display: none !important; }
.pr-main { position: absolute; inset: 0; }
.pr-form { position: absolute; inset: 0; overflow: auto; background: var(--pr-surface); color: var(--pr-text); }
.pr-surface { position: relative; }
.pr-form[data-responsive] > .pr-surface { position: absolute; inset: 0; }
.pr-control { font-family: inherit; }
.pr-disabled { opacity: .55; pointer-events: none; }
:focus-visible { outline: 2px solid var(--pr-focus); outline-offset: 1px; }

/* buttons, labels, text */
.pr-button { cursor: pointer; font: inherit; color: inherit; background: var(--pr-surface-2); border: 1px solid var(--pr-border); border-radius: var(--pr-radius); padding: 2px 10px; }
.pr-button:hover { filter: brightness(1.15); } .pr-button:active { filter: brightness(.9); }
.pr-button .pr-icon { width: 1.3em; height: 1.3em; flex: none; }
.pr-label { display: flex; align-items: center; white-space: pre; overflow: hidden; }
.pr-textbox, .pr-combobox, .pr-numericupdown, .pr-datetimepicker { display: flex; font: inherit; color: var(--pr-field-text); background: var(--pr-field);
  border: 1px solid var(--pr-border); border-radius: 4px; padding: 2px 6px; min-width: 0; }
.pr-textbox input, .pr-textbox textarea, .pr-combobox input, .pr-numericupdown input, .pr-datetimepicker input { flex: 1; min-width: 0; width: 100%; height: 100%;
  font: inherit; color: inherit; background: transparent; border: 0; padding: 0; margin: 0; outline: none; resize: none; text-align: inherit; }
.pr-textbox textarea { overflow: var(--pr-overflow, hidden); }
.pr-textbox input::placeholder, .pr-textbox textarea::placeholder { color: inherit; opacity: .55; }
.pr-combobox select { width: 100%; height: 100%; font: inherit; color: inherit; background: transparent; border: 0; outline: none; }
.pr-combobox select option { color: #000; }
/* A list-only ComboBox at rest: the desktop's closed header (the value, a small triangle) over the native <select>, which stays over it, clear. */
.pr-combobox { position: relative; }
.pr-combobox > .pr-combo-select { position: absolute; inset: 0; width: 100%; height: 100%; opacity: 0; cursor: pointer; }
.pr-combo-text { position: absolute; white-space: pre; pointer-events: none; margin: 0; padding: 0; }
.pr-combo-arrow { position: absolute; pointer-events: none; overflow: visible; }
/* What a date or number field shows at rest: the desktop's text (its formats, not the browser's locale's) centred in the frame, over the native input that edits it. */
.pr-numericupdown, .pr-datetimepicker { position: relative; }
.pr-field-face { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; white-space: pre; pointer-events: none; padding-top: var(--pr-nt, 0px); padding-bottom: var(--pr-nb, 0px); }
.pr-numericupdown input, .pr-datetimepicker input { opacity: 0; text-align: center; }
.pr-numericupdown:focus-within input { opacity: 1; }
.pr-numericupdown:focus-within .pr-field-face { visibility: hidden; }
.pr-numericupdown input::-webkit-inner-spin-button, .pr-numericupdown input::-webkit-outer-spin-button { -webkit-appearance: none; margin: 0; }
.pr-datetimepicker input::-webkit-calendar-picker-indicator { position: absolute; inset: 0; width: 100%; height: 100%; opacity: 0; cursor: pointer; }
.pr-glyph { display: inline-block; width: 1.0287em; overflow: visible; }
.pr-glyph-date { height: .7987em; vertical-align: -.2487em; }
.pr-glyph-time { height: .8741em; vertical-align: -.3418em; }
.pr-checkbox, .pr-radiobutton { display: flex; align-items: center; gap: .5em; cursor: pointer; }
.pr-checkbox input, .pr-radiobutton input { margin: 0; flex: none; }
.pr-switch { display: block; cursor: pointer; background: #555a64; border: 0; border-radius: 999px; padding: 0; }
.pr-switch .pr-thumb { position: absolute; top: 12%; left: 6%; height: 76%; aspect-ratio: 1; border-radius: 50%; background: #fff; transition: left .12s; }
.pr-switch[aria-checked="true"] { background: var(--pr-accent); } .pr-switch[aria-checked="true"] .pr-thumb { left: auto; right: 6%; }
.pr-weblink { cursor: pointer; text-decoration: none; color: var(--pr-link, #6aa6ff); display: inline-flex; align-items: center; }
.pr-weblink.pr-underline-always { text-decoration: underline; } .pr-weblink.pr-underline-hover:hover { text-decoration: underline; }
.pr-weblink:visited { color: var(--pr-visited, #b58cff); } .pr-weblink:active { color: var(--pr-active, #ff7a7a); }
.pr-weblink[aria-disabled="true"] { opacity: .6; cursor: not-allowed; }

/* containers */
.pr-groupbox { border: 1px solid var(--pr-border); border-radius: var(--pr-radius); padding: 0; margin: 0; min-width: 0; }
.pr-groupbox > legend { padding: 0 .4em; }
.pr-panel { overflow: hidden; }
/* The expand / collapse icon of a card that is Expandable (spec 090): a 24 px square in its top-right corner, as the painter lays it. */
.pr-expander { position: absolute; top: var(--pr-exp-inset, 6px); right: var(--pr-exp-inset, 6px); width: 24px; height: 24px; padding: 0; margin: 0; border: 0; border-radius: 6px; background: transparent; color: inherit; cursor: pointer; z-index: 5; }
.pr-expander:hover { background: color-mix(in srgb, currentColor 16%, transparent); }
.pr-expander svg { display: block; width: 24px; height: 24px; }
.pr-tabcontrol { overflow: hidden; }
.pr-tablist { display: flex; gap: 2px; overflow: hidden; }
.pr-tabs-left > .pr-tablist, .pr-tabs-right > .pr-tablist { flex-direction: column; }
.pr-tabs-bottom > .pr-tablist { align-items: flex-start; }
.pr-tab { padding: 4px var(--pr-tab-pad, 12px); cursor: pointer; background: var(--pr-tab-bg, transparent); color: var(--pr-tab-fg, inherit); border: 1px solid var(--pr-border);
  border-radius: 4px 4px 0 0; font: inherit; white-space: nowrap; }
.pr-tabs-left > .pr-tablist > .pr-tab, .pr-tabs-right > .pr-tablist > .pr-tab { border-radius: 4px 0 0 4px; text-align: left; }
.pr-tabs-bottom > .pr-tablist > .pr-tab { border-radius: 0 0 4px 4px; }
.pr-tab[aria-selected="true"] { background: var(--pr-accent); color: var(--pr-tab-active-fg, #fff); border-color: var(--pr-accent); }
.pr-tabpanel { position: relative; width: 100%; height: 100%; }
.pr-splitter { display: flex; overflow: hidden; }
.pr-splitter.pr-vertical { flex-direction: column; }
.pr-splitter > .pr-pane { position: relative; overflow: hidden; min-width: 0; min-height: 0; }
.pr-splitter > .pr-handle { flex: 0 0 var(--pr-line-size, 6px); background: var(--pr-line, var(--pr-border)); cursor: col-resize; touch-action: none; }
.pr-splitter.pr-vertical > .pr-handle { cursor: row-resize; }

/* lists, trees, grids */
/* A ListBox is drawn the way the desktop draws it (view/controls.rs): a frame, a scroll area 3 px inside it with rows of a size
   the painter works out, and a pill-shaped handle on the right while the rows do not fit. */
.pr-listbox { position: relative; overflow: hidden; background: var(--pr-field); color: var(--pr-field-text); border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-lb-scroll { position: absolute; list-style: none; margin: 0; padding: 0; overflow-y: auto; overflow-x: hidden; scrollbar-width: none; overscroll-behavior: contain; outline: none; }
.pr-lb-scroll::-webkit-scrollbar { display: none; }
.pr-lb-scroll li { display: flex; align-items: center; box-sizing: border-box; cursor: default; white-space: nowrap; }
.pr-lb-text { position: relative; white-space: pre; -webkit-text-stroke: 0; }
.pr-lb-box { display: inline-flex; flex: none; box-sizing: border-box; border: 1px solid; align-items: center; justify-content: center; }
.pr-lb-box svg { overflow: visible; display: block; }
.pr-lb-handle { position: absolute; border-radius: 2px; background: rgb(60 60 60 / .6); pointer-events: none; }
.pr-treeview { list-style: none; margin: 0; padding: 2px; overflow: auto; background: var(--pr-field); color: var(--pr-field-text); border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-treeview ul { list-style: none; margin: 0; padding-left: 16px; }
.pr-node-row { display: flex; align-items: center; gap: 4px; padding: 1px 4px; cursor: default; white-space: nowrap; }
.pr-node-row .pr-arrow { width: 1.2em; flex: none; background: transparent; border: 0; color: inherit; padding: 0; cursor: pointer; font: inherit; }
.pr-node-icon { width: 1.1em; height: 1.1em; flex: none; }
.pr-treeview li.pr-selected > .pr-node-row { background: var(--pr-accent); color: #fff; }
/* The DataGrid is drawn the way the desktop draws it (view/datagrid.rs): a clipped frame, a sheet that scrolls natively
   under sticky bands, and overlays for the lines, the shadows and the bars. Sizes and places are written per element. */
.pr-datagrid { position: relative; overflow: hidden; background: var(--pr-field); color: var(--pr-field-text); }
.pr-dg-scroll { position: absolute; inset: 0; overflow: auto; scrollbar-width: none; overscroll-behavior: contain; }
.pr-dg-scroll::-webkit-scrollbar { display: none; }
.pr-dg-sheet { position: relative; }
.pr-dg-caption { position: sticky; top: 0; left: 0; z-index: 5; box-sizing: border-box; overflow: hidden; }
.pr-dg-head { position: sticky; z-index: 4; display: flex; box-sizing: border-box; }
.pr-dg-row { position: absolute; left: 0; display: flex; }
.pr-dg-c, .pr-dg-hc { position: relative; flex: none; box-sizing: border-box; padding: 0 2px; overflow: clip; overflow-clip-margin: content-box; }
.pr-dg-hc { cursor: pointer; user-select: none; }
.pr-dg-fz { position: sticky; z-index: 2; }
.pr-dg-ln { box-shadow: inset 1px 0 0 0 var(--pr-dg-line, rgb(27 42 78)); }
.pr-dg-lf { box-shadow: 1px 0 0 0 var(--pr-dg-line, rgb(27 42 78)); }
.pr-dg-ln.pr-dg-lf { box-shadow: inset 1px 0 0 0 var(--pr-dg-line, rgb(27 42 78)), 1px 0 0 0 var(--pr-dg-line, rgb(27 42 78)); }
/* The desktop's glyphs are hinted to the pixel grid and a browser's are not: at the grid's sizes the two carry the same
   ink with no outline added (measured on the grid's titles and cells), which the labels' calibrated outline would overshoot. */
.pr-dg-t { position: absolute; white-space: pre; margin: 0; padding: 0; font-weight: 400; text-shadow: none; -webkit-text-stroke: 0; pointer-events: none; }
.pr-dg-sel::after { content: ""; position: absolute; inset: 0 2px; background: var(--pr-dg-sel, rgb(80 145 255 / .216)); pointer-events: none; }
.pr-dg-clip { position: absolute; left: 0; top: 0; overflow: hidden; pointer-events: none; }
.pr-dg-mv { position: absolute; overflow: hidden; cursor: pointer; }
.pr-dg-csv { position: absolute; box-sizing: border-box; border: 1px solid; border-radius: 3px; cursor: pointer; }
.pr-dg-fill { position: absolute; pointer-events: none; }
.pr-dg-hl { position: absolute; left: 0; right: 0; height: 1px; background: var(--pr-dg-line, rgb(27 42 78)); z-index: 6; pointer-events: none; }
.pr-dg-vl { position: absolute; width: 1px; background: var(--pr-dg-line, rgb(27 42 78)); z-index: 6; pointer-events: none; }
.pr-dg-ol { position: absolute; inset: 0; box-sizing: border-box; border: 1px solid var(--pr-dg-line, rgb(27 42 78)); z-index: 7; pointer-events: none; }
.pr-dg-sh { position: absolute; z-index: 8; pointer-events: none; }
.pr-dg-sh-l { background: linear-gradient(to right, rgb(0 0 0 / .2157), rgb(0 0 0 / 0)); }
.pr-dg-sh-t { background: linear-gradient(to bottom, rgb(0 0 0 / .2157), rgb(0 0 0 / 0)); }
.pr-dg-track { position: absolute; z-index: 9; border-radius: 5px; background: rgb(0 0 0 / .2157); pointer-events: none; }
.pr-dg-thumb { position: absolute; z-index: 9; border-radius: 5px; background: rgb(0 0 0 / .588); pointer-events: none; }
.pr-dg-thumb-add { background: rgb(230 235 255); mix-blend-mode: plus-lighter; }
.pr-dg-asc::after, .pr-dg-desc::after { content: ""; position: absolute; right: 6px; top: 50%; margin-top: -2px; border: 3.5px solid transparent; opacity: .8; }
.pr-dg-asc::after { border-bottom: 5px solid var(--pr-dg-head-ink, #fff); border-top: 0; }
.pr-dg-desc::after { border-top: 5px solid var(--pr-dg-head-ink, #fff); border-bottom: 0; }
.pr-progressbar { display: flex; align-items: center; gap: 6px; }
.pr-progressbar progress { flex: 1; width: 100%; height: 100%; accent-color: var(--pr-accent); }
.pr-slider { display: flex; align-items: center; gap: 6px; }
.pr-slider input { flex: 1; width: 100%; height: 100%; accent-color: var(--pr-accent); margin: 0; }
.pr-slider.pr-vertical input { writing-mode: vertical-lr; direction: rtl; }
.pr-statusbar { display: flex; align-items: center; background: var(--pr-surface-2); border-top: 1px solid var(--pr-border); }
.pr-statusbar .pr-panel-item { padding: 0 8px; border-right: 1px solid var(--pr-border); }
.pr-line { display: block; }
.pr-picturebox, .pr-animator { overflow: hidden; display: flex; align-items: center; justify-content: center; }
.pr-picturebox img, .pr-animator img { max-width: 100%; max-height: 100%; }

/* menus and the shell */
.pr-menubar, .pr-toolbar { display: flex; align-items: center; gap: 2px; background: var(--pr-surface-2); border-bottom: 1px solid var(--pr-border); padding: 0 4px; }
.pr-menubar.pr-responsive { left: 0 !important; right: 0 !important; width: auto !important; }
.pr-menu { position: relative; list-style: none; }
.pr-menu > button { display: flex; align-items: center; gap: .5em; width: 100%; padding: 4px 12px; background: transparent; border: 0; color: inherit; font: inherit; cursor: pointer; text-align: left; white-space: nowrap; }
.pr-menu > button:hover:not(:disabled) { background: var(--pr-menu-hover, var(--pr-accent)); color: var(--pr-menu-hover-fg, #fff); }
.pr-menu > button:disabled { opacity: .5; cursor: default; }
.pr-menu-icon { width: 1.2em; height: 1.2em; flex: none; }
.pr-menu-accel { margin-left: auto; opacity: .7; padding-left: 1.5em; }
.pr-menu-label { flex: 1; }
.pr-menubar .pr-menu-items { position: absolute; left: 0; top: 100%; min-width: 11em; margin: 0; padding: 2px 0; list-style: none; background: var(--pr-surface-2); border: 1px solid var(--pr-border); box-shadow: 0 6px 18px rgba(0,0,0,.35); z-index: 50; }
.pr-menubar .pr-menu-items .pr-menu-items { left: 100%; top: 0; }
.pr-menu-items hr { margin: 3px 0; border: 0; border-top: 1px solid var(--pr-border); }
.pr-menu-section { padding: 8px 12px 2px; font-size: .8em; text-transform: uppercase; opacity: .6; letter-spacing: .06em; }
.pr-badge { margin-left: auto; font-size: .75em; padding: 0 .6em; border-radius: 999px; background: var(--pr-accent); color: #fff; }
.pr-badge-outline { background: transparent; border: 1px solid var(--pr-accent); color: var(--pr-accent); }
.pr-badge-count { min-width: 1.6em; text-align: center; padding: 0 .3em; }
.pr-sidemenu { display: flex; flex-direction: column; background: var(--pr-surface-2); border-right: 1px solid var(--pr-border); overflow: hidden; }
.pr-sm-header { display: flex; align-items: center; gap: 8px; padding: 10px 12px; min-height: 48px; }
.pr-sm-header img { max-height: 40px; max-width: 100%; }
.pr-sm-title { font-weight: 600; }
.pr-sm-toggle { align-self: flex-end; margin: 0 8px 4px; background: transparent; border: 0; color: inherit; cursor: pointer; font: inherit; }
.pr-sm-items { flex: 1; overflow: auto; margin: 0; padding: 0; list-style: none; }
.pr-sidemenu .pr-menu-items { position: static; margin: 0; padding: 0 0 0 1em; list-style: none; }
.pr-sidemenu .pr-menu > button[aria-current="page"] { background: var(--pr-accent); color: var(--pr-menu-selected-fg, #fff); }
.pr-sidemenu .pr-chevron { margin-left: auto; opacity: .7; }
.pr-sidemenu.pr-collapsed .pr-menu-label, .pr-sidemenu.pr-collapsed .pr-menu-accel, .pr-sidemenu.pr-collapsed .pr-badge, .pr-sidemenu.pr-collapsed .pr-chevron,
.pr-sidemenu.pr-collapsed .pr-menu-section { display: none; }
.pr-sm-footer { flex: none; position: relative; }
.pr-main[data-shell] > .pr-form > .pr-surface > .pr-sidemenu { width: var(--pr-rail, 220px) !important; top: 0 !important; bottom: 0 !important; height: auto !important; }
.pr-shell-pane { position: absolute; top: 0; right: 0; bottom: 0; left: var(--pr-rail-end, 220px); display: flex; flex-direction: column; pointer-events: none; }
.pr-shell-pane > * { pointer-events: auto; }
.pr-crumb-host { flex: none; }
.pr-breadcrumb { display: flex; align-items: center; justify-content: space-between; padding: 4px 12px; background: var(--pr-surface-2); border-bottom: 1px solid var(--pr-border); color: var(--pr-text-dim); }
.pr-breadcrumb ol { display: flex; gap: .5em; list-style: none; margin: 0; padding: 0; }
.pr-breadcrumb li + li::before { content: "\203A"; margin-right: .5em; opacity: .6; }
.pr-breadcrumb button { background: transparent; border: 0; color: var(--pr-accent); cursor: pointer; font: inherit; padding: 0; }
.pr-content { position: relative; flex: 1; overflow: auto; background: var(--pr-surface); }
.pr-content > .pr-form { position: absolute; inset: 0; }
.pr-tb-group { display: flex; gap: 2px; align-items: center; padding: 2px; }
.pr-toolbar button { display: inline-flex; align-items: center; gap: .4em; padding: 3px 8px; background: transparent; border: 1px solid transparent; border-radius: 4px; color: inherit; font: inherit; cursor: pointer; }
.pr-toolbar button:hover:not(:disabled) { background: rgba(255,255,255,.1); border-color: var(--pr-border); }
.pr-toolbar button:disabled { opacity: .45; cursor: default; }
.pr-tb-icon { width: 1.3em; height: 1.3em; }

/* drawn controls */
.pr-barchart, .pr-linechart, .pr-piechart, .pr-areachart, .pr-scatterchart, .pr-donutchart, .pr-radarchart, .pr-gauge, .pr-knob, .pr-shape { display: block; }
.pr-chart, .pr-shape-svg, .pr-barchart > svg, .pr-linechart > svg, .pr-piechart > svg, .pr-areachart > svg, .pr-scatterchart > svg, .pr-donutchart > svg, .pr-radarchart > svg,
.pr-gauge > svg, .pr-knob > svg, .pr-shape > svg { display: block; width: 100%; height: 100%; }
.pr-barchart text, .pr-linechart text, .pr-piechart text, .pr-areachart text, .pr-scatterchart text, .pr-donutchart text, .pr-radarchart text, .pr-gauge text, .pr-knob text { font-family: var(--pr-font); -webkit-text-stroke: 0; }
.pr-knob { touch-action: none; cursor: pointer; }
.pr-maps { position: relative; overflow: hidden; background: #1c1f24; touch-action: none; cursor: grab; }
.pr-tiles { position: absolute; inset: 0; }
.pr-tiles img { position: absolute; width: 256px; height: 256px; user-select: none; -webkit-user-drag: none; max-width: none; }
.pr-map-overlay { position: absolute; left: 0; top: 0; pointer-events: none; }
.pr-markers { position: absolute; inset: 0; pointer-events: none; }
.pr-marker { position: absolute; width: 22px; height: 22px; margin: -22px 0 0 -11px; padding: 0; border: 2px solid #fff; border-radius: 50% 50% 50% 0; transform: rotate(-45deg); background: var(--pr-marker, #e5484d); pointer-events: auto; cursor: pointer; }
.pr-info { position: absolute; transform: translate(-50%, -100%); max-width: 16em; padding: 8px 10px; background: var(--pr-surface-2); border: 1px solid var(--pr-border); border-radius: 8px; box-shadow: 0 4px 14px rgba(0,0,0,.4); z-index: 5; }
.pr-info p { margin: .25em 0 0; }
.pr-map-zoom { position: absolute; right: 8px; top: 8px; display: flex; flex-direction: column; gap: 2px; }
.pr-map-zoom button { width: 28px; height: 28px; background: var(--pr-surface-2); color: inherit; border: 1px solid var(--pr-border); border-radius: 4px; cursor: pointer; font: inherit; }
.pr-attribution { position: absolute; right: 4px; bottom: 2px; font-size: 10px; opacity: .7; background: rgba(0,0,0,.4); padding: 0 4px; }
.pr-viewer { overflow: auto; background: #1c1f24; padding: 8px; user-select: text; }
.pr-viewer .pr-content { position: static; background: transparent; }
.pr-viewer pre, .pr-viewer code { font-family: ui-monospace, Menlo, Consolas, monospace; }
::highlight(pr-find) { background-color: #ffd54a; color: #1a1a1a; } ::highlight(pr-find-current) { background-color: #ff8a00; color: #000; }
.pr-findbar { position: sticky; top: 0; z-index: 2; display: flex; gap: 4px; align-items: center; padding: 4px; margin: -8px -8px 6px; background: var(--pr-surface-2); border-bottom: 1px solid var(--pr-border); }
.pr-findbar input { flex: 1; min-width: 0; font: inherit; color: var(--pr-field-text); background: var(--pr-field); border: 1px solid var(--pr-border); border-radius: 4px; padding: 2px 6px; }
.pr-findbar button { font: inherit; color: inherit; background: transparent; border: 1px solid var(--pr-border); border-radius: 4px; min-width: 1.8em; cursor: pointer; } .pr-findbar button[aria-pressed="true"] { background: var(--pr-accent); color: #fff; }
.pr-find-count { min-width: 4.5em; text-align: center; font-variant-numeric: tabular-nums; opacity: .85; }
.pr-viewer .pr-mermaid { margin: 8px 0; background: #fff; border-radius: 4px; padding: 6px; overflow-x: auto; } .pr-viewer .pr-mermaid svg { max-width: 100%; height: auto; display: block; }
.pr-viewer .pr-mermaid-source figcaption { opacity: .8; font-size: .9em; margin-bottom: 4px; } .pr-viewer .pr-mermaid-source[aria-busy="true"] figcaption { font-style: italic; }
.pr-viewer pre { overflow: auto; padding: 8px; background: rgba(255,255,255,.05); border-radius: 4px; }
.pr-viewer table { border-collapse: collapse; } .pr-viewer td, .pr-viewer th { border: 1px solid var(--pr-border); padding: 2px 6px; }
.pr-viewer blockquote { margin: .5em 0; padding-left: 1em; border-left: 3px solid var(--pr-border); color: var(--pr-text-dim); }
.pr-viewer .pr-message.pr-user { margin: 4px 0 4px auto; max-width: 80%; padding: 6px 10px; border-radius: 12px; }
.pr-viewer .pr-message.pr-agent { margin: 4px auto 4px 0; max-width: 80%; padding: 6px 10px; border-radius: 12px; }
.pr-viewer a { color: var(--pr-link, #6aa6ff); }
.pr-viewer .pr-error, .pr-error { color: var(--pr-danger); }
.pr-filedropzone { display: flex; flex-direction: column; align-items: center; justify-content: center; border: 2px dashed var(--pr-border); border-radius: var(--pr-radius); color: var(--pr-text-dim); cursor: pointer; }
.pr-filedropzone.pr-over { border-color: var(--pr-accent); color: var(--pr-text); }
.pr-filedropzone .pr-files { margin: 4px 0 0; padding: 0 1em; list-style: none; }
.pr-canvas2d canvas, .pr-canvas3d canvas { width: 100%; height: 100%; display: block; }

.pr-authentication, .pr-auth { display: flex; flex-direction: column; gap: 8px; padding: 12px; background: var(--pr-surface-2); border: 1px solid var(--pr-border); border-radius: var(--pr-radius); overflow: auto; }
.pr-auth label { display: flex; flex-direction: column; gap: 2px; } .pr-auth input { font: inherit; color: var(--pr-field-text); background: var(--pr-field); border: 1px solid var(--pr-border); border-radius: 4px; padding: 4px 6px; }
.pr-auth .pr-auth-message { color: var(--pr-danger); min-height: 1.35em; } .pr-auth nav { display: flex; gap: 8px; flex-wrap: wrap; } .pr-auth nav button { background: transparent; border: 0; color: var(--pr-accent); cursor: pointer; font: inherit; padding: 0; }
.pr-auth form, .pr-auth .pr-auth-signed-in { display: flex; flex-direction: column; gap: 8px; } .pr-auth .pr-auth-title { margin: 0; font-size: 1.1em; }
.pr-auth button.pr-auth-primary, .pr-auth .pr-auth-signed-in > button { padding: 6px 12px; background: var(--pr-accent); color: #fff; border: 0; border-radius: 4px; cursor: pointer; font: inherit; }
.pr-auth button.pr-auth-primary:disabled { opacity: .6; cursor: progress; }
.pr-auth input:focus-visible, .pr-auth button:focus-visible { outline: 2px solid var(--pr-accent); outline-offset: 2px; }

/* the page's own furniture */
.pr-layer { position: absolute; inset: 0; pointer-events: none; z-index: 20; }
.pr-layer > * { pointer-events: auto; }
.pr-dialog { padding: 0; border: 1px solid var(--pr-border); border-radius: var(--pr-radius); background: var(--pr-surface); color: var(--pr-text); max-width: 100vw; max-height: 100vh; font: 13px/1.35 var(--pr-font); }
.pr-dialog::backdrop { background: rgba(0,0,0,.45); }
.pr-dialog > .pr-titlebar { display: flex; align-items: center; justify-content: space-between; padding: 6px 10px; background: var(--pr-surface-2); border-bottom: 1px solid var(--pr-border); font-weight: 600; }
.pr-titlebar button { background: transparent; border: 0; color: inherit; font-size: 16px; cursor: pointer; }
.pr-dialog > .pr-body { position: relative; }
.pr-dialog > .pr-body > .pr-form { position: absolute; inset: 0; }
.pr-dialog .pr-dialog-buttons { display: flex; justify-content: flex-end; gap: 8px; padding: 8px 12px; }
.pr-dialog .pr-dialog-buttons button { padding: 4px 16px; background: var(--pr-surface-2); color: inherit; border: 1px solid var(--pr-border); border-radius: 4px; cursor: pointer; font: inherit; }
.pr-dialog .pr-dialog-buttons button.pr-default { background: var(--pr-accent); border-color: var(--pr-accent); color: #fff; }
.pr-dialog .pr-dialog-text { padding: 14px 16px; white-space: pre-wrap; max-width: 36em; }
.pr-dialog .pr-dialog-field { padding: 0 16px 8px; } .pr-dialog .pr-dialog-field input { width: 100%; font: inherit; color: var(--pr-field-text); background: var(--pr-field); border: 1px solid var(--pr-border); border-radius: 4px; padding: 4px 6px; }
.pr-panelwin { position: absolute; border: 1px solid var(--pr-border); border-radius: var(--pr-radius); background: var(--pr-surface); box-shadow: 0 6px 24px rgba(0,0,0,.4); display: flex; flex-direction: column; }
.pr-panelwin > .pr-titlebar { display: flex; justify-content: space-between; align-items: center; padding: 4px 8px; background: var(--pr-surface-2); cursor: move; touch-action: none; user-select: none; }
.pr-panelwin > .pr-body { position: relative; flex: 1 1 auto; min-height: 0; overflow: hidden; }
.pr-panelwin > .pr-body > .pr-form { position: absolute; inset: 0; }
.pr-toasts-host { position: absolute; left: 12px; bottom: 12px; z-index: 40; pointer-events: none; }
.pr-snackbars { display: flex; flex-direction: column; gap: 8px; }
.pr-snackbar { pointer-events: auto; display: flex; align-items: center; gap: 10px; padding: 8px 12px; background: #202329; color: #fff; border-radius: var(--pr-radius); border-left: 4px solid var(--pr-accent); box-shadow: 0 4px 16px rgba(0,0,0,.4); }
.pr-snackbar[data-category="Error"], .pr-snackbar[data-category="Critical"] { border-left-color: var(--pr-danger); } .pr-snackbar[data-category="Warning"] { border-left-color: #f5a623; } .pr-snackbar[data-category="Question"] { border-left-color: #8e4ec6; }
.pr-snack-icon { width: 1.4em; height: 1.4em; flex: none; }
.pr-snackbar button { background: transparent; border: 0; color: var(--pr-accent); cursor: pointer; font: inherit; }
.pr-snackbar .pr-snack-close { color: inherit; opacity: .7; font-size: 16px; }
.pr-banner-host { position: absolute; left: 0; right: 0; top: 0; z-index: 60; }
.pr-error-banner { background: var(--pr-danger); color: #fff; padding: 6px 12px; display: flex; justify-content: space-between; gap: 12px; }
.pr-error-banner button { background: transparent; color: inherit; border: 1px solid currentColor; border-radius: 4px; cursor: pointer; font: inherit; margin-left: 6px; }
.pr-net-host { position: absolute; right: 8px; top: 4px; z-index: 55; }
.pr-ended { position: fixed; inset: 0; z-index: 70; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; padding: 16px; text-align: center; background: var(--pr-surface); color: var(--pr-text); font-family: var(--pr-font); }
.pr-ended .pr-ended-title { margin: 0; font-size: 20px; font-weight: 600; } .pr-ended .pr-ended-hint { margin: 0; color: var(--pr-text-dim); }
.pr-ended button { margin-top: 6px; padding: 6px 18px; background: var(--pr-accent); border: 1px solid var(--pr-accent); color: #fff; border-radius: 4px; cursor: pointer; font: inherit; }
.pr-net { font-size: 11px; padding: 1px 8px; border-radius: 10px; background: #f5a623; color: #000; }
"#;

/// The base stylesheet as the player adopts it: in the lowest cascade layer, so that the property
/// layer (`pr-props`) and every unlayered rule the developer writes come after it.
pub fn base_layer() -> String {
    format!("@layer pr-base {{\n{BASE_CSS}\n}}")
}

fn clean_name(form: &str) -> String {
    form.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect()
}

/// The wrapper for a form's own stylesheet (WASM-007 R9): it applies inside that form's root only.
pub fn scoped(form: &str, css: &str) -> String {
    format!("@scope ([data-pr-form=\"{}\"]) {{\n{css}\n}}", clean_name(form))
}

/// The same scoping by CSS nesting, for a browser that has nesting but not `@scope`.
pub fn nested(form: &str, css: &str) -> String {
    format!("[data-pr-form=\"{}\"] {{\n{css}\n}}", clean_name(form))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_form_stylesheet_is_wrapped_in_a_scope_of_its_own_root() {
        let s = scoped("main-form", "#BTN-OK { color: red }");
        assert!(s.starts_with("@scope ([data-pr-form=\"main-form\"]) {"));
        assert!(s.contains("#BTN-OK { color: red }"));
        // A name with a quote cannot break out of the attribute selector.
        assert!(!scoped("a\"] { } x[\"", "").contains("\"] {"));
        assert!(nested("main-form", "a { b: c }").starts_with("[data-pr-form=\"main-form\"] {"));
    }

    #[test]
    fn the_base_sheet_is_in_the_lowest_layer() {
        let b = base_layer();
        assert!(b.starts_with("@layer pr-base {"));
        assert_eq!(b.matches('{').count(), b.matches('}').count(), "balanced braces");
    }

    #[test]
    fn the_public_class_contract_is_in_the_base_sheet() {
        // The class of a control is `pr-` + its type name in lower case (css::type_class).
        for class in [
            "pr-button", "pr-textbox", "pr-label", "pr-listbox", "pr-combobox", "pr-groupbox", "pr-panel", "pr-tabcontrol", "pr-weblink",
            "pr-datagrid", "pr-treeview", "pr-dialog", "pr-snackbar", "pr-viewer", "pr-maps", "pr-menubar", "pr-sidemenu", "pr-toolbar",
            "pr-statusbar", "pr-splitter", "pr-progressbar", "pr-slider", "pr-switch", "pr-filedropzone", "pr-knob", "pr-gauge",
        ] {
            assert!(BASE_CSS.contains(&format!(".{class}")), "{class}");
        }
        for var in ["--pr-accent", "--pr-surface", "--pr-text", "--pr-border", "--pr-radius", "--pr-font"] {
            assert!(BASE_CSS.contains(var), "{var}");
        }
    }

    #[test]
    fn the_base_sheet_loads_nothing_from_outside() {
        let lower = BASE_CSS.to_ascii_lowercase();
        for bad in ["url(", "@import", "expression(", "javascript:"] {
            assert!(!lower.contains(bad), "{bad}");
        }
    }
}
