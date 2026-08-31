#![cfg(feature = "render")]
//! Background-image MODE coverage (operator, 2026-08-31): "tiled working as
//! stretched, fit working as stretched". `image_dest` implemented Fill / Fit /
//! Center and let TILE fall through to the same `_ => area` arm STRETCH takes,
//! so a tiled background was one stretched copy and the mode did nothing.
//!
//! The witness is the number of image quads the backdrop paints and where they
//! land — a stretched background is ONE quad covering the area, a tiled one is
//! a grid of quads at the image's own size.

use cobolt_forms::model::BgImageMode;
use cobolt_forms::render::{paint_backdrop, Backdrop};
use egui::{Rect, Vec2};

/// Every textured quad the backdrop painted, as its destination rect.
fn image_quads(mode: BgImageMode, area: Vec2, tsize: Vec2) -> Vec<Rect> {
    let ctx = egui::Context::default();
    let tex = egui::TextureId::Managed(1);
    let mut quads = Vec::new();
    let mut out = ctx.run_ui(Default::default(), |ui| {
        let painter = ui.painter().clone();
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), area);
        paint_backdrop(
            &painter,
            rect,
            &Backdrop {
                paint: true,
                color_hex: "#101010".into(),
                image: Some((tex, tsize)),
                image_mode: mode,
                ..Default::default()
            },
        );
    });
    out.textures_delta.clear();
    fn walk(s: &egui::Shape, out: &mut Vec<Rect>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Mesh(m) if m.texture_id == egui::TextureId::Managed(1) => {
                out.push(m.calc_bounds())
            }
            egui::Shape::Rect(r) if r.brush.is_some() => out.push(r.rect),
            _ => {}
        }
    }
    for cs in &out.shapes {
        walk(&cs.shape, &mut quads);
    }
    quads
}

#[test]
fn tile_repeats_the_image_instead_of_stretching_it() {
    let area = Vec2::new(400.0, 300.0);
    let tsize = Vec2::new(100.0, 100.0);

    // STRETCH: one quad, the whole area.
    let stretched = image_quads(BgImageMode::Stretch, area, tsize);
    assert_eq!(stretched.len(), 1, "stretch paints one quad: {stretched:?}");
    assert!(
        (stretched[0].width() - 400.0).abs() < 0.5
            && (stretched[0].height() - 300.0).abs() < 0.5,
        "stretch covers the whole area, got {:?}",
        stretched[0]
    );

    // TILE: a 4x3 grid of quads at the IMAGE's own size — never one big one.
    let tiled = image_quads(BgImageMode::Tile, area, tsize);
    assert_eq!(
        tiled.len(),
        12,
        "a 400x300 area tiled with a 100x100 image is 4x3 = 12 quads, got {}",
        tiled.len()
    );
    for q in &tiled {
        assert!(
            (q.width() - 100.0).abs() < 0.5 && (q.height() - 100.0).abs() < 0.5,
            "every tile is drawn at the image's own size, got {q:?}"
        );
    }
    // The grid is anchored at the area's origin and steps by the image size.
    let mut origins: Vec<(i32, i32)> = tiled
        .iter()
        .map(|q| (q.min.x.round() as i32, q.min.y.round() as i32))
        .collect();
    origins.sort();
    let mut expected: Vec<(i32, i32)> = (0..3)
        .flat_map(|j| (0..4).map(move |i| (i * 100, j * 100)))
        .collect();
    expected.sort();
    assert_eq!(origins, expected, "tiles step by the image size from the origin");

    // …and TILE is genuinely a different picture from STRETCH — the bug was
    // that these two produced identical output.
    assert_ne!(
        tiled.len(),
        stretched.len(),
        "tile must not be stretch (the reported defect)"
    );

    println!(
        "\n  background image modes — area 400x300, image 100x100:\n\
         \x20   Stretch : 1 quad  400x300 (covers the area)\n\
         \x20   Tile    : {} quads 100x100 at (0,0)…(300,200) — repeats, no stretch\n",
        tiled.len()
    );
}

/// Fit / Fill / Center still scale and place against the backdrop area — the
/// tiling branch must not have disturbed them.
#[test]
fn fit_fill_and_center_keep_their_geometry() {
    let area = Vec2::new(400.0, 300.0);
    let tsize = Vec2::new(200.0, 100.0); // 2:1

    let fit = image_quads(BgImageMode::Fit, area, tsize);
    assert_eq!(fit.len(), 1);
    // Fit: scale = min(400/200, 300/100) = 2 → 400x200, centred vertically.
    assert!(
        (fit[0].width() - 400.0).abs() < 0.5 && (fit[0].height() - 200.0).abs() < 0.5,
        "Fit scales to the smaller ratio preserving aspect, got {:?}",
        fit[0]
    );

    let fill = image_quads(BgImageMode::Fill, area, tsize);
    assert_eq!(fill.len(), 1);
    // Fill: scale = max(2, 3) = 3 → 600x300.
    assert!(
        (fill[0].width() - 600.0).abs() < 0.5 && (fill[0].height() - 300.0).abs() < 0.5,
        "Fill scales to the larger ratio, got {:?}",
        fill[0]
    );

    let center = image_quads(BgImageMode::Center, area, tsize);
    assert_eq!(center.len(), 1);
    assert!(
        (center[0].width() - 200.0).abs() < 0.5
            && (center[0].height() - 100.0).abs() < 0.5
            && (center[0].min.x - 100.0).abs() < 0.5
            && (center[0].min.y - 100.0).abs() < 0.5,
        "Center draws at native size in the middle, got {:?}",
        center[0]
    );

    println!(
        "  Fit 400x200 · Fill 600x300 · Center 200x100 at (100,100) — unchanged by tiling"
    );
}

