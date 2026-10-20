// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Canvas3D on a real adapter (WASM-018 R9, AC4, AC5): the shaders validate, the picture
//! has lights, shadows, fog, blending, textures, instancing and skinning, the golden image
//! matches within a stated tolerance, and a device that goes away is an error, not a crash.
//!
//! **These tests need a graphics adapter.** Without one each prints a line that starts with
//! `SKIPPED` and passes - loudly, so a CI log shows it - unless `GRAPHICS_REQUIRE_GPU=1` is
//! set, which turns the skip into a failure (a CI machine that is meant to have lavapipe or
//! a GPU should set it). `GOLDEN_UPDATE=1` rewrites the golden image; `GRAPHICS_DUMP=<dir>`
//! writes every picture a test makes to that folder.
//!
//! **The golden tolerance.** Two adapters (or two drivers of one) round shader maths and
//! multisample resolves differently, so the 3D golden is not compared byte for byte:
//! no channel of any pixel may differ by more than 24, and at most 1.0 % of the pixels may
//! differ by more than 6. The CPU rasteriser's goldens (canvas, plot) stay exact.
#![cfg(feature = "scene3d")]

mod common;

use cobolt_graphics::scene3d::{AlphaMode, Backend, Canvas3D, Scene3D, TextureSlot};
use cobolt_graphics::{Limits, PixelBuffer, Rgba};
use common::{assert_golden, glb, Tolerance};
use std::sync::Arc;

const TOL: Tolerance = Tolerance { max_channel: 24, max_fraction: 0.01, loose: 6 };

/// A canvas connected to an adapter, or `None` after printing why not.
fn canvas(w: u32, h: u32) -> Option<Canvas3D> {
    let mut c = Canvas3D::new(w, h, Limits::default()).unwrap();
    let backend = c.connect();
    if backend == Backend::None {
        let why = c.status().unwrap_or("no reason given").to_string();
        if std::env::var("GRAPHICS_REQUIRE_GPU").map(|v| v == "1").unwrap_or(false) {
            panic!("GRAPHICS_REQUIRE_GPU=1 but there is no graphics adapter: {why}");
        }
        eprintln!("SKIPPED (no graphics adapter): {why}");
        return None;
    }
    Some(c)
}

fn dump(name: &str, img: &PixelBuffer) {
    if let Ok(dir) = std::env::var("GRAPHICS_DUMP") {
        common::write_png(&std::path::Path::new(&dir).join(format!("{name}.png")), img);
    }
}

fn px(img: &PixelBuffer, x: i64, y: i64) -> Rgba {
    img.get(x, y).unwrap()
}

fn luma(c: Rgba) -> f32 {
    0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32
}

/// Where the world point lands in a canvas `w` x `h`, for a camera in `scene` (via the CPU frame, the same maths the GPU uses).
fn project(scene: &mut Scene3D, cam: u32, w: u32, h: u32, p: [f32; 3]) -> (i64, i64) {
    let f = scene.build_frame(Some(cam), w as f32 / h as f32).unwrap();
    let c = f.view_proj.project_point3(cobolt_graphics::scene3d::math::Vec3::from_array(p));
    (((c.x * 0.5 + 0.5) * w as f32) as i64, ((0.5 - c.y * 0.5) * h as f32) as i64)
}

fn camera(scene: &mut Scene3D, eye: [f32; 3], target: [f32; 3], fov: f32) -> u32 {
    let cam = scene.add_perspective_camera(fov, 0.1, 200.0).unwrap();
    scene.set_position(cam, eye).unwrap();
    scene.look_at(cam, target, [0.0, 1.0, 0.0]).unwrap();
    cam
}

fn color(scene: &mut Scene3D, node: u32, rgba: [f32; 4], metallic: f32, roughness: f32) {
    let m = scene.material_of(node).unwrap();
    scene.set_color(m, rgba).unwrap();
    scene.set_metallic(m, metallic).unwrap();
    scene.set_roughness(m, roughness).unwrap();
}

