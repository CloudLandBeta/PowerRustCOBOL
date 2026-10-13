// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The items a DataGrid **Dropdown** column offers (spec 095 R15).
//!
//! A Dropdown cell holds an item's *value* and shows its *display text*. The
//! list is described on the column (`DataGridColumn.dropdown`, inside the grid's
//! `AdvancedGrid`); where it can be read here, this module reads it and publishes
//! it for the renderer as the runtime-only property `_ColumnItems` — one line per
//! item, `<column id> TAB <value> TAB <display text>`.
//!
//! * **COBOL table** — the two OCCURS items the binding names, read the way
//!   `table(i)` reads them; an occurrence with neither filled is left out.
//! * **Indexed file** — the two fields of the records of the `.cidx` the binding
//!   names, formatted as the grid formats them, so a stored value and a list value
//!   spell the same.
//! * **Static items** need no interpreter: the renderer reads them off the column.
//! * **SQL control / REST** lists are kept by the editor but not resolved here; the
//!   column then has no items and the cell shows its stored value as it always did.
//!
//! A source that cannot be read (a table the form does not have, a missing data
//! file) gives no items and no error: the grid is still a grid.

use cobolt_forms::model::{DataGridDropdown, DataGridDropdownOrigin};

use super::Interpreter;
use crate::indexed_ide::GridSession;

/// The runtime-only property the items are published in.
const COLUMN_ITEMS: &str = "_ColumnItems";

/// One item as `(value, display text)`.
type Item = (String, String);

impl Interpreter {
    /// Read every Dropdown column's list the runtime can resolve and publish them
    /// as `_ColumnItems`. Called wherever a grid's binding is reloaded.
    pub(super) fn refresh_dropdown_items(&mut self, control_id: &str) {
        let columns = self.dropdown_columns(control_id);
        let mut lines = Vec::new();
        for (column_id, dropdown) in &columns {
            let items = match dropdown.origin {
                DataGridDropdownOrigin::CobolTable => self.table_items(dropdown),
                DataGridDropdownOrigin::IndexedFile => self.indexed_items(dropdown),
                _ => continue,
            };
            let limit = if dropdown.limit == 0 { usize::MAX } else { dropdown.limit as usize };
            for (value, text) in items.into_iter().take(limit) {
                lines.push(format!("{column_id}\t{}\t{}", one_line(&value), one_line(&text)));
            }
        }
        // A grid that never had a list keeps no property at all.
        if lines.is_empty() && self.obj_get(control_id, COLUMN_ITEMS).is_empty() {
            return;
        }
        self.obj_set(control_id, COLUMN_ITEMS, lines.join("\n"));
    }

    /// The grid's Dropdown columns that name a list: `(column id, list)`.
    fn dropdown_columns(&self, control_id: &str) -> Vec<(String, DataGridDropdown)> {
        let Some(advanced) = self.datagrid_advanced_json(control_id) else {
            return Vec::new();
        };
        let Some(columns) = advanced.get("columns").and_then(|columns| columns.as_array()) else {
            return Vec::new();
        };
        columns
            .iter()
            .filter(|column| {
                let text = |key: &str| column.get(key).and_then(|v| v.as_str()).unwrap_or_default().trim();
                let kind = if text("control_kind").is_empty() { text("edit_control") } else { text("control_kind") };
                kind.eq_ignore_ascii_case("dropdown")
            })
            .filter_map(|column| {
                let id = column.get("id")?.as_str()?.to_owned();
                let list = serde_json::from_value::<DataGridDropdown>(column.get("dropdown")?.clone()).ok()?;
                Some((id, list))
            })
            .collect()
    }

    /// The items of a COBOL table: occurrence *i* of `value_field` and of
    /// `display_field`. Either may stand for both.
    fn table_items(&self, dropdown: &DataGridDropdown) -> Vec<Item> {
        let (value_field, display_field) = both_fields(dropdown);
        if value_field.is_empty() {
            return Vec::new();
        }
        let read = |name: &str, index: usize| -> String {
            let name = self.env.resolve_name(name, &[]);
            let key = crate::environment::subscript_key(&name, &[index as i64]);
            self.env.get(&key).map(|value| value.as_display_string().trim_end().to_owned()).unwrap_or_default()
        };
        let count = [value_field, display_field]
            .into_iter()
            .filter_map(|field| self.env.symbol(&self.env.resolve_name(field, &[])))
            .filter_map(|symbol| symbol.dims.last().copied())
            .max()
            .unwrap_or(0);
        (1..=count)
            .map(|index| (read(value_field, index), read(display_field, index)))
            .filter(|(value, text)| !value.trim().is_empty() || !text.trim().is_empty())
            .collect()
    }

