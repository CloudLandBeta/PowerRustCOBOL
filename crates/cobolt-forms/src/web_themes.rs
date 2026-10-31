// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The web themes** (WASM-007 Q5, R4, R9): what a web or mobile form looks like before its properties
//! and its stylesheets say anything. Three themes, named as the desktop's (`theme::LIQUID_GLASS`,
//! `ELEGANCE`, `SPATIAL`) but designed for the web; each is **one complete stylesheet** in
//! `@layer pr-theme` — the published custom properties ([`CUSTOM_PROPERTIES`]) and the look of every
//! control type — replaceable as a whole, with no sub-themes (a form's `glass_style` does not apply on
//! the web). The player adopts one as its theme sheet; the IDE writes a new form's stylesheet from one
//! ([`form_stylesheet`]); the Build copies one into the bundle. Rust constants, no `.css` source.
//!
//! The player's structure (`pr-base`) holds what makes a control work — layout, the clear native
//! inputs inside a drawn frame, `[hidden]` — and repeats the theme's resets and its default custom
//! properties, so a whole-theme file that omits one still leaves a working page.

use crate::model::ControlType;
use crate::theme::{resolve_theme_id, ELEGANCE, LIQUID_GLASS, SPATIAL};

/// The custom properties every web theme defines and a host page may set on the mount element
/// (WASM-007 R9). Sorted. A proposal pinned by test until the operator settles WASM-007 Q4.
pub const CUSTOM_PROPERTIES: &[&str] = &[
    "--pr-accent",
    "--pr-border",
    "--pr-danger",
    "--pr-field",
    "--pr-field-border",
    "--pr-field-text",
    "--pr-focus",
    "--pr-font",
    "--pr-radius",
    "--pr-surface",
    "--pr-surface-2",
    "--pr-text",
    "--pr-text-dim",
];

