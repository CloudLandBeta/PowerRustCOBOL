// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Grid** (spec 056 R55–R56).
//!
//! Placement first: items with both a `GridColumn` and a `GridRow` take their
//! cell; items locked to a row take the first free column in it; the rest
//! are auto-placed row-major in item order, from a cursor that only moves
//! forward — no back-filling ("sparse", never "dense"). A cell beyond the
//! template extends the implicit grid with `Auto` tracks; nothing is dropped.
//!
//! Then the tracks, the CSS way for items of known size: fixed and percentage
//! tracks take their size; an `Auto` minimum is the largest item that sits
//! only in that track; a `MinMax` grows toward its maximum from the free
//! space; `fr` tracks share what remains — a track whose content is larger
//! than its share keeps its content size and leaves the share to the others;
//! and when no track is flexible, `Auto` tracks stretch to fill.

use std::collections::HashSet;

use crate::layout::defaults;
use crate::layout::flex::{align_of, gaps, Align};
use crate::layout::limits::clamp;
use crate::layout::props::{self, Limits, PropSource};
use crate::layout::tracks::{self, Breadth, Track, TrackList};
use crate::layout::LRect;
use crate::model::Control;

/// A grid container.
#[derive(Clone, Debug, PartialEq)]
pub struct Container {
    pub columns: TrackList,
    pub rows: TrackList,
    pub row_gap: f32,
    pub column_gap: f32,
    pub justify_items: Align,
    pub align_items: Align,
    /// Its merged areas (spec 088 A4), from `GridMerges`.
    pub merges: Vec<Area>,
}

/// One grid item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    /// Width and height before layout.
    pub size: (f32, f32),
    /// 1-based; 0 is auto-placed.
    pub column: usize,
    pub row: usize,
    pub column_span: usize,
    pub row_span: usize,
    /// `None` is `Auto`: the container's `JustifyItems` / `AlignItems`.
    pub justify_self: Option<Align>,
    pub align_self: Option<Align>,
    pub width: Limits,
    pub height: Limits,
}

/// The cell an item sits in, 0-based: its first row and column and how many of
/// each it spans. Auto-placed items have one too — the engine's answer, which an
/// editor must agree with (spec 088).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub row: usize,
    pub column: usize,
    pub row_span: usize,
    pub column_span: usize,
}

impl Cell {
    /// Whether cell `(row, column)` is one of these.
    pub fn contains(&self, row: usize, column: usize) -> bool {
        (self.row..self.row + self.row_span).contains(&row) && (self.column..self.column + self.column_span).contains(&column)
    }

    /// Whether the two share a cell.
    pub fn overlaps(&self, o: &Cell) -> bool {
        self.row < o.row + o.row_span
            && o.row < self.row + self.row_span
            && self.column < o.column + o.column_span
            && o.column < self.column + self.column_span
    }

    /// Whether every cell of `o` is one of these.
    pub fn covers(&self, o: &Cell) -> bool {
        self.row <= o.row
            && self.column <= o.column
            && o.row + o.row_span <= self.row + self.row_span
            && o.column + o.column_span <= self.column + self.column_span
    }
}

/// A **merged area** of a grid (spec 088 A4): a rectangle of two or more cells that
/// lays out as one. Held as the [`Cell`] it covers, 0-based.
pub type Area = Cell;

/// Read `GridMerges` (R36): spreadsheet ranges `R<row>C<column>:R<row>C<column>`,
/// 1-based, top-left to bottom-right (either corner order is read), separated by
/// spaces, commas or semicolons, in any case. An entry that is malformed, covers a
/// single cell, or overlaps an earlier one is left out — never guessed at.
pub fn parse_merges(text: &str) -> Vec<Area> {
    fn corner(s: &str) -> Option<(usize, usize)> {
        let s = s.trim().to_ascii_uppercase();
        let rest = s.strip_prefix('R')?;
        let (r, c) = rest.split_once('C')?;
        let (r, c) = (r.parse::<usize>().ok()?, c.parse::<usize>().ok()?);
        (r >= 1 && c >= 1).then(|| (r - 1, c - 1))
    }
    let mut out: Vec<Area> = Vec::new();
    for token in text.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';').filter(|t| !t.is_empty()) {
        let Some((a, b)) = token.split_once(':') else { continue };
        let (Some((r0, c0)), Some((r1, c1))) = (corner(a), corner(b)) else { continue };
        let area = Area {
            row: r0.min(r1),
            column: c0.min(c1),
            row_span: r0.abs_diff(r1) + 1,
            column_span: c0.abs_diff(c1) + 1,
        };
        if area.row_span * area.column_span >= 2 && !out.iter().any(|o| o.overlaps(&area)) {
            out.push(area);
        }
    }
    out
}

/// Write areas as `GridMerges` reads them, in the order given.
pub fn format_merges(areas: &[Area]) -> String {
    areas
        .iter()
        .map(|a| format!("R{}C{}:R{}C{}", a.row + 1, a.column + 1, a.row + a.row_span, a.column + a.column_span))
        .collect::<Vec<_>>()
        .join(" ")
}

