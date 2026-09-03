// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Default Theme Settings — the per-theme appearance table (spec 016 Q2).
//!
//! Two selectors and a property table, the shape the operator asked for
//! (2026-09-03):
//!
//! ```text
//! [Theme]  [Glass style]        [📥 Import from a form…]
//! Every control          <the theme-owned properties, key/value>
//! Exceptions by type     [control type ▼]  <the same table, for that type>
//! ```
//!
//! **Base plus exceptions**, because that is what the real themes need.
//! Elegance is uniform across every control type; Neumorphic Light is not — a
//! Button is raised with a gradient, a Label has no shadow and a transparent
//! ground, a TextBox is *sunken*. A flat table cannot say "Labels have no
//! shadow"; a full per-type table is forty-three copies of one answer.
//!
//! **Import** is how a theme is authored: style a real form until it looks
//! right, then read the numbers off it ([`ThemeDefaults::from_form`]). The
//! operator's own way of working — "create/adjust themes visually in a form and
//! then apply to the theme".
//!
//! The table is the **source of truth**: the built-in style appliers are what
//! PowerRustCOBOL ships, and an entry here is what this project has decided
//! they are. A value the developer set on a particular control still survives a
//! theme switch — `reset_theme_owned_props` only clears marks a theme could
//! have written.

use crate::i18n::Tr;
use crate::theme::Theme;
use cobolt_forms::model::{ControlType, GlassStyle, PropValue, ThemeDefaults, THEME_OWNED_PROPS};

/// Opening size. From here on the size is whatever the developer dragged it to.
const DEFAULT_W: f32 = 720.0;
const DEFAULT_H: f32 = 560.0;
/// Hard stops for the grip, so the window cannot be dragged into uselessness.
const MIN_W: f32 = 520.0;
const MIN_H: f32 = 320.0;
const MAX_W: f32 = 1600.0;
const MAX_H: f32 = 1400.0;
/// Side of the resize grip, and its inset from the window's border.
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;
/// Width of the property-name column, so both tables line up.
const NAME_W: f32 = 220.0;

/// Which editor a theme-owned property gets.
///
/// Derived from the property's own name rather than listed by hand: every
/// theme-owned key ends in `Color`, is one of three named enums, or is a
/// `Shadow*`/`CornerRadius` number or a `*Enabled` switch. A key added to
/// [`THEME_OWNED_PROPS`] therefore arrives here already classified.
enum Editor {
    Color,
    Bool,
    Int(i64, i64),
    Choice(&'static [&'static str]),
}

const BORDER_STYLES: &[&str] = &["None", "Single", "Fixed3D", "Raised", "Sunken"];
const DIRECTIONS: &[&str] = &[
    "North", "NorthEast", "East", "SouthEast", "South", "SouthWest", "West", "NorthWest",
];

fn editor_for(key: &str) -> Editor {
    match key {
        "BorderStyle" => Editor::Choice(BORDER_STYLES),
        "ShadowDirection" | "BackgroundGradientDirection" => Editor::Choice(DIRECTIONS),
        "CornerRadius" => Editor::Int(0, 200),
        "ShadowOpacity" => Editor::Int(0, 100),
        "ShadowDistance" => Editor::Int(0, 60),
        // Negative is the SUNKEN variant — the shadow goes over the face
        // instead of under it. A range starting at 0 would make the sunken
        // register unreachable from this table while the property pane can
        // still set it.
        "ShadowBlurStrength" => Editor::Int(-20, 20),
        k if k.ends_with("Enabled") || k == "ShadowBlur" => Editor::Bool,
        k if k.ends_with("Color") => Editor::Color,
        _ => Editor::Color,
    }
}

/// The value a fresh row starts at, so a property switched on has something
/// sensible in it rather than an empty string the renderer cannot parse.
fn seed_for(key: &str) -> PropValue {
    match editor_for(key) {
        Editor::Bool => PropValue::Bool(false),
        Editor::Int(lo, _) => PropValue::Int(lo.max(0)),
        Editor::Choice(opts) => PropValue::String(opts[0].to_owned()),
        Editor::Color => PropValue::String("#000000FF".to_owned()),
    }
}

