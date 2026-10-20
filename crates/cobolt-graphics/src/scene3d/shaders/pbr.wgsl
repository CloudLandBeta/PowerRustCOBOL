// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Physically based shading for Scene3D (WASM-018 R9): the metallic-roughness
// model of glTF 2.0 - Cook-Torrance GGX with Smith visibility and Schlick
// Fresnel - with up to eight lights, two shadow maps, fog and a flat ambient term.
//
// Every uniform below is mirrored by a `#[repr(C)]` struct in `gpu.rs`; the two
// must change together (a test checks their sizes against the WGSL layout).

struct Light {
    // xyz: position (point, spot).
    position: vec4<f32>,
    // xyz: the direction the light travels (directional, spot).
    direction: vec4<f32>,
    // rgb: colour times intensity; w: shadow map index plus one (0 = none).
    color: vec4<f32>,
    // x: cos(outer angle); y: 1 / (cos(inner) - cos(outer)); z: kind (0 directional, 1 point, 2 spot); w: range (0 = none).
    cone: vec4<f32>,
}

struct Frame {
    view_proj: mat4x4<f32>,
    // xyz: camera position; w: fog mode (0 none, 1 linear, 2 exponential).
    eye: vec4<f32>,
    // rgb: ambient colour times intensity.
    ambient: vec4<f32>,
    // rgb: fog colour; w: exponential density.
    fog_color: vec4<f32>,
    // x: linear fog near; y: linear fog far.
    fog_range: vec4<f32>,
    // x: number of lights.
    counts: vec4<u32>,
    lights: array<Light, 8>,
    shadow_vp: array<mat4x4<f32>, 2>,
    // x: 1 / shadow map size; y: depth bias; z: normal offset in world units.
    shadow_params: vec4<f32>,
    // The background gradient: top and bottom, premultiplied, already in the output encoding.
    bg_top: vec4<f32>,
    bg_bottom: vec4<f32>,
}

struct Draw {
    base_color: vec4<f32>,
    // rgb: emissive colour; w: 1 for an unlit material.
    emissive: vec4<f32>,
    // x: metallic; y: roughness; z: alpha cutoff; w: normal scale.
    params: vec4<f32>,
    // x: has a normal map; y: alpha mode (0 opaque, 1 mask, 2 blend); z: skinned.
    flags: vec4<u32>,
}

struct Skin {
    joints: array<mat4x4<f32>, 128>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var shadow_map: texture_depth_2d_array;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;

@group(1) @binding(0) var<uniform> draw: Draw;
@group(1) @binding(1) var material_sampler: sampler;
@group(1) @binding(2) var base_tex: texture_2d<f32>;
@group(1) @binding(3) var mr_tex: texture_2d<f32>;
@group(1) @binding(4) var emissive_tex: texture_2d<f32>;
@group(1) @binding(5) var normal_tex: texture_2d<f32>;

@group(2) @binding(0) var<uniform> skin: Skin;

const PI: f32 = 3.14159265359;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) joints: vec4<u32>,
    @location(4) weights: vec4<f32>,
    // One model matrix per instance, as four columns.
    @location(5) m0: vec4<f32>,
    @location(6) m1: vec4<f32>,
    @location(7) m2: vec4<f32>,
    @location(8) m3: vec4<f32>,
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    var p = vec4<f32>(in.position, 1.0);
    var n = vec4<f32>(in.normal, 0.0);
    if (draw.flags.z == 1u) {
        let s = in.weights.x * skin.joints[in.joints.x]
              + in.weights.y * skin.joints[in.joints.y]
              + in.weights.z * skin.joints[in.joints.z]
              + in.weights.w * skin.joints[in.joints.w];
        p = s * p;
        n = s * n;
    }
    let world = model * p;
    // The cofactor matrix of the model's upper 3x3 is the inverse transpose up to a positive
    // factor, which is what a normal needs (it stays right under non-uniform scale).
    let c0 = model[0].xyz;
    let c1 = model[1].xyz;
    let c2 = model[2].xyz;
    let normal_matrix = mat3x3<f32>(cross(c1, c2), cross(c2, c0), cross(c0, c1));
    var out: VertexOut;
    out.clip = frame.view_proj * world;
    out.world = world.xyz;
    out.normal = normal_matrix * n.xyz;
    out.uv = in.uv;
    return out;
}

fn d_ggx(n_dot_h: f32, a: f32) -> f32 {
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

// Smith visibility (height-correlated), already divided by 4 n.l n.v.
fn v_smith(n_dot_v: f32, n_dot_l: f32, a: f32) -> f32 {
    let a2 = a * a;
    let gv = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - a2) + a2);
    let gl = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - a2) + a2);
    return 0.5 / max(gv + gl, 1.0e-5);
}

fn f_schlick(f0: vec3<f32>, v_dot_h: f32) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - v_dot_h, 5.0);
}

// The fraction of light that reaches `world` (1 = fully lit): 3x3 percentage-closer filtering.
fn shadow_factor(index: u32, world: vec3<f32>, n: vec3<f32>, l: vec3<f32>) -> f32 {
    let slope = 1.0 - clamp(dot(n, l), 0.0, 1.0);
    let shifted = world + n * (frame.shadow_params.z * (0.5 + slope));
    let p = frame.shadow_vp[index] * vec4<f32>(shifted, 1.0);
    let ndc = p.xyz / p.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (ndc.z > 1.0 || ndc.z < 0.0 || uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return 1.0;
    }
    let depth = ndc.z - frame.shadow_params.y;
    let texel = frame.shadow_params.x;
    var sum = 0.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let offset = vec2<f32>(f32(dx), f32(dy)) * texel;
            sum += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + offset, i32(index), depth);
        }
    }
    return sum / 9.0;
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    let lo = x * 12.92;
    let hi = 1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(hi, lo, x <= vec3<f32>(0.0031308));
}

