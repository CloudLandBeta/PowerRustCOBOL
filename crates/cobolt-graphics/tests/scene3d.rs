// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Scene3D without a GPU (WASM-018 R9, R15, R20, AC4, AC10): the glTF loader with
//! materials, textures, skins and animation; culling, shadows and picking on a
//! loaded model; and the loader's behaviour on hostile files.
#![cfg(feature = "scene3d-core")]

mod common;

use std::collections::HashMap;

use cobolt_graphics::resources::AssetHost;
use cobolt_graphics::scene3d::math::{Mat4, Vec3};
use cobolt_graphics::scene3d::{AlphaMode, Scene3D, TextureSlot};
use cobolt_graphics::{GraphicsError, Limits};
use common::glb;

fn new_scene() -> Scene3D {
    Scene3D::new(Limits::default())
}

struct MapAssets(HashMap<String, Vec<u8>>);

impl AssetHost for MapAssets {
    fn asset(&self, name: &str) -> Option<Vec<u8>> {
        self.0.get(name).cloned()
    }
    fn save(&self, _name: &str, _bytes: &[u8]) -> Result<(), String> {
        Err("read-only".into())
    }
}

#[test]
fn a_glb_with_skin_animation_and_material_loads_completely() {
    let mut s = new_scene();
    let info = s.load_model_bytes(&glb::glb(&glb::arm_model(None))).unwrap();
    assert!(info.warnings.is_empty(), "{:?}", info.warnings);
    // The root group, shoulder, elbow, the arm mesh node and the crate.
    assert_eq!(info.nodes.len(), 5);
    assert_eq!(s.node_count(), 5);
    assert_eq!(info.triangles, 4 + 2);
    assert_eq!(info.materials, 1);
    assert_eq!(info.animations.len(), 1);
    assert_eq!(s.name(info.root).unwrap(), "arm scene");

    // The crate: its transform, its material and its texture came through.
    let crate_node = s.find_node("crate").unwrap();
    assert_eq!(s.position(crate_node).unwrap(), [3.0, 0.0, 0.0]);
    let m = s.material_of(crate_node).unwrap();
    let mat = s.material(m).unwrap().clone();
    assert_eq!(mat.base_color, [1.0, 0.5, 0.25, 1.0]);
    assert_eq!((mat.metallic, mat.roughness), (1.0, 0.2));
    assert_eq!(mat.emissive, [0.1, 0.2, 0.3]);
    assert!(mat.double_sided);
    assert_eq!(mat.alpha_mode, AlphaMode::Opaque);
    let tex = mat.texture(TextureSlot::BaseColor).expect("the base colour texture was decoded");
    assert_eq!((tex.width(), tex.height()), (2, 2));
    assert_eq!(tex.get(1, 0).unwrap().g, 255, "the green texel");

    // The animation: one second, driving the elbow.
    let anim = info.animations[0];
    assert_eq!(s.animation_name(anim).unwrap(), "bend");
    assert_eq!(s.animation_duration(anim).unwrap(), 1.0);
    assert_eq!(s.find_animation("bend"), Some(anim));
}

#[test]
fn the_skeleton_bends_the_mesh_where_the_animation_says() {
    let mut s = new_scene();
    let info = s.load_model_bytes(&glb::glb(&glb::arm_model(None))).unwrap();
    let arm = s.find_node("arm").unwrap();
    let anim = info.animations[0];
    // Bind pose: the top vertex (0.2, 2, 0) stays where it is.
    s.set_animation_time(anim, 0.0).unwrap();
    let frame = s.build_frame(None, 1.0).unwrap();
    let d = frame.draws.iter().find(|d| d.node == arm).expect("the arm is drawn");
    let joints = d.joints.as_ref().expect("a skinned mesh carries its joint matrices");
    assert_eq!(joints.len(), 2);
    let posed = |j: &[Mat4], v: Vec3, w: [f32; 2]| (j[0].transform_point3(v) * w[0]) + (j[1].transform_point3(v) * w[1]);
    let top = Vec3::new(0.2, 2.0, 0.0);
    assert!((posed(joints, top, [0.0, 1.0]) - top).length() < 1e-5);
    // Fully bent: the elbow turned 90 degrees about Z, so the top vertex swings to (-1, 1.2, 0).
    s.set_animation_time(anim, 1.0).unwrap();
    let frame = s.build_frame(None, 1.0).unwrap();
    let joints = frame.draws.iter().find(|d| d.node == arm).unwrap().joints.clone().unwrap();
    let p = posed(&joints, top, [0.0, 1.0]);
    assert!((p - Vec3::new(-1.0, 1.2, 0.0)).length() < 1e-4, "{p:?}");
    // Halfway: a 45 degree bend.
    s.set_animation_time(anim, 0.5).unwrap();
    let frame = s.build_frame(None, 1.0).unwrap();
    let joints = frame.draws.iter().find(|d| d.node == arm).unwrap().joints.clone().unwrap();
    let p = posed(&joints, top, [0.0, 1.0]);
    // Rotate (0.2, 1) by 45 degrees about the elbow at (0, 1): (0.2 cos45 - 1 sin45, 0.2 sin45 + 1 cos45).
    let (c, sn) = (std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2);
    let want = Vec3::new(0.2 * c - 1.0 * sn, 1.0 + 0.2 * sn + 1.0 * c, 0.0);
    assert!((p - want).length() < 1e-4, "{p:?} vs {want:?}");
    // The playing clock moves it too.
    s.set_animation_time(anim, 0.0).unwrap();
    s.play_animation(anim, false, 1.0).unwrap();
    s.update(0.25).unwrap();
    assert_eq!(s.animation_time(anim).unwrap(), 0.25);
}

