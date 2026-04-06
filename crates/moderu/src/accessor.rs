use crate::{Accessor, AccessorType, GltfModel, MeshPrimitive};
use std::marker::PhantomData;

impl MeshPrimitive {
    /// Zero-copy view over the `POSITION` (Vec3 f32) accessor.
    pub fn positions<'a>(
        &self,
        model: &'a GltfModel,
    ) -> Result<AccessorView<'a, glam::Vec3>, AccessorViewError> {
        get_position_accessor(model, self)
    }

    /// Zero-copy view over the `NORMAL` (Vec3 f32) accessor.
    pub fn normals<'a>(
        &self,
        model: &'a GltfModel,
    ) -> Result<AccessorView<'a, glam::Vec3>, AccessorViewError> {
        get_normal_accessor(model, self)
    }

    /// Zero-copy view over the `TEXCOORD_<set>` (Vec2 f32) accessor.
    pub fn tex_coords<'a>(
        &self,
        model: &'a GltfModel,
        set: u8,
    ) -> Result<AccessorView<'a, [f32; 2]>, AccessorViewError> {
        get_texcoord_accessor(model, self, set)
    }

    /// Zero-copy view over the index accessor typed as `u32`.
    ///
    /// Returns [`AccessorViewError::MissingAttribute`] if the primitive has no
    /// index buffer.
    pub fn indices_u32<'a>(
        &self,
        model: &'a GltfModel,
    ) -> Result<AccessorView<'a, u32>, AccessorViewError> {
        let idx = self
            .indices
            .ok_or_else(|| AccessorViewError::MissingAttribute("indices".into()))?;
        resolve_accessor(model, idx)
    }

    /// Zero-copy view over the index accessor typed as `u16`.
    pub fn indices_u16<'a>(
        &self,
        model: &'a GltfModel,
    ) -> Result<AccessorView<'a, u16>, AccessorViewError> {
        let idx = self
            .indices
            .ok_or_else(|| AccessorViewError::MissingAttribute("indices".into()))?;
        resolve_accessor(model, idx)
    }

    /// Zero-copy view over a named attribute accessor.
    ///
    /// `semantic` is anything that converts to a string — a `&str`, a
    /// [`crate::VertexAttribute`], or an [`crate::InstanceAttribute`].
    ///
    /// ```ignore
    /// let positions = prim.attribute::<glam::Vec3>(model, VertexAttribute::Position)?;
    /// let uvs       = prim.attribute::<[f32; 2]>(model, VertexAttribute::TexCoord(0))?;
    /// ```
    pub fn attribute<'a, T: bytemuck::Pod>(
        &self,
        model: &'a GltfModel,
        semantic: impl AsRef<str>,
    ) -> Result<AccessorView<'a, T>, AccessorViewError> {
        let key = semantic.as_ref();
        let &idx = self
            .attributes
            .get(key)
            .ok_or_else(|| AccessorViewError::MissingAttribute(key.to_owned()))?;
        resolve_accessor(model, idx)
    }
    /// Reads feature IDs as `u64`, widening from whatever integer type the accessor stores.
    ///
    /// `feature_id_index` selects which `_FEATURE_ID_<N>` attribute to read.
    pub fn feature_ids_as_u64(
        &self,
        model: &GltfModel,
        feature_id_index: usize,
    ) -> Result<Vec<u64>, AccessorViewError> {
        get_feature_id_as_u64(model, self, feature_id_index)
    }
}

impl AccessorType {
    /// The glTF string for this type (e.g. `"VEC3"`).
    #[inline]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scalar => "SCALAR",
            Self::Vec2 => "VEC2",
            Self::Vec3 => "VEC3",
            Self::Vec4 => "VEC4",
            Self::Mat2 => "MAT2",
            Self::Mat3 => "MAT3",
            Self::Mat4 => "MAT4",
        }
    }

    /// Number of scalar components in this type (e.g. `Vec3` -> 3, `Mat4` -> 16).
    #[inline]
    pub fn num_components(self) -> u8 {
        match self {
            Self::Scalar => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 => 4,
            Self::Mat2 => 4,
            Self::Mat3 => 9,
            Self::Mat4 => 16,
        }
    }
}

