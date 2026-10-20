// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `wgpu` renderer: uploads a [`Frame`] and draws it.
//!
//! Everything the picture depends on was decided on the CPU in
//! [`Scene3D::build_frame`](super::scene::Scene3D::build_frame); this module only
//! translates it to GPU work: vertex and texture upload (cached while the scene
//! keeps the data alive), one uniform block per frame, one per draw (a dynamic
//! offset into a single buffer), the shadow passes, and the main pass with
//! optional 4x multisampling, resolved into an `Rgba8Unorm` texture.
//!
//! The shaders are `shaders/pbr.wgsl` and `shaders/shadow.wgsl`; the uniform
//! structs below mirror them field for field.
//!
//! Errors the driver raises are collected, not panicked on (`on_uncaptured_error`
//! and the device-lost callback), and come back from [`Renderer::render`] as
//! [`GraphicsError::Gpu`], so a bad shader or a lost device cannot take the
//! host's page down.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex, Weak};

use bytemuck::{Pod, Zeroable};

use super::frame::{DrawItem, Frame, LightItem, MAX_LIGHTS, MAX_SHADOW_MAPS};
use super::math::{Frustum, Mat4};
use super::mesh::MeshData;
use super::scene::{AlphaMode, Background, FogMode, LightKind};
use crate::color::Rgba;
use crate::error::{GraphicsError, Result};
use crate::pixels::PixelBuffer;

const PBR_SHADER: &str = include_str!("shaders/pbr.wgsl");
const SHADOW_SHADER: &str = include_str!("shaders/shadow.wgsl");

/// The colour format of the picture.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Bytes of one skin's joint matrices (128 of them).
const SKIN_BYTES: u64 = 128 * 64;
/// The most draw items one frame can carry.
pub const MAX_DRAWS: usize = 8192;
/// The most skinned draw items one frame can carry.
pub const MAX_SKINNED_DRAWS: usize = 256;

// ---------------------------------------------------------------------------
// The adapter
// ---------------------------------------------------------------------------

/// What Canvas3D runs on.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Backend {
    /// The browser's WebGPU.
    WebGpu,
    /// The browser's WebGL2 (through `wgpu`'s GL backend).
    WebGl2,
    /// Vulkan.
    Vulkan,
    /// Metal.
    Metal,
    /// Direct3D 12.
    Dx12,
    /// OpenGL (desktop).
    OpenGl,
    /// A software adapter (a CPU implementation of Vulkan or GL): slow, and what tests use where there is no GPU.
    Software,
    /// No adapter was found: nothing can be drawn.
    None,
}

impl Backend {
    /// The name the `Backend` property of Canvas3D shows.
    pub fn name(&self) -> &'static str {
        match self {
            Backend::WebGpu => "WebGPU",
            Backend::WebGl2 => "WebGL2",
            Backend::Vulkan => "Vulkan",
            Backend::Metal => "Metal",
            Backend::Dx12 => "Dx12",
            Backend::OpenGl => "OpenGL",
            Backend::Software => "Software",
            Backend::None => "None",
        }
    }
}

/// What the adapter is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuInfo {
    /// The API in use.
    pub backend: Backend,
    /// The adapter's name ("Apple M2", "llvmpipe").
    pub adapter: String,
    /// The driver's description.
    pub driver: String,
}

/// A graphics device: cheap to clone, shared by every Canvas3D that uses it.
#[derive(Clone)]
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: GpuInfo,
    errors: Arc<Mutex<Vec<String>>>,
    lost: Arc<Mutex<Option<String>>>,
    uniform_align: u64,
}

impl std::fmt::Debug for Gpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Gpu({} on {})", self.info.adapter, self.info.backend.name())
    }
}