#[test]
fn a_skinned_model_is_picked_where_it_is_posed() {
    let mut s = new_scene();
    let info = s.load_model_bytes(&glb::glb(&glb::arm_model(None))).unwrap();
    let cam = s.add_perspective_camera(50.0, 0.1, 100.0).unwrap();
    s.set_position(cam, [0.0, 1.0, 8.0]).unwrap();
    let arm = s.find_node("arm").unwrap();
    // Bind pose: a column up the middle of the picture.
    s.set_animation_time(info.animations[0], 0.0).unwrap();
    let hit = s.pick(Some(cam), 400, 400, 200.0, 200.0).unwrap().expect("the arm at the centre");
    assert_eq!(hit.node, arm);
    // Bent 90 degrees: the upper half has swung to the left, so the point above the centre is empty ...
    s.set_animation_time(info.animations[0], 1.0).unwrap();
    let unit = 400.0 / (2.0 * 8.0 * (25.0f32.to_radians()).tan()); // pixels per world unit at distance 8
    assert!(s.pick(Some(cam), 400, 400, 200.0, 200.0 - 0.8 * unit).unwrap().is_none(), "nothing above the elbow any more");
    // ... and the forearm lies along the left: (-0.5, 1.1) is on it.
    let hit = s.pick(Some(cam), 400, 400, 200.0 - 0.5 * unit, 200.0 - 0.1 * unit).unwrap().expect("the forearm");
    assert_eq!(hit.node, arm);
}

#[test]
fn a_gltf_with_a_data_uri_and_one_with_files_beside_it_load_the_same() {
    let reference = {
        let mut s = new_scene();
        s.load_model_bytes(&glb::glb(&glb::arm_model(None))).unwrap().triangles
    };
    // The buffer inline as base64.
    let mut s = new_scene();
    let info = s.load_model_bytes(&glb::gltf_with_data_uri()).unwrap();
    assert_eq!((info.triangles, info.nodes.len()), (reference, 5));
    assert_eq!(info.materials, 1);
    // The buffer in a separate file, found through the host.
    let model = glb::arm_model(Some("arm%20data.bin"));
    let mut assets = HashMap::new();
    assets.insert("models/arm.gltf".to_string(), serde_json::to_vec(&model.json).unwrap());
    assets.insert("models/arm data.bin".to_string(), model.bin.clone());
    let mut s = new_scene();
    let info = s.load_model_asset("models/arm.gltf", &MapAssets(assets.clone())).unwrap();
    assert_eq!(info.triangles, reference);
    // Without the file the error names it.
    assets.remove("models/arm data.bin");
    let mut s = new_scene();
    let e = s.load_model_asset("models/arm.gltf", &MapAssets(assets)).unwrap_err();
    assert!(e.to_string().contains("arm data.bin"), "{e}");
    assert_eq!(s.node_count(), 0, "a failed load leaves nothing behind");
}