pub struct ThemeDefaultsModal {
    pub open: bool,
    /// The theme id being edited (empty = the project's default theme).
    pub theme: String,
    /// The glass style being edited.
    pub style: GlassStyle,
    /// The table under edit. Committed to the project on every change — there
    /// is no OK/Cancel here, the same as the rest of project settings.
    draft: ThemeDefaults,
    /// The control type whose exceptions are on show; empty = none chosen.
    override_type: String,
    /// The form Import will read, as its project-relative path.
    ///
    /// A picker rather than "whichever form is in front": designer forms live
    /// in their own OS windows, so there is no single active one to mean, and
    /// reading a theme off the wrong form would be silent and wrong.
    import_form: String,

    /// Last import's summary, shown under the button.
    status: Option<String>,
    /// The window's size, owned here rather than by egui.
    ///
    /// This is the whole defence against the self-inflating window: the size
    /// changes **only** when the developer drags the grip. Children are laid
    /// out from this stored number, never from `available_width()` or
    /// `max_rect()`, so nothing a child measures can feed back into the size
    /// and grow it again on the next frame.
    size: egui::Vec2,
}

#[derive(Default)]
pub struct ThemeDefaultsAction {
    /// The table changed: save it under `theme_defaults_key(theme, style)`.
    pub save: Option<(String, GlassStyle, ThemeDefaults)>,
    /// Read the values off this form (project-relative path) — the app owns the
    /// load.
    pub import_from: Option<String>,

}

impl ThemeDefaultsModal {
    pub fn new(theme: String, style: GlassStyle, draft: ThemeDefaults) -> Self {
        Self {
            open: true,
            theme,
            style,
            draft,
            override_type: String::new(),
            import_form: String::new(),
            status: None,

            size: egui::vec2(DEFAULT_W, DEFAULT_H),
        }
    }

    /// Replace the table under edit — after an import, or after the developer
    /// picked a different theme/style pair.
    pub fn set_draft(&mut self, draft: ThemeDefaults) {
        self.draft = draft;
    }

    /// Say how many values an import brought in.
    pub fn note_import(&mut self, tr: &Tr, values: usize) {
        self.status = Some(tr.theme_defaults_imported.replacen("{}", &values.to_string(), 1));
    }

