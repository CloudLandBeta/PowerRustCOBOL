// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Plot (WASM-018 R12, AC8): every chart kind renders into pixels and into SVG;
//! a COBOL table of 100,000 values reaches a series in one pass.
#![cfg(feature = "plot")]

mod common;

use std::sync::Arc;

use cobolt_graphics::display_list::DisplayListBuilder;
use cobolt_graphics::par::with_threads;
use cobolt_graphics::plot::{Column, NumKind, Plot, SeriesKind, TableSpec, ZonedSign};
use cobolt_graphics::text::FontSet;
use cobolt_graphics::textapi::TextEngine;
use cobolt_graphics::{Limits, PixelBuffer, Rgba};
use common::{assert_golden, Tolerance};

fn fonts() -> Arc<FontSet> {
    let f = FontSet::new();
    f.add_font_data(epaint_default_fonts::UBUNTU_LIGHT.to_vec()).unwrap();
    f.add_font_data(epaint_default_fonts::HACK_REGULAR.to_vec()).unwrap();
    f.set_generic(cobolt_graphics::text::Generic::SansSerif, "Ubuntu-Light");
    f.set_generic(cobolt_graphics::text::Generic::Monospace, "Hack");
    Arc::new(f)
}

fn plot() -> Plot {
    Plot::new(Limits::default())
}

fn ink(img: &PixelBuffer, bg: Rgba) -> usize {
    img.as_bytes().chunks_exact(4).filter(|p| Rgba::new(p[0], p[1], p[2], p[3]) != bg).count()
}

/// A plot of each kind with data that shows what it does.
fn sample(kind: SeriesKind) -> Plot {
    let mut p = plot();
    p.options.title = format!("{} chart", kind.name().to_lowercase());
    p.options.x_label = "x axis".into();
    p.options.y_label = "y axis".into();
    match kind {
        SeriesKind::Line | SeriesKind::Scatter | SeriesKind::Area => {
            for (name, phase) in [("sine", 0.0), ("shifted", 1.2)] {
                let s = p.add_series(kind, name).unwrap();
                for i in 0..40 {
                    let x = i as f64 / 4.0;
                    p.add_point(s, x, (x + phase).sin() * 3.0 + 4.0).unwrap();
                }
            }
        }
        SeriesKind::Bar => {
            p.options.categories = ["Jan", "Feb", "Mar", "Apr", "May"].iter().map(|s| s.to_string()).collect();
            for (name, vals) in [("2025", [4.0, 7.0, 5.0, 9.0, 6.0]), ("2026", [5.0, 6.0, 8.0, 7.0, 9.5])] {
                let s = p.add_series(kind, name).unwrap();
                for v in vals {
                    p.add_value(s, "", v).unwrap();
                }
            }
        }
        SeriesKind::Pie => {
            let s = p.add_series(kind, "share").unwrap();
            for (l, v) in [("Rust", 45.0), ("COBOL", 30.0), ("SQL", 15.0), ("Other", 10.0)] {
                p.add_value(s, l, v).unwrap();
            }
        }
        SeriesKind::Histogram => {
            let s = p.add_series(kind, "samples").unwrap();
            let mut seed = 12345u64;
            for _ in 0..500 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let a = (seed >> 33) as f64 / (1u64 << 31) as f64;
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let b = (seed >> 33) as f64 / (1u64 << 31) as f64;
                p.add_value(s, "", (a + b) * 50.0).unwrap(); // a triangle distribution
            }
        }
        SeriesKind::Candlestick => {
            let s = p.add_series(kind, "price").unwrap();
            let mut price = 100.0;
            for i in 0..20 {
                let open = price;
                let close = open + ((i * 7 % 11) as f64 - 5.0);
                let high = open.max(close) + 2.0;
                let low = open.min(close) - 2.0;
                p.add_candle(s, i as f64, open, high, low, close).unwrap();
                price = close;
            }
        }
        SeriesKind::Heatmap => {
            let s = p.add_series(kind, "").unwrap();
            let (cols, rows) = (12, 8);
            let vals: Vec<f64> = (0..rows * cols).map(|i| ((i % cols) as f64 * 0.5).sin() * ((i / cols) as f64 * 0.7).cos()).collect();
            p.set_grid(s, cols, rows, &vals).unwrap();
        }
        SeriesKind::Polar => {
            let s = p.add_series(kind, "rose").unwrap();
            for i in 0..=72 {
                let a = i as f64 * 5.0;
                let r = 2.0 + 1.5 * (a * 3.0).to_radians().cos();
                p.add_point(s, a, r).unwrap();
            }
            p.series_mut(s).unwrap().closed = true;
        }
        SeriesKind::ErrorBars => {
            let s = p.add_series(kind, "measure").unwrap();
            for i in 0..10 {
                p.add_error_point(s, i as f64, (i as f64 * 0.8).sin() * 4.0 + 5.0, 0.4 + (i % 3) as f64 * 0.3).unwrap();
            }
        }
        SeriesKind::Surface3d => {
            let s = p.add_series(kind, "").unwrap();
            let (cols, rows) = (24, 24);
            let vals: Vec<f64> = (0..rows * cols)
                .map(|i| {
                    let (x, y) = ((i % cols) as f64 / 3.0 - 4.0, (i / cols) as f64 / 3.0 - 4.0);
                    let r = (x * x + y * y).sqrt().max(1e-6);
                    r.sin() / r * 4.0
                })
                .collect();
            p.set_surface(s, -4.0, 4.0, -4.0, 4.0, cols, rows, &vals).unwrap();
        }
        SeriesKind::Scatter3d => {
            let s = p.add_series(kind, "helix").unwrap();
            for i in 0..80 {
                let t = i as f64 / 6.0;
                p.add_point3(s, t.cos() * 3.0, t.sin() * 3.0, t * 0.5).unwrap();
            }
        }
    }
    p
}