// ---------------------------------------------------------------------------

#[test]
fn the_shaders_are_valid_wgsl() {
    // No adapter needed: naga parses and validates what the GPU would be given.
    for (name, src) in [("pbr.wgsl", include_str!("../src/scene3d/shaders/pbr.wgsl")), ("shadow.wgsl", include_str!("../src/scene3d/shaders/shadow.wgsl"))] {
        let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
        let entries: Vec<&str> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
        eprintln!("{name}: valid, entry points {entries:?}");
    }
}

#[test]
fn the_backend_is_reported() {
    let Some(c) = canvas(64, 64) else { return };
    let b = c.backend();
    eprintln!("Canvas3D backend: {} - {}", b.name(), c.adapter_description().unwrap());
    assert_ne!(b, Backend::None);
    assert!(["WebGPU", "WebGL2", "Vulkan", "Metal", "Dx12", "OpenGL", "Software"].contains(&b.name()));
    assert!(c.status().is_none());
}

#[test]
fn with_no_connection_a_canvas_says_so_and_does_not_crash() {
    let mut c = Canvas3D::new(64, 64, Limits::default()).unwrap();
    assert_eq!(c.backend(), Backend::None);
    let mut s = Scene3D::new(Limits::default());
    s.add_box([1.0; 3]).unwrap();
    let e = c.render(&mut s).unwrap_err();
    assert!(matches!(e, cobolt_graphics::GraphicsError::NoAdapter(_)), "{e:?}");
    assert!(c.screenshot().is_err());
    // Picking needs no device.
    assert!(c.pick(&mut s, 32.0, 32.0).is_ok());
}

#[test]
fn a_lit_sphere_is_bright_where_it_faces_the_light_and_dark_where_it_does_not() {
    let Some(mut c) = canvas(200, 200) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [0.0, 0.0, 6.0], [0.0, 0.0, 0.0], 40.0);
    c.set_camera(Some(cam));
    let ball = s.add_sphere(1.5, 48).unwrap();
    color(&mut s, ball, [0.9, 0.9, 0.9, 1.0], 0.0, 0.7);
    s.set_ambient([1.0; 3], 0.0).unwrap();
    s.set_background(Rgba::rgb(0, 0, 0));
    let sun = s.add_directional_light([1.0; 3], 3.0, false).unwrap();
    s.set_position(sun, [5.0, 0.0, 0.0]).unwrap();
    s.look_at(sun, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap(); // the light comes from +X
    c.render(&mut s).unwrap();
    let img = c.screenshot().unwrap();
    dump("lit-sphere", &img);
    let lit = luma(px(&img, 120, 100));
    let dark = luma(px(&img, 80, 100));
    let edge = luma(px(&img, 100, 100));
    assert!(lit > 100.0, "the +X side faces the sun: {lit}");
    assert!(dark < 12.0, "the -X side faces away: {dark}");
    assert!(lit > edge && edge > dark, "brightness falls off round the sphere: {lit} > {edge} > {dark}");
    assert_eq!(px(&img, 3, 3), Rgba::rgb(0, 0, 0), "the background is the clear colour");
}

