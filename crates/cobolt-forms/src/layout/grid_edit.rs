// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Editing a Grid container's structure** (spec 088): add and remove a row or a
//! column, restore one, merge and split cells, resize two tracks by a line — each
//! a **pure function** from the container as the engine laid it out to a list of
//! property writes. Nothing here draws or reads the canvas; the Form Designer
//! turns the writes into one undoable step, and the one layout engine places the
//! result, so the canvas and Run Form cannot differ.
//!
//! A track list is edited as **tokens** ([`crate::layout::tracks::split`]): a
//! token is replaced, inserted or removed whole, and every other one — an
//! unreadable half-typed one included — stays exactly as the developer typed it.
//! A row or column that exists only because a child was placed in it (an
//! *implicit* track, always `Auto`) is written out as an explicit `Auto` token
//! before an edit touches that axis; nothing moves, since that is what it was.

use crate::layout::defaults;
use crate::layout::grid::Cell;
use crate::layout::props::PropSource;
use crate::layout::tracks::{self, TokenKind};
use crate::layout::ContainerGeom;
use crate::model::{Control, PropValue};

/// One property write: control id, property name, value.
pub type Edit = (String, String, PropValue);
pub type Edits = Vec<Edit>;

/// The two axes of a grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    Column,
    Row,
}

impl Axis {
    /// The container property that lists this axis's tracks.
    pub fn tracks_key(self) -> &'static str {
        match self {
            Axis::Column => "GridColumns",
            Axis::Row => "GridRows",
        }
    }
    /// The child property that places it on this axis (1-based; 0 is auto).
    pub fn place_key(self) -> &'static str {
        match self {
            Axis::Column => "GridColumn",
            Axis::Row => "GridRow",
        }
    }
    /// The child property that spans it on this axis.
    pub fn span_key(self) -> &'static str {
        match self {
            Axis::Column => "ColumnSpan",
            Axis::Row => "RowSpan",
        }
    }
}

/// Why an edit is not offered. The designer turns each into a tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The axis holds a `Repeat(…)`, whose tracks are edited in the Properties
    /// pane (R14).
    Repeat(Axis),
    /// A grid keeps at least one row and one column (R12).
    LastTrack(Axis),
    /// The track named is not there.
    NoSuchTrack,
    /// A merge needs exactly one control in the range, and there is none (R16).
    NoControlInRange,
    /// …and there is more than one (R16).
    SeveralControlsInRange,
    /// The track cannot be dragged: it is `MinMax(…)` or `Repeat(…)` (R20).
    FixedTrack,
    /// A merge of a single cell has nothing to merge.
    OneCell,
    /// Split needs a control that spans more than one cell.
    NothingToSplit,
    /// The container is shown at a scale (an expanded card), so laid-out pixels
    /// are not designed pixels (plan §5).
    Scaled,
}

/// A child of the grid: where the engine put it, and what is written.
#[derive(Clone, Debug, PartialEq)]
pub struct Child {
    pub id: String,
    /// The cell the engine placed it in — auto-placed children included.
    pub cell: Cell,
    /// `GridColumn` as written; 0 is auto.
    pub column: usize,
    /// `GridRow` as written; 0 is auto.
    pub row: usize,
}

/// A Grid container as an editor needs it: the two track texts as written, the
/// track sizes the engine laid out (implicit tracks included), the client
/// rectangle, the gaps, and the children with their cells.
#[derive(Clone, Debug, PartialEq)]
pub struct GridView {
    pub id: String,
    pub columns: String,
    pub rows: String,
    pub column_sizes: Vec<f32>,
    pub row_sizes: Vec<f32>,
    /// The laid-out client size, width and height.
    pub client: (f32, f32),
    /// Column gap, row gap.
    pub gaps: (f32, f32),
    /// Whether the container is laid out at a scale other than its design's.
    pub scaled: bool,
    pub children: Vec<Child>,
}

impl GridView {
    /// Read a container from the layout engine's answer. `kids` are the container's
    /// visual children; a child the engine did not place is left out.
    pub fn from_layout(container: &Control, kids: &[&Control], geom: &ContainerGeom) -> GridView {
        let whole = |c: &Control, k: &str| c.number(k).max(0.0) as usize;
        GridView {
            id: container.id.clone(),
            columns: container.text("GridColumns"),
            rows: container.text("GridRows"),
            column_sizes: geom.columns.clone(),
            row_sizes: geom.rows.clone(),
            client: (geom.client.w, geom.client.h),
            gaps: geom.gaps,
            scaled: (geom.client.w - geom.designed_client.w).abs() > 0.5
                || (geom.client.h - geom.designed_client.h).abs() > 0.5,
            children: kids
                .iter()
                .filter_map(|c| {
                    let cell = geom.cells.iter().find(|(id, _)| *id == c.id)?.1;
                    Some(Child { id: c.id.clone(), cell, column: whole(c, "GridColumn"), row: whole(c, "GridRow") })
                })
                .collect(),
        }
    }

    fn text(&self, axis: Axis) -> &str {
        match axis {
            Axis::Column => &self.columns,
            Axis::Row => &self.rows,
        }
    }

    fn sizes(&self, axis: Axis) -> &[f32] {
        match axis {
            Axis::Column => &self.column_sizes,
            Axis::Row => &self.row_sizes,
        }
    }

    /// How many tracks the engine laid out on `axis`, implicit ones included.
    pub fn track_count(&self, axis: Axis) -> usize {
        self.sizes(axis).len()
    }
}

/// A child's first track and span on `axis`, 0-based, as the engine placed it.
fn first_and_span(c: &Child, axis: Axis) -> (usize, usize) {
    match axis {
        Axis::Column => (c.cell.column, c.cell.column_span),
        Axis::Row => (c.cell.row, c.cell.row_span),
    }
}

/// What a child has written for its placement on `axis` (1-based; 0 is auto).
fn written(c: &Child, axis: Axis) -> usize {
    match axis {
        Axis::Column => c.column,
        Axis::Row => c.row,
    }
}

fn int(id: &str, key: &str, v: usize) -> Edit {
    (id.to_owned(), key.to_owned(), PropValue::Int(v as i64))
}

/// The axis's tokens with every implicit track written out as `Auto`. Refuses an
/// axis that holds a `Repeat(…)`.
fn explicit_tokens(view: &GridView, axis: Axis) -> Result<(Vec<String>, Vec<usize>), Refusal> {
    let slots = tracks::slots(view.text(axis));
    if slots.has_repeat {
        return Err(Refusal::Repeat(axis));
    }
    let mut tokens = slots.tokens;
    let mut track_to_token = slots.track_to_token;
    while track_to_token.len() < view.track_count(axis) {
        tokens.push("Auto".to_owned());
        track_to_token.push(tokens.len() - 1);
    }
    Ok((tokens, track_to_token))
}

/// Put `new` in as the track at index `at` (0-based, over the explicit tracks)
/// and renumber the children: one placed at or after `at` moves over, one that
/// spans across `at` spans one more. An auto-placed child is left auto.
fn splice_in(view: &GridView, axis: Axis, mut tokens: Vec<String>, at: usize, at_token: usize, new: String) -> Edits {
    tokens.insert(at_token.min(tokens.len()), new);
    let mut edits: Edits = vec![(view.id.clone(), axis.tracks_key().to_owned(), PropValue::String(tracks::join(&tokens)))];
    for c in &view.children {
        let w = written(c, axis);
        if w == 0 {
            continue;
        }
        let (first, span) = first_and_span(c, axis);
        if first >= at {
            edits.push(int(&c.id, axis.place_key(), w + 1));
        } else if first + span > at {
            edits.push(int(&c.id, axis.span_key(), span + 1));
        }
    }
    edits
}