#[test]
fn every_chart_kind_renders_pixels_and_svg() {
    let f = fonts();
    let text: &dyn TextEngine = f.as_ref();
    for kind in SeriesKind::ALL {
        let p = sample(kind);
        let img = p.render(480, 320, Some(text)).unwrap_or_else(|e| panic!("{}: {e}", kind.name()));
        assert_eq!((img.width(), img.height()), (480, 320));
        let drawn = ink(&img, Rgba::WHITE);
        assert!(drawn > 2000, "{}: only {drawn} pixels were drawn", kind.name());
        let svg = p.to_svg(480, 320, Some(text)).unwrap_or_else(|e| panic!("{}: {e}", kind.name()));
        assert!(svg.starts_with("<svg") || svg.contains("<svg"), "{}: not SVG", kind.name());
        assert!(svg.contains("</svg>"), "{}: SVG is not closed", kind.name());
        let title = format!("{} chart", kind.name().to_lowercase());
        assert!(svg.contains(&title), "{}: the title is not in the SVG", kind.name());
        assert!(svg.matches("<text").count() >= 2, "{}: labels are missing from the SVG", kind.name());
    }
}

#[test]
fn the_chart_gallery_matches_its_golden_images() {
    let f = fonts();
    let text: &dyn TextEngine = f.as_ref();
    for kind in SeriesKind::ALL {
        let img = sample(kind).render(480, 320, Some(text)).unwrap();
        if let Ok(dir) = std::env::var("GRAPHICS_DUMP") {
            common::write_png(&std::path::Path::new(&dir).join(format!("plot-{}.png", kind.name().to_lowercase())), &img);
        }
        assert_golden(&format!("plot-{}", kind.name().to_lowercase()), &img, Tolerance::EXACT);
    }
}

#[test]
fn bar_charts_show_their_category_names_and_legend_entries() {
    let f = fonts();
    let svg = sample(SeriesKind::Bar).to_svg(480, 320, Some(f.as_ref())).unwrap();
    for name in ["Jan", "Feb", "Mar", "Apr", "May", "2025", "2026", "x axis", "y axis"] {
        assert!(svg.contains(name), "{name} is not in the SVG");
    }
}

#[test]
fn a_pie_labels_its_slices() {
    let f = fonts();
    let svg = sample(SeriesKind::Pie).to_svg(400, 400, Some(f.as_ref())).unwrap();
    for name in ["Rust", "COBOL", "SQL", "Other", "45.0%"] {
        assert!(svg.contains(name), "{name}");
    }
}

#[test]
fn families_cannot_be_mixed() {
    let mut p = plot();
    let a = p.add_series(SeriesKind::Line, "a").unwrap();
    p.add_point(a, 0.0, 0.0).unwrap();
    let b = p.add_series(SeriesKind::Pie, "b").unwrap();
    p.add_value(b, "x", 1.0).unwrap();
    let e = p.render(200, 200, None).unwrap_err();
    assert!(e.to_string().contains("cannot mix"), "{e}");
    assert!(plot().render(200, 200, None).unwrap_err().to_string().contains("no series"));
}