impl std::fmt::Display for AccessorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for AccessorType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SCALAR" => Ok(Self::Scalar),
            "VEC2" => Ok(Self::Vec2),
            "VEC3" => Ok(Self::Vec3),
            "VEC4" => Ok(Self::Vec4),
            "MAT2" => Ok(Self::Mat2),
            "MAT3" => Ok(Self::Mat3),
            "MAT4" => Ok(Self::Mat4),
            _ => Err(()),
        }
    }
}

/// Accessor component type as defined by the glTF specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentType {
    Byte,
    UnsignedByte,
    Short,
    UnsignedShort,
    Int,
    UnsignedInt,
    Float,
    Int64,
    UnsignedInt64,
    Double,
}

impl ComponentType {
    /// The glTF integer ID for this component type.
    pub fn id(self) -> i64 {
        match self {
            Self::Byte => 5120,
            Self::UnsignedByte => 5121,
            Self::Short => 5122,
            Self::UnsignedShort => 5123,
            Self::Int => 5124,
            Self::UnsignedInt => 5125,
            Self::Float => 5126,
            Self::Int64 => 5134,
            Self::UnsignedInt64 => 5135,
            Self::Double => 5130,
        }
    }

    pub fn from_id(id: i64) -> Option<Self> {
        match id {
            5120 => Some(Self::Byte),
            5121 => Some(Self::UnsignedByte),
            5122 => Some(Self::Short),
            5123 => Some(Self::UnsignedShort),
            5124 => Some(Self::Int),
            5125 => Some(Self::UnsignedInt),
            5126 => Some(Self::Float),
            5134 => Some(Self::Int64),
            5135 => Some(Self::UnsignedInt64),
            5130 => Some(Self::Double),
            _ => None,
        }
    }

    /// Byte size of a single component.
    #[inline]
    pub fn byte_size(self) -> u8 {
        use crate::AccessorComponentType as ACT;
        match self {
            // Delegate to AccessorComponentType for the glTF-standard 6 variants.
            Self::Byte => ACT::Byte.byte_size(),
            Self::UnsignedByte => ACT::UnsignedByte.byte_size(),
            Self::Short => ACT::Short.byte_size(),
            Self::UnsignedShort => ACT::UnsignedShort.byte_size(),
            Self::UnsignedInt => ACT::UnsignedInt.byte_size(),
            Self::Float => ACT::Float.byte_size(),
            // EXT_structural_metadata extras.
            Self::Int => 4,
            Self::Int64 | Self::UnsignedInt64 | Self::Double => 8,
        }
    }
}

impl crate::AccessorComponentType {
    /// Byte size of a single component of this type.
    #[inline]
    pub fn byte_size(self) -> u8 {
        match self {
            Self::Byte | Self::UnsignedByte => 1,
            Self::Short | Self::UnsignedShort => 2,
            Self::UnsignedInt | Self::Float => 4,
        }
    }

    /// The glTF integer ID for this component type (e.g. `5126` for `Float`).
    #[inline]
    pub fn gltf_id(self) -> u32 {
        match self {
            Self::Byte => 5120,
            Self::UnsignedByte => 5121,
            Self::Short => 5122,
            Self::UnsignedShort => 5123,
            Self::UnsignedInt => 5125,
            Self::Float => 5126,
        }
    }
}

impl From<crate::AccessorComponentType> for ComponentType {
    fn from(t: crate::AccessorComponentType) -> Self {
        use crate::AccessorComponentType;
        match t {
            AccessorComponentType::Byte => Self::Byte,
            AccessorComponentType::UnsignedByte => Self::UnsignedByte,
            AccessorComponentType::Short => Self::Short,
            AccessorComponentType::UnsignedShort => Self::UnsignedShort,
            AccessorComponentType::UnsignedInt => Self::UnsignedInt,
            AccessorComponentType::Float => Self::Float,
        }
    }
}

