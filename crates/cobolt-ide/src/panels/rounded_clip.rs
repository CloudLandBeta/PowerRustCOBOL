//! Offscreen rounded-corner clip for rounded GroupBox/Panel containers (spec 017).
//!
//! egui can only clip to axis-aligned rects, so child content (charts, grids,
//! images) bleeds past a rounded container's corner arc. The long-standing fix
//! ([`cobolt_forms::paint::draw_container_notch_mask`]) repaints the corner notch
//! with a **flat backdrop colour** on top of the bleed — which only works when the
//! surface behind the corner is opaque. Over a translucent surface it can't
//! reproduce the see-through backdrop, so the bleed shows through (the "transparency
//! issue"), and nested containers are skipped entirely for the same reason.
//!
//! This module fixes that at the GL level:
//!   1. **Capture** — before the container's children are drawn, copy the real
//!      framebuffer pixels of the container's bounding box into a texture (the
//!      genuine backdrop, colour + image + any translucency already composited).
//!   2. **Re-blit** — after the children are drawn, draw the container's bounding
//!      box back, sampling that capture through a **rounded-rect mask** so only the
//!      four corner notches (inside the box, outside the arc) are overwritten. The
//!      container interior is left untouched (`discard`), so children keep drawing.
//!
//! Because the notch pixels are sampled from the exact screen location they
//! overwrite, alignment is pixel-perfect and independent of surface opacity.
//!
//! It is **opt-in**: enabled only when `COBOLT_ROUNDED_CLIP=1` (also `true`/`on`).
//! When off, or when the GL backend is unavailable / a step fails, callers keep the
//! existing flat notch mask, so default rendering is unchanged.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use egui_glow::glow::{self, HasContext};

/// Runtime state of the rounded-corner GL clip, tri-valued so the IDE's Project
/// Settings toggle can override the env var without a rebuild:
/// `0` = uninitialised (fall back to `COBOLT_ROUNDED_CLIP` on first read),
/// `1` = forced off, `2` = forced on.
static ROUNDED_CLIP: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Turn the rounded-corner GL clip on/off at runtime. The IDE calls this each
/// frame from the live project setting; an explicit call always wins over the env.
pub fn set_enabled(on: bool) {
    ROUNDED_CLIP.store(if on { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
}

/// `true` when the rounded-corner GL clip is active (project setting, or the
/// `COBOLT_ROUNDED_CLIP` env var until the IDE overrides it).
pub fn enabled() -> bool {
    use std::sync::atomic::Ordering;
    match ROUNDED_CLIP.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let on = std::env::var("COBOLT_ROUNDED_CLIP")
                .map(|v| {
                    let v = v.trim();
                    v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
                })
                .unwrap_or(false);
            ROUNDED_CLIP.store(if on { 2 } else { 1 }, Ordering::Relaxed);
            on
        }
    }
}

/// `true` when the frame-diagnostics overlay is active. Delegates to
/// `cobolt_forms::paint` so the IDE has a single source of truth — used to label
/// the re-blit region so this frame stops being the nameless offender in the
/// exploded frame view.
fn frame_diagnostics() -> bool {
    cobolt_forms::paint::frame_diagnostics_enabled()
}