#[test]
fn a_box_casts_a_shadow_and_only_when_the_light_asks_for_it() {
    let Some(mut c) = canvas(240, 200) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [0.0, 9.0, 7.0], [0.0, 0.0, 0.0], 40.0);
    c.set_camera(Some(cam));
    let floor = s.add_plane(16.0, 16.0).unwrap();
    color(&mut s, floor, [0.8, 0.8, 0.8, 1.0], 0.0, 0.9);
    let cube = s.add_box([2.0, 2.0, 2.0]).unwrap();
    s.set_position(cube, [0.0, 2.0, 0.0]).unwrap();
    color(&mut s, cube, [0.8, 0.2, 0.2, 1.0], 0.0, 0.8);
    s.set_ambient([1.0; 3], 0.03).unwrap();
    let sun = s.add_directional_light([1.0; 3], 3.0, true).unwrap();
    s.set_position(sun, [6.0, 10.0, 0.0]).unwrap();
    s.look_at(sun, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    // The cube hovers 1 above the floor; the sun at 45 degrees from the +X side throws its shadow towards -X.
    let in_shadow = project(&mut s, cam, 240, 200, [-2.5, 0.0, 0.0]);
    let in_light = project(&mut s, cam, 240, 200, [4.5, 0.0, 0.0]);
    let stats = c.render(&mut s).unwrap();
    assert_eq!(stats.shadow_maps, 1);
    assert!(stats.shadow_draw_calls >= 2, "the floor and the cube are drawn into the map");
    let with = c.screenshot().unwrap();
    dump("shadow-on", &with);
    let (dark, bright) = (luma(px(&with, in_shadow.0, in_shadow.1)), luma(px(&with, in_light.0, in_light.1)));
    assert!(bright > 120.0 && dark < bright * 0.3, "the floor under the cube's shadow ({dark}) is much darker than the lit floor ({bright})");

    // The same scene with the light not casting: no shadow.
    let mut s2 = Scene3D::new(Limits::default());
    let cam2 = camera(&mut s2, [0.0, 9.0, 7.0], [0.0, 0.0, 0.0], 40.0);
    c.set_camera(Some(cam2));
    let floor = s2.add_plane(16.0, 16.0).unwrap();
    color(&mut s2, floor, [0.8, 0.8, 0.8, 1.0], 0.0, 0.9);
    let cube = s2.add_box([2.0, 2.0, 2.0]).unwrap();
    s2.set_position(cube, [0.0, 2.0, 0.0]).unwrap();
    s2.set_ambient([1.0; 3], 0.03).unwrap();
    let sun = s2.add_directional_light([1.0; 3], 3.0, false).unwrap();
    s2.set_position(sun, [6.0, 10.0, 0.0]).unwrap();
    s2.look_at(sun, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    let stats = c.render(&mut s2).unwrap();
    assert_eq!((stats.shadow_maps, stats.shadow_draw_calls), (0, 0));
    let without = c.screenshot().unwrap();
    dump("shadow-off", &without);
    let lit_everywhere = luma(px(&without, in_shadow.0, in_shadow.1));
    assert!(lit_everywhere > 120.0, "with shadows off the same floor point is lit: {lit_everywhere}");
}

#[test]
fn spot_and_point_lights_light_what_they_should() {
    let Some(mut c) = canvas(240, 200) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [0.0, 10.0, 8.0], [0.0, 0.0, 0.0], 40.0);
    c.set_camera(Some(cam));
    let floor = s.add_plane(30.0, 30.0).unwrap();
    color(&mut s, floor, [0.9, 0.9, 0.9, 1.0], 0.0, 0.9);
    s.set_ambient([1.0; 3], 0.0).unwrap();
    s.set_background(Rgba::rgb(0, 0, 0));
    // A spot light straight down at x = -5 with a 20 degree cone; a red point light at x = +5.
    let spot = s.add_spot_light([1.0; 3], 120.0, 30.0, 10.0, 20.0, true).unwrap();
    s.set_position(spot, [-5.0, 6.0, 0.0]).unwrap();
    s.look_at(spot, [-5.0, 0.0, 0.0], [0.0, 0.0, -1.0]).unwrap();
    let bulb = s.add_point_light([1.0, 0.1, 0.1], 5.0, 12.0).unwrap();
    s.set_position(bulb, [5.0, 3.0, 0.0]).unwrap();
    c.render(&mut s).unwrap();
    let img = c.screenshot().unwrap();
    dump("spot-and-point", &img);
    let mut at = |p: [f32; 3]| {
        let (x, y) = project(&mut s, cam, 240, 200, p);
        assert!((0..240).contains(&x) && (0..200).contains(&y), "{p:?} is not in the picture: ({x}, {y})");
        px(&img, x, y)
    };
    let under_spot = at([-5.0, 0.0, 0.0]);
    let outside_cone = at([-5.0, 0.0, -5.0]);
    let under_bulb = at([5.0, 0.0, 0.0]);
    assert!(luma(under_spot) > 150.0 && under_spot.r.abs_diff(under_spot.b) < 20, "white under the spot: {under_spot:?}");
    assert!(luma(outside_cone) < 15.0, "outside the cone it is dark: {outside_cone:?}");
    assert!(under_bulb.r > 120 && under_bulb.g < under_bulb.r / 2, "red under the point light: {under_bulb:?}");
}

