// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 106 — `DATAGRID-ROW-CONTENT`: one row of a DataGrid, as the handler of its
//! `onRowButtonClick` receives it.
//!
//! ```cobol
//! 01 DATAGRID-ROW-CONTENT.
//!    05 ROW-NUMBER   PIC 9(9).      *> the row in Rows, 1-based — sorting does not move it
//!    05 ROW-NAME     PIC X(30).     *> the clicked Button column's name
//!    05 ROW-COLUMNS.
//!       10 <column>  PIC <mask>.    *> one per data column, in display order
//! ```
//!
//! **One description, three readers.** Codegen writes the group into the handler in place
//! of its `COPY DATAGRID-ROW-CONTENT.` line, and sizes the main program's buffer by it; the
//! runtime packs the clicked row's record image by it; Check refuses the names that would
//! make it unsafe. All three read [`fields`], so they cannot disagree about a single byte.
//!
//! The level-10 names are the column names the developer sees — its title, else its source
//! field, else its id — folded into COBOL words (operator, spec 106 Q1). A name that would
//! clash is refused by Check rather than renamed (see [`problems`]).

use crate::model::{cobol_word_part, DataGridAdvanced, DataGridColumn};
use crate::picture::Picture;

/// The event a Button column raises (R13).
pub const EVENT: &str = "onRowButtonClick";
/// The group the handler receives (R14).
pub const GROUP: &str = "DATAGRID-ROW-CONTENT";
/// The main program's buffer the group describes; the dispatcher passes it `USING`.
pub const BUFFER: &str = "DATAGRID-ROW-BUFFER";
/// The line in a handler that the group replaces.
pub const COPY_LINE: &str = "COPY DATAGRID-ROW-CONTENT.";
/// The level-05 items, with their pictures.
pub const ROW_NUMBER: (&str, &str) = ("ROW-NUMBER", "9(9)");
pub const ROW_NAME: (&str, &str) = ("ROW-NAME", "X(30)");
pub const ROW_COLUMNS: &str = "ROW-COLUMNS";
/// The picture of a column that has no COBOL mask.
pub const DEFAULT_PIC: &str = "X(30)";

/// Every name the group itself declares, which no data item of the form may also use.
pub const FIXED_NAMES: [&str; 5] = [GROUP, BUFFER, ROW_NUMBER.0, ROW_NAME.0, ROW_COLUMNS];

/// One level-10 item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Its COBOL name.
    pub name: String,
    /// Its picture, without `PIC`.
    pub pic: String,
    /// Its width in bytes.
    pub width: usize,
    /// The column it comes from (its `id`), to find the row's cell.
    pub column_id: String,
}

/// A column's COBOL name: what the developer sees it called — its title, else its source
/// field, else its id — folded into a COBOL word. Empty when nothing usable is left.
pub fn column_name(column: &DataGridColumn) -> String {
    let seen = [&column.title, &column.source_name, &column.id]
        .into_iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
        .unwrap_or("");
    cobol_word_part(seen)
}

/// A column's picture: its COBOL mask, without any `PIC`/`PICTURE` the developer typed,
/// else [`DEFAULT_PIC`].
pub fn column_pic(column: &DataGridColumn) -> String {
    let mask = column.cobol_mask.trim();
    let upper = mask.to_ascii_uppercase();
    let bare = upper
        .strip_prefix("PICTURE")
        .or_else(|| upper.strip_prefix("PIC"))
        .map(str::trim)
        .unwrap_or(upper.as_str())
        .trim_end_matches('.')
        .trim();
    if bare.is_empty() {
        DEFAULT_PIC.to_owned()
    } else {
        bare.to_owned()
    }
}

/// Width in bytes of a DISPLAY item of picture `pic`.
pub fn pic_width(pic: &str) -> usize {
    Picture::parse(pic, false).width()
}

