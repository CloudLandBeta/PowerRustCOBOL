// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The COBOL-facing API through the registry (WASM-018 R1, R2, R7, R9-R12, R15, AC2, AC8, AC10):
//! every method named in the crate docs is a method that dispatches, programs drive every
//! object through `Registry::invoke` with `Value`s only, objects reach each other by handle,
//! and nothing a program passes can make the crate panic.
#![cfg(all(feature = "text", feature = "imagebuf", feature = "plot", feature = "physics", feature = "scene3d-core"))]

mod common;

use std::sync::Arc;

use cobolt_graphics::error::GraphicsError;
use cobolt_graphics::registry::{ObjectKind, Registry};
use cobolt_graphics::resources::MapAssets;
use cobolt_graphics::text::FontSet;
use cobolt_graphics::value::{Ty, Value};
use cobolt_graphics::{Limits, Rgba};
use common::{glb, synthfont};

fn small() -> Limits {
    Limits {
        max_canvas_width: 512,
        max_canvas_height: 512,
        max_canvas_pixels: 200_000,
        max_image_pixels: 200_000,
        max_texture_size: 512,
        max_nodes: 2_000,
        max_triangles: 100_000,
        max_bodies: 500,
        max_display_list_commands: 100_000,
        max_path_points: 50_000,
        max_series_points: 100_000,
        max_total_bytes: 256 * 1024 * 1024,
        ..Limits::default()
    }
}

fn reg() -> Registry {
    let mut r = Registry::new(Limits::default());
    let fonts = Arc::new(FontSet::new());
    fonts.add_font_data(synthfont::latin("Test Latin")).unwrap();
    fonts.set_generic(cobolt_graphics::text::Generic::SansSerif, "Test Latin");
    r.set_text_engine(fonts);
    r
}

fn f(v: &[f64]) -> Value {
    Value::Floats(v.to_vec())
}

fn int(v: Value) -> i64 {
    match v {
        Value::Int(i) => i,
        other => panic!("expected a whole number, got {other:?}"),
    }
}

fn num(v: Value) -> f64 {
    match v {
        Value::Int(i) => i as f64,
        Value::Num(n) => n,
        other => panic!("expected a number, got {other:?}"),
    }
}

