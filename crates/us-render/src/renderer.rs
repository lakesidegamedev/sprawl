//! The wgpu renderer. Offscreen by default; a `SurfaceTarget` lets a host window present the
//! same picture. No UI or windowing code lives here.

use crate::camera::{Camera, View};
use crate::mesh::{Mesh, VERTEX_BYTES};
use crate::scene::Scene;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;

const SHADER: &str = include_str!("shader.wgsl");
const COLOUR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const AUX_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[derive(Clone, Debug)]
pub struct Style {
    /// Unit vector from the ground toward the sun (x east, y north, z up).
    pub sun: [f32; 3],
    /// Render at this many times the output size and average down. 2 gives crisp edges.
    pub supersample: u32,
    /// Shadow map side in texels.
    pub shadow_size: u32,
    /// Ink line width in render pixels, 1 or 2.
    pub ink_width: u32,
    /// Ink colour.
    pub ink: [u8; 3],
    /// Brightness multiplier of a shadowed face.
    pub shadow_tone: f32,
}

impl Default for Style {
    fn default() -> Style {
        // south east and high, the mockup sun
        let s = Vec3::new(0.55, -0.35, 0.76).normalize();
        Style {
            sun: [s.x, s.y, s.z],
            supersample: 2,
            shadow_size: 4096,
            ink_width: 2,
            ink: [28, 30, 38],
            shadow_tone: 0.72,
        }
    }
}

/// Where a host window gets its picture.
#[derive(Clone, Copy, Debug)]
pub enum SurfaceTarget {
    /// A `CAMetalLayer` (Apple).
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    MetalLayer(*mut std::ffi::c_void),
    /// A Windows window handle (`HWND`).
    Win32(isize),
    /// An X11 window id and the `Display*` it belongs to.
    #[cfg(unix)]
    Xlib {
        window: u64,
        display: *mut std::ffi::c_void,
    },
}

impl SurfaceTarget {
    fn into_wgpu(self) -> Result<wgpu::SurfaceTargetUnsafe, NoAdapter> {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
        };
        #[cfg(unix)]
        use raw_window_handle::{XlibDisplayHandle, XlibWindowHandle};
        let bad = |what: &str| NoAdapter(format!("no {what} to draw into"));
        Ok(match self {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            SurfaceTarget::MetalLayer(layer) => {
                wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer)
            }
            SurfaceTarget::Win32(hwnd) => {
                let hwnd = std::num::NonZeroIsize::new(hwnd).ok_or_else(|| bad("window"))?;
                wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(
                        RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
                    ),
                    raw_window_handle: RawWindowHandle::Win32(Win32WindowHandle::new(hwnd)),
                }
            }
            #[cfg(unix)]
            SurfaceTarget::Xlib { window, display } => {
                let display = std::ptr::NonNull::new(display).ok_or_else(|| bad("display"))?;
                if window == 0 {
                    return Err(bad("window"));
                }
                wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(RawDisplayHandle::Xlib(XlibDisplayHandle::new(
                        Some(display),
                        0,
                    ))),
                    raw_window_handle: RawWindowHandle::Xlib(XlibWindowHandle::new(
                        window as std::ffi::c_ulong,
                    )),
                }
            }
        })
    }
}

#[derive(Debug)]
pub struct NoAdapter(pub String);

impl std::fmt::Display for NoAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no usable graphics adapter: {}", self.0)
    }
}

impl std::error::Error for NoAdapter {}

#[derive(Clone, Debug)]
pub struct AdapterSummary {
    pub name: String,
    pub backend: String,
    pub device_type: String,
}

#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    pub width: u32,
    pub height: u32,
    pub draw_calls: usize,
    pub chunks_drawn: usize,
    pub chunks_total: usize,
    pub triangles: usize,
    /// Scene upload (first frame of a scene only).
    pub upload: Duration,
    /// Submit to finished readback.
    pub gpu_wait: Duration,
    pub total: Duration,
}

pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8, alpha 255.
    pub rgba: Vec<u8>,
    pub stats: FrameStats,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light_vp: [[f32; 4]; 4],
    sun: [f32; 4],
    ink: [f32; 4],
    shade: [f32; 4],
    screen: [f32; 4],
}