    /// The resize grip, in its own foreground [`egui::Area`] pinned to the
    /// window's **outer** bottom-right corner.
    ///
    /// It lives outside the content on purpose: allocated inside, it would join
    /// the layout and move as the content moved, so a drag would fight the
    /// thing it was sizing. Its drag delta is the one and only writer of
    /// `self.size`.
    fn resize_grip(&mut self, ctx: &egui::Context, theme: &Theme, window: egui::Rect) {
        let corner = window.max - egui::vec2(GRIP_INSET, GRIP_INSET);
        let origin = corner - egui::vec2(GRIP, GRIP);
        egui::Area::new(egui::Id::new("theme_defaults_resize_grip"))
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
                let col = if response.hovered() || response.dragged() {
                    theme.accent
                } else {
                    theme.text_dim
                };
                let p = ui.painter();
                for i in 0..3 {
                    let o = 4.0 * i as f32;
                    p.line_segment(
                        [
                            egui::pos2(rect.right() - o, rect.bottom()),
                            egui::pos2(rect.right(), rect.bottom() - o),
                        ],
                        egui::Stroke::new(1.5, col),
                    );
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                }
            });
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        themes: &[(&'static str, &'static str)],
        // The project's forms, as project-relative paths.
        forms: &[String],

        theme: &Theme,
        tr: &Tr,
    ) -> ThemeDefaultsAction {

        let mut action = ThemeDefaultsAction::default();
        let mut open = self.open;
        let before = self.draft.clone();
        let (before_theme, before_style) = (self.theme.clone(), self.style);

        let window = egui::Window::new(
            egui::RichText::new(tr.theme_defaults_title).size(18.0).strong(),
        )
        .id(egui::Id::new("theme_defaults_modal"))
        .open(&mut open)
        .collapsible(false)
        // NEVER `resizable(true)`. egui then negotiates the window rectangle
        // against its contents every frame, and this window's rows are a scroll
        // area that reports what it would like to be — so the two push each
        // other outward and the window walks to the screen edge on its own.
        // The size is ours, it is exact, and the ONLY thing that changes it is
        // the developer dragging the grip.
        .resizable(false)
        .fixed_size(self.size)
        .default_pos([60.0, 60.0])
        .show(ctx, |ui| {
            // Children are laid out from the STORED size, never from what the
            // Ui offers: a child measured against available space feeds its own
            // width back into the window and the pair inflate together.
            let margin = ui.style().spacing.window_margin.sum().x;
            let inner_w = (self.size.x - margin).max(120.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tr.theme_defaults_theme).color(theme.text_dim));
                let current = themes
                    .iter()
                    .find(|(id, _)| *id == self.theme)
                    .map(|(_, name)| *name)
                    .unwrap_or("—");
                egui::ComboBox::from_id_salt("theme_defaults_theme_pick")
                    .selected_text(current)
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for (id, name) in themes {
                            ui.selectable_value(&mut self.theme, (*id).to_owned(), *name);
                        }
                    });
                ui.add_space(12.0);
                ui.label(egui::RichText::new(tr.theme_defaults_style).color(theme.text_dim));
                egui::ComboBox::from_id_salt("theme_defaults_style_pick")
                    .selected_text(self.style.as_str())
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for s in [
                            GlassStyle::Classic,
                            GlassStyle::Enhanced,
                            GlassStyle::Neumorphic,
                            GlassStyle::NeumorphicDark,
                        ] {
                            ui.selectable_value(&mut self.style, s, s.as_str());
                        }
                    });
            });

            ui.add_space(6.0);
            if self.import_form.is_empty() {
                if let Some(first) = forms.first() {
                    self.import_form = first.clone();
                }
            }
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("theme_defaults_import_form")
                    .selected_text(if self.import_form.is_empty() {
                        "—"
                    } else {
                        self.import_form.as_str()
                    })
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for f in forms {
                            ui.selectable_value(&mut self.import_form, f.clone(), f);
                        }
                    });
                let can = !self.import_form.is_empty();
                if ui
                    .add_enabled(can, egui::Button::new(tr.theme_defaults_import))
                    .clicked()
                {
                    action.import_from = Some(self.import_form.clone());
                }
            });
            ui.label(
                egui::RichText::new(tr.theme_defaults_import_hint)
                    .size(12.0)
                    .color(theme.text_dim),
            );

            if let Some(s) = &self.status {
                ui.label(egui::RichText::new(s).size(12.0).color(theme.accent));
            }
            ui.add_space(4.0);
            ui.separator();

            if self.draft.is_empty() {
                ui.label(
                    egui::RichText::new(tr.theme_defaults_empty)
                        .size(12.0)
                        .color(theme.text_dim),
                );
            }

            // The rows scroll; everything above is chrome and stays put. The
            // height is the stored one minus the chrome, never `available_*`.
            let rows_h = (self.size.y - 210.0).max(120.0);
            egui::ScrollArea::vertical()
                .id_salt("theme_defaults_rows")
                .max_height(rows_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(inner_w);
                    ui.label(
                        egui::RichText::new(tr.theme_defaults_base)
                            .strong()
                            .color(theme.text_bright),
                    );
                    for key in THEME_OWNED_PROPS {
                        property_row(ui, theme, tr, key, &mut self.draft.base, None);
                    }

                    ui.add_space(10.0);
                    ui.separator();
                    ui.label(
                        egui::RichText::new(tr.theme_defaults_overrides)
                            .strong()
                            .color(theme.text_bright),
                    );
                    ui.horizontal(|ui| {
                        let shown = if self.override_type.is_empty() {
                            tr.theme_defaults_no_override
                        } else {
                            self.override_type.as_str()
                        };
                        egui::ComboBox::from_id_salt("theme_defaults_override_type")
                            .selected_text(shown)
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.override_type,
                                    String::new(),
                                    tr.theme_defaults_no_override,
                                );
                                // Only controls that paint a face: a theme has
                                // nothing to say about a Timer.
                                for ct in ControlType::ALL.iter() {
                                    if ct.is_non_visual() || matches!(ct, ControlType::Line) {
                                        continue;
                                    }
                                    ui.selectable_value(
                                        &mut self.override_type,
                                        ct.as_str().to_owned(),
                                        ct.as_str(),
                                    );
                                }
                            });
                    });
                    if !self.override_type.is_empty() {
                        let base = self.draft.base.clone();
                        let entry = self
                            .draft
                            .overrides
                            .entry(self.override_type.clone())
                            .or_default();
                        for key in THEME_OWNED_PROPS {
                            property_row(ui, theme, tr, key, entry, Some(&base));
                        }
                        if entry.is_empty() {
                            self.draft.overrides.remove(&self.override_type);
                        }
                    }
                });

            // The grip is pinned to the window's own rect, so it is placed
            // after the body has been laid out.
            ui.add_space(2.0);
        });

        if let Some(w) = &window {
            self.resize_grip(ctx, theme, w.response.rect);
        }
        self.open = open;

        // Saved on change rather than behind an OK button — project settings
        // elsewhere in the IDE behave the same way, and a table the developer
        // edited and lost to a closed window would be worse than either.
        if self.draft != before || self.theme != before_theme || self.style != before_style {
            action.save = Some((self.theme.clone(), self.style, self.draft.clone()));
        }
        action
    }
}

