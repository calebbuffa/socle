use std::sync::Arc;

use egaku::PrepareRendererResources;
use moderu::{
    ComponentType, GltfModel, get_position_accessor, get_texcoord_accessor, resolve_accessor_owned,
};
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::vertex::{VertexPos, VertexPosTex};

// ── Error ──────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum PrepareError {
    #[error("position accessor: {0}")]
    Position(#[from] moderu::AccessorViewError),
}

// ── CPU-side types (worker thread) ─────────────────────────────────────────

pub struct CpuImage {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
}

pub enum CpuMeshKind {
    Solid {
        vertices: Vec<VertexPos>,
        indices: Vec<u32>,
    },
    Textured {
        vertices: Vec<VertexPosTex>,
        indices: Vec<u32>,
        image: CpuImage,
    },
}

pub struct CpuMesh {
    pub kind: CpuMeshKind,
}

// ── GPU-side types (main thread) ────────────────────────────────────────────

pub enum GpuTileKind {
    Solid {
        vertex_buf: wgpu::Buffer,
        index_buf: wgpu::Buffer,
        index_count: u32,
    },
    Textured {
        vertex_buf: wgpu::Buffer,
        index_buf: wgpu::Buffer,
        index_count: u32,
        tex_bind_group: wgpu::BindGroup,
    },
}

pub struct GpuTile {
    pub kind: GpuTileKind,
}

// ── Preparer ────────────────────────────────────────────────────────────────

pub struct WgpuPreparer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    texture_layout: Arc<wgpu::BindGroupLayout>,
}

impl WgpuPreparer {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        texture_layout: Arc<wgpu::BindGroupLayout>,
    ) -> Self {
        Self {
            device,
            queue,
            texture_layout,
        }
    }
}

impl PrepareRendererResources for WgpuPreparer {
    type WorkerResult = Vec<CpuMesh>;
    type Content = Vec<GpuTile>;
    type Error = PrepareError;

    fn prepare_in_load_thread(&self, model: GltfModel) -> Result<Vec<CpuMesh>, PrepareError> {
        let mut meshes = Vec::new();

        for mesh in &model.meshes {
            for prim in &mesh.primitives {
                // ── positions ──────────────────────────────────────────
                let pos_view = get_position_accessor(&model, prim)?;

                // ── indices ────────────────────────────────────────────
                let indices: Vec<u32> = if let Some(idx) = prim.indices {
                    let acc = &model.accessors[idx];
                    match acc.component_type() {
                        ComponentType::UnsignedShort => {
                            let raw: Vec<u16> = resolve_accessor_owned(&model, idx)
                                .map_err(PrepareError::Position)?;
                            raw.into_iter().map(|v| v as u32).collect()
                        }
                        ComponentType::UnsignedByte => {
                            let raw: Vec<u8> = resolve_accessor_owned(&model, idx)
                                .map_err(PrepareError::Position)?;
                            raw.into_iter().map(|v| v as u32).collect()
                        }
                        _ => {
                            // UnsignedInt or any wider type
                            resolve_accessor_owned(&model, idx).map_err(PrepareError::Position)?
                        }
                    }
                } else {
                    (0..pos_view.len() as u32).collect()
                };

                // ── decide kind ────────────────────────────────────────
                let has_texture = prim
                    .material
                    .and_then(|mi| model.materials.get(mi))
                    .and_then(|mat| mat.pbr_metallic_roughness.as_ref())
                    .and_then(|pbr| pbr.base_color_texture.as_ref())
                    .is_some();

                if has_texture {
                    // ── texcoords ──────────────────────────────────────
                    let uv_set = prim
                        .material
                        .and_then(|mi| model.materials.get(mi))
                        .and_then(|mat| mat.pbr_metallic_roughness.as_ref())
                        .and_then(|pbr| pbr.base_color_texture.as_ref())
                        .map(|ti| ti.tex_coord as u8)
                        .unwrap_or(0);

                    let tc_view = get_texcoord_accessor(&model, prim, uv_set)
                        .map_err(PrepareError::Position)?;

                    let vertices: Vec<VertexPosTex> = pos_view
                        .iter()
                        .zip(tc_view.iter())
                        .map(|(p, uv)| VertexPosTex {
                            position: p.into(),
                            texcoord: uv,
                        })
                        .collect();

                    // ── image ──────────────────────────────────────────
                    let image = prim
                        .material
                        .and_then(|mi| model.materials.get(mi))
                        .and_then(|mat| mat.pbr_metallic_roughness.as_ref())
                        .and_then(|pbr| pbr.base_color_texture.as_ref())
                        .and_then(|ti| model.textures.get(ti.index))
                        .and_then(|tex| tex.source)
                        .and_then(|ii| model.images.get(ii))
                        .map(|img| CpuImage {
                            data: img.pixels.data.clone(),
                            width: img.pixels.width,
                            height: img.pixels.height,
                            channels: img.pixels.channels,
                        });

                    if let Some(image) = image {
                        meshes.push(CpuMesh {
                            kind: CpuMeshKind::Textured {
                                vertices,
                                indices,
                                image,
                            },
                        });
                        continue;
                    }
                    // Fall through to solid if image data was missing.
                    let positions: Vec<VertexPos> = pos_view
                        .iter()
                        .map(|p| VertexPos { position: p.into() })
                        .collect();
                    meshes.push(CpuMesh {
                        kind: CpuMeshKind::Solid {
                            vertices: positions,
                            indices,
                        },
                    });
                } else {
                    let vertices: Vec<VertexPos> = pos_view
                        .iter()
                        .map(|p| VertexPos { position: p.into() })
                        .collect();
                    meshes.push(CpuMesh {
                        kind: CpuMeshKind::Solid { vertices, indices },
                    });
                }
            }
        }

        Ok(meshes)
    }

