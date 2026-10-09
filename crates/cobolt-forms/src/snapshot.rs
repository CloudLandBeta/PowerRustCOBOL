// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A picture of a form, as Run Form draws it** (spec 084 R30, T15).
//!
//! The theme is resolved as `rcrun run-form` resolves it, the backdrop built as
//! the host builds it, a responsive form laid out at its designed size, and the
//! controls drawn by the one render engine (`render::render_form`) with every
//! designed property live — then [`crate::raster`] turns the frame into
//! pixels. No window, no GPU, no interpreter: the form as it opens, before any
//! event handler has run.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Color32, Vec2};

use crate::containers::ActiveTabs;
use crate::render::{merge_props, Backdrop, FormState, RenderInput, RenderMode};
use crate::{Control, Form};

/// How to picture a form.
#[derive(Debug, Clone)]
pub struct SnapshotOptions {
    /// The project's default form theme (`[forms] theme`), if any.
    pub theme_default: Option<String>,
    /// Where theme packs live (`assets/themes` beside the IDE or `rcrun`).
    pub themes_dir: Option<PathBuf>,
    /// Picture scale: 1.0 = one pixel per designed point. Clamped to 0.25–3.
    pub scale: f32,
    /// The window to picture the form in, inner size in points; `None` is
    /// the designed size. A responsive form is laid out for it, never below
    /// the form's minimum — as a running window cannot be made smaller.
    pub window: Option<(f32, f32)>,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self { theme_default: None, themes_dir: None, scale: 1.0, window: None }
    }
}

/// The window `form` is pictured in: the one asked for, held at the form's
/// minimum when it lays out, or the designed size.
pub fn picture_window(form: &Form, asked: Option<(f32, f32)>) -> Vec2 {
    let designed = Vec2::new(form.width.max(1) as f32, form.height.max(1) as f32);
    let Some((w, h)) = asked else { return designed };
    let floor = crate::layout::min_size_of(form).unwrap_or((1.0, 1.0));
    Vec2::new(w.max(floor.0).max(1.0).round(), h.max(floor.1).max(1.0).round())
}

/// The run host's merge: every designed property, stringified, live.
struct Stringified;
impl FormState for Stringified {
    fn live(&self, base: &Control) -> Control {
        let props: HashMap<String, String> = base.properties.iter().map(|(k, v)| (k.clone(), v.to_xml_string())).collect();
        merge_props(base, props.iter())
    }
}

/// What a see-through theme's window is pictured over: there is no desktop
/// behind a picture, and the operating system's blur makes any wallpaper a
/// soft, dim field, which this stands in for. White text on Spatial glass —
/// the theme's whole look — reads on it as it does on a desktop; over the
/// transparent pixels a viewer shows as white, it vanished.
pub const STAND_IN_DESKTOP: Color32 = crate::paint::SEE_THROUGH_DESKTOP;

/// Install the form's theme on `ctx`; returns the glass a see-through theme
/// (Spatial) paints the window with, as the run host does.
fn install_theme(ctx: &egui::Context, form: &Form, opts: &SnapshotOptions) -> Option<Color32> {
    let theme_id = crate::theme::resolve_theme_id(form.theme.as_deref(), opts.theme_default.as_deref());
    let pack = if crate::theme::ThemeCatalog::procedural_ids().contains(&theme_id.as_str()) {
        None
    } else {
        opts.themes_dir
            .as_deref()
            .map(crate::theme_pack::discover_packs)
            .unwrap_or_default()
            .into_iter()
            .find(|p| p.id == theme_id)
            .map(Arc::new)
    };
    let surface = match pack.as_ref() {
        Some(p) => crate::surface_theme::for_pack(p.manifest.self_contained),
        None => crate::surface_theme::for_theme_id(&theme_id),
    };
    crate::paint::set_active_theme(ctx, pack);
    crate::paint::set_glass_style(ctx, form.glass_style);
    crate::paint::set_surface_theme(ctx, surface.clone());
    surface.install_widget_visuals(ctx);
    surface
        .see_through()
        .then(|| surface.token(crate::surface_theme::ColorToken::FormBackground))
        .flatten()
}

