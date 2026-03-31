//! [`GltfModelBuilder`] — ergonomic constructor for [`GltfModel`].
//!
//! Handles the bookkeeping of buffers, buffer views, and accessors so callers
//! can focus on the data rather than index wiring.
//!
//! # Example: Draco i3S → glTF
//! ```ignore
//! use moderu::{AccessorType, GltfModelBuilder};
//!
//! let mut b = GltfModelBuilder::new();
//!
//! let pos  = b.push_accessor(&positions_f32, AccessorType::Vec3);
//! let norm = b.push_accessor(&normals_f32,   AccessorType::Vec3);
//! let idxs = b.push_indices(&indices_u32);
//!
//! let prim = b.primitive()
//!     .indices(idxs)
//!     .attribute("POSITION", pos)
//!     .attribute("NORMAL",   norm)
//!     .build();
//!
//! b.mesh().primitive(prim).build();
//! let model = b.finish();
//! ```

use std::collections::HashMap;

use crate::{
    Accessor, AccessorComponentType, AccessorType, Asset, Buffer, BufferView, GltfModel, Mesh,
    MeshPrimitive, PrimitiveMode,
};

mod private {
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for u32 {}
    impl Sealed for u16 {}
    impl Sealed for u8 {}
    impl Sealed for i16 {}
    impl Sealed for i8 {}
}

/// Marker trait that associates a Rust scalar type with its glTF `AccessorComponentType`.
///
/// Implemented for `f32`, `u32`, `u16`, `u8`, `i16`, `i8`.
/// Sealed — cannot be implemented outside this crate.
pub trait GltfData: bytemuck::Pod + private::Sealed {
    const COMPONENT_TYPE: AccessorComponentType;
}

impl GltfData for f32 {
    const COMPONENT_TYPE: AccessorComponentType = AccessorComponentType::Float;
}
impl GltfData for u32 {
    const COMPONENT_TYPE: AccessorComponentType = AccessorComponentType::UnsignedInt;
}
impl GltfData for u16 {
    const COMPONENT_TYPE: AccessorComponentType = AccessorComponentType::UnsignedShort;
}
impl GltfData for u8 {
    const COMPONENT_TYPE: AccessorComponentType = AccessorComponentType::UnsignedByte;
}
impl GltfData for i16 {
    const COMPONENT_TYPE: AccessorComponentType = AccessorComponentType::Short;
}
impl GltfData for i8 {
    const COMPONENT_TYPE: AccessorComponentType = AccessorComponentType::Byte;
}

/// Typed index into `GltfModel::accessors`.
///
/// Returned by [`GltfModelBuilder::push_accessor`] and [`GltfModelBuilder::push_indices`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct AccessorIndex(pub usize);

/// Typed index into `GltfModel::buffer_views`.
///
/// Returned by [`GltfModelBuilder::push_raw`] and [`GltfModelBuilder::push_raw_strided`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct BufferViewIndex(pub usize);

/// Typed index into `GltfModel::meshes`.
///
/// Returned by [`GltfModelBuilder::push_mesh`] and [`MeshBuilder::build`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MeshIndex(pub usize);

/// Ergonomic builder for [`GltfModel`].
///
/// All binary data is accumulated into a single internal buffer.
/// Call [`finish`](GltfModelBuilder::finish) to get the completed model.
pub struct GltfModelBuilder {
    model: GltfModel,
    /// Index of the shared buffer all data is appended to.
    buf_idx: usize,
}

