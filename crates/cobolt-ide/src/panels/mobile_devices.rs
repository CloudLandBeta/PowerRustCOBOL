// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Run on device** (WASM-012 T19; R3, D-Preview): where a Mobile project's Run and Preview go.
//!
//! A Run goes straight to a device when the answer is plain: the one chosen here earlier in this
//! session, while it is still listed, else the only one running, else the only emulator set up
//! (started first). Otherwise — several running, or several emulators that must be started
//! first — the Preview asks for this window, which lists the
//! phones and emulators running now and the emulators set up that a Start starts first; Run →
//! **Run on device…** opens it at any time, to choose again or to stop the application. The
//! choice belongs to this session of the IDE and is never written to the project. The
//! application's log goes to the Output, as for any Run.

use eframe::egui;

use cobolt_compiler::mobile_devices::DeviceTarget;

use crate::i18n::Tr;
use crate::theme::Theme;

const DEFAULT_W: f32 = 560.0;
const DEFAULT_H: f32 = 400.0;
const MIN_W: f32 = 440.0;
const MIN_H: f32 = 300.0;
const MAX_W: f32 = 1200.0;
const MAX_H: f32 = 1000.0;
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;
/// Everything that is not the list: the title bar, the introduction, the status line, the buttons
/// and the spacing. Taken off the stored height, so the list never measures itself against the
/// space it is handed.
const CHROME_H: f32 = 190.0;
/// The list's height at the smallest window.
const LIST_MIN_H: f32 = 90.0;
/// A row: the target's name over its state.
const ROW_H: f32 = 42.0;
/// The card around the list: its padding, and the room kept for the scroll bar.
const CARD_PAD: i8 = 6;
const SCROLL_BAR: f32 = 16.0;

/// What the window asks the IDE to do.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RunOnDeviceAction {
    /// Run the application on this target (its [`DeviceTarget::id`]), and remember it for the
    /// session.
    pub start: Option<String>,
    /// Stop the application where it was started last.
    pub stop: bool,
    /// List the targets again.
    pub refresh: bool,
}

pub struct RunOnDevice {
    pub open: bool,
    /// The window's size, written only by the grip's drag (the window never resizes itself).
    size: egui::Vec2,
    /// The project's name, for the introduction.
    project: String,
    targets: Vec<DeviceTarget>,
    /// The id of the target the next Start goes to.
    selected: Option<String>,
}

impl RunOnDevice {
    /// The window over `targets`, with `chosen` (the session's choice) selected when it is listed.
    pub fn new(project: &str, targets: Vec<DeviceTarget>, chosen: Option<&str>) -> Self {
        let selected = preselect(&targets, chosen);
        Self {
            open: true,
            size: egui::vec2(DEFAULT_W, DEFAULT_H),
            project: project.to_owned(),
            targets,
            selected,
        }
    }

    /// The list read again (Refresh, or a Run that asks again): the selection stays while its
    /// target is listed.
    pub fn set_targets(&mut self, targets: Vec<DeviceTarget>) {
        self.selected = preselect(&targets, self.selected.as_deref());
        self.targets = targets;
    }

    /// The id of the selected target.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Select the target `id` (a row clicked); an id that is not listed changes nothing.
    pub fn select(&mut self, id: &str) {
        if let Some(t) = self.targets.iter().find(|t| t.is(id)) {
            self.selected = Some(t.id.clone());
        }
    }

    /// What Start asks for: the selected target.
    fn start(&self) -> RunOnDeviceAction {
        RunOnDeviceAction {
            start: self.selected.clone(),
            ..RunOnDeviceAction::default()
        }
    }

    /// A row: the target's name, and under it whether it runs now or is started first.
    fn row_text(target: &DeviceTarget, tr: &Tr) -> String {
        format!(
            "{}\n{}",
            target.label,
            if target.running {
                tr.dev_win_running
            } else {
                tr.dev_win_startable
            }
        )
    }

    /// The list's height, from the stored size.
    fn list_height(&self) -> f32 {
        (self.size.y - CHROME_H).max(LIST_MIN_H)
    }

