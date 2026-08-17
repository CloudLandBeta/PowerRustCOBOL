// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The ONE toolbar renderer.
//!
//! The designer canvas, the live preview, Run Form and the compiled binary all
//! draw a toolbar through [`draw`], for the reason the sidebar and the breadcrumb
//! have their own shared renderers: a bar that looks different depending on which
//! surface you are looking at is a bar you cannot design against.
//!
//! Geometry is not decided here — [`ToolbarDef::layout`] owns that, model-side
//! and testable without a window. This module only turns that layout into paint.
//!
//! # The theme decides every colour nobody chose
//!
//! Each colour on a group or a button defaults to empty, and empty asks the
//! form's active theme: a group's frame takes the theme's border, its face the
//! theme's card, a button's face the raised card, its text and icon the theme's
//! text colour. A colour the developer actually picked always wins. So a toolbar
//! belongs to the form it is on without being configured, and a configured
//! toolbar keeps exactly what it was given.

use egui::{Color32, Painter, Rect, Vec2};

use crate::paint;
use crate::surface_theme::ColorToken as Tok;
use crate::toolbar::{Box2, ToolbarButton, ToolbarDef, ToolbarGroup};
use crate::toolbar as cobolt_forms_style;

/// How a button is being touched right now, so the face can answer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonState {
    #[default]
    Idle,
    Hovered,
    Pressed,
}

/// What the caller must tell the renderer about pointer state. Design-time
/// surfaces pass [`Interaction::inert`] — a canvas is not a running form.
#[derive(Clone, Copy, Debug, Default)]
pub struct Interaction<'a> {
    pub hovered: Option<&'a str>,
    pub pressed: Option<&'a str>,
}

impl Interaction<'_> {
    /// Nothing is hovered and nothing is held — the designer canvas and the
    /// static preview face.
    pub fn inert() -> Self {
        Self::default()
    }

    fn state_of(&self, id: &str) -> ButtonState {
        if self.pressed == Some(id) {
            ButtonState::Pressed
        } else if self.hovered == Some(id) {
            ButtonState::Hovered
        } else {
            ButtonState::Idle
        }
    }
}

/// Draw `def` into `rect` and report where each button landed.
///
/// The returned rects are in SCREEN space, in draw order, so the caller can
/// hit-test them, attach tooltips, or (in the designer) draw selection handles.
/// A disabled button is still returned — it occupies its space and the caller
/// still needs to know it is there; it simply must not act on a press.
pub fn draw(
    painter: &Painter,
    rect: Rect,
    def: &ToolbarDef,
    alpha_mul: f32,
    interaction: Interaction<'_>,
) -> Vec<(String, Rect)> {
    let ctx = painter.ctx();
    let theme = Theme::resolve(ctx);
    let to_screen = |b: Box2| -> Rect {
        Rect::from_min_size(
            rect.min + Vec2::new(b.x as f32, b.y as f32),
            Vec2::new(b.w as f32, b.h as f32),
        )
    };

    let layout = def.layout(rect.width().max(0.0) as i64, rect.height().max(0.0) as i64);
    let mut placed = Vec::new();

    for group_layout in &layout {
        let Some(group) = def.groups.get(group_layout.group_index) else {
            continue;
        };
        draw_group_frame(painter, to_screen(group_layout.frame), group, &theme, alpha_mul);

        for (id, box2) in &group_layout.buttons {
            let Some(button) = group.buttons.iter().find(|b| &b.id == id) else {
                continue;
            };
            let screen = to_screen(*box2);
            // The button's own style over its group's over the built-in — one
            // call, so the painter never has to know there are layers.
            let style = button.resolved(group);
            draw_button(
                painter,
                screen,
                button,
                &style,
                &theme,
                alpha_mul,
                interaction.state_of(id),
            );
            placed.push((id.clone(), screen));
        }
    }
    placed
}

/// The colours the active theme supplies, resolved once per frame.
struct Theme {
    border: Color32,
    card: Color32,
    card_raised: Color32,
    text: Color32,
    dim_text: Color32,
}