/// The window's backdrop, as the run host builds it: under a see-through
/// theme it is the theme's `glass`, and the form's own colour, gradient and
/// picture are set aside (they would hide the desktop the theme shows).
fn backdrop(form: &Form, image: Option<(egui::TextureId, Vec2)>, window: Vec2, glass: Option<Color32>) -> Backdrop {
    let (image, color_hex) = match glass {
        Some(g) => {
            let [r, gg, b, a] = g.to_srgba_unmultiplied();
            (None, format!("#{r:02X}{gg:02X}{b:02X}{a:02X}"))
        }
        None => (image, form.background_color.clone()),
    };
    Backdrop {
        paint: true,
        color_hex,
        transparency: form.transparency.clamp(0, 100) as u8,
        gradient_enabled: form.background_gradient_enabled && glass.is_none(),
        gradient_start_hex: form.background_gradient_start_color.clone(),
        gradient_end_hex: form.background_gradient_end_color.clone(),
        gradient_direction: form.background_gradient_direction.clone(),
        image,
        image_mode: form.bg_image_mode,
        use_theme_background: form.use_theme_background,
        window_size: Some(window),
        behind_fill: None,
        image_extent: None,
        draggable: false,
        // The picture is of the window, so a rounded one is pictured rounded.
        window: crate::render::form_window_arc(form, egui::Rect::from_min_size(egui::Pos2::ZERO, window)),
        // A picture shows every layer, stacked as they are — its state answers
        // "shown" to all of them — so a reader of the picture sees what each
        // holds. A running form starts with them all hidden (spec 091 R35).
        layers: form.layers.clone(),
    }
}

/// Picture `form` in its window ([`picture_window`]). Returns the image.
pub fn render_form_image(form: &Form, opts: &SnapshotOptions) -> egui::ColorImage {
    let designed = Vec2::new(form.width.max(1) as f32, form.height.max(1) as f32);
    let window = picture_window(form, opts.window);
    let ctx = egui::Context::default();
    ctx.set_fonts(crate::fonts::base_font_definitions());
    ctx.set_zoom_factor(opts.scale.clamp(0.25, 3.0));
    let glass = install_theme(&ctx, form, opts);
    let mut raster = crate::raster::Rasterizer::new();
    let mut image_tex: Option<Option<egui::TextureHandle>> = None;
    let active = ActiveTabs::new();
    let laid = crate::layout::apply::LaidOutState { inner: &Stringified };
    let state: &dyn FormState = if form.lays_out() { &laid } else { &Stringified };
    let mut picture = egui::ColorImage::filled([1, 1], Color32::TRANSPARENT);
    // Three frames: textures and fonts settle on the first, sizes on the
    // second; the third is the picture.
    for (i, time) in [0.0, 0.5, 1.0].into_iter().enumerate() {
        let draw = |root: &mut egui::Ui| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                let tex = image_tex.get_or_insert_with(|| {
                    if form.background_image.trim().is_empty() {
                        None
                    } else {
                        crate::paint::load_image_texture(ui.ctx(), &form.background_image)
                    }
                });
                let image = tex.as_ref().map(|t| (t.id(), t.size_vec2()));
                let mut controls = form.controls.clone();
                let mut form_size = window;
                if form.lays_out() {
                    let spec = crate::layout::apply::FormSpec {
                        designed_size: (designed.x, designed.y),
                        layout: &form.layout,
                        breakpoints: &form.breakpoints,
                        system_text_factor: 1.0,
                        pinned_breakpoint: None,
                        pinned_font_scale: None,
                    };
                    let p = crate::layout::apply::prepare_with_rail(ui.ctx(), &form.controls, &Stringified, &spec, window, None);
                    controls = p.controls;
                    form_size = p.form_size;
                }
                let inp = RenderInput {
                    controls: &controls,
                    state,
                    form_size,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: backdrop(form, image, window, glass),
                };
                crate::render::render_form(ui, &inp);
            });
        };
        if i < 2 {
            crate::raster::advance_frame(&ctx, &mut raster, window, time, draw);
        } else {
            // A see-through window is pictured over a stand-in desktop.
            let behind = if glass.is_some() { STAND_IN_DESKTOP } else { Color32::TRANSPARENT };
            picture = crate::raster::render_frame(&ctx, &mut raster, window, behind, time, draw);
        }
    }
    picture
}

