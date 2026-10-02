// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Flex and Flow** (spec 056 §4.12, R52–R54, R57).
//!
//! The CSS flexbox algorithm restricted to the properties R52/R53 name, for
//! items whose size is known before layout (their designed or measured size):
//! greedy line breaking, the grow/shrink loop that freezes items at their
//! limits (CSS Flexbox §9.7), `JustifyContent` on what remains after growth,
//! `AlignItems`/`AlignSelf` inside each line and `AlignContent` across lines.
//! Reverse directions mirror the finished placement.
//!
//! `Flow` (R57) is this solver with every item at grow 0 and shrink 0: items
//! keep their size, run in `FlowDirection`, wrap when `WrapContents`, and a
//! `FlowBreak` item ends its line.
//!
//! Pure: it sees item sizes and a client rectangle, never a control.

use crate::layout::defaults;
use crate::layout::limits::clamp;
use crate::layout::props::{self, Limits, PropSource};
use crate::layout::LRect;
use crate::model::Control;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justify {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// `AlignItems` / `AlignSelf` (and, for a grid, `JustifyItems`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Stretch,
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignContent {
    Stretch,
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

/// `FlexBasis`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Basis {
    /// The item's own size on the main axis.
    Auto,
    Px(f32),
    /// Of the container's main size.
    Percent(f32),
}

/// A flex (or flow) container.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Container {
    pub direction: Direction,
    pub wrap: Wrap,
    pub justify: Justify,
    pub align_items: Align,
    pub align_content: AlignContent,
    /// Between items on a line.
    pub main_gap: f32,
    /// Between lines.
    pub cross_gap: f32,
}

/// One item: its size before layout and its flex properties.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    /// Width and height before layout (designed, or measured for AutoSize).
    pub size: (f32, f32),
    pub grow: f32,
    pub shrink: f32,
    pub basis: Basis,
    /// `None` is `Auto`: the container's `AlignItems`.
    pub align_self: Option<Align>,
    pub order: i64,
    pub width: Limits,
    pub height: Limits,
    /// `FlowBreak`: the line ends after this item.
    pub line_break: bool,
}

/// What [`solve`] returns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Solved {
    /// One rectangle per item, in the order the items were given.
    pub rects: Vec<LRect>,
    /// What the items need, with no growth and no stretch: width and height.
    /// A container that sizes to its content on an axis takes this (R53).
    pub content: (f32, f32),
}

impl Direction {
    fn horizontal(self) -> bool {
        matches!(self, Direction::Row | Direction::RowReverse)
    }
    fn reversed(self) -> bool {
        matches!(self, Direction::RowReverse | Direction::ColumnReverse)
    }
}

// ── Reading the properties ──────────────────────────────────────────────────

fn word(src: &dyn PropSource, key: &str) -> String {
    src.text(key).trim().to_ascii_lowercase()
}

/// `RowGap` / `ColumnGap`, each falling back to `Gap` when empty (R52).
pub fn gaps(src: &dyn PropSource) -> (f32, f32) {
    let all = src.number("Gap");
    let axis = |k: &str| src.raw(k).and_then(props::number_of).unwrap_or(all);
    (axis("RowGap"), axis("ColumnGap"))
}

pub fn align_of(s: &str) -> Option<Align> {
    Some(match s.trim().to_ascii_lowercase().as_str() {
        "stretch" => Align::Stretch,
        "start" => Align::Start,
        "center" => Align::Center,
        "end" => Align::End,
        _ => return None,
    })
}

