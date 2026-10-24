// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The backend end of a web form's data controls (WASM-005 R13, R16, R17; T9).
//!
//! On the web a DataGrid bound to an indexed file or to an SQL query keeps its
//! COBOL API, but the rows are read and saved on the backend: every operation is
//! a remote call to a program the Build generates here, one per *(form, grid,
//! operation)*, named by [`cobolt_wire::controls::endpoint_name`]. The browser
//! sends a row window or a change set; what is read and written — the file, the
//! query, the table — is fixed in the generated program at design time (R17).
//!
//! Each program is ordinary COBOL the backend runs like the developer's own: an
//! exported program (`backend/generated/exports.toml`, authenticated, restricted
//! by the grid's `BackendRoles`) whose `LINKAGE SECTION` is the catalogue's
//! parameter and result groups. The developer's `backend/exports.toml` and
//! `backend/src` are never written.
//!
//! - **An indexed file** is read and written through the same `SELECT`/`FD` the
//!   Indexed File Editor's facade uses ([`crate::generate_indexed_select`],
//!   [`crate::generate_indexed_fd`]) with COBOL verbs (operator, 2026-10-23,
//!   T9 option (b)). A cell is what the desktop grid shows for the field, with
//!   the differences recorded in `specs/WASM-PARITY.md` row 19: a `COMP`/`COMP-3`
//!   field shows its value, a signed field its `-`. A save checks every change
//!   first, then writes them, and undoes them with the `ROLLBACK` verb if a
//!   write fails.
//! - **An SQL query** runs on the backend's named connection (`EXEC SQL CONNECT
//!   TO`), its rows by a cursor over the design-time query with the columns
//!   matched by name, the window by offset and count, `NULL` through indicator
//!   variables. A save is one unit of `UPDATE`/`INSERT`/`DELETE` with **host
//!   variables** — a value is never spliced into SQL text — and `ROLLBACK` on the
//!   first failure.
//!
//! **A class C control's method** (T11, [`generate_control_programs`]) — a
//! RestClient's `Get`, an AgentObject's `Ask`, an AWS control's route — gets one
//! program per *call site* in the form's handlers. The backend seeds the control
//! from the program's [`ControlDeclaration`] with its secrets laid over from
//! `backend.toml`'s `[[configuration]]`; the program calls the method as the
//! handler wrote it, the arguments that R17 keeps away from the browser compiled
//! in as the handler's literals, waits until the control has finished, and
//! answers its value, `LastError` and the events it raised.

use cobolt_forms::{
    web_class, BindingDataType, BindingMode, BindingSourceDescriptor, BindingTargetDescriptor, Control,
    DataBindingDef, Form,
};
use cobolt_indexed::{parse_pic, AccessMode, FieldUsage, IndexedDefinition, IndexedField, PicCategory};
use cobolt_wire::controls::{self, Field, Operation, Shape};

/// The folder, under `backend/`, the Build owns.
pub const GENERATED_DIR: &str = "generated";
/// The generated programs' `[[export]]` entries, beside them.
pub const EXPORTS_FILE: &str = "exports.toml";
/// A line every generated file carries: what a Build may replace or remove.
pub const GENERATED_MARK: &str = "This code was generated automatically by PowerRustCOBOL RAD.";

/// The parameter group of every generated program; its name never travels (E1).
const PARAMS: &str = "RC-PARAMS";
/// The `RETURNING` group.
const RESULT: &str = "RC-RESULT";

/// One generated backend program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendProgram {
    /// The endpoint name, which is also the `PROGRAM-ID`.
    pub program_id: String,
    /// `<PROGRAM-ID>.cbl`.
    pub file_name: String,
    pub source: String,
    /// Who may call it; empty = any authenticated caller. Never public.
    pub roles: Vec<String>,
    /// A read can be repeated safely; a save cannot.
    pub idempotent: bool,
    /// The control the program drives, which the backend seeds before it runs
    /// (a class C method, T11); `None` for a grid's program.
    pub control: Option<ControlDeclaration>,
}

/// The control a generated program calls a method on, as the backend must
/// build it (operator, 2026-10-24, T11 point 2): the program cannot create a
/// typed object, so the backend seeds it from this — written by the Build into
/// the program's `[export.control]` entry of `backend/generated/exports.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlDeclaration {
    /// The control's name, as the form spells it.
    pub id: String,
    /// The control type (`RestClient`), which routes its methods.
    pub class: String,
    /// The `[[configuration]]` of `backend.toml` whose values are laid over
    /// the properties (its `BackendConfig`); empty when it names none.
    pub configuration: String,
    /// The design-time properties, as the desktop host seeds them — never one
    /// that holds a secret: that comes only from the configuration (R14).
    pub properties: Vec<(String, String)>,
}

/// Everything the Build writes under `backend/generated/`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneratedBackend {
    pub programs: Vec<BackendProgram>,
    /// The upload slots of the forms' FileDropZones and Viewers (T12).
    pub slots: Vec<UploadSlot>,
}

/// A FileDropZone's or a Viewer's upload slot (T12, R15): the backend takes
/// into it only what the control itself would take, and names every file it
/// stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadSlot {
    /// `RC-<form>-<zone>-COMMITFILES` or `RC-<form>-<viewer>-OPEN`, the name
    /// the frontend uploads to.
    pub name: String,
    /// The control's `AllowedExtensions`, lower case, without dots; empty: any.
    pub extensions: Vec<String>,
    /// The control's `MaximumFileSizeKB`, in bytes; `None` when it sets none.
    pub max_bytes: Option<u64>,
    /// The control's `BackendRoles`; empty: any authenticated caller.
    pub roles: Vec<String>,
}

impl GeneratedBackend {
    pub fn is_empty(&self) -> bool {
        self.programs.is_empty() && self.slots.is_empty()
    }

    /// `backend/generated/exports.toml`: one `[[export]]` per program, the
    /// parameter group read only (`in`), the answer in the `RETURNING` group.
    pub fn exports_toml(&self) -> String {
        let mut out = String::new();
        for line in banner_lines() {
            out.push_str(&format!("#{}{line}\n", if line.is_empty() { "" } else { " " }));
        }
        for p in &self.programs {
            out.push_str("\n[[export]]\n");
            out.push_str(&format!("program = \"{}\"\n", p.program_id));
            if !p.roles.is_empty() {
                let roles: Vec<String> = p.roles.iter().map(|r| format!("\"{}\"", toml_escape(r))).collect();
                out.push_str(&format!("roles = [{}]\n", roles.join(", ")));
            }
            out.push_str("public = false\n");
            if p.idempotent {
                out.push_str("idempotent = true\n");
            }
            out.push_str("directions = [\"in\"]\n");
            if let Some(c) = &p.control {
                out.push_str("\n[export.control]\n");
                out.push_str(&format!("id = \"{}\"\n", toml_escape(&c.id)));
                out.push_str(&format!("class = \"{}\"\n", toml_escape(&c.class)));
                if !c.configuration.is_empty() {
                    out.push_str(&format!("configuration = \"{}\"\n", toml_escape(&c.configuration)));
                }
                if !c.properties.is_empty() {
                    out.push_str("\n[export.control.properties]\n");
                    for (k, v) in &c.properties {
                        out.push_str(&format!("\"{}\" = \"{}\"\n", toml_escape(k), toml_escape(v)));
                    }
                }
            }
        }
        let list = |items: &[String]| items.iter().map(|i| format!("\"{}\"", toml_escape(i))).collect::<Vec<_>>().join(", ");
        for s in &self.slots {
            out.push_str("\n[[slot]]\n");
            out.push_str(&format!("name = \"{}\"\n", s.name));
            if !s.extensions.is_empty() {
                out.push_str(&format!("extensions = [{}]\n", list(&s.extensions)));
            }
            if let Some(max) = s.max_bytes {
                out.push_str(&format!("max_bytes = {max}\n"));
            }
            if !s.roles.is_empty() {
                out.push_str(&format!("roles = [{}]\n", list(&s.roles)));
            }
        }
        out
    }
}

/// The upload slots of `form`'s FileDropZones and Viewers (T12): one per
/// control, named by the catalogue's `COMMITFILES` (a zone) or `OPEN` (a
/// Viewer) endpoint, with the control's own limits
/// (`web_class::upload_slot_limits`: a Viewer's default to the types it
/// renders, up to 10 MB).
pub fn generate_slots(form: &Form) -> Result<Vec<UploadSlot>, Vec<String>> {
    let mut out = Vec::new();
    let mut errs = Vec::new();
    for ctrl in crate::collect_all_controls(&form.controls) {
        let Some((filter, kb)) = web_class::upload_slot_limits(ctrl) else { continue };
        let operation = if ctrl.control_type.as_str() == "Viewer" { "OPEN" } else { "COMMITFILES" };
        let name = match endpoint(form, &ctrl.id, operation) {
            Ok(n) => n,
            Err(e) => {
                errs.push(format!("{}: {} {}: {e}", form.name, ctrl.control_type.as_str(), ctrl.id));
                continue;
            }
        };
        out.push(UploadSlot {
            name,
            extensions: cobolt_forms::dropzone::parse_extensions(&filter),
            // `0` (or less) is no limit, as on the desktop (`dropzone::size_allowed`).
            max_bytes: (kb > 0).then(|| (kb as u64).saturating_mul(1024)),
            roles: web_class::backend_roles(ctrl),
        });
    }
    if errs.is_empty() {
        Ok(out)
    } else {
        Err(errs)
    }
}

/// The endpoint of `operation` on control `control_id` of `form`: the name the
/// frontend calls and the backend exports. Form and control names go through
/// [`crate::cobol_word`] first, as every generated identifier does.
pub fn endpoint(form: &Form, control_id: &str, operation: &str) -> Result<String, String> {
    controls::endpoint_name(&crate::cobol_word(&form.name), &crate::cobol_word(control_id), operation)
}

/// Is `text` a file a Build generated (and may therefore replace or remove)?
pub fn is_generated(text: &str) -> bool {
    text.lines().take(4).any(|l| l.contains(GENERATED_MARK))
}

