// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The reference pack** (spec 080 R7, T4.2).
//!
//! Generated at export from what this binary carries, never stored in the
//! repository: the System KB documents (`cobolt_compiler::system_documentation`
//! — minus the two that describe the in-IDE assistant), the built-ins registry
//! (`BUILTINS`), the whole English Developer's Guide and the supported-syntax
//! document (`docs_embed`), and the `.cfrm` / `.cidx` format descriptions,
//! each with a live example serialised by the very code that writes those
//! files. So the pack cannot describe a different product than the binary
//! that exported it.

use crate::content::{RefDoc, REFERENCE_DIR};

/// The English Developer's Guide, embedded from the one file in `docs/` — so
/// `rcrun`, which does not link the IDE's documentation viewer, serves the same
/// text the IDE does.
pub const DEVELOPERS_GUIDE: &str = include_str!("../../../docs/developers-guide-en.md");
/// The supported COBOL-85 syntax reference, embedded the same way.
pub const SUPPORTED_SYNTAX: &str = include_str!("../../../docs/cobol85-supported-syntax-en.md");

/// System KB documents left out of the pack: they describe the in-IDE agent
/// mesh and its JSON designer operations, which an external agent cannot use
/// and would imitate (plan §1.7).
pub const KB_LEFT_OUT: [&str; 2] = ["agents_registry.md", "ide_functionalities.md"];

/// The System KB document → its name and title in the pack.
const KB_NAMES: [(&str, &str, &str); 6] = [
    ("form_designer_controls.md", "controls.md", "Form controls: properties, events and methods"),
    ("control_methods_reference.md", "control-methods.md", "Control methods"),
    ("rustcobol_extensions.md", "rustcobol-extensions.md", "RustCOBOL extensions and syntax"),
    ("form_layout_and_events.md", "form-layout-and-events.md", "Form layout and events"),
    ("form_themes.md", "form-themes.md", "Form themes"),
    ("project_model_and_settings.md", "project-model-and-settings.md", "Project model and settings"),
];

/// The whole pack, in the order the index lists it.
pub fn pack(version: &str) -> Vec<RefDoc> {
    let mut docs = Vec::new();
    let verbatim = |name: &str, title: &str, body: String| RefDoc { name: name.into(), title: title.into(), body };
    docs.push(verbatim("developers-guide.md", "PowerRustCOBOL Developer's Guide", DEVELOPERS_GUIDE.to_owned()));
    docs.push(verbatim("cobol85-supported-syntax.md", "Supported COBOL-85 syntax", SUPPORTED_SYNTAX.to_owned()));
    let kb = cobolt_compiler::system_documentation();
    for (source, name, title) in KB_NAMES {
        if let Some((_, text)) = kb.iter().find(|(n, _)| *n == source) {
            docs.push(verbatim(name, title, text.clone()));
        }
    }
    docs.push(RefDoc {
        name: "builtins.md".into(),
        title: "Built-ins (the COBOL object's methods)".into(),
        body: builtins_doc(),
    });
    docs.push(RefDoc {
        name: "cfrm-format.md".into(),
        title: "The .cfrm form file".into(),
        body: cfrm_doc(),
    });
    docs.push(RefDoc {
        name: "cidx-format.md".into(),
        title: "The .cidx indexed-file definition".into(),
        body: cidx_doc(),
    });
    let index = readme(version, &docs);
    docs.insert(0, RefDoc { name: "README.md".into(), title: "Reference pack".into(), body: index });
    docs
}

fn readme(version: &str, docs: &[RefDoc]) -> String {
    let mut out = format!(
        "# PowerRustCOBOL reference pack\n\n\
         Written by PowerRustCOBOL AI {version} for this project. It describes exactly what this \
         version of the product supports: **a control, property, method, event or built-in that is \
         not in this pack does not exist** — write a gap report in `docs/compiler-requests/` \
         instead of inventing it.\n\n\
         To find one name without reading a whole file, call the `kb_lookup` tool (a control, \
         property, method, event or built-in). The files are regenerated when the coding-agent kit \
         is exported again; do not edit them.\n\n\
         | File | What it holds |\n|---|---|\n"
    );
    for d in docs {
        out.push_str(&format!("| `{REFERENCE_DIR}/{}` | {} |\n", d.name, d.title));
    }
    out
}