/// Picture the form saved at `cfrm`, as PNG bytes, with `project` as the
/// folder its assets are resolved against. Returns `(png, [width, height])`.
pub fn render_form_png(cfrm: &Path, project: &Path, opts: &SnapshotOptions) -> Result<(Vec<u8>, [usize; 2]), String> {
    let form = crate::load_form(cfrm).map_err(|e| format!("the form does not load: {e}"))?;
    crate::assets::set_base(project);
    let image = render_form_image(&form, opts);
    let png = crate::raster::to_png(&image)?;
    Ok((png, image.size))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Spatial form is pictured as Run Form shows it: the window is the
    /// theme's glass over a desktop — a stand-in for the one the operating
    /// system blurs — and not the form's own solid colour. It used to come out
    /// opaque white, so every white caption the theme draws was invisible.
    #[test]
    fn a_see_through_form_is_pictured_over_a_stand_in_desktop() {
        let mut form = Form::new("F", "F", 200, 120);
        form.background_color = "#FFFFFF".into();
        let px = |img: &egui::ColorImage| img.pixels[60 * img.size[0] + 100];
        let plain = render_form_image(&form, &SnapshotOptions::default());
        let spatial = render_form_image(
            &form,
            &SnapshotOptions { theme_default: Some(crate::theme::SPATIAL.into()), ..Default::default() },
        );
        assert_eq!(px(&plain).to_srgba_unmultiplied()[..3], [255, 255, 255], "Liquid Glass keeps the form's colour");
        let [r, g, b, a] = px(&spatial).to_srgba_unmultiplied();
        assert_eq!(a, 255, "opaque: the stand-in desktop is behind the glass");
        assert!(r < 160 && g < 160 && b < 160, "glass over a dim desktop, not the form's white: {:?}", [r, g, b]);
    }

    /// A responsive form pictured in a bigger window is laid out for it, and
    /// one asked smaller than its minimum is held there, as a running window
    /// is; without a window it is pictured at its designed size.
    #[test]
    fn a_form_pictured_in_another_window_is_laid_out_for_it() {
        use crate::model::{ControlType, PropValue};
        let mut form = Form::new("F", "F", 400, 300);
        form.responsive = true;
        let mut p = Control::new("P", ControlType::Panel, 0, 0);
        p.rect = crate::model::Rect::new(0, 0, 400, 300);
        p.set_prop("Dock", PropValue::String("Fill".into()));
        p.set_prop("BackgroundColor", PropValue::String("#FF0000".into()));
        p.set_prop("MinWidth", PropValue::Int(300));
        p.set_prop("MinHeight", PropValue::Int(200));
        form.controls.push(p);
        let at = |w, h| render_form_image(&form, &SnapshotOptions { window: Some((w, h)), ..Default::default() });

        let big = at(800.0, 500.0);
        assert_eq!(big.size, [800, 500]);
        let red = |img: &egui::ColorImage, x: usize, y: usize| {
            let c = img.pixels[y * img.size[0] + x];
            c.r() > 200 && c.g() < 60 && c.b() < 60
        };
        assert!(red(&big, 780, 480), "the Fill panel follows the window to its far corner");

        assert_eq!(at(100.0, 100.0).size, [300, 200], "held at the form's minimum");
        assert_eq!(render_form_image(&form, &SnapshotOptions::default()).size, [400, 300], "designed by default");
    }

    /// A PowerDemo3 form pictured: its designed size, not blank, the same
    /// bytes twice (deterministic).
    #[test]
    fn a_powerdemo3_form_pictures_at_its_designed_size_deterministically() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let project = repo.join("examples/PowerDemo3");
        let cfrm = project.join("forms/Common/buttons-form.cfrm");
        let form = crate::load_form(&cfrm).unwrap();
        let opts = SnapshotOptions { themes_dir: Some(repo.join("assets/themes")), ..Default::default() };
        let (png, size) = render_form_png(&cfrm, &project, &opts).unwrap();
        assert_eq!(size, [form.width as usize, form.height as usize]);
        let again = render_form_png(&cfrm, &project, &opts).unwrap().0;
        assert_eq!(png, again, "the same picture twice");
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let distinct: std::collections::HashSet<[u8; 4]> = img.pixels().map(|p| p.0).take(200_000).collect();
        assert!(distinct.len() > 50, "not blank: {} colours", distinct.len());
        std::fs::write(std::env::temp_dir().join("prc-084-buttons-form.png"), &png).unwrap();
        println!("snapshot: buttons-form {}x{}, {} bytes, {} colours, deterministic", size[0], size[1], png.len(), distinct.len());
    }
}