/// The backend programs of `forms` (a web or mobile project's forms): a `READ`
/// for every DataGrid bound to an indexed file or an SQL query, and a `SAVE`
/// when the binding is writable. `load_cidx` reads a binding's `.cidx` by the
/// path the form stores. Every problem of every form is reported together,
/// each naming its form and grid.
pub fn generate_backend(
    forms: &[Form],
    load_cidx: &dyn Fn(&str) -> Result<IndexedDefinition, String>,
) -> Result<GeneratedBackend, Vec<String>> {
    let mut out = GeneratedBackend::default();
    let mut errs = Vec::new();
    for form in forms {
        let all = crate::collect_all_controls(&form.controls);
        let mut bindings: Vec<&DataBindingDef> = form.data_bindings.iter().collect();
        bindings.sort_by(|a, b| a.id.cmp(&b.id));
        for binding in bindings {
            let BindingTargetDescriptor::DataGrid { control_id } = &binding.target else { continue };
            if !matches!(binding.source, BindingSourceDescriptor::IndexedFile { .. } | BindingSourceDescriptor::Sql { .. }) {
                continue;
            }
            let at = format!("{}: DataGrid {control_id}", form.name);
            let Some(grid) = all.iter().find(|c| c.id.eq_ignore_ascii_case(control_id)) else {
                errs.push(format!("{at}: binding {} feeds a grid the form does not have", binding.id));
                continue;
            };
            match grid_programs(form, grid, binding, &all, load_cidx) {
                Ok(mut programs) => out.programs.append(&mut programs),
                Err(e) => errs.push(format!("{at}: {e}")),
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for p in &out.programs {
        if !seen.insert(p.program_id.clone()) {
            errs.push(format!("{}: two grids make the same backend program", p.program_id));
        }
    }
    if errs.is_empty() {
        Ok(out)
    } else {
        Err(errs)
    }
}

/// The `READ` (and, when writable, `SAVE`) program of one bound grid.
fn grid_programs(
    form: &Form,
    grid: &Control,
    binding: &DataBindingDef,
    all: &[&Control],
    load_cidx: &dyn Fn(&str) -> Result<IndexedDefinition, String>,
) -> Result<Vec<BackendProgram>, String> {
    let writable = binding.mode == BindingMode::Writable && binding.source.is_writable();
    let roles = web_class::backend_roles(grid);
    let read_op = controls::find("DataGrid", "READ").ok_or("the catalogue has no DataGrid READ")?;
    let save_op = controls::find("DataGrid", "SAVE").ok_or("the catalogue has no DataGrid SAVE")?;
    let read_id = endpoint(form, &grid.id, "READ")?;
    let save_id = endpoint(form, &grid.id, "SAVE")?;
    let about = |op: &str, source: &str| Header { form: &form.name, grid: &grid.id, binding: &binding.id, op: op.to_owned(), source: source.to_owned() };
    let mut programs = Vec::new();
    match &binding.source {
        BindingSourceDescriptor::IndexedFile { definition_path, fields, .. } => {
            let def = load_cidx(definition_path).map_err(|e| format!("cannot read {definition_path}: {e}"))?;
            let ix = IndexedSource::new(&def, fields.iter().map(|f| f.name.clone()).collect(), writable)?;
            let source = format!("the indexed file {} ({definition_path})", def.name);
            programs.push(program(read_id.clone(), indexed_read(&about("READ", &source), &read_id, read_op, &ix), &roles, true));
            if writable {
                programs.push(program(save_id.clone(), indexed_save(&about("SAVE", &source), &save_id, save_op, &ix), &roles, false));
            }
        }
        BindingSourceDescriptor::Sql { source_control_id, fields, key_fields, query, table_name, .. } => {
            let sql = SqlSource::new(source_control_id, fields, key_fields, query, table_name, writable, all)?;
            let source = format!("the SQL query of {source_control_id} on the connection '{}'", sql.connection);
            programs.push(program(read_id.clone(), sql_read(&about("READ", &source), &read_id, read_op, &sql), &roles, true));
            if writable {
                programs.push(program(save_id.clone(), sql_save(&about("SAVE", &source), &save_id, save_op, &sql), &roles, false));
            }
        }
        _ => {}
    }
    Ok(programs)
}

fn program(program_id: String, source: String, roles: &[String], idempotent: bool) -> BackendProgram {
    BackendProgram { file_name: format!("{program_id}.cbl"), program_id, source, roles: roles.to_vec(), idempotent, control: None }
}

// ── The parts every program shares ───────────────────────────────────────────

fn banner_lines() -> [&'static str; 9] {
    [
        "───────────────────────────────────────────────────────────",
        " This code was generated automatically by PowerRustCOBOL RAD.",
        "",
        " DO NOT MODIFY IT DIRECTLY: it is regenerated on every Build of",
        " the project, so manual edits are lost. Edit the form and its",
        " data binding in the Form Designer instead.",
        "",
        " PowerRustCOBOL and its components are distributed under the",
        " Apache 2.0 License.",
    ]
}

/// What a program is for, written under its banner.
struct Header<'a> {
    form: &'a str,
    grid: &'a str,
    binding: &'a str,
    op: String,
    source: String,
}

fn write_head(out: &mut String, h: &Header, program_id: &str) {
    for line in banner_lines() {
        out.push_str(&format!("      *>{}{line}\n", if line.is_empty() { "" } else { " " }));
    }
    out.push_str("      *> ───────────────────────────────────────────────────────────\n\n");
    out.push_str("       IDENTIFICATION DIVISION.\n");
    out.push_str(&format!("       PROGRAM-ID. {program_id}.\n"));
    out.push_str(&format!("      *> {} of DataGrid {} on form {} (binding {}),\n", h.op, h.grid, h.form, h.binding));
    out.push_str(&format!("      *> run on the backend over {}.\n", h.source));
}

/// The catalogue's groups as COBOL: `01 RC-PARAMS` and `01 RC-RESULT`.
fn write_linkage(out: &mut String, op: &Operation) {
    out.push_str("       LINKAGE SECTION.\n");
    for (name, fields) in [(PARAMS, op.params), (RESULT, op.result)] {
        out.push_str(&format!("       01 {name}.\n"));
        write_fields(out, fields, 5, 10);
    }
}

fn write_fields(out: &mut String, fields: &[Field], level: u8, indent: usize) {
    let pad = " ".repeat(indent);
    for f in fields {
        match f.shape {
            Shape::Text(n) => out.push_str(&format!("{pad}{level:02} {} PIC X({n}).\n", f.name)),
            Shape::Count(d) => out.push_str(&format!("{pad}{level:02} {} PIC 9({d}).\n", f.name)),
            Shape::Table { max, counter, of } => {
                out.push_str(&format!("{pad}{level:02} {} OCCURS 0 TO {max} TIMES\n", f.name));
                out.push_str(&format!("{pad}      DEPENDING ON {counter}.\n"));
                write_fields(out, of, level + 5, indent + 3);
            }
        }
    }
}

/// Every name the catalogue's groups declare: a source field may not share one.
fn linkage_names(op: &Operation) -> Vec<String> {
    fn walk(fields: &[Field], out: &mut Vec<String>) {
        for f in fields {
            out.push(f.name.to_owned());
            if let Shape::Table { of, .. } = f.shape {
                walk(of, out);
            }
        }
    }
    let mut out = vec![PARAMS.to_owned(), RESULT.to_owned()];
    walk(op.params, &mut out);
    walk(op.result, &mut out);
    out
}

/// A COBOL alphanumeric literal: quotes doubled.
fn lit(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// The inside of a TOML basic string: a property value may hold a newline
/// (`DefaultHeaders`) or a tab, which a basic string must escape.
fn toml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// `10 ** n` as a literal for a small `n`, the expression otherwise.
fn power_of_ten(n: u16) -> String {
    if n <= 17 {
        format!("1{}", "0".repeat(n as usize))
    } else {
        format!("10 ** {n}")
    }
}

// ── An indexed file ──────────────────────────────────────────────────────────

/// How a field's cell text is made and read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    /// The characters, trailing spaces dropped (by the wire).
    Text { len: usize },
    /// The digits of the value with the point dropped (`12.50` in `9(5)V99` is
    /// `1250`, as the desktop grid shows a DISPLAY field), a `-` when negative.
    Number { digits: u16, scale: u16, signed: bool },
    /// A one-digit `PIC 9`: `1` or `0`.
    Indicator,
}

/// One column of the grid: its field, or nothing when the record has none of
/// that name (the cell is then empty, as on the desktop).
struct Column {
    field: Option<IndexedField>,
    cell: Cell,
}

/// What the programs of an indexed-file grid need from its `.cidx`.
struct IndexedSource {
    def: IndexedDefinition,
    columns: Vec<Column>,
    /// The primary key's fields, in key order, with their grid column.
    keys: Vec<(IndexedField, usize, Cell)>,
}

impl IndexedSource {
    fn new(def: &IndexedDefinition, mut fields: Vec<String>, writable: bool) -> Result<IndexedSource, String> {
        let root = def.record_root().ok_or_else(|| format!("{} declares no record", def.name))?;
        let leaves = root.all_leaves();
        // A binding that names no field shows every leaf (the desktop's quick-bind).
        if fields.is_empty() {
            fields = leaves.iter().map(|l| l.name.clone()).collect();
        }
        if fields.is_empty() {
            return Err(format!("the record of {} has no field to show", def.name));
        }
        let read = controls::find("DataGrid", "READ").map(linkage_names).unwrap_or_default();
        let save = controls::find("DataGrid", "SAVE").map(linkage_names).unwrap_or_default();
        let mut taken: Vec<String> = Vec::new();
        fn names(f: &IndexedField, out: &mut Vec<String>) {
            out.push(f.name.to_ascii_uppercase());
            f.children.iter().for_each(|c| names(c, out));
        }
        def.fields.iter().for_each(|f| names(f, &mut taken));
        taken.push(def.name.to_ascii_uppercase());
        for name in &taken {
            if name.starts_with("RC-") || read.iter().chain(save.iter()).any(|n| n.eq_ignore_ascii_case(name)) {
                return Err(format!(
                    "{} names an item {name}, which the generated backend program declares itself; rename it in the .cidx",
                    def.name
                ));
            }
        }
        let leaf_of = |name: &str| -> Result<Option<IndexedField>, String> {
            let found: Vec<&&IndexedField> = leaves.iter().filter(|l| l.name.eq_ignore_ascii_case(name)).collect();
            match found.as_slice() {
                [] => Ok(None),
                [one] if one.occurs.is_some() => Err(format!("field {name} of {} is a table; a grid column shows one item", def.name)),
                [one] => Ok(Some((**one).clone())),
                _ => Err(format!("{} declares field {name} more than once; a grid column must name one item", def.name)),
            }
        };
        let mut columns = Vec::new();
        for name in &fields {
            let field = leaf_of(name)?;
            let cell = field.as_ref().map_or(Cell::Text { len: 0 }, cell_of);
            columns.push(Column { field, cell });
        }
        let mut keys = Vec::new();
        if writable {
            if def.keys.primary.parts.is_empty() {
                return Err(format!("{} has no primary key, so a row cannot be found to write it", def.name));
            }
            for part in &def.keys.primary.parts {
                let Some(col) = fields.iter().position(|f| f.eq_ignore_ascii_case(&part.field_name)) else {
                    return Err(format!("the key field {} is not one of the grid's columns, so a row cannot be found to write it", part.field_name));
                };
                let field = leaf_of(&part.field_name)?.ok_or_else(|| format!("the key field {} is not a field of the record", part.field_name))?;
                let cell = cell_of(&field);
                keys.push((field, col, cell));
            }
        }
        Ok(IndexedSource { def: def.clone(), columns, keys })
    }

    fn record(&self) -> &str {
        self.def.record_root().map(|r| r.name.as_str()).unwrap_or_default()
    }
}

fn cell_of(field: &IndexedField) -> Cell {
    let pic = parse_pic(&field.pic);
    let numeric = |pic: &cobolt_indexed::ParsedPic| {
        let scale = decimal_places(&field.pic);
        Cell::Number { digits: pic.width as u16, scale, signed: pic.signed }
    };
    match (field.usage, pic.category) {
        (FieldUsage::Display, PicCategory::Numeric) => numeric(&pic),
        (FieldUsage::Display, PicCategory::Indicator) => Cell::Indicator,
        (FieldUsage::Display, _) => Cell::Text { len: field.length.unwrap_or(pic.width as u32) as usize },
        // COMP, COMP-3 and the rest: the value (WASM-PARITY row 19).
        _ => numeric(&pic),
    }
}

/// The digit positions after the `V` of a picture.
fn decimal_places(pic: &str) -> u16 {
    let upper = pic.trim_start_matches('\u{200B}').trim().to_ascii_uppercase();
    let Some((_, after)) = upper.split_once('V') else { return 0 };
    let chars: Vec<char> = after.chars().collect();
    let (mut n, mut i) = (0u16, 0usize);
    while i < chars.len() {
        if chars[i] == '9' {
            if chars.get(i + 1) == Some(&'(') {
                let end = chars[i + 2..].iter().position(|c| *c == ')').map(|p| i + 2 + p).unwrap_or(chars.len());
                n += chars[i + 2..end].iter().collect::<String>().trim().parse::<u16>().unwrap_or(1);
                i = end + 1;
                continue;
            }
            n += 1;
        }
        i += 1;
    }
    n
}

/// The statements that put `field` into `target` as the grid shows it.
fn write_cell_out(out: &mut String, pad: &str, field: &str, cell: Cell, target: &str) {
    match cell {
        Cell::Text { .. } => out.push_str(&format!("{pad}MOVE {field} TO {target}\n")),
        Cell::Number { scale, .. } => {
            if scale == 0 {
                out.push_str(&format!("{pad}MOVE {field} TO RC-NUMBER\n"));
            } else {
                out.push_str(&format!("{pad}COMPUTE RC-NUMBER = {field} * {}\n", power_of_ten(scale)));
            }
            out.push_str(&format!("{pad}MOVE FUNCTION TRIM(RC-NUMBER) TO {target}\n"));
        }
        Cell::Indicator => {
            out.push_str(&format!("{pad}IF {field} = 1\n"));
            out.push_str(&format!("{pad}    MOVE \"1\" TO {target}\n"));
            out.push_str(&format!("{pad}ELSE\n"));
            out.push_str(&format!("{pad}    MOVE \"0\" TO {target}\n"));
            out.push_str(&format!("{pad}END-IF\n"));
        }
    }
}

/// The statements that put cell text `source` into `field` — the inverse of
/// [`write_cell_out`]. The text has been checked by [`write_cell_check`].
fn write_cell_in(out: &mut String, pad: &str, source: &str, cell: Cell, field: &str) {
    match cell {
        Cell::Text { .. } => out.push_str(&format!("{pad}MOVE {source} TO {field}\n")),
        Cell::Number { scale, .. } => {
            if scale == 0 {
                out.push_str(&format!("{pad}COMPUTE {field} = FUNCTION NUMVAL({source})\n"));
            } else {
                out.push_str(&format!("{pad}COMPUTE {field} = FUNCTION NUMVAL({source}) / {}\n", power_of_ten(scale)));
            }
        }
        Cell::Indicator => {
            out.push_str(&format!("{pad}EVALUATE {source}\n"));
            for word in INDICATOR_ON {
                out.push_str(&format!("{pad}    WHEN {}\n", lit(word)));
            }
            out.push_str(&format!("{pad}        MOVE 1 TO {field}\n"));
            out.push_str(&format!("{pad}    WHEN OTHER\n"));
            out.push_str(&format!("{pad}        MOVE 0 TO {field}\n"));
            out.push_str(&format!("{pad}END-EVALUATE\n"));
        }
    }
}

/// The words an indicator cell reads as on / off — the desktop's own
/// (`cobolt_indexed::pic` `encode_indicator`).
const INDICATOR_ON: [&str; 7] = ["1", "Y", "y", "YES", "yes", "TRUE", "true"];
const INDICATOR_OFF: [&str; 7] = ["0", "N", "n", "NO", "no", "FALSE", "false"];

/// The statements that refuse cell text `source` the field cannot hold: they
/// set `RC-WHY`, which the caller turns into status `92`.
fn write_cell_check(out: &mut String, pad: &str, source: &str, cell: Cell, name: &str) {
    match cell {
        // Longer than the field: something but spaces past its last byte.
        Cell::Text { len } if len < controls::CELL_BYTES => {
            out.push_str(&format!("{pad}IF {source}({}:) NOT = SPACES\n", len + 1));
            out.push_str(&format!("{pad}    MOVE {} TO RC-WHY\n", lit(&format!("{name}: value length exceeds PIC"))));
            out.push_str(&format!("{pad}END-IF\n"));
        }
        Cell::Text { .. } => out.push_str(&format!("{pad}CONTINUE\n")),
        Cell::Number { digits, signed, .. } => {
            out.push_str(&format!("{pad}EVALUATE TRUE\n"));
            out.push_str(&format!("{pad}    WHEN {source} = SPACES\n"));
            out.push_str(&format!("{pad}        MOVE {} TO RC-WHY\n", lit(&format!("{name}: value required"))));
            out.push_str(&format!("{pad}    WHEN FUNCTION TEST-NUMVAL({source}) NOT = 0\n"));
            out.push_str(&format!("{pad}        MOVE {} TO RC-WHY\n", lit(&format!("{name}: invalid characters for PIC"))));
            out.push_str(&format!(
                "{pad}    WHEN FUNCTION NUMVAL({source}) NOT = FUNCTION INTEGER-PART(FUNCTION NUMVAL({source}))\n"
            ));
            out.push_str(&format!("{pad}        MOVE {} TO RC-WHY\n", lit(&format!("{name}: invalid characters for PIC"))));
            if !signed {
                out.push_str(&format!("{pad}    WHEN FUNCTION NUMVAL({source}) < 0\n"));
                out.push_str(&format!("{pad}        MOVE {} TO RC-WHY\n", lit(&format!("{name}: invalid characters for PIC"))));
            }
            out.push_str(&format!("{pad}    WHEN FUNCTION ABS(FUNCTION NUMVAL({source})) >= {}\n", power_of_ten(digits)));
            out.push_str(&format!("{pad}        MOVE {} TO RC-WHY\n", lit(&format!("{name}: value too large for PIC"))));
            out.push_str(&format!("{pad}    WHEN OTHER\n"));
            out.push_str(&format!("{pad}        CONTINUE\n"));
            out.push_str(&format!("{pad}END-EVALUATE\n"));
        }
        Cell::Indicator => {
            out.push_str(&format!("{pad}EVALUATE {source}\n"));
            for word in INDICATOR_ON.iter().chain(INDICATOR_OFF.iter()) {
                out.push_str(&format!("{pad}    WHEN {}\n", lit(word)));
            }
            out.push_str(&format!("{pad}    WHEN SPACES\n"));
            out.push_str(&format!("{pad}        CONTINUE\n"));
            out.push_str(&format!("{pad}    WHEN OTHER\n"));
            out.push_str(&format!("{pad}        MOVE {} TO RC-WHY\n", lit(&format!("{name}: indicator must be 0 or 1"))));
            out.push_str(&format!("{pad}END-EVALUATE\n"));
        }
    }
}

/// `ENVIRONMENT DIVISION` and `FILE SECTION` of an indexed-file program: the
/// facade's `SELECT` and `FD`, with dynamic access (the read walks the file,
/// the save reads by key).
fn write_indexed_file(out: &mut String, ix: &IndexedSource) {
    let mut def = ix.def.clone();
    def.access_mode = AccessMode::Dynamic;
    out.push_str("\n       ENVIRONMENT DIVISION.\n");
    out.push_str("       INPUT-OUTPUT SECTION.\n");
    out.push_str("       FILE-CONTROL.\n");
    out.push_str(&crate::generate_indexed_select(&def));
    out.push_str("\n       DATA DIVISION.\n");
    out.push_str("       FILE SECTION.\n");
    // One sentence: `FD name.` followed by a `RECORD CONTAINS` sentence of its
    // own detaches the record from the file (the parser ends the FD entry at
    // the first period), so the clause stays inside the entry.
    let fd = crate::generate_indexed_fd(&def);
    out.push_str(&fd.replacen(&format!("       FD  {}.\n", def.name), &format!("       FD  {}\n", def.name), 1));
}

fn file_status_call(file: &str) -> String {
    format!("COBOL::\"FILE-STATUS\" ( \"{file}\" RC-FILE-STATUS )")
}

fn indexed_read(h: &Header, program_id: &str, op: &Operation, ix: &IndexedSource) -> String {
    let file = ix.def.name.as_str();
    let mut out = String::new();
    write_head(&mut out, h, program_id);
    write_indexed_file(&mut out, ix);
    out.push_str("       WORKING-STORAGE SECTION.\n");
    out.push_str("       01 RC-FILE-STATUS PIC XX.\n");
    out.push_str("       01 RC-SEEN PIC 9(9) VALUE 0.\n");
    out.push_str("       01 RC-ROW PIC 9(4) VALUE 0.\n");
    out.push_str("       01 RC-WANT PIC 9(4) VALUE 0.\n");
    out.push_str("       01 RC-AT-END PIC 9 VALUE 0.\n");
    out.push_str("       01 RC-NUMBER PIC -(37)9.\n");
    write_linkage(&mut out, op);
    out.push_str(&format!("\n       PROCEDURE DIVISION USING {PARAMS} RETURNING {RESULT}.\n"));
    out.push_str("       RC-MAIN.\n");
    out.push_str("           MOVE \"00\" TO READ-STATUS\n");
    out.push_str("           MOVE SPACES TO READ-MESSAGE\n");
    out.push_str("           MOVE 0 TO ROW-TOTAL ROWS-RETURNED\n");
    write_window_and_params(&mut out);
    out.push_str(&format!("           OPEN INPUT {file}\n"));
    out.push_str(&format!("           {}\n", file_status_call(file)));
    out.push_str("           IF RC-FILE-STATUS = \"35\"\n");
    out.push_str("      *>       No file yet is an empty grid, as on the desktop.\n");
    out.push_str("               MOVE \"35\" TO READ-STATUS\n");
    out.push_str("               MOVE \"the file does not exist yet\" TO READ-MESSAGE\n");
    out.push_str("               GOBACK\n");
    out.push_str("           END-IF\n");
    out.push_str("           IF RC-FILE-STATUS(1:1) NOT = \"0\"\n");
    out.push_str("               MOVE RC-FILE-STATUS TO READ-STATUS\n");
    out.push_str("               MOVE \"the file cannot be opened\" TO READ-MESSAGE\n");
    out.push_str("               GOBACK\n");
    out.push_str("           END-IF\n");
    out.push_str("           PERFORM UNTIL RC-AT-END = 1\n");
    out.push_str(&format!("               READ {file} NEXT\n"));
    out.push_str("                   AT END\n");
    out.push_str("                       MOVE 1 TO RC-AT-END\n");
    out.push_str("                   NOT AT END\n");
    out.push_str("                       PERFORM RC-TAKE-RECORD\n");
    out.push_str("               END-READ\n");
    out.push_str(&format!("               {}\n", file_status_call(file)));
    out.push_str("               IF RC-FILE-STATUS(1:1) NOT = \"0\"\n");
    out.push_str("                   MOVE 1 TO RC-AT-END\n");
    out.push_str("               END-IF\n");
    out.push_str("           END-PERFORM\n");
    out.push_str("           IF RC-FILE-STATUS NOT = \"10\"\n");
    out.push_str("               MOVE RC-FILE-STATUS TO READ-STATUS\n");
    out.push_str("               MOVE \"the file cannot be read\" TO READ-MESSAGE\n");
    out.push_str("           END-IF\n");
    out.push_str("           MOVE RC-SEEN TO ROW-TOTAL\n");
    out.push_str(&format!("           CLOSE {file}\n"));
    out.push_str("           GOBACK.\n\n");
    out.push_str("       RC-TAKE-RECORD.\n");
    out.push_str("           ADD 1 TO RC-SEEN\n");
    out.push_str("           IF RC-SEEN > ROW-OFFSET AND ROWS-RETURNED < RC-WANT\n");
    out.push_str("               ADD 1 TO ROWS-RETURNED\n");
    out.push_str("               MOVE ROWS-RETURNED TO RC-ROW\n");
    out.push_str(&format!("               MOVE {} TO CELL-COUNT (RC-ROW)\n", ix.columns.len()));
    for (i, col) in ix.columns.iter().enumerate() {
        let target = format!("CELL-TEXT (RC-ROW {})", i + 1);
        match &col.field {
            Some(field) => write_cell_out(&mut out, "               ", &field.name, col.cell, &target),
            None => out.push_str(&format!("               MOVE SPACES TO {target}\n")),
        }
    }
    out.push_str("           END-IF.\n");
    out
}

/// The row window, capped at the catalogue's most, and the refusal of bound
/// parameters the design-time source does not take.
fn write_window_and_params(out: &mut String) {
    out.push_str(&format!("           MOVE FUNCTION MIN(ROW-COUNT {}) TO RC-WANT\n", controls::MAX_WINDOW_ROWS));
    out.push_str("           IF PARAM-COUNT NOT = 0\n");
    out.push_str("               MOVE \"92\" TO READ-STATUS\n");
    out.push_str("               MOVE \"this source takes no parameters\" TO READ-MESSAGE\n");
    out.push_str("               GOBACK\n");
    out.push_str("           END-IF\n");
}

fn indexed_save(h: &Header, program_id: &str, op: &Operation, ix: &IndexedSource) -> String {
    let file = ix.def.name.as_str();
    let record = ix.record();
    let ncols = ix.columns.len();
    let nkeys = ix.keys.len();
    let key_cols: Vec<usize> = ix.keys.iter().map(|(_, c, _)| *c).collect();
    let mut out = String::new();
    write_head(&mut out, h, program_id);
    write_indexed_file(&mut out, ix);
    out.push_str("       WORKING-STORAGE SECTION.\n");
    out.push_str("       01 RC-FILE-STATUS PIC XX.\n");
    out.push_str("       01 RC-CHANGE PIC 9(4) VALUE 0.\n");
    out.push_str("       01 RC-WHY PIC X(256) VALUE SPACES.\n");
    out.push_str("       01 RC-FOUND PIC 9 VALUE 0.\n");
    write_linkage(&mut out, op);
    out.push_str(&format!("\n       PROCEDURE DIVISION USING {PARAMS} RETURNING {RESULT}.\n"));
    out.push_str("       RC-MAIN.\n");
    out.push_str("           MOVE \"00\" TO COMMIT-STATUS\n");
    out.push_str("           MOVE SPACES TO COMMIT-MESSAGE\n");
    out.push_str("           MOVE 0 TO FAILED-CHANGE\n");
    out.push_str("      *>   A save never makes the file (OPEN I-O would): no file is 35,\n");
    out.push_str("      *>   as on the desktop.\n");
    out.push_str(&format!("           OPEN INPUT {file}\n"));
    out.push_str(&format!("           {}\n", file_status_call(file)));
    out.push_str("           IF RC-FILE-STATUS = \"35\"\n");
    out.push_str("               MOVE \"35\" TO COMMIT-STATUS\n");
    out.push_str("               MOVE \"the file does not exist\" TO COMMIT-MESSAGE\n");
    out.push_str("               GOBACK\n");
    out.push_str("           END-IF\n");
    out.push_str("           IF RC-FILE-STATUS(1:1) = \"0\"\n");
    out.push_str(&format!("               CLOSE {file}\n"));
    out.push_str("           END-IF\n");
    out.push_str(&format!("           OPEN I-O {file}\n"));
    out.push_str(&format!("           {}\n", file_status_call(file)));
    out.push_str("           IF RC-FILE-STATUS(1:1) NOT = \"0\"\n");
    out.push_str("               MOVE RC-FILE-STATUS TO COMMIT-STATUS\n");
    out.push_str("               MOVE \"the file cannot be opened for writing\" TO COMMIT-MESSAGE\n");
    out.push_str("               GOBACK\n");
    out.push_str("           END-IF\n");
    out.push_str("      *>   Every change is checked before the first one is written.\n");
    out.push_str("           PERFORM RC-CHECK-CHANGE VARYING RC-CHANGE FROM 1 BY 1\n");
    out.push_str("               UNTIL RC-CHANGE > CHANGE-COUNT OR COMMIT-STATUS NOT = \"00\"\n");
    out.push_str("           IF COMMIT-STATUS = \"00\"\n");
    out.push_str("               PERFORM RC-APPLY-CHANGE VARYING RC-CHANGE FROM 1 BY 1\n");
    out.push_str("                   UNTIL RC-CHANGE > CHANGE-COUNT OR COMMIT-STATUS NOT = \"00\"\n");
    out.push_str("           END-IF\n");
    out.push_str("      *>   All or nothing: a write that failed undoes the ones before it.\n");
    out.push_str("           IF COMMIT-STATUS = \"00\"\n");
    out.push_str("               COMMIT\n");
    out.push_str("           ELSE\n");
    out.push_str("               ROLLBACK\n");
    out.push_str("           END-IF\n");
    out.push_str(&format!("           CLOSE {file}\n"));
    out.push_str("           GOBACK.\n\n");

    // RC-REFUSE: the change in RC-CHANGE fails with the status already moved.
    out.push_str("       RC-REFUSE.\n");
    out.push_str("           MOVE RC-WHY TO COMMIT-MESSAGE\n");
    out.push_str("           MOVE RC-CHANGE TO FAILED-CHANGE.\n\n");

    out.push_str("       RC-CHECK-CHANGE.\n");
    out.push_str("           MOVE SPACES TO RC-WHY\n");
    out.push_str("           EVALUATE CHANGE-ACTION (RC-CHANGE)\n");
    out.push_str("               WHEN \"UPDATE\"\n");
    write_count_check(&mut out, "KEY-COUNT", nkeys, "key cells");
    write_count_check(&mut out, "CELL-COUNT", ncols, "cells");
    out.push_str("                   IF RC-WHY = SPACES\n");
    out.push_str("                       PERFORM RC-CHECK-KEYS\n");
    out.push_str("                   END-IF\n");
    out.push_str("                   IF RC-WHY = SPACES\n");
    out.push_str("                       PERFORM RC-CHECK-CELLS\n");
    out.push_str("                   END-IF\n");
    out.push_str("                   IF RC-WHY = SPACES\n");
    out.push_str("                       PERFORM RC-KEY-UNCHANGED\n");
    out.push_str("                   END-IF\n");
    out.push_str("                   IF RC-WHY = SPACES\n");
    out.push_str("                       PERFORM RC-KEY-FROM-KEYS\n");
    out.push_str("                       PERFORM RC-FIND\n");
    out.push_str("                       IF RC-FOUND = 0\n");
    out.push_str("                           MOVE \"23\" TO COMMIT-STATUS\n");
    out.push_str("                           MOVE \"the record is no longer in the file\" TO RC-WHY\n");
    out.push_str("                       END-IF\n");
    out.push_str("                   END-IF\n");
    out.push_str("               WHEN \"DELETE\"\n");
    write_count_check(&mut out, "KEY-COUNT", nkeys, "key cells");
    out.push_str("                   IF RC-WHY = SPACES\n");
    out.push_str("                       PERFORM RC-CHECK-KEYS\n");
    out.push_str("                   END-IF\n");
    out.push_str("                   IF RC-WHY = SPACES\n");
    out.push_str("                       PERFORM RC-KEY-FROM-KEYS\n");
    out.push_str("                       PERFORM RC-FIND\n");
    out.push_str("                       IF RC-FOUND = 0\n");
    out.push_str("                           MOVE \"23\" TO COMMIT-STATUS\n");
    out.push_str("                           MOVE \"a deleted row is no longer in the file\" TO RC-WHY\n");
    out.push_str("                       END-IF\n");
    out.push_str("                   END-IF\n");
    out.push_str("               WHEN \"INSERT\"\n");
    write_count_check(&mut out, "CELL-COUNT", ncols, "cells");
    out.push_str("                   IF RC-WHY = SPACES\n");
    out.push_str("                       PERFORM RC-CHECK-CELLS\n");
    out.push_str("                   END-IF\n");
    for (field, col, _) in &ix.keys {
        out.push_str(&format!("                   IF RC-WHY = SPACES AND CELL-TEXT (RC-CHANGE {}) = SPACES\n", col + 1));
        out.push_str(&format!(
            "                       MOVE {} TO RC-WHY\n",
            lit(&format!("'{}' is part of the key; a new row needs it", field.name))
        ));
        out.push_str("                   END-IF\n");
    }
    out.push_str("                   IF RC-WHY = SPACES\n");
    out.push_str("                       PERFORM RC-KEY-FROM-CELLS\n");
    out.push_str("                       PERFORM RC-FIND\n");
    out.push_str("                       IF RC-FOUND = 1\n");
    out.push_str("                           MOVE \"22\" TO COMMIT-STATUS\n");
    out.push_str("                           MOVE \"a record with this key is already in the file\" TO RC-WHY\n");
    out.push_str("                       END-IF\n");
    out.push_str("                   END-IF\n");
    out.push_str("               WHEN OTHER\n");
    out.push_str("                   MOVE \"the change is not UPDATE, INSERT or DELETE\" TO RC-WHY\n");
    out.push_str("           END-EVALUATE\n");
    out.push_str("           IF RC-WHY NOT = SPACES\n");
    out.push_str("               IF COMMIT-STATUS = \"00\"\n");
    out.push_str("                   MOVE \"92\" TO COMMIT-STATUS\n");
    out.push_str("               END-IF\n");
    out.push_str("               PERFORM RC-REFUSE\n");
    out.push_str("           END-IF.\n\n");

    out.push_str("       RC-CHECK-KEYS.\n");
    for (i, (field, _, cell)) in ix.keys.iter().enumerate() {
        out.push_str("           IF RC-WHY = SPACES\n");
        write_cell_check(&mut out, "               ", &format!("KEY-TEXT (RC-CHANGE {})", i + 1), *cell, &field.name);
        out.push_str("           END-IF\n");
    }
    out.push_str("           CONTINUE.\n\n");

    out.push_str("       RC-CHECK-CELLS.\n");
    for (i, col) in ix.columns.iter().enumerate() {
        if let Some(field) = &col.field {
            out.push_str("           IF RC-WHY = SPACES\n");
            write_cell_check(&mut out, "               ", &format!("CELL-TEXT (RC-CHANGE {})", i + 1), col.cell, &field.name);
            out.push_str("           END-IF\n");
        }
    }
    out.push_str("           CONTINUE.\n\n");

    // A row is found by its key: the key cells of an update are what it was loaded with.
    out.push_str("       RC-KEY-UNCHANGED.\n");
    for (i, (_, col, _)) in ix.keys.iter().enumerate() {
        out.push_str(&format!(
            "           IF FUNCTION TRIM(CELL-TEXT (RC-CHANGE {})) NOT = FUNCTION TRIM(KEY-TEXT (RC-CHANGE {}))\n",
            col + 1,
            i + 1
        ));
        out.push_str("               MOVE \"21\" TO COMMIT-STATUS\n");
        out.push_str("               MOVE \"the key of the row was changed; a row is found by its key\" TO RC-WHY\n");
        out.push_str("           END-IF\n");
    }
    out.push_str("           CONTINUE.\n\n");

    out.push_str("       RC-KEY-FROM-KEYS.\n");
    for (i, (field, _, cell)) in ix.keys.iter().enumerate() {
        write_cell_in(&mut out, "           ", &format!("KEY-TEXT (RC-CHANGE {})", i + 1), *cell, &field.name);
    }
    out.push_str("           CONTINUE.\n\n");

    out.push_str("       RC-KEY-FROM-CELLS.\n");
    for (field, col, cell) in &ix.keys {
        write_cell_in(&mut out, "           ", &format!("CELL-TEXT (RC-CHANGE {})", col + 1), *cell, &field.name);
    }
    out.push_str("           CONTINUE.\n\n");

    out.push_str("       RC-FIND.\n");
    out.push_str(&format!("           READ {file}\n"));
    out.push_str("               INVALID KEY\n");
    out.push_str("                   MOVE 0 TO RC-FOUND\n");
    out.push_str("               NOT INVALID KEY\n");
    out.push_str("                   MOVE 1 TO RC-FOUND\n");
    out.push_str("           END-READ.\n\n");

    out.push_str("       RC-APPLY-CHANGE.\n");
    out.push_str("           MOVE SPACES TO RC-WHY\n");
    out.push_str("           EVALUATE CHANGE-ACTION (RC-CHANGE)\n");
    out.push_str("               WHEN \"UPDATE\"\n");
    out.push_str("                   PERFORM RC-KEY-FROM-KEYS\n");
    out.push_str("                   PERFORM RC-FIND\n");
    out.push_str("                   IF RC-FOUND = 1\n");
    for (i, col) in ix.columns.iter().enumerate() {
        if let Some(field) = &col.field {
            if !key_cols.contains(&i) {
                write_cell_in(&mut out, "                       ", &format!("CELL-TEXT (RC-CHANGE {})", i + 1), col.cell, &field.name);
            }
        }
    }
    out.push_str(&format!("                       REWRITE {record}\n"));
    out.push_str("                           INVALID KEY CONTINUE\n");
    out.push_str("                       END-REWRITE\n");
    out.push_str("                   END-IF\n");
    out.push_str("               WHEN \"DELETE\"\n");
    out.push_str("                   PERFORM RC-KEY-FROM-KEYS\n");
    out.push_str(&format!("                   DELETE {file}\n"));
    out.push_str("                       INVALID KEY CONTINUE\n");
    out.push_str("                   END-DELETE\n");
    out.push_str("               WHEN \"INSERT\"\n");
    out.push_str(&format!("                   INITIALIZE {record}\n"));
    for (i, col) in ix.columns.iter().enumerate() {
        if let Some(field) = &col.field {
            write_cell_in(&mut out, "                   ", &format!("CELL-TEXT (RC-CHANGE {})", i + 1), col.cell, &field.name);
        }
    }
    out.push_str(&format!("                   WRITE {record}\n"));
    out.push_str("                       INVALID KEY CONTINUE\n");
    out.push_str("                   END-WRITE\n");
    out.push_str("           END-EVALUATE\n");
    out.push_str(&format!("           {}\n", file_status_call(file)));
    out.push_str("           IF RC-FILE-STATUS(1:1) NOT = \"0\"\n");
    out.push_str("               MOVE RC-FILE-STATUS TO COMMIT-STATUS\n");
    out.push_str("               MOVE \"the file refused the change\" TO RC-WHY\n");
    out.push_str("               PERFORM RC-REFUSE\n");
    out.push_str("           END-IF.\n");
    out
}

/// A change carries exactly as many key cells or cells as the source has.
fn write_count_check(out: &mut String, counter: &str, expected: usize, what: &str) {
    out.push_str(&format!("                   IF RC-WHY = SPACES AND {counter} (RC-CHANGE) NOT = {expected}\n"));
    out.push_str(&format!("                       MOVE {} TO RC-WHY\n", lit(&format!("the change does not carry {expected} {what}"))));
    out.push_str("                   END-IF\n");
}

// ── An SQL query ─────────────────────────────────────────────────────────────

/// What a value is when it is written: the desktop's own three kinds
/// (`cobolt-form-host` `sql_grid_seed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlKind {
    Number,
    Boolean,
    Text,
}

struct SqlSource {
    connection: String,
    query: String,
    table: String,
    fields: Vec<(String, SqlKind)>,
    /// The key fields, in order, with their grid column.
    keys: Vec<(String, usize)>,
}

impl SqlSource {
    fn new(
        source_control_id: &str,
        fields: &[cobolt_forms::BindingField],
        key_fields: &[String],
        query: &str,
        table: &str,
        writable: bool,
        all: &[&Control],
    ) -> Result<SqlSource, String> {
        let control = all
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(source_control_id))
            .ok_or_else(|| format!("the SQL source {source_control_id} is not a control of the form"))?;
        // The backend's connection: the control's `BackendConfig`, else the
        // project connection its `SqlConnection` names. Never a connection string.
        let mut connection = web_class::backend_config(control);
        if connection.is_empty() {
            connection = control.get_prop("SqlConnection").map(|v| v.as_str().trim().to_owned()).unwrap_or_default();
        }
        if connection.is_empty() {
            return Err(format!(
                "{source_control_id} names no backend connection: set its BackendConfig (or SqlConnection) to a connection the backend knows"
            ));
        }
        let query = query.trim().trim_end_matches(';').trim().to_owned();
        if query.is_empty() {
            return Err("the SQL binding has no query".to_owned());
        }
        if query.to_ascii_uppercase().contains("END-EXEC") {
            return Err("the SQL binding's query holds END-EXEC".to_owned());
        }
        if fields.is_empty() {
            return Err("the SQL binding names no field".to_owned());
        }
        let mut out = Vec::new();
        for f in fields {
            if !plain_name(&f.name) {
                return Err(format!("field '{}' is not a plain SQL name (a letter or _, then letters, digits, _ and .)", f.name));
            }
            let kind = match f.data_type {
                BindingDataType::Integer | BindingDataType::Decimal => SqlKind::Number,
                BindingDataType::Boolean => SqlKind::Boolean,
                _ => SqlKind::Text,
            };
            out.push((f.name.clone(), kind));
        }
        let mut keys = Vec::new();
        if writable {
            if !plain_name(table) {
                return Err(format!("the table '{table}' an edit updates is not a plain SQL name"));
            }
            if key_fields.is_empty() {
                return Err("a writable SQL binding names no key field, so a row cannot be found to write it".to_owned());
            }
            for k in key_fields {
                let Some(col) = out.iter().position(|(n, _)| n.eq_ignore_ascii_case(k)) else {
                    return Err(format!("the key field {k} is not one of the grid's columns, so a row cannot be found to write it"));
                };
                keys.push((out[col].0.clone(), col));
            }
        }
        Ok(SqlSource { connection, query, table: table.to_owned(), fields: out, keys })
    }
}