#[test]
fn a_model_that_fails_part_way_leaves_the_scene_as_it_was() {
    let mut s = new_scene();
    s.add_box([1.0; 3]).unwrap();
    let (nodes, tris) = (s.node_count(), s.triangle_count());
    // Cut the binary chunk: the buffer is then shorter than the JSON says.
    let mut bad = glb::glb(&glb::arm_model(None));
    bad.truncate(bad.len() - 200);
    assert!(s.load_model_bytes(&bad).is_err());
    assert_eq!((s.node_count(), s.triangle_count()), (nodes, tris));
    // A reference outside the buffer.
    let mut m = glb::arm_model(None);
    m.json["bufferViews"][2]["byteOffset"] = serde_json::json!(1_000_000);
    assert!(s.load_model_bytes(&glb::glb(&m)).is_err());
    assert_eq!((s.node_count(), s.triangle_count()), (nodes, tris));
}

#[test]
fn limits_refuse_a_model_that_is_too_big_before_building_it() {
    let bytes = glb::glb(&glb::arm_model(None));
    let mut s = Scene3D::new(Limits { max_model_bytes: 100, ..Limits::default() });
    let e = s.load_model_bytes(&bytes).unwrap_err();
    assert!(e.to_string().contains("max_model_bytes"), "{e}");
    let mut s = Scene3D::new(Limits { max_triangles: 3, ..Limits::default() });
    let e = s.load_model_bytes(&bytes).unwrap_err();
    assert!(e.to_string().contains("max_triangles"), "{e}");
    assert_eq!(s.node_count(), 0);
    let mut s = Scene3D::new(Limits { max_nodes: 3, ..Limits::default() });
    let e = s.load_model_bytes(&bytes).unwrap_err();
    assert!(e.to_string().contains("max_nodes"), "{e}");
    assert_eq!(s.node_count(), 0, "the nodes made before the limit hit were taken back");
    // A texture past the texture limit is left out with a warning, not an error.
    let mut s = Scene3D::new(Limits { max_image_pixels: 3, ..Limits::default() });
    let info = s.load_model_bytes(&bytes).unwrap();
    assert!(info.warnings.iter().any(|w| w.contains("base colour")), "{:?}", info.warnings);
}

#[test]
fn a_texture_bomb_inside_a_model_is_refused_from_its_header() {
    // A PNG header claiming 60000 x 60000 pixels, inside an otherwise valid model.
    let mut bomb = Vec::new();
    bomb.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R']);
    bomb.extend_from_slice(&60000u32.to_be_bytes());
    bomb.extend_from_slice(&60000u32.to_be_bytes());
    bomb.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    let mut m = glb::arm_model(None);
    let len = m.bin.len();
    m.bin.extend_from_slice(&bomb);
    m.json["bufferViews"].as_array_mut().unwrap().push(serde_json::json!({"buffer": 0, "byteOffset": len, "byteLength": bomb.len()}));
    let view = m.json["bufferViews"].as_array().unwrap().len() - 1;
    m.json["images"][0]["bufferView"] = serde_json::json!(view);
    m.json["buffers"][0]["byteLength"] = serde_json::json!(m.bin.len());
    let mut s = new_scene();
    let t = std::time::Instant::now();
    let info = s.load_model_bytes(&glb::glb(&m)).unwrap();
    assert!(t.elapsed().as_secs() < 5, "refused from the header, not decoded");
    assert!(info.warnings.iter().any(|w| w.contains("could not be decoded")), "{:?}", info.warnings);
    let crate_node = s.find_node("crate").unwrap();
    let crate_material = s.material_of(crate_node).unwrap();
    assert!(s.material(crate_material).unwrap().base_color_texture.is_none());
}

#[test]
fn network_references_are_not_followed() {
    let mut m = glb::arm_model(Some("https://example.com/arm.bin"));
    m.json["buffers"][0]["uri"] = serde_json::json!("https://example.com/arm.bin");
    let mut s = new_scene();
    let e = s.load_model_bytes(&serde_json::to_vec(&m.json).unwrap()).unwrap_err();
    assert!(e.to_string().contains("network"), "{e}");
}

#[test]
fn a_sparse_accessor_cannot_ask_for_a_billion_vertices() {
    let mut m = glb::arm_model(None);
    m.json["accessors"][0]["count"] = serde_json::json!(4_000_000_000u64);
    let mut s = new_scene();
    let t = std::time::Instant::now();
    assert!(s.load_model_bytes(&glb::glb(&m)).is_err());
    assert!(t.elapsed().as_secs() < 5);
}

