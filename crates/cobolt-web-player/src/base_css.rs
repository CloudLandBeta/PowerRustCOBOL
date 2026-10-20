// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The player's base stylesheet: the lowest layer of the cascade (WASM-007 R8 — "the theme's
//! defaults"). It gives every control type a plain, readable default look; everything a form's
//! properties say comes after it, and everything the developer writes comes after that.
//!
//! The class names (`pr-button`, `pr-textbox`, …) and the custom properties (`--pr-accent`, …) are a
//! public contract once shipped (WASM-007 Q4); `tests/css_contract.rs` pins them. A host page sets the
//! custom properties on the mount element and they inherit into the shadow root (WASM-007 R9).

pub const BASE_CSS: &str = r#"
:host { display: block; contain: layout; }
*, *::before, *::after { box-sizing: border-box; }
:host {
  --pr-accent: #4e8cff; --pr-surface: #2e3138; --pr-surface-2: #3a3e47; --pr-text: #ffffff;
  --pr-text-dim: #b8bcc6; --pr-border: #565b66; --pr-radius: 6px; --pr-danger: #e5484d;
  --pr-font: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; --pr-focus: #8ab4ff;
}
.pr-root { width: 100%; height: 100%; position: relative; color: var(--pr-text); font: 13px/1.35 var(--pr-font); overflow: hidden; }
[hidden] { display: none !important; }
.pr-form { position: absolute; inset: 0; overflow: auto; background: var(--pr-surface); color: var(--pr-text); }
.pr-surface { position: relative; }
.pr-control { font-family: inherit; }
.pr-disabled { opacity: .55; pointer-events: none; }
:focus-visible { outline: 2px solid var(--pr-focus); outline-offset: 1px; }

.pr-button, .pr-togglebutton { cursor: pointer; font: inherit; color: inherit; background: var(--pr-surface-2);
  border: 1px solid var(--pr-border); border-radius: var(--pr-radius); padding: 2px 10px; }
.pr-button:hover { filter: brightness(1.15); } .pr-button:active { filter: brightness(.9); }
.pr-label { display: flex; align-items: center; white-space: pre-wrap; overflow: hidden; }
.pr-textbox, .pr-combobox, .pr-numericupdown, .pr-datetimepicker { font: inherit; color: inherit; background: #23262c;
  border: 1px solid var(--pr-border); border-radius: 4px; padding: 2px 6px; min-width: 0; }