impl Gpu {
    /// Open the best adapter there is (blocking; not available on `wasm32`, where [`Gpu::new_async`] is used).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new() -> Result<Gpu> {
        pollster::block_on(Self::new_async())
    }

    /// Open the best adapter there is.
    pub async fn new_async() -> Result<Gpu> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() })
            .await
            .map_err(|e| GraphicsError::NoAdapter(format!("{e}")))?;
        let info = adapter.get_info();
        let backend = match (info.device_type, info.backend) {
            (wgpu::DeviceType::Cpu, _) => Backend::Software,
            (_, wgpu::Backend::Vulkan) => Backend::Vulkan,
            (_, wgpu::Backend::Metal) => Backend::Metal,
            (_, wgpu::Backend::Dx12) => Backend::Dx12,
            (_, wgpu::Backend::Gl) => {
                if cfg!(target_arch = "wasm32") {
                    Backend::WebGl2
                } else {
                    Backend::OpenGl
                }
            }
            (_, wgpu::Backend::BrowserWebGpu) => Backend::WebGpu,
            (_, _) => Backend::None,
        };
        let limits = adapter.limits();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor { label: Some("cobolt-graphics"), required_limits: limits.clone(), ..Default::default() })
            .await
            .map_err(|e| GraphicsError::Gpu(format!("the graphics device could not be opened: {e}")))?;
        let errors = Arc::new(Mutex::new(Vec::new()));
        let lost = Arc::new(Mutex::new(None));
        {
            let errors = errors.clone();
            device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
                if let Ok(mut v) = errors.lock() {
                    if v.len() < 16 {
                        v.push(e.to_string());
                    }
                }
            }));
            let lost = lost.clone();
            device.set_device_lost_callback(move |reason, message| {
                if let Ok(mut l) = lost.lock() {
                    *l = Some(format!("{reason:?}: {message}"));
                }
            });
        }
        Ok(Gpu {
            device,
            queue,
            info: GpuInfo { backend, adapter: info.name.clone(), driver: format!("{} {}", info.driver, info.driver_info).trim().to_string() },
            errors,
            lost,
            uniform_align: limits.min_uniform_buffer_offset_alignment.max(64) as u64,
        })
    }

    /// What it is.
    pub fn info(&self) -> &GpuInfo {
        &self.info
    }

    /// The `wgpu` device, for a host that shares it (the egui host with a wgpu renderer).
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The `wgpu` queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Errors the driver reported since the last call, and the loss of the device if it was lost.
    fn take_errors(&self) -> Option<String> {
        let mut msgs: Vec<String> = self.errors.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default();
        if let Some(l) = self.lost.lock().ok().and_then(|l| l.clone()) {
            msgs.push(format!("the graphics device was lost ({l})"));
        }
        if msgs.is_empty() {
            None
        } else {
            Some(msgs.join("; "))
        }
    }

    /// Wait until the GPU has finished everything submitted so far (blocking; a no-op on `wasm32`,
    /// where the browser's event loop does the waiting).
    pub fn wait_idle(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        }
    }

    /// Whether the device was lost (the host should make a new [`Gpu`] and [`Renderer`]).
    pub fn is_lost(&self) -> bool {
        // The loss is reported through a callback that only runs when the device is polled (in the
        // browser the event loop polls it).
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
        self.lost.lock().map(|l| l.is_some()).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Uniform blocks (mirrors of the WGSL structs)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct LightU {
    position: [f32; 4],
    direction: [f32; 4],
    color: [f32; 4],
    cone: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FrameU {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    ambient: [f32; 4],
    fog_color: [f32; 4],
    fog_range: [f32; 4],
    counts: [u32; 4],
    lights: [LightU; MAX_LIGHTS],
    shadow_vp: [[[f32; 4]; 4]; MAX_SHADOW_MAPS],
    shadow_params: [f32; 4],
    bg_top: [f32; 4],
    bg_bottom: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct DrawU {
    base_color: [f32; 4],
    emissive: [f32; 4],
    params: [f32; 4],
    flags: [u32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ShadowU {
    view_proj: [[f32; 4]; 4],
    flags: [u32; 4],
}

/// The sizes the shaders expect; a test compares them with what naga computes from the WGSL.
pub const FRAME_UNIFORM_BYTES: usize = std::mem::size_of::<FrameU>();
/// Bytes of one draw's uniform block.
pub const DRAW_UNIFORM_BYTES: usize = std::mem::size_of::<DrawU>();
/// Bytes of one shadow pass's uniform block.
pub const SHADOW_UNIFORM_BYTES: usize = std::mem::size_of::<ShadowU>();

const VERTEX_STRIDE: u64 = 56;
const INSTANCE_STRIDE: u64 = 64;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

struct MeshGpu {
    /// A weak reference keeps the allocation (so its address cannot be reused while the entry exists)
    /// without keeping the data alive: the entry is dropped when the scene lets go of the mesh.
    hold: Weak<MeshData>,
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    index_count: u32,
}

struct TextureGpu {
    hold: Weak<PixelBuffer>,
    view: wgpu::TextureView,
}

type MatKey = [usize; 4];

struct MaterialGroup {
    group: wgpu::BindGroup,
    hold: [Weak<PixelBuffer>; 4],
}

struct Targets {
    width: u32,
    height: u32,
    msaa: u32,
    color_ms_view: Option<wgpu::TextureView>,
    _color_ms: Option<wgpu::Texture>,
    resolve: wgpu::Texture,
    resolve_view: wgpu::TextureView,
    _depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
}

/// A buffer that grows when more is asked of it (vertex and instance data).
struct Grow {
    buffer: wgpu::Buffer,
    size: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl Grow {
    fn new(device: &wgpu::Device, label: &'static str, size: u64, usage: wgpu::BufferUsages) -> Grow {
        let size = size.max(256);
        Grow { buffer: device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }), size, usage, label }
    }

    fn ensure(&mut self, device: &wgpu::Device, needed: u64) {
        if needed > self.size {
            let size = needed.next_power_of_two();
            *self = Grow::new(device, self.label, size, self.usage);
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct PipelineKey {
    blend: bool,
    double_sided: bool,
    msaa: u32,
}

/// What one render did, for the program's `GetStats`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    /// Draw calls in the main pass.
    pub draw_calls: u32,
    /// Draw calls in the shadow passes.
    pub shadow_draw_calls: u32,
    /// Instances drawn in the main pass.
    pub instances: u32,
    /// Triangles drawn in the main pass.
    pub triangles: u64,
    /// Shadow maps rendered.
    pub shadow_maps: u32,
}

/// Settings fixed when a renderer is made.
#[derive(Copy, Clone, Debug)]
pub struct RendererConfig {
    /// The edge of each shadow map in texels (256 to 8192).
    pub shadow_map_size: u32,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self { shadow_map_size: 2048 }
    }
}

/// Draws frames into an offscreen texture.
pub struct Renderer {
    gpu: Gpu,
    config: RendererConfig,
    shader: wgpu::ShaderModule,
    material_layout: wgpu::BindGroupLayout,
    main_layout: wgpu::PipelineLayout,
    background_layout: wgpu::PipelineLayout,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    background_pipelines: HashMap<u32, wgpu::RenderPipeline>,
    shadow_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    white: wgpu::TextureView,
    _white_tex: wgpu::Texture,
    _shadow_tex: wgpu::Texture,
    shadow_layers: Vec<wgpu::TextureView>,
    frame_ub: wgpu::Buffer,
    draw_ub: wgpu::Buffer,
    skin_ub: wgpu::Buffer,
    shadow_ub: wgpu::Buffer,
    frame_bind: wgpu::BindGroup,
    skin_bind: wgpu::BindGroup,
    shadow_bind: wgpu::BindGroup,
    instance_vb: Grow,
    targets: Option<Targets>,
    meshes: HashMap<usize, MeshGpu>,
    textures: HashMap<(usize, bool), TextureGpu>,
    materials: HashMap<MatKey, MaterialGroup>,
    last_stats: RenderStats,
    last_background: Background,
}

fn align_up(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

fn uniform_entry(binding: u32, dynamic: bool, size: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: dynamic, min_binding_size: NonZeroU64::new(size) },
        count: None,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
        count: None,
    }
}

impl Renderer {
    /// A renderer on this device.
    pub fn new(gpu: Gpu, config: RendererConfig) -> Result<Renderer> {
        if !(256..=8192).contains(&config.shadow_map_size) {
            return Err(GraphicsError::invalid("a shadow map is from 256 to 8192 texels on a side"));
        }
        let d = &gpu.device;
        let shader = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("pbr"), source: wgpu::ShaderSource::Wgsl(PBR_SHADER.into()) });
        let shadow_shader = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("shadow"), source: wgpu::ShaderSource::Wgsl(SHADOW_SHADER.into()) });

        // ---- layouts ----
        let frame_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[
                uniform_entry(0, false, FRAME_UNIFORM_BYTES as u64),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
            ],
        });
        let material_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material"),
            entries: &[
                uniform_entry(0, true, DRAW_UNIFORM_BYTES as u64),
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                texture_entry(2),
                texture_entry(3),
                texture_entry(4),
                texture_entry(5),
            ],
        });
        let skin_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("skin"), entries: &[uniform_entry(0, true, SKIN_BYTES)] });
        let shadow_frame_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("shadow frame"), entries: &[uniform_entry(0, true, SHADOW_UNIFORM_BYTES as u64)] });
        let main_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("main"),
            bind_group_layouts: &[Some(&frame_layout), Some(&material_layout), Some(&skin_layout)],
            immediate_size: 0,
        });
        let background_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("background"), bind_group_layouts: &[Some(&frame_layout)], immediate_size: 0 });
        let shadow_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow"),
            bind_group_layouts: &[Some(&shadow_frame_layout), Some(&skin_layout)],
            immediate_size: 0,
        });

        // ---- fixed buffers ----
        let ub = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
        let frame_ub = d.create_buffer(&wgpu::BufferDescriptor { label: Some("frame"), size: FRAME_UNIFORM_BYTES as u64, usage: ub, mapped_at_creation: false });
        let draw_stride = align_up(DRAW_UNIFORM_BYTES as u64, gpu.uniform_align);
        let draw_ub = d.create_buffer(&wgpu::BufferDescriptor { label: Some("draws"), size: draw_stride * MAX_DRAWS as u64, usage: ub, mapped_at_creation: false });
        let skin_stride = align_up(SKIN_BYTES, gpu.uniform_align);
        let skin_ub = d.create_buffer(&wgpu::BufferDescriptor { label: Some("skins"), size: skin_stride * (MAX_SKINNED_DRAWS as u64 + 1), usage: ub, mapped_at_creation: false });
        let shadow_stride = align_up(SHADOW_UNIFORM_BYTES as u64, gpu.uniform_align);
        let shadow_ub = d.create_buffer(&wgpu::BufferDescriptor { label: Some("shadow frames"), size: shadow_stride * (MAX_SHADOW_MAPS as u64 * 2), usage: ub, mapped_at_creation: false });

        // ---- the shadow map array ----
        let size = config.shadow_map_size;
        let shadow_tex = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow maps"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: MAX_SHADOW_MAPS as u32 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_array = shadow_tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        let shadow_layers: Vec<wgpu::TextureView> = (0..MAX_SHADOW_MAPS as u32)
            .map(|i| shadow_tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2), base_array_layer: i, array_layer_count: Some(1), ..Default::default() }))
            .collect();
        let shadow_sampler = d.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow compare"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let frame_bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &frame_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: frame_ub.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_array) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_sampler) },
            ],
        });
        let skin_bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skin"),
            layout: &skin_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &skin_ub, offset: 0, size: NonZeroU64::new(SKIN_BYTES) }) }],
        });
        let shadow_bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow frame"),
            layout: &shadow_frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &shadow_ub, offset: 0, size: NonZeroU64::new(SHADOW_UNIFORM_BYTES as u64) }),
            }],
        });

        // ---- a white texel for every missing texture, and the sampler ----
        let white_tex = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("white"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &white_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let white = white_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = d.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("material"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        // ---- the depth-only pipeline (it does not depend on the target) ----
        let vertex_layouts = vertex_layouts();
        let shadow_pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState { module: &shadow_shader, entry_point: Some("vs_shadow"), compilation_options: Default::default(), buffers: &vertex_layouts },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, front_face: wgpu::FrontFace::Ccw, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });

        let instance_vb = Grow::new(d, "instances", 64 * 1024, wgpu::BufferUsages::VERTEX);
        let r = Renderer {
            gpu: gpu.clone(),
            config,
            shader,
            material_layout,
            main_layout,
            background_layout,
            pipelines: HashMap::new(),
            background_pipelines: HashMap::new(),
            shadow_pipeline,
            sampler,
            white,
            _white_tex: white_tex,
            _shadow_tex: shadow_tex,
            shadow_layers,
            frame_ub,
            draw_ub,
            skin_ub,
            shadow_ub,
            frame_bind,
            skin_bind,
            shadow_bind,
            instance_vb,
            targets: None,
            meshes: HashMap::new(),
            textures: HashMap::new(),
            materials: HashMap::new(),
            last_stats: RenderStats::default(),
            last_background: Background::Color(Rgba::TRANSPARENT),
        };
        if let Some(e) = gpu.take_errors() {
            return Err(GraphicsError::Gpu(format!("the renderer could not be set up: {e}")));
        }
        Ok(r)
    }

    /// The device this renderer runs on.
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// What the last render did.
    pub fn last_stats(&self) -> RenderStats {
        self.last_stats
    }

    /// The picture, for a host that shares the device and draws it itself.
    pub fn color_texture(&self) -> Option<&wgpu::Texture> {
        self.targets.as_ref().map(|t| &t.resolve)
    }

    /// The configured shadow map size.
    pub fn shadow_map_size(&self) -> u32 {
        self.config.shadow_map_size
    }

    // ---- targets ----------------------------------------------------------

    fn ensure_targets(&mut self, width: u32, height: u32, msaa: u32) {
        if let Some(t) = &self.targets {
            if t.width == width && t.height == height && t.msaa == msaa {
                return;
            }
        }
        let d = &self.gpu.device;
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let resolve = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("picture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let resolve_view = resolve.create_view(&wgpu::TextureViewDescriptor::default());
        let (color_ms, color_ms_view) = if msaa > 1 {
            let t = d.create_texture(&wgpu::TextureDescriptor {
                label: Some("picture (multisampled)"),
                size,
                mip_level_count: 1,
                sample_count: msaa,
                dimension: wgpu::TextureDimension::D2,
                format: COLOR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let v = t.create_view(&wgpu::TextureViewDescriptor::default());
            (Some(t), Some(v))
        } else {
            (None, None)
        };
        let depth = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size,
            mip_level_count: 1,
            sample_count: msaa,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        self.targets = Some(Targets { width, height, msaa, color_ms_view, _color_ms: color_ms, resolve, resolve_view, _depth: depth, depth_view });
    }

    // ---- pipelines ----------------------------------------------------------

    fn pipeline(&mut self, key: PipelineKey) -> &wgpu::RenderPipeline {
        if !self.pipelines.contains_key(&key) {
            let layouts = vertex_layouts();
            let blend = if key.blend {
                Some(wgpu::BlendState {
                    color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                    alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                })
            } else {
                None
            };
            let p = self.gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("main"),
                layout: Some(&self.main_layout),
                vertex: wgpu::VertexState { module: &self.shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &layouts },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: if key.double_sided { None } else { Some(wgpu::Face::Back) },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(!key.blend),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState { count: key.msaa, mask: !0, alpha_to_coverage_enabled: false },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            });
            self.pipelines.insert(key, p);
        }
        &self.pipelines[&key]
    }

    fn background_pipeline(&mut self, msaa: u32) {
        if self.background_pipelines.contains_key(&msaa) {
            return;
        }
        let p = self.gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("background"),
            layout: Some(&self.background_layout),
            vertex: wgpu::VertexState { module: &self.shader, entry_point: Some("vs_background"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState { count: msaa, mask: !0, alpha_to_coverage_enabled: false },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_background"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.background_pipelines.insert(msaa, p);
    }

    // ---- uploads ------------------------------------------------------------

    fn upload_mesh(&mut self, data: &Arc<MeshData>) -> usize {
        let key = Arc::as_ptr(data) as usize;
        if self.meshes.contains_key(&key) {
            return key;
        }
        let n = data.positions.len();
        let mut bytes: Vec<u8> = Vec::with_capacity(n * VERTEX_STRIDE as usize);
        for i in 0..n {
            bytes.extend_from_slice(bytemuck::cast_slice(&data.positions[i]));
            bytes.extend_from_slice(bytemuck::cast_slice(&data.normals[i]));
            bytes.extend_from_slice(bytemuck::cast_slice(&data.uvs[i]));
            let joints = data.joints.get(i).copied().unwrap_or([0; 4]);
            let weights = data.weights.get(i).copied().unwrap_or([0.0; 4]);
            bytes.extend_from_slice(bytemuck::cast_slice(&joints));
            bytes.extend_from_slice(bytemuck::cast_slice(&weights));
        }
        use wgpu::util::DeviceExt;
        let vertex = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("mesh vertices"), contents: &bytes, usage: wgpu::BufferUsages::VERTEX });
        let index = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("mesh indices"), contents: bytemuck::cast_slice(&data.indices), usage: wgpu::BufferUsages::INDEX });
        self.meshes.insert(key, MeshGpu { hold: Arc::downgrade(data), vertex, index, index_count: data.indices.len() as u32 });
        key
    }

    fn upload_texture(&mut self, px: &Arc<PixelBuffer>, srgb: bool) -> wgpu::TextureView {
        let key = (Arc::as_ptr(px) as usize, srgb);
        if let Some(t) = self.textures.get(&key) {
            return t.view.clone();
        }
        let (w, h) = (px.width(), px.height());
        let levels = (32 - w.max(h).leading_zeros()).max(1);
        let tex = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("material texture"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if srgb { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm },
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // The mip chain by 2x2 box filtering (straight alpha is averaged premultiplied so a transparent texel adds no colour).
        let mut level: Vec<u8> = px.as_bytes().to_vec();
        let (mut lw, mut lh) = (w, h);
        for m in 0..levels {
            self.gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: m, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &level,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(lw * 4), rows_per_image: Some(lh) },
                wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
            );
            if m + 1 < levels {
                let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
                level = box_down(&level, lw, lh, nw, nh);
                lw = nw;
                lh = nh;
            }
        }
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.textures.insert(key, TextureGpu { hold: Arc::downgrade(px), view: view.clone() });
        view
    }

    fn material_group(&mut self, m: &super::scene::Material) -> MatKey {
        let slots = [&m.base_color_texture, &m.metallic_roughness_texture, &m.emissive_texture, &m.normal_texture];
        let key: MatKey = std::array::from_fn(|i| slots[i].as_ref().map(|a| Arc::as_ptr(a) as usize).unwrap_or(0));
        if self.materials.contains_key(&key) {
            return key;
        }
        let srgb = [true, false, true, false];
        let views: Vec<wgpu::TextureView> = (0..4).map(|i| slots[i].as_ref().map(|a| self.upload_texture(a, srgb[i])).unwrap_or_else(|| self.white.clone())).collect();
        let group = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material"),
            layout: &self.material_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &self.draw_ub, offset: 0, size: NonZeroU64::new(DRAW_UNIFORM_BYTES as u64) }) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&views[0]) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&views[1]) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&views[2]) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&views[3]) },
            ],
        });
        self.materials.insert(key, MaterialGroup { group, hold: std::array::from_fn(|i| slots[i].as_ref().map(Arc::downgrade).unwrap_or_default()) });
        key
    }

    /// Forget GPU data whose CPU side has been released.
    fn evict(&mut self) {
        self.meshes.retain(|_, m| m.hold.strong_count() > 0);
        // A slot with no texture holds a `Weak::new()`, which is dead from birth: only slots that had one are checked.
        self.materials.retain(|k, m| (0..4).all(|i| k[i] == 0 || m.hold[i].strong_count() > 0));
        self.textures.retain(|_, t| t.hold.strong_count() > 0);
    }

    /// How many meshes, textures and material bind groups the renderer holds (for tests and `GetStats`).
    pub fn cache_sizes(&self) -> (usize, usize, usize) {
        (self.meshes.len(), self.textures.len(), self.materials.len())
    }

    // ---- the frame ------------------------------------------------------------

    /// Draw `frame` into the renderer's picture (`width` x `height`, `msaa` of 1 or 4 samples).
    pub fn render(&mut self, frame: &Frame, width: u32, height: u32, msaa: u32) -> Result<RenderStats> {
        if width == 0 || height == 0 || width > 16384 || height > 16384 {
            return Err(GraphicsError::invalid("a Canvas3D is from 1 to 16384 pixels on a side"));
        }
        if !matches!(msaa, 1 | 4) {
            return Err(GraphicsError::invalid("Msaa is 1 (off) or 4"));
        }
        if self.gpu.is_lost() {
            return Err(GraphicsError::Gpu("the graphics device was lost; make a new Canvas3D".into()));
        }
        self.ensure_targets(width, height, msaa);
        self.background_pipeline(msaa);

        // ---- which draws, and what each needs ----
        struct Prep {
            mesh: usize,
            mat: MatKey,
            draw_offset: u32,
            skin_slot: Option<u32>,
            inst: (u32, u32),
            key: PipelineKey,
            tris: u64,
        }
        let draw_stride = align_up(DRAW_UNIFORM_BYTES as u64, self.gpu.uniform_align);
        let skin_stride = align_up(SKIN_BYTES, self.gpu.uniform_align);
        let shadow_stride = align_up(SHADOW_UNIFORM_BYTES as u64, self.gpu.uniform_align);

        let main_draws: Vec<usize> = (0..frame.draws.len()).filter(|&i| !frame.draws[i].visible.is_empty()).collect();
        if main_draws.len() > MAX_DRAWS {
            return Err(GraphicsError::LimitExceeded { what: "draw items in one frame", requested: main_draws.len() as u64, limit: MAX_DRAWS as u64, setting: "(fixed)" });
        }

        let mut inst: Vec<f32> = Vec::new();
        let mut draw_bytes: Vec<u8> = vec![0; draw_stride as usize * main_draws.len().max(1)];
        let mut skin_bytes: Vec<u8> = vec![0; skin_stride as usize * (MAX_SKINNED_DRAWS + 1)];
        let mut skin_slots: HashMap<usize, u32> = HashMap::new();
        let mut next_slot = 1u32; // slot 0 is the zero block non-skinned draws bind
        let mut skin_slot_for = |i: usize, d: &DrawItem, skin_bytes: &mut Vec<u8>| -> Result<Option<u32>> {
            let Some(joints) = &d.joints else { return Ok(None) };
            if let Some(s) = skin_slots.get(&i) {
                return Ok(Some(*s));
            }
            if next_slot as usize > MAX_SKINNED_DRAWS {
                return Err(GraphicsError::LimitExceeded { what: "skinned draw items in one frame", requested: next_slot as u64, limit: MAX_SKINNED_DRAWS as u64, setting: "(fixed)" });
            }
            let off = (next_slot as u64 * skin_stride) as usize;
            for (j, m) in joints.iter().take(128).enumerate() {
                skin_bytes[off + j * 64..off + j * 64 + 64].copy_from_slice(bytemuck::bytes_of(&m.to_cols_array_2d()));
            }
            skin_slots.insert(i, next_slot);
            next_slot += 1;
            Ok(Some(next_slot - 1))
        };

        let mut preps: Vec<Prep> = Vec::with_capacity(main_draws.len());
        for (n, &i) in main_draws.iter().enumerate() {
            let d = &frame.draws[i];
            let mesh = self.upload_mesh(&d.data);
            let mat = self.material_group(&d.material);
            let start = (inst.len() / 16) as u32;
            for &v in &d.visible {
                inst.extend_from_slice(&d.instances[v as usize].model.to_cols_array());
            }
            let m = &d.material;
            let du = DrawU {
                base_color: m.base_color,
                emissive: [m.emissive[0], m.emissive[1], m.emissive[2], if m.unlit { 1.0 } else { 0.0 }],
                params: [m.metallic, m.roughness, m.alpha_cutoff, m.normal_scale],
                flags: [m.normal_texture.is_some() as u32, alpha_code(m.alpha_mode), d.joints.is_some() as u32, 0],
            };
            let off = n * draw_stride as usize;
            draw_bytes[off..off + DRAW_UNIFORM_BYTES].copy_from_slice(bytemuck::bytes_of(&du));
            let skin_slot = skin_slot_for(i, d, &mut skin_bytes)?;
            preps.push(Prep {
                mesh,
                mat,
                draw_offset: off as u32,
                skin_slot,
                inst: (start, d.visible.len() as u32),
                key: PipelineKey { blend: m.alpha_mode == AlphaMode::Blend, double_sided: m.double_sided, msaa },
                tris: d.visible.len() as u64 * d.data.triangle_count() as u64,
            });
        }

        // ---- shadow casters ----
        struct ShadowPrep {
            draw: usize,
            mesh: usize,
            skin_slot: Option<u32>,
            inst: (u32, u32),
        }
        let mut shadow_work: Vec<(usize, Mat4, Vec<ShadowPrep>)> = Vec::new();
        let mut slot = 0usize;
        for l in &frame.lights {
            let Some(vp) = l.shadow else { continue };
            if slot >= MAX_SHADOW_MAPS {
                break;
            }
            let frustum = Frustum::from_view_proj(&vp);
            let mut casters = Vec::new();
            for (i, d) in frame.draws.iter().enumerate() {
                if d.material.alpha_mode == AlphaMode::Blend {
                    continue;
                }
                let start = (inst.len() / 16) as u32;
                let mut count = 0u32;
                for ins in &d.instances {
                    if d.joints.is_some() || frustum.intersects_aabb(&ins.aabb) {
                        inst.extend_from_slice(&ins.model.to_cols_array());
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                let mesh = self.upload_mesh(&d.data);
                let skin_slot = skin_slot_for(i, d, &mut skin_bytes)?;
                casters.push(ShadowPrep { draw: i, mesh, skin_slot, inst: (start, count) });
            }
            shadow_work.push((slot, vp, casters));
            slot += 1;
        }

        // ---- the frame uniform ----
        let mut fu: FrameU = Zeroable::zeroed();
        fu.view_proj = frame.view_proj.to_cols_array_2d();
        let (fog_mode, fog_color, fog_range) = match frame.fog {
            None => (0.0, [0.0; 4], [0.0; 4]),
            Some(f) => match f.mode {
                FogMode::Linear { near, far } => (1.0, [f.color[0], f.color[1], f.color[2], 0.0], [near, far, 0.0, 0.0]),
                FogMode::Exponential { density } => (2.0, [f.color[0], f.color[1], f.color[2], density], [0.0; 4]),
            },
        };
        fu.eye = [frame.eye.x, frame.eye.y, frame.eye.z, fog_mode];
        fu.ambient = [frame.ambient[0], frame.ambient[1], frame.ambient[2], 0.0];
        fu.fog_color = fog_color;
        fu.fog_range = fog_range;
        let mut shadow_slot = 0usize;
        let mut n_lights = 0u32;
        for (i, l) in frame.lights.iter().take(MAX_LIGHTS).enumerate() {
            fu.lights[i] = light_uniform(l, if l.shadow.is_some() && shadow_slot < MAX_SHADOW_MAPS { shadow_slot as f32 + 1.0 } else { 0.0 });
            if let (Some(vp), true) = (l.shadow, shadow_slot < MAX_SHADOW_MAPS) {
                fu.shadow_vp[shadow_slot] = vp.to_cols_array_2d();
                shadow_slot += 1;
            }
            n_lights += 1;
        }
        fu.counts = [n_lights, 0, 0, 0];
        // The shadow bias is in depth units of the map; the normal offset is about two texels of the volume.
        fu.shadow_params = [1.0 / self.config.shadow_map_size as f32, 0.0006, shadow_normal_offset(frame), 0.0];
        let (bg_top, bg_bottom, clear) = background_colors(&frame.background);
        fu.bg_top = bg_top;
        fu.bg_bottom = bg_bottom;
        self.last_background = frame.background;

        // ---- upload ----
        let q = &self.gpu.queue;
        q.write_buffer(&self.frame_ub, 0, bytemuck::bytes_of(&fu));
        q.write_buffer(&self.draw_ub, 0, &draw_bytes[..draw_stride as usize * main_draws.len().max(1)]);
        q.write_buffer(&self.skin_ub, 0, &skin_bytes[..(skin_stride as usize * next_slot as usize).max(skin_stride as usize)]);
        for (s, vp, _) in &shadow_work {
            for skinned in 0..2u32 {
                let su = ShadowU { view_proj: vp.to_cols_array_2d(), flags: [skinned, 0, 0, 0] };
                q.write_buffer(&self.shadow_ub, (*s as u64 * 2 + skinned as u64) * shadow_stride, bytemuck::bytes_of(&su));
            }
        }
        self.instance_vb.ensure(&self.gpu.device, (inst.len() * 4) as u64);
        if !inst.is_empty() {
            self.gpu.queue.write_buffer(&self.instance_vb.buffer, 0, bytemuck::cast_slice(&inst));
        }

        // Pipelines first (they need &mut self), then the encoder borrows immutably.
        let keys: Vec<PipelineKey> = preps.iter().map(|p| p.key).collect();
        for k in keys {
            self.pipeline(k);
        }

        let mut stats = RenderStats::default();
        let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        // ---- shadow passes ----
        for (s, _vp, casters) in &shadow_work {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_layers[*s],
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.shadow_pipeline);
            for c in casters {
                let mesh = &self.meshes[&c.mesh];
                let skinned = c.skin_slot.is_some();
                pass.set_bind_group(0, &self.shadow_bind, &[((*s as u64 * 2 + skinned as u64) * shadow_stride) as u32]);
                pass.set_bind_group(1, &self.skin_bind, &[(c.skin_slot.unwrap_or(0) as u64 * skin_stride) as u32]);
                pass.set_vertex_buffer(0, mesh.vertex.slice(..));
                pass.set_vertex_buffer(1, self.instance_vb.buffer.slice(c.inst.0 as u64 * INSTANCE_STRIDE..(c.inst.0 as u64 + c.inst.1 as u64) * INSTANCE_STRIDE));
                pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..c.inst.1);
                stats.shadow_draw_calls += 1;
                let _ = c.draw;
            }
            stats.shadow_maps += 1;
        }

        // ---- the main pass ----
        {
            let t = self.targets.as_ref().expect("targets exist");
            let (view, resolve) = match &t.color_ms_view {
                Some(ms) => (ms, Some(&t.resolve_view)),
                None => (&t.resolve_view, None),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: clear[0], g: clear[1], b: clear[2], a: clear[3] }),
                        store: if resolve.is_some() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store },
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.frame_bind, &[]);
            pass.set_bind_group(2, &self.skin_bind, &[0]);
            if matches!(frame.background, Background::Gradient { .. }) {
                pass.set_pipeline(&self.background_pipelines[&msaa]);
                pass.draw(0..3, 0..1);
            }
            for p in &preps {
                let mesh = &self.meshes[&p.mesh];
                pass.set_pipeline(&self.pipelines[&p.key]);
                pass.set_bind_group(1, &self.materials[&p.mat].group, &[p.draw_offset]);
                pass.set_bind_group(2, &self.skin_bind, &[(p.skin_slot.unwrap_or(0) as u64 * skin_stride) as u32]);
                pass.set_vertex_buffer(0, mesh.vertex.slice(..));
                pass.set_vertex_buffer(1, self.instance_vb.buffer.slice(p.inst.0 as u64 * INSTANCE_STRIDE..(p.inst.0 as u64 + p.inst.1 as u64) * INSTANCE_STRIDE));
                pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..p.inst.1);
                stats.draw_calls += 1;
                stats.instances += p.inst.1;
                stats.triangles += p.tris;
            }
        }
        self.gpu.queue.submit(Some(encoder.finish()));
        self.evict();
        self.last_stats = stats;
        if let Some(e) = self.gpu.take_errors() {
            return Err(GraphicsError::Gpu(e));
        }
        Ok(stats)
    }

    // ---- reading the picture back ------------------------------------------------

    /// The picture as straight-alpha RGBA bytes (blocking; not available on `wasm32`).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read_pixels(&self) -> Result<PixelBuffer> {
        let (buffer, w, h, bpr) = self.copy_out()?;
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| GraphicsError::Gpu(format!("waiting for the picture failed: {e}")))?;
        rx.recv().map_err(|_| GraphicsError::Gpu("the picture's read-back never completed".into()))?.map_err(|e| GraphicsError::Gpu(format!("the picture could not be read back: {e}")))?;
        let px = Self::unpack(&slice.get_mapped_range().map_err(|e| GraphicsError::Gpu(format!("{e}")))?, w, h, bpr)?;
        buffer.unmap();
        if let Some(e) = self.gpu.take_errors() {
            return Err(GraphicsError::Gpu(e));
        }
        Ok(px)
    }

    /// The picture as straight-alpha RGBA bytes, for hosts that cannot block (the browser).
    pub async fn read_pixels_async(&self) -> Result<PixelBuffer> {
        let (buffer, w, h, bpr) = self.copy_out()?;
        let slot: Arc<Mutex<(Option<std::result::Result<(), wgpu::BufferAsyncError>>, Option<std::task::Waker>)>> = Arc::new(Mutex::new((None, None)));
        let s2 = slot.clone();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            if let Ok(mut g) = s2.lock() {
                g.0 = Some(r);
                if let Some(w) = g.1.take() {
                    w.wake();
                }
            }
        });
        // On native targets the device must be polled for the callback to run; in the browser the event loop does it.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
        }
        let result = std::future::poll_fn(|cx| {
            let mut g = slot.lock().expect("the read-back state is not poisoned");
            match g.0.take() {
                Some(r) => std::task::Poll::Ready(r),
                None => {
                    g.1 = Some(cx.waker().clone());
                    std::task::Poll::Pending
                }
            }
        })
        .await;
        result.map_err(|e| GraphicsError::Gpu(format!("the picture could not be read back: {e}")))?;
        let px = Self::unpack(&buffer.slice(..).get_mapped_range().map_err(|e| GraphicsError::Gpu(format!("{e}")))?, w, h, bpr)?;
        buffer.unmap();
        Ok(px)
    }

    fn copy_out(&self) -> Result<(wgpu::Buffer, u32, u32, u32)> {
        let t = self.targets.as_ref().ok_or_else(|| GraphicsError::invalid("nothing has been rendered yet"))?;
        let (w, h) = (t.width, t.height);
        let bpr = align_up(w as u64 * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64) as u32;
        let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("read-back"), size: bpr as u64 * h as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut enc = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("read-back") });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &t.resolve, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.gpu.queue.submit(Some(enc.finish()));
        Ok((buffer, w, h, bpr))
    }

    /// Remove the row padding and turn premultiplied alpha into straight alpha.
    fn unpack(mapped: &[u8], w: u32, h: u32, bpr: u32) -> Result<PixelBuffer> {
        let mut out = Vec::with_capacity(w as usize * h as usize * 4);
        for y in 0..h as usize {
            let row = &mapped[y * bpr as usize..y * bpr as usize + w as usize * 4];
            for px in row.chunks_exact(4) {
                let a = px[3];
                if a == 255 || a == 0 {
                    out.extend_from_slice(if a == 0 { &[0, 0, 0, 0] } else { px });
                } else {
                    let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                    out.extend_from_slice(&[un(px[0]), un(px[1]), un(px[2]), a]);
                }
            }
        }
        PixelBuffer::from_rgba(w, h, out)
    }
}