impl Accessor {
    #[inline]
    pub fn accessor_type(&self) -> AccessorType {
        self.r#type
    }

    #[inline]
    pub fn component_type(&self) -> ComponentType {
        ComponentType::from(self.component_type)
    }

    #[inline]
    pub fn num_components(&self) -> u8 {
        self.r#type.num_components()
    }

    #[inline]
    pub fn component_byte_size(&self) -> u8 {
        self.component_type().byte_size()
    }

    /// Bytes per vertex element (components × component size).
    pub fn bytes_per_vertex(&self) -> u64 {
        self.num_components() as u64 * self.component_byte_size() as u64
    }

    /// Byte stride, falling back to tight packing when the buffer view has none.
    pub fn byte_stride(&self, model: &GltfModel) -> Option<u64> {
        let bv = model.buffer_views.get(self.buffer_view?)?;
        if let Some(s) = bv.byte_stride {
            if s > 0 {
                return Some(s as u64);
            }
        }
        Some(self.bytes_per_vertex())
    }
}

/// Mutable write access to a typed accessor buffer (borrowed slice, in-place).
///
/// Obtain one via [`resolve_accessor_mut`] to mutate an existing accessor's elements
/// directly in the runtime buffer without any copy.
pub struct AccessorWriter<'a, T: bytemuck::Pod> {
    data: &'a mut [u8],
    count: usize,
    stride: usize,
    byte_offset: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: bytemuck::Pod> AccessorWriter<'a, T> {
    /// Build a writer over an existing mutable byte slice.
    ///
    /// `data` must already contain `count * stride` bytes starting at `byte_offset`.
    pub fn from_slice(data: &'a mut [u8], count: usize, stride: usize, byte_offset: usize) -> Self {
        Self {
            data,
            count,
            stride,
            byte_offset,
            _marker: PhantomData,
        }
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub fn get(&self, index: usize) -> Option<T> {
        if index >= self.count {
            return None;
        }
        let start = self.byte_offset + index * self.stride;
        let bytes = self.data.get(start..start + std::mem::size_of::<T>())?;
        Some(bytemuck::pod_read_unaligned(bytes))
    }
    /// Overwrite element at `index` in-place. Returns `false` if out of bounds.
    pub fn set(&mut self, index: usize, value: T) -> bool {
        if index >= self.count {
            return false;
        }
        let start = self.byte_offset + index * self.stride;
        match self.data.get_mut(start..start + std::mem::size_of::<T>()) {
            Some(bytes) => {
                bytes.copy_from_slice(bytemuck::bytes_of(&value));
                true
            }
            None => false,
        }
    }
    pub fn as_bytes(&self) -> &[u8] {
        self.data
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum AccessorViewError {
    #[error("accessor {0} not found")]
    AccessorNotFound(usize),
    #[error("buffer view {0} not found")]
    BufferViewNotFound(usize),
    #[error("buffer {0} not found")]
    BufferNotFound(usize),
    #[error("buffer too small: required {required} bytes, available {available}")]
    BufferTooSmall { required: usize, available: usize },
    #[error("missing attribute or field: '{0}'")]
    MissingAttribute(String),
    /// Sparse accessors are not supported in this context.
    #[error("sparse accessors are not supported")]
    SparseNotSupported,
    /// Expected accessor to have sparse data, but `sparse` field was `None`.
    #[error("expected sparse data but field was None")]
    NoSparseData,
    /// Accessor component type is not compatible with the requested type.
    #[error("incompatible component type id: {0}")]
    IncompatibleComponentType(i64),
    /// Accessor type (SCALAR/VEC2/…) does not match the expected type.
    #[error("incompatible accessor type: {0}")]
    IncompatibleType(String),
    /// General invalid accessor error.
    #[error("invalid accessor: {0}")]
    InvalidAccessor(String),
    /// Arithmetic overflow computing buffer offsets.
    #[error("arithmetic overflow computing buffer offsets")]
    Overflow,
}

/// Zero-copy typed view over a strided accessor buffer slice.
#[derive(Clone, Copy)]
pub struct AccessorView<'a, T: bytemuck::Pod> {
    data: &'a [u8],
    count: usize,
    stride: usize,
    byte_offset: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: bytemuck::Pod> AccessorView<'a, T> {
    pub fn new(data: &'a [u8], count: usize, stride: usize, byte_offset: usize) -> Self {
        Self {
            data,
            count,
            stride,
            byte_offset,
            _marker: PhantomData,
        }
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub fn get(&self, i: usize) -> Option<T> {
        if i >= self.count {
            return None;
        }
        let start = self.byte_offset + i * self.stride;
        let bytes = self.data.get(start..start + std::mem::size_of::<T>())?;
        Some(bytemuck::pod_read_unaligned(bytes))
    }
    pub fn iter(&self) -> AccessorIter<'a, T> {
        AccessorIter {
            data: self.data,
            count: self.count,
            stride: self.stride,
            byte_offset: self.byte_offset,
            idx: 0,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: bytemuck::Pod> IntoIterator for AccessorView<'a, T> {
    type Item = T;
    type IntoIter = AccessorIter<'a, T>;
    fn into_iter(self) -> AccessorIter<'a, T> {
        self.iter()
    }
}

impl<'a, T: bytemuck::Pod> IntoIterator for &'_ AccessorView<'a, T> {
    type Item = T;
    type IntoIter = AccessorIter<'a, T>;
    fn into_iter(self) -> AccessorIter<'a, T> {
        self.iter()
    }
}