struct GpuMesh {
    vb: wgpu::Buffer,
    cast: Option<wgpu::Buffer>,
    plain: Option<wgpu::Buffer>,
    n_cast: u32,
    n_plain: u32,
}

struct GpuChunk {
    rev: u64,
    mesh: GpuMesh,
    lo: Vec3,
    hi: Vec3,
}

struct GpuScene {
    uid: u64,
    ground_rev: u64,
    ground: GpuMesh,
    chunks: Vec<GpuChunk>,
}

struct Targets {
    w: u32,
    h: u32,
    colour: wgpu::TextureView,
    aux: wgpu::TextureView,
    depth: wgpu::TextureView,
    out: wgpu::Texture,
    out_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    padded: u32,
    edge_group: wgpu::BindGroup,
}

struct ShadowMap {
    size: u32,
    view: wgpu::TextureView,
}

struct SurfaceState {
    surface: wgpu::Surface<'static>,
    format: wgpu::TextureFormat,
    alpha: wgpu::CompositeAlphaMode,
    size: Option<(u32, u32)>,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: AdapterSummary,
    layout0: wgpu::BindGroupLayout,
    layout1: wgpu::BindGroupLayout,
    module: wgpu::ShaderModule,
    edge_layout: wgpu::PipelineLayout,
    main_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    edge_pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    compare: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    shadow: Option<ShadowMap>,
    dummy_shadow: wgpu::TextureView,
    targets: Option<Targets>,
    scene: Option<GpuScene>,
    surface: Option<SurfaceState>,
}

fn vertex_layout() -> (wgpu::VertexBufferLayout<'static>, ()) {
    const ATTRS: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Unorm8x4, 3 => Uint32];
    (
        wgpu::VertexBufferLayout {
            array_stride: VERTEX_BYTES as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRS,
        },
        (),
    )
}

impl Renderer {
    /// A renderer that draws offscreen on whatever adapter wgpu finds. With no GPU that is the
    /// Mesa software Vulkan driver.
    pub fn new() -> Result<Renderer, NoAdapter> {
        Renderer::create(None)
    }

    /// A renderer that presents into a host window.
    ///
    /// # Safety
    /// The target must stay valid for as long as the renderer lives.
    pub unsafe fn for_window(target: SurfaceTarget) -> Result<Renderer, NoAdapter> {
        Renderer::create(Some(target))
    }