/// The reported symptom, with the operator's own numbers: `datagrid-form.cfrm`
/// is 1320x720, its background image is 1920x1200, and it is an `Embedded`
/// form loaded into the shell's ContentPane.
///
/// Every mode is evaluated against `backdrop_size(form_size, window_size)`,
/// and the occupant path handed that the WHOLE WINDOW — while the form is
/// drawn into the pane, narrower by the MenuPane rail and shorter by the
/// breadcrumb band. The modes were therefore laid out against a rectangle the
/// developer cannot see, which is what "fit/stretched seems to be the same"
/// and "modes misbehave when the window resizes" both come down to (operator,
/// 2026-08-31).
///
/// The witness is FIT's own contract, which holds whatever the aspect ratios
/// happen to be: the WHOLE image is visible, and it is CENTRED on the surface.
/// Against the window it satisfies neither — part of the image is cut off by
/// the pane edge, and what remains sits off-centre. `FormBody::backdrop` now
/// takes its extent from the Ui it is drawn into, so the surface is the pane.
#[test]
fn modes_laid_out_against_the_window_break_inside_the_pane() {
    use cobolt_forms::render::backdrop_size;

    let form = Vec2::new(1320.0, 720.0);
    let image = Vec2::new(1920.0, 1200.0);
    // A shell window, and the pane left after the rail (200) and band (48).
    let window = Vec2::new(1712.0, 2000.0);
    let pane = Vec2::new(window.x - 200.0, window.y - 48.0); // 1512 x 1952
    // The occupant is drawn from the pane's own origin, so the visible region
    // is the pane rect anchored where the backdrop starts.
    let visible = Rect::from_min_size(egui::pos2(0.0, 0.0), pane);

    // ── WRONG (what the occupant did): laid out against the WINDOW ──────────
    let fit_wrong = image_quads(BgImageMode::Fit, backdrop_size(form, Some(window)), image)[0];
    assert!(
        !visible.contains_rect(fit_wrong),
        "against the window, Fit runs past the pane edge — the mode's one \
         promise (show all of it) is broken. fit={fit_wrong:?} pane={visible:?}"
    );
    assert!(
        (fit_wrong.center().x - visible.center().x).abs() > 50.0,
        "…and it is centred on the WINDOW, not the pane the developer sees: \
         image centre {:?} vs pane centre {:?}",
        fit_wrong.center(),
        visible.center()
    );

    // ── RIGHT: laid out against the PANE ────────────────────────────────────
    let fit_right = image_quads(BgImageMode::Fit, backdrop_size(form, Some(pane)), image)[0];
    assert!(
        visible.contains_rect(fit_right),
        "Fit against the pane shows the whole image inside it, got {fit_right:?}"
    );
    assert!(
        (fit_right.center().x - visible.center().x).abs() < 0.5
            && (fit_right.center().y - visible.center().y).abs() < 0.5,
        "…centred on the pane, got {:?} vs {:?}",
        fit_right.center(),
        visible.center()
    );
    // Aspect preserved either way — that was never the broken part.
    let aspect = fit_right.width() / fit_right.height();
    assert!(
        (aspect - image.x / image.y).abs() < 0.01,
        "Fit preserves the image aspect, got {aspect}"
    );

    // Center has the same failure: it centres on whatever rect it is given.
    let c_wrong = image_quads(BgImageMode::Center, backdrop_size(form, Some(window)), image)[0];
    let c_right = image_quads(BgImageMode::Center, backdrop_size(form, Some(pane)), image)[0];
    assert!(
        (c_wrong.center().y - c_right.center().y).abs() > 20.0,
        "Center moved with the fix — window {:?} vs pane {:?}",
        c_wrong.center(),
        c_right.center()
    );

    println!(
        "\n  embedded form 1320x720 · image 1920x1200 · window 1712x2000 · pane 1512x1952\n\
         \x20   against the WINDOW : Fit {:.0}x{:.0} at centre ({:.0},{:.0}) — clipped by the pane, off-centre\n\
         \x20   against the PANE   : Fit {:.0}x{:.0} at centre ({:.0},{:.0}) — whole image, centred\n\
         \x20   Center             : moved {:.0}px vertically back onto the pane\n",
        fit_wrong.width(), fit_wrong.height(), fit_wrong.center().x, fit_wrong.center().y,
        fit_right.width(), fit_right.height(), fit_right.center().x, fit_right.center().y,
        (c_wrong.center().y - c_right.center().y).abs(),
    );
}