/// The level-10 items of a grid, in display order. Button columns are left out: they
/// are the actions, not the row's data.
pub fn fields(advanced: &DataGridAdvanced) -> Vec<Field> {
    advanced
        .columns
        .iter()
        .filter(|c| !c.is_button())
        .map(|c| {
            let pic = column_pic(c);
            Field {
                name: column_name(c),
                width: pic_width(&pic),
                pic,
                column_id: c.id.clone(),
            }
        })
        .collect()
}

/// The hidden property the generated program seeds on a grid as the form opens: the layout
/// its handler was generated with. The runtime packs by it, never by the grid's columns as
/// they stand — the developer's user may reorder or retitle them at run time (the header's
/// ‹ › rewrite `AdvancedGrid`), and the handler's description cannot follow.
pub const LAYOUT_PROP: &str = "_RowContent";

/// The seed for [`LAYOUT_PROP`]: `F:<column id>=<picture>` for each field in order and
/// `B:<column id>=<name>` for each Button column, joined by `;`.
pub fn layout_seed(advanced: &DataGridAdvanced) -> String {
    let mut parts: Vec<String> = fields(advanced)
        .iter()
        .map(|f| format!("F:{}={}", f.column_id, f.pic))
        .collect();
    parts.extend(
        advanced
            .columns
            .iter()
            .filter(|c| c.is_button())
            .map(|c| format!("B:{}={}", c.id, column_name(c))),
    );
    parts.join(";")
}

/// A seed read back: the fields (`column_id`, `pic`, `width` filled; `name` empty — the
/// bytes need no name) and the buttons as `(column id, name)`.
pub fn parse_layout_seed(seed: &str) -> (Vec<Field>, Vec<(String, String)>) {
    let mut fields = Vec::new();
    let mut buttons = Vec::new();
    for part in seed.split(';') {
        let Some((kind, rest)) = part.split_once(':') else { continue };
        let Some((id, value)) = rest.rsplit_once('=') else { continue };
        match kind {
            "F" => fields.push(Field {
                name: String::new(),
                width: pic_width(value),
                pic: value.to_owned(),
                column_id: id.to_owned(),
            }),
            "B" => buttons.push((id.to_owned(), value.to_owned())),
            _ => {}
        }
    }
    (fields, buttons)
}

/// The whole record's width: ROW-NUMBER, ROW-NAME and every column.
pub fn record_width(fields: &[Field]) -> usize {
    pic_width(ROW_NUMBER.1) + pic_width(ROW_NAME.1) + fields.iter().map(|f| f.width).sum::<usize>()
}

/// The group, as COBOL lines indented by `indent` — what replaces [`COPY_LINE`] in a
/// handler. A grid with no data column still gets `ROW-COLUMNS`, as a one-byte FILLER, so
/// the group is always valid COBOL.
pub fn group_text(fields: &[Field], indent: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("{indent}01 {GROUP}.\n"));
    out.push_str(&format!("{indent}   05 {} PIC {}.\n", ROW_NUMBER.0, ROW_NUMBER.1));
    out.push_str(&format!("{indent}   05 {} PIC {}.\n", ROW_NAME.0, ROW_NAME.1));
    if fields.is_empty() {
        out.push_str(&format!("{indent}   05 {ROW_COLUMNS} PIC X.\n"));
    } else {
        out.push_str(&format!("{indent}   05 {ROW_COLUMNS}.\n"));
        for f in fields {
            out.push_str(&format!("{indent}      10 {} PIC {}.\n", f.name, f.pic));
        }
    }
    out
}

/// Whether `line` is a handler's `COPY DATAGRID-ROW-CONTENT.` — with or without quotes
/// around the name, in any case.
pub fn is_copy_line(line: &str) -> bool {
    let compact: String = line
        .trim()
        .chars()
        .filter(|c| *c != '"' && *c != '\'')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" .", ".");
    compact.eq_ignore_ascii_case(COPY_LINE)
}

