use crate::{Accessor, AccessorType, GltfModel, MeshPrimitive};
use std::marker::PhantomData;

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

#[derive(Debug, Clone)]
pub enum AccessorViewError {
    AccessorNotFound(usize),
    BufferViewNotFound(usize),
    BufferNotFound(usize),
    BufferTooSmall {
        required: usize,
        available: usize,
    },
    MissingAttribute(String),
    /// Sparse accessors are not supported in this context.
    SparseNotSupported,
    /// Accessor component type is not compatible with the requested type.
    IncompatibleComponentType(i64),
    /// Accessor type (SCALAR/VEC2/…) does not match the expected type.
    IncompatibleType(String),
    /// Sparse accessor not supported in typed views.
    SparseAccessorNotSupported,
    /// General invalid accessor error.
    InvalidAccessor(String),
    /// Arithmetic overflow computing buffer offsets.
    Overflow,
}

impl std::fmt::Display for AccessorViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccessorNotFound(i) => write!(f, "accessor {i} not found"),
            Self::BufferViewNotFound(i) => write!(f, "buffer view {i} not found"),
            Self::BufferNotFound(i) => write!(f, "buffer {i} not found"),
            Self::BufferTooSmall {
                required,
                available,
            } => write!(
                f,
                "buffer too small: required {required} bytes, available {available}"
            ),
            Self::MissingAttribute(key) => write!(f, "missing attribute or field: '{key}'"),
            Self::SparseNotSupported => write!(f, "sparse accessors are not supported"),
            Self::IncompatibleComponentType(id) => {
                write!(f, "incompatible component type id: {id}")
            }
            Self::IncompatibleType(msg) => write!(f, "incompatible accessor type: {msg}"),
            Self::SparseAccessorNotSupported => {
                write!(f, "sparse accessors are not supported in typed views")
            }
            Self::InvalidAccessor(msg) => write!(f, "invalid accessor: {msg}"),
            Self::Overflow => write!(f, "arithmetic overflow computing buffer offsets"),
        }
    }
}
impl std::error::Error for AccessorViewError {}

/// Zero-copy typed view over a strided accessor buffer slice.
pub struct AccessorTypedView<'a, T: bytemuck::Pod> {
    data: &'a [u8],
    count: usize,
    stride: usize,
    byte_offset: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: bytemuck::Pod> AccessorTypedView<'a, T> {
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
    pub fn iter(&self) -> AccessorViewIter<'a, T> {
        AccessorViewIter {
            data: self.data,
            count: self.count,
            stride: self.stride,
            byte_offset: self.byte_offset,
            idx: 0,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: bytemuck::Pod> IntoIterator for &'_ AccessorTypedView<'a, T> {
    type Item = T;
    type IntoIter = AccessorViewIter<'a, T>;
    fn into_iter(self) -> AccessorViewIter<'a, T> {
        self.iter()
    }
}

/// Iterator for [`AccessorTypedView`]. Stores data directly — no borrow of the view needed.
pub struct AccessorViewIter<'a, T: bytemuck::Pod> {
    data: &'a [u8],
    count: usize,
    stride: usize,
    byte_offset: usize,
    idx: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: bytemuck::Pod> Iterator for AccessorViewIter<'a, T> {
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
impl<'a, T: bytemuck::Pod> ExactSizeIterator for AccessorViewIter<'a, T> {}

/// Like [`resolve_accessor`] but also verifies that `T`'s byte size matches the
/// accessor's element size (`component_byte_size × num_components`).
///
/// This catches mismatches like calling `resolve_accessor::<f32>` on a `u16` accessor
/// at compile time would be impossible but at runtime is trivially detectable.
pub fn resolve_accessor_checked<'a, T: bytemuck::Pod>(
    model: &'a GltfModel,
    accessor_index: usize,
) -> Result<AccessorTypedView<'a, T>, AccessorViewError> {
    let acc = model
        .accessors
        .get(accessor_index)
        .ok_or(AccessorViewError::AccessorNotFound(accessor_index))?;
    let expected = acc.component_type().byte_size() as usize * acc.num_components() as usize;
    let actual = std::mem::size_of::<T>();
    if actual != expected {
        return Err(AccessorViewError::IncompatibleComponentType(
            ComponentType::from(acc.component_type).id(),
        ));
    }
    resolve_accessor(model, accessor_index)
}