#[test]
fn fog_blends_distant_things_to_its_colour() {
    let Some(mut c) = canvas(160, 120) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [0.0, 1.0, 5.0], [0.0, 1.0, 0.0], 50.0);
    c.set_camera(Some(cam));
    let near = s.add_box([1.0; 3]).unwrap();
    s.set_position(near, [-1.2, 1.0, 0.0]).unwrap();
    color(&mut s, near, [1.0, 0.0, 0.0, 1.0], 0.0, 1.0);
    let far = s.add_box([1.0; 3]).unwrap();
    s.set_position(far, [1.2, 1.0, -60.0]).unwrap();
    color(&mut s, far, [1.0, 0.0, 0.0, 1.0], 0.0, 1.0);
    let far2 = s.add_box([8.0, 8.0, 1.0]).unwrap();
    s.set_position(far2, [8.0, 1.0, -60.0]).unwrap();
    color(&mut s, far2, [1.0, 0.0, 0.0, 1.0], 0.0, 1.0);
    let sun = s.add_directional_light([1.0; 3], 3.0, false).unwrap();
    s.set_position(sun, [0.0, 5.0, 10.0]).unwrap();
    s.look_at(sun, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    s.set_background(Rgba::rgb(0, 0, 0));
    s.set_fog(Some(cobolt_graphics::scene3d::Fog { color: [0.0, 0.0, 1.0], mode: cobolt_graphics::scene3d::FogMode::Linear { near: 5.0, far: 40.0 } })).unwrap();
    c.render(&mut s).unwrap();
    let img = c.screenshot().unwrap();
    dump("fog", &img);
    // The big far box (8 wide at 65 away) is fully fogged: blue. The near box keeps its red.
    let near_px = px(&img, project(&mut s, cam, 160, 120, [-1.2, 1.0, 0.5]).0, project(&mut s, cam, 160, 120, [-1.2, 1.0, 0.5]).1);
    let far_px = px(&img, project(&mut s, cam, 160, 120, [8.0, 1.0, -59.4]).0, project(&mut s, cam, 160, 120, [8.0, 1.0, -59.4]).1);
    assert!(near_px.r > 150 && near_px.b < 90 && near_px.g < 90, "the near box is red: {near_px:?}");
    assert!(far_px.b > 200 && far_px.r < 20, "the far box is all fog: {far_px:?}");
}

#[test]
fn a_transparent_surface_blends_over_what_is_behind_it() {
    let Some(mut c) = canvas(120, 120) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [0.0, 0.0, 5.0], [0.0, 0.0, 0.0], 40.0);
    c.set_camera(Some(cam));
    s.set_background(Rgba::rgb(0, 0, 255));
    let sheet = s.add_box([3.0, 3.0, 0.1]).unwrap();
    let m = s.material_of(sheet).unwrap();
    s.set_color(m, [1.0, 0.0, 0.0, 0.5]).unwrap();
    s.set_alpha_mode(m, AlphaMode::Blend, 0.5).unwrap();
    s.set_unlit(m, true).unwrap();
    c.render(&mut s).unwrap();
    let img = c.screenshot().unwrap();
    dump("blend", &img);
    let p = px(&img, 60, 60);
    // Unlit red at half opacity over blue, blended in the output encoding: roughly (128, 0, 128).
    assert!(p.r > 100 && p.r < 160 && p.b > 100 && p.b < 160 && p.g < 20, "{p:?}");
    assert_eq!(p.a, 255);
    // A transparent background gives transparent pixels (and the screenshot is straight alpha).
    s.set_background(Rgba::TRANSPARENT);
    c.render(&mut s).unwrap();
    let img = c.screenshot().unwrap();
    assert_eq!(px(&img, 2, 2).a, 0);
    let p = px(&img, 60, 60);
    assert!(p.a > 120 && p.a < 135 && p.r > 240 && p.b < 20, "the half-transparent red over nothing is straight-alpha red at alpha 128: {p:?}");
}