/// One property row: name, editor, and a Clear that takes the property out of
/// the table so the shipped style answers for it again.
///
/// `inherited` is the base table when this row belongs to a control-type
/// override — an override that says nothing shows what it would inherit, so the
/// developer can see the value they are about to make an exception to.
fn property_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    tr: &Tr,
    key: &str,
    table: &mut std::collections::BTreeMap<String, PropValue>,
    inherited: Option<&std::collections::BTreeMap<String, PropValue>>,
) {
    ui.horizontal(|ui| {
        let present = table.contains_key(key);
        let mut on = present;
        if ui.checkbox(&mut on, "").changed() {
            if on {
                let seed = inherited
                    .and_then(|b| b.get(key).cloned())
                    .unwrap_or_else(|| seed_for(key));
                table.insert(key.to_owned(), seed);
            } else {
                table.remove(key);
            }
        }
        let name_col = if present { theme.text_bright } else { theme.text_dim };
        ui.add_sized(
            [NAME_W, 18.0],
            egui::Label::new(egui::RichText::new(key).color(name_col)).truncate(),
        );
        let Some(value) = table.get_mut(key) else {
            // Not in the table: show what answers instead, greyed.
            let shown = inherited
                .and_then(|b| b.get(key))
                .map(value_text)
                .unwrap_or_else(|| "—".to_owned());
            ui.label(egui::RichText::new(shown).size(12.0).color(theme.text_dim));
            return;
        };
        match editor_for(key) {
            Editor::Bool => {
                let mut b = matches!(value, PropValue::Bool(true));
                if ui.checkbox(&mut b, "").changed() {
                    *value = PropValue::Bool(b);
                }
            }
            Editor::Int(lo, hi) => {
                let mut n = match &*value {
                    PropValue::Int(i) => *i,
                    other => other.as_str().parse().unwrap_or(0),
                };

                if ui.add(egui::DragValue::new(&mut n).range(lo..=hi)).changed() {
                    *value = PropValue::Int(n);
                }
            }
            Editor::Choice(opts) => {
                let cur = value.as_str().to_owned();
                egui::ComboBox::from_id_salt(("theme_defaults_choice", key))
                    .selected_text(&cur)
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        for o in opts {
                            if ui.selectable_label(cur == *o, *o).clicked() {
                                *value = PropValue::String((*o).to_owned());
                            }
                        }
                    });
            }
            Editor::Color => {
                let mut text = value.as_str().to_owned();
                if ui
                    .add(egui::TextEdit::singleline(&mut text).desired_width(120.0))
                    .changed()
                {
                    *value = PropValue::String(text.clone());
                }
                // A swatch of what that hex actually is, so a typo is visible
                // without running the form.
                let c = cobolt_forms::paint::parse_color(&text);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 16.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 3.0, c);
                ui.painter().rect_stroke(
                    rect,
                    3.0,
                    egui::Stroke::new(1.0, theme.line()),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if present && ui.small_button(tr.theme_defaults_clear).clicked() {
            table.remove(key);
        }
    });
}

/// A value as the greyed inherited column shows it.
fn value_text(v: &PropValue) -> String {
    match v {
        PropValue::String(s) => s.clone(),
        PropValue::Int(i) => i.to_string(),
        PropValue::Bool(b) => b.to_string(),
    }
}