#[test]
fn a_deep_hierarchy_is_not_recursion() {
    // 50,000 nodes, each the child of the one before: a recursive loader would overflow its stack.
    let mut m = glb::arm_model(None);
    let n = 50_000usize;
    let mut nodes = vec![serde_json::json!({"name": "n0", "children": [1]})];
    for i in 1..n {
        nodes.push(if i + 1 < n { serde_json::json!({"children": [i + 1]}) } else { serde_json::json!({}) });
    }
    m.json["nodes"] = serde_json::Value::Array(nodes);
    m.json["scenes"] = serde_json::json!([{"nodes": [0]}]);
    m.json["skins"] = serde_json::json!([]);
    m.json["animations"] = serde_json::json!([]);
    m.json["meshes"] = serde_json::json!([]);
    let mut s = new_scene();
    let info = s.load_model_bytes(&glb::glb(&m)).unwrap();
    assert_eq!(info.nodes.len(), n + 1);
    // And with the default node limit it is refused cleanly when too deep or too many.
    let mut s = Scene3D::new(Limits { max_nodes: 1000, ..Limits::default() });
    assert!(s.load_model_bytes(&glb::glb(&m)).is_err());
    assert_eq!(s.node_count(), 0);
}

/// A tiny deterministic generator for the mutation tests.
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

#[test]
fn mutated_models_never_panic() {
    let iters: usize = std::env::var("GRAPHICS_FUZZ_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(4000);
    let seeds = [glb::glb(&glb::arm_model(None)), glb::gltf_with_data_uri()];
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut loaded, mut refused) = (0usize, 0usize);
    let t = std::time::Instant::now();
    for i in 0..iters {
        let mut bytes = seeds[i % seeds.len()].clone();
        match rng.below(5) {
            0 => {
                // Flip a handful of bytes.
                for _ in 0..1 + rng.below(8) {
                    let at = rng.below(bytes.len());
                    bytes[at] ^= 1 << rng.below(8);
                }
            }
            1 => bytes.truncate(rng.below(bytes.len())),
            2 => {
                // Overwrite a run with 0xFF or 0x00.
                let at = rng.below(bytes.len());
                let n = rng.below(32).min(bytes.len() - at);
                let v = if rng.below(2) == 0 { 0xFF } else { 0 };
                bytes[at..at + n].fill(v);
            }
            3 => {
                // Insert random bytes.
                let at = rng.below(bytes.len());
                for _ in 0..1 + rng.below(16) {
                    bytes.insert(at, rng.next() as u8);
                }
            }
            _ => {
                // Change a digit in the JSON text (counts, indices, offsets).
                let digits: Vec<usize> = bytes.iter().enumerate().filter(|(_, b)| b.is_ascii_digit()).map(|(i, _)| i).take(4000).collect();
                if !digits.is_empty() {
                    let at = digits[rng.below(digits.len())];
                    bytes[at] = b'0' + rng.below(10) as u8;
                }
            }
        }
        let mut s = Scene3D::new(Limits { max_triangles: 2_000_000, max_nodes: 10_000, ..Limits::default() });
        match s.load_model_bytes(&bytes) {
            Ok(_) => {
                loaded += 1;
                // Whatever loaded must be drawable and pickable without a panic.
                let frame = s.build_frame(None, 1.0).expect("a loaded model can be framed");
                let _ = s.pick(None, 64, 64, 32.0, 32.0);
                let _ = frame;
            }
            Err(e) => {
                assert!(!e.to_string().contains("INTERNAL ERROR"), "mutation {i} got past the loader's checks and panicked inside the glTF reader: {e}");
                refused += 1;
            }
        }
    }
    eprintln!("glTF mutation: {iters} mutated files in {:?}: {loaded} loaded, {refused} refused, no panic", t.elapsed());
    assert!(refused > 0 && loaded > 0, "the mutations hit both outcomes");
}

#[test]
fn random_input_with_a_gltf_signature_never_panics() {
    let mut rng = Rng(0xDEAD_BEEF_1234_5678);
    let n = std::env::var("GRAPHICS_FUZZ_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(3000usize);
    for i in 0..n {
        let len = 12 + rng.below(600);
        let mut b: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        if i % 2 == 0 {
            b[..4].copy_from_slice(b"glTF");
            b[4..8].copy_from_slice(&2u32.to_le_bytes());
        } else {
            b[0] = b'{';
        }
        let mut s = new_scene();
        if let Err(e) = s.load_model_bytes(&b) {
            assert!(!e.to_string().contains("INTERNAL ERROR"), "input {i}: {e}");
        }
    }
}

#[test]
fn errors_say_what_is_wrong_in_words() {
    let mut s = new_scene();
    let e = s.load_model_bytes(b"not a model at all").unwrap_err();
    assert!(matches!(e, GraphicsError::Decode(_)), "{e:?}");
    assert!(e.to_string().to_lowercase().contains("gltf"), "{e}");
}