    /// One grip, in its own foreground `Area`, the only writer of `self.size`.
    fn resize_grip(&mut self, ctx: &egui::Context, theme: &Theme, window: egui::Rect) {
        let corner = window.max - egui::vec2(GRIP_INSET, GRIP_INSET);
        let origin = corner - egui::vec2(GRIP, GRIP);
        egui::Area::new(egui::Id::new("run_on_device_resize_grip"))
            .order(egui::Order::Foreground)
            .fixed_pos(origin)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(GRIP, GRIP), egui::Sense::drag());
                if response.dragged() {
                    self.size += response.drag_delta();
                    self.size.x = self.size.x.clamp(MIN_W, MAX_W);
                    self.size.y = self.size.y.clamp(MIN_H, MAX_H);
                }
                let colour = if response.hovered() || response.dragged() {
                    theme.accent
                } else {
                    theme.text_dim
                };
                let painter = ui.painter();
                for i in 0..3 {
                    let o = 4.0 * i as f32;
                    painter.line_segment(
                        [
                            egui::pos2(rect.right() - o, rect.bottom()),
                            egui::pos2(rect.right(), rect.bottom() - o),
                        ],
                        egui::Stroke::new(1.5, colour),
                    );
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                }
            });
    }

    /// Draw the window. `started_on`: where the application was started last; `busy`: a package
    /// build or a run is under way (nothing can be started or stopped until it ends).
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        theme: &Theme,
        tr: &Tr,
        started_on: Option<&str>,
        busy: bool,
    ) -> RunOnDeviceAction {
        let mut action = RunOnDeviceAction::default();
        let mut open = self.open;
        let list_h = self.list_height();
        let window = egui::Window::new(egui::RichText::new(tr.dev_win_title).size(18.0).strong())
            .id(egui::Id::new("run_on_device"))
            .open(&mut open)
            .collapsible(false)
            // NEVER `resizable(true)`: egui would negotiate the rectangle against the list every
            // frame and the window would grow by itself. The size is ours; only the grip moves it.
            .resizable(false)
            .fixed_size(self.size)
            .default_pos([120.0, 90.0])
            .show(ctx, |ui| {
                // Laid out from the STORED size, never from the space the Ui offers.
                let margin = ui.style().spacing.window_margin.sum().x;
                let stroke = 2.0 * ui.style().visuals.window_stroke.width;
                let width = (self.size.x - margin - stroke).max(120.0);
                ui.set_width(width);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(tr.dev_win_intro.replacen("{}", &self.project, 1))
                            .color(theme.text_dim),
                    )
                    .wrap(),
                );
                ui.add_space(8.0);
                let inner = width - 2.0 * f32::from(CARD_PAD) - 2.0;
                egui::Frame::NONE
                    .fill(theme.bg_panel)
                    .stroke(egui::Stroke::new(1.0, theme.panel_border()))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(CARD_PAD))
                    .show(ui, |ui| {
                        ui.set_min_width(inner);
                        ui.set_max_width(inner);
                        // `both`: a name longer than the row scrolls instead of widening the window.
                        egui::ScrollArea::both()
                            .id_salt("run_on_device_rows")
                            .min_scrolled_height(list_h)
                            .max_height(list_h)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if self.targets.is_empty() {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(tr.dev_win_empty)
                                                .color(theme.text_dim),
                                        )
                                        .wrap(),
                                    );
                                }
                                let mut clicked = None;
                                for t in &self.targets {
                                    let selected = self.selected.as_deref() == Some(t.id.as_str());
                                    let row = egui::Button::selectable(
                                        selected,
                                        egui::RichText::new(Self::row_text(t, tr)),
                                    )
                                    .min_size(egui::vec2(inner - SCROLL_BAR, ROW_H));
                                    if ui.add(row).clicked() {
                                        clicked = Some(t.id.clone());
                                    }
                                }
                                if let Some(id) = clicked {
                                    self.select(&id);
                                }
                            });
                    });
                ui.add_space(8.0);
                let status = if busy {
                    tr.dev_win_busy.to_owned()
                } else {
                    started_on
                        .map(|d| tr.dev_win_started_on.replacen("{}", d, 1))
                        .unwrap_or_default()
                };
                ui.add(
                    egui::Label::new(egui::RichText::new(status).color(theme.text_dim)).truncate(),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !busy && self.selected.is_some(),
                            egui::Button::new(tr.dev_win_start),
                        )
                        .clicked()
                    {
                        action = self.start();
                    }
                    if ui
                        .add_enabled(
                            !busy && started_on.is_some(),
                            egui::Button::new(tr.dev_win_stop),
                        )
                        .clicked()
                    {
                        action.stop = true;
                    }
                    if ui
                        .add_enabled(!busy, egui::Button::new(tr.dev_win_refresh))
                        .clicked()
                    {
                        action.refresh = true;
                    }
                });
            });
        if let Some(r) = window {
            self.resize_grip(ctx, theme, r.response.rect);
        }
        if !open {
            self.open = false;
        }
        action
    }
}

