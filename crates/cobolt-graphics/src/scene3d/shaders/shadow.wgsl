// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Depth-only pass for Scene3D shadow maps (WASM-018 R9).

struct ShadowFrame {
    view_proj: mat4x4<f32>,
    // x: 1 when the draw is skinned.
    flags: vec4<u32>,
}

struct Skin {
    joints: array<mat4x4<f32>, 128>,
}

@group(0) @binding(0) var<uniform> shadow: ShadowFrame;
@group(1) @binding(0) var<uniform> skin: Skin;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(3) joints: vec4<u32>,
    @location(4) weights: vec4<f32>,
    @location(5) m0: vec4<f32>,
    @location(6) m1: vec4<f32>,
    @location(7) m2: vec4<f32>,
    @location(8) m3: vec4<f32>,
}

@vertex
fn vs_shadow(in: VertexIn) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    var p = vec4<f32>(in.position, 1.0);
    if (shadow.flags.x == 1u) {
        let s = in.weights.x * skin.joints[in.joints.x]
              + in.weights.y * skin.joints[in.joints.y]
              + in.weights.z * skin.joints[in.joints.z]
              + in.weights.w * skin.joints[in.joints.w];
        p = s * p;
    }
    return shadow.view_proj * (model * p);
}