    fn prepare_in_main_thread(&self, worker_result: Vec<CpuMesh>) -> Vec<GpuTile> {
        worker_result
            .into_iter()
            .map(|mesh| match mesh.kind {
                CpuMeshKind::Solid { vertices, indices } => {
                    let vertex_buf =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("tile_vertex"),
                                contents: bytemuck::cast_slice(&vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            });
                    let index_buf =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("tile_index"),
                                contents: bytemuck::cast_slice(&indices),
                                usage: wgpu::BufferUsages::INDEX,
                            });
                    GpuTile {
                        kind: GpuTileKind::Solid {
                            vertex_buf,
                            index_buf,
                            index_count: indices.len() as u32,
                        },
                    }
                }

                CpuMeshKind::Textured {
                    vertices,
                    indices,
                    image,
                } => {
                    let vertex_buf =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("tile_vertex_tex"),
                                contents: bytemuck::cast_slice(&vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            });
                    let index_buf =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("tile_index_tex"),
                                contents: bytemuck::cast_slice(&indices),
                                usage: wgpu::BufferUsages::INDEX,
                            });

                    let format = if image.channels == 4 {
                        wgpu::TextureFormat::Rgba8UnormSrgb
                    } else {
                        // Convert RGB → RGBA on the fly
                        wgpu::TextureFormat::Rgba8UnormSrgb
                    };

                    // Pad RGB → RGBA if needed
                    let rgba_data: Vec<u8> = if image.channels == 3 {
                        image
                            .data
                            .chunks_exact(3)
                            .flat_map(|c| [c[0], c[1], c[2], 255])
                            .collect()
                    } else {
                        image.data
                    };

                    let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("tile_texture"),
                        size: wgpu::Extent3d {
                            width: image.width,
                            height: image.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    self.queue.write_texture(
                        tex.as_image_copy(),
                        &rgba_data,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * image.width),
                            rows_per_image: None,
                        },
                        wgpu::Extent3d {
                            width: image.width,
                            height: image.height,
                            depth_or_array_layers: 1,
                        },
                    );

                    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                    let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                        address_mode_u: wgpu::AddressMode::ClampToEdge,
                        address_mode_v: wgpu::AddressMode::ClampToEdge,
                        mag_filter: wgpu::FilterMode::Linear,
                        min_filter: wgpu::FilterMode::Linear,
                        mipmap_filter: wgpu::FilterMode::Nearest,
                        ..Default::default()
                    });

                    let tex_bind_group =
                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("tile_tex_group"),
                            layout: &self.texture_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(&sampler),
                                },
                            ],
                        });

                    GpuTile {
                        kind: GpuTileKind::Textured {
                            vertex_buf,
                            index_buf,
                            index_count: indices.len() as u32,
                            tex_bind_group,
                        },
                    }
                }
            })
            .collect()
    }
}