.pr-textbox textarea, .pr-textbox input { font: inherit; color: inherit; background: transparent; border: 0; width: 100%; height: 100%; resize: none; outline: none; }
.pr-checkbox, .pr-radiobutton, .pr-switch { display: flex; align-items: center; gap: .5em; cursor: pointer; }
.pr-groupbox { border: 1px solid var(--pr-border); border-radius: var(--pr-radius); padding: 0; margin: 0; min-width: 0; }
.pr-groupbox > legend { padding: 0 .4em; }
.pr-panel { overflow: hidden; }
.pr-listbox { list-style: none; margin: 0; padding: 2px; overflow: auto; background: #23262c; border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-listbox li { padding: 2px 6px; cursor: default; white-space: nowrap; }
.pr-listbox li[aria-selected="true"], .pr-tree [aria-selected="true"] > .pr-node-row { background: var(--pr-accent); color: #fff; }
.pr-progressbar { width: 100%; height: 100%; accent-color: var(--pr-accent); }
.pr-slider input { width: 100%; height: 100%; accent-color: var(--pr-accent); margin: 0; }
.pr-statusbar { display: flex; align-items: center; padding: 0 8px; background: var(--pr-surface-2); border-top: 1px solid var(--pr-border); }
.pr-line { display: block; }
.pr-picturebox, .pr-animator { overflow: hidden; display: flex; align-items: center; justify-content: center; }
.pr-picturebox img, .pr-animator img { max-width: 100%; max-height: 100%; }

.pr-tabs { display: flex; flex-direction: column; }
.pr-tablist { display: flex; gap: 2px; border-bottom: 1px solid var(--pr-border); }
.pr-tab { padding: 4px 12px; cursor: pointer; background: transparent; color: inherit; border: 1px solid transparent; border-bottom: 0; border-radius: 4px 4px 0 0; font: inherit; }
.pr-tab[aria-selected="true"] { background: var(--pr-surface-2); border-color: var(--pr-border); }
.pr-tabpanel { position: relative; flex: 1; }
.pr-splitter { display: flex; } .pr-splitter > .pr-handle { flex: 0 0 6px; background: var(--pr-border); cursor: col-resize; }
.pr-splitter.pr-vertical { flex-direction: column; } .pr-splitter.pr-vertical > .pr-handle { cursor: row-resize; }
.pr-splitter > .pr-pane { position: relative; overflow: hidden; }

.pr-menubar, .pr-toolbar { display: flex; align-items: center; gap: 2px; background: var(--pr-surface-2); border-bottom: 1px solid var(--pr-border); padding: 0 4px; }
.pr-menubar > .pr-menu, .pr-toolbar > button { position: relative; padding: 3px 10px; background: transparent; border: 0; color: inherit; font: inherit; cursor: pointer; }
.pr-menu > .pr-menu-items { position: absolute; left: 0; top: 100%; min-width: 10em; background: var(--pr-surface-2); border: 1px solid var(--pr-border); z-index: 50; }
.pr-menu-items button { display: block; width: 100%; text-align: left; padding: 4px 12px; background: transparent; color: inherit; border: 0; font: inherit; cursor: pointer; }
.pr-menu-items button:hover { background: var(--pr-accent); }
.pr-sidemenu { display: flex; flex-direction: column; background: var(--pr-surface-2); border-right: 1px solid var(--pr-border); overflow: auto; }
.pr-sidemenu button { text-align: left; padding: 8px 14px; background: transparent; color: inherit; border: 0; font: inherit; cursor: pointer; }
.pr-sidemenu button[aria-current="page"] { background: var(--pr-accent); }
.pr-shell { position: absolute; inset: 0; display: flex; }
.pr-shell > .pr-pane { flex: 1; position: relative; overflow: auto; display: flex; flex-direction: column; }
.pr-breadcrumb { padding: 4px 12px; border-bottom: 1px solid var(--pr-border); color: var(--pr-text-dim); }
.pr-content { position: relative; flex: 1; }

.pr-tree { list-style: none; margin: 0; padding: 0 0 0 0; overflow: auto; background: #23262c; border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-tree ul { list-style: none; margin: 0; padding-left: 16px; }
.pr-node-row { padding: 1px 4px; cursor: default; display: flex; gap: 4px; }
.pr-grid { overflow: auto; background: #23262c; border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-grid table { border-collapse: collapse; width: 100%; }
.pr-grid th { position: sticky; top: 0; background: var(--pr-surface-2); text-align: left; padding: 3px 8px; border-bottom: 1px solid var(--pr-border); cursor: pointer; user-select: none; }
.pr-grid td { padding: 2px 8px; border-bottom: 1px solid rgba(255,255,255,.06); white-space: nowrap; }
.pr-grid tr[aria-selected="true"] td { background: var(--pr-accent); color: #fff; }
.pr-weblink { cursor: pointer; text-decoration: none; color: var(--pr-link, #6aa6ff); display: inline-flex; align-items: center; }
.pr-weblink.pr-underline-always { text-decoration: underline; } .pr-weblink.pr-underline-hover:hover { text-decoration: underline; }
.pr-weblink:visited { color: var(--pr-visited, #b58cff); } .pr-weblink:active { color: var(--pr-active, #ff7a7a); }
.pr-chart, .pr-gauge, .pr-knob { display: block; width: 100%; height: 100%; }
.pr-chart text, .pr-gauge text, .pr-knob text { fill: var(--pr-text); font: 11px var(--pr-font); }
.pr-maps { position: relative; overflow: hidden; background: #1c1f24; }
.pr-maps img { position: absolute; user-select: none; -webkit-user-drag: none; }
.pr-viewer { overflow: auto; background: #1c1f24; padding: 8px; user-select: text; }
.pr-viewer pre, .pr-viewer code { font-family: ui-monospace, Menlo, Consolas, monospace; }
.pr-viewer table { border-collapse: collapse; } .pr-viewer td, .pr-viewer th { border: 1px solid var(--pr-border); padding: 2px 6px; }
.pr-dropzone { display: flex; align-items: center; justify-content: center; border: 2px dashed var(--pr-border); border-radius: var(--pr-radius); color: var(--pr-text-dim); cursor: pointer; }
.pr-dropzone.pr-over { border-color: var(--pr-accent); color: var(--pr-text); }
.pr-canvas2d canvas, .pr-canvas3d canvas { width: 100%; height: 100%; display: block; }

.pr-auth { display: flex; flex-direction: column; gap: 8px; padding: 12px; background: var(--pr-surface-2); border: 1px solid var(--pr-border); border-radius: var(--pr-radius); overflow: auto; }
.pr-auth label { display: flex; flex-direction: column; gap: 2px; } .pr-auth input { font: inherit; color: inherit; background: #23262c; border: 1px solid var(--pr-border); border-radius: 4px; padding: 4px 6px; }
.pr-auth .pr-auth-message { color: var(--pr-danger); min-height: 1.35em; } .pr-auth nav { display: flex; gap: 8px; flex-wrap: wrap; } .pr-auth nav button { background: transparent; border: 0; color: var(--pr-accent); cursor: pointer; font: inherit; padding: 0; }

.pr-dialog { padding: 0; border: 1px solid var(--pr-border); border-radius: var(--pr-radius); background: var(--pr-surface); color: var(--pr-text); max-width: 100vw; max-height: 100vh; }
.pr-dialog::backdrop { background: rgba(0,0,0,.45); }
.pr-dialog > .pr-titlebar { display: flex; align-items: center; justify-content: space-between; padding: 6px 10px; background: var(--pr-surface-2); border-bottom: 1px solid var(--pr-border); }
.pr-dialog > .pr-titlebar button { background: transparent; border: 0; color: inherit; font-size: 16px; cursor: pointer; }
.pr-dialog > .pr-body { position: relative; }
.pr-dialog .pr-dialog-buttons { display: flex; justify-content: flex-end; gap: 8px; padding: 8px 12px; }
.pr-dialog .pr-dialog-text { padding: 14px 16px; white-space: pre-wrap; max-width: 36em; }
.pr-panelwin { position: absolute; z-index: 20; border: 1px solid var(--pr-border); border-radius: var(--pr-radius); background: var(--pr-surface); box-shadow: 0 6px 24px rgba(0,0,0,.4); display: flex; flex-direction: column; }
.pr-panelwin > .pr-titlebar { display: flex; justify-content: space-between; align-items: center; padding: 4px 8px; background: var(--pr-surface-2); cursor: move; }
.pr-panelwin > .pr-body { position: relative; flex: 1; overflow: auto; }
.pr-snackbars { position: absolute; left: 12px; bottom: 12px; display: flex; flex-direction: column; gap: 8px; z-index: 40; pointer-events: none; }
.pr-snackbar { pointer-events: auto; display: flex; align-items: center; gap: 12px; padding: 8px 12px; background: #202329; color: #fff; border-radius: var(--pr-radius); border-left: 4px solid var(--pr-accent); box-shadow: 0 4px 16px rgba(0,0,0,.4); }
.pr-snackbar[data-category="Error"] { border-left-color: var(--pr-danger); } .pr-snackbar[data-category="Warning"] { border-left-color: #f5a623; } .pr-snackbar[data-category="Success"] { border-left-color: #30a46c; }
.pr-snackbar button { background: transparent; border: 0; color: var(--pr-accent); cursor: pointer; font: inherit; }
.pr-error-banner { position: absolute; left: 0; right: 0; top: 0; z-index: 60; background: var(--pr-danger); color: #fff; padding: 6px 12px; display: flex; justify-content: space-between; gap: 12px; }
.pr-error-banner button { background: transparent; color: inherit; border: 1px solid currentColor; border-radius: 4px; cursor: pointer; font: inherit; }
.pr-net { position: absolute; right: 8px; top: 4px; z-index: 55; font-size: 11px; padding: 1px 8px; border-radius: 10px; background: #f5a623; color: #000; }
"#;

/// The wrapper for a form's own stylesheet (WASM-007 R9): it applies inside that form's root only.
pub fn scoped(form: &str, css: &str) -> String {
    let name: String = form.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    format!("@scope ([data-pr-form=\"{name}\"]) {{\n{css}\n}}")
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
    }

    #[test]
    fn the_public_class_contract_is_in_the_base_sheet() {
        for class in ["pr-button", "pr-textbox", "pr-label", "pr-listbox", "pr-groupbox", "pr-panel", "pr-weblink", "pr-grid", "pr-tree", "pr-dialog", "pr-snackbar", "pr-viewer"] {
            assert!(BASE_CSS.contains(&format!(".{class}")), "{class}");
        }
        for var in ["--pr-accent", "--pr-surface", "--pr-text", "--pr-border", "--pr-radius", "--pr-font"] {
            assert!(BASE_CSS.contains(var), "{var}");
        }
    }
}