impl Theme {
    fn resolve(ctx: &egui::Context) -> Self {
        // Liquid Glass supplies no flat tokens, so each falls back to a value
        // that reads on both a light and a dark form.
        Self {
            border: paint::theme_token(ctx, Tok::Border).unwrap_or(Color32::from_gray(120)),
            card: paint::theme_token(ctx, Tok::Card)
                .unwrap_or(Color32::from_rgba_unmultiplied(255, 255, 255, 18)),
            card_raised: paint::theme_token(ctx, Tok::CardRaised)
                .unwrap_or(Color32::from_rgba_unmultiplied(255, 255, 255, 32)),
            text: paint::theme_token(ctx, Tok::Text).unwrap_or(Color32::from_gray(230)),
            dim_text: paint::theme_token(ctx, Tok::DimText).unwrap_or(Color32::from_gray(140)),
        }
    }
}

/// A colour the developer chose, or `None` when they left it to the theme.
fn chosen(raw: &str) -> Option<Color32> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let c = paint::parse_color(raw);
    (c.a() > 0).then_some(c)
}

fn faded(c: Color32, alpha_mul: f32) -> Color32 {
    if alpha_mul >= 1.0 {
        return c;
    }
    let m = alpha_mul.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        (c.r() as f32 * m) as u8,
        (c.g() as f32 * m) as u8,
        (c.b() as f32 * m) as u8,
        (c.a() as f32 * m) as u8,
    )
}

fn draw_group_frame(
    painter: &Painter,
    rect: Rect,
    group: &ToolbarGroup,
    theme: &Theme,
    alpha_mul: f32,
) {
    let radius = group.corner_radius.clamp(0, 200) as u8;

    // A group's face is optional: left unset it shows the form through, so a
    // toolbar does not stack two panels on top of each other for no reason. It
    // is drawn only when the developer asked for one, or when the group has a
    // frame to hold together.
    if let Some(face) = chosen(&group.background_color) {
        painter.rect_filled(rect, radius, faded(face, alpha_mul));
    } else if group.draws_frame() {
        painter.rect_filled(rect, radius, faded(theme.card, alpha_mul));
    }

    if !group.draws_frame() {
        return;
    }
    let colour = chosen(&group.border_color).unwrap_or(theme.border);
    let width = group.border_width.clamp(1, 40) as f32;
    painter.rect_stroke(
        rect,
        radius,
        egui::Stroke::new(width, faded(colour, alpha_mul)),
        egui::StrokeKind::Inside,
    );
    // Fixed3D reads as a lip: a lighter top-left inside the outline, the way
    // every other bordered control in the form model draws it.
    if group.border_style.eq_ignore_ascii_case("Fixed3D") {
        let inner = rect.shrink(width);
        painter.line_segment(
            [inner.left_bottom(), inner.left_top()],
            egui::Stroke::new(1.0, faded(theme.card_raised, alpha_mul)),
        );
        painter.line_segment(
            [inner.left_top(), inner.right_top()],
            egui::Stroke::new(1.0, faded(theme.card_raised, alpha_mul)),
        );
    }
}

