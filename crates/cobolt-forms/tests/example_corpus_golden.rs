// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

#![cfg(feature = "render")]
//! **Spec 056 §4.16 — the golden of the example corpus (R79, R82; AC2, AC39).**
//!
//! Every `.cfrm` of `examples/PowerDemo3/forms` and `examples/PowerChat/forms`
//! (backup copies excluded) is rendered headlessly on the three engine surfaces
//! — the designer canvas (`render_faces`), the run form (`render_form` with the
//! host's stringified live state) and the preview (`render_form_with_chrome`)
//! — at 0.75×, 1× and 1.5× its designed size (never below 64 px). The second of
//! two frames is recorded: each control's rect and font size, and a digest of
//! every painted shape.
//!
//! The golden was captured BEFORE the first responsive-layout change to the
//! engine; any later difference is a regression unless the operator accepts it
//! as a named, intended change. `COBOLT_WRITE_GOLDEN=1` rewrites the files.
//!
//! Text shapes are recorded by font size, family, colour and a hash of their
//! string — never by width, which depends on the fonts this machine has
//! installed. Map tile meshes are left out (they arrive from the network).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::paint::ctrl_font_size;
use cobolt_forms::render::{
    merge_props, render_faces, render_form, render_form_with_chrome, Backdrop, DesignedState,
    FormState, RenderInput, RenderMode,
};
use cobolt_forms::{Control, ControlType, Form};
use egui::{pos2, Color32, Rect, Vec2};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const PROJECTS: [&str; 2] = ["PowerDemo3", "PowerChat"];
const FACTORS: [(f32, &str); 3] = [(0.75, "0.75x"), (1.0, "1x"), (1.5, "1.5x")];

#[derive(Clone, Copy, Debug)]
enum Surface {
    Canvas,
    Run,
    Preview,
}
const SURFACES: [Surface; 3] = [Surface::Canvas, Surface::Run, Surface::Preview];

/// The run-form host's merge: every designed prop stringified and merged back.
struct Stringified;
impl FormState for Stringified {
    fn live(&self, base: &Control) -> Control {
        let props: HashMap<String, String> = base
            .properties
            .iter()
            .map(|(k, v)| (k.clone(), v.to_xml_string()))
            .collect();
        merge_props(base, props.iter())
    }
}

/// Every `.cfrm` under `dir`, sorted by path; `.bak`/`.backup` copies are not
/// `.cfrm` files and so never match.
fn forms_in(dir: &Path) -> Vec<PathBuf> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("cfrm") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

fn r2(v: f32) -> f32 {
    (v * 4.0).round() / 4.0
}
fn fr(r: Rect) -> String {
    format!("[{} {} {} {}]", r2(r.min.x), r2(r.min.y), r2(r.max.x), r2(r.max.y))
}
fn c8(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r(), c.g(), c.b(), c.a())
}
fn fnv64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// One line per painted shape. Text: size/family/colour per section and a
/// hash of the string. Anything else: a hash of the whole shape.
fn dump(out: &mut Vec<String>, shape: &egui::Shape) {
    use egui::Shape as S;
    match shape {
        S::Vec(v) => v.iter().for_each(|s| dump(out, s)),
        S::Noop => {}
        S::Text(t) => {
            let job = &t.galley.job;
            let secs: Vec<String> = job
                .sections
                .iter()
                .map(|s| {
                    format!(
                        "{}/{:?}/{}",
                        r2(s.format.font_id.size),
                        s.format.font_id.family,
                        c8(s.format.color)
                    )
                })
                .collect();
            out.push(format!(
                "TEXT {} str={:016x} fb={}",
                secs.join(","),
                fnv64(&job.text),
                c8(t.fallback_color)
            ));
        }
        S::Mesh(_) => out.push(format!("MESH h={:016x}", fnv64(&format!("{shape:?}")))),
        // A paint callback holds a pointer; only where it paints is stable.
        S::Callback(cb) => out.push(format!("CALLBACK {}", fr(cb.rect))),
        other => out.push(format!("SHAPE h={:016x}", fnv64(&format!("{other:?}")))),
    }
}