/// What [`solve`] returns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Solved {
    /// One rectangle per item, in the order given.
    pub rects: Vec<LRect>,
    /// One cell per item, in the order given.
    pub cells: Vec<Cell>,
    /// The column widths and row heights chosen.
    pub columns: Vec<f32>,
    pub rows: Vec<f32>,
    /// The tracks plus gaps: what the grid needs on each axis.
    pub content: (f32, f32),
}

pub fn container(src: &dyn PropSource) -> Container {
    let (row_gap, column_gap) = gaps(src);
    Container {
        columns: tracks::parse(&src.text("GridColumns")),
        rows: tracks::parse(&src.text("GridRows")),
        row_gap,
        column_gap,
        justify_items: align_of(&src.text("JustifyItems")).unwrap_or(Align::Stretch),
        align_items: align_of(&src.text("AlignItems")).unwrap_or(Align::Stretch),
        merges: parse_merges(&src.text("GridMerges")),
    }
}

pub fn item(c: &Control, size: (f32, f32)) -> Item {
    let whole = |k: &str| c.number(k).max(0.0) as usize;
    Item {
        size,
        column: whole("GridColumn"),
        row: whole("GridRow"),
        column_span: whole("ColumnSpan").max(1),
        row_span: whole("RowSpan").max(1),
        justify_self: align_of(&c.text("JustifySelf")),
        align_self: align_of(&c.text("AlignSelf")),
        width: props::width_limits(c),
        height: props::height_limits(c),
    }
}

/// Where the items sit (spec 056 R55; spec 088 A4).
struct Placed {
    /// Each item's first cell, 0-based (row, column) — an area's top-left for an item in it.
    cells: Vec<(usize, usize)>,
    /// The area each item is in, by index into the container's merges.
    areas: Vec<Option<usize>>,
    rows: usize,
    columns: usize,
}

impl Placed {
    /// The rows and columns item `i` covers: its area's, or its own span.
    fn span(&self, i: usize, it: &Item, merges: &[Area]) -> (usize, usize) {
        match self.areas[i] {
            Some(a) => (merges[a].row_span, merges[a].column_span),
            None => (it.row_span, it.column_span),
        }
    }
}

/// The placement as it is built: who is where, which cells are spoken for, and how
/// many items each area holds.
struct Board<'a> {
    items: &'a [Item],
    merges: &'a [Area],
    cell: Vec<Option<(usize, usize)>>,
    area_of: Vec<Option<usize>>,
    held: Vec<usize>,
    taken: HashSet<(usize, usize)>,
}

impl Board<'_> {
    /// An item's own footprint at `(r, c)` is free — no item there, and no area (A4).
    fn fits(&self, r: usize, c: usize, it: &Item) -> bool {
        (r..r + it.row_span).all(|rr| (c..c + it.column_span).all(|cc| !self.taken.contains(&(rr, cc))))
    }

    /// The empty area whose top-left cell is `(r, c)`: one cell to automatic
    /// placement (R38), so an item reaching it takes the whole area.
    fn open_area(&self, r: usize, c: usize) -> Option<usize> {
        self.merges.iter().position(|a| a.row == r && a.column == c).filter(|&a| self.held[a] == 0)
    }

    /// Put item `i` at `(r, c)` — or in area `a`, which it then fills. Answers how many
    /// columns it took.
    fn put(&mut self, i: usize, r: usize, c: usize, a: Option<usize>) -> usize {
        match a {
            Some(a) => {
                let area = self.merges[a];
                self.cell[i] = Some((area.row, area.column));
                self.area_of[i] = Some(a);
                self.held[a] += 1;
                area.column_span
            }
            None => {
                let it = self.items[i];
                self.cell[i] = Some((r, c));
                take(&mut self.taken, r, c, it.row_span, it.column_span);
                it.column_span
            }
        }
    }
}

fn take(taken: &mut HashSet<(usize, usize)>, r: usize, c: usize, rows: usize, columns: usize) {
    for rr in r..r + rows {
        for cc in c..c + columns {
            taken.insert((rr, cc));
        }
    }
}