fn builtins_doc() -> String {
    let mut out = String::from(
        "# Built-ins — the `COBOL` object's methods\n\n\
         Every RustCOBOL built-in is written inline, as a method of the `COBOL` object, with its \
         arguments in the order listed: data items are passed by reference, literals by content. \
         It is a statement, never a value.\n\n\
         ```cobol\n\
         \x20          COBOL::\"HTTP-GET\" ( WS-URL WS-RESPONSE WS-HTTP-STATUS )\n\
         ```\n\n\
         Each argument is marked `in` (the built-in reads it), `out` (it writes it) or `in-out`; \
         one in brackets may be left out. This is the complete list.\n\n\
         | Built-in | Arguments | What it does |\n|---|---|---|\n",
    );
    for b in cobolt_runtime::builtins::BUILTINS {
        let params = if b.params.is_empty() { "—" } else { b.params };
        out.push_str(&format!("| `COBOL::\"{}\"` | {} | {} |\n", b.name, params, b.description));
    }
    out
}

// ── .cfrm ────────────────────────────────────────────────────────────────────

/// A small but complete form, serialised by `cobolt_forms::form_to_string` —
/// the function the IDE saves with.
pub fn sample_form_xml() -> String {
    use cobolt_forms::{Control, ControlType, EventBinding, Form};
    let mut form = Form::new("CUSTOMER-FORM", "Customers", 480, 280);
    form.user_ws_source = "       01 WS-NAME         GLOBAL PIC X(40).\n".into();
    if let Some(on_load) = form.form_events.iter_mut().find(|e| e.event == "onLoad") {
        on_load.code = "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           MOVE SPACES TO WS-NAME\n".into();
    }
    let control = |id: &str, ty: ControlType, x: i32, y: i32, w: i32, h: i32, keep: &[(&str, &str)]| {
        let mut c = Control::new(id, ty, x, y);
        c.rect.w = w as _;
        c.rect.h = h as _;
        c.properties.retain(|k, _| keep.iter().any(|(n, _)| n == k));
        for (n, v) in keep {
            c.properties.insert((*n).into(), cobolt_forms::PropValue::String((*v).into()));
        }
        c
    };
    let panel = control("PNL-DETAILS", ControlType::Panel, 16, 16, 448, 176, &[]);
    let mut label = control("LBL-NAME", ControlType::Label, 32, 40, 120, 24, &[("Caption", "Name")]);
    label.parent = Some("PNL-DETAILS".into());
    let mut text = control("TXT-NAME", ControlType::TextBox, 160, 40, 280, 28, &[("Text", "")]);
    text.parent = Some("PNL-DETAILS".into());
    let mut button = control("BTN-SAVE", ControlType::Button, 352, 216, 112, 32, &[("Caption", "Save")]);
    let mut click = EventBinding::new("onClick", cobolt_forms::model::derive_paragraph_name("BTN-SAVE", "onClick"));
    click.code = "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           MOVE TXT-NAME::Text TO WS-NAME\n           SET LBL-NAME::Caption TO \"Saved\"\n".into();
    button.events.push(click);
    form.controls = vec![panel, label, text, button];
    cobolt_forms::form_to_string(&form).unwrap_or_default()
}

const CFRM_PROSE: &str = r#"# The `.cfrm` form file

A form is one XML file in `forms/`, UTF-8, that the IDE's Form Designer reads and writes and
from which PowerRustCOBOL generates the form's COBOL program. You may write it directly; then
call `validate` (it must load), `add_to_project` (a new form), `regenerate` and `check`.

## The `Form` element

The root element. Its attributes:

