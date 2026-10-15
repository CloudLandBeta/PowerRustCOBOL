// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 088 — editing a **Grid container's layout** on the Form Designer's canvas
//! (not to be confused with `grid_edit`, spec 094, which edits a *DataGrid* control).
//!
//! The edits themselves are pure functions in `cobolt_forms::layout::grid_edit`: a
//! container as the layout engine laid it out goes in, property writes come out.
//! This module is what the canvas adds — the view state, the geometry a pointer is
//! hit-tested against, the painting, and the floating toolbar — and `designer.rs`
//! hosts it.

use cobolt_forms::layout::grid_edit::{Axis, CellRange, GridView, Refusal, RemovedTrack};
use cobolt_forms::layout::{LRect, LayoutMode, LayoutOutput};

/// How close to a track line the pointer must be to grab it, in canvas points (R18).
pub(crate) const LINE_GRAB: f32 = 4.0;

/// What the designer remembers about grid editing. View state: never saved, never
/// an undo step of its own (R5).
#[derive(Clone, Debug)]
pub(crate) struct LayoutGridView {
    /// The **Show Grid** toggle (R1–R5): on, the canvas edits the layout grids; off,
    /// it edits the controls. Starts on, and is not saved.
    pub show: bool,
    /// The selected cell or range of cells (R7).
    pub selection: Option<CellSel>,
    /// Every row or column removed in this session, oldest first — what Restore
    /// puts back, most recent first (R13).
    pub history: Vec<RemovedTrack>,
    /// Where the floating toolbar was drawn last frame, on screen — a press there is the
    /// toolbar's, not the grid's.
    pub toolbar: Option<egui::Rect>,
    /// What the last removal left auto-placed, shown beside the toolbar (R12).
    pub notice: Option<Notice>,
}

/// The controls a removal left auto-placed, and when the notice was raised.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Notice {
    pub controls: Vec<String>,
    pub at: f64,
}

/// How long a notice stays up, in seconds.
pub(crate) const NOTICE_SECONDS: f64 = 8.0;

impl Default for LayoutGridView {
    fn default() -> Self {
        LayoutGridView { show: true, selection: None, history: Vec::new(), toolbar: None, notice: None }
    }
}

/// A selection of cells in one Grid container: where it was pressed and where it
/// was dragged (or Shift-clicked) to, 0-based `(row, column)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CellSel {
    pub container: String,
    pub anchor: (usize, usize),
    pub head: (usize, usize),
}

impl CellSel {
    pub fn single(container: &str, cell: (usize, usize)) -> CellSel {
        CellSel { container: container.to_owned(), anchor: cell, head: cell }
    }

    pub fn range(&self) -> CellRange {
        CellRange::between(self.anchor, self.head)
    }
}

impl CellSel {
    /// The same selection with every cell brought inside a grid that has `rows` rows and
    /// `columns` columns — a track may have been removed since it was made.
    pub fn clamped(&self, rows: usize, columns: usize) -> CellSel {
        let fit = |(r, c): (usize, usize)| (r.min(rows.saturating_sub(1)), c.min(columns.saturating_sub(1)));
        CellSel { container: self.container.clone(), anchor: fit(self.anchor), head: fit(self.head) }
    }
}

/// The buttons of the floating toolbar, in the order they are drawn (R8, R9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    AddColumn,
    RemoveColumn,
    AddRow,
    RemoveRow,
    Restore,
    Merge,
    Split,
}

pub(crate) const TOOLS: [Tool; 7] =
    [Tool::AddColumn, Tool::RemoveColumn, Tool::AddRow, Tool::RemoveRow, Tool::Restore, Tool::Merge, Tool::Split];

impl Tool {
    /// What the button does, in English — the key of its translated tip.
    pub fn tip(self) -> &'static str {
        match self {
            Tool::AddColumn => "Add a column after the selected cell",
            Tool::RemoveColumn => "Remove the selected cell's column",
            Tool::AddRow => "Add a row after the selected cell",
            Tool::RemoveRow => "Remove the selected cell's row",
            Tool::Restore => "Restore the last removed row or column",
            Tool::Merge => "Merge the selected cells into one",
            Tool::Split => "Split the merged cell back into single cells",
        }
    }
}

/// Why a button is disabled (R9): each has its own translated sentence, shown as the
/// button's tip in place of what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Why {
    NoCell,
    SeveralTracks,
    Repeat,
    LastTrack,
    NothingToRestore,
    OneCell,
    NoControl,
    SeveralControls,
    NothingToSplit,
    Scaled,
}

impl Why {
    pub fn text(self) -> &'static str {
        match self {
            Why::NoCell => "Select a cell first",
            Why::SeveralTracks => "Select a single cell: this removes one row or column",
            Why::Repeat => "These rows or columns are written as Repeat(…): edit the list in the Properties pane",
            Why::LastTrack => "A grid keeps at least one row and one column",
            Why::NothingToRestore => "Nothing has been removed from this grid",
            Why::OneCell => "Select two or more cells to merge",
            Why::NoControl => "There is no control in the selected cells to merge",
            Why::SeveralControls => "More than one control is in the selected cells: a merge needs exactly one",
            Why::NothingToSplit => "The selected cell holds no merged control",
            Why::Scaled => "This grid is shown at a scale: edit it at its own size",
        }
    }
}