/// The target a list opens on: `keep` when it is listed, else the first running, else the first.
fn preselect(targets: &[DeviceTarget], keep: Option<&str>) -> Option<String> {
    keep.and_then(|k| targets.iter().find(|t| t.is(k)))
        .or_else(|| targets.iter().find(|t| t.running))
        .or_else(|| targets.first())
        .map(|t| t.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    fn target(id: &str, label: &str, running: bool, emulator: Option<&str>) -> DeviceTarget {
        DeviceTarget {
            id: id.to_owned(),
            label: label.to_owned(),
            running,
            emulator: emulator.map(str::to_owned),
        }
    }

    /// A phone and an emulator running, and an emulator set up but not running.
    fn three() -> Vec<DeviceTarget> {
        vec![
            target("R58N12ABC", "Pixel 8 (R58N12ABC)", true, None),
            target(
                "emulator-5554",
                "Medium Phone API 37.0 (emulator-5554)",
                true,
                Some("Medium_Phone_API_37.0"),
            ),
            target("Pixel_9", "Pixel 9", false, Some("Pixel_9")),
        ]
    }

    /// The list model (T19): what is selected when the window opens, after a click and after a
    /// Refresh, and what Start and Stop ask for.
    #[test]
    fn the_window_selects_the_sessions_choice_and_start_asks_for_the_selected_target() {
        // No choice yet: the first one running.
        let w = RunOnDevice::new("PowerMobileHello", three(), None);
        assert_eq!(w.selected(), Some("R58N12ABC"));
        // The session's choice, when it is listed — by its id, or by the emulator's name once that emulator runs.
        assert_eq!(
            RunOnDevice::new("P", three(), Some("Pixel_9")).selected(),
            Some("Pixel_9")
        );
        assert_eq!(
            RunOnDevice::new("P", three(), Some("Medium_Phone_API_37.0")).selected(),
            Some("emulator-5554")
        );
        // A choice no longer listed: the first running again.
        assert_eq!(
            RunOnDevice::new("P", three(), Some("gone")).selected(),
            Some("R58N12ABC")
        );
        // Only emulators to start: the first of them.
        let only = vec![
            target("Pixel_9", "Pixel 9", false, Some("Pixel_9")),
            target("Tablet", "Tablet", false, Some("Tablet")),
        ];
        assert_eq!(
            RunOnDevice::new("P", only, None).selected(),
            Some("Pixel_9")
        );
        // Nothing: nothing selected, and Start has nothing to ask for.
        let none = RunOnDevice::new("P", Vec::new(), None);
        assert_eq!(none.selected(), None);
        assert_eq!(none.start(), RunOnDeviceAction::default());

        let mut w = RunOnDevice::new("P", three(), None);
        w.select("Pixel_9");
        assert_eq!(
            w.start(),
            RunOnDeviceAction {
                start: Some("Pixel_9".into()),
                stop: false,
                refresh: false
            }
        );
        w.select("not-listed");
        assert_eq!(
            w.selected(),
            Some("Pixel_9"),
            "a target that is not listed is never selected"
        );
        // Refresh: Pixel_9 now runs as emulator-5556 and keeps the selection by its name.
        w.set_targets(vec![
            target(
                "emulator-5556",
                "Pixel 9 (emulator-5556)",
                true,
                Some("Pixel_9"),
            ),
            target("R58N12ABC", "Pixel 8 (R58N12ABC)", true, None),
        ]);
        assert_eq!(w.selected(), Some("emulator-5556"));
        // The phone unplugged and the emulator closed: the selection moves to what is listed.
        w.set_targets(vec![target("Tablet", "Tablet", false, Some("Tablet"))]);
        assert_eq!(w.selected(), Some("Tablet"));
        println!("run on device: opened on the first running of 3, on the session's choice by id and by emulator name, on the first running when the choice is gone, on the first of 2 to start, on nothing of 0; a click, an unlisted id refused, 2 refreshes");
    }

    /// Every text of the window in the six languages, with its value in place.
    #[test]
    fn the_window_speaks_the_six_languages() {
        for &lang in Language::ALL {
            let tr = lang.tr();
            for text in [
                tr.dev_win_title,
                tr.dev_win_running,
                tr.dev_win_startable,
                tr.dev_win_empty,
                tr.dev_win_start,
                tr.dev_win_stop,
                tr.dev_win_refresh,
                tr.dev_win_busy,
                tr.menu_run_on_device,
            ] {
                assert!(
                    !text.trim().is_empty() && !text.contains("{}"),
                    "{lang:?}: {text:?}"
                );
            }
            for template in [tr.dev_win_intro, tr.dev_win_started_on] {
                assert_eq!(template.matches("{}").count(), 1, "{lang:?}: {template}");
            }
            assert!(
                tr.dev_win_empty.contains("Android Studio")
                    && tr.dev_win_empty.contains("Device Manager"),
                "{lang:?}"
            );
            let row = RunOnDevice::row_text(&three()[2], &tr);
            assert!(
                row.starts_with("Pixel 9\n") && row.ends_with(tr.dev_win_startable),
                "{lang:?}: {row}"
            );
        }
        println!("run on device: 11 texts x 6 languages, the 2 templates with one value each");
    }

    /// The window keeps the size it opened at, frame after frame, with a long target name in the
    /// list and in each language: only the grip changes it.
    #[test]
    fn the_window_never_resizes_itself() {
        for &lang in Language::ALL {
            let ctx = egui::Context::default();
            let theme = crate::theme::default_theme();
            let tr = lang.tr();
            crate::app::apply_glass_visuals(&ctx, theme);
            let mut targets = three();
            targets.push(target(
                "emulator-5556",
                "A very long emulator name that does not fit in one row at all (emulator-5556)",
                true,
                Some("Long"),
            ));
            let mut w = RunOnDevice::new("PowerMobileHello", targets, None);
            let mut rects = Vec::new();
            for frame in 0..12 {
                let mut input = egui::RawInput::default();
                input.screen_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(3000.0, 2000.0),
                ));
                ctx.begin_pass(input);
                let _ = w.show(
                    &ctx,
                    theme,
                    &tr,
                    if frame % 2 == 0 {
                        Some("emulator-5554")
                    } else {
                        None
                    },
                    frame % 3 == 0,
                );
                ctx.end_pass().textures_delta.clear();
                rects.push(
                    ctx.memory(|m| m.area_rect(egui::Id::new("run_on_device")))
                        .expect("the window registered an area"),
                );
            }
            assert_eq!(
                w.size,
                egui::vec2(DEFAULT_W, DEFAULT_H),
                "{lang:?}: the stored size moved with no drag"
            );
            let settled = rects[2];
            for r in &rects[2..] {
                assert!(
                    (r.width() - settled.width()).abs() < 0.5
                        && (r.height() - settled.height()).abs() < 0.5,
                    "{lang:?}: {rects:?}"
                );
            }
            assert!(
                (settled.width() - DEFAULT_W).abs() < 0.5,
                "{lang:?}: the window is {} wide, not {DEFAULT_W}",
                settled.width()
            );
        }
        println!("run on device: 12 frames x 6 languages at {DEFAULT_W} x {DEFAULT_H}, a 4th row longer than the window, the status changing each frame: the size never moved");
    }

    #[test]
    fn the_list_follows_the_stored_height() {
        let mut w = RunOnDevice::new("P", three(), None);
        assert_eq!(w.list_height(), DEFAULT_H - CHROME_H);
        w.size.y = MIN_H;
        assert!(
            w.list_height() >= LIST_MIN_H,
            "the list keeps a floor at the smallest window"
        );
    }
}