/// Iterator for [`AccessorView`]. Stores data directly — no borrow of the view needed.
pub struct AccessorIter<'a, T: bytemuck::Pod> {
    data: &'a [u8],
    count: usize,
    stride: usize,
    byte_offset: usize,
    idx: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: bytemuck::Pod> Iterator for AccessorIter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.idx >= self.count {
            return None;
        }
        let start = self.byte_offset + self.idx * self.stride;
        let bytes = self.data.get(start..start + std::mem::size_of::<T>())?;
        self.idx += 1;
        Some(bytemuck::pod_read_unaligned(bytes))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let r = self.count.saturating_sub(self.idx);
        (r, Some(r))
    }
}
impl<'a, T: bytemuck::Pod> ExactSizeIterator for AccessorIter<'a, T> {}

#[must_use = "ignoring this view is likely a bug"]
pub fn resolve_accessor<'a, T: bytemuck::Pod>(
    model: &'a GltfModel,
    accessor_index: usize,
) -> Result<AccessorView<'a, T>, AccessorViewError> {
    let acc = model
        .accessors
        .get(accessor_index)
        .ok_or(AccessorViewError::AccessorNotFound(accessor_index))?;
    if acc.sparse.is_some() {
        return Err(AccessorViewError::SparseNotSupported);
    }
    // Verify T's byte size matches the accessor element size.
    let expected = acc.component_type().byte_size() as usize * acc.num_components() as usize;
    let actual = std::mem::size_of::<T>();
    if actual != expected {
        return Err(AccessorViewError::IncompatibleComponentType(
            ComponentType::from(acc.component_type).id(),
        ));
    }
    let bv_idx = acc
        .buffer_view
        .ok_or_else(|| AccessorViewError::MissingAttribute("no bufferView".into()))?;
    let bv = model
        .buffer_views
        .get(bv_idx)
        .ok_or(AccessorViewError::BufferViewNotFound(bv_idx))?;
    let buf: &[u8] = &model
        .buffers
        .get(bv.buffer)
        .ok_or(AccessorViewError::BufferNotFound(bv.buffer))?
        .data;
    let stride = bv.byte_stride.unwrap_or(std::mem::size_of::<T>());
    let needed = bv.byte_offset + acc.byte_offset + acc.count * stride;
    if needed > buf.len() {
        return Err(AccessorViewError::BufferTooSmall {
            required: needed,
            available: buf.len(),
        });
    }
    Ok(AccessorView::new(
        &buf[bv.byte_offset..],
        acc.count,
        stride,
        acc.byte_offset,
    ))
}