/// Draw a named outline for a clip frame at its TRUE position (the GL re-blit can't
/// be exploded like the shape-based frames, so labelling it in place is what pins
/// the nameless corner-hole to this operation). Foreground layer, so nothing hides it.
fn label_frame(painter: &egui::Painter, rect: egui::Rect, radius: f32, name: &str) {
    let color = egui::Color32::from_rgb(255, 140, 0); // orange — distinct from the slot palette
    let overlay = egui::Painter::new(
        painter.ctx().clone(),
        egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("cobolt_roundclip_diagnostics"),
        ),
        egui::Rect::EVERYTHING,
    );
    overlay.rect_stroke(
        rect,
        egui::CornerRadius::same(crate::cr8(radius)),
        egui::Stroke::new(1.5, color),
        egui::StrokeKind::Middle,
    );
    let t = 4.0;
    for c in [
        rect.left_top(),
        rect.right_top(),
        rect.left_bottom(),
        rect.right_bottom(),
    ] {
        overlay.line_segment(
            [c - egui::vec2(t, 0.0), c + egui::vec2(t, 0.0)],
            egui::Stroke::new(1.0, color),
        );
        overlay.line_segment(
            [c - egui::vec2(0.0, t), c + egui::vec2(0.0, t)],
            egui::Stroke::new(1.0, color),
        );
    }
    let tag = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(0.0, -13.0),
        egui::vec2(190.0, 12.0),
    );
    overlay.rect_filled(tag, 2.0, color);
    overlay.text(
        tag.left_center() + egui::vec2(4.0, 0.0),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::monospace(9.0),
        egui::Color32::BLACK,
    );
}

/// Shared GL state, cloneable handle. All heavy resources live behind a mutex and
/// are created lazily inside a paint callback (the only place a GL context exists).
#[derive(Clone, Default)]
pub struct RoundedClip {
    state: Arc<Mutex<ClipState>>,
}

#[derive(Default)]
struct ClipState {
    res: Option<GlRes>,
    /// One captured backdrop texture per container id, reused across frames.
    captures: HashMap<String, glow::Texture>,
}

struct GlRes {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    a_pos: u32,
    a_uv: u32,
    u_tex: Option<glow::UniformLocation>,
    u_size: Option<glow::UniformLocation>,
    u_radius: Option<glow::UniformLocation>,
}