/// The theme, resolved as `rcrun run-form` resolves it (`form_gui.rs`), with
/// packs discovered in the repository's `assets/themes`.
fn install_theme(ctx: &egui::Context, form: &Form, project_default: Option<&str>) {
    let theme_id = cobolt_forms::theme::resolve_theme_id(form.theme.as_deref(), project_default);
    let pack = if cobolt_forms::theme::ThemeCatalog::procedural_ids().contains(&theme_id.as_str()) {
        None
    } else {
        cobolt_forms::theme_pack::discover_packs(&repo().join("assets/themes"))
            .into_iter()
            .find(|p| p.id == theme_id)
            .map(Arc::new)
    };
    let surface = match pack.as_ref() {
        Some(p) => cobolt_forms::surface_theme::for_pack(p.manifest.self_contained),
        None => cobolt_forms::surface_theme::for_theme_id(&theme_id),
    };
    cobolt_forms::paint::set_active_theme(ctx, pack);
    cobolt_forms::paint::set_glass_style(ctx, form.glass_style);
    cobolt_forms::paint::set_surface_theme(ctx, surface.clone());
    surface.install_widget_visuals(ctx);
}

/// The form's backdrop as the host builds it (`FormBody::backdrop`).
fn form_backdrop(form: &Form, image: Option<(egui::TextureId, Vec2)>, window: Vec2) -> Backdrop {
    Backdrop {
        paint: true,
        color_hex: form.background_color.clone(),
        transparency: form.transparency.clamp(0, 100) as u8,
        gradient_enabled: form.background_gradient_enabled,
        gradient_start_hex: form.background_gradient_start_color.clone(),
        gradient_end_hex: form.background_gradient_end_color.clone(),
        gradient_direction: form.background_gradient_direction.clone(),
        image,
        image_mode: form.bg_image_mode,
        use_theme_background: form.use_theme_background,
        window_size: Some(window),
        behind_fill: None,
        image_extent: None,
    }
}

/// The controls the canvas and the preview paint: the designed ones with a
/// rail designed collapsed drawn collapsed (`sidebar::rail_view`).
fn rail_applied(controls: &[Control]) -> Vec<Control> {
    match cobolt_forms::breadcrumb::shell_side_menu_in(controls) {
        Some(side) if side.side_menu_collapsed() => {
            cobolt_forms::sidebar::rail_view(controls, side, true)
        }
        _ => controls.to_vec(),
    }
}