/// A name the backend may put in a statement unquoted (the Guardian's rule, as
/// the desktop's `grid_edit::is_plain_name`).
fn plain_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && name.len() <= 128
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

fn write_sql_status_items(out: &mut String) {
    out.push_str("       01 SQLSTATE PIC X(5).\n");
    out.push_str("       01 SQLCODE PIC S9(9) COMP-5.\n");
    out.push_str("       01 SQLMSG PIC X(200).\n");
}

fn write_connect(out: &mut String, sql: &SqlSource, status: &str, message: &str) {
    out.push_str(&format!(
        "           EXEC SQL CONNECT TO '{}' AS RC-CONN END-EXEC\n",
        sql.connection.replace('\'', "''")
    ));
    out.push_str("           IF SQLSTATE NOT = \"00000\"\n");
    out.push_str(&format!("               MOVE \"90\" TO {status}\n"));
    out.push_str(&format!("               MOVE SQLMSG TO {message}\n"));
    out.push_str("               GOBACK\n");
    out.push_str("           END-IF\n");
}

/// The design-time query, one line of it per line, as the inner `SELECT`.
fn write_query(out: &mut String, sql: &SqlSource, pad: &str) {
    out.push_str(&format!("{pad}FROM (\n"));
    for line in sql.query.lines() {
        if !line.trim().is_empty() {
            out.push_str(&format!("{pad}    {}\n", line.trim_end()));
        }
    }
    out.push_str(&format!("{pad}) AS RC_Q\n"));
}