fn floats(v: Value) -> Vec<f64> {
    match v {
        Value::Floats(f) => f,
        other => panic!("expected a list of numbers, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The tables
// ---------------------------------------------------------------------------

fn sample(t: Ty) -> Value {
    match t {
        Ty::Int => Value::Int(1),
        Ty::Num => Value::Num(1.0),
        Ty::Text => Value::Text("x".into()),
        Ty::Bytes => Value::Bytes(vec![1, 2, 3, 4]),
        Ty::Floats => Value::Floats(vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]),
        Ty::Ints => Value::Ints(vec![0, 1, 2]),
        Ty::Handle => Value::Int(1),
        Ty::Color => Value::Text("#336699".into()),
        Ty::Flag => Value::Int(1),
    }
}

fn ty_of_word(w: &str) -> Ty {
    match w {
        "int" => Ty::Int,
        "num" => Ty::Num,
        "text" => Ty::Text,
        "bytes" => Ty::Bytes,
        "numbers" => Ty::Floats,
        "ints" => Ty::Ints,
        "handle" => Ty::Handle,
        "color" => Ty::Color,
        "flag" => Ty::Flag,
        other => panic!("unknown kind word {other}"),
    }
}

#[test]
fn every_documented_method_dispatches_and_nothing_else_does() {
    let methods = Registry::all_methods();
    assert!(methods.len() > 250, "{} methods", methods.len());
    let mut by_kind = std::collections::BTreeMap::new();
    for m in &methods {
        *by_kind.entry(m.object.name()).or_insert(0usize) += 1;
    }
    eprintln!("methods per object: {by_kind:?}");
    let mut r = Registry::new(small());
    for kind in ObjectKind::available() {
        let ctor: Vec<Value> = match kind {
            ObjectKind::Canvas2D | ObjectKind::Canvas3D => vec![Value::Int(64), Value::Int(48)],
            _ => vec![],
        };
        let h = r.create(kind, 1, &ctor).unwrap();
        for m in Registry::methods(kind) {
            if kind == ObjectKind::Canvas3D && matches!(m.name.as_str(), "Connect" | "Render" | "Screenshot") {
                continue; // these open a graphics device; the GPU tests cover them
            }
            let args: Vec<Value> = m.params.iter().filter(|p| !p.2).map(|p| sample(ty_of_word(p.1))).collect();
            let outcome = r.invoke(h, &m.name, &args);
            if let Err(e) = outcome {
                assert!(
                    !matches!(e, GraphicsError::UnknownMethod { .. } | GraphicsError::ArgumentCount { .. }),
                    "{}.{} is documented but does not dispatch with its own parameters: {e:?}",
                    kind.name(),
                    m.name
                );
            }
            // Too many arguments is always refused by the count check, for a method that is not a drawing command.
            if kind != ObjectKind::Canvas2D {
                let mut many = args.clone();
                many.extend((0..m.params.len() + 1).map(|_| Value::Int(1)));
                assert!(matches!(r.invoke(h, &m.name, &many), Err(GraphicsError::ArgumentCount { .. })), "{}.{}", kind.name(), m.name);
            }
            // The name matches without regard to case.
            let shouting = m.name.to_ascii_uppercase();
            if let Err(e) = r.invoke(h, &shouting, &args) {
                assert!(!matches!(e, GraphicsError::UnknownMethod { .. }), "{}.{shouting}", kind.name());
            }
        }
        assert!(matches!(r.invoke(h, "NoSuchMethod", &[]), Err(GraphicsError::UnknownMethod { .. })));
    }
}

#[test]
fn the_names_a_program_needs_are_all_there() {
    // R7, R9, R10, R11, R12: spot-check the names the spec lists.
    let have = |kind: ObjectKind, name: &str| Registry::methods(kind).iter().any(|m| m.name == name);
    for n in [
        "Save", "Restore", "Clear", "BeginPath", "MoveTo", "LineTo", "QuadTo", "CubicTo", "ArcTo", "Arc", "Ellipse", "Rect", "RoundRect", "ClosePath", "Fill", "Stroke", "Clip", "FillRect",
        "StrokeRect", "ClearRect", "DrawLine", "DrawCircle", "DrawPolygon", "SetFillColor", "SetStrokeColor", "SetLineWidth", "SetLineCap", "SetLineJoin", "SetDash", "SetAlpha", "SetBlendMode",
        "SetLinearGradient", "SetRadialGradient", "SetPattern", "SetShadow", "Translate", "Rotate", "Scale", "SetTransform", "ResetTransform", "SetFont", "SetTextAlign", "DrawText", "MeasureText",
        "DrawImage", "DrawSprite", "ToImage", "GetPixel", "IsPointInPath", "Present",
    ] {
        assert!(have(ObjectKind::Canvas2D, n), "Canvas2D.{n}");
    }
    for n in [
        "Create", "LoadAsset", "LoadFromBytes", "Encode", "Save", "Resize", "Crop", "Rotate", "Flip", "Fill", "GetPixel", "SetPixel", "DrawOnto", "Composite", "Grayscale", "Invert", "Brightness",
        "Contrast", "Hue", "Saturation", "Threshold", "BlurGaussian", "BlurBox", "Sharpen", "EdgeDetect", "Convolve", "Dilate", "Erode", "Histogram",
    ] {
        assert!(have(ObjectKind::ImageBuffer, n), "ImageBuffer.{n}");
    }
    for n in ["AddPoint", "SetSeriesFromTable", "Render", "ToSvg", "RenderToCanvas"] {
        assert!(have(ObjectKind::Plot, n), "Plot.{n}");
    }
    for kind in [ObjectKind::Physics2D, ObjectKind::Physics3D] {
        for n in ["CreateWorld", "AddBody", "AddBall", "AddBox", "AddCapsule", "AddConvexHull", "SetPosition", "SetVelocity", "ApplyForce", "ApplyImpulse", "AddFixedJoint", "AddRevoluteJoint", "AddPrismaticJoint", "RayCast", "Step", "SetFixedStep", "GetPosition", "GetRotation", "TakeEvents", "BindSprite", "BindNode"] {
            assert!(have(kind, n), "{}.{n}", kind.name());
        }
    }
    assert!(have(ObjectKind::Physics3D, "AddTrimesh"));
    for n in [
        "AddBox", "AddSphere", "AddPlane", "AddCylinder", "AddCone", "AddTorus", "LoadModel", "SetPosition", "SetRotation", "SetScale", "LookAt", "AddDirectionalLight", "AddPointLight", "AddSpotLight",
        "SetAmbient", "SetBackground", "SetFog", "AddCamera", "PlayAnimation", "SetAnimationTime", "Remove", "SetInstances", "SetTexture", "SetMetallic", "SetRoughness", "SetEmissive",
    ] {
        assert!(have(ObjectKind::Scene3D, n), "Scene3D.{n}");
    }
    for n in ["SetScene", "SetCamera", "Render", "Resize", "Screenshot", "Pick", "GetBackend", "SetMsaa"] {
        assert!(have(ObjectKind::Canvas3D, n), "Canvas3D.{n}");
    }
}

/// The method tables as the crate docs print them.
fn render_tables() -> String {
    let mut out = String::new();
    for kind in ObjectKind::available() {
        let methods = Registry::methods(kind);
        out.push_str(&format!("//! ### {} ({} methods)\n//!\n", kind.name(), methods.len()));
        out.push_str("//! | Method | Parameters | Returns | What it does |\n//! |---|---|---|---|\n");
        for m in methods {
            let ps: Vec<String> = m.params.iter().map(|(n, t, opt)| if *opt { format!("[{n}: {t}]") } else { format!("{n}: {t}") }).collect();
            let params = if ps.is_empty() { "-".to_string() } else { ps.join(", ") };
            out.push_str(&format!("//! | `{}` | {} | {} | {} |\n", m.name, params.replace('|', "/"), m.returns, m.doc.trim().replace('|', "/")));
        }
        out.push_str("//!\n");
    }
    out
}

#[test]
fn the_method_tables_in_the_crate_docs_are_the_tables_the_registry_dispatches() {
    const BEGIN: &str = "//! <!-- methods:begin -->\n";
    const END: &str = "//! <!-- methods:end -->\n";
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("lib.rs");
    let src = std::fs::read_to_string(&path).unwrap();
    let (a, b) = (src.find(BEGIN).expect("lib.rs has the begin marker") + BEGIN.len(), src.find(END).expect("lib.rs has the end marker"));
    let want = render_tables();
    if std::env::var("DOC_UPDATE").map(|v| v == "1").unwrap_or(false) {
        let new = format!("{}{}{}", &src[..a], want, &src[b..]);
        std::fs::write(&path, new).unwrap();
        eprintln!("lib.rs method tables updated");
        return;
    }
    assert_eq!(&src[a..b], want, "the method tables in src/lib.rs are out of date: run this test once with DOC_UPDATE=1 and review the diff");
}

// ---------------------------------------------------------------------------
// Programs
// ---------------------------------------------------------------------------

#[test]
fn a_canvas_program_draws_through_values_only_and_ten_thousand_shapes_are_one_host_call() {
    let mut r = reg();
    let canvas = r.create(ObjectKind::Canvas2D, 1, &[Value::Int(200), Value::Int(100)]).unwrap();
    r.invoke(canvas, "SetFillColor", &[Value::Text("#2060C0".into())]).unwrap();
    for i in 0..10_000 {
        r.invoke(canvas, "FillRect", &[Value::Int(i % 190), Value::Int(i % 90), Value::Int(4), Value::Int(4)]).unwrap();
    }
    assert_eq!(int(r.invoke(canvas, "HostCalls", &[]).unwrap()), 0, "ten thousand shapes recorded, nothing drawn yet");
    r.invoke(canvas, "Present", &[]).unwrap();
    assert_eq!(int(r.invoke(canvas, "HostCalls", &[]).unwrap()), 1, "and one call to draw them all (AC2)");
    assert_eq!(int(r.invoke(canvas, "TakePresented", &[]).unwrap()), 1);
    // ToImage returns what was drawn, as an ImageBuffer handle.
    let img = r.invoke(canvas, "ToImage", &[]).unwrap();
    let Value::Int(img) = img else { panic!() };
    assert_eq!(r.kind_of(img as u32).unwrap(), ObjectKind::ImageBuffer);
    let p = int(r.invoke(img as u32, "GetPixel", &[Value::Int(0), Value::Int(0)]).unwrap());
    assert_eq!(p as u32, Rgba::rgb(0x20, 0x60, 0xC0).to_u32());
    assert_eq!(int(r.invoke(img as u32, "GetWidth", &[]).unwrap()), 200);
    // GetPixel on the canvas agrees, and outside the canvas it is -1.
    assert_eq!(int(r.invoke(canvas, "GetPixel", &[Value::Int(1), Value::Int(1)]).unwrap()) as u32, Rgba::rgb(0x20, 0x60, 0xC0).to_u32());
    assert_eq!(int(r.invoke(canvas, "GetPixel", &[Value::Int(500), Value::Int(1)]).unwrap()), -1);
}

#[test]
fn text_lines_measure_the_way_they_draw() {
    let mut r = reg();
    let canvas = r.create(ObjectKind::Canvas2D, 1, &[Value::Int(300), Value::Int(60)]).unwrap();
    r.invoke(canvas, "SetFont", &[Value::Text("20px Test Latin".into())]).unwrap();
    let m = floats(r.invoke(canvas, "MeasureText", &[Value::Text("HELLO".into())]).unwrap());
    assert_eq!(m.len(), 8);
    assert!(m[0] > 20.0, "a five-letter word has a width: {m:?}");
    r.invoke(canvas, "SetFillColor", &[Value::Text("black".into())]).unwrap();
    r.invoke(canvas, "DrawText", &[Value::Text("HELLO".into()), Value::Int(10), Value::Int(40), Value::Int(0), Value::Int(1)]).unwrap();
    let img = int(r.invoke(canvas, "ToImage", &[]).unwrap()) as u32;
    // The ink starts at x = 10 and ends where MeasureText says (within a pixel).
    let ink_right = (0..300).rev().find(|&x| (0..60).any(|y| int(r.invoke(img, "GetPixel", &[Value::Int(x), Value::Int(y)]).unwrap()) as u32 & 0xFF > 0)).expect("some ink");
    assert!((ink_right as f64 - (10.0 + m[0])).abs() <= 2.0, "ink ends at {ink_right}, MeasureText says {}", 10.0 + m[0]);
}

#[test]
fn images_cross_between_objects_by_handle() {
    let mut r = reg();
    let img = r.create(ObjectKind::ImageBuffer, 1, &[]).unwrap();
    r.invoke(img, "Create", &[Value::Int(16), Value::Int(16), Value::Text("#FF0000".into())]).unwrap();
    let canvas = r.create(ObjectKind::Canvas2D, 1, &[Value::Int(64), Value::Int(64)]).unwrap();
    // The image draws itself onto the canvas, at 2x scale.
    r.invoke(img, "DrawOnto", &[Value::Int(canvas as i64), Value::Int(8), Value::Int(8), Value::Int(32), Value::Int(32)]).unwrap();
    r.invoke(canvas, "Present", &[]).unwrap();
    assert_eq!(int(r.invoke(canvas, "GetPixel", &[Value::Int(20), Value::Int(20)]).unwrap()) as u32, 0xFF0000FF);
    assert_eq!(int(r.invoke(canvas, "GetPixel", &[Value::Int(4), Value::Int(4)]).unwrap()) as u32, 0);
    // Changing the image afterwards does not change what was drawn; drawing again does.
    r.invoke(img, "Fill", &[Value::Text("#0000FF".into())]).unwrap();
    assert_eq!(int(r.invoke(canvas, "GetPixel", &[Value::Int(20), Value::Int(20)]).unwrap()) as u32, 0xFF0000FF);
    r.invoke(img, "DrawOnto", &[Value::Int(canvas as i64), Value::Int(8), Value::Int(8)]).unwrap();
    assert_eq!(int(r.invoke(canvas, "GetPixel", &[Value::Int(12), Value::Int(12)]).unwrap()) as u32, 0x0000FFFF);
    // A released image is "no image" to the canvas, not a crash: the canvas says so.
    r.release(img).unwrap();
    r.invoke(canvas, "DrawImage", &[Value::Int(img as i64), Value::Int(0), Value::Int(0)]).unwrap();
    r.invoke(canvas, "Flush", &[]).unwrap();
    assert!(matches!(r.invoke(img, "GetWidth", &[]), Err(GraphicsError::ReleasedHandle(_))));
    let d = r.invoke(canvas, "GetDiagnostics", &[]).unwrap();
    assert!(matches!(d, Value::Text(s) if !s.is_empty()), "the canvas reports the missing image");
}

#[test]
fn an_image_resized_blurred_and_encoded_is_the_same_on_one_thread_and_on_many() {
    let make = |threads: usize| {
        let mut r = reg();
        let img = r.create(ObjectKind::ImageBuffer, 1, &[]).unwrap();
        r.invoke(img, "Create", &[Value::Int(300), Value::Int(200), Value::Text("#336699".into())]).unwrap();
        for i in 0..40 {
            r.invoke(img, "SetPixel", &[Value::Int(i * 7), Value::Int(i * 5), Value::Int(0xFFCC00FF)]).unwrap();
        }
        let out = cobolt_graphics::par::with_threads(threads, || {
            r.invoke(img, "Resize", &[Value::Int(150), Value::Int(100), Value::Text("lanczos".into())]).unwrap();
            r.invoke(img, "BlurGaussian", &[Value::Num(2.5)]).unwrap();
            r.invoke(img, "Saturation", &[Value::Num(1.3)]).unwrap();
            r.invoke(img, "Encode", &[Value::Text("png".into())]).unwrap()
        });
        let Value::Bytes(b) = out else { panic!() };
        b
    };
    let (one, many) = (make(1), make(8));
    assert!(one.len() > 100);
    assert_eq!(one, many, "the encoded bytes do not depend on the number of threads (AC7)");
}

#[test]
fn a_cobol_table_of_a_hundred_thousand_values_reaches_a_series_in_one_call() {
    let mut r = reg();
    let plot = r.create(ObjectKind::Plot, 1, &[]).unwrap();
    let series = int(r.invoke(plot, "AddSeries", &[Value::Text("line".into()), Value::Text("data".into())]).unwrap()) as u32;
    // Rows of (x PIC 9(6), y PIC S9(5)V99 COMP-3).
    let n = 100_000usize;
    let mut block = Vec::with_capacity(n * 10);
    for i in 0..n {
        block.extend_from_slice(format!("{:06}", i).as_bytes());
        let v = (((i as f64 * 0.01).sin() * 1000.0 + 1000.0) * 100.0) as u64 % 10_000_000;
        let d: Vec<u8> = format!("{:07}", v).bytes().map(|c| c - b'0').collect();
        block.extend_from_slice(&[d[0] << 4 | d[1], d[2] << 4 | d[3], d[4] << 4 | d[5], d[6] << 4 | 0x0C]);
    }
    let t = std::time::Instant::now();
    let skipped = int(r.invoke(plot, "SetSeriesFromTable", &[Value::Int(series as i64), Value::Bytes(block), Value::Int(n as i64), Value::Int(10), Value::Text("0:zoned(6,0); 6:packed(7,2,s)".into())]).unwrap());
    eprintln!("100,000 rows through SetSeriesFromTable in {:?}", t.elapsed());
    assert_eq!(skipped, 0);
    assert_eq!(int(r.invoke(plot, "GetSeriesPoints", &[Value::Int(series as i64)]).unwrap()), n as i64);
    // A bad description names the column.
    let e = r.invoke(plot, "SetSeriesFromTable", &[Value::Int(series as i64), Value::Bytes(vec![0; 10]), Value::Int(1), Value::Int(10), Value::Text("0:nonsense".into())]).unwrap_err();
    assert!(e.to_string().contains("0:nonsense"), "{e}");
}

#[test]
fn every_chart_kind_renders_into_an_image_a_canvas_and_svg_text() {
    let mut r = reg();
    let plot = r.create(ObjectKind::Plot, 1, &[]).unwrap();
    r.invoke(plot, "SetTitle", &[Value::Text("sales".into())]).unwrap();
    let line = int(r.invoke(plot, "AddSeries", &[Value::Text("LINE".into()), Value::Text("a".into())]).unwrap()) as u32;
    for i in 0..20 {
        r.invoke(plot, "AddPoint", &[Value::Int(line as i64), Value::Int(i), Value::Num((i as f64 * 0.4).sin())]).unwrap();
    }
    let image = int(r.invoke(plot, "Render", &[Value::Int(300), Value::Int(200)]).unwrap()) as u32;
    assert_eq!(int(r.invoke(image, "GetHeight", &[]).unwrap()), 200);
    let canvas = r.create(ObjectKind::Canvas2D, 1, &[Value::Int(400), Value::Int(300)]).unwrap();
    r.invoke(plot, "RenderToCanvas", &[Value::Int(canvas as i64), Value::Int(50), Value::Int(40), Value::Int(300), Value::Int(200)]).unwrap();
    r.invoke(canvas, "Present", &[]).unwrap();
    let ink = (0..400).filter(|&x| int(r.invoke(canvas, "GetPixel", &[Value::Int(x), Value::Int(140)]).unwrap()) as u32 & 0xFF != 0).count();
    assert!(ink > 100, "the chart landed on the canvas: {ink}");
    assert_eq!(int(r.invoke(canvas, "GetPixel", &[Value::Int(10), Value::Int(10)]).unwrap()), 0, "outside the chart it is still empty");
    let Value::Text(svg) = r.invoke(plot, "ToSvg", &[Value::Int(300), Value::Int(200)]).unwrap() else { panic!() };
    assert!(svg.starts_with("<svg") && svg.contains("polyline"), "{}", &svg[..svg.len().min(200)]);
    // A pie cannot be mixed with a line: a sentence, not a silent drop.
    let pie = int(r.invoke(plot, "AddSeries", &[Value::Text("pie".into()), Value::Text("p".into())]).unwrap()) as u32;
    r.invoke(plot, "AddValue", &[Value::Int(pie as i64), Value::Text("x".into()), Value::Int(1)]).unwrap();
    let e = r.invoke(plot, "Render", &[Value::Int(300), Value::Int(200)]).unwrap_err();
    assert!(e.to_string().to_lowercase().contains("mix"), "{e}");
}

#[test]
fn physics_bodies_drive_scene_nodes_and_events_come_back_as_numbers() {
    let mut r = reg();
    let scene = r.create(ObjectKind::Scene3D, 1, &[]).unwrap();
    let ball_node = int(r.invoke(scene, "AddSphere", &[Value::Num(0.5)]).unwrap()) as u32;
    let phys = r.create(ObjectKind::Physics3D, 1, &[]).unwrap();
    let world = int(r.invoke(phys, "CreateWorld", &[Value::Int(0), Value::Num(-9.81), Value::Int(0)]).unwrap());
    let w = Value::Int(world);
    let floor = int(r.invoke(phys, "AddBody", &[w.clone(), Value::Int(0), Value::Int(0), Value::Int(-1), Value::Int(0)]).unwrap());
    r.invoke(phys, "AddBox", &[w.clone(), Value::Int(floor), Value::Int(10), Value::Num(0.5), Value::Int(10)]).unwrap();
    let ball = int(r.invoke(phys, "AddBody", &[w.clone(), Value::Int(1), Value::Int(0), Value::Int(4), Value::Int(0)]).unwrap());
    r.invoke(phys, "AddBall", &[w.clone(), Value::Int(ball), Value::Num(0.5)]).unwrap();
    r.invoke(phys, "BindNode", &[w.clone(), Value::Int(ball), Value::Int(scene as i64), Value::Int(ball_node as i64)]).unwrap();
    let mut events = Vec::new();
    for _ in 0..180 {
        r.invoke(phys, "Step", &[w.clone(), Value::Num(1.0 / 60.0)]).unwrap();
        events.extend(floats(r.invoke(phys, "TakeEvents", &[w.clone()]).unwrap()));
    }
    let body = floats(r.invoke(phys, "GetPosition", &[w.clone(), Value::Int(ball)]).unwrap());
    let node = floats(r.invoke(scene, "GetPosition", &[Value::Int(ball_node as i64)]).unwrap());
    assert!((body[1] - node[1]).abs() < 1e-5, "the node follows the body: body {body:?}, node {node:?}");
    assert!(node[1].abs() < 0.01, "and it came to rest on the floor (its centre 0.5 above the surface at -0.5): {node:?}");
    assert!(events.len() >= 6 && events[0] == 0.0, "a contact started: {events:?}");
    // A ray from above finds the floor or the ball.
    let hit = floats(r.invoke(phys, "RayCast", &[w.clone(), Value::Int(5), Value::Int(5), Value::Int(0), Value::Int(0), Value::Int(-1), Value::Int(0), Value::Int(50)]).unwrap());
    assert_eq!(hit.len(), 10);
    assert_eq!(hit[0], 1.0);
    // Fixed step: whole steps only.
    r.invoke(phys, "SetFixedStep", &[w.clone(), Value::Num(0.01), Value::Int(5)]).unwrap();
    assert_eq!(int(r.invoke(phys, "Step", &[w.clone(), Value::Num(0.004)]).unwrap()), 0);
    assert_eq!(int(r.invoke(phys, "Step", &[w.clone(), Value::Num(0.007)]).unwrap()), 1);
}

#[test]
fn a_scene_is_built_loaded_posed_and_picked_through_values() {
    let mut r = reg();
    let mut assets = MapAssets::new();
    assets = assets.with_asset("models/arm.glb", glb::glb(&glb::arm_model(None)));
    r.set_assets(Arc::new(assets));
    let scene = r.create(ObjectKind::Scene3D, 1, &[]).unwrap();
    let s = |r: &mut Registry, m: &str, a: &[Value]| r.invoke(scene, m, a).unwrap();
    let root = int(s(&mut r, "LoadModel", &[Value::Text("models/arm.glb".into())]));
    assert!(root > 0);
    assert_eq!(int(s(&mut r, "GetNodeCount", &[])), 5);
    assert_eq!(int(s(&mut r, "GetTriangleCount", &[])), 6);
    let anims = match s(&mut r, "GetAnimations", &[]) {
        Value::Ints(v) => v,
        o => panic!("{o:?}"),
    };
    assert_eq!(anims.len(), 1);
    assert_eq!(num(s(&mut r, "GetAnimationDuration", &[Value::Int(anims[0])])), 1.0);
    s(&mut r, "PlayAnimation", &[Value::Int(anims[0]), Value::Int(0), Value::Num(2.0)]);
    s(&mut r, "Update", &[Value::Int(250)]); // 250 ms at double speed
    assert!((num(s(&mut r, "GetAnimationTime", &[Value::Int(anims[0])])) - 0.5).abs() < 1e-6);
    let cam = int(s(&mut r, "AddCamera", &[Value::Int(50), Value::Num(0.1), Value::Int(100)]));
    s(&mut r, "SetPosition", &[Value::Int(cam), Value::Int(0), Value::Int(1), Value::Int(8)]);
    // Canvas3D picks without a graphics device.
    let c3 = r.create(ObjectKind::Canvas3D, 1, &[Value::Int(400), Value::Int(400)]).unwrap();
    r.invoke(c3, "SetScene", &[Value::Int(scene as i64)]).unwrap();
    r.invoke(c3, "SetCamera", &[Value::Int(cam)]).unwrap();
    s(&mut r, "SetAnimationTime", &[Value::Int(anims[0]), Value::Int(0)]);
    let hit = floats(r.invoke(c3, "Pick", &[Value::Int(200), Value::Int(200)]).unwrap());
    assert_eq!(hit.len(), 9);
    assert!(hit[0] > 0.0, "the arm is at the centre: {hit:?}");
    let name = r.invoke(scene, "GetName", &[Value::Int(hit[0] as i64)]).unwrap();
    assert_eq!(name, Value::Text("arm".into()));
    // A wrong kind of handle is a sentence naming both kinds.
    let e = r.invoke(c3, "SetScene", &[Value::Int(c3 as i64)]).unwrap_err();
    assert!(e.to_string().contains("not a Scene3D"), "{e}");
    let e = r.invoke(scene, "SetTexture", &[Value::Int(1), Value::Int(0), Value::Int(c3 as i64)]).unwrap_err();
    assert!(matches!(e, GraphicsError::WrongHandleKind { .. }), "{e:?}");
    // Without a device the canvas says so instead of drawing.
    assert_eq!(r.invoke(c3, "GetBackend", &[]).unwrap(), Value::Text("None".into()));
}

#[test]
fn a_form_that_closes_takes_everything_it_made_with_it() {
    let mut r = reg();
    let a = r.create(ObjectKind::Canvas2D, 7, &[Value::Int(50), Value::Int(50)]).unwrap();
    let b = r.create(ObjectKind::ImageBuffer, 7, &[]).unwrap();
    let c = r.create(ObjectKind::Scene3D, 8, &[]).unwrap();
    let made = int(r.invoke(a, "ToImage", &[]).unwrap()) as u32;
    assert_eq!(r.owner_of(made).unwrap(), 7, "what a method makes belongs to the same form");
    assert_eq!(r.len(), 4);
    let before = r.memory_bytes();
    assert!(before > 0);
    assert_eq!(r.release_owner(7), 3);
    assert_eq!(r.len(), 1);
    for h in [a, b, made] {
        assert!(matches!(r.kind_of(h), Err(GraphicsError::ReleasedHandle(_))), "handle {h} is gone");
    }
    assert!(r.kind_of(c).is_ok(), "another form's objects stay");
    r.release_owner(8);
    assert!(r.is_empty());
    assert_eq!(r.memory_bytes(), 0);
}

#[test]
fn a_method_that_uses_its_own_handle_gets_busy_not_a_deadlock() {
    let mut r = reg();
    let img = r.create(ObjectKind::ImageBuffer, 1, &[]).unwrap();
    r.invoke(img, "Create", &[Value::Int(4), Value::Int(4)]).unwrap();
    // Composite of an image onto itself works (it copies first); DrawOnto itself is not a thing.
    r.invoke(img, "Composite", &[Value::Int(img as i64), Value::Int(1), Value::Int(1)]).unwrap();
    let e = r.invoke(img, "Composite", &[Value::Int(999_999), Value::Int(0), Value::Int(0)]).unwrap_err();
    assert!(matches!(e, GraphicsError::UnknownHandle(_) | GraphicsError::ReleasedHandle(_)), "{e:?}");
}

#[test]
fn limits_refuse_at_the_api_with_the_name_of_the_setting() {
    let mut r = Registry::new(small());
    let e = r.create(ObjectKind::Canvas2D, 1, &[Value::Int(4000), Value::Int(4000)]).unwrap_err();
    assert!(e.to_string().contains("max_canvas"), "{e}");
    let img = r.create(ObjectKind::ImageBuffer, 1, &[]).unwrap();
    let e = r.invoke(img, "Create", &[Value::Int(1000), Value::Int(1000)]).unwrap_err();
    assert!(e.to_string().contains("max_image_pixels"), "{e}");
    let scene = r.create(ObjectKind::Scene3D, 1, &[]).unwrap();
    let e = r.invoke(scene, "AddSphere", &[Value::Int(1), Value::Int(100_000)]).unwrap_err();
    assert!(e.to_string().contains("3 to 512"), "{e}");
    let mut tiny = Registry::new(Limits { max_total_bytes: 100_000, ..small() });
    let e = tiny.create(ObjectKind::Canvas2D, 1, &[Value::Int(400), Value::Int(400)]).unwrap_err();
    assert!(e.to_string().contains("max_total_bytes"), "{e}");
    assert!(tiny.is_empty(), "a refused object is not left behind");
}

// ---------------------------------------------------------------------------
// Nothing a program passes can panic
// ---------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn random_value(rng: &mut Rng, hint: Ty, handles: &[u32]) -> Value {
    // Mostly the right kind with a nasty value, sometimes the wrong kind altogether.
    let nasty_f = [0.0, -0.0, 1.0, -1.0, 0.5, 255.0, 4096.0, 1e9, -1e9, 1e300, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 2147483648.0, 1e-300];
    let nasty_i = [0i64, 1, -1, 2, 3, 7, 255, 256, 4096, 65535, 2147483647, 2147483648, 4294967295, 4294967296, i64::MAX, i64::MIN];
    if rng.below(12) == 0 {
        return match rng.below(7) {
            0 => Value::Nothing,
            1 => Value::Int(nasty_i[rng.below(nasty_i.len())]),
            2 => Value::Num(nasty_f[rng.below(nasty_f.len())]),
            3 => Value::Text("sphere".into()),
            4 => Value::Bytes(vec![0xFF; rng.below(40)]),
            5 => Value::Floats(vec![nasty_f[rng.below(nasty_f.len())]; rng.below(10)]),
            _ => Value::Ints(vec![nasty_i[rng.below(nasty_i.len())]; rng.below(10)]),
        };
    }
    match hint {
        Ty::Int => Value::Int(nasty_i[rng.below(nasty_i.len())]),
        Ty::Num => Value::Num(nasty_f[rng.below(nasty_f.len())]),
        Ty::Flag => Value::Int(rng.below(3) as i64),
        Ty::Handle => Value::Int(if rng.below(3) == 0 { nasty_i[rng.below(nasty_i.len())] } else { handles[rng.below(handles.len())] as i64 }),
        Ty::Color => match rng.below(4) {
            0 => Value::Int(nasty_i[rng.below(nasty_i.len())]),
            1 => Value::Text("#12345678".into()),
            2 => Value::Text("not a colour".into()),
            _ => Value::Int(0xFF8800FF),
        },
        Ty::Text => Value::Text(["", "x", "png", "lanczos", "multiply", "LINE", "10px sans-serif", "0:f64", "0:packed(1000,5)", "日本語 abc"][rng.below(10)].into()),
        Ty::Bytes => Value::Bytes((0..rng.below(64)).map(|_| rng.next() as u8).collect()),
        Ty::Floats => Value::Floats((0..rng.below(40)).map(|_| nasty_f[rng.below(nasty_f.len())]).collect()),
        Ty::Ints => Value::Ints((0..rng.below(40)).map(|_| nasty_i[rng.below(nasty_i.len())]).collect()),
    }
}

#[test]
fn random_calls_with_hostile_values_never_panic() {
    let iters: usize = std::env::var("GRAPHICS_FUZZ_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    let mut r = Registry::new(Limits { max_nodes: 300, max_triangles: 20_000, max_bodies: 100, max_canvas_width: 128, max_canvas_height: 128, max_canvas_pixels: 20_000, max_image_pixels: 20_000, max_texture_size: 64, max_display_list_commands: 5_000, max_path_points: 2_000, max_series_points: 5_000, ..Limits::default() });
    let fonts = Arc::new(FontSet::new());
    fonts.add_font_data(synthfont::latin("Test Latin")).unwrap();
    r.set_text_engine(fonts);
    let mut objects: Vec<(u32, ObjectKind)> = Vec::new();
    let all = Registry::all_methods();
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let (mut ok, mut refused) = (0usize, 0usize);
    let mut spent: std::collections::HashMap<String, (std::time::Duration, usize)> = std::collections::HashMap::new();
    let t = std::time::Instant::now();
    for i in 0..iters {
        // Keep a handful of live objects of every kind (a program holds some).
        if objects.len() < 14 || (i % 500 == 0 && !objects.is_empty()) {
            if i % 500 == 0 && objects.len() >= 14 {
                let (h, _) = objects.remove(rng.below(objects.len()));
                let _ = r.release(h);
            }
            let kinds = ObjectKind::available();
            let k = kinds[rng.below(kinds.len())];
            let ctor = match k {
                ObjectKind::Canvas2D | ObjectKind::Canvas3D => vec![Value::Int(64), Value::Int(48)],
                _ => vec![],
            };
            if let Ok(h) = r.create(k, (i % 3) as u32, &ctor) {
                objects.push((h, k));
            }
            continue;
        }
        let handles: Vec<u32> = objects.iter().map(|o| o.0).collect();
        let (h, kind) = objects[rng.below(objects.len())];
        let candidates: Vec<&cobolt_graphics::MethodInfo> = all.iter().filter(|m| m.object == kind && !(kind == ObjectKind::Canvas3D && matches!(m.name.as_str(), "Connect" | "Render" | "Screenshot"))).collect();
        let m = candidates[rng.below(candidates.len())];
        // Skip the calls whose cost is the point of the call (a huge blur radius is "valid" and slow).
        if matches!(m.name.as_str(), "BlurBox" | "Dilate" | "Erode" | "Convolve" | "Rotate" | "Resize" | "Sharpen" | "BlurGaussian" | "SetFixedStep" | "Step") && rng.below(4) != 0 {
            continue;
        }
        let n = if rng.below(10) == 0 { rng.below(m.params.len() + 2) } else { m.params.len() - m.params.iter().filter(|p| p.2).count() + rng.below(m.params.iter().filter(|p| p.2).count() + 1) };
        let args: Vec<Value> = (0..n).map(|k| random_value(&mut rng, m.params.get(k).map(|p| ty_of_word(p.1)).unwrap_or(Ty::Int), &handles)).collect();
        let started = std::time::Instant::now();
        let outcome = r.invoke(h, &m.name, &args);
        let e = spent.entry(format!("{}.{}", kind.name(), m.name)).or_default();
        e.0 += started.elapsed();
        e.1 += 1;
        match outcome {
            Ok(_) => ok += 1,
            Err(e) => {
                refused += 1;
                let _ = e.to_string();
                assert!(!e.to_string().is_empty());
            }
        }
        if i % 997 == 0 {
            // A stray release or a call on a dead handle is an error too.
            let _ = r.invoke(rng.next() as u32, "Release", &[]);
        }
    }
    eprintln!("API fuzz: {iters} random calls in {:?}: {ok} succeeded, {refused} refused with a sentence, no panic; {} objects, {} bytes live", t.elapsed(), r.len(), r.memory_bytes());
    let mut slow: Vec<_> = spent.into_iter().collect();
    slow.sort_by_key(|(_, (d, _))| std::cmp::Reverse(*d));
    for (name, (d, n)) in slow.iter().take(6) {
        eprintln!("  slowest: {name}: {n} call(s), {d:?} in all");
    }
    assert!(ok > 500 && refused > 500, "the fuzz exercised both outcomes ({ok} / {refused})");
}
