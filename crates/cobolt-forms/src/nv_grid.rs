// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `Non-Visuals` tab's grid (spec 091 R49–R52): where each non-visual
//! control's card sits.
//!
//! **Derived, never stored** (Q12). The cells follow from the controls' types
//! and names alone, so a form opened and saved without an edit is unchanged
//! (R2, R54), a control's cell changes when it is renamed or another is added
//! (R51), and two cards can never claim one cell (R49). A non-visual control's
//! own `X`, `Y`, `Width` and `Height` are untouched — and ignored here.
//!
//! The grid is five columns wide, filled row by row from the top-left corner
//! (R49), with the controls of one type together: the types in A–Z order by
//! their **English** name — so the arrangement is the same in every IDE language
//! (Q19) — and inside a type the controls A–Z by name (R50). Both comparisons
//! ignore letter case, and the name order is plain, not natural: `TMR-10` comes
//! before `TMR-2` (Q18).
//!
//! The grid lives in the designer's canvas space and never grows the form or
//! the window: when the rows do not fit, the tab scrolls (R49, Q16).

use crate::model::{Control, Form, Rect};

/// Cards per row (R49).
pub const COLUMNS: usize = 5;
/// A cell's width and height, in canvas pixels: room for the card's icon and its
/// label (R49).
pub const CELL_W: i32 = 112;
pub const CELL_H: i32 = 96;
/// Space between neighbouring cells, and between the grid and the canvas edge.
pub const GAP: i32 = 12;
pub const MARGIN: i32 = 16;

/// One non-visual control's place in the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// The control's id.
    pub id: String,
    /// 0-based row, then column, counting from the top-left cell.
    pub row: usize,
    pub col: usize,
    /// The cell's rectangle in canvas pixels, from the canvas's top-left corner.
    pub rect: Rect,
}

/// The rectangle of the cell at `row`, `col`.
pub fn cell_rect(row: usize, col: usize) -> Rect {
    Rect::new(
        MARGIN + col as i32 * (CELL_W + GAP),
        MARGIN + row as i32 * (CELL_H + GAP),
        CELL_W,
        CELL_H,
    )
}

/// How many rows `count` cards need — what the tab scrolls over.
pub fn rows_for(count: usize) -> usize {
    count.div_ceil(COLUMNS)
}

/// The height, in canvas pixels, the grid of `count` cards needs, margins
/// included: the scroll area's content height.
pub fn content_height(count: usize) -> i32 {
    match rows_for(count) {
        0 => 2 * MARGIN,
        rows => 2 * MARGIN + rows as i32 * CELL_H + (rows as i32 - 1) * GAP,
    }
}

/// Every control of `controls` the catalogue calls non-visual
/// ([`ControlType::is_non_visual`](crate::model::ControlType::is_non_visual)),
/// nested ones included — a `.cfrm` can hold one inside a container (Q17).
fn non_visual<'a>(controls: &'a [Control], out: &mut Vec<&'a Control>) {
    for c in controls {
        if c.control_type.is_non_visual() {
            out.push(c);
        }
        non_visual(&c.children, out);
    }
}

/// The cell of every non-visual control of `form`, in grid order (R49–R51).
pub fn non_visual_grid(form: &Form) -> Vec<Cell> {
    let mut cards: Vec<&Control> = Vec::new();
    non_visual(&form.controls, &mut cards);
    // Stable, so two controls that somehow share a name keep the file's order.
    cards.sort_by_cached_key(|c| {
        (
            c.control_type.as_str().to_ascii_lowercase(),
            c.id.to_ascii_lowercase(),
        )
    });
    cards
        .into_iter()
        .enumerate()
        .map(|(n, c)| {
            let (row, col) = (n / COLUMNS, n % COLUMNS);
            Cell {
                id: c.id.clone(),
                row,
                col,
                rect: cell_rect(row, col),
            }
        })
        .collect()
}