    /// A renderer that presents into a window from a safe handle (for example an
    /// `Arc<winit::window::Window>`). Works on every platform winit does, including Wayland.
    pub fn for_window_handle(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
    ) -> Result<Renderer, NoAdapter> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .map_err(|e| NoAdapter(e.to_string()))?;
        Renderer::create_with(instance, Some(surface))
    }

    fn create(target: Option<SurfaceTarget>) -> Result<Renderer, NoAdapter> {
        let instance = wgpu::Instance::default();
        let surface: Option<wgpu::Surface<'static>> = match target {
            None => None,
            Some(target) => Some(
                unsafe { instance.create_surface_unsafe(target.into_wgpu()?) }
                    .map_err(|e| NoAdapter(e.to_string()))?,
            ),
        };
        Renderer::create_with(instance, surface)
    }

    fn create_with(
        instance: wgpu::Instance,
        surface: Option<wgpu::Surface<'static>>,
    ) -> Result<Renderer, NoAdapter> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface.as_ref(),
            apply_limit_buckets: false,
        }))
        .map_err(|e| NoAdapter(e.to_string()))?;
        let surface = match surface {
            None => None,
            Some(surface) => {
                let caps = surface.get_capabilities(&adapter);
                // the shader writes display colours as they are, so the surface must be linear
                let format = [
                    wgpu::TextureFormat::Bgra8Unorm,
                    wgpu::TextureFormat::Rgba8Unorm,
                ]
                .into_iter()
                .find(|f| caps.formats.contains(f))
                .ok_or_else(|| NoAdapter("the display has no linear colour format".into()))?;
                let alpha = caps
                    .alpha_modes
                    .first()
                    .copied()
                    .unwrap_or(wgpu::CompositeAlphaMode::Auto);
                Some(SurfaceState {
                    surface,
                    format,
                    alpha,
                    size: None,
                })
            }
        };
        let ai = adapter.get_info();
        let info = AdapterSummary {
            name: ai.name.clone(),
            backend: format!("{:?}", ai.backend),
            device_type: format!("{:?}", ai.device_type),
        };
        let request = |limits: wgpu::Limits| {
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("us-render"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                ..Default::default()
            }))
        };
        let (device, queue) =
            match request(wgpu::Limits::default().using_resolution(adapter.limits())) {
                Ok(d) => d,
                Err(_) => request(adapter.limits()).map_err(|e| NoAdapter(e.to_string()))?,
            };

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty,
            count: None,
        };
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let layout0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[
                entry(
                    0,
                    vf,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(
                    1,
                    vf,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    2,
                    vf,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                ),
            ],
        });
        let tex_unfilterable = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let layout1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("edge inputs"),
            entries: &[
                entry(0, wgpu::ShaderStages::FRAGMENT, tex_unfilterable),
                entry(1, wgpu::ShaderStages::FRAGMENT, tex_unfilterable),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&layout0)],
            immediate_size: 0,
        });
        let edge_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("edge"),
            bind_group_layouts: &[Some(&layout0), Some(&layout1)],
            immediate_size: 0,
        });
        let (vbl, _) = vertex_layout();
        let depth_state = |bias: wgpu::DepthBiasState| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias,
        };
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[Some(vbl.clone())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_state(wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            })),
            multisample: wgpu::MultisampleState::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        let main_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("main"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(vbl)],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(depth_state(wgpu::DepthBiasState::default())),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[
                    Some(wgpu::ColorTargetState {
                        format: COLOUR_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: AUX_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
            }),
            multiview_mask: None,
            cache: None,
        });
        let compare = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dummy_shadow = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("dummy shadow"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Ok(Renderer {
            device,
            queue,
            info,
            layout0,
            layout1,
            module,
            edge_layout,
            main_pipeline,
            shadow_pipeline,
            edge_pipelines: Vec::new(),
            compare,
            uniforms,
            shadow: None,
            dummy_shadow,
            targets: None,
            scene: None,
            surface,
        })
    }

    pub fn adapter(&self) -> &AdapterSummary {
        &self.info
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Format of the window surface, if this renderer has one.
    pub fn surface_format(&self) -> Option<wgpu::TextureFormat> {
        self.surface.as_ref().map(|s| s.format)
    }

    fn edge_pipeline_index(&mut self, format: wgpu::TextureFormat) -> usize {
        if let Some(i) = self.edge_pipelines.iter().position(|(f, _)| *f == format) {
            return i;
        }
        let p = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("edge"),
                layout: Some(&self.edge_layout),
                vertex: wgpu::VertexState {
                    module: &self.module,
                    entry_point: Some("vs_full"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.module,
                    entry_point: Some("fs_edge"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        self.edge_pipelines.push((format, p));
        self.edge_pipelines.len() - 1
    }

    fn upload_mesh(&self, m: &Mesh, label: &str) -> GpuMesh {
        let buf = |data: &[u8], usage| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: data,
                    usage,
                })
        };
        let zero = [crate::mesh::Vertex::zeroed()];
        let verts: &[crate::mesh::Vertex] = if m.vertices.is_empty() {
            &zero
        } else {
            &m.vertices
        };
        let vb = buf(bytemuck::cast_slice(verts), wgpu::BufferUsages::VERTEX);
        let idx = |v: &Vec<u32>| {
            (!v.is_empty()).then(|| buf(bytemuck::cast_slice(v), wgpu::BufferUsages::INDEX))
        };
        GpuMesh {
            vb,
            cast: idx(&m.cast),
            plain: idx(&m.plain),
            n_cast: m.cast.len() as u32,
            n_plain: m.plain.len() as u32,
        }
    }

    fn ensure_scene(&mut self, scene: &Scene) {
        let same = self.scene.as_ref().is_some_and(|s| s.uid == scene.uid);
        if !same {
            self.scene = None;
            let ground = self.upload_mesh(&scene.ground, "ground");
            let chunks = scene
                .chunks
                .iter()
                .map(|c| GpuChunk {
                    rev: c.rev,
                    mesh: self.upload_mesh(&c.mesh, "chunk"),
                    lo: c.lo,
                    hi: c.hi,
                })
                .collect();
            self.scene = Some(GpuScene {
                uid: scene.uid,
                ground_rev: scene.ground_rev,
                ground,
                chunks,
            });
            return;
        }
        // same scene: upload only what changed
        let mut gpu = self.scene.take().unwrap();
        if gpu.ground_rev != scene.ground_rev {
            gpu.ground = self.upload_mesh(&scene.ground, "ground");
            gpu.ground_rev = scene.ground_rev;
        }
        gpu.chunks.truncate(scene.chunks.len());
        for (i, c) in scene.chunks.iter().enumerate() {
            if i < gpu.chunks.len() && gpu.chunks[i].rev == c.rev {
                continue;
            }
            let g = GpuChunk {
                rev: c.rev,
                mesh: self.upload_mesh(&c.mesh, "chunk"),
                lo: c.lo,
                hi: c.hi,
            };
            if i < gpu.chunks.len() {
                gpu.chunks[i] = g;
            } else {
                gpu.chunks.push(g);
            }
        }
        self.scene = Some(gpu);
    }

    fn ensure_shadow(&mut self, size: u32) {
        if self.shadow.as_ref().is_some_and(|s| s.size == size) {
            return;
        }
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.shadow = Some(ShadowMap {
            size,
            view: tex.create_view(&Default::default()),
        });
    }

    fn ensure_targets(&mut self, w: u32, h: u32, out_format: wgpu::TextureFormat) {
        if self.targets.as_ref().is_some_and(|t| t.w == w && t.h == h) {
            return;
        }
        self.targets = None;
        let make = |label: &str, format, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let rt = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let colour = make("colour", COLOUR_FORMAT, rt).create_view(&Default::default());
        let aux = make("aux", AUX_FORMAT, rt).create_view(&Default::default());
        let depth = make(
            "depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        let out = make(
            "out",
            out_format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let out_view = out.create_view(&Default::default());
        let padded = (w * 4).div_ceil(256) * 256;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: padded as u64 * h as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let edge_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("edge inputs"),
            layout: &self.layout1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&colour),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&aux),
                },
            ],
        });
        self.targets = Some(Targets {
            w,
            h,
            colour,
            aux,
            depth,
            out,
            out_view,
            readback,
            padded,
            edge_group,
        });
    }

    /// Render and return tightly packed RGBA8 at `width` x `height`.
    pub fn render_to_rgba(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        style: &Style,
        width: u32,
        height: u32,
    ) -> Frame {
        let t_all = Instant::now();
        let ss = style.supersample.max(1);
        let (rw, rh) = (width * ss, height * ss);
        let mut stats = FrameStats {
            width,
            height,
            ..Default::default()
        };
        let t0 = Instant::now();
        self.ensure_scene(scene);
        stats.upload = t0.elapsed();
        self.ensure_targets(rw, rh, COLOUR_FORMAT);
        let out_view = self.targets.as_ref().unwrap().out_view.clone();
        let mut enc = self.device.create_command_encoder(&Default::default());
        self.draw_frame(
            &mut enc,
            scene,
            camera,
            style,
            rw,
            rh,
            &out_view,
            COLOUR_FORMAT,
            &mut stats,
        );
        let tg = self.targets.as_ref().unwrap();
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &tg.out,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &tg.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(tg.padded),
                    rows_per_image: Some(rh),
                },
            },
            wgpu::Extent3d {
                width: rw,
                height: rh,
                depth_or_array_layers: 1,
            },
        );
        let t1 = Instant::now();
        self.queue.submit([enc.finish()]);
        let slice = tg.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback"));
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
        stats.gpu_wait = t1.elapsed();
        let data = slice.get_mapped_range().expect("mapped");
        let mut rgba = vec![255u8; (width * height * 4) as usize];
        let area = ss * ss;
        for y in 0..height {
            for x in 0..width {
                let mut acc = [0u32; 3];
                for sy in 0..ss {
                    let row = &data[((y * ss + sy) * tg.padded) as usize..];
                    for sx in 0..ss {
                        let s = ((x * ss + sx) * 4) as usize;
                        acc[0] += row[s] as u32;
                        acc[1] += row[s + 1] as u32;
                        acc[2] += row[s + 2] as u32;
                    }
                }
                let d = ((y * width + x) * 4) as usize;
                rgba[d] = ((acc[0] + area / 2) / area) as u8;
                rgba[d + 1] = ((acc[1] + area / 2) / area) as u8;
                rgba[d + 2] = ((acc[2] + area / 2) / area) as u8;
            }
        }
        drop(data);
        tg.readback.unmap();
        stats.total = t_all.elapsed();
        Frame {
            width,
            height,
            rgba,
            stats,
        }
    }

    /// Render straight into the window surface given to `for_window`, at the surface's size.
    /// Returns None when the surface has no frame to draw into right now.
    pub fn render_to_surface(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        style: &Style,
        width: u32,
        height: u32,
    ) -> Option<FrameStats> {
        self.render_to_surface_with(scene, camera, style, width, height, |_, _, _, _| {})
    }

    /// Like `render_to_surface`, then calls `overlay(device, queue, encoder, surface_view)` so the
    /// host can draw its UI on top in the same frame before it is presented.
    pub fn render_to_surface_with(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        style: &Style,
        width: u32,
        height: u32,
        overlay: impl FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) -> Option<FrameStats> {
        let t_all = Instant::now();
        let (format, frame) = {
            let device = &self.device;
            let st = self.surface.as_mut()?;
            if st.size != Some((width, height)) {
                st.surface.configure(
                    device,
                    &wgpu::SurfaceConfiguration {
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        format: st.format,
                        color_space: wgpu::SurfaceColorSpace::Auto,
                        width,
                        height,
                        present_mode: wgpu::PresentMode::AutoVsync,
                        desired_maximum_frame_latency: 2,
                        alpha_mode: st.alpha,
                        view_formats: vec![],
                    },
                );
                st.size = Some((width, height));
            }
            let frame = match st.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(f)
                | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                _ => {
                    st.size = None;
                    return None;
                }
            };
            (st.format, frame)
        };
        let mut stats = FrameStats {
            width,
            height,
            ..Default::default()
        };
        self.ensure_scene(scene);
        self.ensure_targets(width, height, format);
        let view = frame.texture.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        let one = Style {
            supersample: 1,
            ..style.clone()
        };
        self.draw_frame(
            &mut enc, scene, camera, &one, width, height, &view, format, &mut stats,
        );
        overlay(&self.device, &self.queue, &mut enc, &view);
        self.queue.submit([enc.finish()]);
        self.queue.present(frame);
        stats.total = t_all.elapsed();
        Some(stats)
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_frame(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        scene: &Scene,
        camera: &Camera,
        style: &Style,
        rw: u32,
        rh: u32,
        final_view: &wgpu::TextureView,
        final_format: wgpu::TextureFormat,
        stats: &mut FrameStats,
    ) {
        let aspect = rw as f32 / rh as f32;
        let view = camera.view(aspect);
        let sun = Vec3::from(style.sun).normalize();
        let (light_vp, texel, range) = light_matrix(scene, &view, sun, style.shadow_size);
        self.ensure_shadow(style.shadow_size);
        let edge_i = self.edge_pipeline_index(final_format);
        let globals = Globals {
            view_proj: view.view_proj.to_cols_array_2d(),
            light_vp: light_vp.to_cols_array_2d(),
            sun: [sun.x, sun.y, sun.z, texel],
            ink: [
                style.ink[0] as f32 / 255.0,
                style.ink[1] as f32 / 255.0,
                style.ink[2] as f32 / 255.0,
                style.ink_width as f32,
            ],
            shade: [style.shadow_tone, 0.2 / range, 0.0, 0.0],
            screen: [rw as f32, rh as f32, 0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&globals));
        let shadow = self.shadow.as_ref().unwrap();
        let group0 = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &self.layout0,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.compare),
                },
            ],
        });
        // the shadow pass writes the shadow map, so it binds a stand-in for reading
        let group0_shadow = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals (shadow pass)"),
            layout: &self.layout0,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_shadow),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.compare),
                },
            ],
        });
        let gpu = self.scene.as_ref().unwrap();
        let tg = self.targets.as_ref().unwrap();
        let visible_main: Vec<&GpuChunk> = gpu
            .chunks
            .iter()
            .filter(|c| in_view(&view.view_proj, c.lo, c.hi))
            .collect();
        let visible_light: Vec<&GpuChunk> = gpu
            .chunks
            .iter()
            .filter(|c| in_view(&light_vp, c.lo, c.hi))
            .collect();
        stats.chunks_total = gpu.chunks.len();
        stats.chunks_drawn = visible_main.len();

        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &shadow.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, Some(&group0_shadow), &[]);
            for c in &visible_light {
                if let Some(ib) = &c.mesh.cast {
                    pass.set_vertex_buffer(0, c.mesh.vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..c.mesh.n_cast, 0, 0..1);
                }
            }
        }
        {
            let clear = wgpu::Color {
                r: crate::scene::GRASS[0] as f64 / 255.0,
                g: crate::scene::GRASS[1] as f64 / 255.0,
                b: crate::scene::GRASS[2] as f64 / 255.0,
                a: 1.0,
            };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &tg.colour,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &tg.aux,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 1.0e9,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &tg.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.main_pipeline);
            pass.set_bind_group(0, Some(&group0), &[]);
            let mut draw = |m: &GpuMesh, stats: &mut FrameStats| {
                pass.set_vertex_buffer(0, m.vb.slice(..));
                for (ib, n) in [(&m.plain, m.n_plain), (&m.cast, m.n_cast)] {
                    if let Some(ib) = ib {
                        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..n, 0, 0..1);
                        stats.draw_calls += 1;
                        stats.triangles += n as usize / 3;
                    }
                }
            };
            draw(&gpu.ground, stats);
            for c in &visible_main {
                draw(&c.mesh, stats);
            }
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("edge"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: final_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.edge_pipelines[edge_i].1);
            pass.set_bind_group(0, Some(&group0), &[]);
            pass.set_bind_group(1, Some(&tg.edge_group), &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

/// False when every corner of the box lies outside the same side of the view volume.
fn in_view(vp: &Mat4, lo: Vec3, hi: Vec3) -> bool {
    let mut out = [0u32; 4];
    for i in 0..8 {
        let p = Vec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        );
        let c = *vp * p.extend(1.0);
        if c.w <= 1e-4 {
            // behind the eye: keep it, the plane tests below are not valid
            return true;
        }
        if c.x < -c.w {
            out[0] += 1;
        }
        if c.x > c.w {
            out[1] += 1;
        }
        if c.y < -c.w {
            out[2] += 1;
        }
        if c.y > c.w {
            out[3] += 1;
        }
    }
    out.iter().all(|&n| n < 8)
}