    /// The items of an indexed file: every record's `value_field` and
    /// `display_field`, in the file's order.
    fn indexed_items(&self, dropdown: &DataGridDropdown) -> Vec<Item> {
        let (value_field, display_field) = both_fields(dropdown);
        if dropdown.source.trim().is_empty() || value_field.is_empty() {
            return Vec::new();
        }
        let def_file = cobolt_forms::assets::resolve(dropdown.source.trim());
        let Ok(def) = cobolt_indexed::load_indexed(&def_file) else {
            return Vec::new();
        };
        let leaves = def.record_root().map(|root| root.all_leaves()).unwrap_or_default();
        let leaf = |name: &str| leaves.iter().find(|leaf| leaf.name.eq_ignore_ascii_case(name));
        let (Some(value_leaf), Some(display_leaf)) = (leaf(value_field), leaf(display_field)) else {
            return Vec::new();
        };
        let data_path = cobolt_forms::assets::resolve(&def.assign_path);
        // `GridSession::open` opens the file for update and would create it; a list
        // is only ever read, so a file that is not there is an empty list.
        if !data_path.exists() {
            return Vec::new();
        }
        let Ok(session) = GridSession::open(&def, &data_path) else {
            return Vec::new();
        };
        let read = |record: &[u8], leaf: &cobolt_indexed::IndexedField| -> String {
            let start = (leaf.offset.unwrap_or(0) as usize).min(record.len());
            let end = (start + leaf.length.unwrap_or(0) as usize).min(record.len());
            cobolt_indexed::format_field_display(leaf, &record[start..end]).trim_end().to_owned()
        };
        session
            .rows()
            .iter()
            .map(|record| (read(record, value_leaf), read(record, display_leaf)))
            .filter(|(value, text)| !value.trim().is_empty() || !text.trim().is_empty())
            .collect()
    }
}

/// `(value field, display field)`, each standing for the other when one is not named.
fn both_fields(dropdown: &DataGridDropdown) -> (&str, &str) {
    let value = dropdown.value_field.trim();
    let display = dropdown.display_field.trim();
    match (value.is_empty(), display.is_empty()) {
        (true, true) => ("", ""),
        (true, false) => (display, display),
        (false, true) => (value, value),
        (false, false) => (value, display),
    }
}