/// **Add a row or a column** after track `after` (0-based), or at the end when
/// `None` (R10). The new track is sized like the one it follows; every child that
/// is placed after the insertion point is renumbered so it keeps its cell, and a
/// child that spans across it spans one more. An auto-placed child is left auto.
pub fn insert_track(view: &GridView, axis: Axis, after: Option<usize>) -> Result<Edits, Refusal> {
    let (tokens, track_to_token) = explicit_tokens(view, axis)?;
    let count = track_to_token.len();
    if after.is_some_and(|a| a >= count) {
        return Err(Refusal::NoSuchTrack);
    }
    let at = after.map_or(count, |a| a + 1);
    // Sized like the track it follows; an empty axis starts with one `1fr`.
    let new = after
        .or(count.checked_sub(1))
        .map(|t| tokens[track_to_token[t]].clone())
        .unwrap_or_else(|| "1fr".to_owned());
    // Right after the track it follows (an unreadable token between the two stays
    // where it was typed); at the very end when nothing is selected.
    let at_token = after.map_or(tokens.len(), |t| track_to_token[t] + 1);
    Ok(splice_in(view, axis, tokens, at, at_token, new))
}

/// No track is dragged below this many pixels (R21).
pub const MIN_TRACK_PX: f32 = defaults::GRID_EDIT_MIN_TRACK_PX;

/// **Move the line between track `line` and the one after it by `delta` pixels**
/// (R19–R21): the first grows by `delta` and the second shrinks by as much, so the
/// container's total is kept; every other track keeps its size. Neither goes below
/// [`MIN_TRACK_PX`] — a larger drag stops there — and a zero result writes nothing.
///
/// The arithmetic is in pixels, and each of the two is then written back in **its
/// own unit**: `px` as a whole number of pixels, `%` as its share of the container's
/// client size, `Auto` as `px`, and `fr` scaled by its own change of size (which
/// keeps the pixel-per-`fr` rate, so the other flexible tracks do not move). `MinMax`
/// and `Repeat` are not dragged, and nothing is while the container is laid out at a
/// scale. `delta` is measured from where the tracks were in `view`, not from the last
/// frame: apply it to the same view for as long as the drag lasts.
pub fn resize_pair(view: &GridView, axis: Axis, line: usize, delta: f32) -> Result<Edits, Refusal> {
    if view.scaled {
        return Err(Refusal::Scaled);
    }
    let (tokens, track_to_token) = explicit_tokens(view, axis)?;
    let sizes = view.sizes(axis);
    if line + 1 >= track_to_token.len() || line + 1 >= sizes.len() {
        return Err(Refusal::NoSuchTrack);
    }
    let (a, b) = (sizes[line], sizes[line + 1]);
    let (ta, tb) = (track_to_token[line], track_to_token[line + 1]);
    let (ka, kb) = (tracks::kind_of(&tokens[ta]), tracks::kind_of(&tokens[tb]));
    let (TokenKind::Plain(pa), TokenKind::Plain(pb)) = (ka, kb) else {
        return Err(Refusal::FixedTrack);
    };
    // Neither shrinks below the floor, and one already below it is not made smaller.
    let d = delta.clamp((MIN_TRACK_PX - a).min(0.0), (b - MIN_TRACK_PX).max(0.0));
    if d.abs() < defaults::GRID_EDIT_MIN_DRAG_PX {
        return Ok(Vec::new());
    }
    let avail = match axis {
        Axis::Column => view.client.0,
        Axis::Row => view.client.1,
    };
    let gap = match axis {
        Axis::Column => view.gaps.0,
        Axis::Row => view.gaps.1,
    };
    let fr_of = |p: tracks::Track| match p.max {
        tracks::Breadth::Fr(f) => Some(f),
        _ => None,
    };
    let (fa, fb) = (fr_of(pa), fr_of(pb));
    // Every flexible track outside the pair, and the space the fixed ones leave.
    let mut others_flex = false;
    let (mut fixed, mut flex_factors) = (0.0f32, 0.0f32);
    for (k, &t) in track_to_token.iter().enumerate() {
        let size = sizes.get(k).copied().unwrap_or_default();
        match tracks::kind_of(&tokens[t]) {
            TokenKind::Plain(p) => match fr_of(p) {
                Some(f) => {
                    flex_factors += f;
                    others_flex |= k != line && k != line + 1;
                }
                None => fixed += size,
            },
            _ => fixed += size,
        }
    }
    let left = (avail - gap * sizes.len().saturating_sub(1) as f32 - fixed).max(0.0);
    // The pixels one `fr` is worth as the engine has it laid out.
    let unit = sizes
        .iter()
        .zip(&track_to_token)
        .find_map(|(&s, &t)| match tracks::kind_of(&tokens[t]) {
            TokenKind::Plain(p) => fr_of(p).filter(|f| *f > 0.0 && s > 0.0).map(|f| s / f),
            _ => None,
        })
        .unwrap_or(left / flex_factors.max(1.0));

    let (na, nb) = (a + d, b - d);
    // A track that is not flexible: pixels, or a share of the container.
    let fixed_token = |p: tracks::Track, new: f32| match p.max {
        tracks::Breadth::Percent(_) => tracks::fmt_percent(new / avail.max(1.0) * crate::layout::defaults::PERCENT),
        _ => tracks::fmt_px(new),
    };
    // A flexible track beside a fixed one, with no other flexible track: it takes what
    // the fixed ones leave, so its factor only matters while it is under 1 (then it
    // takes that fraction of the space) — and an untouched factor stays as typed.
    let alone = |f: f32, new: f32, left_after: f32| if f >= 1.0 { None } else { Some(tracks::fmt_fr((new / left_after.max(1.0)).min(1.0))) };
    let mut tokens = tokens;
    match (fa, fb) {
        // Two flexible tracks share their combined factor in the new proportion: the
        // flexible tracks as a whole keep their total, so none of the others moves.
        (Some(fa), Some(fb)) => {
            tokens[ta] = tracks::fmt_fr((fa + fb) * na / (a + b));
            tokens[tb] = tracks::fmt_fr((fa + fb) * nb / (a + b));
        }
        (Some(fa), None) => {
            tokens[tb] = fixed_token(pb, nb);
            let rewritten = if others_flex {
                Some(tracks::fmt_fr(if a > 0.0 { fa * na / a } else { na / unit.max(defaults::GRID_EDIT_MIN_UNIT) }))
            } else {
                alone(fa, na, left + b - nb)
            };
            if let Some(t) = rewritten {
                tokens[ta] = t;
            }
        }
        (None, Some(fb)) => {
            tokens[ta] = fixed_token(pa, na);
            let rewritten = if others_flex {
                Some(tracks::fmt_fr(if b > 0.0 { fb * nb / b } else { nb / unit.max(defaults::GRID_EDIT_MIN_UNIT) }))
            } else {
                alone(fb, nb, left + a - na)
            };
            if let Some(t) = rewritten {
                tokens[tb] = t;
            }
        }
        (None, None) => {
            tokens[ta] = fixed_token(pa, na);
            tokens[tb] = fixed_token(pb, nb);
        }
    }
    Ok(vec![(view.id.clone(), axis.tracks_key().to_owned(), PropValue::String(tracks::join(&tokens)))])
}