fn alpha_code(m: AlphaMode) -> u32 {
    match m {
        AlphaMode::Opaque => 0,
        AlphaMode::Mask => 1,
        AlphaMode::Blend => 2,
    }
}

fn vertex_layouts() -> [Option<wgpu::VertexBufferLayout<'static>>; 2] {
    const MESH_ATTRS: [wgpu::VertexAttribute; 5] = [
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 1 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 24, shader_location: 2 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint16x4, offset: 32, shader_location: 3 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 40, shader_location: 4 },
    ];
    const INSTANCE_ATTRS: [wgpu::VertexAttribute; 4] = [
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 5 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 6 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 7 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 48, shader_location: 8 },
    ];
    [
        Some(wgpu::VertexBufferLayout { array_stride: VERTEX_STRIDE, step_mode: wgpu::VertexStepMode::Vertex, attributes: &MESH_ATTRS }),
        Some(wgpu::VertexBufferLayout { array_stride: INSTANCE_STRIDE, step_mode: wgpu::VertexStepMode::Instance, attributes: &INSTANCE_ATTRS }),
    ]
}

fn light_uniform(l: &LightItem, shadow_slot: f32) -> LightU {
    let (kind, cos_outer, inv_range) = match l.kind {
        LightKind::Directional => (0.0, 0.0, 0.0),
        LightKind::Point => (1.0, 0.0, 0.0),
        LightKind::Spot { inner, outer } => {
            let (ci, co) = (inner.cos(), outer.cos());
            (2.0, co, 1.0 / (ci - co).max(1.0e-4))
        }
    };
    LightU {
        position: [l.position.x, l.position.y, l.position.z, 0.0],
        direction: [l.direction.x, l.direction.y, l.direction.z, 0.0],
        color: [l.color.x, l.color.y, l.color.z, shadow_slot],
        cone: [cos_outer, inv_range, kind, l.range],
    }
}