- `name` — the form's id: a COBOL word, which becomes the generated `PROGRAM-ID`.
- `title` — the window title. `width`, `height` — the form's size in pixels.
- `background` — the background colour, `RRGGBBAA` hex. `transparency` — 0 (opaque) to 100.
- `background-gradient-enabled`, `background-gradient-start`, `background-gradient-end`,
  `background-gradient-direction` — an optional background gradient.
- `background-image`, `bg-image-mode` — an optional background image and how it is scaled.
- `grid-size`, `snap-to-grid` — the designer's grid. `target` — the designer's target device.
- `theme`, `use-theme-background`, `glass-style`, `control-style` — the form's look.
- `main-form` — `true` on exactly **one** form of the project: the form a built application
  starts. Never put it on a second form.
- `taskbar-icon`, `can-minimize`, `can-maximize`, `window-state`, `full-screen`,
  `title-visible`, `start-position`, `x`, `y`, `window-effects`, `modal-overlay-style`,
  `form-format`, `responsive` — window behaviour; the properties of the same names in
  `form-layout-and-events.md` and `controls.md` say what each value means.
- `corner-radius` — the window's corner radius in pixels, rounding it while `title-visible` is
  `false`; written only when it is not 0.
- `index-base` — always `1`: the stored indexes (`tab`, `SelectedIndex`, `SelectedTab`) count
  from 1, with 0 for none. A file without it was saved before indexes counted from 1, and is
  shifted once as it loads. Write `index-base="1"` on every form you write.

An attribute that is left out takes its default; the IDE writes many of them only when they
differ from it.

## Blocks of COBOL the form owns

Each holds COBOL source in a CDATA section and is woven into the generated program:

- `working-storage` — the form's own data items. A handler sees one only when it is
  declared `GLOBAL`.
- `special-names`, `repository`, `file-control`, `file-section` — the form's
  SPECIAL-NAMES, REPOSITORY, FILE-CONTROL (`SELECT`) and FILE SECTION (`FD`, declared
  `IS GLOBAL` when a handler uses the file).
- `user-procedures` — procedures you write for the form, each an `Event` element whose
  `name` and `paragraph` are the procedure's name; callable with `CALL "<name>"`.
- `form-events` — the form's own events (`onLoad`, `onClose`, …), each an `Event` element.

## The `Control` element

One per control, in the order they are painted. Attributes:

- `id` — the control's id: a COBOL word, unique in the form.
- `type` — the control type, exactly as `controls.md` names it (`Button`, `TextBox`, …).
- `x`, `y`, `w`, `h` — position and size in pixels, in **form** coordinates — also for a
  control inside a container, whose rectangle lies inside the container's.
- `tab-order` — keyboard order. `z-order` — stacking. `visible`, `enabled` — `true`/`false`.
- `parent` — the id of the container (GroupBox, Panel, TabControl, Splitter, …) the control
  is inside. **Membership is this attribute, never the geometry.**
- `tab` — inside a TabControl, the page the control sits on, counting from 1 like `SelectedTab`.

Inside a `Control`:

- `Property` — one property: its `name` attribute and its value as text. Use only the names
  `controls.md` lists for that type; a property left out takes its default.
- `Event` — one bound handler: `name` is the event (`onClick`) and `paragraph` the handler's
  program name, `<CONTROL-ID>--<EVENT>` upper-cased (`BTN-SAVE--ONCLICK`). Its CDATA body is the
  handler's source: `ENVIRONMENT DIVISION.`, `DATA DIVISION.` (with the handler's own
  `WORKING-STORAGE SECTION.` when it needs one) and `PROCEDURE DIVISION.` with the statements.
  An event with no `Event` element runs nothing.
- `Animation` — a designer-made animation (`name`, `trigger`, `kind`, `duration`, `delay`,
  `easing`, `repeat`, `repeat-count`, `repeat-delay`, `slide-dx`, `slide-dy`).
- `Children` — an older nesting form the loader still reads; the IDE writes every control
  flat, with `parent`.

## Other elements the IDE writes