#[test]
fn the_wrong_data_call_for_a_kind_is_refused_with_the_kind_named() {
    let mut p = plot();
    let bar = p.add_series(SeriesKind::Bar, "").unwrap();
    let e = p.add_point(bar, 1.0, 2.0).unwrap_err();
    assert!(e.to_string().contains("BAR") && e.to_string().contains("AddPoint"), "{e}");
    let line = p.add_series(SeriesKind::Line, "").unwrap();
    assert!(p.add_value(line, "", 1.0).is_err());
    assert!(p.add_candle(line, 0.0, 1.0, 2.0, 0.5, 1.5).is_err());
    assert!(p.set_grid(line, 2, 2, &[1.0; 4]).is_err());
    let heat = p.add_series(SeriesKind::Heatmap, "").unwrap();
    assert!(p.set_grid(heat, 2, 2, &[1.0; 3]).is_err(), "a 2x2 grid needs 4 values");
    assert!(p.set_grid(heat, 0, 2, &[]).is_err());
}

#[test]
fn handles_are_released_and_never_reused_by_mistake() {
    let mut p = plot();
    let a = p.add_series(SeriesKind::Line, "a").unwrap();
    p.remove_series(a).unwrap();
    assert_eq!(p.series_count(), 0);
    assert_eq!(p.add_point(a, 1.0, 1.0).unwrap_err().code(), "released-handle");
    let b = p.add_series(SeriesKind::Line, "b").unwrap();
    assert_ne!(a, b);
}

#[test]
fn a_series_reads_a_cobol_table_in_one_pass() {
    // 100,000 rows of (x PIC 9(6) DISPLAY, y PIC S9(5)V99 COMP-3): 6 + 4 bytes per row.
    let n = 100_000usize;
    let mut block = Vec::with_capacity(n * 10);
    for i in 0..n {
        block.extend_from_slice(format!("{:06}", i).as_bytes());
        let v = (((i as f64 * 0.01).sin() * 1000.0 + 1000.0) * 100.0) as u64 % 10_000_000;
        let d: Vec<u8> = format!("{:07}", v).bytes().map(|c| c - b'0').collect();
        block.extend_from_slice(&[d[0] << 4 | d[1], d[2] << 4 | d[3], d[4] << 4 | d[5], d[6] << 4 | 0x0C]);
    }
    let spec = TableSpec {
        count: n,
        stride: 10,
        columns: vec![
            Column { offset: 0, kind: NumKind::Zoned { digits: 6, scale: 0, sign: ZonedSign::Unsigned } },
            Column { offset: 6, kind: NumKind::Packed { digits: 7, scale: 2, signed: true } },
        ],
    };
    let mut p = plot();
    let s = p.add_series(SeriesKind::Line, "table").unwrap();
    let t = std::time::Instant::now();
    let skipped = p.set_series_from_table(s, &block, &spec).unwrap();
    let took = t.elapsed();
    assert_eq!(skipped, 0);
    assert_eq!(p.series(s).unwrap().points(), n);
    eprintln!("100,000 rows (zoned x, packed y) read into a series in {took:?}");
    // The series renders (decimated by the backend into one polyline).
    let img = p.render(640, 360, None).unwrap();
    assert!(ink(&img, Rgba::WHITE) > 500);
}

#[test]
fn a_mixed_kind_table_decodes_each_field_by_its_own_usage() {
    // x = binary big-endian u16 (COMP), y = a signed zoned S9(3)V9 with a trailing overpunch.
    let mut block = Vec::new();
    for (x, y) in [(1u16, "001B"), (2, "003D"), (3, "005O")] {
        block.extend_from_slice(&x.to_be_bytes());
        block.extend_from_slice(y.as_bytes());
    }
    let spec = TableSpec {
        count: 3,
        stride: 6,
        columns: vec![
            Column { offset: 0, kind: NumKind::Binary { bytes: 2, signed: false, big_endian: true, scale: 0 } },
            Column { offset: 2, kind: NumKind::Zoned { digits: 4, scale: 1, sign: ZonedSign::TrailingOverpunch } },
        ],
    };
    let t = cobolt_graphics::plot::read_table(&block, &spec, &Limits::default()).unwrap();
    assert_eq!(t.bad, 0);
    assert_eq!(t.columns[0], vec![1.0, 2.0, 3.0]);
    assert_eq!(t.columns[1], vec![1.2, 3.4, -5.6]);
}