/// A rectangle of cells, 0-based: its top-left cell and how many rows and columns
/// it covers (R7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellRange {
    pub row: usize,
    pub column: usize,
    pub rows: usize,
    pub columns: usize,
}

impl CellRange {
    pub fn single(row: usize, column: usize) -> CellRange {
        CellRange { row, column, rows: 1, columns: 1 }
    }

    /// The range between two corner cells, whichever way round they are given.
    pub fn between(a: (usize, usize), b: (usize, usize)) -> CellRange {
        let (r0, r1) = (a.0.min(b.0), a.0.max(b.0));
        let (c0, c1) = (a.1.min(b.1), a.1.max(b.1));
        CellRange { row: r0, column: c0, rows: r1 - r0 + 1, columns: c1 - c0 + 1 }
    }

    pub fn contains(&self, row: usize, column: usize) -> bool {
        (self.row..self.row + self.rows).contains(&row) && (self.column..self.column + self.columns).contains(&column)
    }
}

/// The children that are in `range`. A child is in **every cell its span covers**,
/// so one that spans into the range counts (R16).
pub fn controls_in<'v>(view: &'v GridView, range: CellRange) -> Vec<&'v Child> {
    view.children
        .iter()
        .filter(|c| {
            let k = c.cell;
            k.row < range.row + range.rows
                && range.row < k.row + k.row_span
                && k.column < range.column + range.columns
                && range.column < k.column + k.column_span
        })
        .collect()
}

/// **Merge cells** (R15, R16): the one control in the range now spans it — placed
/// at the range's top-left cell, spanning its rows and columns. Refused unless there
/// are at least two cells and **exactly one** control in them, because the model has
/// no span without a control.
pub fn merge(view: &GridView, range: CellRange) -> Result<Edits, Refusal> {
    if range.rows * range.columns < 2 {
        return Err(Refusal::OneCell);
    }
    let inside = controls_in(view, range);
    let c = match inside.as_slice() {
        [] => return Err(Refusal::NoControlInRange),
        [one] => *one,
        _ => return Err(Refusal::SeveralControlsInRange),
    };
    let mut edits = vec![int(&c.id, "GridColumn", range.column + 1), int(&c.id, "GridRow", range.row + 1)];
    if c.cell.column_span != range.columns {
        edits.push(int(&c.id, "ColumnSpan", range.columns));
    }
    if c.cell.row_span != range.rows {
        edits.push(int(&c.id, "RowSpan", range.rows));
    }
    Ok(edits)
}

/// **Split cells** (R17): the control that covers cell `(row, column)` and spans more
/// than one cell goes back to one cell — its top-left, which it is pinned to.
pub fn split_cell(view: &GridView, row: usize, column: usize) -> Result<Edits, Refusal> {
    let c = controls_in(view, CellRange::single(row, column))
        .into_iter()
        .find(|c| c.cell.column_span > 1 || c.cell.row_span > 1)
        .ok_or(Refusal::NothingToSplit)?;
    let mut edits = vec![int(&c.id, "GridColumn", c.cell.column + 1), int(&c.id, "GridRow", c.cell.row + 1)];
    if c.cell.column_span > 1 {
        edits.push(int(&c.id, "ColumnSpan", 1));
    }
    if c.cell.row_span > 1 {
        edits.push(int(&c.id, "RowSpan", 1));
    }
    Ok(edits)
}

/// A child the removal of a track left auto-placed on that axis, as it was.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldChild {
    pub id: String,
    /// Its `GridColumn` / `GridRow` before (1-based).
    pub place: usize,
}

/// What a removal took out — everything Restore needs to put it back (R13).
#[derive(Clone, Debug, PartialEq)]
pub struct RemovedTrack {
    pub container: String,
    pub axis: Axis,
    /// The track's index (0-based) when it was removed.
    pub index: usize,
    /// The track as written, whatever it was.
    pub token: String,
    /// The children that sat only in it.
    pub held: Vec<HeldChild>,
}

/// What a removal answers: the writes, what to remember, and the children whose
/// placement became automatic (R12's notice names them).
#[derive(Clone, Debug, PartialEq)]
pub struct Removal {
    pub edits: Edits,
    pub record: RemovedTrack,
    pub now_auto: Vec<String>,
}

/// **Remove a row or a column** (R11, R12). The track's token is dropped and the
/// remaining tracks share the container by their own rules. Children after it are
/// renumbered; one that spans across it spans one less; one that sat **only** in it
/// is never deleted — it becomes auto-placed on this axis. The last track of an axis
/// cannot be removed.
pub fn remove_track(view: &GridView, axis: Axis, index: usize) -> Result<Removal, Refusal> {
    let (mut tokens, track_to_token) = explicit_tokens(view, axis)?;
    let count = track_to_token.len();
    if index >= count {
        return Err(Refusal::NoSuchTrack);
    }
    if count == 1 {
        return Err(Refusal::LastTrack(axis));
    }
    let token = tokens.remove(track_to_token[index]);
    let mut edits: Edits = vec![(view.id.clone(), axis.tracks_key().to_owned(), PropValue::String(tracks::join(&tokens)))];
    let (mut held, mut now_auto) = (Vec::new(), Vec::new());
    for c in &view.children {
        let w = written(c, axis);
        if w == 0 {
            continue;
        }
        let (first, span) = first_and_span(c, axis);
        if first > index {
            edits.push(int(&c.id, axis.place_key(), w - 1));
        } else if first + span > index {
            if span == 1 {
                edits.push(int(&c.id, axis.place_key(), 0));
                held.push(HeldChild { id: c.id.clone(), place: w });
                now_auto.push(c.id.clone());
            } else {
                edits.push(int(&c.id, axis.span_key(), span - 1));
            }
        }
    }
    Ok(Removal { edits, record: RemovedTrack { container: view.id.clone(), axis, index, token, held }, now_auto })
}