- `DataBindings` (attribute `schema-version`) — data bindings, as JSON in CDATA. Edit them in
  the IDE.
- `MenuPaneBackground` (`color`, `gradient-enabled`, `gradient-start`, `gradient-end`,
  `gradient-direction`, `transparency`, `image`, `image-mode`) — the shell sidebar's background.
- `FormLayout` and `Breakpoints` / `Breakpoint` (`name`, `min-width`, `font-factor`) /
  `Override` (`control`, `property`) — responsive layout.
- `deleted-controls` / `DeletedControl` (`id`, `deleted-at`) — handler code of deleted controls,
  kept and never compiled. Leave it alone.

## A live example

Serialised by the same code the IDE saves forms with (a Panel holding a Label and a TextBox, and
a Button whose `onClick` is bound):

"#;

fn cfrm_doc() -> String {
    format!("{CFRM_PROSE}```xml\n{}\n```\n", sample_form_xml().trim_end())
}

// ── .cidx ────────────────────────────────────────────────────────────────────

/// A definition with a group record, a primary and an alternate key, read and
/// re-serialised by `cobolt_indexed` — the code the IDE saves `.cidx` with.
pub fn sample_cidx_xml() -> String {
    const SOURCE: &str = r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="CUSTOMER-FILE" finalized="false" version="1.0"><assign-path>data/customers.idx</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="56"/><storage mode="disk" compression="false" persistence="false"/><comment><![CDATA[One row per customer.]]></comment><keys><primary duplicates="false" ordering="ascending"><part field="CUST-ID" offset="0" length="8" encoding="bytes"/></primary><alternate name="CUST-NAME" duplicates="true" ordering="ascending"><part field="CUST-NAME" offset="8" length="40" encoding="bytes"/></alternate></keys><fields><Field level="1" name="CUSTOMER-RECORD" usage="display"><Field level="5" name="CUST-ID" pic="9(8)" usage="display" offset="0" length="8"><comment><![CDATA[Primary key]]></comment></Field><Field level="5" name="CUST-NAME" pic="X(40)" usage="display" offset="8" length="40"></Field><Field level="5" name="CUST-BALANCE" pic="9(6)V99" usage="display" offset="48" length="8"></Field></Field></fields></IndexedFile>"#;
    cobolt_indexed::load_indexed_from_str(SOURCE)
        .ok()
        .and_then(|def| cobolt_indexed::save_indexed_to_string(&def).ok())
        .unwrap_or_default()
}

const CIDX_PROSE: &str = r#"# The `.cidx` indexed-file definition

An indexed (ISAM) file is described by one XML file in `indexed/`, which the IDE's Indexed File
Editor reads and writes. From it PowerRustCOBOL generates `generated/<name>-indexed.cbl` and the
copybooks `COPYBOOKS/<name>.SEL` (the `SELECT`) and `COPYBOOKS/<name>.FD` (the `FD`). Write the
definition, call `validate`, `add_to_project` and `regenerate`; never edit what is generated.

## The `IndexedFile` element

The root. Attributes: `name` — the file's COBOL name (the `SELECT`/`FD` name, a COBOL word);
`finalized` — `true` once the developer has finalized it in the Indexed File Editor, which
creates the data file and locks the structure (write `false`); `version` — the format version,
`1.0`.

Its children, in this order:

- `assign-path` — the data file's path. A relative path starts at the application's folder
  (the project folder under Run Form), e.g. `data/customers.idx`.
- `access-mode` — `dynamic`, `sequential` or `random`.
- `record-format` — `fixed-length` (the record length in bytes), or `min` and `max` for a
  variable-length record.
- `storage` — `mode` (`disk` or `memory`), `compression` and `persistence` (`true`/`false`;
  `persistence` applies to `memory` only).