@fragment
fn fs_main(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // Everything that needs uniform control flow comes first.
    let base_s = textureSample(base_tex, material_sampler, in.uv);
    let mr_s = textureSample(mr_tex, material_sampler, in.uv);
    let em_s = textureSample(emissive_tex, material_sampler, in.uv);
    let nm_s = textureSample(normal_tex, material_sampler, in.uv);
    let dp1 = dpdx(in.world);
    let dp2 = dpdy(in.world);
    let duv1 = dpdx(in.uv);
    let duv2 = dpdy(in.uv);

    let base = draw.base_color * base_s;
    if (draw.flags.y == 1u && base.a < draw.params.z) {
        discard;
    }
    let alpha = select(1.0, base.a, draw.flags.y == 2u);

    var n_geo = normalize(in.normal);
    if (!front) {
        n_geo = -n_geo;
    }
    var n = n_geo;
    if (draw.flags.x == 1u) {
        // A tangent frame from the screen-space derivatives of position and texture coordinates.
        // glTF normal maps have +Y pointing up the image, which is towards decreasing v.
        let dp2perp = cross(dp2, n_geo);
        let dp1perp = cross(n_geo, dp1);
        let t = dp2perp * duv1.x + dp1perp * duv2.x;
        let b = -(dp2perp * duv1.y + dp1perp * duv2.y);
        let inv = inverseSqrt(max(max(dot(t, t), dot(b, b)), 1.0e-12));
        let tn = (nm_s.xyz * 2.0 - vec3<f32>(1.0)) * vec3<f32>(draw.params.w, draw.params.w, 1.0);
        n = normalize(mat3x3<f32>(t * inv, b * inv, n_geo) * tn);
    }

    var color: vec3<f32>;
    if (draw.emissive.w > 0.5) {
        // Unlit: the colour as it is.
        color = base.rgb;
    } else {
        let metallic = clamp(draw.params.x * mr_s.b, 0.0, 1.0);
        let roughness = clamp(draw.params.y * mr_s.g, 0.045, 1.0);
        let a = roughness * roughness;
        let f0 = mix(vec3<f32>(0.04), base.rgb, metallic);
        let diffuse_color = base.rgb * (1.0 - metallic);
        let v = normalize(frame.eye.xyz - in.world);
        let n_dot_v = max(dot(n, v), 1.0e-4);

        var lo = vec3<f32>(0.0);
        for (var i = 0u; i < frame.counts.x; i++) {
            let light = frame.lights[i];
            let kind = light.cone.z;
            var l: vec3<f32>;
            var atten = 1.0;
            if (kind < 0.5) {
                l = -light.direction.xyz;
            } else {
                let to_light = light.position.xyz - in.world;
                let dist = length(to_light);
                l = to_light / max(dist, 1.0e-4);
                // Inverse square: an intensity of 1 lights a surface one unit away as a
                // directional light of intensity 1 does.
                atten = 1.0 / max(dist * dist, 1.0e-4);
                if (light.cone.w > 0.0) {
                    let r = clamp(1.0 - pow(dist / light.cone.w, 4.0), 0.0, 1.0);
                    atten = atten * r * r;
                }
                if (kind > 1.5) {
                    let cd = dot(-l, light.direction.xyz);
                    let s = clamp((cd - light.cone.x) * light.cone.y, 0.0, 1.0);
                    atten = atten * s * s * (3.0 - 2.0 * s);
                }
            }
            let n_dot_l = clamp(dot(n, l), 0.0, 1.0);
            if (n_dot_l <= 0.0 || atten <= 0.0) {
                continue;
            }
            var vis = 1.0;
            let shadow_slot = u32(light.color.w + 0.5);
            if (shadow_slot > 0u) {
                vis = shadow_factor(shadow_slot - 1u, in.world, n_geo, l);
            }
            let h = normalize(l + v);
            let n_dot_h = clamp(dot(n, h), 0.0, 1.0);
            let v_dot_h = clamp(dot(v, h), 0.0, 1.0);
            let spec = d_ggx(n_dot_h, a) * v_smith(n_dot_v, n_dot_l, a) * f_schlick(f0, v_dot_h);
            // The pi folded in: a white surface facing a directional light of intensity 1 is 1.
            lo += (diffuse_color + PI * spec) * light.color.rgb * atten * n_dot_l * vis;
        }
        let ambient = frame.ambient.rgb * (diffuse_color + f0 * (1.0 - roughness));
        color = lo + ambient + draw.emissive.rgb * em_s.rgb;
    }

    // Fog.
    let mode = frame.eye.w;
    if (mode > 0.5) {
        let d = length(frame.eye.xyz - in.world);
        var f = 0.0;
        if (mode < 1.5) {
            f = clamp((d - frame.fog_range.x) / max(frame.fog_range.y - frame.fog_range.x, 1.0e-4), 0.0, 1.0);
        } else {
            f = 1.0 - exp(-frame.fog_color.w * d);
        }
        color = mix(color, frame.fog_color.rgb, f);
    }

    // The target is not an sRGB format: encode here, then premultiply for blending.
    return vec4<f32>(linear_to_srgb(color) * alpha, alpha);
}

struct BackgroundOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) t: f32,
}

// One triangle that covers the screen.
@vertex
fn vs_background(@builtin(vertex_index) index: u32) -> BackgroundOut {
    var out: BackgroundOut;
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    out.clip = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 1.0, 1.0);
    out.t = y;
    return out;
}

@fragment
fn fs_background(in: BackgroundOut) -> @location(0) vec4<f32> {
    return mix(frame.bg_top, frame.bg_bottom, clamp(in.t, 0.0, 1.0));
}