/// Mutable counterpart of [`resolve_accessor`]: borrows the underlying buffer slice
/// directly and returns an [`AccessorWriter`] for in-place element mutation.
///
/// Sparse accessors are not supported (returns [`AccessorViewError::SparseNotSupported`]).
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

pub fn resolve_accessor<'a, T: bytemuck::Pod>(
    model: &'a GltfModel,
    accessor_index: usize,
) -> Result<AccessorTypedView<'a, T>, AccessorViewError> {
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
    Ok(AccessorTypedView::new(
        &buf[bv.byte_offset..],
        acc.count,
        stride,
        acc.byte_offset,
    ))
}

pub fn get_position_accessor<'a>(
    model: &'a GltfModel,
    primitive: &MeshPrimitive,
) -> Result<AccessorTypedView<'a, glam::Vec3>, AccessorViewError> {
    let idx = *primitive
        .attributes
        .get("POSITION")
        .ok_or_else(|| AccessorViewError::MissingAttribute("POSITION".into()))?;
    resolve_accessor(model, idx)
}

pub fn get_normal_accessor<'a>(
    model: &'a GltfModel,
    primitive: &MeshPrimitive,
) -> Result<AccessorTypedView<'a, glam::Vec3>, AccessorViewError> {
    let idx = *primitive
        .attributes
        .get("NORMAL")
        .ok_or_else(|| AccessorViewError::MissingAttribute("NORMAL".into()))?;
    resolve_accessor(model, idx)
}

pub fn get_texcoord_accessor<'a>(
    model: &'a GltfModel,
    primitive: &MeshPrimitive,
    set: u8,
) -> Result<AccessorTypedView<'a, [f32; 2]>, AccessorViewError> {
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
pub fn get_feature_id_as_u64(
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
            ComponentType::Byte => i8::from_le_bytes([*data.get(s).ok_or_else(|| err(1))?]) as u64,
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
    let sparse = acc.sparse.as_ref().unwrap(); // caller ensures Some

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

pub fn get_instancing_translation<'a>(
    model: &'a GltfModel,
    node_index: usize,
) -> Result<AccessorTypedView<'a, glam::Vec3>, AccessorViewError> {
    let node = model
        .nodes
        .get(node_index)
        .ok_or_else(|| AccessorViewError::MissingAttribute("node not found".into()))?;
    let ext = node
        .extensions
        .get("EXT_mesh_gpu_instancing")
        .ok_or_else(|| {
            AccessorViewError::MissingAttribute("EXT_mesh_gpu_instancing not present".into())
        })?;
    let acc_idx = ext
        .get("attributes")
        .and_then(|a| a.get("TRANSLATION"))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            AccessorViewError::MissingAttribute("no TRANSLATION in EXT_mesh_gpu_instancing".into())
        })? as usize;
    resolve_accessor(model, acc_idx)
}

// ── Data-trait-based accessor view ───────────────────────────────────────────

/// Trait for types that can be read element-by-element from a glTF accessor.
///
/// Implement this for custom types. The built-in impls cover `f32`, `u32`,
/// `u16`, `u8`, `glam::Vec2`, and `glam::Vec3`.
pub trait AccessorData: Sized {
    /// glTF component type integer (e.g. `5126` for `FLOAT`).
    const COMPONENT_TYPE: i64;
    /// glTF accessor type string (e.g. `"VEC3"`).
    const TYPE: &'static str;
    /// Number of scalar components (1 for SCALAR, 3 for VEC3, …).
    const COMPONENTS: usize;