- `comment` — optional, CDATA: a description that becomes a comment above the `FD`.
- `keys` — the `primary` key and any `alternate` keys. Each has `duplicates`
  (`true`/`false`), `ordering` (`ascending` or `descending`) and, on an alternate, a `name`;
  each holds one or more `part` elements with `field` (a field of the record), `offset` and
  `length` (bytes from the record's start) and `encoding` (`bytes`, `display-ascii`,
  `display-utf8`, `ucs2-le`, `ucs2-be`, `utf32-le`, `utf32-be`, `packed-decimal`,
  `binary-be`, `binary-le`).
- `fields` — the record: one level-1 `Field` (the record group) holding its sub-fields.

## The `Field` element

Attributes: `level` (`1`, `5`, …), `name` (a COBOL word), `pic` (the PICTURE, without `PIC`),
`usage` (`display`, `comp`, `comp-3`, `comp-4`, `binary`, `packed-decimal`, `index`,
`pointer`), `offset` and `length` (bytes, on elementary fields), and optionally `occurs`,
`redefines` (a field above it), `synchronized` and `grid-control` (the control the IDE's grid
uses for the field). A group field holds its sub-fields as nested `Field` elements; a field may
hold a `comment` (CDATA), which trails it in the generated `FD`.

Offsets and lengths must agree with the PICTUREs: a `9(8)` field is 8 bytes, and the next
field starts where it ends. Every key `part` must name a field and use that field's offset and
length.

## A live example

Read and re-serialised by the same code the IDE saves definitions with (line breaks added
between elements; the IDE writes it on one line):

"#;

fn cidx_doc() -> String {
    let xml = sample_cidx_xml().replace("><", ">\n<");
    format!("{CIDX_PROSE}```xml\n{}\n```\n", xml.trim_end())
}