/// A `Flex` container's properties (R52).
pub fn flex_container(src: &dyn PropSource) -> Container {
    let direction = match word(src, "FlexDirection").as_str() {
        "column" => Direction::Column,
        "rowreverse" => Direction::RowReverse,
        "columnreverse" => Direction::ColumnReverse,
        _ => Direction::Row,
    };
    let wrap = match word(src, "FlexWrap").as_str() {
        "wrap" => Wrap::Wrap,
        "wrapreverse" => Wrap::WrapReverse,
        _ => Wrap::NoWrap,
    };
    let justify = match word(src, "JustifyContent").as_str() {
        "center" => Justify::Center,
        "end" => Justify::End,
        "spacebetween" => Justify::SpaceBetween,
        "spacearound" => Justify::SpaceAround,
        "spaceevenly" => Justify::SpaceEvenly,
        _ => Justify::Start,
    };
    let align_content = match word(src, "AlignContent").as_str() {
        "start" => AlignContent::Start,
        "center" => AlignContent::Center,
        "end" => AlignContent::End,
        "spacebetween" => AlignContent::SpaceBetween,
        "spacearound" => AlignContent::SpaceAround,
        _ => AlignContent::Stretch,
    };
    let (row_gap, column_gap) = gaps(src);
    let (main_gap, cross_gap) = if direction.horizontal() { (column_gap, row_gap) } else { (row_gap, column_gap) };
    Container {
        direction,
        wrap,
        justify,
        align_items: align_of(&src.text("AlignItems")).unwrap_or(Align::Stretch),
        align_content,
        main_gap,
        cross_gap,
    }
}

/// A `Flow` container (R57): a flex container whose items keep their size.
pub fn flow_container(src: &dyn PropSource) -> Container {
    let direction = match word(src, "FlowDirection").as_str() {
        "topdown" => Direction::Column,
        "righttoleft" => Direction::RowReverse,
        "bottomup" => Direction::ColumnReverse,
        _ => Direction::Row,
    };
    let (row_gap, column_gap) = gaps(src);
    let (main_gap, cross_gap) = if direction.horizontal() { (column_gap, row_gap) } else { (row_gap, column_gap) };
    Container {
        direction,
        wrap: if src.flag("WrapContents") { Wrap::Wrap } else { Wrap::NoWrap },
        justify: Justify::Start,
        align_items: Align::Start,
        align_content: AlignContent::Start,
        main_gap,
        cross_gap,
    }
}

/// `c` as a flex item of intrinsic `size` (R53).
pub fn flex_item(c: &Control, size: (f32, f32)) -> Item {
    let basis = {
        let t = c.text("FlexBasis");
        let t = t.trim();
        if let Some(p) = t.strip_suffix('%') {
            p.trim().parse::<f32>().map(Basis::Percent).unwrap_or(Basis::Auto)
        } else {
            let px = t.strip_suffix("px").unwrap_or(t).trim();
            px.parse::<f32>().ok().filter(|v| *v >= 0.0).map(Basis::Px).unwrap_or(Basis::Auto)
        }
    };
    Item {
        size,
        grow: c.number("FlexGrow").max(0.0),
        shrink: c.number("FlexShrink").max(0.0),
        basis,
        align_self: align_of(&c.text("AlignSelf")),
        order: c.number("Order") as i64,
        width: props::width_limits(c),
        height: props::height_limits(c),
        line_break: false,
    }
}

/// `c` as a flow item (R57): its own size, no growth, no shrink.
pub fn flow_item(c: &Control, size: (f32, f32)) -> Item {
    Item {
        size,
        grow: 0.0,
        shrink: 0.0,
        basis: Basis::Auto,
        align_self: None,
        order: c.number("Order") as i64,
        width: props::width_limits(c),
        height: props::height_limits(c),
        line_break: c.flag("FlowBreak"),
    }
}

// ── The algorithm ───────────────────────────────────────────────────────────