/// How far the shader shifts a point along its normal before looking it up in a shadow map: about
/// two texels of the largest shadow volume.
fn shadow_normal_offset(frame: &Frame) -> f32 {
    let mut b = super::math::Aabb::EMPTY;
    for d in &frame.draws {
        for i in &d.instances {
            b = b.union(&i.aabb);
        }
    }
    if b.is_valid() {
        (b.radius() * 2.0 / 2048.0 * 2.0).max(1.0e-4)
    } else {
        1.0e-3
    }
}

/// The background as premultiplied colours in the output encoding, and the clear colour of the pass.
fn background_colors(bg: &Background) -> ([f32; 4], [f32; 4], [f64; 4]) {
    let pm = |c: Rgba| {
        let a = c.a as f32 / 255.0;
        [c.r as f32 / 255.0 * a, c.g as f32 / 255.0 * a, c.b as f32 / 255.0 * a, a]
    };
    match *bg {
        Background::Color(c) => {
            let p = pm(c);
            (p, p, [p[0] as f64, p[1] as f64, p[2] as f64, p[3] as f64])
        }
        Background::Gradient { top, bottom } => {
            let (t, b) = (pm(top), pm(bottom));
            (t, b, [b[0] as f64, b[1] as f64, b[2] as f64, b[3] as f64])
        }
    }
}

