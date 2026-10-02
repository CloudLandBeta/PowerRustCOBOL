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

/// What [`solve`] returns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Solved {
    /// One rectangle per item, in the order given.
    pub rects: Vec<LRect>,
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

/// Where each item sits: 0-based (row, column) of its first cell.
fn place(items: &[Item], explicit_columns: usize) -> (Vec<(usize, usize)>, usize, usize) {
    let n = items.len();
    let mut cell: Vec<Option<(usize, usize)>> = vec![None; n];
    let mut taken: HashSet<(usize, usize)> = HashSet::new();
    let fits = |taken: &HashSet<(usize, usize)>, r: usize, c: usize, it: &Item| {
        (r..r + it.row_span).all(|rr| (c..c + it.column_span).all(|cc| !taken.contains(&(rr, cc))))
    };
    let take = |taken: &mut HashSet<(usize, usize)>, r: usize, c: usize, it: &Item| {
        for rr in r..r + it.row_span {
            for cc in c..c + it.column_span {
                taken.insert((rr, cc));
            }
        }
    };

    // The implicit grid's width: the template, widened by explicit columns.
    let mut columns = explicit_columns.max(1);
    for it in items {
        if it.column > 0 {
            columns = columns.max(it.column - 1 + it.column_span);
        } else {
            columns = columns.max(it.column_span);
        }
    }

    // 1. Items with both lines given.
    for (i, it) in items.iter().enumerate() {
        if it.column > 0 && it.row > 0 {
            cell[i] = Some((it.row - 1, it.column - 1));
            take(&mut taken, it.row - 1, it.column - 1, it);
        }
    }
    // 2. Items locked to a row: the first free column, a cursor per row.
    let mut row_cursor: std::collections::HashMap<usize, usize> = Default::default();
    for (i, it) in items.iter().enumerate() {
        if it.row > 0 && it.column == 0 {
            let r = it.row - 1;
            let mut c = *row_cursor.get(&r).unwrap_or(&0);
            while !(c + it.column_span <= columns && fits(&taken, r, c, it)) {
                c += 1;
                if c + it.column_span > columns {
                    // Past the last column: the row grows the grid.
                    columns = c + it.column_span;
                }
            }
            cell[i] = Some((r, c));
            take(&mut taken, r, c, it);
            row_cursor.insert(r, c + it.column_span);
        }
    }
    // 3. Everything else, row-major from a cursor that never moves back.
    let (mut cr, mut cc) = (0usize, 0usize);
    for (i, it) in items.iter().enumerate() {
        if cell[i].is_some() {
            continue;
        }
        if it.column > 0 {
            let c = it.column - 1;
            if c < cc {
                cr += 1;
            }
            while !fits(&taken, cr, c, it) {
                cr += 1;
            }
            cell[i] = Some((cr, c));
            take(&mut taken, cr, c, it);
            cc = c + it.column_span;
            continue;
        }
        loop {
            if cc + it.column_span > columns {
                cr += 1;
                cc = 0;
                continue;
            }
            if fits(&taken, cr, cc, it) {
                break;
            }
            cc += 1;
        }
        cell[i] = Some((cr, cc));
        take(&mut taken, cr, cc, it);
        cc += it.column_span;
    }

    let cells: Vec<(usize, usize)> = cell.into_iter().map(|c| c.unwrap_or_default()).collect();
    let rows = items
        .iter()
        .zip(&cells)
        .map(|(it, (r, _))| r + it.row_span)
        .max()
        .unwrap_or(0);
    (cells, rows, columns)
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
    let (cells, row_count, column_count) = place(items, columns_t.len());
    let implicit = Track { min: Breadth::Auto, max: Breadth::Auto };
    columns_t.resize(column_count.max(columns_t.len()), implicit);
    // The rows the items form at the laid-out width.
    let (row_cells, row_count) = match width {
        Some(w) if c.columns.auto_fills() => {
            let (cells, rows, _) = place(items, c.columns.expand(Some(w), c.column_gap).len());
            (cells, rows)
        }
        _ => (cells.clone(), row_count),
    };
    let mut rows_t = c.rows.expand(None, c.row_gap);
    rows_t.resize(row_count.max(rows_t.len()), implicit);
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
    let w = axis(
        &columns_t,
        c.column_gap,
        items.iter().zip(&cells).map(|(it, (_, col))| (*col, it.column_span, clamp(it.size.0, it.width))).collect(),
    );
    let h = axis(
        &rows_t,
        c.row_gap,
        items.iter().zip(&row_cells).map(|(it, (row, _))| (*row, it.row_span, clamp(it.size.1, it.height))).collect(),
    );
    (w, h)
}

/// Lay `items` out in `client`.
pub fn solve(c: &Container, items: &[Item], client: LRect) -> Solved {
    let mut columns_t = c.columns.expand(Some(client.w), c.column_gap);
    let (cells, row_count, column_count) = place(items, columns_t.len());
    let implicit = Track { min: Breadth::Auto, max: Breadth::Auto };
    columns_t.resize(column_count.max(columns_t.len()), implicit);
    let mut rows_t = c.rows.expand(Some(client.h), c.row_gap);
    rows_t.resize(row_count.max(rows_t.len()), implicit);

    let col_spans: Vec<(usize, usize, f32)> = items
        .iter()
        .zip(&cells)
        .map(|(it, (_, col))| (*col, it.column_span, clamp(it.size.0, it.width)))
        .collect();
    let row_spans: Vec<(usize, usize, f32)> = items
        .iter()
        .zip(&cells)
        .map(|(it, (row, _))| (*row, it.row_span, clamp(it.size.1, it.height)))
        .collect();
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
    let rects = items
        .iter()
        .zip(&cells)
        .map(|(it, &(row, col))| {
            let (ax, aw) = area(&xs, &columns, c.column_gap, col, it.column_span);
            let (ay, ah) = area(&ys, &rows, c.row_gap, row, it.row_span);
            let (x, w) = fit(it.justify_self.unwrap_or(c.justify_items), ax, aw, clamp(it.size.0, it.width), it.width);
            let (y, h) = fit(it.align_self.unwrap_or(c.align_items), ay, ah, clamp(it.size.1, it.height), it.height);
            LRect::new(client.x + x, client.y + y, w, h)
        })
        .collect();

    let total = |sizes: &[f32], gap: f32| sizes.iter().sum::<f32>() + sizes.len().saturating_sub(1) as f32 * gap;
    let content_columns = size_tracks(&columns_t, None, c.column_gap, &col_spans);
    let content_rows = size_tracks(&rows_t, None, c.row_gap, &row_spans);
    Solved {
        rects,
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
        }
    }

    const CLIENT: LRect = LRect { x: 10.0, y: 20.0, w: 800.0, h: 400.0 };

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
}