fn why_of(r: Refusal) -> Why {
    match r {
        Refusal::Repeat(_) => Why::Repeat,
        Refusal::LastTrack(_) => Why::LastTrack,
        Refusal::NoSuchTrack => Why::NoCell,
        Refusal::NoControlInRange => Why::NoControl,
        Refusal::SeveralControlsInRange => Why::SeveralControls,
        Refusal::OneCell => Why::OneCell,
        Refusal::NothingToSplit => Why::NothingToSplit,
        Refusal::FixedTrack => Why::Repeat,
        Refusal::Scaled => Why::Scaled,
    }
}

/// Whether `tool` applies to this grid and this selection, or why it does not — the
/// one answer the toolbar's enabled state, its tips and the action all read (R9). The
/// answer is the edit's own: a button is enabled exactly when the edit would be made.
pub(crate) fn availability(
    tool: Tool,
    view: &GridView,
    sel: Option<&CellSel>,
    latest_removal: Option<&RemovedTrack>,
) -> Result<(), Why> {
    use cobolt_forms::layout::grid_edit as ge;
    let range = sel.map(|s| s.clamped(view.row_sizes.len(), view.column_sizes.len()).range());
    match tool {
        Tool::AddColumn => ge::insert_track(view, Axis::Column, None).map(|_| ()).map_err(why_of),
        Tool::AddRow => ge::insert_track(view, Axis::Row, None).map(|_| ()).map_err(why_of),
        Tool::RemoveColumn | Tool::RemoveRow => {
            let (axis, index, span) = match (tool, range) {
                (_, None) => return Err(Why::NoCell),
                (Tool::RemoveColumn, Some(r)) => (Axis::Column, r.column, r.columns),
                (_, Some(r)) => (Axis::Row, r.row, r.rows),
            };
            if span > 1 {
                return Err(Why::SeveralTracks);
            }
            ge::remove_track(view, axis, index).map(|_| ()).map_err(why_of)
        }
        Tool::Restore => match latest_removal {
            None => Err(Why::NothingToRestore),
            Some(r) => ge::restore_track(view, r).map(|_| ()).map_err(why_of),
        },
        Tool::Merge => match range {
            None => Err(Why::NoCell),
            Some(r) => ge::merge(view, r).map(|_| ()).map_err(why_of),
        },
        Tool::Split => match sel {
            None => Err(Why::NoCell),
            Some(s) => ge::split_cell(view, s.anchor.0, s.anchor.1).map(|_| ()).map_err(why_of),
        },
    }
}

/// A line between two tracks that the pointer can take (R18): the track before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LineHit {
    pub grid: String,
    pub axis: Axis,
    pub line: usize,
}

/// A Grid container as the canvas drew it: where its tracks are, in form space.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GridGeom {
    pub id: String,
    pub client: LRect,
    pub columns: Vec<f32>,
    pub rows: Vec<f32>,
    pub gaps: (f32, f32),
}