/// Lay `items` out in `client`.
pub fn solve(c: &Container, items: &[Item], client: LRect) -> Solved {
    let n = items.len();
    let h = c.direction.horizontal();
    let (main_size, cross_size) = if h { (client.w, client.h) } else { (client.h, client.w) };
    let main_of = |it: &Item| if h { it.size.0 } else { it.size.1 };
    let cross_of = |it: &Item| if h { it.size.1 } else { it.size.0 };
    let main_lim = |it: &Item| if h { it.width } else { it.height };
    let cross_lim = |it: &Item| if h { it.height } else { it.width };

    // `Order`, then the order given (the caller's reading order — R54).
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| items[i].order);

    let base: Vec<f32> = items
        .iter()
        .map(|it| match it.basis {
            Basis::Auto => main_of(it),
            Basis::Px(v) => v,
            Basis::Percent(p) => p / defaults::PERCENT * main_size,
        })
        .collect();
    let hyp: Vec<f32> = items.iter().zip(&base).map(|(it, b)| clamp(*b, main_lim(it))).collect();
    let hyp_cross: Vec<f32> = items.iter().map(|it| clamp(cross_of(it), cross_lim(it))).collect();

    let lines = break_lines(c, items, &order, &hyp, main_size);

    // What the items need, before any growth or stretch.
    let line_need = |l: &[usize]| l.iter().map(|&i| hyp[i]).sum::<f32>() + gaps_between(l.len(), c.main_gap);
    let content_main = lines.iter().map(|l| line_need(l)).fold(0.0f32, f32::max);
    let content_lines: Vec<f32> = lines
        .iter()
        .map(|l| l.iter().map(|&i| hyp_cross[i]).fold(0.0f32, f32::max))
        .collect();
    let content_cross = content_lines.iter().sum::<f32>() + gaps_between(lines.len(), c.cross_gap);

    // Main sizes, line by line (CSS §9.7).
    let mut main_len = hyp.clone();
    for l in &lines {
        for (i, v) in l.iter().zip(resolve_line(l, items, &base, &hyp, main_size, c.main_gap, &main_lim)) {
            main_len[*i] = v;
        }
    }

    // Cross size of each line: a single-line container's line is the
    // container; otherwise each line is as tall as its tallest item, and
    // `AlignContent` shares what is left.
    let mut line_cross = content_lines.clone();
    let mut line_pos = vec![0.0f32; lines.len()];
    if c.wrap == Wrap::NoWrap {
        line_cross = vec![cross_size; lines.len()];
    } else {
        let free = cross_size - content_cross;
        let k = lines.len() as f32;
        let (mut lead, mut between) = (0.0, c.cross_gap);
        match c.align_content {
            AlignContent::Stretch if free > 0.0 => line_cross.iter_mut().for_each(|v| *v += free / k),
            AlignContent::Center => lead = free / 2.0,
            AlignContent::End => lead = free,
            AlignContent::SpaceBetween if free > 0.0 && lines.len() > 1 => {
                between += free / (k - 1.0)
            }
            AlignContent::SpaceAround if free > 0.0 => {
                lead = free / k / 2.0;
                between += free / k;
            }
            AlignContent::SpaceAround => lead = free / 2.0,
            _ => {}
        }
        let mut at = lead;
        for (p, lc) in line_pos.iter_mut().zip(&line_cross) {
            *p = at;
            at += lc + between;
        }
    }

    let mut rects = vec![LRect::default(); n];
    for (li, l) in lines.iter().enumerate() {
        // Main axis: `JustifyContent` on what remains after growth.
        let k = l.len() as f32;
        let free = main_size - l.iter().map(|&i| main_len[i]).sum::<f32>() - gaps_between(l.len(), c.main_gap);
        let (lead, between) = match c.justify {
            Justify::Start => (0.0, c.main_gap),
            Justify::Center => (free / 2.0, c.main_gap),
            Justify::End => (free, c.main_gap),
            Justify::SpaceBetween if free > 0.0 && l.len() > 1 => (0.0, c.main_gap + free / (k - 1.0)),
            Justify::SpaceBetween => (0.0, c.main_gap),
            Justify::SpaceAround if free > 0.0 => (free / k / 2.0, c.main_gap + free / k),
            Justify::SpaceEvenly if free > 0.0 => (free / (k + 1.0), c.main_gap + free / (k + 1.0)),
            Justify::SpaceAround | Justify::SpaceEvenly => (free / 2.0, c.main_gap),
        };
        let mut at = lead;
        for &i in l {
            let it = &items[i];
            // Cross axis: the item's own alignment inside its line.
            let lc = line_cross[li];
            let (cpos, clen) = match it.align_self.unwrap_or(c.align_items) {
                Align::Stretch => (0.0, clamp(lc, cross_lim(it))),
                Align::Start => (0.0, hyp_cross[i]),
                Align::Center => ((lc - hyp_cross[i]) / 2.0, hyp_cross[i]),
                Align::End => (lc - hyp_cross[i], hyp_cross[i]),
            };
            let mut mpos = at;
            if c.direction.reversed() {
                mpos = main_size - at - main_len[i];
            }
            let mut cpos = line_pos[li] + cpos;
            if c.wrap == Wrap::WrapReverse {
                cpos = cross_size - cpos - clen;
            }
            rects[i] = if h {
                LRect::new(client.x + mpos, client.y + cpos, main_len[i], clen)
            } else {
                LRect::new(client.x + cpos, client.y + mpos, clen, main_len[i])
            };
            at += main_len[i] + between;
        }
    }

    Solved {
        rects,
        content: if h { (content_main, content_cross) } else { (content_cross, content_main) },
    }
}