fn sql_read(h: &Header, program_id: &str, op: &Operation, sql: &SqlSource) -> String {
    let n = sql.fields.len();
    let mut out = String::new();
    write_head(&mut out, h, program_id);
    out.push_str("\n       DATA DIVISION.\n");
    out.push_str("       WORKING-STORAGE SECTION.\n");
    write_sql_status_items(&mut out);
    out.push_str("       01 RC-TOTAL PIC 9(9) VALUE 0.\n");
    out.push_str("       01 RC-SEEN PIC 9(9) VALUE 0.\n");
    out.push_str("       01 RC-ROW PIC 9(4) VALUE 0.\n");
    out.push_str("       01 RC-WANT PIC 9(4) VALUE 0.\n");
    out.push_str("       01 RC-DONE PIC 9 VALUE 0.\n");
    for i in 1..=n {
        out.push_str(&format!("       01 RC-V{i} PIC X({}).\n", controls::CELL_BYTES));
        out.push_str(&format!("       01 RC-I{i} PIC S9(4) COMP-5.\n"));
    }
    write_linkage(&mut out, op);
    out.push_str(&format!("\n       PROCEDURE DIVISION USING {PARAMS} RETURNING {RESULT}.\n"));
    out.push_str("       RC-MAIN.\n");
    out.push_str("           MOVE \"00\" TO READ-STATUS\n");
    out.push_str("           MOVE SPACES TO READ-MESSAGE\n");
    out.push_str("           MOVE 0 TO ROW-TOTAL ROWS-RETURNED\n");
    write_window_and_params(&mut out);
    write_connect(&mut out, sql, "READ-STATUS", "READ-MESSAGE");
    out.push_str("           EXEC SQL SELECT COUNT(*) INTO :RC-TOTAL\n");
    write_query(&mut out, sql, "               ");
    out.push_str("           END-EXEC\n");
    out.push_str("           IF SQLSTATE NOT = \"00000\"\n");
    out.push_str("               PERFORM RC-FAILED\n");
    out.push_str("               GOBACK\n");
    out.push_str("           END-IF\n");
    out.push_str("           MOVE RC-TOTAL TO ROW-TOTAL\n");
    out.push_str("      *>   The bound fields, found in the query's result by name.\n");
    out.push_str("           EXEC SQL DECLARE RC-ROWS CURSOR FOR\n");
    let names: Vec<&str> = sql.fields.iter().map(|(n, _)| n.as_str()).collect();
    out.push_str(&format!("               SELECT {}\n", names.join(", ")));
    write_query(&mut out, sql, "               ");
    out.push_str("           END-EXEC\n");
    out.push_str("           EXEC SQL OPEN RC-ROWS END-EXEC\n");
    out.push_str("           IF SQLSTATE NOT = \"00000\"\n");
    out.push_str("               PERFORM RC-FAILED\n");
    out.push_str("               GOBACK\n");
    out.push_str("           END-IF\n");
    out.push_str("           IF RC-WANT = 0\n");
    out.push_str("               MOVE 1 TO RC-DONE\n");
    out.push_str("           END-IF\n");
    out.push_str("           PERFORM UNTIL RC-DONE = 1\n");
    let vs: Vec<String> = (1..=n).map(|i| format!("RC-V{i}")).collect();
    out.push_str(&format!("               MOVE SPACES TO {}\n", vs.join(" ")));
    let into: Vec<String> = (1..=n).map(|i| format!(":RC-V{i}:RC-I{i}")).collect();
    out.push_str(&format!("               EXEC SQL FETCH RC-ROWS INTO {}\n", into.join(", ")));
    out.push_str("               END-EXEC\n");
    out.push_str("               EVALUATE TRUE\n");
    out.push_str("                   WHEN SQLSTATE = \"02000\"\n");
    out.push_str("                       MOVE 1 TO RC-DONE\n");
    out.push_str("                   WHEN SQLSTATE(1:2) = \"00\" OR SQLSTATE(1:2) = \"01\"\n");
    out.push_str("                       PERFORM RC-TAKE-ROW\n");
    out.push_str("                   WHEN OTHER\n");
    out.push_str("                       PERFORM RC-FAILED\n");
    out.push_str("                       MOVE 1 TO RC-DONE\n");
    out.push_str("               END-EVALUATE\n");
    out.push_str("           END-PERFORM\n");
    out.push_str("           EXEC SQL CLOSE RC-ROWS END-EXEC\n");
    out.push_str("           EXEC SQL DISCONNECT RC-CONN END-EXEC\n");
    out.push_str("           GOBACK.\n\n");
    out.push_str("       RC-TAKE-ROW.\n");
    out.push_str("           ADD 1 TO RC-SEEN\n");
    out.push_str("           IF RC-SEEN > ROW-OFFSET\n");
    out.push_str("               ADD 1 TO ROWS-RETURNED\n");
    out.push_str("               MOVE ROWS-RETURNED TO RC-ROW\n");
    out.push_str(&format!("               MOVE {n} TO CELL-COUNT (RC-ROW)\n"));
    for i in 1..=n {
        out.push_str(&format!("               MOVE RC-V{i} TO CELL-TEXT (RC-ROW {i})\n"));
    }
    out.push_str("               IF ROWS-RETURNED >= RC-WANT\n");
    out.push_str("                   MOVE 1 TO RC-DONE\n");
    out.push_str("               END-IF\n");
    out.push_str("           END-IF.\n\n");
    out.push_str("       RC-FAILED.\n");
    out.push_str("           MOVE \"90\" TO READ-STATUS\n");
    out.push_str("           MOVE SQLMSG TO READ-MESSAGE\n");
    out.push_str("           MOVE 0 TO ROWS-RETURNED\n");
    out.push_str("           EXEC SQL DISCONNECT RC-CONN END-EXEC.\n");
    out
}