impl GridGeom {
    /// Every Grid container the layout reports that has tracks to show.
    pub fn all(layout: &LayoutOutput) -> Vec<GridGeom> {
        let mut out: Vec<GridGeom> = layout
            .containers
            .iter()
            .filter(|(_, g)| g.mode == LayoutMode::Grid && !g.columns.is_empty() && !g.rows.is_empty())
            .map(|(id, g)| GridGeom { id: id.clone(), client: g.client, columns: g.columns.clone(), rows: g.rows.clone(), gaps: g.gaps })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn of(layout: &LayoutOutput, id: &str) -> Option<GridGeom> {
        GridGeom::all(layout).into_iter().find(|g| g.id == id)
    }

    fn total(sizes: &[f32], gap: f32) -> f32 {
        sizes.iter().sum::<f32>() + sizes.len().saturating_sub(1) as f32 * gap
    }

    /// The tracks and their gaps: the rectangle the grid draws in.
    pub fn bounds(&self) -> LRect {
        LRect::new(self.client.x, self.client.y, Self::total(&self.columns, self.gaps.0), Self::total(&self.rows, self.gaps.1))
    }

    fn start(sizes: &[f32], gap: f32, i: usize) -> f32 {
        sizes[..i.min(sizes.len())].iter().sum::<f32>() + gap * i as f32
    }

    /// Where column `i` starts (form x).
    pub fn column_x(&self, i: usize) -> f32 {
        self.client.x + Self::start(&self.columns, self.gaps.0, i)
    }

    /// Where row `i` starts (form y).
    pub fn row_y(&self, i: usize) -> f32 {
        self.client.y + Self::start(&self.rows, self.gaps.1, i)
    }

    pub fn cell_rect(&self, row: usize, column: usize) -> LRect {
        LRect::new(self.column_x(column), self.row_y(row), self.columns[column], self.rows[row])
    }

    /// The rectangle a range of cells covers, gaps between them included.
    pub fn range_rect(&self, r: CellRange) -> LRect {
        let last_c = (r.column + r.columns - 1).min(self.columns.len() - 1);
        let last_r = (r.row + r.rows - 1).min(self.rows.len() - 1);
        let (a, b) = (self.cell_rect(r.row.min(self.rows.len() - 1), r.column.min(self.columns.len() - 1)), self.cell_rect(last_r, last_c));
        LRect::new(a.x, a.y, b.x + b.w - a.x, b.y + b.h - a.y)
    }

    fn contains(&self, x: f32, y: f32) -> bool {
        let b = self.bounds();
        x >= b.x && x <= b.x + b.w && y >= b.y && y <= b.y + b.h
    }

    /// The cell under `(x, y)`; a point in a gap belongs to the track before it.
    pub fn cell_at(&self, x: f32, y: f32) -> Option<(usize, usize)> {
        if !self.contains(x, y) {
            return None;
        }
        let pick = |sizes: &[f32], gap: f32, from: f32, at: f32| {
            let mut edge = from;
            for (i, s) in sizes.iter().enumerate() {
                if at < edge + s + gap / 2.0 {
                    return i;
                }
                edge += s + gap;
            }
            sizes.len() - 1
        };
        Some((pick(&self.rows, self.gaps.1, self.client.y, y), pick(&self.columns, self.gaps.0, self.client.x, x)))
    }

    /// The cell nearest `(x, y)`: the point is first brought inside the tracks, so a drag
    /// that leaves the grid still names the cell at its edge.
    pub fn nearest_cell(&self, x: f32, y: f32) -> (usize, usize) {
        let b = self.bounds();
        self.cell_at(x.clamp(b.x, b.x + b.w), y.clamp(b.y, b.y + b.h)).unwrap_or((0, 0))
    }

    /// The inner line within `tol` of `(x, y)` — the nearest, a column line first
    /// when two are as near. The grid's outer border is not a line (R18).
    pub fn line_at(&self, x: f32, y: f32, tol: f32) -> Option<LineHit> {
        let b = self.bounds();
        let mut best: Option<(f32, LineHit)> = None;
        let mut consider = |d: f32, hit: LineHit| {
            if d <= tol && best.as_ref().is_none_or(|(bd, _)| d < *bd - 0.0001) {
                best = Some((d, hit));
            }
        };
        if y >= b.y - tol && y <= b.y + b.h + tol {
            for i in 0..self.columns.len().saturating_sub(1) {
                let at = self.column_x(i) + self.columns[i] + self.gaps.0 / 2.0;
                consider((x - at).abs(), LineHit { grid: self.id.clone(), axis: Axis::Column, line: i });
            }
        }
        if x >= b.x - tol && x <= b.x + b.w + tol {
            for i in 0..self.rows.len().saturating_sub(1) {
                let at = self.row_y(i) + self.rows[i] + self.gaps.1 / 2.0;
                consider((y - at).abs(), LineHit { grid: self.id.clone(), axis: Axis::Row, line: i });
            }
        }
        best.map(|(_, h)| h)
    }

    /// Where the line is, in form space: its x for a column line, its y for a row line.
    pub fn line_pos(&self, axis: Axis, line: usize) -> f32 {
        match axis {
            Axis::Column => self.column_x(line) + self.columns[line] + self.gaps.0 / 2.0,
            Axis::Row => self.row_y(line) + self.rows[line] + self.gaps.1 / 2.0,
        }
    }
}

/// Draw a grid's track lines — dashed, with each track's size — in form space
/// shifted by `origin` (spec 056 R45, for every grid the canvas shows, spec 088 R3).
pub(crate) fn paint_grid(painter: &egui::Painter, origin: egui::Pos2, g: &GridGeom, accent: egui::Color32) {
    let at = |x: f32, y: f32| origin + egui::vec2(x, y);
    let dash = egui::Stroke::new(1.0, accent.gamma_multiply(0.8));
    let font = egui::FontId::proportional(9.0);
    let b = g.bounds();
    let line = |a: egui::Pos2, z: egui::Pos2| painter.extend(egui::Shape::dashed_line(&[a, z], dash, 4.0, 3.0));
    for (i, w) in g.columns.iter().enumerate() {
        let x = g.column_x(i);
        line(at(x, b.y), at(x, b.y + b.h));
        painter.text(at(x + w / 2.0, b.y + 2.0), egui::Align2::CENTER_TOP, format!("{w:.0}"), font.clone(), accent);
    }
    let right = b.x + b.w;
    line(at(right, b.y), at(right, b.y + b.h));
    for (i, h) in g.rows.iter().enumerate() {
        let y = g.row_y(i);
        line(at(b.x, y), at(b.x + b.w, y));
        painter.text(at(b.x + 2.0, y + h / 2.0), egui::Align2::LEFT_CENTER, format!("{h:.0}"), font.clone(), accent);
    }
    let bottom = b.y + b.h;
    line(at(b.x, bottom), at(b.x + b.w, bottom));
}

/// Highlight the selected cells (R7): a tinted block with an outline.
pub(crate) fn paint_selection(painter: &egui::Painter, origin: egui::Pos2, g: &GridGeom, sel: &CellSel, accent: egui::Color32) {
    let sel = sel.clamped(g.rows.len(), g.columns.len());
    paint_range(painter, origin, g, sel.range(), accent);
}

/// Highlight a range of cells: a tinted block with an outline.
pub(crate) fn paint_range(painter: &egui::Painter, origin: egui::Pos2, g: &GridGeom, range: CellRange, accent: egui::Color32) {
    let r = g.range_rect(range);
    let rect = egui::Rect::from_min_size(origin + egui::vec2(r.x, r.y), egui::vec2(r.w, r.h));
    painter.rect_filled(rect, 2.0, accent.gamma_multiply(0.28));
    painter.rect_stroke(rect, 2.0, egui::Stroke::new(2.0, accent), egui::StrokeKind::Inside);
}

// ── The floating toolbar (R8, R9) ────────────────────────────────────────────

pub(crate) const TOOL_SIZE: f32 = 26.0;
pub(crate) const TOOL_GAP: f32 = 4.0;
pub(crate) const TOOL_PAD: f32 = 5.0;
/// The space kept between the toolbar and the grid it belongs to.
const TOOLBAR_MARGIN: f32 = 6.0;

/// The toolbar's size: a framed strip of seven buttons — fixed, never measured from the
/// room there is (the no-self-resizing rule).
pub(crate) fn toolbar_size() -> egui::Vec2 {
    let n = TOOLS.len() as f32;
    egui::vec2(TOOL_PAD * 2.0 + n * TOOL_SIZE + (n - 1.0) * TOOL_GAP, TOOL_PAD * 2.0 + TOOL_SIZE)
}

/// Where the toolbar goes, on screen: beside the container (`grid`, its whole rectangle),
/// **outside** it and so never over it, and clamped inside the `canvas` (R8). It goes above
/// the grid, left-aligned, when there is room; else below; else to the right; else to the
/// left. When the grid leaves no room at all the toolbar is kept on the canvas and sits
/// over the grid's edge — the one case where it must.
pub(crate) fn toolbar_pos(grid: egui::Rect, canvas: egui::Rect, size: egui::Vec2) -> egui::Pos2 {
    let m = TOOLBAR_MARGIN;
    let fit_x = |x: f32| x.min(canvas.max.x - size.x).max(canvas.min.x);
    let fit_y = |y: f32| y.min(canvas.max.y - size.y).max(canvas.min.y);
    let candidates = [
        egui::pos2(fit_x(grid.min.x), grid.min.y - m - size.y),
        egui::pos2(fit_x(grid.min.x), grid.max.y + m),
        egui::pos2(grid.max.x + m, fit_y(grid.min.y)),
        egui::pos2(grid.min.x - m - size.x, fit_y(grid.min.y)),
    ];
    let inside = |p: egui::Pos2| {
        let r = egui::Rect::from_min_size(p, size);
        canvas.contains_rect(r) && !r.intersects(grid)
    };
    candidates
        .into_iter()
        .find(|p| inside(*p))
        .unwrap_or_else(|| egui::pos2(fit_x(grid.min.x), fit_y(grid.min.y - m - size.y)))
}

/// A tool's icon, drawn as plain shapes inside `r`: bars with a plus or a minus, a circular
/// arrow, a cell with arrows meeting or leaving.
fn paint_icon(p: &egui::Painter, r: egui::Rect, tool: Tool, ink: egui::Color32) {
    use egui::{pos2, vec2};
    let stroke = egui::Stroke::new(1.6, ink);
    let c = r.center();
    let bars = |vertical: bool| {
        for k in -1..=1 {
            let off = k as f32 * 5.0;
            let (a, b) = if vertical {
                (pos2(c.x + off, c.y - 8.0), pos2(c.x + off, c.y + 5.0))
            } else {
                (pos2(c.x - 8.0, c.y + off - 1.5), pos2(c.x + 5.0, c.y + off - 1.5))
            };
            p.line_segment([a, b], egui::Stroke::new(2.4, ink.gamma_multiply(0.7)));
        }
    };
    let badge = |plus: bool| {
        let o = pos2(r.max.x - 6.5, r.max.y - 6.5);
        p.circle_filled(o, 5.0, ink);
        let dark = egui::Color32::from_rgb(20, 24, 40);
        p.line_segment([o - vec2(2.6, 0.0), o + vec2(2.6, 0.0)], egui::Stroke::new(1.4, dark));
        if plus {
            p.line_segment([o - vec2(0.0, 2.6), o + vec2(0.0, 2.6)], egui::Stroke::new(1.4, dark));
        }
    };
    let cell = egui::Rect::from_center_size(c, vec2(16.0, 12.0));
    // An arrowhead at `tip` pointing along `dir` (a unit vector).
    let head = |tip: egui::Pos2, dir: egui::Vec2| {
        let n = vec2(-dir.y, dir.x);
        p.add(egui::Shape::convex_polygon(vec![tip, tip - dir * 4.0 + n * 3.0, tip - dir * 4.0 - n * 3.0], ink, egui::Stroke::NONE));
    };
    match tool {
        Tool::AddColumn | Tool::RemoveColumn => {
            bars(true);
            badge(tool == Tool::AddColumn);
        }
        Tool::AddRow | Tool::RemoveRow => {
            bars(false);
            badge(tool == Tool::AddRow);
        }
        Tool::Restore => {
            let radius = 7.0;
            let pts: Vec<egui::Pos2> = (0..=18)
                .map(|i| {
                    let a = -0.6 + i as f32 * (std::f32::consts::TAU * 0.78 / 18.0);
                    c + vec2(a.cos(), a.sin()) * radius
                })
                .collect();
            let tip = *pts.last().unwrap_or(&c);
            p.add(egui::Shape::line(pts, stroke));
            p.line_segment([tip, tip + vec2(-4.0, -1.5)], stroke);
            p.line_segment([tip, tip + vec2(0.5, -4.5)], stroke);
        }
        Tool::Merge => {
            // Two cells becoming one: the frame, and an arrow from each side to the middle.
            p.rect_stroke(cell, 1.5, stroke, egui::StrokeKind::Middle);
            head(pos2(c.x - 1.0, c.y), vec2(1.0, 0.0));
            head(pos2(c.x + 1.0, c.y), vec2(-1.0, 0.0));
        }
        Tool::Split => {
            // One cell becoming two: the frame, the new line, an arrow out of the middle each way.
            p.rect_stroke(cell, 1.5, stroke, egui::StrokeKind::Middle);
            p.line_segment([pos2(c.x, cell.min.y), pos2(c.x, cell.max.y)], stroke);
            head(pos2(c.x - 3.0, c.y), vec2(-1.0, 0.0));
            head(pos2(c.x + 3.0, c.y), vec2(1.0, 0.0));
        }
    }
}

/// The floating toolbar: seven icon-only buttons in a framed strip, each with a translated
/// tip — what it does, or why it does not — and disabled, never hidden, when it does not
/// apply. A foreground `Area` at a fixed position and of a fixed size, so nothing it holds
/// can resize it. Returns the tool pressed this frame and the strip's rectangle.
pub(crate) fn show_toolbar(
    ctx: &egui::Context,
    id: egui::Id,
    pos: egui::Pos2,
    state: &[(Tool, Result<(), Why>)],
    accent: egui::Color32,
) -> (Option<Tool>, egui::Rect) {
    use egui::{pos2, vec2};
    let size = toolbar_size();
    let mut pressed = None;
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .movable(false)
        .show(ctx, |ui| {
            let strip = egui::Rect::from_min_size(pos, size);
            ui.allocate_exact_size(size, egui::Sense::hover());
            let p = ui.painter();
            p.rect_filled(strip, 7.0, egui::Color32::from_rgba_unmultiplied(22, 26, 46, 238));
            p.rect_stroke(strip, 7.0, egui::Stroke::new(1.0, accent), egui::StrokeKind::Inside);
            for (i, (tool, avail)) in state.iter().enumerate() {
                let x = strip.min.x + TOOL_PAD + i as f32 * (TOOL_SIZE + TOOL_GAP);
                let r = egui::Rect::from_min_size(pos2(x, strip.min.y + TOOL_PAD), vec2(TOOL_SIZE, TOOL_SIZE));
                let on = avail.is_ok();
                let resp = ui.interact(r, id.with(("tool", i)), if on { egui::Sense::click() } else { egui::Sense::hover() });
                if on && resp.hovered() {
                    p.rect_filled(r, 5.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 34));
                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                let ink = if on { egui::Color32::from_gray(238) } else { egui::Color32::from_gray(110) };
                paint_icon(p, r, *tool, ink);
                let tip = match avail {
                    Ok(()) => crate::ui_text::ide(tool.tip()).to_owned(),
                    Err(why) => format!("{} — {}", crate::ui_text::ide(tool.tip()), crate::ui_text::ide(why.text())),
                };
                resp.clone().on_hover_text(tip);
                if on && resp.clicked() {
                    pressed = Some(*tool);
                }
            }
        });
    (pressed, egui::Rect::from_min_size(pos, size))
}

/// A line of small text under the toolbar: the notice of what a removal left auto-placed
/// (R12), else the hint that the mode is on. A foreground `Area` at a fixed position.
pub(crate) fn show_caption(ctx: &egui::Context, id: egui::Id, below: egui::Rect, text: &str, warn: bool) {
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(below.left_bottom() + egui::vec2(2.0, 3.0))
        .movable(false)
        .interactable(false)
        .show(ctx, |ui| {
            let ink = if warn { egui::Color32::from_rgb(232, 176, 64) } else { egui::Color32::from_gray(170) };
            ui.label(egui::RichText::new(text).small().italics().color(ink));
        });
}

/// The innermost grid under `(x, y)`: of the grids whose tracks hold the point, the
/// smallest — a grid in a cell of another is inside it (R31).
pub(crate) fn grid_at(grids: &[GridGeom], x: f32, y: f32) -> Option<&GridGeom> {
    grids
        .iter()
        .filter(|g| g.contains(x, y))
        .min_by(|a, b| {
            let (ba, bb) = (a.bounds(), b.bounds());
            (ba.w * ba.h).total_cmp(&(bb.w * bb.h))
        })
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

impl LayoutGridView {
    /// The most recent removal made in `container`: its place in the history, and
    /// the record.
    pub fn latest_for(&self, container: &str) -> Option<(usize, &RemovedTrack)> {
        self.history.iter().enumerate().rev().find(|(_, r)| same(&r.container, container))
    }

    /// A control was renamed: the records that name it follow it.
    pub fn renamed(&mut self, old: &str, new: &str) {
        for r in &mut self.history {
            if same(&r.container, old) {
                r.container = new.to_owned();
            }
            for h in &mut r.held {
                if same(&h.id, old) {
                    h.id = new.to_owned();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::layout::LayoutOutput;
    use cobolt_forms::model::{Control, PropValue, Rect};
    use cobolt_forms::ControlType;

    fn grid_panel(id: &str, rect: (i32, i32, i32, i32), cols: &str, rows: &str, parent: Option<&str>) -> Control {
        let mut p = Control::new(id, ControlType::Panel, rect.0, rect.1);
        p.rect = Rect::new(rect.0, rect.1, rect.2, rect.3);
        p.parent = parent.map(str::to_owned);
        for (k, v) in [("LayoutMode", "Grid"), ("GridColumns", cols), ("GridRows", rows), ("BorderStyle", "None")] {
            p.set_prop(k, PropValue::String(v.into()));
        }
        p.set_prop("BorderWidth", PropValue::Int(0));
        p
    }

    /// OUTER `100px 100px` × `50px 50px` at (0, 0) with INNER in its first cell (a grid
    /// `1fr 1fr` of 30-px rows), and an empty grid E1 beside it.
    fn layout() -> LayoutOutput {
        let controls = vec![
            grid_panel("OUTER", (0, 0, 200, 100), "100px 100px", "50px 50px", None),
            grid_panel("INNER", (0, 0, 10, 10), "1fr 1fr", "30px 30px", Some("OUTER")),
            grid_panel("E1", (300, 0, 100, 100), "1fr", "1fr", None),
        ];
        let bag = std::collections::BTreeMap::new();
        let input = cobolt_forms::layout::LayoutInput::new(&controls, (500.0, 300.0), (500.0, 300.0), &bag, &[]);
        cobolt_forms::layout::solve(&input)
    }

    /// R31 — of two grids that hold a point, the inner one (the smaller) is the one a
    /// press belongs to; a point only the outer holds belongs to the outer; an empty grid
    /// is a grid too.
    #[test]
    fn the_innermost_grid_under_the_pointer_wins() {
        let l = layout();
        let grids = GridGeom::all(&l);
        assert_eq!(grids.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), ["E1", "INNER", "OUTER"], "an empty grid has its tracks too");
        let at = |x: f32, y: f32| grid_at(&grids, x, y).map(|g| g.id.as_str());
        assert_eq!(at(20.0, 10.0), Some("INNER"), "inside the inner grid's tracks");
        assert_eq!(at(150.0, 70.0), Some("OUTER"), "in the outer's other cell");
        assert_eq!(at(350.0, 40.0), Some("E1"));
        assert_eq!(at(450.0, 280.0), None, "outside every grid");
        println!("innermost grid: (20,10) → INNER, (150,70) → OUTER, (350,40) → E1, (450,280) → none");
    }

    /// Cells are where the tracks are, a point in a gap belongs to the nearer track, and a
    /// range's rectangle covers its cells and the gaps between them.
    #[test]
    fn cells_and_ranges_are_where_the_tracks_are() {
        let mut g = GridGeom::of(&layout(), "OUTER").unwrap();
        let (x0, y0) = (g.client.x, g.client.y);
        assert_eq!(
            (g.cell_at(x0 + 10.0, y0 + 10.0), g.cell_at(x0 + 150.0, y0 + 10.0), g.cell_at(x0 + 150.0, y0 + 80.0)),
            (Some((0, 0)), Some((0, 1)), Some((1, 1)))
        );
        assert_eq!(g.cell_at(x0 + 250.0, y0 + 10.0), None, "past the last column");
        g.gaps = (10.0, 10.0);
        g.columns = vec![100.0, 100.0];
        g.rows = vec![50.0, 50.0];
        assert_eq!(g.cell_at(x0 + 104.0, y0 + 10.0), Some((0, 0)), "in the gap, before the half-way point: the track before");
        assert_eq!(g.cell_at(x0 + 106.0, y0 + 10.0), Some((0, 1)));
        let r = g.range_rect(CellRange::between((0, 0), (1, 1)));
        assert_eq!((r.w, r.h), (210.0, 110.0), "both cells and the gap between them");
        println!("cells: (10,10) → (0,0); in a 10-px gap a point goes to the nearer track; the 2×2 range is {}×{}", r.w, r.h);
    }

    /// R18 — a line is grabbed within 4 points; the outer border is not a line; in a gap
    /// the line is its middle; the nearest line wins and a column line wins a tie.
    #[test]
    fn a_line_is_grabbed_near_it_and_the_border_is_not_a_line() {
        let l = layout();
        let g = GridGeom::of(&l, "OUTER").unwrap();
        let (x0, y0) = (g.client.x, g.client.y);
        let hit = |x: f32, y: f32| g.line_at(x0 + x, y0 + y, LINE_GRAB);
        assert_eq!(hit(102.0, 10.0), Some(LineHit { grid: "OUTER".into(), axis: Axis::Column, line: 0 }));
        assert_eq!(hit(105.0, 10.0), None, "more than 4 points away");
        assert_eq!(hit(60.0, 52.0), Some(LineHit { grid: "OUTER".into(), axis: Axis::Row, line: 0 }));
        assert_eq!(hit(0.0, 10.0), None, "the left border");
        assert_eq!(hit(200.0, 10.0), None, "the right border");
        assert_eq!(hit(60.0, 100.0), None, "the bottom border");
        assert_eq!(hit(101.0, 51.0).map(|h| h.axis), Some(Axis::Column), "a tie goes to the column line");
        assert_eq!(hit(102.0, 400.0), None, "far below the grid");
        let mut gapped = g.clone();
        gapped.gaps = (10.0, 0.0);
        assert!((gapped.line_pos(Axis::Column, 0) - (x0 + 105.0)).abs() < 0.001, "the middle of a 10-px gap");
        println!("lines: grabbed within {LINE_GRAB} points, border refused, ties to the column line, the middle of a gap");
    }

    /// A selection is the range between where it started and where it was dragged to,
    /// whichever way round.
    #[test]
    fn a_selection_is_a_range_whichever_way_it_is_dragged() {
        let mut s = CellSel::single("G", (2, 3));
        assert_eq!(s.range(), CellRange { row: 2, column: 3, rows: 1, columns: 1 });
        s.head = (0, 1);
        assert_eq!(s.range(), CellRange { row: 0, column: 1, rows: 3, columns: 3 });
        assert!(LayoutGridView::default().show, "Show Grid starts on");
        println!("selection: (2,3) dragged to (0,1) → rows 0–2, columns 1–3");
    }

    // ── The toolbar's answers (T13) ───────────────────────────────────────────

    use cobolt_forms::layout::grid_edit as ge;

    /// A grid `1fr 1fr` × `1fr 1fr` holding A at (1,1) and B at (2,2), as the engine laid it out.
    fn view(columns: &str) -> GridView {
        view_with(columns, &[("A", 1, 1), ("B", 2, 2)])
    }

    fn view_with(columns: &str, children: &[(&str, i64, i64)]) -> GridView {
        let mut p = grid_panel("G", (0, 0, 400, 200), columns, "1fr 1fr", None);
        p.rect = Rect::new(0, 0, 400, 200);
        let mut kids = Vec::new();
        for &(id, col, row) in children {
            let mut c = Control::new(id, ControlType::Button, 0, 0);
            c.parent = Some("G".into());
            c.rect = Rect::new(0, 0, 40, 20);
            c.set_prop("GridColumn", PropValue::Int(col));
            c.set_prop("GridRow", PropValue::Int(row));
            kids.push(c);
        }
        let mut all = vec![p.clone()];
        all.extend(kids.clone());
        let bag = std::collections::BTreeMap::new();
        let input = cobolt_forms::layout::LayoutInput::new(&all, (400.0, 200.0), (400.0, 200.0), &bag, &[]);
        let out = cobolt_forms::layout::solve(&input);
        let refs: Vec<&Control> = kids.iter().collect();
        GridView::from_layout(&p, &refs, &out.containers["G"])
    }

    /// R9 — each button says whether it applies, and if not, which rule says no. A button is
    /// enabled exactly when the edit it stands for would be made.
    #[test]
    fn each_button_is_enabled_exactly_when_its_edit_would_be_made() {
        let v = view("1fr 1fr");
        let one = CellSel::single("G", (0, 0));
        let empty = CellSel::single("G", (1, 0));
        let at = |t: Tool, s: Option<&CellSel>| availability(t, &v, s, None);
        // No cell: adds are offered (at the end); the rest need a cell.
        assert_eq!(at(Tool::AddColumn, None), Ok(()));
        assert_eq!(at(Tool::AddRow, None), Ok(()));
        assert_eq!(at(Tool::RemoveColumn, None), Err(Why::NoCell));
        assert_eq!(at(Tool::Merge, None), Err(Why::NoCell));
        assert_eq!(at(Tool::Split, None), Err(Why::NoCell));
        // A cell: remove is offered; merge of one cell has nothing to merge; split needs a span.
        assert_eq!(at(Tool::RemoveColumn, Some(&one)), Ok(()));
        assert_eq!(at(Tool::RemoveRow, Some(&one)), Ok(()));
        assert_eq!(at(Tool::Merge, Some(&one)), Err(Why::OneCell));
        assert_eq!(at(Tool::Split, Some(&one)), Err(Why::NothingToSplit));
        // AC4 — Merge is disabled for a single empty cell.
        assert_eq!(at(Tool::Merge, Some(&empty)), Err(Why::OneCell));
        // A range: one control merges (A alone in the first two columns of row 1); two do not;
        // none do not; a range is not a single cell to remove.
        let a_only = CellSel { container: "G".into(), anchor: (0, 0), head: (0, 1) };
        assert_eq!(at(Tool::Merge, Some(&a_only)), Ok(()));
        let both = CellSel { container: "G".into(), anchor: (0, 0), head: (1, 1) };
        assert_eq!(at(Tool::Merge, Some(&both)), Err(Why::SeveralControls));
        let none = CellSel { container: "G".into(), anchor: (1, 0), head: (1, 0) };
        assert_eq!(at(Tool::Merge, Some(&none)), Err(Why::OneCell));
        assert_eq!(at(Tool::RemoveColumn, Some(&a_only)), Err(Why::SeveralTracks));
        assert_eq!(at(Tool::RemoveRow, Some(&a_only)), Ok(()), "one row, two columns: the row can go");
        // Nothing removed, nothing to restore; with a record it is offered.
        assert_eq!(at(Tool::Restore, Some(&one)), Err(Why::NothingToRestore));
        let removal = ge::remove_track(&v, Axis::Row, 0).unwrap();
        assert_eq!(availability(Tool::Restore, &v, None, Some(&removal.record)), Ok(()));
        // The last track of an axis, and a Repeat axis.
        let one_col = view_with("1fr", &[("A", 1, 1), ("B", 1, 2)]);
        assert_eq!(availability(Tool::RemoveColumn, &one_col, Some(&one), None), Err(Why::LastTrack));
        let rep = view("Repeat(2, 1fr)");
        assert_eq!(availability(Tool::AddColumn, &rep, None, None), Err(Why::Repeat));
        assert_eq!(availability(Tool::RemoveColumn, &rep, Some(&one), None), Err(Why::Repeat));
        assert_eq!(availability(Tool::AddRow, &rep, None, None), Ok(()), "the other axis is fine");
        // Every reason has words, and every tool a tip.
        assert!(TOOLS.iter().all(|t| !t.tip().is_empty()));
        for w in [Why::NoCell, Why::SeveralTracks, Why::Repeat, Why::LastTrack, Why::NothingToRestore, Why::OneCell, Why::NoControl, Why::SeveralControls, Why::NothingToSplit, Why::Scaled] {
            assert!(!w.text().is_empty());
        }
        println!("toolbar: 7 buttons; enabled exactly when the edit would be made; 10 reasons for 'no'; Merge disabled for one cell, none, or several controls");
    }

    /// R8 — the toolbar sits beside the container, outside it, and on the canvas: above when
    /// there is room, else below, else beside; and when the grid leaves no room it stays on
    /// the canvas.
    #[test]
    fn the_toolbar_is_beside_the_container_outside_it_and_on_the_canvas() {
        let canvas = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(1000.0, 700.0));
        let size = toolbar_size();
        let mut cases = 0;
        let mut check = |name: &str, grid: egui::Rect, expect: &str| {
            let at = toolbar_pos(grid, canvas, size);
            let bar = egui::Rect::from_min_size(at, size);
            assert!(canvas.contains_rect(bar), "{name}: on the canvas: {bar:?}");
            let side = if bar.max.y <= grid.min.y { "above" } else if bar.min.y >= grid.max.y { "below" } else if bar.min.x >= grid.max.x { "right" } else if bar.max.x <= grid.min.x { "left" } else { "over" };
            assert_eq!(side, expect, "{name}");
            if expect != "over" {
                assert!(!bar.intersects(grid), "{name}: never over the grid");
            }
            cases += 1;
        };
        let r = |x: f32, y: f32, w: f32, h: f32| egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h));
        check("a grid in the middle", r(400.0, 300.0, 300.0, 200.0), "above");
        check("a grid at the top of the canvas", r(400.0, 50.0, 300.0, 200.0), "below");
        check("a grid at the bottom, room above", r(400.0, 560.0, 300.0, 190.0), "above");
        check("a grid at the left edge", r(100.0, 300.0, 300.0, 200.0), "above");
        check("a grid at the right edge", r(800.0, 300.0, 300.0, 200.0), "above");
        // No room above or below: beside.
        check("a tall grid, room on the right", r(100.0, 50.0, 400.0, 700.0), "right");
        check("a tall grid, room on the left", r(700.0, 50.0, 400.0, 700.0), "left");
        // No room anywhere: kept on the canvas.
        check("a grid that fills the canvas", canvas, "over");
        println!("toolbar position: {cases} grid placements — above, below, right, left, and on the canvas when there is no room");
    }
}