/// A value or text on one line, so a tab or a line break inside it cannot split an item.
fn one_line(text: &str) -> String {
    text.chars().map(|ch| if matches!(ch, '\t' | '\n' | '\r') { ' ' } else { ch }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_indexed::{
        AccessMode, FieldUsage, IndexedDefinition, IndexedField, KeyDef, KeyEncodingDef, KeyOrderingDef, KeyPartDef,
        RecordFormatDef, StorageMode,
    };
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse;
    use serde_json::json;

    use crate::indexed::{status, KeySpec, OpenMode};
    use crate::indexed_disk::DiskIndexedFile;

    const ITEMS: &str = "_ColumnItems";

    /// An interpreter for a program that declares `data` and does nothing.
    fn interpreter(data: &str) -> Interpreter {
        let source = format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{data}\nPROCEDURE DIVISION.\nMAIN.\n    STOP RUN.\n"
        );
        Interpreter::new(parse(tokenize(&source, SourceFormat::Free)).program.expect("program"))
    }

    /// The `AdvancedGrid` of a grid with a plain column and the given Dropdown columns.
    fn advanced(dropdowns: &[(&str, serde_json::Value)]) -> String {
        let mut columns = vec![json!({"id": "Name", "title": "Name", "source_name": "Name", "value_type": "string", "width": 100.0})];
        for (id, list) in dropdowns {
            columns.push(json!({
                "id": id, "title": id, "source_name": id, "value_type": "string", "width": 100.0,
                "control_kind": "Dropdown", "dropdown": list,
            }));
        }
        json!({ "columns": columns }).to_string()
    }

    fn table_list(limit: u32) -> serde_json::Value {
        json!({"origin": "CobolTable", "source": "WS-CATEGORY-TABLE", "value_field": "CATEGORY-ID",
               "display_field": "CATEGORY-NAME", "limit": limit})
    }

    const CATEGORIES: &str = "\
01 WS-CATEGORY-TABLE.
   05 WS-CATEGORY-ROW OCCURS 5 TIMES.
      10 CATEGORY-ID   PIC X(04).
      10 CATEGORY-NAME PIC X(20).
01 WS-ACTOR-TABLE.
   05 WS-ACTOR-ROW OCCURS 3 TIMES.
      10 ACTOR-NAME     PIC X(20).
      10 ACTOR-CATEGORY PIC X(04).";

    /// A form with a categories table (occurrences 1, 2 and 4 filled) and a grid bound to
    /// an actors table whose `Category` column is a Dropdown over the categories.
    fn form(columns: &[(&str, serde_json::Value)]) -> Interpreter {
        let mut interp = interpreter(CATEGORIES);
        interp.seed_objects([(
            "Grid".to_owned(),
            "DataGrid".to_owned(),
            vec![
                ("_BindingKind".to_owned(), "CobolTable".to_owned()),
                ("_BindingFields".to_owned(), "ACTOR-NAME\nACTOR-CATEGORY".to_owned()),
                ("AdvancedGrid".to_owned(), advanced(columns)),
            ],
        )]);
        for (n, id, name) in [(1, "D", "Drama"), (2, "C", "Crime"), (4, "T", "Thriller")] {
            interp.env.set_str(&format!("CATEGORY-ID({n})"), id);
            interp.env.set_str(&format!("CATEGORY-NAME({n})"), name);
        }
        interp.env.set_str("ACTOR-NAME(1)", "Al Pacino");
        interp.env.set_str("ACTOR-CATEGORY(1)", "C");
        interp
    }

    /// The items a COBOL table gives a Dropdown column: occurrence by occurrence, the
    /// blank one left out, published with the grid's own rows by `RefreshBinding`.
    #[test]
    fn a_cobol_table_gives_its_filled_occurrences_as_items() {
        let mut interp = form(&[("Category", table_list(0))]);
        assert_eq!(interp.refresh_binding("Grid"), 1, "the grid still loads its row");
        assert_eq!(interp.obj_get("Grid", "Rows"), "Al Pacino\tC");
        assert_eq!(
            interp.obj_get("Grid", ITEMS),
            "Category\tD\tDrama\nCategory\tC\tCrime\nCategory\tT\tThriller",
            "occurrence 3 is blank and is not an item"
        );
        println!("\n  _ColumnItems from a COBOL table: 3 items from 5 occurrences (1, 2, 4 filled)\n");
    }

    /// A list shows at most `limit` items; 0 is no limit.
    #[test]
    fn the_limit_caps_the_items() {
        let mut interp = form(&[("Category", table_list(2))]);
        interp.refresh_binding("Grid");
        assert_eq!(interp.obj_get("Grid", ITEMS), "Category\tD\tDrama\nCategory\tC\tCrime");
    }

    /// The items follow the table: reloading after the table changed gives the new list, and
    /// a list that ends up empty clears the property instead of leaving the old one.
    #[test]
    fn a_reload_publishes_the_table_as_it_is_now() {
        let mut interp = form(&[("Category", table_list(0))]);
        interp.refresh_binding("Grid");
        interp.env.set_str("CATEGORY-NAME(2)", "Heist");
        interp.refresh_binding("Grid");
        assert!(interp.obj_get("Grid", ITEMS).contains("Category\tC\tHeist"));
        for n in [1, 2, 4] {
            interp.env.set_str(&format!("CATEGORY-ID({n})"), "");
            interp.env.set_str(&format!("CATEGORY-NAME({n})"), "");
        }
        interp.refresh_binding("Grid");
        assert_eq!(interp.obj_get("Grid", ITEMS), "", "an emptied table is an empty list");
    }

    /// A list the runtime cannot read gives no items and no error; a grid with no Dropdown
    /// column never gets the property at all.
    #[test]
    fn an_unreadable_or_unresolved_list_gives_no_items() {
        let missing = json!({"origin": "CobolTable", "source": "NO-SUCH-TABLE", "value_field": "NO-ID", "display_field": "NO-NAME"});
        let sql = json!({"origin": "SqlControl", "source": "Db", "value_field": "ID", "display_field": "NAME"});
        let rest = json!({"origin": "RestApi", "source": "Api", "value_field": "id", "display_field": "name"});
        let statics = json!({"origin": "Static", "items": "A=Alpha\nB=Beta"});
        let no_file = json!({"origin": "IndexedFile", "source": "/no/such/file.cidx", "value_field": "ID", "display_field": "NAME"});
        let mut interp = form(&[("A", missing), ("B", sql), ("C", rest), ("D", statics), ("E", no_file)]);
        interp.refresh_binding("Grid");
        assert_eq!(interp.obj_get("Grid", ITEMS), "", "nothing to publish");
        assert_eq!(interp.obj_get("Grid", "Rows"), "Al Pacino\tC", "the grid itself is untouched");

        let mut plain = form(&[]);
        plain.refresh_binding("Grid");
        assert_eq!(plain.obj_get("Grid", ITEMS), "");
    }

    /// Only a column of kind Dropdown is read, whichever field names the kind; two Dropdown
    /// columns each keep their own items.
    #[test]
    fn each_dropdown_column_has_its_own_items_and_other_columns_have_none() {
        let mut interp = form(&[("Category", table_list(1)), ("Genre", table_list(0))]);
        // A third column that has a list configured but is a plain Textbox.
        let mut grid: serde_json::Value = serde_json::from_str(&interp.obj_get("Grid", "AdvancedGrid")).unwrap();
        grid["columns"].as_array_mut().unwrap().push(json!({
            "id": "Notes", "title": "Notes", "source_name": "Notes", "value_type": "string", "width": 80.0,
            "control_kind": "", "edit_control": "Textbox", "dropdown": table_list(0),
        }));
        // …and one whose kind is named by `edit_control` alone.
        grid["columns"].as_array_mut().unwrap().push(json!({
            "id": "Mood", "title": "Mood", "source_name": "Mood", "value_type": "string", "width": 80.0,
            "edit_control": "Dropdown", "dropdown": table_list(1),
        }));
        interp.obj_set("Grid", "AdvancedGrid", grid.to_string());
        interp.refresh_binding("Grid");
        let items = interp.obj_get("Grid", ITEMS);
        let lines: Vec<&str> = items.lines().collect();
        assert_eq!(
            lines,
            ["Category\tD\tDrama", "Genre\tD\tDrama", "Genre\tC\tCrime", "Genre\tT\tThriller", "Mood\tD\tDrama"]
        );
    }

    /// A tab or a line break inside a value cannot split an item.
    #[test]
    fn a_tab_or_a_line_break_in_an_item_does_not_split_it() {
        assert_eq!(one_line("a\tb\nc\rd"), "a b c d");
    }

    // ── an indexed file ──────────────────────────────────────────────────────

    fn leaf(name: &str, pic: &str, offset: u32, length: u32) -> IndexedField {
        IndexedField {
            level: 5,
            name: name.to_owned(),
            pic: pic.to_owned(),
            usage: FieldUsage::Display,
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

    /// A lookup file `COUNTRY-ID 9(3)` (key) · `COUNTRY-NAME X(12)` with `records`, and the
    /// path of its `.cidx`.
    fn lookup_file(dir: &std::path::Path, records: &[(&str, &str)]) -> String {
        let data = dir.join("country.idx");
        let cidx = dir.join("country.cidx");
        let (id, name) = (leaf("COUNTRY-ID", "9(3)", 0, 3), leaf("COUNTRY-NAME", "X(12)", 3, 12));
        let root = IndexedField {
            level: 1,
            name: "COUNTRY-RECORD".to_owned(),
            pic: String::new(),
            usage: FieldUsage::Display,
            offset: None,
            length: None,
            comment: String::new(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: vec![id.clone(), name.clone()],
        };
        let mut def = IndexedDefinition::new("COUNTRY-FILE", data.to_str().unwrap());
        def.record_format = RecordFormatDef::Fixed { length: 15 };
        def.storage = StorageMode::Disk;
        def.access_mode = AccessMode::Dynamic;
        def.keys.primary = KeyDef {
            name: Some("COUNTRY-ID".into()),
            parts: vec![KeyPartDef { field_name: "COUNTRY-ID".into(), offset: 0, length: 3, encoding: KeyEncodingDef::Bytes }],
            duplicates_allowed: false,
            ordering: KeyOrderingDef::Ascending,
        };
        def.fields = vec![root];
        def.finalized = true;
        cobolt_indexed::save_indexed(&cidx, &def).expect("write .cidx");
        let mut file = DiskIndexedFile::new(&data, 15, KeySpec { parts: Vec::new(), offset: 0, len: 3, duplicates: false }, Vec::new());
        assert_eq!(file.open(OpenMode::Output), status::OK);
        for (code, text) in records {
            let mut record = vec![b' '; 15];
            record[0..3].copy_from_slice(&cobolt_indexed::encode_field_display(&id, code, 3).unwrap());
            record[3..15].copy_from_slice(&cobolt_indexed::encode_field_display(&name, text, 12).unwrap());
            assert_eq!(file.write(&record), status::OK);
        }
        assert_eq!(file.close(), status::OK);
        cidx.to_str().unwrap().to_owned()
    }

    fn indexed_list(cidx: &str, limit: u32) -> serde_json::Value {
        json!({"origin": "IndexedFile", "source": cidx, "value_field": "COUNTRY-ID", "display_field": "COUNTRY-NAME", "limit": limit})
    }

    /// An indexed file gives one item per record, in the file's order, spelled the way the
    /// grid spells the same fields; a `DataSource` quick-bound grid reads it at load too.
    #[test]
    fn an_indexed_file_gives_one_item_per_record() {
        let dir = tempfile::tempdir().unwrap();
        let cidx = lookup_file(dir.path(), &[("55", "Brazil"), ("1", "USA"), ("81", "Japan")]);

        let mut interp = interpreter("");
        interp.seed_objects([(
            "Grid".to_owned(),
            "DataGrid".to_owned(),
            vec![
                ("_BindingKind".to_owned(), "IndexedFile".to_owned()),
                ("_BindingIndexedPath".to_owned(), cidx.clone()),
                ("_BindingFields".to_owned(), "COUNTRY-ID\nCOUNTRY-NAME".to_owned()),
                ("AdvancedGrid".to_owned(), advanced(&[("Country", indexed_list(&cidx, 0))])),
            ],
        )]);
        assert_eq!(interp.refresh_binding("Grid"), 3);
        let items = interp.obj_get("Grid", ITEMS);
        let rows = interp.obj_get("Grid", "Rows");
        // Key order: 001, 055, 081.
        let expected: Vec<String> = rows
            .lines()
            .map(|row| {
                let (code, text) = row.split_once('\t').unwrap();
                format!("Country\t{code}\t{text}")
            })
            .collect();
        assert_eq!(items.lines().collect::<Vec<_>>(), expected, "an item is spelled as the grid spells its row");
        assert_eq!(expected.len(), 3);
        assert!(items.contains("\tBrazil") && items.contains("\tJapan") && items.contains("\tUSA"), "{items}");

        // The limit applies here too.
        interp.obj_set("Grid", "AdvancedGrid", advanced(&[("Country", indexed_list(&cidx, 2))]));
        interp.refresh_binding("Grid");
        assert_eq!(interp.obj_get("Grid", ITEMS).lines().count(), 2);

        // A grid bound by `DataSource` alone is read at form load.
        let mut quick = interpreter("");
        quick.seed_objects([(
            "Grid".to_owned(),
            "DataGrid".to_owned(),
            vec![
                ("DataSource".to_owned(), format!("{cidx} / COUNTRY-RECORD")),
                ("AdvancedGrid".to_owned(), advanced(&[("Country", indexed_list(&cidx, 0))])),
            ],
        )]);
        quick.populate_datasource_grids();
        assert_eq!(quick.obj_get("Grid", ITEMS).lines().count(), 3);
        println!("\n  _ColumnItems from an indexed file: 3 records -> 3 items in key order; limit 2 -> 2; DataSource grid read at load\n");
    }

    /// A data file that is not there yet is an empty list, and is not created by asking.
    #[test]
    fn a_missing_data_file_is_an_empty_list_and_is_not_created() {
        let dir = tempfile::tempdir().unwrap();
        let cidx = lookup_file(dir.path(), &[("1", "Chile")]);
        std::fs::remove_file(dir.path().join("country.idx")).unwrap();
        let mut interp = interpreter("");
        interp.seed_objects([(
            "Grid".to_owned(),
            "DataGrid".to_owned(),
            vec![("AdvancedGrid".to_owned(), advanced(&[("Country", indexed_list(&cidx, 0))]))],
        )]);
        interp.refresh_dropdown_items("Grid");
        assert_eq!(interp.obj_get("Grid", ITEMS), "");
        assert!(!dir.path().join("country.idx").exists(), "reading a list must not create the file");
    }
}
