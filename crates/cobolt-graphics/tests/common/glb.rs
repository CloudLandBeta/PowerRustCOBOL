// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A small glTF 2.0 model, built here so the tests need no files: a two-joint
//! skinned "arm" with a bend animation, and a textured, metallic "crate" quad.
#![allow(dead_code)]

use serde_json::{json, Value};

/// A 2 x 2 RGBA PNG: red, green / blue, white.
pub fn texture_png() -> Vec<u8> {
    let img = image::RgbaImage::from_raw(2, 2, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]).unwrap();
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// The glTF JSON and its binary buffer.
pub struct Model {
    pub json: Value,
    pub bin: Vec<u8>,
}

fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

fn f32s(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// The arm and the crate. `uri` is `None` for a GLB (the buffer is the binary chunk), else the
/// URI the single buffer is given.
pub fn arm_model(uri: Option<&str>) -> Model {
    let mut bin: Vec<u8> = Vec::new();
    let mut views: Vec<Value> = Vec::new();
    let mut accessors: Vec<Value> = Vec::new();
    let mut add = |bin: &mut Vec<u8>, data: &[u8], target: Option<u32>| -> usize {
        pad4(bin);
        let offset = bin.len();
        bin.extend_from_slice(data);
        let mut v = json!({"buffer": 0, "byteOffset": offset, "byteLength": data.len()});
        if let Some(t) = target {
            v["target"] = json!(t);
        }
        views.push(v);
        views.len() - 1
    };
    let mut acc = |views_idx: usize, ctype: u32, count: usize, ty: &str, extra: Option<(Value, Value)>| -> usize {
        let mut a = json!({"bufferView": views_idx, "componentType": ctype, "count": count, "type": ty});
        if let Some((min, max)) = extra {
            a["min"] = min;
            a["max"] = max;
        }
        accessors.push(a);
        accessors.len() - 1
    };

    // The arm: a strip of three rows of two vertices, y = 0, 1, 2.
    let pos = [-0.2f32, 0.0, 0.0, 0.2, 0.0, 0.0, -0.2, 1.0, 0.0, 0.2, 1.0, 0.0, -0.2, 2.0, 0.0, 0.2, 2.0, 0.0];
    let idx: [u16; 12] = [0, 1, 3, 0, 3, 2, 2, 3, 5, 2, 5, 4];
    let joints: [u16; 24] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0];
    let weights = [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    let v = add(&mut bin, &f32s(&pos), Some(34962));
    let a_pos = acc(v, 5126, 6, "VEC3", Some((json!([-0.2, 0.0, 0.0]), json!([0.2, 2.0, 0.0]))));
    let v = add(&mut bin, &idx.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>(), Some(34963));
    let a_idx = acc(v, 5123, 12, "SCALAR", None);
    let v = add(&mut bin, &joints.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>(), Some(34962));
    let a_joints = acc(v, 5123, 6, "VEC4", None);
    let v = add(&mut bin, &f32s(&weights), Some(34962));
    let a_weights = acc(v, 5126, 6, "VEC4", None);
    // Inverse bind matrices: identity, and a translation down by 1 (column-major).
    let ibm = [1.0f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 1.0];
    let v = add(&mut bin, &f32s(&ibm), None);
    let a_ibm = acc(v, 5126, 2, "MAT4", None);
    // The bend: the elbow turns 90 degrees about Z over one second.
    let v = add(&mut bin, &f32s(&[0.0, 1.0]), None);
    let a_times = acc(v, 5126, 2, "SCALAR", Some((json!([0.0]), json!([1.0]))));
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let v = add(&mut bin, &f32s(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, s, s]), None);
    let a_rot = acc(v, 5126, 2, "VEC4", None);

    // The crate: a quad with normals and texture coordinates.
    let qpos = [-1.0f32, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0];
    let qnorm = [0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
    let quv = [0.0f32, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0];
    let qidx: [u16; 6] = [0, 1, 2, 0, 2, 3];
    let v = add(&mut bin, &f32s(&qpos), Some(34962));
    let q_pos = acc(v, 5126, 4, "VEC3", Some((json!([-1.0, -1.0, 0.0]), json!([1.0, 1.0, 0.0]))));
    let v = add(&mut bin, &f32s(&qnorm), Some(34962));
    let q_norm = acc(v, 5126, 4, "VEC3", None);
    let v = add(&mut bin, &f32s(&quv), Some(34962));
    let q_uv = acc(v, 5126, 4, "VEC2", None);
    let v = add(&mut bin, &qidx.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>(), Some(34963));
    let q_idx = acc(v, 5123, 6, "SCALAR", None);
    let png = texture_png();
    let image_view = add(&mut bin, &png, None);
    pad4(&mut bin);

    let buffer = match uri {
        Some(u) => json!({"byteLength": bin.len(), "uri": u}),
        None => json!({"byteLength": bin.len()}),
    };
    let json = json!({
        "asset": {"version": "2.0", "generator": "cobolt-graphics tests"},
        "scene": 0,
        "scenes": [{"name": "arm scene", "nodes": [0, 2, 3]}],
        "nodes": [
            {"name": "shoulder", "children": [1]},
            {"name": "elbow", "translation": [0.0, 1.0, 0.0]},
            {"name": "arm", "mesh": 0, "skin": 0},
            {"name": "crate", "mesh": 1, "translation": [3.0, 0.0, 0.0]}
        ],
        "meshes": [
            {"name": "arm mesh", "primitives": [{"attributes": {"POSITION": a_pos, "JOINTS_0": a_joints, "WEIGHTS_0": a_weights}, "indices": a_idx}]},
            {"name": "crate mesh", "primitives": [{"attributes": {"POSITION": q_pos, "NORMAL": q_norm, "TEXCOORD_0": q_uv}, "indices": q_idx, "material": 0}]}
        ],
        "skins": [{"joints": [0, 1], "inverseBindMatrices": a_ibm}],
        "materials": [{
            "name": "painted metal",
            "pbrMetallicRoughness": {"baseColorFactor": [1.0, 0.5, 0.25, 1.0], "metallicFactor": 1.0, "roughnessFactor": 0.2, "baseColorTexture": {"index": 0}},
            "emissiveFactor": [0.1, 0.2, 0.3],
            "doubleSided": true
        }],
        "textures": [{"source": 0}],
        "images": [{"bufferView": image_view, "mimeType": "image/png"}],
        "animations": [{
            "name": "bend",
            "samplers": [{"input": a_times, "output": a_rot, "interpolation": "LINEAR"}],
            "channels": [{"sampler": 0, "target": {"node": 1, "path": "rotation"}}]
        }],
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [buffer]
    });
    Model { json, bin }
}

/// Pack JSON and a binary chunk as a GLB file.
pub fn glb(model: &Model) -> Vec<u8> {
    let mut json_bytes = serde_json::to_vec(&model.json).unwrap();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let mut bin = model.bin.clone();
    pad4(&mut bin);
    let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

/// A `.gltf` with its buffer as a `data:` URI.
pub fn gltf_with_data_uri() -> Vec<u8> {
    let probe = arm_model(Some("x"));
    let uri = format!("data:application/octet-stream;base64,{}", base64(&probe.bin));
    let m = arm_model(Some(&uri));
    serde_json::to_vec(&m.json).unwrap()
}

pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}