/// Halve an RGBA image by averaging 2x2 blocks (colour weighted by alpha, so transparent texels add no colour).
fn box_down(src: &[u8], w: u32, h: u32, nw: u32, nh: u32) -> Vec<u8> {
    let mut out = vec![0u8; nw as usize * nh as usize * 4];
    for y in 0..nh {
        for x in 0..nw {
            let (mut r, mut g, mut b, mut a, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for dy in 0..2 {
                for dx in 0..2 {
                    let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                    let i = (sy as usize * w as usize + sx as usize) * 4;
                    let al = src[i + 3] as u32;
                    r += src[i] as u32 * al;
                    g += src[i + 1] as u32 * al;
                    b += src[i + 2] as u32 * al;
                    a += al;
                    n += 1;
                }
            }
            let o = (y as usize * nw as usize + x as usize) * 4;
            if a > 0 {
                out[o] = ((r + a / 2) / a) as u8;
                out[o + 1] = ((g + a / 2) / a) as u8;
                out[o + 2] = ((b + a / 2) / a) as u8;
            }
            out[o + 3] = ((a + n / 2) / n) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uniform_blocks_have_the_sizes_the_shaders_expect() {
        assert_eq!(FRAME_UNIFORM_BYTES, 832, "pbr.wgsl `Frame`");
        assert_eq!(DRAW_UNIFORM_BYTES, 64, "pbr.wgsl `Draw`");
        assert_eq!(SHADOW_UNIFORM_BYTES, 80, "shadow.wgsl `ShadowFrame`");
    }

    #[test]
    fn mips_average_and_keep_transparent_texels_colourless() {
        // 2x2: three opaque red and one fully transparent green.
        let src = [255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 0];
        let out = box_down(&src, 2, 2, 1, 1);
        assert_eq!(out, vec![255, 0, 0, 191], "the transparent green adds no colour, only less alpha");
    }

    #[test]
    fn backgrounds_are_premultiplied() {
        let (t, b, c) = background_colors(&Background::Color(Rgba::new(255, 0, 0, 128)));
        assert_eq!(t, b);
        assert!((t[0] - 128.0 / 255.0).abs() < 1e-6 && t[3] > 0.5 && t[3] < 0.51);
        assert!((c[3] as f32 - t[3]).abs() < 1e-6);
        let (t, b, _) = background_colors(&Background::Gradient { top: Rgba::WHITE, bottom: Rgba::BLACK });
        assert_eq!(t, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(b, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn straight_alpha_comes_back_from_premultiplied() {
        // One opaque pixel and one at alpha 128 holding premultiplied (64, 0, 0).
        let mapped = [10, 20, 30, 255, 64, 0, 0, 128];
        let px = Renderer::unpack(&mapped, 2, 1, 8).unwrap();
        assert_eq!(px.get(0, 0), Some(Rgba::new(10, 20, 30, 255)));
        let p = px.get(1, 0).unwrap();
        assert!(p.r >= 126 && p.r <= 128 && p.a == 128, "{p:?}");
    }
}