const VERT_SRC: &str = r#"#version 150
in vec2 a_pos;
in vec2 a_uv;
out vec2 v_uv;
void main() {
    v_uv = a_uv;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
"#;

// Keep (draw the captured backdrop) only where the fragment lies OUTSIDE the
// rounded rect — i.e. in a corner notch. Everything inside the arc is discarded so
// the child content painted there survives.
const FRAG_SRC: &str = r#"#version 150
uniform sampler2D u_tex;
uniform vec2  u_size;    // container bbox size, px
uniform float u_radius;  // corner radius, px
in vec2 v_uv;
out vec4 frag;
void main() {
    vec2 halfsz = u_size * 0.5;
    vec2 local  = v_uv * u_size - halfsz;
    vec2 q = abs(local) - (halfsz - vec2(u_radius));
    float d = length(max(q, vec2(0.0))) - u_radius; // >0 outside the rounded rect
    if (d <= 0.0) {
        discard;                     // container interior: keep the children
    }
    float cov = clamp(d, 0.0, 1.0);  // 1px feather along the arc
    vec4 c = texture(u_tex, v_uv);
    frag = vec4(c.rgb, c.a * cov);
}
"#;

/// Process-wide clipper handle. GL resources and captured backdrop textures are
/// reused across frames, so this returns a clone of one shared instance.
pub fn instance() -> RoundedClip {
    static INST: OnceLock<RoundedClip> = OnceLock::new();
    INST.get_or_init(RoundedClip::new).clone()
}

impl RoundedClip {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a capture of `bbox` (logical points) for `id`. Must be enqueued after
    /// the backdrop is painted but before the container's children.
    pub fn enqueue_capture(&self, painter: &egui::Painter, id: &str, bbox: egui::Rect) {
        if !enabled() {
            return;
        }
        let state = self.state.clone();
        let id = id.to_owned();
        let cb = egui_glow::CallbackFn::new(move |info, gp| {
            let gl = gp.gl();
            let mut st = state.lock().unwrap();
            if st.res.is_none() {
                st.res = build_gl(gl);
            }
            if st.res.is_none() {
                return;
            }
            let vp = info.viewport_in_pixels();
            let screen_h = info.screen_size_px[1] as i32;
            // GL framebuffers are bottom-left origin; egui reports top-left.
            let x = vp.left_px as i32;
            let w = (vp.width_px as i32).max(1);
            let h = (vp.height_px as i32).max(1);
            let y = (screen_h - (vp.top_px as i32 + h)).max(0);
            let tex = *st
                .captures
                .entry(id.clone())
                .or_insert_with(|| unsafe { gl.create_texture().unwrap() });
            unsafe {
                gl.bind_texture(glow::TEXTURE_2D, Some(tex));
                // Copy the current framebuffer region into the texture (reallocates).
                gl.copy_tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA, x, y, w, h, 0);
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MIN_FILTER,
                    glow::NEAREST as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MAG_FILTER,
                    glow::NEAREST as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_S,
                    glow::CLAMP_TO_EDGE as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_T,
                    glow::CLAMP_TO_EDGE as i32,
                );
                gl.bind_texture(glow::TEXTURE_2D, None);
            }
        });
        painter.add(egui::PaintCallback {
            rect: bbox,
            callback: Arc::new(cb),
        });
    }

    /// Queue the masked re-blit of `bbox` for `id` at corner `radius` (logical px).
    /// Must be enqueued after the container's children are drawn.
    pub fn enqueue_reblit(&self, painter: &egui::Painter, id: &str, bbox: egui::Rect, radius: f32) {
        if !enabled() || radius < 0.5 {
            return;
        }
        let state = self.state.clone();
        let id = id.to_owned();
        let cb = egui_glow::CallbackFn::new(move |info, gp| {
            let gl = gp.gl();
            let st = state.lock().unwrap();
            let (Some(res), Some(&tex)) = (st.res.as_ref(), st.captures.get(&id)) else {
                return;
            };
            let vp = info.viewport_in_pixels();
            let ppp = info.pixels_per_point;
            let (sw, sh) = (info.screen_size_px[0] as f32, info.screen_size_px[1] as f32);
            let w = vp.width_px.max(1) as f32;
            let h = vp.height_px.max(1) as f32;
            let x0 = vp.left_px as f32;
            // Bottom-left origin pixel bounds, matching the captured texture.
            let yb = sh - (vp.top_px as f32 + h);
            let yt = sh - vp.top_px as f32;
            let x1 = x0 + w;

            // Pixel bounds → NDC. uv shares the bottom-left orientation of the
            // capture so each output pixel samples the backdrop it overwrites.
            let ndc = |px: f32, py: f32| -> [f32; 2] { [px / sw * 2.0 - 1.0, py / sh * 2.0 - 1.0] };
            let bl = ndc(x0, yb);
            let br = ndc(x1, yb);
            let tl = ndc(x0, yt);
            let tr = ndc(x1, yt);
            // TRIANGLE_STRIP order: BL, BR, TL, TR with matching uv.
            let verts: [f32; 16] = [
                bl[0], bl[1], 0.0, 0.0, //
                br[0], br[1], 1.0, 0.0, //
                tl[0], tl[1], 0.0, 1.0, //
                tr[0], tr[1], 1.0, 1.0, //
            ];

            unsafe {
                gl.use_program(Some(res.program));
                gl.bind_vertex_array(Some(res.vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.vbo));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck_cast(&verts),
                    glow::DYNAMIC_DRAW,
                );
                let stride = 4 * std::mem::size_of::<f32>() as i32;
                gl.enable_vertex_attrib_array(res.a_pos);
                gl.vertex_attrib_pointer_f32(res.a_pos, 2, glow::FLOAT, false, stride, 0);
                gl.enable_vertex_attrib_array(res.a_uv);
                gl.vertex_attrib_pointer_f32(
                    res.a_uv,
                    2,
                    glow::FLOAT,
                    false,
                    stride,
                    2 * std::mem::size_of::<f32>() as i32,
                );

                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, Some(tex));
                gl.uniform_1_i32(res.u_tex.as_ref(), 0);
                gl.uniform_2_f32(res.u_size.as_ref(), w, h);
                gl.uniform_1_f32(res.u_radius.as_ref(), radius * ppp);

                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

                // Leave state as egui expects (it rebinds per-primitive, but reset
                // the bindings we touched so nothing dangles).
                gl.bind_texture(glow::TEXTURE_2D, None);
                gl.bind_vertex_array(None);
                gl.use_program(None);
            }
        });
        painter.add(egui::PaintCallback {
            rect: bbox,
            callback: Arc::new(cb),
        });
    }
}