fn sql_save(h: &Header, program_id: &str, op: &Operation, sql: &SqlSource) -> String {
    let n = sql.fields.len();
    let nkeys = sql.keys.len();
    let key_cols: Vec<usize> = sql.keys.iter().map(|(_, c)| *c).collect();
    let mut out = String::new();
    write_head(&mut out, h, program_id);
    out.push_str("\n       DATA DIVISION.\n");
    out.push_str("       WORKING-STORAGE SECTION.\n");
    write_sql_status_items(&mut out);
    out.push_str("       01 RC-CHANGE PIC 9(4) VALUE 0.\n");
    out.push_str("       01 RC-WHY PIC X(256) VALUE SPACES.\n");
    for i in 1..=n {
        out.push_str(&format!("       01 RC-V{i} PIC X({}).\n", controls::CELL_BYTES));
        out.push_str(&format!("       01 RC-I{i} PIC S9(4) COMP-5.\n"));
    }
    for i in 1..=nkeys {
        out.push_str(&format!("       01 RC-K{i} PIC X({}).\n", controls::CELL_BYTES));
    }
    write_linkage(&mut out, op);
    out.push_str(&format!("\n       PROCEDURE DIVISION USING {PARAMS} RETURNING {RESULT}.\n"));
    out.push_str("       RC-MAIN.\n");
    out.push_str("           MOVE \"00\" TO COMMIT-STATUS\n");
    out.push_str("           MOVE SPACES TO COMMIT-MESSAGE\n");
    out.push_str("           MOVE 0 TO FAILED-CHANGE\n");
    out.push_str("      *>   Every change is checked before the database is reached.\n");
    out.push_str("           PERFORM RC-CHECK-CHANGE VARYING RC-CHANGE FROM 1 BY 1\n");
    out.push_str("               UNTIL RC-CHANGE > CHANGE-COUNT OR COMMIT-STATUS NOT = \"00\"\n");
    out.push_str("           IF COMMIT-STATUS NOT = \"00\"\n");
    out.push_str("               GOBACK\n");
    out.push_str("           END-IF\n");
    write_connect(&mut out, sql, "COMMIT-STATUS", "COMMIT-MESSAGE");
    out.push_str("           PERFORM RC-APPLY-CHANGE VARYING RC-CHANGE FROM 1 BY 1\n");
    out.push_str("               UNTIL RC-CHANGE > CHANGE-COUNT OR COMMIT-STATUS NOT = \"00\"\n");
    out.push_str("      *>   One unit: all of it, or none of it.\n");
    out.push_str("           IF COMMIT-STATUS = \"00\"\n");
    out.push_str("               EXEC SQL COMMIT END-EXEC\n");
    out.push_str("               IF SQLSTATE NOT = \"00000\"\n");
    out.push_str("                   MOVE \"90\" TO COMMIT-STATUS\n");
    out.push_str("                   MOVE SQLMSG TO COMMIT-MESSAGE\n");
    out.push_str("                   EXEC SQL ROLLBACK END-EXEC\n");
    out.push_str("               END-IF\n");
    out.push_str("           ELSE\n");
    out.push_str("               EXEC SQL ROLLBACK END-EXEC\n");
    out.push_str("           END-IF\n");
    out.push_str("           EXEC SQL DISCONNECT RC-CONN END-EXEC\n");
    out.push_str("           GOBACK.\n\n");

    out.push_str("       RC-REFUSE.\n");
    out.push_str("           MOVE RC-WHY TO COMMIT-MESSAGE\n");
    out.push_str("           MOVE RC-CHANGE TO FAILED-CHANGE.\n\n");

    out.push_str("       RC-CHECK-CHANGE.\n");
    out.push_str("           MOVE SPACES TO RC-WHY\n");
    out.push_str("           EVALUATE CHANGE-ACTION (RC-CHANGE)\n");
    out.push_str("               WHEN \"UPDATE\"\n");
    write_count_check(&mut out, "KEY-COUNT", nkeys, "key cells");
    write_count_check(&mut out, "CELL-COUNT", n, "cells");
    out.push_str("                   IF RC-WHY = SPACES\n");
    out.push_str("                       PERFORM RC-CHECK-CELLS\n");
    out.push_str("                   END-IF\n");
    for (i, col) in key_cols.iter().enumerate() {
        out.push_str(&format!(
            "                   IF RC-WHY = SPACES AND FUNCTION TRIM(CELL-TEXT (RC-CHANGE {})) NOT = FUNCTION TRIM(KEY-TEXT (RC-CHANGE {}))\n",
            col + 1,
            i + 1
        ));
        out.push_str("                       MOVE \"21\" TO COMMIT-STATUS\n");
        out.push_str("                       MOVE \"the key of the row was changed; a row is found by its key\" TO RC-WHY\n");
        out.push_str("                   END-IF\n");
    }
    out.push_str("               WHEN \"DELETE\"\n");
    write_count_check(&mut out, "KEY-COUNT", nkeys, "key cells");
    out.push_str("               WHEN \"INSERT\"\n");
    write_count_check(&mut out, "CELL-COUNT", n, "cells");
    out.push_str("                   IF RC-WHY = SPACES\n");
    out.push_str("                       PERFORM RC-CHECK-CELLS\n");
    out.push_str("                   END-IF\n");
    out.push_str("               WHEN OTHER\n");
    out.push_str("                   MOVE \"the change is not UPDATE, INSERT or DELETE\" TO RC-WHY\n");
    out.push_str("           END-EVALUATE\n");
    out.push_str("           IF RC-WHY NOT = SPACES\n");
    out.push_str("               IF COMMIT-STATUS = \"00\"\n");
    out.push_str("                   MOVE \"92\" TO COMMIT-STATUS\n");
    out.push_str("               END-IF\n");
    out.push_str("               PERFORM RC-REFUSE\n");
    out.push_str("           END-IF.\n\n");

    // The desktop's checks (`grid_edit::sql_literal`): a number is a number or
    // empty (NULL), a flag one of the yes/no words; text is taken as it is.
    out.push_str("       RC-CHECK-CELLS.\n");
    for (i, (name, kind)) in sql.fields.iter().enumerate() {
        let cell = format!("CELL-TEXT (RC-CHANGE {})", i + 1);
        match kind {
            SqlKind::Number => {
                out.push_str(&format!("           IF RC-WHY = SPACES AND {cell} NOT = SPACES\n"));
                out.push_str(&format!("               IF FUNCTION TEST-NUMVAL({cell}) NOT = 0\n"));
                out.push_str(&format!("                   MOVE {} TO RC-WHY\n", lit(&format!("{name}: the value is not a number"))));
                out.push_str("               END-IF\n");
                out.push_str("           END-IF\n");
            }
            SqlKind::Boolean => {
                out.push_str("           IF RC-WHY = SPACES\n");
                out.push_str(&format!("               EVALUATE FUNCTION LOWER-CASE(FUNCTION TRIM({cell}))\n"));
                for w in SQL_YES.iter().chain(SQL_NO.iter()) {
                    out.push_str(&format!("                   WHEN {}\n", lit(w)));
                }
                out.push_str("                       CONTINUE\n");
                out.push_str("                   WHEN OTHER\n");
                out.push_str(&format!("                       MOVE {} TO RC-WHY\n", lit(&format!("{name}: the value is not a yes/no value"))));
                out.push_str("               END-EVALUATE\n");
                out.push_str("           END-IF\n");
            }
            SqlKind::Text => {}
        }
    }
    out.push_str("           CONTINUE.\n\n");

    // The values into host variables: a number left empty is NULL, a flag 1 or 0.
    out.push_str("       RC-TAKE-VALUES.\n");
    for (i, (_, kind)) in sql.fields.iter().enumerate() {
        let k = i + 1;
        let cell = format!("CELL-TEXT (RC-CHANGE {k})");
        out.push_str(&format!("           MOVE {cell} TO RC-V{k}\n"));
        out.push_str(&format!("           MOVE 0 TO RC-I{k}\n"));
        match kind {
            SqlKind::Number => {
                out.push_str(&format!("           IF RC-V{k} = SPACES\n"));
                out.push_str(&format!("               MOVE -1 TO RC-I{k}\n"));
                out.push_str("           END-IF\n");
            }
            SqlKind::Boolean => {
                out.push_str(&format!("           EVALUATE FUNCTION LOWER-CASE(FUNCTION TRIM(RC-V{k}))\n"));
                for w in SQL_YES {
                    out.push_str(&format!("               WHEN {}\n", lit(w)));
                }
                out.push_str(&format!("                   MOVE \"1\" TO RC-V{k}\n"));
                out.push_str("               WHEN OTHER\n");
                out.push_str(&format!("                   MOVE \"0\" TO RC-V{k}\n"));
                out.push_str("           END-EVALUATE\n");
            }
            SqlKind::Text => {}
        }
    }
    for i in 1..=nkeys {
        out.push_str(&format!("           MOVE KEY-TEXT (RC-CHANGE {i}) TO RC-K{i}\n"));
    }
    out.push_str("           CONTINUE.\n\n");

    let table = &sql.table;
    let where_keys: Vec<String> = sql.keys.iter().enumerate().map(|(i, (name, _))| format!("{name} = :RC-K{}", i + 1)).collect();
    let sets: Vec<String> = sql
        .fields
        .iter()
        .enumerate()
        .filter(|(i, _)| !key_cols.contains(i))
        .map(|(i, (name, _))| format!("{name} = :RC-V{k}:RC-I{k}", k = i + 1))
        .collect();
    out.push_str("       RC-APPLY-CHANGE.\n");
    out.push_str("           PERFORM RC-TAKE-VALUES\n");
    out.push_str("           EVALUATE CHANGE-ACTION (RC-CHANGE)\n");
    out.push_str("               WHEN \"UPDATE\"\n");
    if sets.is_empty() {
        out.push_str("      *>           Every column is a key: there is nothing an update can change.\n");
        out.push_str("                   MOVE \"00000\" TO SQLSTATE\n");
    } else {
        out.push_str(&format!("                   EXEC SQL UPDATE {table}\n"));
        for (i, s) in sets.iter().enumerate() {
            let lead = if i == 0 { "SET" } else { "   " };
            let comma = if i + 1 < sets.len() { "," } else { "" };
            out.push_str(&format!("                       {lead} {s}{comma}\n"));
        }
        out.push_str(&format!("                       WHERE {}\n", where_keys.join(" AND ")));
        out.push_str("                   END-EXEC\n");
    }
    out.push_str("               WHEN \"INSERT\"\n");
    let names: Vec<&str> = sql.fields.iter().map(|(n, _)| n.as_str()).collect();
    let values: Vec<String> = (1..=n).map(|k| format!(":RC-V{k}:RC-I{k}")).collect();
    out.push_str(&format!("                   EXEC SQL INSERT INTO {table} ({})\n", names.join(", ")));
    out.push_str(&format!("                       VALUES ({})\n", values.join(", ")));
    out.push_str("                   END-EXEC\n");
    out.push_str("               WHEN \"DELETE\"\n");
    out.push_str(&format!("                   EXEC SQL DELETE FROM {table}\n"));
    out.push_str(&format!("                       WHERE {}\n", where_keys.join(" AND ")));
    out.push_str("                   END-EXEC\n");
    out.push_str("           END-EVALUATE\n");
    out.push_str("           EVALUATE TRUE\n");
    out.push_str("               WHEN SQLSTATE = \"02000\"\n");
    out.push_str("                   MOVE \"23\" TO COMMIT-STATUS\n");
    out.push_str("                   MOVE \"no row has this key\" TO RC-WHY\n");
    out.push_str("                   PERFORM RC-REFUSE\n");
    out.push_str("               WHEN SQLSTATE(1:2) = \"00\" OR SQLSTATE(1:2) = \"01\"\n");
    out.push_str("                   CONTINUE\n");
    out.push_str("               WHEN OTHER\n");
    out.push_str("                   MOVE \"90\" TO COMMIT-STATUS\n");
    out.push_str("                   MOVE SQLMSG TO RC-WHY\n");
    out.push_str("                   PERFORM RC-REFUSE\n");
    out.push_str("           END-EVALUATE.\n");
    out
}