#[test]
fn textures_reach_the_surface() {
    let Some(mut c) = canvas(120, 120) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [0.0, 0.0, 4.0], [0.0, 0.0, 0.0], 40.0);
    c.set_camera(Some(cam));
    // A plane stood up to face the camera, with a 2x2 texture: red, green / blue, white.
    let board = s.add_plane(3.0, 3.0).unwrap();
    // Tilted back to stand upright with the texture's top edge up (its normal then faces away: double-sided).
    s.set_rotation(board, [-90.0, 0.0, 0.0]).unwrap();
    let m = s.material_of(board).unwrap();
    s.set_unlit(m, true).unwrap();
    s.set_double_sided(m, true).unwrap();
    let tex = Arc::new(PixelBuffer::from_rgba(2, 2, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]).unwrap());
    s.set_texture(m, TextureSlot::BaseColor, Some(tex)).unwrap();
    // Nearest-ish: look at the middle of each quarter of the plane.
    s.set_background(Rgba::rgb(10, 10, 10));
    c.render(&mut s).unwrap();
    let img = c.screenshot().unwrap();
    dump("texture", &img);
    let unit = 120.0 / (2.0 * 4.0 * (20.0f32.to_radians()).tan());
    let at = |dx: f32, dy: f32| px(&img, (60.0 + dx * unit) as i64, (60.0 + dy * unit) as i64);
    let (tl, tr, bl, br) = (at(-0.75, -0.75), at(0.75, -0.75), at(-0.75, 0.75), at(0.75, 0.75));
    assert!(tl.r > 200 && tl.g < 60 && tl.b < 60, "top left is red: {tl:?}");
    assert!(tr.g > 200 && tr.r < 60, "top right is green: {tr:?}");
    assert!(bl.b > 200 && bl.r < 60, "bottom left is blue: {bl:?}");
    assert!(br.r > 200 && br.g > 200 && br.b > 200, "bottom right is white: {br:?}");
}

#[test]
fn ten_thousand_instances_are_one_draw_call_and_the_culled_ones_are_not_drawn() {
    let Some(mut c) = canvas(240, 160) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [0.0, 3.0, 12.0], [0.0, 0.0, 0.0], 45.0);
    c.set_camera(Some(cam));
    let n = s.add_box([0.4, 0.4, 0.4]).unwrap();
    let mut m = Vec::new();
    for i in 0..100 {
        for j in 0..100 {
            // A 100 x 100 grid, 1 apart, centred on the origin: much of it is outside the view.
            let t = cobolt_graphics::scene3d::math::Mat4::from_translation(cobolt_graphics::scene3d::math::Vec3::new(i as f32 - 50.0, 0.0, j as f32 - 50.0));
            m.extend_from_slice(&t.to_cols_array());
        }
    }
    s.set_instances(n, &m).unwrap();
    let sun = s.add_directional_light([1.0; 3], 3.0, false).unwrap();
    s.set_position(sun, [0.0, 10.0, 5.0]).unwrap();
    s.look_at(sun, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    let t = std::time::Instant::now();
    let stats = c.render(&mut s).unwrap();
    let took = t.elapsed();
    let fs = c.last_frame_stats().unwrap();
    eprintln!("10,000 instances: {} drawn, {} culled, {} draw call(s), {:?} to render", fs.instances_drawn, fs.instances_culled, stats.draw_calls, took);
    assert_eq!(stats.draw_calls, 1, "one mesh, one draw call, however many copies");
    assert_eq!(stats.instances as usize, fs.instances_drawn);
    assert!(fs.instances_culled > 5000 && fs.instances_drawn > 100, "{fs:?}");
    assert_eq!(fs.instances_total, 10_000);
    let img = c.screenshot().unwrap();
    dump("instances", &img);
    let lit = (0..240).flat_map(|x| (0..160).map(move |y| (x, y))).filter(|&(x, y)| luma(px(&img, x, y)) > 60.0).count();
    assert!(lit > 3000, "a field of cubes: {lit} lit pixels");
}