fn draw_button(
    painter: &Painter,
    rect: Rect,
    button: &ToolbarButton,
    style: &cobolt_forms_style::ResolvedStyle,
    theme: &Theme,
    alpha_mul: f32,
    state: ButtonState,
) {
    // A disabled button is drawn, not hidden — it has to show that it is there
    // and unavailable. Everything about it fades together.
    let alpha_mul = if button.enabled { alpha_mul } else { alpha_mul * 0.45 };
    let radius_px = style.corner_radius.clamp(0, 200);
    let radius = radius_px as u8;

    if style.shadow {
        paint::draw_loose_drop_shadow(
            painter,
            rect,
            chosen(&style.shadow_color).unwrap_or(Color32::BLACK),
            if style.shadow_opacity > 0 { style.shadow_opacity } else { 25 },
            if style.shadow_distance > 0 { style.shadow_distance } else { 2 },
            90.0,
            style.shadow_blur_strength,
            radius_px as f32,
            alpha_mul,
        );
    }

    // The face: a gradient when asked for, otherwise a solid — the developer's
    // colour, or the theme's raised card.
    let base = chosen(&style.background_color).unwrap_or(theme.card_raised);
    if style.gradient {
        let start = chosen(&style.gradient_start_color).unwrap_or(base);
        let end = chosen(&style.gradient_end_color).unwrap_or(base);
        let dir = if style.gradient_direction.trim().is_empty() {
            "Vertical"
        } else {
            style.gradient_direction.trim()
        };
        let mesh = paint::background_gradient_mesh(
            rect,
            faded(lift(start, state), alpha_mul),
            faded(lift(end, state), alpha_mul),
            dir,
            egui::CornerRadius::same(radius),
        );
        painter.add(egui::Shape::mesh(mesh));
    } else {
        painter.rect_filled(rect, radius, faded(lift(base, state), alpha_mul));
    }

    // Icon and label share the button: an icon-only button centres its icon, a
    // label-only button centres its text, and a button with both puts the icon
    // on the left of the text.
    let ink = chosen(&style.foreground_color).unwrap_or(if button.enabled {
        theme.text
    } else {
        theme.dim_text
    });
    let ink = faded(ink, alpha_mul);
    let icon_ink = faded(chosen(&style.icon_color).unwrap_or(ink), alpha_mul);

    // A button carries a label OR an icon, never both — the model keeps that
    // true, and the icon wins here if an old definition somehow holds both.
    let has_icon = !button.icon.trim().is_empty();
    let label = if has_icon { "" } else { button.label.trim() };
    let icon_side = (style.icon_size.clamp(4, 512) as f32).min(rect.height() - 2.0);

    let galley = (!label.is_empty()).then(|| {
        painter.layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional((rect.height() * 0.42).clamp(8.0, 18.0)),
            ink,
        )
    });

    let content_w = match (has_icon, &galley) {
        (true, Some(g)) => icon_side + 4.0 + g.size().x,
        (true, None) => icon_side,
        (false, Some(g)) => g.size().x,
        (false, None) => 0.0,
    };
    let mut x = rect.center().x - content_w / 2.0;

    if has_icon {
        let icon_rect = Rect::from_min_size(
            egui::pos2(x, rect.center().y - icon_side / 2.0),
            Vec2::splat(icon_side),
        );
        crate::icons::draw_menu_icon(painter, icon_rect, button.icon.trim(), icon_ink);
        x += icon_side + 4.0;
    }
    if let Some(g) = galley {
        let y = rect.center().y - g.size().y / 2.0;
        painter.galley(egui::pos2(x, y), g, ink);
    }
}