/// The items' main sizes before growth or shrink: the basis, clamped to the
/// item's limits on the main axis.
fn hypothetical_main(c: &Container, items: &[Item], main_size: f32) -> Vec<f32> {
    let h = c.direction.horizontal();
    items
        .iter()
        .map(|it| {
            let (own, lim) = if h { (it.size.0, it.width) } else { (it.size.1, it.height) };
            let b = match it.basis {
                Basis::Auto => own,
                Basis::Px(v) => v,
                Basis::Percent(p) => p / defaults::PERCENT * main_size,
            };
            clamp(b, lim)
        })
        .collect()
}

/// Lines, broken greedily when wrapping, of the items in `order` (indices
/// into `items`) with main sizes `hyp`; a `FlowBreak` always ends one.
fn break_lines(c: &Container, items: &[Item], order: &[usize], hyp: &[f32], main_size: f32) -> Vec<Vec<usize>> {
    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut line: Vec<usize> = Vec::new();
    let mut used = 0.0f32;
    for &i in order {
        let add = hyp[i] + if line.is_empty() { 0.0 } else { c.main_gap };
        if c.wrap != Wrap::NoWrap && !line.is_empty() && used + add > main_size + defaults::EPSILON {
            lines.push(std::mem::take(&mut line));
            used = 0.0;
        }
        used += hyp[i] + if line.is_empty() { 0.0 } else { c.main_gap };
        line.push(i);
        if items[i].line_break {
            lines.push(std::mem::take(&mut line));
            used = 0.0;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// The smallest client a flex or flow container can lay `items` out in
/// (R18), width and height. `mins` is each item's own minimum (its
/// `MinWidth`/`MinHeight`, or what a nested container needs). On the main
/// axis an item that may shrink needs only its minimum, one that may not its
/// size; across, a stretched item needs its minimum, any other its size. At
/// the smallest width a wrapping container has one item per line.
pub fn min_size(c: &Container, items: &[Item], mins: &[(f32, f32)]) -> (f32, f32) {
    min_size_at(c, items, mins, None)
}

/// [`min_size`], and when `main` is the main size a wrapping container is
/// laid out at, its cross size is what the lines it forms THERE need — the
/// lines [`solve`] breaks, in `items`' order — rather than one item per line
/// (spec 056 R18: the minimum is computed at the window's minimum on the
/// other axis). `items` must be in reading order, as [`solve`] is given them.
pub fn min_size_at(c: &Container, items: &[Item], mins: &[(f32, f32)], main: Option<f32>) -> (f32, f32) {
    let h = c.direction.horizontal();
    let pick = |v: (f32, f32)| if h { v } else { (v.1, v.0) };
    let per: Vec<(f32, f32)> = items
        .iter()
        .zip(mins)
        .map(|(it, m)| {
            let (size, m) = (pick(it.size), pick(*m));
            let main = if it.shrink > 0.0 { m.0 } else { size.0.max(m.0) };
            let stretched = it.align_self.unwrap_or(c.align_items) == Align::Stretch;
            let cross = if stretched { m.1 } else { size.1.max(m.1) };
            (main, cross)
        })
        .collect();
    let laid_main = main;
    let (main, cross) = if c.wrap == Wrap::NoWrap {
        (
            per.iter().map(|p| p.0).sum::<f32>() + gaps_between(per.len(), c.main_gap),
            per.iter().map(|p| p.1).fold(0.0f32, f32::max),
        )
    } else if let Some(at) = laid_main {
        let mut order: Vec<usize> = (0..items.len()).collect();
        order.sort_by_key(|&i| items[i].order);
        let lines = break_lines(c, items, &order, &hypothetical_main(c, items, at), at);
        (
            per.iter().map(|p| p.0).fold(0.0f32, f32::max),
            lines.iter().map(|l| l.iter().map(|&i| per[i].1).fold(0.0f32, f32::max)).sum::<f32>()
                + gaps_between(lines.len(), c.cross_gap),
        )
    } else {
        (
            per.iter().map(|p| p.0).fold(0.0f32, f32::max),
            per.iter().map(|p| p.1).sum::<f32>() + gaps_between(per.len(), c.cross_gap),
        )
    };
    if h { (main, cross) } else { (cross, main) }
}

fn gaps_between(count: usize, gap: f32) -> f32 {
    count.saturating_sub(1) as f32 * gap
}

/// CSS Flexbox §9.7 "Resolving Flexible Lengths" for one line: the main size
/// of each of its items, in the line's order.
fn resolve_line(
    line: &[usize],
    items: &[Item],
    base: &[f32],
    hyp: &[f32],
    main_size: f32,
    gap: f32,
    main_lim: &dyn Fn(&Item) -> Limits,
) -> Vec<f32> {
    let gaps = gaps_between(line.len(), gap);
    let growing = line.iter().map(|&i| hyp[i]).sum::<f32>() + gaps < main_size;
    let factor = |i: usize| if growing { items[i].grow } else { items[i].shrink };
    let mut target: Vec<f32> = line.iter().map(|&i| hyp[i]).collect();
    let mut frozen: Vec<bool> = line
        .iter()
        .map(|&i| factor(i) == 0.0 || (growing && base[i] > hyp[i]) || (!growing && base[i] < hyp[i]))
        .collect();
    let space = |target: &[f32], frozen: &[bool]| {
        main_size
            - gaps
            - line
                .iter()
                .enumerate()
                .map(|(k, &i)| if frozen[k] { target[k] } else { base[i] })
                .sum::<f32>()
    };
    let initial_free = space(&target, &frozen);
    for _ in 0..=line.len() {
        if frozen.iter().all(|f| *f) {
            break;
        }
        let mut free = space(&target, &frozen);
        let open = || line.iter().enumerate().filter(|(k, _)| !frozen[*k]);
        let sum_factors: f32 = open().map(|(_, &i)| factor(i)).sum();
        if sum_factors < 1.0 {
            let scaled = initial_free * sum_factors;
            if scaled.abs() < free.abs() {
                free = scaled;
            }
        }
        let sum_scaled: f32 = open().map(|(_, &i)| items[i].shrink * base[i]).sum();
        let open_now: Vec<(usize, usize)> = open().map(|(k, &i)| (k, i)).collect();
        for &(k, i) in &open_now {
            target[k] = if growing {
                base[i] + free * items[i].grow / sum_factors
            } else if sum_scaled > 0.0 {
                base[i] - free.abs() * items[i].shrink * base[i] / sum_scaled
            } else {
                base[i]
            };
        }
        // Clamp, then freeze the side that was violated (all, if neither).
        let mut total = 0.0f32;
        let mut violation = vec![0.0f32; line.len()];
        for &(k, i) in &open_now {
            let clamped = clamp(target[k], main_lim(&items[i]));
            violation[k] = clamped - target[k];
            total += violation[k];
            target[k] = clamped;
        }
        for &(k, _) in &open_now {
            let v = violation[k];
            if total.abs() < defaults::EPSILON || (total > 0.0 && v > 0.0) || (total < 0.0 && v < 0.0) {
                frozen[k] = true;
            }
        }
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(w: f32, h: f32) -> Item {
        Item {
            size: (w, h),
            grow: 0.0,
            shrink: 1.0,
            basis: Basis::Auto,
            align_self: None,
            order: 0,
            width: Limits::default(),
            height: Limits::default(),
            line_break: false,
        }
    }

    fn row() -> Container {
        Container {
            direction: Direction::Row,
            wrap: Wrap::NoWrap,
            justify: Justify::Start,
            align_items: Align::Start,
            align_content: AlignContent::Stretch,
            main_gap: 0.0,
            cross_gap: 0.0,
        }
    }

    fn at(s: &Solved, i: usize) -> (f32, f32, f32, f32) {
        let r = s.rects[i];
        (r.x, r.y, r.w, r.h)
    }

    const CLIENT: LRect = LRect { x: 10.0, y: 20.0, w: 300.0, h: 100.0 };

    /// AC24 — every `JustifyContent` on a Row, at a larger container.
    /// Items 50 and 70 wide (120 total) in 300 with gap 10: free = 300 −
    /// 120 − 10 = 170.
    ///   Start:        x 10, 70
    ///   Center:       lead 85 → 95, 155
    ///   End:          lead 170 → 180, 240
    ///   SpaceBetween: 10, then 10+50+10+170 = 240
    ///   SpaceAround:  170/2 = 85 per item → lead 42.5 → 52.5; next 52.5+50+10+85 = 197.5
    ///   SpaceEvenly:  170/3 ≈ 56.67 → 66.67; next 66.67+50+10+56.67 = 183.33
    #[test]
    fn justify_content_on_a_row() {
        let items = [item(50.0, 20.0), item(70.0, 30.0)];
        let cases = [
            (Justify::Start, 10.0, 70.0),
            (Justify::Center, 95.0, 155.0),
            (Justify::End, 180.0, 240.0),
            (Justify::SpaceBetween, 10.0, 240.0),
            (Justify::SpaceAround, 52.5, 197.5),
            (Justify::SpaceEvenly, 10.0 + 170.0 / 3.0, 10.0 + 170.0 / 3.0 * 2.0 + 60.0),
        ];
        for (j, a, b) in cases {
            let c = Container { justify: j, main_gap: 10.0, ..row() };
            let s = solve(&c, &items, CLIENT);
            assert!((s.rects[0].x - a).abs() < 0.01 && (s.rects[1].x - b).abs() < 0.01, "{j:?}: {:?}", s.rects);
        }
        println!("flex JustifyContent × 6 on a 300 px row: all hand-derived positions hold");
    }

    /// AC24 — `AlignItems` in a single line: the line is the container
    /// (100 high). Stretch → 100; Start → y 20; Center → 20 + (100−30)/2 =
    /// 55; End → 20 + 70 = 90. `AlignSelf` overrides for one item.
    #[test]
    fn align_items_and_align_self() {
        let mut items = [item(50.0, 30.0), item(50.0, 30.0)];
        for (a, y, hh) in [(Align::Stretch, 20.0, 100.0), (Align::Start, 20.0, 30.0), (Align::Center, 55.0, 30.0), (Align::End, 90.0, 30.0)] {
            let s = solve(&Container { align_items: a, ..row() }, &items, CLIENT);
            assert_eq!((s.rects[0].y, s.rects[0].h), (y, hh), "{a:?}");
        }
        items[1].align_self = Some(Align::End);
        let s = solve(&Container { align_items: Align::Start, ..row() }, &items, CLIENT);
        assert_eq!((s.rects[0].y, s.rects[1].y), (20.0, 90.0));
    }

    /// AC24 — grow 1:2. Items 50 + 50 in 300, free 200: +66.67 and +133.33
    /// → 116.67 and 183.33. With `MaxWidth` 100 on the first it freezes at
    /// 100 and the second takes the rest: 300 − 100 = 200.
    #[test]
    fn grow_ratios_and_a_max_limit() {
        let mut items = [item(50.0, 10.0), item(50.0, 10.0)];
        items[0].grow = 1.0;
        items[1].grow = 2.0;
        let s = solve(&row(), &items, CLIENT);
        assert!((s.rects[0].w - 116.667).abs() < 0.01 && (s.rects[1].w - 183.333).abs() < 0.01, "{:?}", s.rects);
        items[0].width.max = 100.0;
        let s = solve(&row(), &items, CLIENT);
        assert_eq!((s.rects[0].w, s.rects[1].w), (100.0, 200.0));
        println!("flex grow 1:2 → {:.2} / {:.2}; with MaxWidth 100 → 100 / 200", 116.667, 183.333);
    }

    /// AC24 — shrink weighted by basis. Items 200 and 400 (shrink 1) in
    /// 300: overflow 300; scaled 200 : 400 → shrink 100 and 200 → 100, 200.
    /// With `MinWidth` 150 on the first it freezes at 150 and the second
    /// takes 150.
    #[test]
    fn shrink_is_weighted_by_basis_and_respects_min() {
        let mut items = [item(200.0, 10.0), item(400.0, 10.0)];
        let s = solve(&row(), &items, CLIENT);
        assert_eq!((s.rects[0].w, s.rects[1].w), (100.0, 200.0));
        items[0].width.min = 150.0;
        let s = solve(&row(), &items, CLIENT);
        assert_eq!((s.rects[0].w, s.rects[1].w), (150.0, 150.0));
    }

    /// AC24 — a smaller container with the default shrink 0 keeps sizes and
    /// overflows; `FlexBasis` 25% of 300 = 75 is the start size.
    #[test]
    fn no_shrink_overflows_and_percent_basis() {
        let mut items = [item(200.0, 10.0), item(200.0, 10.0)];
        items[0].shrink = 0.0;
        items[1].shrink = 0.0;
        let s = solve(&row(), &items, CLIENT);
        assert_eq!((s.rects[0].w, s.rects[1].x), (200.0, 210.0));
        let mut p = item(10.0, 10.0);
        p.basis = Basis::Percent(25.0);
        let s = solve(&row(), &[p], CLIENT);
        assert_eq!(s.rects[0].w, 75.0);
    }

    /// AC24 — wrap into two lines with `AlignContent`. Three items of 120 ×
    /// 20 in 300 (gap 0): two fit (240), the third wraps. Lines are 20
    /// high; free cross = 100 − 40 = 60.
    ///   Stretch: each line 50 → second line at y 20 + 50 = 70
    ///   Center:  lead 30 → lines at 50 and 70
    ///   End:     lead 60 → 80 and 100
    ///   SpaceBetween: 20 and 20 + 20 + 60 = 100
    ///   SpaceAround: 60/2 = 30 per line → 35 and 35 + 20 + 30 = 85
    /// `WrapReverse` puts the first line at the bottom: 20 + 100 − 20 = 100.
    #[test]
    fn wrapping_lines_and_align_content() {
        let items = [item(120.0, 20.0), item(120.0, 20.0), item(120.0, 20.0)];
        for (ac, y1, y3) in [
            (AlignContent::Stretch, 20.0, 70.0),
            (AlignContent::Center, 50.0, 70.0),
            (AlignContent::End, 80.0, 100.0),
            (AlignContent::SpaceBetween, 20.0, 100.0),
            (AlignContent::SpaceAround, 35.0, 85.0),
        ] {
            let c = Container { wrap: Wrap::Wrap, align_content: ac, ..row() };
            let s = solve(&c, &items, CLIENT);
            assert_eq!((s.rects[0].y, s.rects[2].y), (y1, y3), "{ac:?}: {:?}", s.rects);
            assert_eq!(s.rects[2].x, 10.0, "the third item starts the second line");
        }
        let c = Container { wrap: Wrap::WrapReverse, align_content: AlignContent::Start, ..row() };
        let s = solve(&c, &items, CLIENT);
        assert_eq!((s.rects[0].y, s.rects[2].y), (100.0, 80.0));
        assert_eq!(s.content, (240.0, 40.0), "what the items need: the widest line, two lines high");
    }

    /// AC24 — `Order` and the reverse directions. Items A 50, B 60, C 70
    /// with C `Order` −1: C, A, B → x 10, 80, 130. `RowReverse` mirrors from
    /// the right: C ends at 310 → x 240; A at 190; B at 130. A column runs
    /// down: y 20, 20+h…; `ColumnReverse` from the bottom.
    #[test]
    fn order_and_reverse_directions() {
        let mut items = [item(50.0, 10.0), item(60.0, 20.0), item(70.0, 30.0)];
        items[2].order = -1;
        let s = solve(&row(), &items, CLIENT);
        assert_eq!((s.rects[2].x, s.rects[0].x, s.rects[1].x), (10.0, 80.0, 130.0));
        let s = solve(&Container { direction: Direction::RowReverse, ..row() }, &items, CLIENT);
        assert_eq!((s.rects[2].x, s.rects[0].x, s.rects[1].x), (240.0, 190.0, 130.0));
        let col = Container { direction: Direction::Column, ..row() };
        let s = solve(&col, &items, CLIENT);
        assert_eq!((s.rects[2].y, s.rects[0].y, s.rects[1].y), (20.0, 50.0, 60.0));
        let s = solve(&Container { direction: Direction::ColumnReverse, ..col }, &items, CLIENT);
        // Bottom 120: C 30 high → 90; A 10 → 80; B 20 → 60.
        assert_eq!((s.rects[2].y, s.rects[0].y, s.rects[1].y), (90.0, 80.0, 60.0));
    }

    /// AC24 — a content-sized column: three items 20, 30, 40 high with gap
    /// 5 need 20+30+40+10 = 100, as wide as the widest (80).
    #[test]
    fn a_column_reports_the_size_of_its_content() {
        let items = [item(50.0, 20.0), item(80.0, 30.0), item(60.0, 40.0)];
        let c = Container { direction: Direction::Column, main_gap: 5.0, ..row() };
        let s = solve(&c, &items, LRect::new(0.0, 0.0, 300.0, 1000.0));
        assert_eq!(s.content, (80.0, 100.0));
    }

    /// AC26 — Flow: items keep their size and wrap at the edge in each
    /// direction; `FlowBreak` ends a line; `WrapContents = false` keeps one
    /// line. Items 100, 150, 80 wide × 20 in 300, gap 10: 100 + 10 + 150 =
    /// 260 fits, + 10 + 80 = 350 does not → the third wraps.
    #[test]
    fn flow_wraps_breaks_and_can_stay_on_one_line() {
        let mut items = [item(100.0, 20.0), item(150.0, 20.0), item(80.0, 20.0)];
        for it in items.iter_mut() {
            it.shrink = 0.0;
        }
        let flow = |d: Direction, w: Wrap| Container {
            direction: d,
            wrap: w,
            justify: Justify::Start,
            align_items: Align::Start,
            align_content: AlignContent::Start,
            main_gap: 10.0,
            cross_gap: 10.0,
        };
        let s = solve(&flow(Direction::Row, Wrap::Wrap), &items, CLIENT);
        assert_eq!((at(&s, 1).0, at(&s, 2).0, at(&s, 2).1), (120.0, 10.0, 50.0));
        // RightToLeft: first item ends at the right edge (310) → 210.
        let s = solve(&flow(Direction::RowReverse, Wrap::Wrap), &items, CLIENT);
        assert_eq!((at(&s, 0).0, at(&s, 1).0, at(&s, 2).0), (210.0, 50.0, 230.0));
        // TopDown in 100 high: 20 + 10 + 20 + 10 + 20 = 80 fits → one column.
        let s = solve(&flow(Direction::Column, Wrap::Wrap), &items, CLIENT);
        assert_eq!((at(&s, 2).0, at(&s, 2).1), (10.0, 80.0));
        // BottomUp: from the bottom (120): 100, 70, 40.
        let s = solve(&flow(Direction::ColumnReverse, Wrap::Wrap), &items, CLIENT);
        assert_eq!((at(&s, 0).1, at(&s, 1).1, at(&s, 2).1), (100.0, 70.0, 40.0));
        // FlowBreak after the first item.
        items[0].line_break = true;
        let s = solve(&flow(Direction::Row, Wrap::Wrap), &items, CLIENT);
        assert_eq!((at(&s, 1).0, at(&s, 1).1), (10.0, 50.0));
        items[0].line_break = false;
        let s = solve(&flow(Direction::Row, Wrap::NoWrap), &items, CLIENT);
        assert_eq!((at(&s, 2).0, at(&s, 2).1), (280.0, 20.0), "one line, overflowing");
        println!("flow: wraps at 300 px in all four directions; FlowBreak forces a line; WrapContents off keeps one");
    }
}
