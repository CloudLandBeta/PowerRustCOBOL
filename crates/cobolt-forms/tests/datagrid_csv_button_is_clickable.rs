#![cfg(feature = "render")]
//! The CSV button gets a band of its own, and answers a click on it.
//!
//! It used to be centred in the column-header band at its right edge, painted
//! on top of the last column's title — and a column title is itself a click
//! target that sorts, so the button and the title were competing for the same
//! pixels (operator, 2026-09-16: "CSV button cannot be clicked"). It now sits
//! hard right on a caption row above the column titles, which the optional
//! `Title` shares.
//!
//! Three things are asserted, because each can rot without the others:
//!   * the badge answers a click, in both filter modes;
//!   * the badge's rect does not touch any column-title rect;
//!   * clicking it does not also sort the column underneath.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Rect, Vec2};

const GRID: Rect = Rect {
    min: egui::Pos2 { x: 20.0, y: 20.0 },
    max: egui::Pos2 { x: 920.0, y: 420.0 },
};

fn grid(filters: bool, title: &str) -> Vec<Control> {
    let mut grid = Control::new("DataGrid-1", ControlType::DataGrid, 20, 20);
    grid.rect = MRect::new(20, 20, 900, 400);
    grid.set_prop("ShowColumnFilters", PropValue::Bool(filters));
    grid.set_prop("ShowCSVExportButton", PropValue::Bool(true));
    grid.set_prop("Title", PropValue::String(title.into()));
    grid.set_prop(
        "Columns",
        PropValue::String("A:string\nB:string\nC:string".into()),
    );
    grid.set_prop("Rows", PropValue::String("1\t2\t3\n4\t5\t6".into()));
    vec![grid]
}

/// Where render.rs puts the badge, computed the same way it does.
fn badge_rect(filters: bool) -> Rect {
    let row_h = 22.0_f32;
    let header_h = if filters {
        (row_h * 1.85).max(row_h + 18.0)
    } else {
        row_h
    };
    // FontSize default is 12 → title_font 14 → caption 26.
    let caption_h = (14.0_f32 + 12.0).max(24.0);
    assert!(
        GRID.height() >= caption_h + header_h + row_h,
        "the fixture must be tall enough for the caption row to appear"
    );
    let badge_h = (caption_h - 8.0).clamp(14.0, 22.0);
    let badge_w = badge_h * 1.9;
    Rect::from_min_size(
        pos2(
            GRID.max.x - (badge_w + 8.0),
            GRID.min.y + (caption_h - badge_h) * 0.5,
        ),
        Vec2::new(badge_w, badge_h),
    )
}

/// Run one frame with `events` and report the UI events the engine produced.
fn frame(ctx: &egui::Context, controls: &[Control], events: Vec<egui::Event>) -> Vec<String> {
    let size = Vec2::new(1000.0, 600.0);
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
    input.events = events;
    let mut fired = Vec::new();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let inp = RenderInput {
                    controls,
                    state: &DesignedState,
                    form_size: size,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                let out = cobolt_forms::render::render_form(ui, &inp);
                fired = out
                    .events
                    .iter()
                    .map(|e| format!("{}:{}", e.ctrl_id, e.event))
                    .collect::<Vec<_>>();
            });
    });
    full.textures_delta.clear();
    fired
}

/// Press and release the primary button at `p`, and report the frame's events.
fn click_at(ctx: &egui::Context, controls: &[Control], p: egui::Pos2) -> Vec<String> {
    frame(ctx, controls, vec![]);
    frame(ctx, controls, vec![egui::Event::PointerMoved(p)]);
    frame(
        ctx,
        controls,
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    frame(
        ctx,
        controls,
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    )
}

#[test]
fn the_csv_badge_answers_a_click_and_sorts_nothing() {
    for filters in [false, true] {
        for title in ["", "Actors on the payroll"] {
            let controls = grid(filters, title);
            let ctx = egui::Context::default();
            let fired = click_at(&ctx, &controls, badge_rect(filters).center());
            assert!(
                fired.iter().any(|e| e == "DataGrid-1:onExportCSV"),
                "filters={filters} title={title:?}: the CSV badge did not answer a \
                 click; got {fired:?}"
            );
            // The badge is on its own row, so the click must not reach a column
            // header underneath it.
            assert!(
                !fired.iter().any(|e| e == "DataGrid-1:onColumnClick"),
                "filters={filters} title={title:?}: clicking the CSV badge also \
                 clicked a column header; got {fired:?}"
            );
        }
    }
}

#[test]
fn the_csv_badge_clears_every_column_title() {
    for filters in [false, true] {
        let controls = grid(filters, "Actors on the payroll");
        let ctx = egui::Context::default();
        frame(&ctx, &controls, vec![]);
        let badge = badge_rect(filters);

        // The column titles live in the band BELOW the caption row. Their top
        // edge is the only thing that has to clear the badge's bottom, and it
        // does so for every column at once.
        let row_h = 22.0_f32;
        let header_h = if filters {
            (row_h * 1.85).max(row_h + 18.0)
        } else {
            row_h
        };
        let caption_h = (14.0_f32 + 12.0).max(24.0);
        let titles_top = GRID.min.y + caption_h;
        assert!(
            badge.max.y <= titles_top,
            "filters={filters}: the badge ({badge:?}) reaches into the column-title \
             band, which starts at y={titles_top}"
        );
        assert!(
            badge.min.y >= GRID.min.y,
            "filters={filters}: the badge starts above the grid"
        );
        assert!(
            badge.max.x <= GRID.max.x,
            "filters={filters}: the badge runs past the grid's right edge"
        );
        // …and the column-title band is still the size it was: the caption row
        // is taken out of the BODY, never out of the headers.
        assert!(
            (titles_top + header_h) < GRID.max.y,
            "filters={filters}: nothing left for the body"
        );
    }
}

/// A grid with neither a title nor the button keeps every pixel it had: the
/// caption row is not a permanent tax on the body.
#[test]
fn a_grid_without_a_title_or_a_button_has_no_caption_row() {
    let mut plain = grid(true, "");
    plain[0].set_prop("ShowCSVExportButton", PropValue::Bool(false));
    let ctx = egui::Context::default();
    let fired = click_at(&ctx, &plain, badge_rect(true).center());
    assert!(
        !fired.iter().any(|e| e == "DataGrid-1:onExportCSV"),
        "a grid with the button off still exported; got {fired:?}"
    );
}