/// The desktop's yes/no words (`grid_edit::sql_literal`), lower case.
const SQL_YES: [&str; 4] = ["1", "true", "yes", "y"];
const SQL_NO: [&str; 4] = ["0", "false", "no", "n"];

/// The forms and the `.cidx` the generator's tests pin, shared with the
/// backend's tests that run what it generates (as `code_site::all_sites_fixture`).
#[doc(hidden)]
// ── A class C control's method (T11) ─────────────────────────────────────────

/// One member call on a class C control in a form's handlers — `REST-1::Get(
/// "orders")`, `INVOKE AGENT-1 "Ask" USING …` — as the web build finds it in
/// the parsed program (`cobolt_compiler::web_remote`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    /// The control, as the handler names it.
    pub control: String,
    /// The member called (the parser upper-cases a `::` member).
    pub method: String,
    /// Where it is in the form's generated program.
    pub line: u32,
    pub column: u32,
    pub args: Vec<SiteArg>,
}

/// One argument of a [`CallSite`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteArg {
    /// A literal, as text (`"orders"`, `42`).
    Literal(String),
    /// Anything else: its value is only known when the handler runs.
    Expression,
}

/// A call site the backend cannot serve, at its line of the generated program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteError {
    pub line: u32,
    pub message: String,
}

/// The control types whose catalogue rows are not member calls served here:
/// an IndexedFile's operations are the facade's `PERFORM`ed paragraphs, and
/// FileDropZone and Viewer move files through the backend's slots (T12).
const NOT_METHOD_PROGRAMS: &[&str] = &["DataGrid", "IndexedFile", "FileDropZone", "Viewer"];

/// The catalogue row a call of `method` on a control of type `class` is, when
/// the backend serves it as a generated program.
pub fn method_row(class: &str, method: &str) -> Option<&'static Operation> {
    if NOT_METHOD_PROGRAMS.iter().any(|c| c.eq_ignore_ascii_case(class)) {
        return None;
    }
    let method = method.trim().trim_matches('"').trim_matches('\'');
    controls::of_control(class).find(|r| r.serves.is_empty() && r.method.eq_ignore_ascii_case(method))
}

/// A call site the backend serves: its control, its catalogue row, its number
/// among the sites of that *(control, operation)*, and its endpoint.
pub struct ServedSite<'a> {
    pub site: &'a CallSite,
    pub control: &'a Control,
    pub row: &'static Operation,
    pub number: usize,
    /// `RC-<form>-<control>-<OP>-<number>`; an error when a name cannot make one.
    pub endpoint: Result<String, String>,
}

/// The sites of `sites` the backend serves, in source order, numbered per
/// *(control, operation)* from 1. One program per site, because the arguments
/// R17 fixes are the site's own literals. The frontend's lowering (T14) names
/// its `CALL` by this same function, so both ends agree on every name.
pub fn served_sites<'a>(form: &'a Form, sites: &'a [CallSite]) -> Vec<ServedSite<'a>> {
    let all = crate::collect_all_controls(&form.controls);
    let mut ordered: Vec<&CallSite> = sites.iter().collect();
    ordered.sort_by_key(|s| (s.line, s.column));
    let mut numbers: std::collections::HashMap<(String, &'static str), usize> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for site in ordered {
        let Some(ctrl) = all.iter().find(|c| c.id.eq_ignore_ascii_case(site.control.trim())).copied() else { continue };
        let Some(row) = method_row(ctrl.control_type.as_str(), &site.method) else { continue };
        let n = numbers.entry((ctrl.id.to_ascii_uppercase(), row.operation)).or_insert(0);
        *n += 1;
        let endpoint = endpoint(form, &ctrl.id, row.operation).map(|e| format!("{e}-{n}"));
        out.push(ServedSite { site, control: ctrl, row, number: *n, endpoint });
    }
    out
}

/// The backend programs of the class C method calls in `form`'s handlers.
/// Every site the backend cannot serve is reported, at its line.
pub fn generate_control_programs(form: &Form, sites: &[CallSite]) -> Result<Vec<BackendProgram>, Vec<SiteError>> {
    let mut out = Vec::new();
    let mut errs = Vec::new();
    for s in served_sites(form, sites) {
        let at = |m: String| SiteError { line: s.site.line, message: format!("{}: {}::{} — {m}", form.name, s.control.id, s.row.method) };
        let generated = s.endpoint.clone().and_then(|id| method_program(form, s.control, s.row, s.site, s.number, &id));
        match generated {
            Ok(p) => out.push(p),
            Err(e) => errs.push(at(e)),
        }
    }
    if errs.is_empty() {
        Ok(out)
    } else {
        Err(errs)
    }
}

/// How one argument of the method call is written in the backend program.
enum Arg {
    /// The `k`-th value the browser sends, from 1.
    Sent(usize),
    /// A literal fixed at Build (R17).
    Fixed(String),
}