/// Every element and attribute name in `xml` (CDATA bodies skipped).
#[cfg(test)]
fn xml_names(xml: &str) -> (std::collections::BTreeSet<String>, std::collections::BTreeSet<String>) {
    let mut elements = std::collections::BTreeSet::new();
    let mut attrs = std::collections::BTreeSet::new();
    let mut rest = xml;
    // Drop CDATA bodies: they hold COBOL, not format names.
    let mut clean = String::new();
    while let Some(i) = rest.find("<![CDATA[") {
        clean.push_str(&rest[..i]);
        rest = rest[i..].find("]]>").map(|j| &rest[i + j + 3..]).unwrap_or("");
    }
    clean.push_str(rest);
    for tag in clean.split('<').skip(1) {
        if tag.starts_with('/') || tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        let body = tag.split('>').next().unwrap_or("");
        let name: String = body.chars().take_while(|c| !c.is_whitespace() && *c != '/').collect();
        if !name.is_empty() {
            elements.insert(name.clone());
        }
        let mut after = &body[name.len()..];
        while let Some(eq) = after.find("=\"") {
            let attr = after[..eq].trim().rsplit(|c: char| c.is_whitespace()).next().unwrap_or("");
            if !attr.is_empty() {
                attrs.insert(attr.to_string());
            }
            after = &after[eq + 2..];
            after = after.find('"').map(|q| &after[q + 1..]).unwrap_or("");
        }
    }
    (elements, attrs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc<'a>(pack: &'a [RefDoc], name: &str) -> &'a str {
        &pack.iter().find(|d| d.name == name).unwrap_or_else(|| panic!("{name} not in the pack")).body
    }

    /// AC4: every control type, property, runtime property, event and method
    /// the System KB tables know, and every built-in, appears in the pack.
    #[test]
    fn pack_names_every_kb_entry_and_builtin() {
        let pack = pack("1.80.70");
        let controls = doc(&pack, "controls.md");
        let universal = controls.split("## Control: ").next().unwrap_or("");
        let (mut n_ctrl, mut n_prop, mut n_rt, mut n_ev, mut n_meth) = (0, 0, 0, 0, 0);
        let mut misses: Vec<String> = Vec::new();
        for ct in cobolt_forms::ControlType::ALL.iter().cloned() {
            let name = ct.as_str().to_owned();
            n_ctrl += 1;
            let heading = format!("## Control: {name}\n");
            let Some(start) = controls.find(&heading) else {
                misses.push(format!("control {name}"));
                continue;
            };
            let section = &controls[start + heading.len()..];
            let section = &section[..section.find("## Control: ").unwrap_or(section.len())];
            let seeded = cobolt_forms::Control::new("_", ct.clone(), 0, 0);
            for p in seeded.properties.keys() {
                n_prop += 1;
                let needle = format!("`{p}`");
                if !section.contains(&needle) && !universal.contains(&needle) {
                    misses.push(format!("{name}.{p}"));
                }
            }
            for p in cobolt_forms::model::runtime_property_names_for(&name) {
                n_rt += 1;
                if !section.contains(&format!("`{p}`")) {
                    misses.push(format!("{name}.{p} (runtime)"));
                }
            }
            for e in ct.supported_events() {
                n_ev += 1;
                if !section.contains(&format!("`{e}`")) {
                    misses.push(format!("{name}.{e} (event)"));
                }
            }
            for (sig, _) in cobolt_compiler::control_method_docs(&name) {
                n_meth += 1;
                if !section.contains(&format!("`{sig}`")) {
                    misses.push(format!("{name}.{sig} (method)"));
                }
            }
        }
        let builtins = doc(&pack, "builtins.md");
        let mut n_bi = 0;
        for b in cobolt_runtime::builtins::BUILTINS {
            n_bi += 1;
            if !builtins.contains(&format!("`COBOL::\"{}\"`", b.name)) {
                misses.push(format!("built-in {}", b.name));
            }
        }
        for left_out in KB_LEFT_OUT {
            assert!(!pack.iter().any(|d| d.name == left_out), "{left_out} must stay out of the pack");
        }
        for d in ["README.md", "developers-guide.md", "cobol85-supported-syntax.md", "control-methods.md",
                  "rustcobol-extensions.md", "form-layout-and-events.md", "form-themes.md",
                  "project-model-and-settings.md", "cfrm-format.md", "cidx-format.md"] {
            assert!(!doc(&pack, d).trim().is_empty(), "{d} is empty");
        }
        println!(
            "reference pack: {} files, {} bytes; compared {n_ctrl} controls, {n_prop} seeded properties, \
             {n_rt} runtime properties, {n_ev} events, {n_meth} methods, {n_bi} built-ins; {} missing: {misses:?}",
            pack.len(),
            pack.iter().map(|d| d.body.len()).sum::<usize>(),
            misses.len()
        );
        assert!(misses.is_empty(), "the pack is missing: {misses:?}");
    }

    /// AC4 (format docs): every element and attribute the live examples
    /// serialise is named in the prose, and the examples load back.
    #[test]
    fn format_docs_name_every_serialised_name() {
        let mut total = 0;
        for (label, xml, prose) in [
            ("cfrm", sample_form_xml(), CFRM_PROSE),
            ("cidx", sample_cidx_xml(), CIDX_PROSE),
        ] {
            assert!(!xml.is_empty(), "{label} example did not serialise");
            let (elements, attrs) = xml_names(&xml);
            let missing: Vec<&String> = elements
                .iter()
                .chain(attrs.iter())
                .filter(|n| !prose.contains(&format!("`{n}`")))
                .collect();
            println!(
                "{label}: {} elements, {} attributes in the live example; unnamed in the prose: {missing:?}",
                elements.len(),
                attrs.len()
            );
            assert!(missing.is_empty(), "{label} prose does not name {missing:?}");
            total += elements.len() + attrs.len();
        }
        let form = cobolt_forms::load_form_from_str(&sample_form_xml()).expect("the .cfrm example loads");
        assert_eq!(form.controls.len(), 4);
        let def = cobolt_indexed::load_indexed_from_str(&sample_cidx_xml()).expect("the .cidx example loads");
        cobolt_indexed::validate_definition(&def).expect("the .cidx example is valid");
        assert!(cobolt_indexed::finalize_warnings(&def).is_empty(), "the .cidx example has warnings");
        println!("format docs: {total} serialised names, all named; both examples load and validate");
    }
}
