use std::sync::Arc;

use bytemuck::cast_slice;
use glam::{DMat4, Mat4};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::prepare::{GpuTile, GpuTileKind};
use crate::vertex::{VertexPos, VertexPosTex};

const SOLID_WGSL: &str = include_str!("solid.wgsl");
const TEXTURED_WGSL: &str = include_str!("textured.wgsl");

/// Per-tile MVP uniform — 64 bytes.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MvpUniform {
    mvp: [[f32; 4]; 4],
}

pub struct Renderer {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    pipeline_solid: wgpu::RenderPipeline,
    pipeline_textured: wgpu::RenderPipeline,
    mvp_layout: wgpu::BindGroupLayout,
    pub texture_layout: Arc<wgpu::BindGroupLayout>,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // SAFETY: surface lives as long as `window`, and `window` is Arc-owned by the caller.
        let surface = instance.create_surface(window).expect("create_surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("no suitable adapter found");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("socle-viewer"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .expect("request_device");

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // ── bind group layouts ──────────────────────────────────────────────
        let mvp_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mvp_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let texture_layout = Arc::new(texture_layout);

        // ── pipelines ───────────────────────────────────────────────────────
        let solid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("solid_shader"),
            source: wgpu::ShaderSource::Wgsl(SOLID_WGSL.into()),
        });
        let textured_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("textured_shader"),
            source: wgpu::ShaderSource::Wgsl(TEXTURED_WGSL.into()),
        });

        let solid_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("solid_layout"),
            bind_group_layouts: &[&mvp_layout],
            push_constant_ranges: &[],
        });
        let textured_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("textured_pipeline_layout"),
                bind_group_layouts: &[&mvp_layout, &texture_layout],
                push_constant_ranges: &[],
            });

        let depth_format = wgpu::TextureFormat::Depth32Float;

        let pipeline_solid = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline_solid"),
            layout: Some(&solid_layout),
            vertex: wgpu::VertexState {
                module: &solid_shader,
                entry_point: Some("vs_main"),
                buffers: &[VertexPos::LAYOUT],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &solid_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let pipeline_textured = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline_textured"),
            layout: Some(&textured_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &textured_shader,
                entry_point: Some("vs_main"),
                buffers: &[VertexPosTex::LAYOUT],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &textured_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let (depth_texture, depth_view) =
            create_depth_texture(&device, size.width, size.height, depth_format);

        Self {
            device,
            queue,
            surface,
            surface_config,
            pipeline_solid,
            pipeline_textured,
            mvp_layout,
            texture_layout,
            depth_texture,
            depth_view,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        let (dt, dv) = create_depth_texture(
            &self.device,
            width,
            height,
            wgpu::TextureFormat::Depth32Float,
        );
        self.depth_texture = dt;
        self.depth_view = dv;
    }

    /// Draw one frame.
    ///
    /// `proj_view`: projection × view matrix (camera at ENU origin).
    /// `ecef_to_enu`: ECEF→ENU transform at camera position (f64 for tile transform composition).
    /// `tiles`: iterator of `(tile, world_transform_ecef)`.
    pub fn draw_frame<'a>(
        &self,
        proj_view: glam::Mat4,
        ecef_to_enu: DMat4,
        tiles: impl Iterator<Item = (&'a GpuTile, glam::DMat4)>,
    ) {
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => return,
            Err(e) => {
                log::error!("surface error: {e}");
                return;
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.05,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            for (tile, world_transform) in tiles {
                // Camera-relative: compose ecef_to_enu (f64) with world_transform (f64), cast to f32
                let model_to_enu: Mat4 = (ecef_to_enu * world_transform).as_mat4();
                let mvp = proj_view * model_to_enu;

                let uniform = MvpUniform {
                    mvp: mvp.to_cols_array_2d(),
                };

                let mvp_buf = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("mvp"),
                        contents: cast_slice(&[uniform]),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                let mvp_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("mvp_bg"),
                    layout: &self.mvp_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: mvp_buf.as_entire_binding(),
                    }],
                });

                match &tile.kind {
                    GpuTileKind::Solid {
                        vertex_buf,
                        index_buf,
                        index_count,
                    } => {
                        pass.set_pipeline(&self.pipeline_solid);
                        pass.set_bind_group(0, &mvp_bg, &[]);
                        pass.set_vertex_buffer(0, vertex_buf.slice(..));
                        pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..*index_count, 0, 0..1);
                    }
                    GpuTileKind::Textured {
                        vertex_buf,
                        index_buf,
                        index_count,
                        tex_bind_group,
                    } => {
                        pass.set_pipeline(&self.pipeline_textured);
                        pass.set_bind_group(0, &mvp_bg, &[]);
                        pass.set_bind_group(1, tex_bind_group, &[]);
                        pass.set_vertex_buffer(0, vertex_buf.slice(..));
                        pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..*index_count, 0, 0..1);
                    }
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