fn method_program(form: &Form, ctrl: &Control, row: &'static Operation, site: &CallSite, n: usize, id: &str) -> Result<BackendProgram, String> {
    // The arguments: what R17 fixes is the handler's literal, never a value
    // the browser sends; the rest travel in ARGS, in order.
    let mut args = Vec::new();
    let mut sent = 0;
    for (i, a) in site.args.iter().enumerate() {
        let pos = (i + 1) as u8;
        match (row.fixed.iter().find(|(p, _)| *p == pos).map(|(_, f)| *f), a) {
            (None, _) => {
                sent += 1;
                args.push(Arg::Sent(sent));
            }
            (Some(controls::Fixed::Connector), _) => {
                return Err(format!(
                    "argument {pos} is a connection string, which never leaves the backend on the web; leave it out and name the backend's connection in {}",
                    web_class::BACKEND_CONFIG
                ));
            }
            (Some(controls::Fixed::File), _) => {
                return Err(format!(
                    "argument {pos} is a local file; on the web a file moves through a backend slot, which this control does not have yet (WASM-005 T12)"
                ));
            }
            (Some(_), SiteArg::Literal(text)) => args.push(Arg::Fixed(text.clone())),
            (Some(f), SiteArg::Expression) => {
                let what = if f == controls::Fixed::Statement { "the statement" } else { "what is reached" };
                return Err(format!(
                    "argument {pos} ({what}) must be a literal: on the web it is fixed when the backend program is built, never chosen by the browser (R17)"
                ));
            }
        }
    }
    let decl = declaration(ctrl)?;
    let ctrl_name = crate::cobol_word(&ctrl.id);
    let is_sql = ctrl.control_type.as_str() == "SqlDatabase";
    let needs_open = is_sql && matches!(row.operation, "QUERY" | "EXECUTE");
    let takes_rows = is_sql && row.operation == "QUERY";

    let mut out = String::new();
    for line in banner_lines() {
        out.push_str(&format!("      *>{}{line}\n", if line.is_empty() { "" } else { " " }));
    }
    out.push_str("      *> ───────────────────────────────────────────────────────────\n\n");
    out.push_str("       IDENTIFICATION DIVISION.\n");
    out.push_str(&format!("       PROGRAM-ID. {id}.\n"));
    out.push_str(&format!(
        "      *> {} of {} {} on form {} (call site {n}), run on the backend\n",
        row.method,
        ctrl.control_type.as_str(),
        ctrl.id,
        form.name
    ));
    out.push_str(&match (is_sql, decl.configuration.is_empty()) {
        (true, _) => format!("      *> over the connection '{}'.\n", sql_connection(&decl)),
        (false, false) => format!("      *> with the configuration '{}' of backend.toml.\n", decl.configuration),
        (false, true) => "      *> with its design-time properties.\n".to_owned(),
    });
    out.push_str("\n       DATA DIVISION.\n");
    out.push_str("       WORKING-STORAGE SECTION.\n");
    out.push_str("       01 RC-EVENT PIC X(64).\n");
    out.push_str("       01 RC-SOURCE PIC X(64).\n");
    out.push_str("       01 RC-BUSY PIC X(8).\n");
    out.push_str("       01 RC-WAITING PIC 9 VALUE 0.\n");
    out.push_str("       01 RC-KEEP PIC 9 VALUE 1.\n");
    if needs_open {
        out.push_str("       01 RC-OPENED PIC 9 VALUE 0.\n");
    }
    if takes_rows {
        out.push_str("       01 RC-LINE PIC X(8192).\n");
        out.push_str("       01 RC-COLS PIC 9(3) VALUE 0.\n");
        out.push_str("       01 RC-C PIC 9(3) VALUE 0.\n");
        out.push_str("       01 RC-PTR PIC 9(5) VALUE 0.\n");
        out.push_str("       01 RC-ROW PIC 9(4) VALUE 0.\n");
        out.push_str("       01 RC-DONE PIC 9 VALUE 0.\n");
    }
    write_linkage(&mut out, row);
    out.push_str(&format!("\n       PROCEDURE DIVISION USING {PARAMS} RETURNING {RESULT}.\n"));
    out.push_str("       RC-MAIN.\n");
    out.push_str("           MOVE SPACES TO RESULT-VALUE LAST-ERROR\n");
    out.push_str("           MOVE 0 TO PROP-COUNT EVENT-COUNT ROWS-RETURNED\n");
    out.push_str(&format!("           IF ARG-COUNT NOT = {sent}\n"));
    out.push_str(&format!(
        "               COBOL::\"WS-FAIL\" ( 1 {} )\n",
        lit(&format!("{id} takes {sent} argument(s)"))
    ));
    out.push_str("               GOBACK\n");
    out.push_str("           END-IF\n");
    if needs_open {
        out.push_str("      *>   No connection outlives a call: open it on the first one.\n");
        out.push_str("           IF RC-OPENED = 0\n");
        out.push_str("               MOVE 1 TO RC-OPENED\n");
        out.push_str(&format!("               {ctrl_name}::Open()\n"));
        out.push_str("               MOVE 0 TO RC-KEEP\n");
        out.push_str("               PERFORM RC-WAIT\n");
        out.push_str("               MOVE 1 TO RC-KEEP\n");
        out.push_str(&format!("               IF {ctrl_name}::StatusCode NOT = 0\n"));
        out.push_str(&format!("                   MOVE {ctrl_name}::LastError TO LAST-ERROR\n"));
        out.push_str("                   MOVE 0 TO RC-OPENED\n");
        out.push_str("                   GOBACK\n");
        out.push_str("               END-IF\n");
        out.push_str("           END-IF\n");
    }
    let written: Vec<String> = args
        .iter()
        .map(|a| match a {
            Arg::Sent(k) => format!("ARG-VALUE ( {k} )"),
            Arg::Fixed(text) => lit(text),
        })
        .collect();
    out.push_str(&format!("           MOVE {ctrl_name}::{}( {} ) TO RESULT-VALUE\n", row.method, written.join(" ")));
    out.push_str("           PERFORM RC-WAIT\n");
    if takes_rows {
        out.push_str("           PERFORM RC-TAKE-ROWS\n");
    }
    out.push_str(&format!("           MOVE {ctrl_name}::LastError TO LAST-ERROR\n"));
    out.push_str("           GOBACK.\n\n");
    out.push_str("      *>   Wait until the control has finished, as the desktop's event\n");
    out.push_str(&format!(
        "      *>   loop would, keeping the events it raised (RC-KEEP = 1). The\n      *>   backend's {} reaches this program only when no event\n",
        controls::WAIT_TICK
    ));
    out.push_str("      *>   is queued, so a tick while the control is not busy ends it.\n");
    out.push_str("       RC-WAIT.\n");
    out.push_str("           MOVE 1 TO RC-WAITING\n");
    out.push_str("           PERFORM UNTIL RC-WAITING = 0\n");
    out.push_str("               COBOL::\"WAIT-EVENT\" ( RC-EVENT RC-SOURCE )\n");
    out.push_str("               EVALUATE TRUE\n");
    out.push_str(&format!("                   WHEN RC-EVENT = {}\n", lit(controls::WAIT_TICK)));
    out.push_str(&format!("                       MOVE {ctrl_name}::Busy TO RC-BUSY\n"));
    out.push_str("                       IF RC-BUSY NOT = \"1\" AND RC-BUSY NOT = \"true\"\n");
    out.push_str("                           MOVE 0 TO RC-WAITING\n");
    out.push_str("                       END-IF\n");
    out.push_str(&format!("                   WHEN RC-KEEP = 1 AND EVENT-COUNT < {}\n", controls::MAX_EVENTS));
    out.push_str("                       ADD 1 TO EVENT-COUNT\n");
    out.push_str("                       MOVE RC-EVENT TO EVENT-NAME (EVENT-COUNT)\n");
    out.push_str("               END-EVALUATE\n");
    out.push_str("           END-PERFORM.\n");
    if takes_rows {
        out.push_str("\n      *>   No result set outlives a call: its rows travel with the answer,\n");
        out.push_str("      *>   fetched here; the fetches' own events are not the call's.\n");
        out.push_str("       RC-TAKE-ROWS.\n");
        out.push_str(&format!("           MOVE {ctrl_name}::ColumnCount() TO RC-COLS\n"));
        out.push_str(&format!("           IF RC-COLS > {}\n", controls::MAX_CELLS));
        out.push_str(&format!("               MOVE {} TO RC-COLS\n", controls::MAX_CELLS));
        out.push_str("           END-IF\n");
        out.push_str("           MOVE 0 TO RC-DONE\n");
        out.push_str(&format!("           PERFORM UNTIL RC-DONE = 1 OR ROWS-RETURNED >= {}\n", controls::MAX_WINDOW_ROWS));
        out.push_str(&format!("               MOVE {ctrl_name}::Fetch() TO RC-LINE\n"));
        out.push_str("               IF RC-LINE = SPACES\n");
        out.push_str("                   MOVE 1 TO RC-DONE\n");
        out.push_str("               ELSE\n");
        out.push_str("                   ADD 1 TO ROWS-RETURNED\n");
        out.push_str("                   MOVE ROWS-RETURNED TO RC-ROW\n");
        out.push_str("                   MOVE RC-COLS TO CELL-COUNT (RC-ROW)\n");
        out.push_str("                   MOVE 1 TO RC-PTR\n");
        out.push_str("                   PERFORM VARYING RC-C FROM 1 BY 1 UNTIL RC-C > RC-COLS\n");
        out.push_str("                       UNSTRING RC-LINE DELIMITED BY X\"09\"\n");
        out.push_str("                           INTO CELL-TEXT (RC-ROW RC-C) WITH POINTER RC-PTR\n");
        out.push_str("                       END-UNSTRING\n");
        out.push_str("                   END-PERFORM\n");
        out.push_str("               END-IF\n");
        out.push_str("           END-PERFORM\n");
        out.push_str("           MOVE 0 TO RC-KEEP\n");
        out.push_str("           PERFORM RC-WAIT\n");
        out.push_str("           MOVE 1 TO RC-KEEP.\n");
    }
    let mut p = program(id.to_owned(), out, &web_class::backend_roles(ctrl), false);
    p.control = Some(decl);
    Ok(p)
}

// ── The frontend's half of a class C method call (T14) ───────────────────────

/// The loop counter of a lowered call's answer.
pub const FRONTEND_INDEX: &str = "RC-I";
/// Where a lowered call's failure is read into (`COBOL::"WS-LAST-ERROR"`).
pub const FRONTEND_ERR_CLASS: &str = "RC-ERR-CLASS";
pub const FRONTEND_ERR_CODE: &str = "RC-ERR-CODE";
pub const FRONTEND_ERR_MESSAGE: &str = "RC-ERR-MESSAGE";

/// The group a lowered call sends (`RC-PARAMS`) and the one its answer comes
/// back in (`RC-RESULT`): the names the backend program's LINKAGE gives them.
pub fn frontend_groups() -> (&'static str, &'static str) {
    (PARAMS, RESULT)
}

/// The item that holds the value of the `n`-th lowered call site (from 1)
/// until the statement that used it has run.
pub fn frontend_value(n: usize) -> String {
    format!("RC-VALUE-{n}")
}

/// Every name the frontend's declarations take: a developer's item named like
/// one would be ambiguous beside them, so the web build refuses the form.
pub fn frontend_names(row: &Operation) -> Vec<String> {
    let mut out = linkage_names(row);
    out.extend([FRONTEND_INDEX, FRONTEND_ERR_CLASS, FRONTEND_ERR_CODE, FRONTEND_ERR_MESSAGE].map(str::to_owned));
    out
}

/// The WORKING-STORAGE a web form's program gains for its class C method
/// calls (WASM-005 T14): the parameter and result groups of `row` — every
/// method row shares them — under the names the generated backend program's
/// LINKAGE uses, so the JSON members match by name (R5, R6); the loop counter,
/// the failure's three items, and one value item per call site. All `GLOBAL`:
/// the form's handlers are programs nested in it, and one task runs at a time
/// (events are queued while a call is pending, R9), so one pair of groups
/// serves every call. The web build parses this into the form's program; the
/// developer's source is never touched.
pub fn frontend_working_storage(row: &Operation, sites: usize) -> String {
    let mut out = String::new();
    out.push_str("      *> The groups the class C controls' calls travel in on the web,\n");
    out.push_str("      *> generated by PowerRustCOBOL RAD on every Build (WASM-005).\n");
    for (name, fields) in [(PARAMS, row.params), (RESULT, row.result)] {
        out.push_str(&format!("       01 {name} GLOBAL.\n"));
        write_fields(&mut out, fields, 5, 10);
    }
    out.push_str(&format!("       01 {FRONTEND_INDEX} PIC 9(2) GLOBAL.\n"));
    out.push_str(&format!("       01 {FRONTEND_ERR_CLASS} PIC X(16) GLOBAL.\n"));
    out.push_str(&format!("       01 {FRONTEND_ERR_CODE} PIC 9(5) GLOBAL.\n"));
    out.push_str(&format!("       01 {FRONTEND_ERR_MESSAGE} PIC X(512) GLOBAL.\n"));
    for n in 1..=sites {
        out.push_str(&format!("       01 {} PIC X({}) GLOBAL.\n", frontend_value(n), controls::ARG_BYTES));
    }
    out
}

/// The SQL connection a SqlDatabase reaches on the backend.
fn sql_connection(decl: &ControlDeclaration) -> &str {
    decl.properties.iter().find(|(k, _)| k == "SqlConnection").map(|(_, v)| v.as_str()).unwrap_or("")
}

/// A connection string or a key typed into a control of a web or mobile form
/// (R14, T13): the control and the property — there is no field for the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretOnForm {
    pub control: String,
    pub property: String,
}

/// Every secret-bearing property (the catalogue's `secrets_of` its control
/// type) that a control of `form` holds a value in. The structural check
/// beside the bundle's shape scan: a connection string without a password,
/// which no shape gives away, is found as surely as a key. The value the
/// model itself seeds (a SqlDatabase's `sqlite::memory:`) names no server and
/// holds no secret, so it is not one.
pub fn secrets_on_form(form: &Form) -> Vec<SecretOnForm> {
    let mut out = Vec::new();
    for ctrl in crate::collect_all_controls(&form.controls) {
        let secrets = controls::secrets_of(ctrl.control_type.as_str());
        if secrets.is_empty() {
            continue;
        }
        let seeded = Control::new("SEED", ctrl.control_type.clone(), 0, 0);
        for name in secrets {
            let value = ctrl.get_prop(name).map(|v| v.to_xml_string()).unwrap_or_default();
            let default = seeded.get_prop(name).map(|v| v.to_xml_string()).unwrap_or_default();
            if !value.trim().is_empty() && value != default {
                out.push(SecretOnForm { control: ctrl.id.clone(), property: (*name).to_owned() });
            }
        }
    }
    out
}