    /// Read one element from `bytes` at the given byte `offset`.
    fn read_from_bytes(bytes: &[u8], offset: usize) -> Option<Self>;

    /// Returns `true` if `component_type` and `accessor_type` match this impl.
    fn is_compatible(component_type: i64, accessor_type: &str) -> bool {
        Self::COMPONENT_TYPE == component_type && Self::TYPE == accessor_type
    }
}

impl AccessorData for f32 {
    const COMPONENT_TYPE: i64 = 5126;
    const TYPE: &'static str = "SCALAR";
    const COMPONENTS: usize = 1;
    fn read_from_bytes(bytes: &[u8], offset: usize) -> Option<Self> {
        bytes
            .get(offset..offset + 4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}
impl AccessorData for u32 {
    const COMPONENT_TYPE: i64 = 5125;
    const TYPE: &'static str = "SCALAR";
    const COMPONENTS: usize = 1;
    fn read_from_bytes(bytes: &[u8], offset: usize) -> Option<Self> {
        bytes
            .get(offset..offset + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}
impl AccessorData for u16 {
    const COMPONENT_TYPE: i64 = 5123;
    const TYPE: &'static str = "SCALAR";
    const COMPONENTS: usize = 1;
    fn read_from_bytes(bytes: &[u8], offset: usize) -> Option<Self> {
        bytes
            .get(offset..offset + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
}
impl AccessorData for u8 {
    const COMPONENT_TYPE: i64 = 5121;
    const TYPE: &'static str = "SCALAR";
    const COMPONENTS: usize = 1;
    fn read_from_bytes(bytes: &[u8], offset: usize) -> Option<Self> {
        bytes.get(offset).copied()
    }
}
impl AccessorData for glam::Vec3 {
    const COMPONENT_TYPE: i64 = 5126;
    const TYPE: &'static str = "VEC3";
    const COMPONENTS: usize = 3;
    fn read_from_bytes(bytes: &[u8], offset: usize) -> Option<Self> {
        let x = f32::read_from_bytes(bytes, offset)?;
        let y = f32::read_from_bytes(bytes, offset + 4)?;
        let z = f32::read_from_bytes(bytes, offset + 8)?;
        Some(glam::Vec3::new(x, y, z))
    }
}
impl AccessorData for glam::Vec2 {
    const COMPONENT_TYPE: i64 = 5126;
    const TYPE: &'static str = "VEC2";
    const COMPONENTS: usize = 2;
    fn read_from_bytes(bytes: &[u8], offset: usize) -> Option<Self> {
        let x = f32::read_from_bytes(bytes, offset)?;
        let y = f32::read_from_bytes(bytes, offset + 4)?;
        Some(glam::Vec2::new(x, y))
    }
}

/// Zero-copy, bounds-checked view over a typed glTF accessor.
///
/// Created via [`AccessorDataView::new`]. Iterate with [`into_iter`] or
/// index with [`get`].
pub struct AccessorDataView<'a, T: AccessorData> {
    accessor: &'a Accessor,
    buffer_data: &'a [u8],
    element_size: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T: AccessorData> AccessorDataView<'a, T> {
    /// Construct a view for `accessor_index` in `model`.
    ///
    /// Validates that the accessor exists, has a buffer view, and the
    /// component/type match `T`. Returns an error otherwise.
    pub fn new(model: &'a GltfModel, accessor_index: usize) -> Result<Self, AccessorViewError> {
        let accessor = model
            .accessors
            .get(accessor_index)
            .ok_or(AccessorViewError::AccessorNotFound(accessor_index))?;

        if accessor.sparse.is_some() {
            return Err(AccessorViewError::SparseAccessorNotSupported);
        }

        let bv_idx = accessor
            .buffer_view
            .ok_or_else(|| AccessorViewError::InvalidAccessor("no buffer_view".into()))?;
        let buffer_view = model
            .buffer_views
            .get(bv_idx)
            .ok_or(AccessorViewError::BufferViewNotFound(bv_idx))?;
        let buffer: &'a [u8] = &model
            .buffers
            .get(buffer_view.buffer)
            .ok_or(AccessorViewError::BufferNotFound(buffer_view.buffer))?
            .data;

        let accessor_type = accessor.r#type.as_str();
        let component_type = ComponentType::from(accessor.component_type).id();

        if !T::is_compatible(component_type, accessor_type) {
            return Err(AccessorViewError::IncompatibleType(format!(
                "expected {}/{}, got {}/{}",
                T::COMPONENT_TYPE,
                T::TYPE,
                component_type,
                accessor_type
            )));
        }

        let stride = buffer_view.byte_stride.unwrap_or(T::COMPONENTS * 4);
        let required = accessor.byte_offset + accessor.count * stride;
        if required > buffer.len() {
            return Err(AccessorViewError::BufferTooSmall {
                required,
                available: buffer.len(),
            });
        }

        Ok(Self {
            accessor,
            buffer_data: buffer,
            element_size: stride,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Number of elements.
    #[inline]
    pub fn count(&self) -> usize {
        self.accessor.count
    }

    /// Get element at `index` (bounds-checked).
    #[inline]
    pub fn get(&self, index: usize) -> Result<T, AccessorViewError> {
        if index >= self.count() {
            return Err(AccessorViewError::BufferTooSmall {
                required: index + 1,
                available: self.count(),
            });
        }
        let offset = self.accessor.byte_offset + index * self.element_size;
        T::read_from_bytes(self.buffer_data, offset)
            .ok_or_else(|| AccessorViewError::InvalidAccessor("read_from_bytes failed".into()))
    }

    /// Consume the view and return an iterator over all elements.
    #[inline]
    pub fn into_iter(self) -> AccessorDataIter<'a, T> {
        AccessorDataIter {
            buffer_data: self.buffer_data,
            element_size: self.element_size,
            byte_offset: self.accessor.byte_offset,
            count: self.accessor.count,
            index: 0,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Iterator produced by [`AccessorDataView::into_iter`].
pub struct AccessorDataIter<'a, T: AccessorData> {
    buffer_data: &'a [u8],
    element_size: usize,
    byte_offset: usize,
    count: usize,
    index: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T: AccessorData> Iterator for AccessorDataIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.count {
            return None;
        }
        let offset = self.byte_offset + self.index * self.element_size;
        let result = T::read_from_bytes(self.buffer_data, offset)?;
        self.index += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.index;
        (remaining, Some(remaining))
    }
}

impl<T: AccessorData> ExactSizeIterator for AccessorDataIter<'_, T> {}

// ── Accessor builder helper ───────────────────────────────────────────────────

/// Append typed data to the model's last buffer, registering a new
/// `BufferView` and `Accessor`. Creates an empty buffer if none exist.
///
/// Returns the index of the newly added accessor.
pub fn append_accessor<T: bytemuck::NoUninit>(
    model: &mut GltfModel,
    data: &[T],
    accessor_type: crate::AccessorType,
    component_type: crate::AccessorComponentType,
) -> usize {
    if model.buffers.is_empty() {
        model.buffers.push(crate::Buffer::default());
    }

    let buf_idx = model.buffers.len() - 1;
    let byte_offset = model.buffers[buf_idx].data.len();
    let raw: &[u8] = bytemuck::cast_slice(data);
    model.buffers[buf_idx].data.extend_from_slice(raw);
    model.buffers[buf_idx].byte_length = model.buffers[buf_idx].data.len();

    let bv_idx = model.buffer_views.len();
    model.buffer_views.push(crate::BufferView {
        buffer: buf_idx,
        byte_offset,
        byte_length: raw.len(),
        byte_stride: None,
        ..Default::default()
    });

    let acc_idx = model.accessors.len();
    model.accessors.push(crate::Accessor {
        buffer_view: Some(bv_idx),
        byte_offset: 0,
        component_type,
        count: data.len(),
        r#type: accessor_type,
        ..Default::default()
    });

    acc_idx
}