/// A hovered face lifts, a held one sinks — the feedback a button owes a press.
fn lift(c: Color32, state: ButtonState) -> Color32 {
    let scale = match state {
        ButtonState::Idle => return c,
        ButtonState::Hovered => 1.18,
        ButtonState::Pressed => 0.86,
    };
    let ch = |v: u8| ((v as f32 * scale).clamp(0.0, 255.0)) as u8;
    Color32::from_rgba_premultiplied(ch(c.r()), ch(c.g()), ch(c.b()), c.a())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolbar::{ToolbarButton, ToolbarGroup};

    /// Every button the layout places is drawn and reported, at the rect the
    /// layout chose — the contract the running host hit-tests against.
    #[test]
    fn every_button_is_drawn_and_reported_where_the_layout_put_it() {
        let mut def = ToolbarDef::default();
        let mut g = ToolbarGroup::new("group-1", "Clipboard");
        // Labelled buttons, no icons — a button carries one or the other.
        for (n, label) in ["Copy", "Cut", "Paste"].iter().enumerate() {
            g.buttons
                .push(ToolbarButton::new(format!("b{}", n + 1), *label));
        }
        g.separator_after = true;
        def.groups.push(g);
        let mut g2 = ToolbarGroup::new("group-2", "Output");
        let mut disabled = ToolbarButton::new("b4", "Print");
        disabled.enabled = false;
        disabled.style.shadow = Some(true);
        g2.buttons.push(disabled);
        let mut gradient = ToolbarButton::new("b5", "");
        gradient.set_icon("chart-bar");
        gradient.style.gradient = Some(true);
        gradient.style.gradient_start_color = "#204080".into();
        gradient.style.gradient_end_color = "#4080C0".into();
        g2.buttons.push(gradient);
        def.groups.push(g2);

        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(
            egui::Pos2::ZERO,
            Vec2::new(600.0, 200.0),
        ));
        let bar = Rect::from_min_size(egui::pos2(10.0, 20.0), Vec2::new(400.0, 40.0));
        let mut placed = Vec::new();
        let mut full = ctx.run_ui(input, |ui| {
            placed = draw(
                ui.painter(),
                bar,
                &def,
                1.0,
                Interaction {
                    hovered: Some("b2"),
                    pressed: Some("b3"),
                },
            );
        });
        full.textures_delta.clear();

        assert_eq!(
            placed.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            vec!["b1", "b2", "b3", "b4", "b5"],
            "every button is reported, in draw order, disabled ones included"
        );
        for (id, r) in &placed {
            assert!(
                bar.contains_rect(*r),
                "{id} was placed outside the bar: {r:?} not in {bar:?}"
            );
        }
        // The reported rects agree with the model-side layout, offset to screen.
        let model = def.layout(400, 40);
        let first = model[0].buttons[0].1;
        assert_eq!(placed[0].1.min.x, bar.min.x + first.x as f32);
        assert_eq!(placed[0].1.width(), first.w as f32);

        // Something was actually painted for each: text, icon paths and faces.
        let mut texts = 0usize;
        let mut shapes = 0usize;
        fn walk(s: &egui::Shape, texts: &mut usize, shapes: &mut usize) {
            *shapes += 1;
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, texts, shapes)),
                egui::Shape::Text(_) => *texts += 1,
                _ => {}
            }
        }
        for cs in &full.shapes {
            walk(&cs.shape, &mut texts, &mut shapes);
        }
        assert!(
            texts >= 4,
            "the four labelled buttons must have drawn their text, got {texts}"
        );
        assert!(shapes > 10, "faces, frames and icons must have been drawn");

        println!(
            "\n  Toolbar paint — 2 groups, 5 buttons (1 disabled, 1 gradient+icon-only, \
             1 hovered, 1 pressed) drawn into a 400x40 bar: all 5 reported inside it, \
             rects agree with the model layout, {texts} labels and {shapes} shapes painted\n"
        );
    }

    /// An unset colour asks the theme; a set one is obeyed. The rule the whole
    /// module runs on.
    #[test]
    fn an_unset_colour_defers_to_the_theme_and_a_set_one_wins() {
        assert_eq!(chosen(""), None);
        assert_eq!(chosen("   "), None);
        // A fully transparent colour is not a choice either — it would paint
        // nothing and read as a bug rather than as a decision.
        assert_eq!(chosen("#00000000"), None);
        assert_eq!(chosen("#FF8800"), Some(Color32::from_rgb(0xFF, 0x88, 0x00)));

        // Hover lifts, press sinks, idle is untouched.
        let base = Color32::from_rgb(100, 100, 100);
        assert_eq!(lift(base, ButtonState::Idle), base);
        assert!(lift(base, ButtonState::Hovered).r() > base.r());
        assert!(lift(base, ButtonState::Pressed).r() < base.r());

        println!(
            "\n  Toolbar colours — empty and transparent defer to the theme, \
             #FF8800 is obeyed; hover lifts 100⇒{}, press sinks 100⇒{}\n",
            lift(base, ButtonState::Hovered).r(),
            lift(base, ButtonState::Pressed).r()
        );
    }
}