/// What the backend seeds for `ctrl`: its design-time properties as the
/// desktop host seeds them, without its secrets and without the two Backend
/// properties, which are the Build's. A SqlDatabase reaches a connection of
/// the backend's SQL catalogue by name — its `BackendConfig`, else its
/// `SqlConnection` — as a bound grid does (T9); it needs no configuration.
fn declaration(ctrl: &Control) -> Result<ControlDeclaration, String> {
    let class = ctrl.control_type.as_str();
    let secrets = controls::secrets_of(class);
    let mut properties: Vec<(String, String)> = ctrl
        .properties
        .iter()
        .filter(|(k, _)| {
            !secrets.iter().any(|s| s.eq_ignore_ascii_case(k))
                && !k.eq_ignore_ascii_case(web_class::BACKEND_CONFIG)
                && !k.eq_ignore_ascii_case(web_class::BACKEND_ROLES)
        })
        .map(|(k, v)| (k.clone(), cobolt_forms::model::property_runtime_text(class, k, &v.to_xml_string())))
        .collect();
    properties.push(("Name".into(), ctrl.id.clone()));
    let mut configuration = web_class::backend_config(ctrl);
    if class == "SqlDatabase" {
        let named = if configuration.is_empty() {
            ctrl.get_prop("SqlConnection").map(|v| v.as_str().trim().to_owned()).unwrap_or_default()
        } else {
            std::mem::take(&mut configuration)
        };
        if named.is_empty() {
            return Err(format!(
                "{} names no backend connection: set its {} (or SqlConnection) to a connection of the backend",
                ctrl.id,
                web_class::BACKEND_CONFIG
            ));
        }
        properties.retain(|(k, _)| !k.eq_ignore_ascii_case("SqlConnection"));
        properties.push(("SqlConnection".into(), named));
    }
    Ok(ControlDeclaration { id: ctrl.id.clone(), class: class.to_owned(), configuration, properties })
}

pub mod fixtures {
    use cobolt_forms::{
        BindingDataType, BindingField, BindingMode, BindingSourceDescriptor, BindingTargetDescriptor, Control,
        ControlType, DataBindingDef, Form, PropValue,
    };
    use cobolt_indexed::{
        FieldUsage, IndexedDefinition, IndexedField, KeyDef, KeyEncodingDef, KeyOrderingDef, KeyPartDef, KeySchema,
        RecordFormatDef,
    };

    /// The path the fixture forms store for [`customers_cidx`].
    pub const CIDX_PATH: &str = "indexed/customers.cidx";

    fn leaf(level: u8, name: &str, pic: &str, usage: FieldUsage, offset: u32, length: u32) -> IndexedField {
        IndexedField {
            level,
            name: name.into(),
            pic: pic.into(),
            usage,
            offset: Some(offset),
            length: Some(length),
            comment: String::new(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: Vec::new(),
        }
    }

    /// A customer file: a numeric key, a name, a signed packed balance and an
    /// indicator — one field of each kind a cell is made from.
    pub fn customers_cidx(assign_path: &str) -> IndexedDefinition {
        let mut def = IndexedDefinition::new("CUSTOMER-FILE", assign_path);
        def.record_format = RecordFormatDef::Fixed { length: 30 };
        def.keys = KeySchema {
            primary: KeyDef {
                name: Some("CUST-ID".into()),
                parts: vec![KeyPartDef { field_name: "CUST-ID".into(), offset: 0, length: 5, encoding: KeyEncodingDef::Bytes }],
                duplicates_allowed: false,
                ordering: KeyOrderingDef::Ascending,
            },
            alternates: Vec::new(),
        };
        let mut root = leaf(1, "CUSTOMER-REC", "", FieldUsage::Display, 0, 30);
        root.offset = None;
        root.length = None;
        root.children = vec![
            leaf(5, "CUST-ID", "9(5)", FieldUsage::Display, 0, 5),
            leaf(5, "CUST-NAME", "X(20)", FieldUsage::Display, 5, 20),
            leaf(5, "CUST-BALANCE", "S9(5)V99", FieldUsage::Comp3, 25, 4),
            leaf(5, "CUST-ACTIVE", "9", FieldUsage::Display, 29, 1),
        ];
        def.fields = vec![root];
        def
    }

    fn field(name: &str, t: BindingDataType) -> BindingField {
        BindingField::new(name, t)
    }

    /// `ORDERS`: an indexed-file grid (writable) and an SQL grid (writable);
    /// `REPORT`: a read-only SQL grid.
    pub fn grid_forms() -> Vec<Form> {
        let mut orders = Form::new("ORDERS", "Orders", 800, 600);
        orders.controls.push(Control::new("GRID-CUST", ControlType::DataGrid, 10, 10));
        orders.controls.push(Control::new("GRID-ITEMS", ControlType::DataGrid, 10, 300));
        let mut db = Control::new("DB-SALES", ControlType::SqlDatabase, 0, 0);
        db.set_prop(cobolt_forms::web_class::BACKEND_CONFIG, PropValue::String("SALES".into()));
        orders.controls.push(db);
        orders.controls[0].set_prop(cobolt_forms::web_class::BACKEND_ROLES, PropValue::String("clerk, manager".into()));
        let mut ix = DataBindingDef::new(
            "customers",
            "Customers",
            BindingSourceDescriptor::IndexedFile {
                definition_path: CIDX_PATH.into(),
                record_name: "CUSTOMER-REC".into(),
                fields: vec![
                    field("CUST-ID", BindingDataType::Integer).key(),
                    field("CUST-NAME", BindingDataType::Text),
                    field("CUST-BALANCE", BindingDataType::Decimal),
                    field("CUST-ACTIVE", BindingDataType::Boolean),
                ],
                key_field: Some("CUST-ID".into()),
                writable: true,
            },
            BindingTargetDescriptor::DataGrid { control_id: "GRID-CUST".into() },
        );
        ix.mode = BindingMode::Writable;
        orders.data_bindings.push(ix);
        let mut sql = DataBindingDef::new(
            "items",
            "Items",
            BindingSourceDescriptor::Sql {
                source_control_id: "DB-SALES".into(),
                query_name: "items".into(),
                result_set_name: "items".into(),
                fields: vec![
                    field("ID", BindingDataType::Integer).key(),
                    field("NAME", BindingDataType::Text),
                    field("PRICE", BindingDataType::Decimal),
                    field("IN_STOCK", BindingDataType::Boolean),
                ],
                key_fields: vec!["ID".into()],
                writable: true,
                query: "SELECT ID, NAME, PRICE, IN_STOCK\nFROM ITEMS\nORDER BY ID;".into(),
                table_name: "ITEMS".into(),
            },
            BindingTargetDescriptor::DataGrid { control_id: "GRID-ITEMS".into() },
        );
        sql.mode = BindingMode::Writable;
        orders.data_bindings.push(sql);

        let mut report = Form::new("REPORT", "Report", 800, 600);
        report.controls.push(Control::new("GRID-TOTALS", ControlType::DataGrid, 10, 10));
        let mut db = Control::new("DB-SALES", ControlType::SqlDatabase, 0, 0);
        db.set_prop("SqlConnection", PropValue::String("SALES".into()));
        report.controls.push(db);
        report.data_bindings.push(DataBindingDef::new(
            "totals",
            "Totals",
            BindingSourceDescriptor::Sql {
                source_control_id: "DB-SALES".into(),
                query_name: "totals".into(),
                result_set_name: "totals".into(),
                fields: vec![field("NAME", BindingDataType::Text), field("TOTAL", BindingDataType::Decimal)],
                key_fields: Vec::new(),
                writable: false,
                query: "SELECT NAME, SUM(PRICE) AS TOTAL FROM ITEMS GROUP BY NAME".into(),
                table_name: String::new(),
            },
            BindingTargetDescriptor::DataGrid { control_id: "GRID-TOTALS".into() },
        ));
        vec![orders, report]
    }

    /// `SHOP`: a RestClient, an AgentObject, a SqlDatabase and a DynamoDB
    /// control, each with a configuration name, and the calls its handlers
    /// make (T11) — the RestClient's `Get` twice, so two sites of one
    /// operation are numbered.
    pub fn control_form() -> (Form, Vec<super::CallSite>) {
        use super::{CallSite, SiteArg};
        let mut shop = Form::new("SHOP", "Shop", 800, 600);
        let mut rest = Control::new("REST-1", ControlType::RestClient, 0, 0);
        rest.set_prop("BaseURL", PropValue::String("https://api.example.test/v1".into()));
        rest.set_prop("AuthType", PropValue::String("Bearer".into()));
        rest.set_prop("AuthToken", PropValue::String("typed-on-the-form".into()));
        rest.set_prop(cobolt_forms::web_class::BACKEND_CONFIG, PropValue::String("CRM".into()));
        rest.set_prop(cobolt_forms::web_class::BACKEND_ROLES, PropValue::String("clerk".into()));
        shop.controls.push(rest);
        let mut agent = Control::new("AGENT-1", ControlType::AgentObject, 0, 0);
        agent.set_prop(cobolt_forms::web_class::BACKEND_CONFIG, PropValue::String("ASSISTANT".into()));
        shop.controls.push(agent);
        let mut db = Control::new("DB-1", ControlType::SqlDatabase, 0, 0);
        db.set_prop(cobolt_forms::web_class::BACKEND_CONFIG, PropValue::String("SALES".into()));
        shop.controls.push(db);
        shop.controls.push(Control::new("ORDERS-TABLE", ControlType::AwsDynamoDB, 0, 0));
        // A FileDropZone makes an upload slot, never a method program (T12).
        let mut drop = Control::new("DROP-1", ControlType::FileDropZone, 0, 0);
        drop.set_prop("AllowedExtensions", PropValue::String("csv, .XLSX".into()));
        drop.set_prop("MaximumFileSizeKB", PropValue::Int(64));
        drop.set_prop(cobolt_forms::web_class::BACKEND_ROLES, PropValue::String("clerk".into()));
        shop.controls.push(drop);
        let site =|control: &str, method: &str, line: u32, args: Vec<SiteArg>| CallSite {
            control: control.into(),
            method: method.into(),
            line,
            column: 12,
            args,
        };
        let text = |t: &str| SiteArg::Literal(t.into());
        let sites = vec![
            // Listed out of order on purpose: numbering follows the source.
            site("REST-1", "Get", 40, vec![text("customers/7")]),
            site("REST-1", "get", 20, vec![text("customers")]),
            site("REST-1", "Post", 30, vec![text("orders"), SiteArg::Expression]),
            site("AGENT-1", "Ask", 50, vec![SiteArg::Expression]),
            site("DB-1", "Query", 60, vec![text("SELECT NAME, PRICE FROM ITEMS ORDER BY NAME")]),
            site("ORDERS-TABLE", "GetItem", 70, vec![SiteArg::Expression, SiteArg::Expression]),
            // Not a class C member: no program.
            site("REST-1", "Caption", 80, Vec::new()),
        ];
        (shop, sites)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_places_reads_the_digits_after_the_point() {
        assert_eq!(decimal_places("9(5)V99"), 2);
        assert_eq!(decimal_places("S9(7)V9(3)"), 3);
        assert_eq!(decimal_places("9(4)"), 0);
        assert_eq!(decimal_places("SV999"), 3);
    }

    #[test]
    fn a_generated_file_is_recognised_by_its_banner() {
        let mut s = String::new();
        write_head(
            &mut s,
            &Header { form: "F", grid: "G", binding: "b", op: "READ".into(), source: "x".into() },
            "RC-1-F-1-G-READ",
        );
        assert!(is_generated(&s));
        assert!(is_generated(&GeneratedBackend::default().exports_toml()));
        assert!(!is_generated("       IDENTIFICATION DIVISION.\n"));
    }

    #[test]
    fn the_frontend_groups_are_the_backend_programs_linkage_made_global_005() {
        let row = controls::find("RestClient", "GET").unwrap();
        let ws = frontend_working_storage(row, 2);
        // The same members, under the same group names, as `write_linkage`.
        let mut linkage = String::new();
        write_linkage(&mut linkage, row);
        let members = |t: &str| t.lines().filter(|l| l.trim_start().starts_with(['0', '1'])).map(|l| l.trim().replace(" GLOBAL", "")).collect::<Vec<_>>();
        let front: Vec<String> = members(&ws).into_iter().take_while(|l| !l.starts_with("01 RC-I")).collect();
        assert_eq!(front, members(&linkage), "{ws}");
        assert!(ws.contains("       01 RC-PARAMS GLOBAL.\n") && ws.contains("       01 RC-RESULT GLOBAL.\n"));
        assert!(ws.contains("01 RC-VALUE-1 PIC X(2048) GLOBAL.") && ws.contains("01 RC-VALUE-2 PIC X(2048) GLOBAL."));
        assert!(!ws.contains("RC-VALUE-3"));
        let names = frontend_names(row);
        for n in ["RC-PARAMS", "ARG-COUNT", "EVENTS", "EVENT-NAME", "CELL-TEXT", "RC-I", "RC-ERR-MESSAGE"] {
            assert!(names.iter().any(|x| x == n), "{n} is reserved");
        }
    }
}