/// Orthographic light matrix that covers the ground the camera sees (inside the map) up to
/// building height. Returns it with the texel size in metres and the depth range in metres.
fn light_matrix(scene: &Scene, view: &View, sun: Vec3, size: u32) -> (Mat4, f32, f32) {
    let lim = (-100.0, scene.size + 100.0);
    let mut pts: Vec<Vec3> = Vec::new();
    for (x, y) in [
        (-1.0, -1.0),
        (1.0, -1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
        (0.0, -1.0),
        (0.0, 1.0),
    ] {
        let near = view.inv_view_proj * Vec4::new(x, y, 0.0, 1.0);
        let far = view.inv_view_proj * Vec4::new(x, y, 1.0, 1.0);
        let (a, b) = (near.truncate() / near.w, far.truncate() / far.w);
        let dir = (b - a).normalize();
        let t = if dir.z < -0.02 {
            (-a.z / dir.z).min(view.far)
        } else {
            view.far
        };
        let p = a + dir * t;
        pts.push(Vec3::new(
            p.x.clamp(lim.0, lim.1),
            p.y.clamp(lim.0, lim.1),
            0.0,
        ));
    }
    let zmax = 70.0;
    let mut corners = Vec::new();
    for p in &pts {
        corners.push(*p);
        corners.push(Vec3::new(p.x, p.y, zmax));
    }
    let centre = corners.iter().fold(Vec3::ZERO, |a, b| a + *b) / corners.len() as f32;
    let lv = Mat4::look_at_rh(centre + sun * 3000.0, centre, Vec3::Z);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for c in &corners {
        let p = lv.transform_point3(*c);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let half = ((hi.x - lo.x).max(hi.y - lo.y) / 2.0) * 1.03 + 6.0;
    let (cx, cy) = ((hi.x + lo.x) / 2.0, (hi.y + lo.y) / 2.0);
    let (near, far) = (-hi.z - 120.0, -lo.z + 120.0);
    let proj = Mat4::orthographic_rh(cx - half, cx + half, cy - half, cy + half, near, far);
    (proj * lv, 2.0 * half / size as f32, far - near)
}