impl GltfModelBuilder {
    /// Create a new builder with a single shared buffer and glTF 2.0 asset metadata.
    pub fn new() -> Self {
        let mut model = GltfModel {
            asset: Asset {
                version: "2.0".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        model.buffers.push(Buffer::default());
        Self { model, buf_idx: 0 }
    }

    /// Append `bytes` to the shared buffer and register a buffer view over them.
    /// Returns the buffer view index.
    pub fn push_raw(&mut self, bytes: &[u8]) -> BufferViewIndex {
        let offset = self.model.buffers[self.buf_idx].data.len();
        self.model.buffers[self.buf_idx]
            .data
            .extend_from_slice(bytes);
        let bv_idx = self.model.buffer_views.len();
        self.model.buffer_views.push(BufferView {
            buffer: self.buf_idx,
            byte_offset: offset,
            byte_length: bytes.len(),
            ..Default::default()
        });
        BufferViewIndex(bv_idx)
    }

    /// Like [`push_raw`] but records an explicit `byteStride` for interleaved layouts.
    /// Only use this when multiple accessors share the same buffer view.
    pub fn push_raw_strided(&mut self, bytes: &[u8], byte_stride: usize) -> BufferViewIndex {
        let offset = self.model.buffers[self.buf_idx].data.len();
        self.model.buffers[self.buf_idx]
            .data
            .extend_from_slice(bytes);
        let bv_idx = self.model.buffer_views.len();
        self.model.buffer_views.push(BufferView {
            buffer: self.buf_idx,
            byte_offset: offset,
            byte_length: bytes.len(),
            byte_stride: Some(byte_stride),
            ..Default::default()
        });
        BufferViewIndex(bv_idx)
    }

    /// Push a typed attribute array and create an accessor for it.
    ///
    /// `T` must implement [`GltfData`] (`f32`, `u32`, `u16`, `u8`, `i16`, `i8`).
    /// `accessor_type` specifies the element shape (`Vec3`, `Vec2`, `Scalar`, …).
    ///
    /// ```ignore
    /// let pos = b.push_accessor(&positions_f32, AccessorType::Vec3);
    /// let uvs = b.push_accessor(&uvs_f32,       AccessorType::Vec2);
    /// ```
    pub fn push_accessor<T: GltfData>(
        &mut self,
        data: &[T],
        accessor_type: AccessorType,
    ) -> AccessorIndex {
        let bytes = bytemuck::cast_slice(data);
        let num_components = components_for_type(accessor_type);
        debug_assert_eq!(
            data.len() % num_components.max(1),
            0,
            "accessor data length ({}) is not divisible by num_components ({})",
            data.len(),
            num_components,
        );
        let count = data.len() / num_components.max(1);
        // Each accessor gets its own buffer view — no byteStride needed (tightly packed).
        let bv = self.push_raw(bytes);
        let acc_idx = self.model.accessors.len();
        self.model.accessors.push(Accessor {
            buffer_view: Some(bv.0),
            component_type: T::COMPONENT_TYPE,
            count,
            r#type: accessor_type,
            ..Default::default()
        });
        AccessorIndex(acc_idx)
    }

    /// Push a triangle index buffer and create a `Scalar` accessor for it.
    ///
    /// `T` is typically `u32` or `u16`.
    ///
    /// ```ignore
    /// let idxs = b.push_indices(&indices_u32);
    /// ```
    pub fn push_indices<T: GltfData>(&mut self, indices: &[T]) -> AccessorIndex {
        let bytes = bytemuck::cast_slice(indices);
        let bv = self.push_raw(bytes);
        let acc_idx = self.model.accessors.len();
        self.model.accessors.push(Accessor {
            buffer_view: Some(bv.0),
            component_type: T::COMPONENT_TYPE,
            count: indices.len(),
            r#type: AccessorType::Scalar,
            ..Default::default()
        });
        AccessorIndex(acc_idx)
    }

    /// Start building a [`MeshPrimitive`].
    pub fn primitive(&self) -> PrimitiveBuilder {
        PrimitiveBuilder::new()
    }

    /// Start building a [`Mesh`] that is pushed into the model on
    /// [`MeshBuilder::build`], which returns the mesh index.
    pub fn mesh(&mut self) -> MeshBuilder<'_> {
        MeshBuilder {
            builder: self,
            name: None,
            primitives: Vec::new(),
        }
    }

    /// Push a pre-built [`MeshPrimitive`] as a single-primitive mesh.
    /// Returns the mesh index.
    pub fn push_mesh(&mut self, primitive: MeshPrimitive) -> MeshIndex {
        let mesh_idx = self.model.meshes.len();
        self.model.meshes.push(Mesh {
            primitives: vec![primitive],
            ..Default::default()
        });
        MeshIndex(mesh_idx)
    }

    /// Finalise and return the [`GltfModel`].
    /// Updates `buffer.byte_length` to match the actual payload.
    pub fn finish(mut self) -> GltfModel {
        let len = self.model.buffers[self.buf_idx].data.len();
        self.model.buffers[self.buf_idx].byte_length = len;
        self.model
    }

    /// Borrow the model being built (e.g. to inspect indices mid-build).
    pub fn model(&self) -> &GltfModel {
        &self.model
    }
}

impl Default for GltfModelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for a single [`MeshPrimitive`].
pub struct PrimitiveBuilder {
    indices: Option<usize>,
    attributes: HashMap<String, usize>,
    mode: PrimitiveMode,
    material: Option<usize>,
}

impl PrimitiveBuilder {
    fn new() -> Self {
        Self {
            indices: None,
            attributes: HashMap::new(),
            mode: PrimitiveMode::Triangles,
            material: None,
        }
    }

    /// Set the index accessor.
    pub fn indices(mut self, acc: AccessorIndex) -> Self {
        self.indices = Some(acc.0);
        self
    }

    /// Add a vertex attribute (e.g. `"POSITION"`, `"NORMAL"`, `"TEXCOORD_0"`).
    pub fn attribute(mut self, semantic: impl Into<String>, acc: AccessorIndex) -> Self {
        self.attributes.insert(semantic.into(), acc.0);
        self
    }

    /// Set the primitive topology (default: `Triangles`).
    pub fn mode(mut self, mode: PrimitiveMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the material index.
    pub fn material(mut self, mat_idx: usize) -> Self {
        self.material = Some(mat_idx);
        self
    }

    /// Consume the builder and produce a [`MeshPrimitive`].
    pub fn build(self) -> MeshPrimitive {
        MeshPrimitive {
            indices: self.indices,
            attributes: self.attributes,
            mode: self.mode,
            material: self.material,
            ..Default::default()
        }
    }
}

/// Builder for a [`Mesh`] that is pushed into the model on [`build`](Self::build).
pub struct MeshBuilder<'a> {
    builder: &'a mut GltfModelBuilder,
    name: Option<String>,
    primitives: Vec<MeshPrimitive>,
}

impl<'a> MeshBuilder<'a> {
    /// Add a primitive.
    pub fn primitive(mut self, prim: MeshPrimitive) -> Self {
        self.primitives.push(prim);
        self
    }

    /// Set the mesh name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Push the mesh into the model and return its index.
    pub fn build(self) -> MeshIndex {
        let mesh_idx = self.builder.model.meshes.len();
        self.builder.model.meshes.push(Mesh {
            primitives: self.primitives,
            name: self.name,
            ..Default::default()
        });
        MeshIndex(mesh_idx)
    }
}

fn components_for_type(t: AccessorType) -> usize {
    match t {
        AccessorType::Scalar => 1,
        AccessorType::Vec2 => 2,
        AccessorType::Vec3 => 3,
        AccessorType::Vec4 => 4,
        AccessorType::Mat2 => 4,
        AccessorType::Mat3 => 9,
        AccessorType::Mat4 => 16,
    }
}