#[test]
fn a_skinned_animated_model_renders_and_its_pose_changes_the_picture() {
    let Some(mut c) = canvas(200, 200) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [0.5, 1.0, 9.0], [0.5, 1.0, 0.0], 40.0);
    c.set_camera(Some(cam));
    let info = s.load_model_bytes(&glb::glb(&glb::arm_model(None))).unwrap();
    s.set_background(Rgba::rgb(0, 0, 0));
    s.set_ambient([1.0; 3], 1.0).unwrap();
    let arm = s.find_node("arm").unwrap();
    let m = s.material_of(arm).unwrap();
    s.set_unlit(m, true).unwrap();
    s.set_double_sided(m, true).unwrap();
    let anim = info.animations[0];
    let mut renders = Vec::new();
    for t in [0.0f32, 0.5, 1.0] {
        s.set_animation_time(anim, t).unwrap();
        c.render(&mut s).unwrap();
        let img = c.screenshot().unwrap();
        dump(&format!("skinned-{t}"), &img);
        let lit = (0..200).flat_map(|x| (0..200).map(move |y| (x, y))).filter(|&(x, y)| luma(px(&img, x, y)) > 100.0).count();
        renders.push((img, lit));
    }
    assert!(renders.iter().all(|(_, lit)| *lit > 200), "the arm is on screen at every pose: {:?}", renders.iter().map(|r| r.1).collect::<Vec<_>>());
    // The picture at the three poses differs: the GPU really skinned.
    let diff = |a: &PixelBuffer, b: &PixelBuffer| a.as_bytes().iter().zip(b.as_bytes()).filter(|(x, y)| x != y).count();
    assert!(diff(&renders[0].0, &renders[2].0) > 1000, "straight and bent arms look different");
    assert!(diff(&renders[0].0, &renders[1].0) > 500 && diff(&renders[1].0, &renders[2].0) > 500);
    // In the bent pose the left of the picture is occupied where the straight arm left it empty.
    let left_lit = |img: &PixelBuffer| (0..80).flat_map(|x| (60..120).map(move |y| (x, y))).filter(|&(x, y)| luma(px(img, x, y)) > 100.0).count();
    assert!(left_lit(&renders[2].0) > left_lit(&renders[0].0) + 100, "the forearm swung left");
}

#[test]
fn multisampling_smooths_edges() {
    let Some(mut c) = canvas(120, 120) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [0.0, 0.0, 5.0], [0.0, 0.0, 0.0], 40.0);
    c.set_camera(Some(cam));
    let ball = s.add_sphere(1.5, 48).unwrap();
    let m = s.material_of(ball).unwrap();
    s.set_unlit(m, true).unwrap();
    s.set_color(m, [1.0, 1.0, 1.0, 1.0]).unwrap();
    s.set_background(Rgba::rgb(0, 0, 0));
    let mut counts = Vec::new();
    for msaa in [1u32, 4] {
        c.set_msaa(msaa).unwrap();
        c.render(&mut s).unwrap();
        let img = c.screenshot().unwrap();
        dump(&format!("msaa{msaa}"), &img);
        let between = (0..120).flat_map(|x| (0..120).map(move |y| (x, y))).filter(|&(x, y)| {
            let l = luma(px(&img, x, y));
            l > 30.0 && l < 225.0
        });
        counts.push(between.count());
    }
    assert!(counts[1] > counts[0], "4x multisampling gives more in-between edge pixels than none: {counts:?}");
    assert!(c.set_msaa(2).is_err());
}