/// Where each item sits. An item written inside an area is in that area (R37); an
/// area's cells are never anyone else's; and an empty area is one cell to the
/// automatic placement below (R38).
fn place(items: &[Item], explicit_columns: usize, merges: &[Area]) -> Placed {
    let n = items.len();
    let mut b = Board {
        items,
        merges,
        cell: vec![None; n],
        area_of: vec![None; n],
        held: vec![0; merges.len()],
        taken: HashSet::new(),
    };
    for a in merges {
        take(&mut b.taken, a.row, a.column, a.row_span, a.column_span);
    }

    // The implicit grid's width: the template, widened by explicit columns and areas.
    let mut columns = explicit_columns.max(1);
    for it in items {
        if it.column > 0 {
            columns = columns.max(it.column - 1 + it.column_span);
        } else {
            columns = columns.max(it.column_span);
        }
    }
    for a in merges {
        columns = columns.max(a.column + a.column_span);
    }

    // 1. Items with both lines given — in an area when the cell is one of its own.
    for (i, it) in items.iter().enumerate() {
        if it.column > 0 && it.row > 0 {
            let (r, c) = (it.row - 1, it.column - 1);
            let a = merges.iter().position(|a| a.contains(r, c));
            b.put(i, r, c, a);
        }
    }
    // 2. Items locked to a row: the first free column, a cursor per row.
    let mut row_cursor: std::collections::HashMap<usize, usize> = Default::default();
    for (i, it) in items.iter().enumerate() {
        if it.row > 0 && it.column == 0 {
            let r = it.row - 1;
            let mut c = *row_cursor.get(&r).unwrap_or(&0);
            let a = loop {
                if let Some(a) = b.open_area(r, c) {
                    break Some(a);
                }
                if c + it.column_span <= columns && b.fits(r, c, it) {
                    break None;
                }
                c += 1;
                if c + it.column_span > columns {
                    // Past the last column: the row grows the grid.
                    columns = c + it.column_span;
                }
            };
            let width = b.put(i, r, c, a);
            row_cursor.insert(r, c + width);
        }
    }
    // 3. Everything else, row-major from a cursor that never moves back.
    let (mut cr, mut cc) = (0usize, 0usize);
    for (i, it) in items.iter().enumerate() {
        if b.cell[i].is_some() {
            continue;
        }
        if it.column > 0 {
            let c = it.column - 1;
            if c < cc {
                cr += 1;
            }
            let a = loop {
                if let Some(a) = b.open_area(cr, c) {
                    break Some(a);
                }
                if b.fits(cr, c, it) {
                    break None;
                }
                cr += 1;
            };
            cc = c + b.put(i, cr, c, a);
            continue;
        }
        let a = loop {
            if let Some(a) = b.open_area(cr, cc) {
                break Some(a);
            }
            if cc + it.column_span > columns {
                cr += 1;
                cc = 0;
                continue;
            }
            if b.fits(cr, cc, it) {
                break None;
            }
            cc += 1;
        };
        cc += b.put(i, cr, cc, a);
    }

    let area_of = b.area_of;
    let cells: Vec<(usize, usize)> = b.cell.into_iter().map(|c| c.unwrap_or_default()).collect();
    let rows = items
        .iter()
        .enumerate()
        .map(|(i, it)| cells[i].0 + area_of[i].map_or(it.row_span, |a| merges[a].row_span))
        .chain(merges.iter().map(|a| a.row + a.row_span))
        .max()
        .unwrap_or(0);
    Placed { cells, areas: area_of, rows, columns }
}

/// What each item asks of one axis's tracks: (first track, span, size). The items an
/// area holds ask once, together — side by side, so their widths and the gaps
/// between them, and the tallest of them (A4).
fn spans_on(items: &[Item], p: &Placed, merges: &[Area], gap: f32, columns: bool) -> Vec<(usize, usize, f32)> {
    let own = |it: &Item| if columns { clamp(it.size.0, it.width) } else { clamp(it.size.1, it.height) };
    let mut out: Vec<(usize, usize, f32)> = items
        .iter()
        .enumerate()
        .filter(|(i, _)| p.areas[*i].is_none())
        .map(|(i, it)| {
            let (row, column) = p.cells[i];
            if columns {
                (column, it.column_span, own(it))
            } else {
                (row, it.row_span, own(it))
            }
        })
        .collect();
    for (a, area) in merges.iter().enumerate() {
        let sizes: Vec<f32> = items.iter().zip(&p.areas).filter(|(_, x)| **x == Some(a)).map(|(it, _)| own(it)).collect();
        if sizes.is_empty() {
            continue;
        }
        if columns {
            let w = sizes.iter().sum::<f32>() + gap * (sizes.len() - 1) as f32;
            out.push((area.column, area.column_span, w));
        } else {
            out.push((area.row, area.row_span, sizes.iter().copied().fold(0.0, f32::max)));
        }
    }
    out
}