/// What kind of problem a [`Problem`] is — so a window can say it in its own language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemKind {
    /// A Button column with neither caption nor icon (a warning).
    SilentButton,
    /// No name that can be a COBOL data name.
    NotAWord,
    /// Longer than the 30 characters a data name may have.
    TooLong,
    /// A COBOL reserved word.
    Reserved,
    /// A name `DATAGRID-ROW-CONTENT` itself uses.
    GroupName,
    /// Also a data item of the form.
    ClashesWithForm,
    /// Another column of the grid has it too (for two Button columns, a warning: the
    /// handler cannot tell their ROW-NAMEs apart).
    Duplicate,
    /// A Button column's name is longer than ROW-NAME's 30 characters, so the handler
    /// sees it cut (a warning).
    ButtonNameTooLong,
}

/// One thing Check reports for a grid (R17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub kind: ProblemKind,
    /// The column's display index in `AdvancedGrid`.
    pub column: usize,
    /// The column's name as the group would have it (or as the developer sees it, when
    /// it has none that can be a COBOL word).
    pub name: String,
    /// A sentence naming the grid and the column, for Check.
    pub message: String,
    /// An error: the group cannot be generated safely. Otherwise a warning — the program
    /// works, but probably not as meant.
    pub is_error: bool,
}