#[test]
fn rendering_twice_gives_the_same_bytes_and_a_resize_changes_the_picture_size() {
    let Some(mut c) = canvas(96, 64) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [2.0, 2.0, 5.0], [0.0, 0.0, 0.0], 40.0);
    c.set_camera(Some(cam));
    let a = s.add_torus(1.0, 0.3, 16, 32).unwrap();
    color(&mut s, a, [0.2, 0.6, 0.9, 1.0], 0.8, 0.3);
    let sun = s.add_directional_light([1.0; 3], 3.0, true).unwrap();
    s.set_position(sun, [4.0, 6.0, 3.0]).unwrap();
    s.look_at(sun, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    c.render(&mut s).unwrap();
    let one = c.screenshot().unwrap();
    c.render(&mut s).unwrap();
    let two = c.screenshot().unwrap();
    assert_eq!(one.as_bytes(), two.as_bytes(), "the same scene gives the same picture on the same device");
    assert_eq!((one.width(), one.height()), (96, 64));
    c.resize(130, 50).unwrap();
    c.render(&mut s).unwrap();
    let three = c.screenshot().unwrap();
    assert_eq!((three.width(), three.height()), (130, 50));
    assert!(c.resize(0, 5).is_err());
    assert!(c.resize(100_000, 5).is_err());
}

#[test]
fn the_pick_and_the_picture_agree() {
    let Some(mut c) = canvas(200, 200) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [0.0, 0.0, 8.0], [0.0, 0.0, 0.0], 40.0);
    c.set_camera(Some(cam));
    let red = s.add_sphere(1.2, 32).unwrap();
    s.set_position(red, [-2.0, 0.0, 0.0]).unwrap();
    let green = s.add_box([2.0, 2.0, 2.0]).unwrap();
    s.set_position(green, [2.0, 0.0, 0.0]).unwrap();
    for (n, col) in [(red, [1.0, 0.0, 0.0, 1.0]), (green, [0.0, 1.0, 0.0, 1.0])] {
        let m = s.material_of(n).unwrap();
        s.set_unlit(m, true).unwrap();
        s.set_color(m, col).unwrap();
    }
    s.set_background(Rgba::rgb(0, 0, 0));
    c.render(&mut s).unwrap();
    let img = c.screenshot().unwrap();
    for (x, y) in [(60.0f32, 100.0f32), (140.0, 100.0), (100.0, 100.0), (60.0, 60.0), (150.0, 130.0)] {
        let hit = c.pick(&mut s, x, y).unwrap();
        let p = px(&img, x as i64, y as i64);
        match hit {
            Some(h) if h.node == red => assert!(p.r > 150 && p.g < 40, "({x},{y}) picks the red sphere and is red: {p:?}"),
            Some(h) if h.node == green => assert!(p.g > 150 && p.r < 40, "({x},{y}) picks the green box and is green: {p:?}"),
            Some(h) => panic!("picked an unexpected node {h:?}"),
            None => assert!(luma(p) < 10.0, "({x},{y}) picks nothing and is background: {p:?}"),
        }
    }
}