#[must_use = "ignoring this view is likely a bug"]
pub fn resolve_accessor_mut<'a, T: bytemuck::Pod>(
    model: &'a mut GltfModel,
    accessor_index: usize,
) -> Result<AccessorWriter<'a, T>, AccessorViewError> {
    // Copy what we need before taking a mutable borrow of model.buffers.
    let (buf_idx, bv_byte_offset, acc_byte_offset, count, stride, needed) = {
        let acc = model
            .accessors
            .get(accessor_index)
            .ok_or(AccessorViewError::AccessorNotFound(accessor_index))?;
        if acc.sparse.is_some() {
            return Err(AccessorViewError::SparseNotSupported);
        }
        let bv_idx = acc
            .buffer_view
            .ok_or_else(|| AccessorViewError::MissingAttribute("no bufferView".into()))?;
        let bv = model
            .buffer_views
            .get(bv_idx)
            .ok_or(AccessorViewError::BufferViewNotFound(bv_idx))?;
        let buf_idx = bv.buffer;
        let bv_byte_offset = bv.byte_offset;
        let acc_byte_offset = acc.byte_offset;
        let count = acc.count;
        let stride = bv.byte_stride.unwrap_or(std::mem::size_of::<T>());
        let needed = bv_byte_offset + acc_byte_offset + count * stride;
        (
            buf_idx,
            bv_byte_offset,
            acc_byte_offset,
            count,
            stride,
            needed,
        )
    };
    let buf = model
        .buffers
        .get_mut(buf_idx)
        .ok_or(AccessorViewError::BufferNotFound(buf_idx))?;
    if needed > buf.data.len() {
        return Err(AccessorViewError::BufferTooSmall {
            required: needed,
            available: buf.data.len(),
        });
    }
    Ok(AccessorWriter::from_slice(
        &mut buf.data[bv_byte_offset..],
        count,
        stride,
        acc_byte_offset,
    ))
}

pub(crate) fn get_position_accessor<'a>(
    model: &'a GltfModel,
    primitive: &MeshPrimitive,
) -> Result<AccessorView<'a, glam::Vec3>, AccessorViewError> {
    let idx = *primitive
        .attributes
        .get("POSITION")
        .ok_or_else(|| AccessorViewError::MissingAttribute("POSITION".into()))?;
    resolve_accessor(model, idx)
}

pub(crate) fn get_normal_accessor<'a>(
    model: &'a GltfModel,
    primitive: &MeshPrimitive,
) -> Result<AccessorView<'a, glam::Vec3>, AccessorViewError> {
    let idx = *primitive
        .attributes
        .get("NORMAL")
        .ok_or_else(|| AccessorViewError::MissingAttribute("NORMAL".into()))?;
    resolve_accessor(model, idx)
}

pub(crate) fn get_texcoord_accessor<'a>(
    model: &'a GltfModel,
    primitive: &MeshPrimitive,
    set: u8,
) -> Result<AccessorView<'a, [f32; 2]>, AccessorViewError> {
    // Avoid a heap allocation for the common sets (0–7).
    const NAMES: [&str; 8] = [
        "TEXCOORD_0",
        "TEXCOORD_1",
        "TEXCOORD_2",
        "TEXCOORD_3",
        "TEXCOORD_4",
        "TEXCOORD_5",
        "TEXCOORD_6",
        "TEXCOORD_7",
    ];
    let owned;
    let key: &str = if let Some(name) = NAMES.get(set as usize) {
        name
    } else {
        owned = format!("TEXCOORD_{set}");
        &owned
    };
    let idx = *primitive
        .attributes
        .get(key)
        .ok_or_else(|| AccessorViewError::MissingAttribute(key.to_owned()))?;
    resolve_accessor(model, idx)
}