/// **Restore** a removed row or column (R13): the same track, at the same place
/// (or the end, if the list has since got shorter), every child after it moved
/// over as an add would, and the children it held put back — each only if it is
/// still auto-placed on that axis, so a control the developer has placed since is
/// left where they put it. Nothing else is touched.
pub fn restore_track(view: &GridView, removed: &RemovedTrack) -> Result<Edits, Refusal> {
    let axis = removed.axis;
    let (tokens, track_to_token) = explicit_tokens(view, axis)?;
    let at = removed.index.min(track_to_token.len());
    // Back before the token of the track that now sits where it was, or at the end.
    let at_token = track_to_token.get(at).copied().unwrap_or(tokens.len());
    let mut edits = splice_in(view, axis, tokens, at, at_token, removed.token.clone());
    for h in &removed.held {
        if let Some(c) = view.children.iter().find(|c| c.id == h.id) {
            if written(c, axis) == 0 {
                edits.push(int(&c.id, axis.place_key(), at + 1));
            }
        }
    }
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::test_support::{ctrl, solve_at, with};
    use crate::ControlType;
    use std::collections::HashMap;

    /// A Panel `G` laid out as a Grid, holding buttons. `kids`: id, then the
    /// written `GridColumn`, `GridRow`, `ColumnSpan`, `RowSpan` (0 leaves a
    /// property unwritten).
    fn grid(columns: &str, rows: &str, kids: &[(&str, [i64; 4])]) -> Vec<Control> {
        let s = |v: &str| PropValue::String(v.into());
        let mut panel = ctrl("G", ControlType::Panel, (0, 0, 400, 200), None);
        panel = with(panel, "LayoutMode", s("Grid"));
        panel = with(panel, "GridColumns", s(columns));
        panel = with(panel, "GridRows", s(rows));
        let mut out = vec![panel];
        for (i, (id, p)) in kids.iter().enumerate() {
            let mut c = ctrl(id, ControlType::Button, (0, 30 * i as i32, 40, 20), Some("G"));
            for (key, v) in ["GridColumn", "GridRow", "ColumnSpan", "RowSpan"].iter().zip(p) {
                if *v > 0 {
                    c = with(c, key, *v);
                }
            }
            out.push(c);
        }
        out
    }

    fn view_of(controls: &[Control]) -> GridView {
        let o = solve_at(controls, (400.0, 200.0), (400.0, 200.0));
        let kids: Vec<&Control> = controls.iter().filter(|c| c.parent.as_deref() == Some("G")).collect();
        GridView::from_layout(&controls[0], &kids, &o.containers["G"])
    }

    fn apply(controls: &mut [Control], edits: &Edits) {
        for (id, key, v) in edits {
            controls.iter_mut().find(|c| c.id == *id).expect("a control the edit names").set_prop(key, v.clone());
        }
    }

    /// Read the container as laid out, run `f` on it, and write what it answers.
    fn run(controls: &mut [Control], f: impl FnOnce(&GridView) -> Result<Edits, Refusal>) -> Result<(), Refusal> {
        let edits = f(&view_of(controls))?;
        apply(controls, &edits);
        Ok(())
    }

    fn cells_of(controls: &[Control]) -> HashMap<String, (usize, usize, usize, usize)> {
        let o = solve_at(controls, (400.0, 200.0), (400.0, 200.0));
        o.containers["G"].cells.iter().map(|(id, c)| (id.clone(), (c.row, c.column, c.row_span, c.column_span))).collect()
    }

    fn text(controls: &[Control], key: &str) -> String {
        controls[0].text(key)
    }

    /// AC5 — a 2×2 grid `1fr 1fr` / `1fr 1fr` with a child at (column 2, row 2): add
    /// a column after column 1 gives `1fr 1fr 1fr`, and the child is at column 3,
    /// in the same cell it was — it has only moved over, with the track it was in.
    #[test]
    fn add_a_column_after_the_first_renumbers_what_follows_it() {
        let mut g = grid("1fr 1fr", "1fr 1fr", &[("A", [2, 2, 0, 0]), ("B", [1, 1, 0, 0])]);
        let before = cells_of(&g);
        assert_eq!(before["A"], (1, 1, 1, 1));
        let edits = insert_track(&view_of(&g), Axis::Column, Some(0)).expect("a column can be added");
        apply(&mut g, &edits);
        assert_eq!(text(&g, "GridColumns"), "1fr 1fr 1fr");
        assert_eq!(g[1].text("GridColumn"), "3", "A moved over with the track it was in");
        assert_eq!(g[2].text("GridColumn"), "1", "B was before the new column and stays");
        let after = cells_of(&g);
        assert_eq!(after["A"], (1, 2, 1, 1));
        assert_eq!(after["B"], (0, 0, 1, 1));
        let o = solve_at(&g, (400.0, 200.0), (400.0, 200.0));
        assert_eq!(o.containers["G"].columns.len(), 3);
        println!("add column after 1: `1fr 1fr` → `{}`, A (2,2) → column {}, row {}", text(&g, "GridColumns"), g[1].text("GridColumn"), g[1].text("GridRow"));
    }

    /// With no cell selected the track goes at the end, sized like the last one; an
    /// empty axis starts with one `1fr`.
    #[test]
    fn add_at_the_end_copies_the_last_track_and_an_empty_axis_gets_a_fraction() {
        let mut g = grid("100px 2fr", "1fr", &[("A", [1, 1, 0, 0])]);
        run(&mut g, |v| insert_track(v, Axis::Column, None)).unwrap();
        assert_eq!(text(&g, "GridColumns"), "100px 2fr 2fr");
        assert_eq!(g[1].text("GridColumn"), "1", "a child before the end does not move");

        let mut e = grid("", "", &[]);
        run(&mut e, |v| insert_track(v, Axis::Row, None)).unwrap();
        assert_eq!(text(&e, "GridRows"), "1fr");
        println!("add at the end: `100px 2fr` → `{}`; an empty list → `{}`", text(&g, "GridColumns"), text(&e, "GridRows"));
    }

    /// A child that spans across the insertion point spans one more; one that starts
    /// at it moves; one that ends at it is untouched.
    #[test]
    fn a_spanning_child_grows_when_a_track_is_added_inside_it() {
        let mut g = grid("1fr 1fr 1fr", "1fr", &[("WIDE", [1, 1, 2, 0]), ("NEXT", [3, 1, 0, 0]), ("AFTER", [2, 1, 0, 0])]);
        let edits = insert_track(&view_of(&g), Axis::Column, Some(0)).unwrap();
        apply(&mut g, &edits);
        assert_eq!(g[1].text("ColumnSpan"), "3", "WIDE covered columns 1–2; the new one is inside it");
        assert_eq!(g[2].text("GridColumn"), "4");
        assert_eq!(g[3].text("GridColumn"), "3");
        // A track added exactly where WIDE ends leaves it alone.
        let mut h = grid("1fr 1fr 1fr", "1fr", &[("WIDE", [1, 1, 2, 0])]);
        run(&mut h, |v| insert_track(v, Axis::Column, Some(1))).unwrap();
        assert_eq!(h[1].text("ColumnSpan"), "2");
        println!("a spanning child: span 2 → {} when a column is added inside it, 2 when added at its end", g[1].text("ColumnSpan"));
    }

    /// A child placed only by the engine stays auto: no edit names it.
    #[test]
    fn an_auto_placed_child_is_left_auto() {
        let g = grid("1fr 1fr", "1fr 1fr", &[("A", [2, 2, 0, 0]), ("AUTO", [0, 0, 0, 0])]);
        let edits = insert_track(&view_of(&g), Axis::Column, Some(0)).unwrap();
        assert!(edits.iter().all(|(id, ..)| id != "AUTO"), "{edits:?}");
        println!("auto-placed child: not named by any of the {} writes", edits.len());
    }

    /// A row that exists only because a child sits in it is written out as `Auto`
    /// before the edit, so the list holds every track the canvas shows.
    #[test]
    fn an_implicit_track_is_written_out_before_the_edit() {
        let mut g = grid("1fr", "", &[("A", [1, 3, 0, 0])]);
        let v = view_of(&g);
        assert_eq!(v.track_count(Axis::Row), 3, "rows 1 and 2 are implicit, row 3 holds A");
        apply(&mut g, &insert_track(&v, Axis::Row, Some(0)).unwrap());
        assert_eq!(text(&g, "GridRows"), "Auto Auto Auto Auto");
        assert_eq!(g[1].text("GridRow"), "4", "A moved down with its track");
        assert_eq!(cells_of(&g)["A"], (3, 0, 1, 1));
        println!("implicit rows: `` → `{}`, A at row {}", text(&g, "GridRows"), g[1].text("GridRow"));
    }

    /// A token the parser cannot read, and one that is a `MinMax`, come through an
    /// edit exactly as written; a `Repeat` axis is refused.
    #[test]
    fn what_the_edit_does_not_touch_it_leaves_as_written() {
        let mut g = grid("1FR nonsense MinMax(10px, 1fr)", "1fr", &[]);
        run(&mut g, |v| insert_track(v, Axis::Column, Some(0))).unwrap();
        assert_eq!(text(&g, "GridColumns"), "1FR 1FR nonsense MinMax(10px, 1fr)");

        let r = grid("100px Repeat(3, 1fr)", "1fr", &[]);
        assert_eq!(insert_track(&view_of(&r), Axis::Column, None), Err(Refusal::Repeat(Axis::Column)));
        assert!(insert_track(&view_of(&r), Axis::Row, None).is_ok(), "the other axis is fine");
        assert_eq!(insert_track(&view_of(&g), Axis::Column, Some(9)), Err(Refusal::NoSuchTrack));
        println!("untouched tokens: `{}`; Repeat axis refused, the other axis offered", text(&g, "GridColumns"));
    }

    // ── Remove (T4) ───────────────────────────────────────────────────────────

    /// AC6 — removing row 1 of a 2-row grid, with a child only in row 1: the list
    /// loses the track, the child survives and becomes auto-placed on that axis, the
    /// notice list names it, and the remaining row fills the container.
    #[test]
    fn removing_a_row_keeps_the_control_that_sat_in_it() {
        let mut g = grid("1fr 1fr", "1fr 1fr", &[("A", [1, 1, 0, 0]), ("B", [2, 2, 0, 0])]);
        let removal = remove_track(&view_of(&g), Axis::Row, 0).expect("a row can be removed");
        apply(&mut g, &removal.edits);
        assert_eq!(text(&g, "GridRows"), "1fr");
        assert_eq!(g[1].text("GridRow"), "0", "A stays, auto-placed on rows");
        assert_eq!(g[2].text("GridRow"), "1", "B moved up with its track");
        assert_eq!(removal.now_auto, vec!["A".to_owned()], "the notice names A and only A");
        assert!(g.iter().any(|c| c.id == "A"), "a control is never removed");
        let o = solve_at(&g, (400.0, 200.0), (400.0, 200.0));
        let client_h = o.containers["G"].client.h;
        assert_eq!(o.containers["G"].rows, vec![client_h], "the remaining row fills the container's client");
        println!("remove row 1: GridRows `1fr 1fr` → `{}`; A auto ({:?}); B at row {}; the row is {} px = the client's height", text(&g, "GridRows"), removal.now_auto, g[2].text("GridRow"), o.containers["G"].rows[0]);
    }

    /// A child that spans across the removed track spans one less; one that starts at
    /// it and spans more keeps its place; one before it is untouched.
    #[test]
    fn removing_a_track_shrinks_what_spans_it() {
        let mut g = grid(
            "1fr 1fr 1fr 1fr",
            "1fr",
            &[("ACROSS", [1, 1, 3, 0]), ("STARTS", [2, 1, 2, 0]), ("BEFORE", [1, 1, 0, 0]), ("AFTER", [4, 1, 0, 0])],
        );
        let removal = remove_track(&view_of(&g), Axis::Column, 1).unwrap();
        apply(&mut g, &removal.edits);
        assert_eq!(text(&g, "GridColumns"), "1fr 1fr 1fr");
        assert_eq!(g[1].text("ColumnSpan"), "2", "covered columns 1–3 and lost one");
        assert_eq!((g[2].text("GridColumn"), g[2].text("ColumnSpan")), ("2".into(), "1".into()), "started at the removed column");
        assert_eq!(g[3].text("GridColumn"), "1", "before the removed column: untouched");
        assert_eq!(g[4].text("GridColumn"), "3", "after it: moved up");
        assert_eq!(removal.now_auto, Vec::<String>::new());
        println!("remove column 2: ACROSS span 3 → {}, STARTS span 2 → {}, AFTER column 4 → {}", g[1].text("ColumnSpan"), g[2].text("ColumnSpan"), g[4].text("GridColumn"));
    }

    /// The last row and the last column cannot be removed; a `Repeat` axis cannot be
    /// removed from; what the parser cannot read is left as written.
    #[test]
    fn a_remove_is_refused_where_it_must_be_and_leaves_the_rest_as_written() {
        let one = grid("1fr", "1fr 1fr", &[]);
        assert_eq!(remove_track(&view_of(&one), Axis::Column, 0), Err(Refusal::LastTrack(Axis::Column)));
        assert!(remove_track(&view_of(&one), Axis::Row, 0).is_ok());
        let r = grid("100px Repeat(2, 1fr)", "1fr 1fr", &[]);
        assert_eq!(remove_track(&view_of(&r), Axis::Column, 0), Err(Refusal::Repeat(Axis::Column)));
        assert_eq!(remove_track(&view_of(&one), Axis::Row, 7), Err(Refusal::NoSuchTrack));
        let mut m = grid("1FR nonsense 2fr", "1fr", &[]);
        let removal = remove_track(&view_of(&m), Axis::Column, 0).unwrap();
        apply(&mut m, &removal.edits);
        assert_eq!(text(&m, "GridColumns"), "nonsense 2fr");
        assert_eq!(removal.record.token, "1FR", "the token is recorded as written");
        println!("remove refused: last column, Repeat axis, no such track; `1FR nonsense 2fr` − track 1 → `{}`", text(&m, "GridColumns"));
    }

    /// An implicit track is written out first, so removing it removes a real token.
    #[test]
    fn an_implicit_track_can_be_removed_once_it_is_written_out() {
        let mut g = grid("1fr", "", &[("A", [1, 3, 0, 0])]);
        let removal = remove_track(&view_of(&g), Axis::Row, 1).unwrap();
        apply(&mut g, &removal.edits);
        assert_eq!(text(&g, "GridRows"), "Auto Auto");
        assert_eq!(g[1].text("GridRow"), "2");
        assert_eq!(cells_of(&g)["A"], (1, 0, 1, 1));
        println!("remove an implicit row: `` → `{}`, A at row {}", text(&g, "GridRows"), g[1].text("GridRow"));
    }

    // ── Restore (T5) ──────────────────────────────────────────────────────────

    /// AC7 — remove the row, change something unrelated, restore: the track and the
    /// child's placement are back exactly, and the unrelated edit remains.
    #[test]
    fn restore_puts_the_row_and_its_control_back_and_keeps_the_unrelated_edit() {
        let mut g = grid("1fr 1fr", "1fr 2fr", &[("A", [1, 1, 0, 0]), ("B", [2, 2, 0, 0])]);
        let removal = remove_track(&view_of(&g), Axis::Row, 0).unwrap();
        apply(&mut g, &removal.edits);
        assert_eq!(text(&g, "GridRows"), "2fr");
        // An unrelated edit elsewhere, made after the removal: B is given a colour and a caption.
        g[2].set_prop("BackgroundColor", PropValue::String("#112233".into()));
        g[2].set_prop("Caption", PropValue::String("kept".into()));
        run(&mut g, |v| restore_track(v, &removal.record)).unwrap();
        assert_eq!(text(&g, "GridRows"), "1fr 2fr", "the track, at its place");
        assert_eq!(g[1].text("GridRow"), "1", "A is back in row 1");
        assert_eq!(g[2].text("GridRow"), "2", "B moved back down with its row");
        assert_eq!((g[2].text("BackgroundColor"), g[2].text("Caption")), ("#112233".into(), "kept".into()), "the unrelated edits remain");
        assert_eq!(cells_of(&g)["A"], (0, 0, 1, 1));
        assert_eq!(cells_of(&g)["B"], (1, 1, 1, 1));
        println!("restore: rows `{}`, A row {}, B row {} with its colour and caption kept", text(&g, "GridRows"), g[1].text("GridRow"), g[2].text("GridRow"));
    }

    /// A held child the developer has placed since is left where they put it; the
    /// index is clamped when the list is shorter; a spanning child grows as for an add.
    #[test]
    fn restore_leaves_a_control_placed_since_and_clamps_the_place() {
        let mut g = grid("1fr 1fr", "1fr 1fr 1fr", &[("A", [1, 3, 0, 0]), ("WIDE", [1, 1, 0, 2])]);
        let removal = remove_track(&view_of(&g), Axis::Row, 2).unwrap();
        apply(&mut g, &removal.edits);
        assert_eq!(removal.now_auto, vec!["A".to_owned()]);
        // The developer puts A in row 2 themselves, and removes another row.
        g[1].set_prop("GridRow", PropValue::Int(2));
        let second = remove_track(&view_of(&g), Axis::Row, 0).unwrap();
        apply(&mut g, &second.edits);
        assert_eq!(text(&g, "GridRows"), "1fr");
        // Restore the first removal: index 2 is past the end of a one-row list.
        run(&mut g, |v| restore_track(v, &removal.record)).unwrap();
        assert_eq!(text(&g, "GridRows"), "1fr 1fr", "added at the end, the list being shorter");
        assert_eq!(g[1].text("GridRow"), "1", "A was placed since (and renumbered by the second removal); it keeps its own place");
        println!("restore after a shorter list: rows `{}`, A placed by the developer stays at row {}", text(&g, "GridRows"), g[1].text("GridRow"));
    }

    // ── Merge and split (T6) ──────────────────────────────────────────────────

    /// AC9 — a 2×1 range holding one control: Merge gives it `ColumnSpan = 2` at the
    /// range's top-left; Split returns it to one cell. The engine agrees at each step.
    #[test]
    fn merge_makes_the_control_span_the_range_and_split_undoes_it() {
        let mut g = grid("1fr 1fr 1fr", "1fr 1fr", &[("A", [2, 1, 0, 0])]);
        let range = CellRange::between((0, 1), (0, 2));
        assert_eq!(controls_in(&view_of(&g), range).len(), 1);
        run(&mut g, |v| merge(v, range)).expect("one control in a two-cell range merges");
        assert_eq!((g[1].text("GridColumn"), g[1].text("GridRow"), g[1].text("ColumnSpan")), ("2".into(), "1".into(), "2".into()));
        assert_eq!(cells_of(&g)["A"], (0, 1, 1, 2));
        // The cell the range starts at, or any cell it now covers, splits it.
        run(&mut g, |v| split_cell(v, 0, 2)).expect("a spanning control splits from any cell it covers");
        assert_eq!(g[1].text("ColumnSpan"), "1");
        assert_eq!(cells_of(&g)["A"], (0, 1, 1, 1), "back in its top-left cell");
        println!("merge 2×1 → ColumnSpan {}; split → {}, still at column {}", 2, g[1].text("ColumnSpan"), g[1].text("GridColumn"));
    }

    /// A block of cells: rows and columns both, the corners given either way round;
    /// an auto-placed control is pinned to the block's top-left.
    #[test]
    fn a_block_merges_in_both_directions() {
        let mut g = grid("1fr 1fr 1fr", "1fr 1fr 1fr", &[("A", [0, 0, 0, 0])]);
        let range = CellRange::between((2, 2), (1, 1));
        assert_eq!(range, CellRange { row: 1, column: 1, rows: 2, columns: 2 });
        // A is auto-placed at (0, 0): it is not in a block elsewhere.
        assert_eq!(merge(&view_of(&g), range), Err(Refusal::NoControlInRange));
        run(&mut g, |v| merge(v, CellRange::between((0, 0), (1, 1)))).unwrap();
        assert_eq!(cells_of(&g)["A"], (0, 0, 2, 2));
        run(&mut g, |v| split_cell(v, 1, 1)).unwrap();
        assert_eq!(cells_of(&g)["A"], (0, 0, 1, 1));
        println!("block merge 2×2 → (0,0) spanning 2×2; split back to 1×1");
    }

    /// The reasons a merge or a split is refused, each its own — the toolbar says why.
    #[test]
    fn merge_and_split_say_why_they_are_refused() {
        let g = grid("1fr 1fr", "1fr", &[("A", [1, 1, 0, 0]), ("B", [2, 1, 0, 0])]);
        let v = view_of(&g);
        assert_eq!(merge(&v, CellRange::between((0, 0), (0, 1))), Err(Refusal::SeveralControlsInRange), "two controls");
        assert_eq!(merge(&v, CellRange::single(0, 0)), Err(Refusal::OneCell), "one cell has nothing to merge");
        let empty = grid("1fr 1fr", "1fr", &[]);
        assert_eq!(merge(&view_of(&empty), CellRange::between((0, 0), (0, 1))), Err(Refusal::NoControlInRange), "no control");
        assert_eq!(split_cell(&v, 0, 0), Err(Refusal::NothingToSplit), "a one-cell control");
        // A control that spans into the range counts in it, so it counts once.
        let wide = grid("1fr 1fr 1fr", "1fr", &[("W", [1, 1, 2, 0]), ("X", [3, 1, 0, 0])]);
        let r = CellRange::between((0, 1), (0, 2));
        assert_eq!(controls_in(&view_of(&wide), r).len(), 2, "W spans into column 2, X is in column 3");
        // A control that only TOUCHES the range — the row above, the column beside — is not in it.
        let touching = grid("1fr 1fr", "1fr 1fr", &[("T", [1, 1, 0, 0])]);
        let below = CellRange::between((1, 0), (1, 1));
        let beside = CellRange::between((0, 1), (1, 1));
        assert_eq!(merge(&view_of(&touching), below), Err(Refusal::NoControlInRange), "T is in the row above");
        assert_eq!(merge(&view_of(&touching), beside), Err(Refusal::NoControlInRange), "T is in the column beside");
        println!("refusals: several controls, one cell, no control, nothing to split; a spanning control counts in every cell it covers");
    }

    // ── Resize a pair of tracks (T7) ──────────────────────────────────────────

    /// The sizes of `axis` after the form is laid out with the written text.
    fn sizes_of(controls: &[Control], axis: Axis) -> Vec<f32> {
        let o = solve_at(controls, (400.0, 200.0), (400.0, 200.0));
        match axis {
            Axis::Column => o.containers["G"].columns.clone(),
            Axis::Row => o.containers["G"].rows.clone(),
        }
    }

    /// Drag the line after track `line` by `delta`, write the result, lay it out, and
    /// answer `(text, sizes before, sizes after)`. The assertion every caller makes is
    /// that the two tracks moved by `delta` and nothing else did.
    fn drag(mut g: Vec<Control>, axis: Axis, line: usize, delta: f32) -> (String, Vec<f32>, Vec<f32>) {
        let before = sizes_of(&g, axis);
        let edits = resize_pair(&view_of(&g), axis, line, delta).expect("this line can be dragged");
        apply(&mut g, &edits);
        (text(&g, axis.tracks_key()), before, sizes_of(&g, axis))
    }

    fn assert_moved(before: &[f32], after: &[f32], line: usize, delta: f32, tol: f32, what: &str) {
        assert_eq!(before.len(), after.len(), "{what}: the track count is kept");
        for (k, (b, a)) in before.iter().zip(after).enumerate() {
            let want = if k == line { b + delta } else if k == line + 1 { b - delta } else { *b };
            assert!((a - want).abs() <= tol, "{what}: track {k} is {a}, wanted {want} (±{tol}); before {before:?} after {after:?}");
        }
    }

    /// AC10 — `1fr 1fr` dragged to the 25 % mark is `0.5fr 1.5fr` (the total, 2, is
    /// kept); `100px 200px` + 50 is `150px 150px`; an `Auto` track dragged is `Npx`.
    #[test]
    fn a_drag_is_written_in_each_tracks_own_unit() {
        // 1fr 1fr to the 25 % mark of the pair.
        let g = grid("1fr 1fr", "1fr", &[]);
        let w = sizes_of(&g, Axis::Column);
        let to_quarter = w[0] + w[1];
        let d = to_quarter * 0.25 - w[0];
        let (t, before, after) = drag(g, Axis::Column, 0, d);
        assert_eq!(t, "0.5fr 1.5fr");
        assert_moved(&before, &after, 0, d, 0.5, "1fr 1fr");
        println!("AC10: `1fr 1fr` to the 25 % mark → `{t}` (sizes {before:?} → {after:?})");

        let (t, before, after) = drag(grid("100px 200px", "1fr", &[]), Axis::Column, 0, 50.0);
        assert_eq!(t, "150px 150px");
        assert_moved(&before, &after, 0, 50.0, 0.01, "100px 200px");
        println!("AC10: `100px 200px` + 50 → `{t}`");

        // An Auto track becomes a pixel track of the dragged size; the fraction scales.
        let (t, before, after) = drag(grid("Auto 1fr", "1fr", &[]), Axis::Column, 0, 50.0);
        assert_eq!(t, "50px 1fr", "the only flexible track takes what is left: its factor stays as typed");
        assert_moved(&before, &after, 0, 50.0, 0.5, "Auto 1fr");
        println!("AC10: `Auto 1fr` + 50 → `{t}`");
        // A flexible track that takes only part of the space keeps taking the part it took.
        let (t, before, after) = drag(grid("Auto 0.5fr", "1fr", &[]), Axis::Column, 0, 50.0);
        assert!(t.starts_with("50px 0.4"), "{t}");
        assert_moved(&before, &after, 0, 50.0, 0.6, "Auto 0.5fr");
        println!("partial fraction: `Auto 0.5fr` + 50 → `{t}`; sizes {before:?} → {after:?}");
        // …from either side of the line.
        let (t, before, after) = drag(grid("0.5fr 100px", "1fr", &[]), Axis::Column, 0, -30.0);
        assert!(t.starts_with("0.44") && t.ends_with("130px"), "{t}");
        assert_moved(&before, &after, 0, -30.0, 0.6, "0.5fr 100px");
        println!("partial fraction, other side: `0.5fr 100px` − 30 → `{t}`; sizes {before:?} → {after:?}");
        // A collapsed `0fr` track has no size to scale from; it takes its share of the pair.
        let (t, before, after) = drag(grid("0fr 1fr", "1fr", &[]), Axis::Column, 0, 99.0);
        assert_eq!(t, "0.25fr 0.75fr");
        assert_moved(&before, &after, 0, 99.0, 0.6, "0fr 1fr");
        println!("collapsed track: `0fr 1fr` + 99 → `{t}`; sizes {before:?} → {after:?}");
    }

    /// `%` stays `%`; a pixel track beside a fraction scales the fraction by its change
    /// of size so a third, flexible track does not move; a gap is kept.
    #[test]
    fn percent_and_mixed_units_move_only_the_two_tracks_they_touch() {
        let (t, before, after) = drag(grid("50% 50%", "1fr", &[]), Axis::Column, 0, 40.0);
        assert!(t.ends_with('%') && t.contains("% "), "{t}");
        assert_moved(&before, &after, 0, 40.0, 0.5, "50% 50%");

        let (t, before, after) = drag(grid("100px 1fr 1fr", "1fr", &[]), Axis::Column, 0, 30.0);
        assert!(t.starts_with("130px "), "{t}");
        assert_moved(&before, &after, 0, 30.0, 0.5, "100px 1fr 1fr");
        println!("mixed: `100px 1fr 1fr` + 30 → `{t}`; sizes {before:?} → {after:?}");

        // Between two flexible tracks with a third: the third keeps its size.
        let (t, before, after) = drag(grid("1fr 2fr 1fr", "1fr", &[]), Axis::Column, 0, 25.0);
        assert_moved(&before, &after, 0, 25.0, 0.5, "1fr 2fr 1fr");
        println!("three fractions: `1fr 2fr 1fr` + 25 → `{t}`; sizes {before:?} → {after:?}");

        // A gap between the tracks is kept as it was.
        let mut gapped = grid("1fr 1fr 1fr", "1fr", &[]);
        gapped[0].set_prop("ColumnGap", PropValue::Int(10));
        let (t, before, after) = drag(gapped, Axis::Column, 1, -20.0);
        assert_moved(&before, &after, 1, -20.0, 0.5, "gap 10");
        println!("gap 10: `1fr 1fr 1fr` the second line −20 → `{t}`; sizes {before:?} → {after:?}");
    }

    /// AC11 — no drag makes a track narrower than 8 px, in either direction, and a drag
    /// that cannot move anything writes nothing.
    #[test]
    fn no_drag_goes_below_eight_pixels() {
        let (t, _, after) = drag(grid("100px 100px", "1fr", &[]), Axis::Column, 0, -500.0);
        assert_eq!(t, "8px 192px");
        assert!(after[0] >= 8.0 - 0.01);
        let (t, _, after) = drag(grid("100px 100px", "1fr", &[]), Axis::Column, 0, 500.0);
        assert_eq!(t, "192px 8px");
        assert!(after[1] >= 8.0 - 0.01);
        let (t, _, after) = drag(grid("1fr 1fr", "1fr", &[]), Axis::Column, 0, -10_000.0);
        assert!(after[0] >= 8.0 - 0.5, "{t} → {after:?}");
        assert_eq!(resize_pair(&view_of(&grid("100px 100px", "1fr", &[])), Axis::Column, 0, 0.0), Ok(Vec::new()));
        println!("floor: `100px 100px` −500 → `8px 192px`, +500 → `192px 8px`; `1fr 1fr` −10000 → {after:?}; a zero drag writes nothing");
    }

    /// What cannot be dragged: a `MinMax` or a `Repeat` track, a line with no track
    /// after it, a scaled container — and an implicit track is written out first.
    #[test]
    fn some_lines_cannot_be_dragged_and_an_implicit_track_can() {
        let v = |c: &str| view_of(&grid(c, "1fr", &[]));
        assert_eq!(resize_pair(&v("MinMax(50px, 1fr) 1fr"), Axis::Column, 0, 10.0), Err(Refusal::FixedTrack));
        assert_eq!(resize_pair(&v("1fr MinMax(50px, 1fr)"), Axis::Column, 0, 10.0), Err(Refusal::FixedTrack));
        assert_eq!(resize_pair(&v("1fr Repeat(2, 1fr)"), Axis::Column, 0, 10.0), Err(Refusal::Repeat(Axis::Column)));
        assert_eq!(resize_pair(&v("1fr 1fr"), Axis::Column, 1, 10.0), Err(Refusal::NoSuchTrack), "the outer border is not a line");
        let mut scaled = v("1fr 1fr");
        scaled.scaled = true;
        assert_eq!(resize_pair(&scaled, Axis::Column, 0, 10.0), Err(Refusal::Scaled));
        // Rows that exist only because children sit in them.
        let g = grid("1fr", "", &[("A", [1, 1, 0, 0]), ("B", [1, 3, 0, 0])]);
        let (t, before, after) = drag(g, Axis::Row, 0, 12.0);
        assert_eq!(before.len(), after.len(), "the same three rows, now all written out");
        assert_eq!(t.split(' ').count(), 3, "{t}");
        // The two tracks moved by the drag. With no flexible track the third row is `Auto`,
        // and an `Auto` track shares what is left (the engine's stretch), so it may take
        // up what the pair gave back — nothing else is promised on such an axis.
        assert!((after[0] - before[0] - 12.0).abs() < 0.6 && (before[1] - after[1] - 12.0).abs() < 0.6, "{before:?} → {after:?}");
        println!("refused: MinMax either side, Repeat axis, the border, a scaled container; implicit rows `` → `{t}` ({before:?} → {after:?})");
    }

    // ── The canvas and Run Form agree (T8, R25, R26) ──────────────────────────

    /// What Run Form's host does with a control: every property stringified and merged.
    struct Stringified;
    impl crate::render::FormState for Stringified {
        fn live(&self, base: &Control) -> Control {
            let props: HashMap<String, String> = base.properties.iter().map(|(k, v)| (k.clone(), v.to_xml_string())).collect();
            crate::render::merge_props(base, props.iter())
        }
    }

    /// Every control's rectangle as laid out for a 400 × 200 surface.
    fn laid(controls: &[Control], state: &dyn crate::render::FormState) -> HashMap<String, (f32, f32, f32, f32)> {
        let bag = std::collections::BTreeMap::new();
        let spec = crate::layout::apply::FormSpec {
            designed_size: (400.0, 200.0),
            layout: &bag,
            breakpoints: &[],
            system_text_factor: 1.0,
            pinned_breakpoint: None,
            pinned_font_scale: None,
        };
        let ctx = egui::Context::default();
        let p = crate::layout::apply::prepare(&ctx, controls, state, &spec, egui::vec2(400.0, 200.0));
        p.layout.rects.iter().map(|(id, r)| (id.clone(), (r.x, r.y, r.w, r.h))).collect()
    }

    /// The form as the designer holds it and as Run Form reads the saved file.
    fn both_ways(controls: &[Control]) -> (HashMap<String, (f32, f32, f32, f32)>, HashMap<String, (f32, f32, f32, f32)>) {
        let mut form = crate::model::Form::new("F", "F", 400, 200);
        form.responsive = true;
        form.controls = controls.to_vec();
        let saved = crate::xml::form_to_string(&form).expect("the form saves");
        let loaded = crate::xml::load_form_from_str(&saved).expect("the saved form loads");
        (laid(controls, &crate::render::DesignedState), laid(&loaded.controls, &Stringified))
    }

    /// AC14 / R26 — after each operation, the layout of the form as the designer holds it
    /// and the layout of what is saved, as Run Form reads it, give every control the same
    /// rectangle: an edit writes properties, and the one engine places them.
    #[test]
    fn the_canvas_and_run_form_place_an_edited_grid_the_same() {
        let base = || grid("1fr 2fr 1fr", "1fr 1fr", &[("A", [1, 1, 0, 0]), ("B", [2, 1, 2, 0]), ("C", [1, 2, 0, 0]), ("D", [0, 0, 0, 0])]);
        type Op = Box<dyn Fn(&mut Vec<Control>)>;
        let ops: Vec<(&str, Op)> = vec![
            ("add a column", Box::new(|g| run(g, |v| insert_track(v, Axis::Column, Some(0))).unwrap())),
            ("add a row at the end", Box::new(|g| run(g, |v| insert_track(v, Axis::Row, None)).unwrap())),
            ("remove a column", Box::new(|g| run(g, |v| remove_track(v, Axis::Column, 1).map(|r| r.edits)).unwrap())),
            ("remove a row", Box::new(|g| run(g, |v| remove_track(v, Axis::Row, 0).map(|r| r.edits)).unwrap())),
            (
                "remove then restore",
                Box::new(|g| {
                    let r = remove_track(&view_of(g), Axis::Column, 0).unwrap();
                    apply(g, &r.edits);
                    run(g, |v| restore_track(v, &r.record)).unwrap();
                }),
            ),
            ("merge cells", Box::new(|g| run(g, |v| merge(v, CellRange::between((1, 1), (1, 2)))).unwrap())),
            ("split a cell", Box::new(|g| run(g, |v| split_cell(v, 0, 2)).unwrap())),
            ("drag a line", Box::new(|g| run(g, |v| resize_pair(v, Axis::Column, 0, 37.0)).unwrap())),
        ];
        let (mut controls_checked, mut differ) = (0usize, 0usize);
        for (name, op) in &ops {
            let mut g = base();
            op(&mut g);
            let (designer, run_form) = both_ways(&g);
            assert_eq!(designer.len(), run_form.len(), "{name}: the same controls are placed");
            for (id, d) in &designer {
                controls_checked += 1;
                let r = run_form.get(id).unwrap_or_else(|| panic!("{name}: Run Form does not place {id}"));
                if (d.0 - r.0).abs() > 0.01 || (d.1 - r.1).abs() > 0.01 || (d.2 - r.2).abs() > 0.01 || (d.3 - r.3).abs() > 0.01 {
                    differ += 1;
                    println!("{name}: {id} designer {d:?} run-form {r:?}");
                }
            }
            // The edit did something: at least one control sits somewhere new.
            let before = both_ways(&base()).0;
            assert!(designer.iter().any(|(id, r)| before.get(id) != Some(r)) || *name == "remove then restore", "{name}: nothing moved");
        }
        assert_eq!(differ, 0);
        println!("canvas vs Run Form: {} operations, {controls_checked} control placements compared, {differ} rectangles differ", ops.len());
    }
}
