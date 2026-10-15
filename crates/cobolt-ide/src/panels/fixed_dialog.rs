// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A small dialog of one exact width — the Platform SDK and Build Folder windows.
//!
//! A window never resizes itself (`CONVENTIONS.md`), so these have a width of their own and
//! nothing negotiates it. What is easy to get wrong is WHICH width: `fixed_size` is the window's
//! OUTER size. egui fits the title bar and the body inside it after the frame's margin and stroke
//! and then the content margin around the body, so a body made as wide as the outer width is wider
//! than the room it was given. The frame follows the body, the title bar stays at the width the
//! window offered, and the title bar ends short of the right edge — which is how the Build Folder
//! window first reached the operator (2026-10-15). [`content_width`] is the one place that does
//! the subtraction, as `leaderboard_modal.rs` does for its own window.

/// The width a dialog's body can have inside a window whose outer width is `outer_width`.
pub(crate) fn content_width(ui: &egui::Ui, outer_width: f32) -> f32 {
    let margin = ui.style().spacing.window_margin.sum().x;
    let stroke = 2.0 * ui.style().visuals.window_stroke.width;
    (outer_width - margin - stroke).max(1.0)
}

/// Show a centred dialog of outer width `outer_width`, its height whatever the body needs. `open`
/// is the window's close button; the body lays itself out from the width it is given and never
/// from the space available.
pub(crate) fn show(
    ctx: &egui::Context,
    title: impl Into<egui::WidgetText>,
    outer_width: f32,
    open: &mut bool,
    body: impl FnOnce(&mut egui::Ui),
) {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .fixed_size([outer_width, 0.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .open(open)
        .show(ctx, |ui| {
            ui.set_width(content_width(ui, outer_width));
            body(ui);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 560.0;

    /// A body like the Build Folder window's: a long paragraph, a long path, a rule, three buttons.
    fn body(ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(
            "Building an application stages it in a working folder, where the generated files and \
             everything already compiled are kept between builds. Leave it on the default to use the \
             system's own place — the per-user cache folder — or choose another folder, for example \
             on a larger drive.",
        );
        ui.horizontal_wrapped(|ui| {
            ui.strong("In use:");
            ui.monospace("/Users/emersonlopes/Library/Caches/PowerRustCOBOL/builds");
        });
        ui.separator();
        ui.horizontal(|ui| {
            let _ = ui.button("Choose folder…");
            let _ = ui.button("Use the default");
            let _ = ui.button("Close");
        });
    }

    /// Run `frames` frames of the dialog under the app's real visuals and return the rects painted on
    /// the last one.
    fn run(frames: usize, pixels_per_point: f32) -> Vec<egui::epaint::RectShape> {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(pixels_per_point);
        crate::app::apply_glass_visuals(&ctx, crate::theme::default_theme());
        let mut open = true;
        let mut rects = Vec::new();
        for frame in 0..frames {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(2000.0, 1200.0)));
            ctx.begin_pass(input);
            show(&ctx, "📁 Build Folder…", W, &mut open, body);
            let mut full = ctx.end_pass();
            full.textures_delta.clear();
            if frame + 1 == frames {
                for cs in &full.shapes {
                    collect(&cs.shape, &mut rects);
                }
            }
        }
        rects
    }

    /// The window frame: rounded on all four corners and stroked.
    fn frame_of(rects: &[egui::epaint::RectShape]) -> &egui::epaint::RectShape {
        rects
            .iter()
            .find(|r| {
                let c = r.corner_radius;
                c.nw > 0 && c.ne > 0 && c.sw > 0 && c.se > 0 && r.stroke.width > 0.0
            })
            .expect("no window frame painted")
    }

    /// The title bar: the top corners rounded, the bottom ones square.
    fn title_of(rects: &[egui::epaint::RectShape]) -> &egui::epaint::RectShape {
        rects
            .iter()
            .find(|r| {
                let c = r.corner_radius;
                c.nw > 0 && c.ne > 0 && c.sw == 0 && c.se == 0
            })
            .expect("no title bar painted")
    }

    fn collect(shape: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
        match shape {
            egui::Shape::Rect(r) => out.push(r.clone()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }

    /// The window frame and its title bar are the same rectangle, left and right
    /// (operator, 2026-10-15: "build folder settings window is broken (title)").
    #[test]
    fn the_title_bar_and_the_window_frame_are_the_same_rect() {
        for ppp in [1.0, 2.0] {
            let rects = run(4, ppp);
            let (frame, title) = (frame_of(&rects), title_of(&rects));
            println!("ppp {ppp}: frame {:?} title bar {:?}", frame.rect, title.rect);
            for (edge, a, b) in [("left", frame.rect.left(), title.rect.left()), ("right", frame.rect.right(), title.rect.right())] {
                assert!((a - b).abs() < 0.5, "the frame and the title bar disagree on their {edge} edge ({a:.1} vs {b:.1}) at {ppp}x");
            }
        }
    }

    /// The window is the width it was given, and stays there frame after frame.
    #[test]
    fn the_dialog_is_exactly_as_wide_as_it_was_given_and_does_not_grow() {
        for frames in [2, 6, 16] {
            let rects = run(frames, 1.0);
            let width = frame_of(&rects).rect.width();
            println!("{frames} frames: window {width:.1} wide");
            assert!((width - W).abs() < 1.0, "the window is {width:.1} wide after {frames} frames, not {W}");
        }
    }
}