/// Render one surface at one size: fresh context, two frames, the second kept.
/// Returns the golden section text and the number of (rect, font) rows.
fn render_one(form: &Form, theme_default: Option<&str>, s: Surface, window: Vec2) -> (String, usize) {
    let designed = Vec2::new(form.width as f32, form.height as f32);
    let responsive = form.responsive;
    // A responsive form's surfaces lay it out first, then narrow the rail
    // (spec 056 R26); a form that is not responsive takes today's path.
    let base_controls: Vec<Control> = form.controls.clone();
    let controls: Vec<Control> = match s {
        Surface::Run => form.controls.clone(),
        _ => rail_applied(&form.controls),
    };
    let ctx = egui::Context::default();
    ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
    install_theme(&ctx, form, theme_default);
    // Loaded inside the first frame, once the context knows the texture
    // limit the input reports (as a real backend's first frame does).
    let mut image_tex: Option<Option<egui::TextureHandle>> = None;
    let active = ActiveTabs::new();
    let inner: &dyn FormState = match s {
        Surface::Run => &Stringified,
        _ => &DesignedState,
    };
    let laid_state = cobolt_forms::layout::apply::LaidOutState { inner };
    let state: &dyn FormState = if responsive { &laid_state } else { inner };
    let mut controls = controls;
    let mut rects: HashMap<String, Rect> = HashMap::new();
    let mut shapes = Vec::new();
    for _frame in 0..2 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), window));
        input.max_texture_side = Some(8192);
        input.time = Some(1.0);
        let mut full = ctx.run_ui(input, |root| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| {
                    let tex = image_tex.get_or_insert_with(|| {
                        if form.background_image.trim().is_empty() {
                            None
                        } else {
                            cobolt_forms::paint::load_image_texture(ui.ctx(), &form.background_image)
                        }
                    });
                    let image = tex.as_ref().map(|t| (t.id(), t.size_vec2()));
                    let mut backdrop = match s {
                        Surface::Canvas => Backdrop::default(),
                        _ => form_backdrop(form, image, window),
                    };
                    if let Surface::Preview = s {
                        backdrop.image_extent = Some(designed);
                    }
                    // The designer rewrites an AutoSize control's designed rect
                    // every frame before it paints (`designer.rs`,
                    // `paint::apply_autosize`); the canvas emulation does too, so
                    // the plain and the responsive canvas start from the same
                    // controls (spec 056 T4.6).
                    let mut base = base_controls.clone();
                    if let Surface::Canvas = s {
                        cobolt_forms::paint::apply_autosize(ui.ctx(), &mut base);
                        if !responsive {
                            controls = rail_applied(&base);
                        }
                    }
                    let mut form_size = designed;
                    if responsive {
                        let spec = cobolt_forms::layout::apply::FormSpec {
                            designed_size: (designed.x, designed.y),
                            layout: &form.layout,
                            breakpoints: &form.breakpoints,
                            system_text_factor: 1.0,
                        };
                        // The canvas and the preview draw a rail designed
                        // collapsed at its collapsed width; the run surface
                        // leaves it to the shell (as `rail_applied`).
                        let side = cobolt_forms::breadcrumb::shell_side_menu_in(&base)
                            .filter(|side| side.side_menu_collapsed())
                            .map(|side| side.id.clone());
                        let rail = match s {
                            Surface::Run => None,
                            _ => side.as_deref().map(|id| (id, true)),
                        };
                        let p = cobolt_forms::layout::apply::prepare_with_rail(ui.ctx(), &base, inner, &spec, window, rail);
                        controls = p.controls;
                        form_size = p.form_size;
                    }
                    let inp = RenderInput {
                        controls: &controls,
                        state,
                        form_size,
                        glass: true,
                        mode: match s {
                            Surface::Canvas => RenderMode::Static,
                            _ => RenderMode::Interactive,
                        },
                        active_tabs: &active,
                        backdrop,
                    };
                    rects = match s {
                        Surface::Canvas => {
                            let painter = ui.painter().clone();
                            render_faces(&painter, ui.min_rect().min, &inp).control_rects
                        }
                        Surface::Run => render_form(ui, &inp).control_rects,
                        Surface::Preview => render_form_with_chrome(ui, &inp, None).control_rects,
                    };
                });
        });
        full.textures_delta.clear();
        shapes = full.shapes;
    }

    let maps: Vec<Rect> = controls
        .iter()
        .filter(|c| c.control_type == ControlType::Maps)
        .filter_map(|c| rects.get(&c.id).copied())
        .collect();
    let mut lines = Vec::new();
    let mut ids: Vec<&Control> = controls.iter().collect();
    ids.sort_by(|a, b| a.id.cmp(&b.id));
    for c in &ids {
        let rect = rects.get(&c.id).map(|r| fr(*r)).unwrap_or_else(|| "-".into());
        let font = ctrl_font_size(&state.live(c));
        lines.push(format!("{} rect={} font={}", c.id, rect, r2(font)));
    }
    let rows = lines.len();
    let mut shape_lines = Vec::new();
    for cs in &shapes {
        // What a Maps control paints clipped to its own rect — tiles (which
        // overhang the rect), and the placeholders shown while they download —
        // depends on the network, and is skipped.
        if maps.iter().any(|r| r.expand(0.5).contains_rect(cs.clip_rect)) {
            continue;
        }
        dump(&mut shape_lines, &cs.shape);
    }
    lines.push(format!(
        "shapes={} digest={:016x}",
        shape_lines.len(),
        fnv64(&shape_lines.join("\n"))
    ));
    (lines.join("\n"), rows)
}

/// `[forms] theme` of the project manifest — the project default a form's own
/// `theme` falls back to.
fn project_theme_default(project_dir: &Path, project: &str) -> Option<String> {
    let text = std::fs::read_to_string(project_dir.join(format!("{project}.project.toml"))).ok()?;
    let v: toml::Value = toml::from_str(&text).ok()?;
    v.get("forms")?.get("theme")?.as_str().map(str::to_owned)
}