#[test]
fn a_lost_device_is_an_error_and_an_event_never_a_crash() {
    let Some(mut c) = canvas(64, 64) else { return };
    let mut s = Scene3D::new(Limits::default());
    s.add_box([1.0; 3]).unwrap();
    c.render(&mut s).unwrap();
    c.renderer().unwrap().gpu().device().destroy();
    // The device reports its loss once the queue has drained (the browser's event loop does this by itself).
    c.renderer().unwrap().gpu().wait_idle();
    let e = c.render(&mut s);
    assert!(e.is_err(), "rendering on a destroyed device fails");
    assert_eq!(c.backend(), Backend::None, "and the canvas knows it has no device any more");
    assert!(c.status().unwrap().contains("lost"), "{:?}", c.status());
    assert!(c.take_event().is_some(), "one event for the host to raise");
    // The control can be connected again.
    let b = c.connect();
    assert_ne!(b, Backend::None);
    c.render(&mut s).unwrap();
}

#[test]
fn the_reference_scene_matches_its_golden_image() {
    let Some(mut c) = canvas(320, 240) else { return };
    let mut s = Scene3D::new(Limits::default());
    let cam = camera(&mut s, [4.0, 3.5, 6.5], [0.0, 0.8, 0.0], 45.0);
    c.set_camera(Some(cam));
    s.set_background_gradient(Rgba::rgb(30, 60, 110), Rgba::rgb(170, 190, 210));
    s.set_ambient([0.8, 0.85, 1.0], 0.12).unwrap();

    let floor = s.add_plane(16.0, 16.0).unwrap();
    color(&mut s, floor, [0.75, 0.75, 0.72, 1.0], 0.0, 0.85);
    let ball = s.add_sphere(1.0, 48).unwrap();
    s.set_position(ball, [-1.6, 1.0, 0.0]).unwrap();
    color(&mut s, ball, [0.9, 0.15, 0.1, 1.0], 0.9, 0.25);
    let cube = s.add_box([1.4, 1.4, 1.4]).unwrap();
    s.set_position(cube, [1.4, 0.7, 0.4]).unwrap();
    s.set_rotation(cube, [0.0, 25.0, 0.0]).unwrap();
    color(&mut s, cube, [0.2, 0.4, 0.9, 1.0], 0.0, 0.6);
    let ring = s.add_torus(0.9, 0.25, 24, 48).unwrap();
    s.set_position(ring, [0.0, 1.4, -2.4]).unwrap();
    s.set_rotation(ring, [70.0, 0.0, 0.0]).unwrap();
    color(&mut s, ring, [0.2, 0.8, 0.3, 1.0], 0.2, 0.4);
    let m = s.material_of(ring).unwrap();
    s.set_emissive(m, [0.02, 0.1, 0.03]).unwrap();
    let post = s.add_cylinder(0.4, 1.6, 32).unwrap();
    s.set_position(post, [-3.4, 0.8, -2.0]).unwrap();
    color(&mut s, post, [0.9, 0.8, 0.2, 1.0], 0.5, 0.35);
    let cone = s.add_cone(0.6, 1.5, 32).unwrap();
    s.set_position(cone, [3.4, 0.75, -2.0]).unwrap();
    color(&mut s, cone, [0.8, 0.3, 0.8, 1.0], 0.0, 0.5);

    let sun = s.add_directional_light([1.0, 0.95, 0.85], 1.1, true).unwrap();
    s.set_position(sun, [5.0, 8.0, 4.0]).unwrap();
    s.look_at(sun, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    let lamp = s.add_spot_light([1.0, 1.0, 1.0], 22.0, 20.0, 15.0, 30.0, true).unwrap();
    s.set_position(lamp, [-4.0, 6.0, 3.0]).unwrap();
    s.look_at(lamp, [-1.6, 0.5, 0.0], [0.0, 1.0, 0.0]).unwrap();
    let bulb = s.add_point_light([1.0, 0.6, 0.3], 3.0, 8.0).unwrap();
    s.set_position(bulb, [1.4, 2.2, 1.6]).unwrap();

    let stats = c.render(&mut s).unwrap();
    assert_eq!(stats.shadow_maps, 2, "the sun and the spot light both cast");
    let img = c.screenshot().unwrap();
    dump("reference", &img);
    assert_golden("scene3d-reference", &img, TOL);
}