/// Size one axis. `spans` is each item's (first track, span, size, limits)
/// on this axis; `avail` is `None` when the container sizes to its content.
fn size_tracks(tracks: &[Track], avail: Option<f32>, gap: f32, spans: &[(usize, usize, f32)]) -> Vec<f32> {
    let n = tracks.len();
    let gaps = n.saturating_sub(1) as f32 * gap;
    // The largest item that sits only in each track.
    let mut content = vec![0.0f32; n];
    for &(start, span, size) in spans {
        if span == 1 && start < n {
            content[start] = content[start].max(size);
        }
    }
    let definite = |b: Breadth, i: usize| -> Option<f32> {
        match b {
            Breadth::Px(v) => Some(v),
            Breadth::Percent(p) => avail.map(|a| p / defaults::PERCENT * a),
            Breadth::Auto => Some(content[i]),
            Breadth::Fr(_) => None,
        }
    };
    let flexible = |i: usize| matches!(tracks[i].max, Breadth::Fr(_));
    let fr = |i: usize| match tracks[i].max {
        Breadth::Fr(v) => v,
        _ => 0.0,
    };
    let mut base: Vec<f32> = (0..n).map(|i| definite(tracks[i].min, i).unwrap_or(content[i])).collect();
    let limit: Vec<f32> = (0..n)
        .map(|i| if flexible(i) { base[i] } else { definite(tracks[i].max, i).unwrap_or(base[i]).max(base[i]) })
        .collect();

    // Grow toward each non-flexible maximum from the free space, equally.
    if let Some(a) = avail {
        for _ in 0..=n {
            let free = a - gaps - base.iter().sum::<f32>();
            let open: Vec<usize> = (0..n).filter(|&i| !flexible(i) && limit[i] > base[i] + defaults::EPSILON).collect();
            if free <= defaults::EPSILON || open.is_empty() {
                break;
            }
            let share = free / open.len() as f32;
            for i in open {
                base[i] = (base[i] + share).min(limit[i]);
            }
        }
    }

    // `fr` tracks share what remains; one whose content outgrows its share
    // keeps its size and drops out of the sharing.
    let flex: Vec<usize> = (0..n).filter(|&i| flexible(i)).collect();
    if !flex.is_empty() {
        let unit = match avail {
            Some(a) => {
                let mut sharing = flex.clone();
                let mut unit = 0.0;
                for _ in 0..=n {
                    let fixed: f32 = (0..n).filter(|i| !sharing.contains(i)).map(|i| base[i]).sum();
                    let left = (a - gaps - fixed).max(0.0);
                    let factors: f32 = sharing.iter().map(|&i| fr(i)).sum::<f32>().max(1.0);
                    unit = left / factors;
                    let before = sharing.len();
                    sharing.retain(|&i| base[i] <= unit * fr(i) + defaults::EPSILON);
                    if sharing.len() == before {
                        break;
                    }
                }
                unit
            }
            // Content-sized: the smallest unit that holds every track's content.
            None => flex
                .iter()
                .filter(|&&i| fr(i) > 0.0)
                .map(|&i| base[i] / fr(i))
                .fold(0.0f32, f32::max),
        };
        for &i in &flex {
            base[i] = base[i].max(unit * fr(i));
        }
    } else if let Some(a) = avail {
        // No flexible track: `Auto` tracks share the leftover (CSS stretch).
        let autos: Vec<usize> = (0..n).filter(|&i| tracks[i].max == Breadth::Auto).collect();
        let free = a - gaps - base.iter().sum::<f32>();
        if free > defaults::EPSILON && !autos.is_empty() {
            let share = free / autos.len() as f32;
            for i in autos {
                base[i] += share;
            }
        }
    }
    base
}

/// The smallest client a grid can lay `items` out in (R18): on each axis the
/// sum of every track's minimum — fixed tracks their size, a percentage
/// nothing, `Auto` and `fr` tracks the largest item that sits only in them —
/// plus the gaps. `AutoFill` repeats once.
pub fn min_size(c: &Container, items: &[Item]) -> (f32, f32) {
    min_size_at(c, items, None)
}

/// [`min_size`], and when `width` is the client width a grid is laid out at,
/// its height is what the rows it forms THERE need: a `Repeat(AutoFill, …)`
/// column list holds as many columns as fit that width, as [`solve`] gives
/// it, instead of one (spec 056 R18). The width stays the one-column minimum.
/// `items` must be in reading order, as [`solve`] is given them.
pub fn min_size_at(c: &Container, items: &[Item], width: Option<f32>) -> (f32, f32) {
    let mut columns_t = c.columns.expand(None, c.column_gap);
    let p = place(items, columns_t.len(), &c.merges);
    let implicit = Track { min: Breadth::Auto, max: Breadth::Auto };
    columns_t.resize(p.columns.max(columns_t.len()), implicit);
    // The rows the items form at the laid-out width.
    let at_width = match width {
        Some(w) if c.columns.auto_fills() => Some(place(items, c.columns.expand(Some(w), c.column_gap).len(), &c.merges)),
        _ => None,
    };
    let row_p = at_width.as_ref().unwrap_or(&p);
    let mut rows_t = c.rows.expand(None, c.row_gap);
    rows_t.resize(row_p.rows.max(rows_t.len()), implicit);
    let axis = |tracks: &[Track], gap: f32, spans: Vec<(usize, usize, f32)>| {
        let mut content = vec![0.0f32; tracks.len()];
        for (start, span, size) in spans {
            if span == 1 && start < content.len() {
                content[start] = content[start].max(size);
            }
        }
        tracks
            .iter()
            .zip(&content)
            .map(|(t, c)| match t.min {
                Breadth::Px(v) => v,
                Breadth::Percent(_) => 0.0,
                _ => *c,
            })
            .sum::<f32>()
            + tracks.len().saturating_sub(1) as f32 * gap
    };
    let w = axis(&columns_t, c.column_gap, spans_on(items, &p, &c.merges, c.column_gap, true));
    let h = axis(&rows_t, c.row_gap, spans_on(items, row_p, &c.merges, c.column_gap, false));
    (w, h)
}