#[test]
fn every_example_form_renders_as_its_golden() {
    let started = Instant::now();
    let write = std::env::var("COBOLT_WRITE_GOLDEN").is_ok();
    let golden_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/056_corpus");
    let (mut forms, mut rows, mut renders, mut written) = (0usize, 0usize, 0usize, 0usize);
    let mut differences: Vec<String> = Vec::new();
    let (mut responsive_checked, mut responsive_diffs) = (0usize, Vec::<String>::new());
    for project in PROJECTS {
        let dir = repo().join("examples").join(project);
        // `assets::set_base` is process-global: one test drives the corpus.
        cobolt_forms::assets::set_base(&dir);
        let theme_default = project_theme_default(&dir, project);
        for path in forms_in(&dir.join("forms")) {
            let rel = path.strip_prefix(dir.join("forms")).unwrap();
            let form = cobolt_forms::load_form(&path)
                .unwrap_or_else(|e| panic!("{} must parse: {e}", path.display()));
            forms += 1;
            let mut text = format!("# {project}/{}\n", rel.display());
            for (f, fname) in FACTORS {
                let window = Vec2::new(
                    (form.width as f32 * f).round().max(64.0),
                    (form.height as f32 * f).round().max(64.0),
                );
                for s in SURFACES {
                    let (section, n) = render_one(&form, theme_default.as_deref(), s, window);
                    rows += n;
                    renders += 1;
                    text.push_str(&format!("## {s:?} {fname} {}x{}\n{section}\n", window.x, window.y));
                }
            }
            // R81 / AC38 — the same form with only `responsive="true"` renders
            // at its designed size exactly as the golden does, on every surface.
            {
                let mut copy = form.clone();
                copy.responsive = true;
                let window = Vec2::new(
                    (form.width as f32).round().max(64.0),
                    (form.height as f32).round().max(64.0),
                );
                for s in SURFACES {
                    let header = format!("## {s:?} 1x {}x{}\n", window.x, window.y);
                    let (section, _) = render_one(&copy, theme_default.as_deref(), s, window);
                    let plain = text
                        .split(&header)
                        .nth(1)
                        .and_then(|rest| rest.split("\n## ").next())
                        .unwrap_or_default()
                        .trim_end()
                        .to_owned();
                    responsive_checked += 1;
                    if plain != section {
                        let first = plain.lines().zip(section.lines()).find(|(a, b)| a != b);
                        responsive_diffs.push(format!("{project}/{} {s:?}: {first:?}", rel.display()));
                    }
                }
            }
            let file = golden_root
                .join(project)
                .join(format!("{}.txt", rel.display().to_string().replace(['/', '\\'], "__")));
            if write {
                std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                std::fs::write(&file, &text).unwrap();
                written += 1;
                continue;
            }
            let expected = std::fs::read_to_string(&file).unwrap_or_else(|_| {
                panic!("golden {} missing — capture it with COBOLT_WRITE_GOLDEN=1", file.display())
            });
            if expected != text {
                let e: Vec<_> = expected.lines().collect();
                let g: Vec<_> = text.lines().collect();
                let differing = e.iter().zip(g.iter()).filter(|(a, b)| a != b).count()
                    + e.len().abs_diff(g.len());
                let first = e
                    .iter()
                    .zip(g.iter())
                    .position(|(a, b)| a != b)
                    .unwrap_or(e.len().min(g.len()));
                differences.push(format!(
                    "{project}/{}: {differing} line(s) differ; first at line {}:\n    expected: {}\n    got:      {}",
                    rel.display(),
                    first + 1,
                    e.get(first).unwrap_or(&"<end>"),
                    g.get(first).unwrap_or(&"<end>")
                ));
            }
        }
    }
    println!("── 056 example-corpus golden ─────────────────────────────");
    println!("  projects   : {}", PROJECTS.join(", "));
    println!("  forms      : {forms}");
    println!("  sizes      : {}", FACTORS.map(|f| f.1).join(", "));
    println!("  surfaces   : canvas (render_faces), run (render_form), preview (render_form_with_chrome)");
    println!("  renders    : {renders} (two frames each, second recorded)");
    println!("  rects+fonts: {rows} control rows compared");
    if write {
        println!("  WROTE      : {written} golden files under {}", golden_root.display());
    } else {
        println!("  differences: {} form(s)", differences.len());
    }
    println!("  responsive : {responsive_checked} designed-size renders with responsive=\"true\", {} differing from the plain form", responsive_diffs.len());
    println!("  elapsed    : {:.1} s", started.elapsed().as_secs_f32());
    assert!(
        differences.is_empty(),
        "the example corpus no longer renders as its golden:\n{}",
        differences.join("\n")
    );
    assert!(
        responsive_diffs.is_empty(),
        "R81 — turning Responsive design on moved something at the designed size:\n{}",
        responsive_diffs.join("\n")
    );
}