/// Adapter that drives [`RoundedClip`] from the `cobolt-forms` render walk: it
/// captures each rounded container's backdrop as the walk reaches it (after the
/// face + shadow, before the children) and re-blits every captured notch on
/// `finish`. Recording the rects here means nested containers are handled too —
/// each captures against whatever surface its own parent already painted.
pub struct RoundedClipHook {
    clip: RoundedClip,
    pending: Mutex<Vec<(String, egui::Rect, f32)>>,
}

impl RoundedClipHook {
    pub fn new() -> Self {
        Self {
            clip: instance(),
            pending: Mutex::new(Vec::new()),
        }
    }
}

impl Default for RoundedClipHook {
    fn default() -> Self {
        Self::new()
    }
}

impl cobolt_forms::render::RoundedClipHook for RoundedClipHook {
    fn on_container(&self, painter: &egui::Painter, id: &str, rect: egui::Rect, radius: f32) {
        self.clip.enqueue_capture(painter, id, rect);
        if frame_diagnostics() {
            label_frame(painter, rect, radius, "ROUNDCLIP_CAPTURE");
        }
        self.pending
            .lock()
            .unwrap()
            .push((id.to_owned(), rect, radius));
    }

    fn finish(&self, painter: &egui::Painter) {
        for (id, rect, radius) in self.pending.lock().unwrap().drain(..) {
            self.clip.enqueue_reblit(painter, &id, rect, radius);
            // Name the offender: this re-blit is the frame that paints the corner
            // notch (and holes any overlapping sibling like the pizza image).
            if frame_diagnostics() {
                label_frame(painter, rect, radius, "ROUNDCLIP_REBLIT");
            }
        }
    }
}

/// Compile the program + geometry. Returns `None` (caller falls back) on any error.
fn build_gl(gl: &glow::Context) -> Option<GlRes> {
    unsafe {
        let program = gl.create_program().ok()?;
        let compile = |ty: u32, src: &str| -> Option<glow::Shader> {
            let sh = gl.create_shader(ty).ok()?;
            gl.shader_source(sh, src);
            gl.compile_shader(sh);
            if !gl.get_shader_compile_status(sh) {
                tracing::warn!(
                    "rounded_clip shader compile failed: {}",
                    gl.get_shader_info_log(sh)
                );
                gl.delete_shader(sh);
                return None;
            }
            Some(sh)
        };
        let vs = compile(glow::VERTEX_SHADER, VERT_SRC)?;
        let fs = compile(glow::FRAGMENT_SHADER, FRAG_SRC)?;
        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        if !gl.get_program_link_status(program) {
            tracing::warn!(
                "rounded_clip program link failed: {}",
                gl.get_program_info_log(program)
            );
            gl.delete_program(program);
            return None;
        }
        let vao = gl.create_vertex_array().ok()?;
        let vbo = gl.create_buffer().ok()?;
        let a_pos = gl.get_attrib_location(program, "a_pos")? as u32;
        let a_uv = gl.get_attrib_location(program, "a_uv")? as u32;
        Some(GlRes {
            u_tex: gl.get_uniform_location(program, "u_tex"),
            u_size: gl.get_uniform_location(program, "u_size"),
            u_radius: gl.get_uniform_location(program, "u_radius"),
            program,
            vao,
            vbo,
            a_pos,
            a_uv,
        })
    }
}

/// Reinterpret an `f32` slice as bytes without pulling in the `bytemuck` crate.
fn bytemuck_cast(v: &[f32]) -> &[u8] {
    // Safe: `f32` has no padding/invalid bit patterns and the lengths line up.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}