/// Reads feature IDs as `u64`, widening from whatever integer type the accessor stores.
pub(crate) fn get_feature_id_as_u64(
    model: &GltfModel,
    primitive: &MeshPrimitive,
    feature_id_index: usize,
) -> Result<Vec<u64>, AccessorViewError> {
    let key = format!("_FEATURE_ID_{feature_id_index}");
    let &acc_idx = primitive
        .attributes
        .get(&key)
        .ok_or_else(|| AccessorViewError::MissingAttribute(key))?;
    let acc = model
        .accessors
        .get(acc_idx)
        .ok_or(AccessorViewError::AccessorNotFound(acc_idx))?;
    let bv_idx = acc
        .buffer_view
        .ok_or_else(|| AccessorViewError::MissingAttribute("no bufferView".into()))?;
    let bv = model
        .buffer_views
        .get(bv_idx)
        .ok_or(AccessorViewError::BufferViewNotFound(bv_idx))?;
    let buf: &[u8] = &model
        .buffers
        .get(bv.buffer)
        .ok_or(AccessorViewError::BufferNotFound(bv.buffer))?
        .data;
    let ct = ComponentType::from(acc.component_type);
    let stride = bv.byte_stride.unwrap_or(ct.byte_size() as usize);
    let base = bv.byte_offset + acc.byte_offset;
    let data: &[u8] = buf;
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let s = base + i * stride;
        let err = |n| AccessorViewError::BufferTooSmall {
            required: s + n,
            available: data.len(),
        };
        let val: u64 = match ct {
            ComponentType::Byte => {
                let v = i8::from_le_bytes([*data.get(s).ok_or_else(|| err(1))?]);
                if v < 0 {
                    return Err(AccessorViewError::InvalidAccessor(format!(
                        "negative signed feature ID {v} at index {i}"
                    )));
                }
                v as u64
            }
            ComponentType::UnsignedByte => *data.get(s).ok_or_else(|| err(1))? as u64,
            ComponentType::Short => i16::from_le_bytes(
                data.get(s..s + 2)
                    .and_then(|b| b.try_into().ok())
                    .ok_or_else(|| err(2))?,
            ) as u64,
            ComponentType::UnsignedShort => u16::from_le_bytes(
                data.get(s..s + 2)
                    .and_then(|b| b.try_into().ok())
                    .ok_or_else(|| err(2))?,
            ) as u64,
            ComponentType::Int => i32::from_le_bytes(
                data.get(s..s + 4)
                    .and_then(|b| b.try_into().ok())
                    .ok_or_else(|| err(4))?,
            ) as u64,
            ComponentType::UnsignedInt => u32::from_le_bytes(
                data.get(s..s + 4)
                    .and_then(|b| b.try_into().ok())
                    .ok_or_else(|| err(4))?,
            ) as u64,
            ComponentType::Float => f32::from_le_bytes(
                data.get(s..s + 4)
                    .and_then(|b| b.try_into().ok())
                    .ok_or_else(|| err(4))?,
            ) as u64,
            ComponentType::Int64 => i64::from_le_bytes(
                data.get(s..s + 8)
                    .and_then(|b| b.try_into().ok())
                    .ok_or_else(|| err(8))?,
            ) as u64,
            ComponentType::UnsignedInt64 => u64::from_le_bytes(
                data.get(s..s + 8)
                    .and_then(|b| b.try_into().ok())
                    .ok_or_else(|| err(8))?,
            ),
            ComponentType::Double => f64::from_le_bytes(
                data.get(s..s + 8)
                    .and_then(|b| b.try_into().ok())
                    .ok_or_else(|| err(8))?,
            ) as u64,
        };
        out.push(val);
    }
    Ok(out)
}