/// What Check reports for a grid (R17). Two checks need the rest of the program and are
/// passed in: `is_reserved` says whether a word is a COBOL reserved word, and `form_names`
/// are the data names the form's program already declares (upper case).
///
/// A Button column with neither caption nor icon is only a warning: it still draws each
/// row's own cell value, as a Button column always has (R10).
pub fn problems(
    grid_id: &str,
    advanced: &DataGridAdvanced,
    is_reserved: &dyn Fn(&str) -> bool,
    form_names: &std::collections::HashSet<String>,
) -> Vec<Problem> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut seen_buttons = std::collections::HashSet::new();
    for (index, c) in advanced.columns.iter().enumerate() {
        let name = column_name(c);
        let button = c.is_button();
        let mut add = |kind: ProblemKind, name: &str, message: String| {
            out.push(Problem {
                kind,
                column: index,
                name: name.to_owned(),
                message: format!("DataGrid {grid_id}: {message}"),
                // A Button column's name only reaches the handler as ROW-NAME's text:
                // nothing about it can break the group, so all of its are warnings.
                is_error: !button,
            })
        };
        if button {
            if !c.has_own_button() {
                add(
                    ProblemKind::SilentButton,
                    &name,
                    format!("Button column \"{name}\" has neither a caption nor an icon, so its buttons show only each row's own cell value"),
                );
            }
            if name.len() > pic_width(ROW_NAME.1) {
                add(
                    ProblemKind::ButtonNameTooLong,
                    &name,
                    format!("Button column name {name} is longer than the 30 characters ROW-NAME holds, so the handler sees it cut"),
                );
            }
            if !name.is_empty() && !seen_buttons.insert(name.clone()) {
                add(
                    ProblemKind::Duplicate,
                    &name,
                    format!("two Button columns are both named {name}, so the handler cannot tell their ROW-NAMEs apart"),
                );
            }
            continue;
        }
        if name.is_empty() || !name.starts_with(|ch: char| ch.is_ascii_alphabetic()) {
            let shown = if c.title.trim().is_empty() { c.id.as_str() } else { c.title.as_str() };
            add(
                ProblemKind::NotAWord,
                shown,
                format!("column \"{shown}\" has no name that can be a COBOL data name; give it a title that starts with a letter"),
            );
            continue;
        }
        if name.len() > 30 {
            add(
                ProblemKind::TooLong,
                &name,
                format!("column name {name} is longer than the 30 characters a COBOL data name may have"),
            );
        }
        if is_reserved(&name) {
            add(ProblemKind::Reserved, &name, format!("column name {name} is a COBOL reserved word; rename the column"));
        }
        if FIXED_NAMES.contains(&name.as_str()) {
            add(
                ProblemKind::GroupName,
                &name,
                format!("column name {name} is a name {GROUP} itself uses; rename the column"),
            );
        }
        if form_names.contains(&name) {
            add(
                ProblemKind::ClashesWithForm,
                &name,
                format!("column name {name} clashes with a data item of the form; rename the column or the data item"),
            );
        }
        if !seen.insert(name.clone()) {
            add(ProblemKind::Duplicate, &name, format!("two columns are both named {name}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(title: &str, mask: &str, kind: &str) -> DataGridColumn {
        DataGridColumn {
            id: title.to_ascii_uppercase().replace(' ', "_"),
            title: title.into(),
            cobol_mask: mask.into(),
            edit_control: kind.into(),
            ..Default::default()
        }
    }

    fn grid() -> DataGridAdvanced {
        DataGridAdvanced {
            columns: vec![
                col("Actor Id", "9(5)", "Textbox"),
                col("Actor Name", "", "Textbox"),
                col("Edit", "", "Button"),
                col("Salary", "PIC S9(5)V99.", "Textbox"),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn the_columns_become_level_10_items_buttons_left_out() {
        let f = fields(&grid());
        let names: Vec<_> = f.iter().map(|f| (f.name.as_str(), f.pic.as_str(), f.width)).collect();
        assert_eq!(
            names,
            [("ACTOR-ID", "9(5)", 5), ("ACTOR-NAME", "X(30)", 30), ("SALARY", "S9(5)V99", 7)]
        );
        assert_eq!(record_width(&f), 9 + 30 + 5 + 30 + 7);
    }

    #[test]
    fn the_group_is_the_one_the_operator_described() {
        let text = group_text(&fields(&grid()), "       ");
        assert_eq!(
            text,
            "       01 DATAGRID-ROW-CONTENT.\n\
             \x20         05 ROW-NUMBER PIC 9(9).\n\
             \x20         05 ROW-NAME PIC X(30).\n\
             \x20         05 ROW-COLUMNS.\n\
             \x20            10 ACTOR-ID PIC 9(5).\n\
             \x20            10 ACTOR-NAME PIC X(30).\n\
             \x20            10 SALARY PIC S9(5)V99.\n"
        );
        let empty = group_text(&[], "");
        assert!(empty.contains("05 ROW-COLUMNS PIC X."), "always valid COBOL:\n{empty}");
    }

    #[test]
    fn the_layout_seed_round_trips() {
        let mut g = grid();
        g.columns[2].button_caption = "Edit".into();
        let seed = layout_seed(&g);
        assert_eq!(seed, "F:ACTOR_ID=9(5);F:ACTOR_NAME=X(30);F:SALARY=S9(5)V99;B:EDIT=EDIT");
        let (fields, buttons) = parse_layout_seed(&seed);
        let got: Vec<_> = fields.iter().map(|f| (f.column_id.as_str(), f.pic.as_str(), f.width)).collect();
        assert_eq!(got, [("ACTOR_ID", "9(5)", 5), ("ACTOR_NAME", "X(30)", 30), ("SALARY", "S9(5)V99", 7)]);
        assert_eq!(buttons, [("EDIT".to_owned(), "EDIT".to_owned())]);
    }

    /// R12 — the two new column fields are written only when set: a grid saved before
    /// spec 106 loads (the fields default empty) and saves exactly as it did.
    #[test]
    fn a_grid_without_buttons_saves_as_it_did() {
        let old = grid().to_json().unwrap();
        assert!(!old.contains("button_caption") && !old.contains("button_icon"), "{old}");
        let back: DataGridAdvanced = serde_json::from_str(&old).expect("an older grid loads");
        assert_eq!(back.to_json().unwrap(), old, "byte for byte");
        let mut with = grid();
        with.columns[2].button_caption = "Edit".into();
        with.columns[2].button_icon = "pencil".into();
        let json = with.to_json().unwrap();
        let again: DataGridAdvanced = serde_json::from_str(&json).unwrap();
        assert_eq!((again.columns[2].button_caption.as_str(), again.columns[2].button_icon.as_str()), ("Edit", "pencil"));
    }

    #[test]
    fn the_copy_line_is_recognised_however_it_is_written() {
        for l in [
            "       COPY DATAGRID-ROW-CONTENT.",
            "COPY \"DATAGRID-ROW-CONTENT\".",
            "  copy 'datagrid-row-content'.",
            "COPY   DATAGRID-ROW-CONTENT .",
        ] {
            assert!(is_copy_line(l), "{l:?}");
        }
        assert!(!is_copy_line("COPY CUSTREC."));
        assert!(!is_copy_line("*> COPY DATAGRID-ROW-CONTENT."));
    }

    #[test]
    fn check_refuses_every_unsafe_name_and_a_silent_button() {
        let mut g = grid();
        g.columns.push(col("Customer", "", "Textbox"));
        g.columns.push(col("Move", "", "Textbox"));
        g.columns.push(col("Actor-Id", "", "Textbox"));
        g.columns.push(col("Row Number", "", "Textbox"));
        g.columns.push(col("1st", "", "Textbox"));
        let form_names: std::collections::HashSet<String> = ["CUSTOMER".to_owned()].into();
        let reserved = |w: &str| w == "MOVE";
        let found = problems("Grid1", &g, &reserved, &form_names);
        let has = |needle: &str| found.iter().any(|p| p.message.contains(needle) && p.is_error);
        let silent = found
            .iter()
            .find(|p| p.message.contains("Button column \"EDIT\" has neither a caption nor an icon"));
        assert!(
            silent.is_some_and(|p| !p.is_error),
            "a cell-valued Button column still works (R10), so it is only a warning: {found:#?}"
        );
        assert!(has("CUSTOMER clashes with a data item of the form"), "{found:#?}");
        assert!(has("MOVE is a COBOL reserved word"), "{found:#?}");
        assert!(has("two columns are both named ACTOR-ID"), "{found:#?}");
        assert!(has("ROW-NUMBER is a name DATAGRID-ROW-CONTENT itself uses"), "{found:#?}");
        assert!(has("\"1st\" has no name that can be a COBOL data name"), "{found:#?}");
        // Each names its column by display index: Edit is the third, 1st the ninth.
        let at = |kind: ProblemKind| found.iter().find(|p| p.kind == kind).map(|p| (p.column, p.name.as_str()));
        assert_eq!(at(ProblemKind::SilentButton), Some((2, "EDIT")));
        assert_eq!(at(ProblemKind::ClashesWithForm), Some((4, "CUSTOMER")));
        assert_eq!(at(ProblemKind::Duplicate), Some((6, "ACTOR-ID")));
        assert_eq!(at(ProblemKind::NotAWord), Some((8, "1st")));
        // Two Button columns of one name, and one too long for ROW-NAME: warnings.
        let mut buttons = grid();
        buttons.columns[2].button_caption = "Edit".into();
        let mut twin = col("Edit", "", "Button");
        twin.button_caption = "Edit".into();
        buttons.columns.push(twin);
        let mut long = col("Send this row to the archive now", "", "Button");
        long.button_caption = "Archive".into();
        buttons.columns.push(long);
        let found = problems("Grid1", &buttons, &|_| false, &Default::default());
        let warned = |kind: ProblemKind| found.iter().any(|p| p.kind == kind && !p.is_error);
        assert!(warned(ProblemKind::Duplicate), "{found:#?}");
        assert!(warned(ProblemKind::ButtonNameTooLong), "{found:#?}");
        assert!(found.iter().all(|p| !p.is_error), "a Button column's name never breaks the group: {found:#?}");
        // A Button column with its own caption is fine.
        let mut ok = grid();
        ok.columns[2].button_caption = "Edit".into();
        assert!(problems("Grid1", &ok, &|_| false, &Default::default()).is_empty());
    }
}