#[test]
fn tables_of_every_series_kind_are_accepted_with_the_right_number_of_columns() {
    let mut block = Vec::new();
    for i in 0..10 {
        for k in 0..5 {
            block.extend_from_slice(&((i * 10 + k) as f64).to_le_bytes());
        }
    }
    let spec = |cols: Vec<usize>| TableSpec::uniform(10, 40, NumKind::F64 { big_endian: false }, &cols);
    let mut p = plot();
    let line = p.add_series(SeriesKind::Line, "").unwrap();
    assert_eq!(p.set_series_from_table(line, &block, &spec(vec![0])).unwrap(), 0);
    assert_eq!(p.set_series_from_table(line, &block, &spec(vec![0, 8])).unwrap(), 0);
    assert!(p.set_series_from_table(line, &block, &spec(vec![0, 8, 16])).is_err());
    let candle = p.add_series(SeriesKind::Candlestick, "").unwrap();
    assert!(p.set_series_from_table(candle, &block, &spec(vec![0, 8])).is_err());
    assert_eq!(p.set_series_from_table(candle, &block, &spec(vec![0, 8, 16, 24, 32])).unwrap(), 0);
    let heat = p.add_series(SeriesKind::Heatmap, "").unwrap();
    assert!(p.set_series_from_table(heat, &block, &spec(vec![0])).unwrap_err().to_string().contains("SetGrid"));
    // Values that are not numbers are skipped and counted.
    let mut bad = block.clone();
    bad[0..8].copy_from_slice(&f64::NAN.to_le_bytes());
    assert_eq!(p.set_series_from_table(line, &bad, &spec(vec![0])).unwrap(), 1);
    // The series limit applies.
    let mut small = Plot::new(Limits { max_series_points: 5, ..Limits::default() });
    let s = small.add_series(SeriesKind::Line, "").unwrap();
    assert!(small.set_series_from_table(s, &block, &spec(vec![0])).unwrap_err().to_string().contains("max_series_points"));
}

#[test]
fn a_chart_can_be_recorded_straight_into_a_canvas_list() {
    use cobolt_graphics::canvas::{Surface, SurfaceConfig};
    let f = fonts();
    let p = sample(SeriesKind::Line);
    let mut b = DisplayListBuilder::new();
    b.set_fill_color(Rgba::rgb(230, 230, 255)).fill_rect(0.0, 0.0, 700.0, 400.0);
    b.save().translate(100.0, 50.0);
    p.render_into(&mut b, 480, 320, Some(f.as_ref())).unwrap();
    b.restore();
    b.present();
    let mut s = Surface::new(&SurfaceConfig::new(700, 400), &Limits::default()).unwrap();
    struct R(Arc<FontSet>);
    impl cobolt_graphics::resources::Resources for R {
        fn image(&self, _: u32) -> Option<Arc<PixelBuffer>> {
            None
        }
        fn text_engine(&self) -> Option<&dyn TextEngine> {
            Some(self.0.as_ref())
        }
    }
    s.execute(&b.take().unwrap(), &R(f)).unwrap();
    let img = s.to_image();
    assert_eq!(img.get(10, 10), Some(Rgba::rgb(230, 230, 255)), "outside the chart is the canvas background");
    assert_eq!(img.get(120, 70), Some(Rgba::WHITE), "inside the chart is the plot background");
}

#[test]
fn rendering_does_not_depend_on_the_number_of_threads() {
    let f = fonts();
    let render = || sample(SeriesKind::Scatter).render(1920, 1080, Some(f.as_ref())).unwrap().into_bytes();
    let one = with_threads(1, render);
    let many = with_threads(8, render);
    assert!(one == many, "a large chart differs between 1 thread and 8");
}

#[test]
fn degenerate_plots_do_not_panic() {
    let f = fonts();
    let mut p = plot();
    // One point, constant data, NaN and infinite values, empty series.
    let a = p.add_series(SeriesKind::Line, "one").unwrap();
    p.add_point(a, 1.0, 1.0).unwrap();
    let b = p.add_series(SeriesKind::Scatter, "bad").unwrap();
    for (x, y) in [(f64::NAN, 1.0), (1.0, f64::INFINITY), (2.0, 2.0)] {
        p.add_point(b, x, y).unwrap();
    }
    p.add_series(SeriesKind::Area, "empty").unwrap();
    p.render(300, 200, Some(f.as_ref())).unwrap();
    p.to_svg(300, 200, Some(f.as_ref())).unwrap();
    for kind in SeriesKind::ALL {
        let mut q = plot();
        q.add_series(kind, "nothing").unwrap();
        let _ = q.render(200, 160, Some(f.as_ref()));
        let _ = q.to_svg(200, 160, None);
    }
    assert!(p.render(10, 10, None).is_err(), "too small");
    let lim = Limits { max_canvas_pixels: 100_000, ..Limits::default() };
    assert!(Plot::new(lim).render(2000, 2000, None).is_err());
}