/// Decode a sparse accessor, applying deltas on top of base buffer data.
///
/// Returns an owned `Vec<u8>` (tightly-packed, no stride) containing exactly
/// `acc.count` elements of size `elem_bytes`.  Works for any element size.
fn decode_sparse(
    model: &GltfModel,
    acc: &crate::Accessor,
    elem_bytes: usize,
) -> Result<Vec<u8>, AccessorViewError> {
    use crate::AccessorComponentType;

    let count = acc.count;
    let sparse = acc.sparse.as_ref().ok_or(AccessorViewError::NoSparseData)?;

    // --- base data (may be absent for pure-sparse accessors) ---
    let mut out = if let Some(bv_idx) = acc.buffer_view {
        let bv = model
            .buffer_views
            .get(bv_idx)
            .ok_or(AccessorViewError::BufferViewNotFound(bv_idx))?;
        let buf: &[u8] = &model
            .buffers
            .get(bv.buffer)
            .ok_or(AccessorViewError::BufferNotFound(bv.buffer))?
            .data;
        let stride = bv.byte_stride.unwrap_or(elem_bytes);
        let base = bv.byte_offset + acc.byte_offset;
        let total = count
            .checked_mul(elem_bytes)
            .ok_or(AccessorViewError::Overflow)?;
        let mut v = vec![0u8; total];
        for i in 0..count {
            let src = base
                .checked_add(i.checked_mul(stride).ok_or(AccessorViewError::Overflow)?)
                .ok_or(AccessorViewError::Overflow)?;
            let dst = i
                .checked_mul(elem_bytes)
                .ok_or(AccessorViewError::Overflow)?;
            v[dst..dst + elem_bytes].copy_from_slice(buf.get(src..src + elem_bytes).ok_or(
                AccessorViewError::BufferTooSmall {
                    required: src + elem_bytes,
                    available: buf.len(),
                },
            )?);
        }
        v
    } else {
        vec![
            0u8;
            count
                .checked_mul(elem_bytes)
                .ok_or(AccessorViewError::Overflow)?
        ]
    };

    // --- sparse indices ---
    let idx_bv_idx = sparse.indices.buffer_view;
    let idx_bv = model
        .buffer_views
        .get(idx_bv_idx)
        .ok_or(AccessorViewError::BufferViewNotFound(idx_bv_idx))?;
    let idx_buf: &[u8] = &model
        .buffers
        .get(idx_bv.buffer)
        .ok_or(AccessorViewError::BufferNotFound(idx_bv.buffer))?
        .data;
    let idx_base = idx_bv.byte_offset + sparse.indices.byte_offset;
    let idx_comp_bytes = match sparse.indices.component_type {
        AccessorComponentType::UnsignedByte => 1usize,
        AccessorComponentType::UnsignedShort => 2,
        AccessorComponentType::UnsignedInt => 4,
        _ => 2, // fallback
    };
    let sparse_count = sparse.count as usize;

    let read_idx = |i: usize| -> Option<usize> {
        let s = idx_base + i * idx_comp_bytes;
        let b = idx_buf.get(s..s + idx_comp_bytes)?;
        Some(match idx_comp_bytes {
            1 => b[0] as usize,
            2 => u16::from_le_bytes(b.try_into().ok()?) as usize,
            4 => u32::from_le_bytes(b.try_into().ok()?) as usize,
            _ => 0,
        })
    };

    // --- sparse values ---
    let val_bv_idx = sparse.values.buffer_view;
    let val_bv = model
        .buffer_views
        .get(val_bv_idx)
        .ok_or(AccessorViewError::BufferViewNotFound(val_bv_idx))?;
    let val_buf: &[u8] = &model
        .buffers
        .get(val_bv.buffer)
        .ok_or(AccessorViewError::BufferNotFound(val_bv.buffer))?
        .data;
    let val_base = val_bv.byte_offset + sparse.values.byte_offset;

    for i in 0..sparse_count {
        let dest_idx = read_idx(i).ok_or(AccessorViewError::BufferTooSmall {
            required: idx_base + i * idx_comp_bytes + idx_comp_bytes,
            available: idx_buf.len(),
        })?;
        if dest_idx >= count {
            continue; // out-of-range sparse index — skip gracefully
        }
        let src = val_base + i * elem_bytes;
        let dst = dest_idx * elem_bytes;
        out[dst..dst + elem_bytes].copy_from_slice(val_buf.get(src..src + elem_bytes).ok_or(
            AccessorViewError::BufferTooSmall {
                required: src + elem_bytes,
                available: val_buf.len(),
            },
        )?);
    }

    Ok(out)
}