/// The look of every control type, drawn from the custom properties: shared by the three themes,
/// which differ in their properties and in the rules each adds after it.
const LOOK: &str = r#"
.pr-root { color: var(--pr-text); }
.pr-form { background: var(--pr-surface); color: var(--pr-text); }
:focus-visible { outline: 2px solid var(--pr-focus); outline-offset: 1px; }
/* buttons, labels, text */ .pr-button { color: inherit; background: var(--pr-surface-2); border: 1px solid var(--pr-border); border-radius: var(--pr-radius); }
.pr-button:hover { filter: brightness(1.15); }
.pr-button:active { filter: brightness(.9); }
.pr-textbox, .pr-combobox, .pr-numericupdown, .pr-datetimepicker { color: var(--pr-field-text); background: var(--pr-field); border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-textbox input, .pr-textbox textarea, .pr-combobox input, .pr-numericupdown input, .pr-datetimepicker input { color: inherit; background: transparent; border: 0; outline: none; }
.pr-textbox input::placeholder, .pr-textbox textarea::placeholder { color: inherit; }
.pr-combobox select { color: inherit; background: transparent; border: 0; outline: none; }
.pr-combobox select option { color: #000; }
.pr-switch { background: #555a64; border: 0; border-radius: 999px; }
.pr-switch .pr-thumb { border-radius: 50%; background: #fff; }
.pr-switch[aria-checked="true"] { background: var(--pr-accent); }
.pr-hyperlink { text-decoration: none; color: var(--pr-link, #6aa6ff); }
.pr-hyperlink:hover { color: var(--pr-hover, var(--pr-link, #9cc3ff)); }
.pr-hyperlink.pr-underline-always { text-decoration: underline; }
.pr-hyperlink.pr-underline-hover:hover { text-decoration: underline; }
.pr-hyperlink:visited { color: var(--pr-visited, #b58cff); }
.pr-hyperlink:active { color: var(--pr-active, #ff7a7a); }
/* containers */ .pr-groupbox { border: 1px solid var(--pr-border); border-radius: var(--pr-radius); }
/* The expand / collapse icon of a card that is Expandable (spec 090): a 24 px square in its top-right corner, as the painter lays it. */ .pr-expander { border: 0; border-radius: 6px; background: transparent; color: inherit; }
.pr-expander:hover { background: color-mix(in srgb, currentColor 16%, transparent); }
.pr-tab { background: var(--pr-tab-bg, transparent); color: var(--pr-tab-fg, inherit); border: 1px solid var(--pr-border); border-radius: 4px 4px 0 0; }
.pr-tabs-left > .pr-tablist > .pr-tab, .pr-tabs-right > .pr-tablist > .pr-tab { border-radius: 4px 0 0 4px; }
.pr-tabs-bottom > .pr-tablist > .pr-tab { border-radius: 0 0 4px 4px; }
.pr-tab[aria-selected="true"] { background: var(--pr-accent); color: var(--pr-tab-active-fg, #fff); border-color: var(--pr-accent); }
.pr-splitter > .pr-handle { background: var(--pr-line, var(--pr-border)); }
/* lists, trees, grids */ /* A ListBox is drawn the way the desktop draws it (view/controls.rs): a frame, a scroll area 3 px inside it with rows of a size the painter works out, and a pill-shaped handle on the right while the rows do not fit. */ .pr-listbox { background: var(--pr-field); color: var(--pr-field-text); border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-lb-scroll { outline: none; }
.pr-lb-box { border: 1px solid; }
.pr-lb-handle { border-radius: 2px; background: rgb(60 60 60 / .6); }
.pr-treeview { background: var(--pr-field); color: var(--pr-field-text); border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-node-row .pr-arrow { background: transparent; border: 0; color: inherit; }
.pr-treeview li.pr-selected > .pr-node-row { background: var(--pr-accent); color: #fff; }
/* The DataGrid is drawn the way the desktop draws it (view/datagrid.rs): a clipped frame, a sheet that scrolls natively under sticky bands, and overlays for the lines, the shadows and the bars. Sizes and places are written per element. */ .pr-datagrid { background: var(--pr-field); color: var(--pr-field-text); }
.pr-dg-ln { box-shadow: inset 1px 0 0 0 var(--pr-dg-line, rgb(27 42 78)); }
.pr-dg-lf { box-shadow: 1px 0 0 0 var(--pr-dg-line, rgb(27 42 78)); }
.pr-dg-ln.pr-dg-lf { box-shadow: inset 1px 0 0 0 var(--pr-dg-line, rgb(27 42 78)), 1px 0 0 0 var(--pr-dg-line, rgb(27 42 78)); }
.pr-dg-sel::after { background: var(--pr-dg-sel, rgb(80 145 255 / .216)); }
.pr-dg-csv { border: 1px solid; border-radius: 3px; }
.pr-dg-hl { background: var(--pr-dg-line, rgb(27 42 78)); }
.pr-dg-vl { background: var(--pr-dg-line, rgb(27 42 78)); }
.pr-dg-ol { border: 1px solid var(--pr-dg-line, rgb(27 42 78)); }
.pr-dg-sh-l { background: linear-gradient(to right, rgb(0 0 0 / .2157), rgb(0 0 0 / 0)); }
.pr-dg-sh-t { background: linear-gradient(to bottom, rgb(0 0 0 / .2157), rgb(0 0 0 / 0)); }
.pr-dg-track { border-radius: 5px; background: rgb(0 0 0 / .2157); }
.pr-dg-thumb { border-radius: 5px; background: rgb(0 0 0 / .588); }
.pr-dg-thumb-add { background: rgb(230 235 255); }
.pr-dg-asc::after, .pr-dg-desc::after { border: 3.5px solid transparent; }
.pr-dg-asc::after { border-bottom: 5px solid var(--pr-dg-head-ink, #fff); border-top: 0; }
.pr-dg-desc::after { border-top: 5px solid var(--pr-dg-head-ink, #fff); border-bottom: 0; }
.pr-progressbar progress { accent-color: var(--pr-accent); }
.pr-slider input { accent-color: var(--pr-accent); }
.pr-statusbar { background: var(--pr-surface-2); border-top: 1px solid var(--pr-border); }
.pr-statusbar .pr-panel-item { border-right: 1px solid var(--pr-border); }
/* menus and the shell */ .pr-menubar, .pr-toolbar { background: var(--pr-surface-2); border-bottom: 1px solid var(--pr-border); }
.pr-menu > button { background: transparent; border: 0; color: inherit; }
.pr-menu > button:hover:not(:disabled) { background: var(--pr-menu-hover, var(--pr-accent)); color: var(--pr-menu-hover-fg, #fff); }
.pr-menubar .pr-menu-items { background: var(--pr-surface-2); border: 1px solid var(--pr-border); box-shadow: 0 6px 18px rgba(0,0,0,.35); }
.pr-menu-items hr { border: 0; border-top: 1px solid var(--pr-border); }
.pr-badge { border-radius: 999px; background: var(--pr-accent); color: #fff; }
.pr-badge-outline { background: transparent; border: 1px solid var(--pr-accent); color: var(--pr-accent); }
.pr-sidemenu { background: var(--pr-surface-2); border-right: 1px solid var(--pr-border); }
.pr-sm-toggle { background: transparent; border: 0; color: inherit; }
.pr-sidemenu .pr-menu > button[aria-current="page"] { background: var(--pr-accent); color: var(--pr-menu-selected-fg, #fff); }
.pr-breadcrumb { background: var(--pr-surface-2); border-bottom: 1px solid var(--pr-border); color: var(--pr-text-dim); }
.pr-breadcrumb button { background: transparent; border: 0; color: var(--pr-accent); }
.pr-content { background: var(--pr-surface); }
.pr-toolbar button { background: transparent; border: 1px solid transparent; border-radius: 4px; color: inherit; }
.pr-toolbar button:hover:not(:disabled) { background: rgba(255,255,255,.1); border-color: var(--pr-border); }
.pr-maps { background: #1c1f24; }
.pr-marker { border: 2px solid #fff; border-radius: 50% 50% 50% 0; background: var(--pr-marker, #e5484d); }
.pr-info { background: var(--pr-surface-2); border: 1px solid var(--pr-border); border-radius: 8px; box-shadow: 0 4px 14px rgba(0,0,0,.4); }
.pr-map-zoom button { background: var(--pr-surface-2); color: inherit; border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-attribution { background: rgba(0,0,0,.4); }
.pr-viewer { background: #1c1f24; }
.pr-viewer .pr-content { background: transparent; }
::highlight(pr-find) { background-color: #ffd54a; color: #1a1a1a; }
::highlight(pr-find-current) { background-color: #ff8a00; color: #000; }
.pr-findbar { background: var(--pr-surface-2); border-bottom: 1px solid var(--pr-border); }
.pr-findbar input { color: var(--pr-field-text); background: var(--pr-field); border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-findbar button { color: inherit; background: transparent; border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-findbar button[aria-pressed="true"] { background: var(--pr-accent); color: #fff; }
.pr-viewer .pr-mermaid { background: #fff; border-radius: 4px; }
.pr-viewer pre { background: rgba(255,255,255,.05); border-radius: 4px; }
.pr-viewer table { border-collapse: collapse; }
.pr-viewer td, .pr-viewer th { border: 1px solid var(--pr-border); }
.pr-viewer blockquote { border-left: 3px solid var(--pr-border); color: var(--pr-text-dim); }
.pr-viewer .pr-message.pr-user { border-radius: 12px; }
.pr-viewer .pr-message.pr-agent { border-radius: 12px; }
.pr-viewer a { color: var(--pr-link, #6aa6ff); }
.pr-viewer .pr-error, .pr-error { color: var(--pr-danger); }
.pr-filedropzone { border: 2px dashed var(--pr-border); border-radius: var(--pr-radius); color: var(--pr-text-dim); }
.pr-filedropzone.pr-over { border-color: var(--pr-accent); color: var(--pr-text); }
.pr-authentication, .pr-auth { background: var(--pr-surface-2); border: 1px solid var(--pr-border); border-radius: var(--pr-radius); }
.pr-auth input { color: var(--pr-field-text); background: var(--pr-field); border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-auth .pr-auth-message { color: var(--pr-danger); }
.pr-auth nav button { background: transparent; border: 0; color: var(--pr-accent); }
.pr-auth button.pr-auth-primary, .pr-auth .pr-auth-signed-in > button { background: var(--pr-accent); color: #fff; border: 0; border-radius: 4px; }
.pr-auth input:focus-visible, .pr-auth button:focus-visible { outline: 2px solid var(--pr-accent); outline-offset: 2px; }
.pr-dialog { border: 1px solid var(--pr-border); border-radius: var(--pr-radius); background: var(--pr-surface); color: var(--pr-text); }
.pr-dialog::backdrop { background: rgba(0,0,0,.45); }
.pr-dialog > .pr-titlebar { background: var(--pr-surface-2); border-bottom: 1px solid var(--pr-border); }
.pr-titlebar button { background: transparent; border: 0; color: inherit; }
.pr-dialog .pr-dialog-buttons button { background: var(--pr-surface-2); color: inherit; border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-dialog .pr-dialog-buttons button.pr-default { background: var(--pr-accent); border-color: var(--pr-accent); color: #fff; }
.pr-dialog .pr-dialog-field input { color: var(--pr-field-text); background: var(--pr-field); border: 1px solid var(--pr-border); border-radius: 4px; }
.pr-panelwin { border: 1px solid var(--pr-border); border-radius: var(--pr-radius); background: var(--pr-surface); box-shadow: 0 6px 24px rgba(0,0,0,.4); }
.pr-panelwin > .pr-titlebar { background: var(--pr-surface-2); }
.pr-snackbar { background: #202329; color: #fff; border-radius: var(--pr-radius); border-left: 4px solid var(--pr-accent); box-shadow: 0 4px 16px rgba(0,0,0,.4); }
.pr-snackbar[data-category="Error"], .pr-snackbar[data-category="Critical"] { border-left-color: var(--pr-danger); }
.pr-snackbar[data-category="Warning"] { border-left-color: #f5a623; }
.pr-snackbar[data-category="Question"] { border-left-color: #8e4ec6; }
.pr-snackbar button { background: transparent; border: 0; color: var(--pr-accent); }
.pr-snackbar .pr-snack-close { color: inherit; }
.pr-error-banner { background: var(--pr-danger); color: #fff; }
.pr-error-banner button { background: transparent; color: inherit; border: 1px solid currentColor; border-radius: 4px; }
.pr-ended { background: var(--pr-surface); color: var(--pr-text); }
.pr-ended .pr-ended-hint { color: var(--pr-text-dim); }
.pr-ended button { background: var(--pr-accent); border: 1px solid var(--pr-accent); color: #fff; border-radius: 4px; }
.pr-net { border-radius: 10px; background: #f5a623; color: #000; }
"#;

const LIQUID_GLASS_TOKENS: &str = r#":host { --pr-accent: #4e8cff; --pr-surface: #2e3138; --pr-surface-2: #3a3e47; --pr-text: #ffffff; --pr-text-dim: #b8bcc6; --pr-border: #565b66; --pr-radius: 6px; --pr-danger: #e5484d; --pr-font: "PR Ubuntu Light", system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; --pr-focus: #8ab4ff; --pr-field: #f4f5f7; --pr-field-text: #14161a; --pr-field-border: #aab0ba; }"#;

/// Elegance on the web: flat slate surfaces, a cool blue for buttons and a green for what is on.
const ELEGANCE_TOKENS: &str = r#":host { --pr-accent: #3761e2; --pr-surface: #0f172a; --pr-surface-2: #20293a; --pr-text: #ffffff; --pr-text-dim: #8691a3; --pr-border: #8691a3; --pr-radius: 5px; --pr-danger: #e5484d; --pr-font: "PR Ubuntu Light", system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; --pr-focus: #8ab4ff; --pr-field: #20293a; --pr-field-text: #ffffff; --pr-field-border: #8691a3; }"#;

const ELEGANCE_RULES: &str = r#"
.pr-button { background: var(--pr-accent); border-color: var(--pr-accent); color: #ffffff; }
.pr-switch[aria-checked="true"] { background: #4ca053; }
.pr-checkbox input, .pr-radiobutton input { accent-color: #4ca053; }
.pr-groupbox, .pr-panel { background: var(--pr-surface-2); }
"#;

/// Spatial on the web: warm grey translucent glass, white text, a faint white rim, large radii.
const SPATIAL_TOKENS: &str = r#":host { --pr-accent: #4e8cff; --pr-surface: rgb(66 65 62 / 0.588); --pr-surface-2: rgb(255 255 255 / 0.102); --pr-text: #ffffff; --pr-text-dim: rgb(255 255 255 / 0.7); --pr-border: rgb(255 255 255 / 0.18); --pr-radius: 14px; --pr-danger: #e5484d; --pr-font: "PR Ubuntu Light", system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; --pr-focus: #8ab4ff; --pr-field: rgb(0 0 0 / 0.18); --pr-field-text: #ffffff; --pr-field-border: rgb(255 255 255 / 0.18); }"#;

const SPATIAL_RULES: &str = r#"
.pr-button { background: rgb(255 255 255 / 0.157); }
.pr-textbox, .pr-combobox, .pr-numericupdown, .pr-datetimepicker, .pr-listbox, .pr-treeview { border-radius: var(--pr-radius); }
.pr-groupbox, .pr-panel { border-radius: 24px; background: var(--pr-surface-2); }
"#;

/// The built-in web themes, by id.
pub const THEMES: [&str; 3] = [LIQUID_GLASS, ELEGANCE, SPATIAL];

/// The three parts of a theme: its custom properties, the shared look, its own rules.
fn parts(id: &str) -> (&'static str, &'static str) {
    match id {
        ELEGANCE => (ELEGANCE_TOKENS, ELEGANCE_RULES),
        SPATIAL => (SPATIAL_TOKENS, SPATIAL_RULES),
        _ => (LIQUID_GLASS_TOKENS, ""),
    }
}

/// A theme's rules without the layer around them. An id that is not a built-in web theme (an asset
/// pack of the desktop) is Liquid Glass, as on the desktop.
pub fn theme_rules(id: &str) -> String {
    let (tokens, own) = parts(id);
    format!("{tokens}\n{}{own}", LOOK.trim_start())
}

/// A theme as the player adopts it: in `@layer pr-theme`.
pub fn theme_css(id: &str) -> String {
    format!("@layer pr-theme {{\n{}}}\n", theme_rules(id))
}

/// The theme of a form, resolved as on the desktop: the form's, else the project's, else Liquid Glass.
pub fn theme_for(form: Option<&str>, project: Option<&str>) -> String {
    theme_css(&resolve_theme_id(form, project))
}

/// The class a control type's element carries (`pr-button`), as the player names it.
pub fn type_class(t: &ControlType) -> String {
    if let Some(k) = crate::web_class::player_kind_of(t) {
        return format!("pr-{}", k.name().to_ascii_lowercase());
    }
    format!("pr-{}", t.as_str().to_ascii_lowercase().replace(':', "-"))
}

/// Does a selector name the class `class` (not a longer class that starts with it)?
fn names_class(selector: &str, class: &str) -> bool {
    selector.match_indices(&format!(".{class}")).any(|(i, m)| {
        let after = selector[i + m.len()..].chars().next();
        !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    })
}

const BEGIN: &str = "/* pr-theme block: begin";
const END: &str = "/* pr-theme block: end */";

/// FNV-1a, 64 bits: a hash with no dependency, enough to see that the block was edited.
fn hash(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The theme's rules for the control types a form uses, one per line, in the theme's order.
fn block_rules(id: &str, used: &[ControlType]) -> String {
    let classes: Vec<String> = used.iter().map(type_class).collect();
    let mut out = String::new();
    for rule in theme_rules(id).lines().filter(|l| !l.trim().is_empty()) {
        let selector = rule.split('{').next().unwrap_or("");
        if names_class(selector, "pr-form") || classes.iter().any(|c| names_class(selector, c)) {
            out.push_str("  ");
            out.push_str(rule.trim());
            out.push('\n');
        }
    }
    out
}

/// The first content of a web or mobile form's stylesheet (WASM-007 R4): a banner, the theme's rules
/// for the types the form uses inside `@layer pr-theme { … }` between two markers — the first carries
/// a hash of the block — and an empty unlayered section for the developer. The block reproduces the
/// theme's look and stays under the properties; what the developer writes after it wins over both.
pub fn form_stylesheet(theme: &str, used: &[ControlType]) -> String {
    let id = resolve_theme_id(Some(theme), None);
    let block = format!("@layer pr-theme {{\n{}}}\n", block_rules(&id, used));
    format!(
        "/* The stylesheet of this form, generated from the web theme \"{id}\". It is yours to edit.\n   The block between the two markers is the theme's look for the controls of this form, in the theme's layer:\n   the Properties panel's values win over it. A rule you write after the end marker wins over both. */\n{BEGIN} {} */\n{block}{END}\n\n/* Your rules. */\n",
        hash(&block)
    )
}

/// The block between the markers and the hash its begin marker carries.
fn generated_block(text: &str) -> Option<(&str, &str)> {
    let b = text.find(BEGIN)?;
    let line_end = text[b..].find('\n')? + b;
    let stated = text[b + BEGIN.len()..line_end].trim().strip_suffix("*/")?.trim();
    let e = text[line_end..].find(END)? + line_end;
    Some((&text[line_end + 1..e], stated))
}

/// Is the generated block exactly as it was written (WASM-007 R4, R6)? A theme switch replaces it only
/// then; otherwise the IDE asks. A sheet without the markers is never "untouched".
pub fn generated_block_untouched(text: &str) -> bool {
    generated_block(text).is_some_and(|(block, stated)| hash(block) == stated)
}

/// The sheet with its generated block replaced by `theme`'s for `used`, the rest byte for byte; `None`
/// when the sheet has no markers.
pub fn replace_generated_block(text: &str, theme: &str, used: &[ControlType]) -> Option<String> {
    let b = text.find(BEGIN)?;
    let e = text[b..].find(END)? + b + END.len();
    let fresh = form_stylesheet(theme, used);
    let fb = fresh.find(BEGIN)?;
    let fe = fresh[fb..].find(END)? + fb + END.len();
    Some(format!("{}{}{}", &text[..b], &fresh[fb..fe], &text[e..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stylesheet_lint::{has_errors, lint, message, Options};
    use std::collections::BTreeSet;

    /// `--pr-…` names in `css`: those defined (`--pr-x:`) and those read with no fallback (`var(--pr-x)`).
    fn custom_properties(css: &str) -> (BTreeSet<String>, BTreeSet<String>) {
        let (mut defined, mut used) = (BTreeSet::new(), BTreeSet::new());
        let bytes = css.as_bytes();
        let mut i = 0;
        while let Some(at) = css[i..].find("--pr-") {
            let start = i + at;
            let mut end = start + 5;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
                end += 1;
            }
            let name = css[start..end].to_owned();
            let after = css[end..].trim_start();
            if css[..start].ends_with("var(") {
                if !after.starts_with(',') {
                    used.insert(name);
                }
            } else if after.starts_with(':') {
                defined.insert(name);
            }
            i = end;
        }
        (defined, used)
    }

    #[test]
    fn each_theme_defines_every_published_custom_property_and_reads_only_defined_ones() {
        for id in THEMES {
            let css = theme_css(id);
            let (defined, used) = custom_properties(&css);
            let missing: Vec<&&str> = CUSTOM_PROPERTIES.iter().filter(|p| !defined.contains(**p)).collect();
            assert!(missing.is_empty(), "{id}: published custom properties it does not define: {missing:?}");
            let undefined: Vec<&String> = used.iter().filter(|u| !defined.contains(*u)).collect();
            assert!(undefined.is_empty(), "{id}: read with no fallback and never defined: {undefined:?}");
            assert!(css.starts_with("@layer pr-theme {"), "{id}");
            let d = lint(&css, &Options::default());
            assert!(!has_errors(&d), "{id}: {:?}", d.iter().map(|d| format!("{}: {}", d.line, message(d))).collect::<Vec<_>>());
            println!("{id}: {} bytes, {} custom properties defined, {} read, {} findings ({} warnings)", css.len(), defined.len(), used.len(), d.len(), d.len());
        }
        assert_ne!(theme_css(ELEGANCE), theme_css(LIQUID_GLASS));
        assert_ne!(theme_css(SPATIAL), theme_css(LIQUID_GLASS));
        assert_eq!(theme_css("some-asset-pack"), theme_css(LIQUID_GLASS), "an id that is not a web theme is Liquid Glass");
        assert_eq!(theme_for(None, Some(" ")), theme_css(LIQUID_GLASS));
        assert_eq!(theme_for(None, Some(SPATIAL)), theme_css(SPATIAL));
    }

    #[test]
    fn a_new_forms_stylesheet_has_the_themes_rules_for_its_types_and_passes_the_check() {
        let used = [ControlType::Button, ControlType::TextBox, ControlType::Label];
        for id in THEMES {
            let s = form_stylesheet(id, &used);
            assert!(s.starts_with("/* The stylesheet of this form, generated from the web theme"), "{s}");
            assert!(s.contains(&format!("\"{id}\"")));
            assert!(s.contains(".pr-button {") && s.contains(".pr-textbox"), "{s}");
            assert!(!s.contains(".pr-datagrid") && !s.contains(".pr-tab {"), "only the types the form uses: {s}");
            assert!(s.trim_end().ends_with("/* Your rules. */"));
            let d = lint(&s, &Options::default());
            assert!(!has_errors(&d), "{id}: {:?}", d.iter().map(|d| format!("{}: {}", d.line, message(d))).collect::<Vec<_>>());
            assert!(generated_block_untouched(&s), "{id}: a fresh sheet is untouched");
            println!("{id}: a form of Button, TextBox, Label gets {} lines, {} findings", s.lines().count(), d.len());
        }
        assert!(!names_class(".pr-buttonx", "pr-button") && names_class(".a, .pr-button:hover", "pr-button"));
    }

    #[test]
    fn editing_one_character_of_the_block_flips_untouched_and_the_developer_section_does_not() {
        let s = form_stylesheet(LIQUID_GLASS, &[ControlType::Button]);
        let i = s.find(".pr-button {").unwrap() + 2;
        let mut edited = s.clone();
        edited.replace_range(i..i + 1, "X");
        assert!(!generated_block_untouched(&edited));
        let mut mine = s.clone();
        mine.push_str("#BTN-1 { color: red }\r\n");
        assert!(generated_block_untouched(&mine), "the developer's section is not the block");
        assert!(!generated_block_untouched("a { b: c }"), "no markers, never untouched");
        // Replacing the block keeps the developer's bytes, CRLF included.
        let swapped = replace_generated_block(&mine, ELEGANCE, &[ControlType::Button]).unwrap();
        assert!(swapped.ends_with("#BTN-1 { color: red }\r\n"), "the developer's section is byte for byte");
        assert!(swapped.contains(".pr-button { background: var(--pr-accent); border-color: var(--pr-accent); color: #ffffff; }"), "Elegance's own button rule: {swapped}");
        assert!(generated_block_untouched(&swapped));
        assert!(swapped.starts_with(&mine[..mine.find(BEGIN).unwrap()]), "the banner is the developer's text now");
    }
}