/// Lay `items` out in `client`.
pub fn solve(c: &Container, items: &[Item], client: LRect) -> Solved {
    let mut columns_t = c.columns.expand(Some(client.w), c.column_gap);
    let p = place(items, columns_t.len(), &c.merges);
    let implicit = Track { min: Breadth::Auto, max: Breadth::Auto };
    columns_t.resize(p.columns.max(columns_t.len()), implicit);
    let mut rows_t = c.rows.expand(Some(client.h), c.row_gap);
    rows_t.resize(p.rows.max(rows_t.len()), implicit);

    let col_spans = spans_on(items, &p, &c.merges, c.column_gap, true);
    let row_spans = spans_on(items, &p, &c.merges, c.column_gap, false);
    let columns = size_tracks(&columns_t, Some(client.w), c.column_gap, &col_spans);
    let rows = size_tracks(&rows_t, Some(client.h), c.row_gap, &row_spans);

    let starts = |sizes: &[f32], gap: f32| -> Vec<f32> {
        let mut at = 0.0;
        sizes
            .iter()
            .map(|s| {
                let here = at;
                at += s + gap;
                here
            })
            .collect()
    };
    let (xs, ys) = (starts(&columns, c.column_gap), starts(&rows, c.row_gap));
    let area = |start: &[f32], sizes: &[f32], gap: f32, first: usize, span: usize| -> (f32, f32) {
        let last = (first + span).min(sizes.len());
        let len: f32 = sizes[first..last].iter().sum::<f32>() + (last - first).saturating_sub(1) as f32 * gap;
        (start[first], len)
    };
    let fit = |align: Align, pos: f32, len: f32, own: f32, lim: Limits| -> (f32, f32) {
        match align {
            Align::Stretch => (pos, clamp(len, lim)),
            Align::Start => (pos, own),
            Align::Center => (pos + (len - own) / 2.0, own),
            Align::End => (pos + len - own, own),
        }
    };
    // The items an area holds share it side by side, in the order given (R37): each an
    // equal share of the width, less the gaps between neighbours.
    let held: Vec<usize> = (0..c.merges.len()).map(|a| p.areas.iter().filter(|x| **x == Some(a)).count()).collect();
    let mut nth = vec![0usize; c.merges.len()];
    let rects = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let (row, col) = p.cells[i];
            let (row_span, column_span) = p.span(i, it, &c.merges);
            let (mut ax, mut aw) = area(&xs, &columns, c.column_gap, col, column_span);
            let (ay, ah) = area(&ys, &rows, c.row_gap, row, row_span);
            if let Some(a) = p.areas[i] {
                let (n, k) = (held[a] as f32, nth[a] as f32);
                nth[a] += 1;
                let share = ((aw - c.column_gap * (n - 1.0)) / n).max(0.0);
                ax += k * (share + c.column_gap);
                aw = share;
            }
            let (x, w) = fit(it.justify_self.unwrap_or(c.justify_items), ax, aw, clamp(it.size.0, it.width), it.width);
            let (y, h) = fit(it.align_self.unwrap_or(c.align_items), ay, ah, clamp(it.size.1, it.height), it.height);
            LRect::new(client.x + x, client.y + y, w, h)
        })
        .collect();

    let total = |sizes: &[f32], gap: f32| sizes.iter().sum::<f32>() + sizes.len().saturating_sub(1) as f32 * gap;
    let content_columns = size_tracks(&columns_t, None, c.column_gap, &col_spans);
    let content_rows = size_tracks(&rows_t, None, c.row_gap, &row_spans);
    let cells: Vec<Cell> = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let (row, column) = p.cells[i];
            let (row_span, column_span) = p.span(i, it, &c.merges);
            Cell { row, column, row_span, column_span }
        })
        .collect();
    Solved {
        rects,
        cells,
        content: (total(&content_columns, c.column_gap), total(&content_rows, c.row_gap)),
        columns,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(w: f32, h: f32) -> Item {
        Item {
            size: (w, h),
            column: 0,
            row: 0,
            column_span: 1,
            row_span: 1,
            justify_self: None,
            align_self: None,
            width: Limits::default(),
            height: Limits::default(),
        }
    }

    fn grid(columns: &str, rows: &str) -> Container {
        Container {
            columns: tracks::parse(columns),
            rows: tracks::parse(rows),
            row_gap: 0.0,
            column_gap: 0.0,
            justify_items: Align::Stretch,
            align_items: Align::Stretch,
            merges: Vec::new(),
        }
    }

    const CLIENT: LRect = LRect { x: 10.0, y: 20.0, w: 800.0, h: 400.0 };

    /// Spec 088 T2 — the solver says which cell each item is in, auto-placed
    /// items included, so an editor agrees with the engine about where a control
    /// is. Each cell is checked against the rectangle `solve` returns for it.
    #[test]
    fn solve_reports_the_cell_of_every_item() {
        let at = |col: usize, row: usize, cs: usize, rs: usize| Item { column: col, row, column_span: cs, row_span: rs, ..it(10.0, 10.0) };
        // 2 × 2 template; A explicit (2, 1), B auto, C spans two columns on row 2,
        // D is locked to row 4 — a row the template does not have.
        let items = vec![at(2, 1, 1, 1), at(0, 0, 1, 1), at(1, 2, 2, 1), at(0, 4, 1, 1)];
        let g = grid("100px 100px", "50px 50px");
        let s = solve(&g, &items, CLIENT);
        let cell = |row: usize, column: usize, row_span: usize, column_span: usize| Cell { row, column, row_span, column_span };
        assert_eq!(
            s.cells,
            vec![cell(0, 1, 1, 1), cell(0, 0, 1, 1), cell(1, 0, 1, 2), cell(3, 0, 1, 1)],
            "explicit, auto (first free cell), a two-column span, and an implicit row"
        );
        assert_eq!(s.rows.len(), 4, "row 4 exists only because D sits in it");
        // Every cell agrees with the rectangle: a Stretch item fills its cell area.
        let (mut checked, mut apart) = (0, 0);
        for (rect, c) in s.rects.iter().zip(&s.cells) {
            let x0: f32 = s.columns[..c.column].iter().sum();
            let y0: f32 = s.rows[..c.row].iter().sum();
            let w: f32 = s.columns[c.column..c.column + c.column_span].iter().sum();
            checked += 1;
            if (rect.x - (CLIENT.x + x0)).abs() > 0.01 || (rect.y - (CLIENT.y + y0)).abs() > 0.01 || (rect.w - w).abs() > 0.01 {
                apart += 1;
            }
        }
        assert_eq!(apart, 0);
        println!("grid cells: {checked} items, each cell matches the rectangle the engine placed it in");
    }

    /// AC25 — `200px 1fr 2fr` in 800: 200 fixed, 600 left for 3 fr → 200,
    /// 400. Three items auto-placed across: x 10, 210, 410.
    #[test]
    fn fixed_then_fractions() {
        let s = solve(&grid("200px 1fr 2fr", "100px"), &[it(10.0, 10.0), it(10.0, 10.0), it(10.0, 10.0)], CLIENT);
        assert_eq!(s.columns, vec![200.0, 200.0, 400.0]);
        assert_eq!((s.rects[1].x, s.rects[2].x, s.rects[2].w), (210.0, 410.0, 400.0));
    }

    /// AC25 — `Repeat(3, 1fr)` with a 10 px gap in 800: (800 − 20)/3 = 260.
    #[test]
    fn repeat_three_fractions_with_a_gap() {
        let mut g = grid("Repeat(3, 1fr)", "");
        g.column_gap = 10.0;
        let s = solve(&g, &[it(10.0, 10.0)], CLIENT);
        assert_eq!(s.columns, vec![260.0, 260.0, 260.0]);
    }

    /// AC25 — `Repeat(AutoFill, MinMax(160px, 1fr))` at 200, 500 and 820:
    /// 1, 3 and 5 columns, each sharing the width equally (200; 166.67;
    /// 164). Five items wrap to rows accordingly: at 500, item 4 starts row
    /// 2 at x = client.x.
    #[test]
    fn auto_fill_columns_at_three_widths() {
        let g = grid("Repeat(AutoFill, MinMax(160px, 1fr))", "");
        let items = vec![it(50.0, 30.0); 5];
        for (w, n, each) in [(200.0, 1, 200.0), (500.0, 3, 500.0 / 3.0), (820.0, 5, 164.0)] {
            let s = solve(&g, &items, LRect::new(0.0, 0.0, w, 400.0));
            assert_eq!(s.columns.len(), n, "{w}");
            assert!(s.columns.iter().all(|c| (c - each).abs() < 0.01), "{w}: {:?}", s.columns);
        }
        let s = solve(&g, &items, LRect::new(0.0, 0.0, 500.0, 400.0));
        assert_eq!((s.rects[3].x, s.rects[3].y), (0.0, s.rows[0]));
        println!("AutoFill grid: 200 → 1 col, 500 → 3 cols of 166.67, 820 → 5 cols of 164");
    }

    /// AC25 — an `Auto` track is as wide as the widest item that sits only
    /// in it: `Auto 1fr` with items 120 and 50 wide in column 1 → 120, and
    /// the fr takes 800 − 120 = 680. A spanning item does not size `Auto`.
    #[test]
    fn auto_tracks_take_their_content() {
        let mut items = vec![it(120.0, 10.0), it(10.0, 10.0), it(50.0, 10.0), it(900.0, 10.0)];
        items[3].column_span = 2;
        let s = solve(&grid("Auto 1fr", ""), &items, CLIENT);
        assert_eq!(s.columns, vec![120.0, 680.0]);
        // Without a flexible track the Auto columns share the leftover:
        // content 120 and 10, free 670 → +335 each.
        let s = solve(&grid("Auto Auto", ""), &items[..2], CLIENT);
        assert_eq!(s.columns, vec![455.0, 345.0]);
    }

    /// AC25 — spans cover their tracks and the gaps between them:
    /// `100px 100px 100px`, gap 10, an item spanning 2 → 210 wide; a row
    /// span of 2 over two 50 px rows with gap 10 → 110 high.
    #[test]
    fn spans_cover_tracks_and_gaps() {
        let mut g = grid("100px 100px 100px", "50px 50px");
        g.column_gap = 10.0;
        g.row_gap = 10.0;
        let mut a = it(10.0, 10.0);
        a.column_span = 2;
        a.row_span = 2;
        let s = solve(&g, &[a], CLIENT);
        assert_eq!((s.rects[0].w, s.rects[0].h), (210.0, 110.0));
    }

    /// AC25 — an explicit cell beyond the template extends the implicit
    /// grid: column 5 of a 3-column template adds columns 4 and 5 (Auto);
    /// the item is placed, never dropped. Row 3 of a 1-row template too.
    #[test]
    fn explicit_cells_beyond_the_template_extend_the_grid() {
        let mut a = it(60.0, 30.0);
        a.column = 5;
        a.row = 3;
        let s = solve(&grid("100px 100px 100px", "40px"), &[a], CLIENT);
        assert_eq!((s.columns.len(), s.rows.len()), (5, 3));
        assert_eq!(s.rects[0].x, 10.0 + 300.0 + s.columns[3]);
        assert!(s.rects[0].w > 0.0 && s.rects[0].h > 0.0);
    }

    /// AC25 — auto-placement skips an explicitly placed item and never
    /// back-fills. 3 columns; X is at row 1 column 2; A, B, C auto: A → (1,1),
    /// B skips X → (1,3), C → (2,1). Then a 2-wide D after a 1-wide E at
    /// (2,2): D does not fit the one column left in row 2 → row 3, and a later
    /// 1-wide F goes AFTER D, not back into the hole at (2,3).
    #[test]
    fn auto_placement_skips_taken_cells_and_never_back_fills() {
        let g = grid("100px 100px 100px", "20px 20px 20px 20px");
        let mut x = it(10.0, 10.0);
        x.column = 2;
        x.row = 1;
        let (a, b, cc, e) = (it(10.0, 10.0), it(10.0, 10.0), it(10.0, 10.0), it(10.0, 10.0));
        let mut d = it(10.0, 10.0);
        d.column_span = 2;
        let f = it(10.0, 10.0);
        let s = solve(&g, &[x, a, b, cc, e, d, f], LRect::new(0.0, 0.0, 300.0, 80.0));
        let cell = |i: usize| ((s.rects[i].y / 20.0) as i32 + 1, (s.rects[i].x / 100.0) as i32 + 1);
        assert_eq!([cell(1), cell(2), cell(3), cell(4), cell(5), cell(6)], [(1, 1), (1, 3), (2, 1), (2, 2), (3, 1), (3, 3)]);
    }

    /// `JustifyItems`/`JustifySelf` and `AlignItems` inside the cell:
    /// cell 200 × 100 at (10, 20); an item 50 × 30: Center → (85, 55).
    #[test]
    fn items_align_inside_their_cell() {
        let mut g = grid("200px", "100px");
        g.justify_items = Align::Center;
        g.align_items = Align::Center;
        let s = solve(&g, &[it(50.0, 30.0)], CLIENT);
        assert_eq!((s.rects[0].x, s.rects[0].y, s.rects[0].w, s.rects[0].h), (85.0, 55.0, 50.0, 30.0));
        let mut e = it(50.0, 30.0);
        e.justify_self = Some(Align::End);
        let s = solve(&g, &[e], CLIENT);
        assert_eq!(s.rects[0].x, 10.0 + 150.0);
    }

    // ── Spec 088 A4 — merged areas ──────────────────────────────────────────

    fn area(row: usize, column: usize, row_span: usize, column_span: usize) -> Area {
        Area { row, column, row_span, column_span }
    }

    fn placed(column: usize, row: usize) -> Item {
        Item { column, row, ..it(40.0, 20.0) }
    }

    /// R36 — `GridMerges` is read as spreadsheet ranges, either corner first, in any
    /// case and with any separator; a single cell, an overlap and nonsense are left
    /// out; and what is written reads back as itself.
    #[test]
    fn grid_merges_are_read_as_ranges_and_nonsense_is_left_out() {
        assert_eq!(parse_merges("R1C1:R1C2"), vec![area(0, 0, 1, 2)]);
        assert_eq!(parse_merges("r3c3:r2c1"), vec![area(1, 0, 2, 3)], "the other corner first, lower case");
        assert_eq!(parse_merges("R1C1:R2C1; R1C2:R1C3,R5C5:R6C6"), vec![area(0, 0, 2, 1), area(0, 1, 1, 2), area(4, 4, 2, 2)]);
        assert_eq!(parse_merges("R2C2:R2C2"), vec![], "one cell is no merge");
        assert_eq!(parse_merges("R1C1:R2C2 R2C2:R3C3"), vec![area(0, 0, 2, 2)], "an overlap with an earlier one is left out");
        for junk in ["", "   ", "R0C1:R1C2", "A1:B2", "R1C1", "R1:C2", "RxC1:R2C2"] {
            assert_eq!(parse_merges(junk), vec![], "{junk:?}");
        }
        let areas = vec![area(0, 0, 1, 2), area(2, 1, 3, 2)];
        assert_eq!(format_merges(&areas), "R1C1:R1C2 R3C2:R5C3");
        assert_eq!(parse_merges(&format_merges(&areas)), areas);
    }

    /// R37 — the controls written in an area share it side by side, each an equal share
    /// of its width less the gap between them, as tall as the area; no two overlap,
    /// and each reports the area as its cell.
    #[test]
    fn the_controls_of_an_area_lay_out_side_by_side_without_overlap() {
        let mut g = grid("100px 100px 100px", "50px 50px");
        g.merges = vec![area(0, 0, 1, 2)];
        let s = solve(&g, &[placed(1, 1), placed(2, 1)], CLIENT);
        let r = |i: usize| (s.rects[i].x, s.rects[i].y, s.rects[i].w, s.rects[i].h);
        assert_eq!((r(0), r(1)), ((10.0, 20.0, 100.0, 50.0), (110.0, 20.0, 100.0, 50.0)));
        assert_eq!(s.cells, vec![area(0, 0, 1, 2), area(0, 0, 1, 2)], "both are in the area");

        // Three in a 2 × 2 area with a 10 px gap: (210 − 20) / 3 each, 10 apart.
        g.column_gap = 10.0;
        g.row_gap = 10.0;
        g.merges = vec![area(0, 0, 2, 2)];
        let s = solve(&g, &[placed(1, 1), placed(2, 2), placed(1, 2)], CLIENT);
        let share = (210.0 - 20.0) / 3.0;
        for (k, rect) in s.rects.iter().enumerate() {
            assert!((rect.x - (10.0 + k as f32 * (share + 10.0))).abs() < 0.01, "item {k} at {rect:?}");
            assert!((rect.w - share).abs() < 0.01 && (rect.h - 110.0).abs() < 0.01, "item {k} is {rect:?}");
        }
        for pair in s.rects.windows(2) {
            assert!(pair[0].x + pair[0].w <= pair[1].x, "no overlap: {pair:?}");
        }
        println!("A4: two controls in a 1×2 area at x 10 and 110; three in a 2×2 area, {share:.2} wide, 10 apart");
    }

    /// One control in an area fills it, its own span ignored; its own alignment still
    /// holds inside it.
    #[test]
    fn one_control_fills_its_area_and_keeps_its_alignment() {
        let mut g = grid("100px 100px 100px", "50px 50px");
        g.merges = vec![area(0, 1, 2, 2)];
        let mut a = placed(3, 2);
        a.column_span = 3;
        let s = solve(&g, &[a], CLIENT);
        assert_eq!((s.rects[0].x, s.rects[0].y, s.rects[0].w, s.rects[0].h), (110.0, 20.0, 200.0, 100.0));
        a.justify_self = Some(Align::Center);
        let s = solve(&g, &[a], CLIENT);
        assert_eq!((s.rects[0].x, s.rects[0].w), (110.0 + 80.0, 40.0), "centred in the 200 of the area");
    }

    /// R38 — an empty area is one cell to automatic placement: the item that reaches
    /// its top-left cell takes it all, no item is placed in its other cells, and once
    /// it holds a control the cursor passes it by.
    #[test]
    fn an_empty_area_is_one_cell_to_automatic_placement() {
        let mut g = grid("100px 100px 100px", "50px 50px 50px");
        g.merges = vec![area(0, 1, 2, 2)];
        let auto = it(40.0, 20.0);
        let s = solve(&g, &[auto, auto, auto, auto], CLIENT);
        assert_eq!(
            s.cells,
            vec![area(0, 0, 1, 1), area(0, 1, 2, 2), area(1, 0, 1, 1), area(2, 0, 1, 1)],
            "A at R1C1, B takes the area, C and D never land in it"
        );
        assert_eq!((s.rects[1].x, s.rects[1].w, s.rects[1].h), (110.0, 200.0, 100.0));

        // Held by an explicit control: the automatic ones pass it by.
        let s = solve(&g, &[placed(3, 2), auto, auto], CLIENT);
        assert_eq!(s.cells, vec![area(0, 1, 2, 2), area(0, 0, 1, 1), area(1, 0, 1, 1)]);
        // An area beyond the template widens the implicit grid.
        g.merges = vec![area(3, 3, 1, 2)];
        let s = solve(&g, &[], CLIENT);
        assert_eq!((s.columns.len(), s.rows.len()), (5, 4));
    }

    /// The minimum size counts an area's controls side by side: two 60-wide controls
    /// in a one-column area of an `Auto` column need 60 + 10 + 60.
    #[test]
    fn the_minimum_size_counts_an_areas_controls_side_by_side() {
        let mut g = grid("Auto", "Auto Auto");
        g.column_gap = 10.0;
        g.merges = vec![area(0, 0, 2, 1)];
        let wide = |row| Item { column: 1, row, ..it(60.0, 20.0) };
        assert_eq!(min_size(&g, &[wide(1), wide(2)]).0, 130.0);
        g.merges.clear();
        assert_eq!(min_size(&g, &[wide(1), wide(2)]).0, 60.0, "without the area, one above the other");
    }
}