/// Like [`resolve_accessor`] but returns an owned buffer, which is required
/// when the accessor is sparse (deltas must be applied) or when the caller
/// needs data that outlives the borrow of `buffers`.
///
/// For non-sparse, dense accessors this copies the relevant bytes once.
#[must_use = "ignoring this view is likely a bug"]
pub fn resolve_accessor_owned<T: bytemuck::Pod>(
    model: &GltfModel,
    accessor_index: usize,
) -> Result<Vec<T>, AccessorViewError> {
    let acc = model
        .accessors
        .get(accessor_index)
        .ok_or(AccessorViewError::AccessorNotFound(accessor_index))?;

    let elem_bytes = std::mem::size_of::<T>();

    let raw = if acc.sparse.is_some() {
        decode_sparse(model, acc, elem_bytes)?
    } else {
        let bv_idx = acc
            .buffer_view
            .ok_or_else(|| AccessorViewError::MissingAttribute("no bufferView".into()))?;
        let bv = model
            .buffer_views
            .get(bv_idx)
            .ok_or(AccessorViewError::BufferViewNotFound(bv_idx))?;
        let buf: &[u8] = &model
            .buffers
            .get(bv.buffer)
            .ok_or(AccessorViewError::BufferNotFound(bv.buffer))?
            .data;
        let stride = bv.byte_stride.unwrap_or(elem_bytes);
        let base = bv.byte_offset + acc.byte_offset;
        let mut v = vec![0u8; acc.count * elem_bytes];
        for i in 0..acc.count {
            let src = base + i * stride;
            v[i * elem_bytes..i * elem_bytes + elem_bytes].copy_from_slice(
                buf.get(src..src + elem_bytes)
                    .ok_or(AccessorViewError::BufferTooSmall {
                        required: src + elem_bytes,
                        available: buf.len(),
                    })?,
            );
        }
        v
    };

    // Safety: T: Pod and we have exactly acc.count * size_of::<T>() bytes.
    let mut result: Vec<T> = vec![T::zeroed(); acc.count];
    bytemuck::cast_slice_mut::<T, u8>(&mut result).copy_from_slice(&raw);
    Ok(result)
}

impl GltfModel {
    /// Append typed data to the model's last buffer, registering a new
    /// `BufferView` and `Accessor`. Creates an empty buffer if none exist.
    ///
    /// Both the glTF component type and accessor shape are derived from `T`
    /// via [`crate::GltfData`] — no extra parameters needed.
    ///
    /// Returns the index of the newly added accessor.
    pub fn append_accessor<T: crate::GltfData>(&mut self, data: &[T]) -> usize {
        if self.buffers.is_empty() {
            self.buffers.push(crate::Buffer::default());
        }

        let buf_idx = self.buffers.len() - 1;
        let byte_offset = self.buffers[buf_idx].data.len();
        let raw: &[u8] = bytemuck::cast_slice(data);
        self.buffers[buf_idx].data.extend_from_slice(raw);
        self.buffers[buf_idx].byte_length = self.buffers[buf_idx].data.len();

        let bv_idx = self.buffer_views.len();
        self.buffer_views.push(crate::BufferView {
            buffer: buf_idx,
            byte_offset,
            byte_length: raw.len(),
            byte_stride: None,
            ..Default::default()
        });

        let acc_idx = self.accessors.len();
        self.accessors.push(crate::Accessor {
            buffer_view: Some(bv_idx),
            byte_offset: 0,
            component_type: T::COMPONENT_TYPE,
            count: data.len(),
            r#type: T::ACCESSOR_TYPE,
            ..Default::default()
        });

        acc_idx
    }
}
