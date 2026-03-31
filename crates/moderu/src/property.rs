use std::fmt;
use std::marker::PhantomData;
use std::ops::{Add, Mul};

/// The possible types of a property in a property table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PropertyType {
    #[default]
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
    String,
    Boolean,
    Enum,
}

/// The possible component types of a property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PropertyComponentType {
    #[default]
    None,
    Int8,
    Uint8,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Int64,
    Uint64,
    Float32,
    Float64,
}

impl PropertyType {
    pub fn is_vec(self) -> bool {
        matches!(self, Self::Vec2 | Self::Vec3 | Self::Vec4)
    }
    pub fn is_mat(self) -> bool {
        matches!(self, Self::Mat2 | Self::Mat3 | Self::Mat4)
    }
    pub fn dimensions(self) -> Option<u8> {
        match self {
            Self::Scalar => Some(1),
            Self::Vec2 | Self::Mat2 => Some(2),
            Self::Vec3 | Self::Mat3 => Some(3),
            Self::Vec4 | Self::Mat4 => Some(4),
            _ => None,
        }
    }
    pub fn component_count(self) -> Option<u8> {
        match self {
            Self::Scalar => Some(1),
            Self::Vec2 => Some(2),
            Self::Vec3 => Some(3),
            Self::Vec4 | Self::Mat2 => Some(4),
            Self::Mat3 => Some(9),
            Self::Mat4 => Some(16),
            _ => None,
        }
    }
}

impl fmt::Display for PropertyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Scalar => "SCALAR",
            Self::Vec2 => "VEC2",
            Self::Vec3 => "VEC3",
            Self::Vec4 => "VEC4",
            Self::Mat2 => "MAT2",
            Self::Mat3 => "MAT3",
            Self::Mat4 => "MAT4",
            Self::String => "STRING",
            Self::Boolean => "BOOLEAN",
            Self::Enum => "ENUM",
        })
    }
}

impl std::str::FromStr for PropertyType {
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
            "STRING" => Ok(Self::String),
            "BOOLEAN" => Ok(Self::Boolean),
            "ENUM" => Ok(Self::Enum),
            _ => Err(()),
        }
    }
}

impl serde::Serialize for PropertyType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for PropertyType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <&str as serde::Deserialize>::deserialize(deserializer)?;
        s.parse()
            .map_err(|_| serde::de::Error::custom(format!("unknown PropertyType: {s}")))
    }
}

impl PropertyComponentType {
    pub fn is_integer(self) -> bool {
        matches!(
            self,
            Self::Int8
                | Self::Uint8
                | Self::Int16
                | Self::Uint16
                | Self::Int32
                | Self::Uint32
                | Self::Int64
                | Self::Uint64
        )
    }
    pub fn byte_size(self) -> Option<u8> {
        match self {
            Self::Int8 | Self::Uint8 => Some(1),
            Self::Int16 | Self::Uint16 => Some(2),
            Self::Int32 | Self::Uint32 | Self::Float32 => Some(4),
            Self::Int64 | Self::Uint64 | Self::Float64 => Some(8),
            Self::None => None,
        }
    }
}

impl fmt::Display for PropertyComponentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Int8 => "INT8",
            Self::Uint8 => "UINT8",
            Self::Int16 => "INT16",
            Self::Uint16 => "UINT16",
            Self::Int32 => "INT32",
            Self::Uint32 => "UINT32",
            Self::Int64 => "INT64",
            Self::Uint64 => "UINT64",
            Self::Float32 => "FLOAT32",
            Self::Float64 => "FLOAT64",
            Self::None => "NONE",
        })
    }
}

impl std::str::FromStr for PropertyComponentType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "INT8" => Ok(Self::Int8),
            "UINT8" => Ok(Self::Uint8),
            "INT16" => Ok(Self::Int16),
            "UINT16" => Ok(Self::Uint16),
            "INT32" => Ok(Self::Int32),
            "UINT32" => Ok(Self::Uint32),
            "INT64" => Ok(Self::Int64),
            "UINT64" => Ok(Self::Uint64),
            "FLOAT32" => Ok(Self::Float32),
            "FLOAT64" => Ok(Self::Float64),
            _ => Err(()),
        }
    }
}

impl serde::Serialize for PropertyComponentType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for PropertyComponentType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <&str as serde::Deserialize>::deserialize(deserializer)?;
        s.parse()
            .map_err(|_| serde::de::Error::custom(format!("unknown PropertyComponentType: {s}")))
    }
}

/// A type that can be stored / retrieved as a structural metadata value.
pub trait PropertyElement: Sized + Copy + 'static {
    fn from_le_bytes(bytes: &[u8]) -> Option<Self>;
    fn byte_size() -> usize;
    fn property_type() -> PropertyType;
    fn property_component_type() -> PropertyComponentType;
}

macro_rules! impl_scalar {
    ($t:ty, $size:expr, $comp:expr) => {
        impl PropertyElement for $t {
            fn from_le_bytes(bytes: &[u8]) -> Option<Self> {
                let arr: [u8; $size] = bytes.get(..$size)?.try_into().ok()?;
                Some(<$t>::from_le_bytes(arr))
            }
            fn byte_size() -> usize {
                $size
            }
            fn property_type() -> PropertyType {
                PropertyType::Scalar
            }
            fn property_component_type() -> PropertyComponentType {
                $comp
            }
        }
    };
}
impl_scalar!(u8, 1, PropertyComponentType::Uint8);
impl_scalar!(i8, 1, PropertyComponentType::Int8);
impl_scalar!(u16, 2, PropertyComponentType::Uint16);
impl_scalar!(i16, 2, PropertyComponentType::Int16);
impl_scalar!(u32, 4, PropertyComponentType::Uint32);
impl_scalar!(i32, 4, PropertyComponentType::Int32);
impl_scalar!(u64, 8, PropertyComponentType::Uint64);
impl_scalar!(i64, 8, PropertyComponentType::Int64);
impl_scalar!(f32, 4, PropertyComponentType::Float32);
impl_scalar!(f64, 8, PropertyComponentType::Float64);

impl PropertyElement for [f32; 2] {
    fn from_le_bytes(b: &[u8]) -> Option<Self> {
        Some([
            f32::from_le_bytes(b.get(0..4)?.try_into().ok()?),
            f32::from_le_bytes(b.get(4..8)?.try_into().ok()?),
        ])
    }
    fn byte_size() -> usize {
        8
    }
    fn property_type() -> PropertyType {
        PropertyType::Vec2
    }
    fn property_component_type() -> PropertyComponentType {
        PropertyComponentType::Float32
    }
}
impl PropertyElement for [f32; 3] {
    fn from_le_bytes(b: &[u8]) -> Option<Self> {
        Some([
            f32::from_le_bytes(b.get(0..4)?.try_into().ok()?),
            f32::from_le_bytes(b.get(4..8)?.try_into().ok()?),
            f32::from_le_bytes(b.get(8..12)?.try_into().ok()?),
        ])
    }
    fn byte_size() -> usize {
        12
    }
    fn property_type() -> PropertyType {
        PropertyType::Vec3
    }
    fn property_component_type() -> PropertyComponentType {
        PropertyComponentType::Float32
    }
}
impl PropertyElement for [f32; 4] {
    fn from_le_bytes(b: &[u8]) -> Option<Self> {
        Some([
            f32::from_le_bytes(b.get(0..4)?.try_into().ok()?),
            f32::from_le_bytes(b.get(4..8)?.try_into().ok()?),
            f32::from_le_bytes(b.get(8..12)?.try_into().ok()?),
            f32::from_le_bytes(b.get(12..16)?.try_into().ok()?),
        ])
    }
    fn byte_size() -> usize {
        16
    }
    fn property_type() -> PropertyType {
        PropertyType::Vec4
    }
    fn property_component_type() -> PropertyComponentType {
        PropertyComponentType::Float32
    }
}

/// 2×2 float matrix (column-major f32 × 4).
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PropertyMat2(pub [f32; 4]);
impl From<[f32; 4]> for PropertyMat2 {
    fn from(v: [f32; 4]) -> Self {
        Self(v)
    }
}
impl From<PropertyMat2> for [f32; 4] {
    fn from(m: PropertyMat2) -> Self {
        m.0
    }
}
impl AsRef<[f32]> for PropertyMat2 {
    fn as_ref(&self) -> &[f32] {
        &self.0
    }
}
impl std::ops::Deref for PropertyMat2 {
    type Target = [f32; 4];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl PropertyElement for PropertyMat2 {
    fn from_le_bytes(b: &[u8]) -> Option<Self> {
        Some(PropertyMat2(<[f32; 4]>::from_le_bytes(b)?))
    }
    fn byte_size() -> usize {
        16
    }
    fn property_type() -> PropertyType {
        PropertyType::Mat2
    }
    fn property_component_type() -> PropertyComponentType {
        PropertyComponentType::Float32
    }
}

/// 3×3 float matrix (column-major f32 × 9).
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PropertyMat3(pub [f32; 9]);
impl From<[f32; 9]> for PropertyMat3 {
    fn from(v: [f32; 9]) -> Self {
        Self(v)
    }
}
impl From<PropertyMat3> for [f32; 9] {
    fn from(m: PropertyMat3) -> Self {
        m.0
    }
}
impl AsRef<[f32]> for PropertyMat3 {
    fn as_ref(&self) -> &[f32] {
        &self.0
    }
}
impl std::ops::Deref for PropertyMat3 {
    type Target = [f32; 9];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl PropertyElement for PropertyMat3 {
    fn from_le_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < 36 {
            return None;
        }
        let mut arr = [0f32; 9];
        for (i, v) in arr.iter_mut().enumerate() {
            *v = f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().ok()?);
        }
        Some(PropertyMat3(arr))
    }
    fn byte_size() -> usize {
        36
    }
    fn property_type() -> PropertyType {
        PropertyType::Mat3
    }
    fn property_component_type() -> PropertyComponentType {
        PropertyComponentType::Float32
    }
}

/// 4×4 float matrix (column-major f32 × 16).
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PropertyMat4(pub [f32; 16]);
impl From<[f32; 16]> for PropertyMat4 {
    fn from(v: [f32; 16]) -> Self {
        Self(v)
    }
}
impl From<PropertyMat4> for [f32; 16] {
    fn from(m: PropertyMat4) -> Self {
        m.0
    }
}
impl AsRef<[f32]> for PropertyMat4 {
    fn as_ref(&self) -> &[f32] {
        &self.0
    }
}
impl std::ops::Deref for PropertyMat4 {
    type Target = [f32; 16];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl PropertyElement for PropertyMat4 {
    fn from_le_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < 64 {
            return None;
        }
        let mut arr = [0f32; 16];
        for (i, v) in arr.iter_mut().enumerate() {
            *v = f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().ok()?);
        }
        Some(PropertyMat4(arr))
    }
    fn byte_size() -> usize {
        64
    }
    fn property_type() -> PropertyType {
        PropertyType::Mat4
    }
    fn property_component_type() -> PropertyComponentType {
        PropertyComponentType::Float32
    }
}

#[inline]
pub fn normalize_i8(v: i8) -> f64 {
    (v as f64 / i8::MAX as f64).max(-1.0)
}
#[inline]
pub fn normalize_i16(v: i16) -> f64 {
    (v as f64 / i16::MAX as f64).max(-1.0)
}
#[inline]
pub fn normalize_i32(v: i32) -> f64 {
    (v as f64 / i32::MAX as f64).max(-1.0)
}
#[inline]
pub fn normalize_i64(v: i64) -> f64 {
    (v as f64 / i64::MAX as f64).max(-1.0)
}
#[inline]
pub fn normalize_u8(v: u8) -> f64 {
    v as f64 / u8::MAX as f64
}
#[inline]
pub fn normalize_u16(v: u16) -> f64 {
    v as f64 / u16::MAX as f64
}
#[inline]
pub fn normalize_u32(v: u32) -> f64 {
    v as f64 / u32::MAX as f64
}
#[inline]
pub fn normalize_u64(v: u64) -> f64 {
    v as f64 / u64::MAX as f64
}

#[inline]
pub fn apply_scale<T: Mul<Output = T> + Copy>(value: T, scale: T) -> T {
    value * scale
}

pub fn transform_value<T: Add<Output = T> + Mul<Output = T> + Copy>(
    value: T,
    offset: Option<T>,
    scale: Option<T>,
) -> T {
    let scaled = scale.map_or(value, |s| value * s);
    offset.map_or(scaled, |o| scaled + o)
}

pub trait IntoF64: Copy {
    fn into_f64(self) -> f64;
}
macro_rules! impl_into_f64 {
    ($($t:ty),*) => { $(impl IntoF64 for $t { fn into_f64(self) -> f64 { self as f64 } })* };
}
impl_into_f64!(f32, f64, u8, i8, u16, i16, u32, i32, u64, i64);

pub trait MetadataConvert<To>: Sized {
    fn try_convert(self) -> Option<To>;
}
impl<T: Copy> MetadataConvert<T> for T {
    fn try_convert(self) -> Option<T> {
        Some(self)
    }
}

macro_rules! impl_lossless {
    ($from:ty => $to:ty) => {
        impl MetadataConvert<$to> for $from {
            fn try_convert(self) -> Option<$to> {
                Some(self as $to)
            }
        }
    };
}
impl_lossless!(u8 => u16);
impl_lossless!(u8 => u32);
impl_lossless!(u8 => u64);
impl_lossless!(u8 => i16);
impl_lossless!(u8 => i32);
impl_lossless!(u8 => i64);
impl_lossless!(u8 => f32);
impl_lossless!(u8 => f64);
impl_lossless!(i8 => i16);
impl_lossless!(i8 => i32);
impl_lossless!(i8 => i64);
impl_lossless!(i8 => f32);
impl_lossless!(i8 => f64);
impl_lossless!(u16 => u32);
impl_lossless!(u16 => u64);
impl_lossless!(u16 => i32);
impl_lossless!(u16 => i64);
impl_lossless!(u16 => f32);
impl_lossless!(u16 => f64);
impl_lossless!(i16 => i32);
impl_lossless!(i16 => i64);
impl_lossless!(i16 => f32);
impl_lossless!(i16 => f64);
impl_lossless!(u32 => u64);
impl_lossless!(u32 => i64);
impl_lossless!(u32 => f64);
impl_lossless!(i32 => i64);
impl_lossless!(i32 => f64);
impl_lossless!(f32 => f64);

macro_rules! impl_checked_tryfrom {
    ($from:ty => $to:ty) => {
        impl MetadataConvert<$to> for $from {
            fn try_convert(self) -> Option<$to> {
                <$to>::try_from(self).ok()
            }
        }
    };
}
impl_checked_tryfrom!(u64 => u32);
impl_checked_tryfrom!(u64 => u16);
impl_checked_tryfrom!(u64 => u8);
impl_checked_tryfrom!(u32 => u16);
impl_checked_tryfrom!(u32 => u8);
impl_checked_tryfrom!(u16 => u8);
impl_checked_tryfrom!(i64 => i32);
impl_checked_tryfrom!(i64 => i16);
impl_checked_tryfrom!(i64 => i8);
impl_checked_tryfrom!(i32 => i16);
impl_checked_tryfrom!(i32 => i8);
impl_checked_tryfrom!(i16 => i8);

macro_rules! impl_f2i {
    ($from:ty => $to:ty) => {
        impl MetadataConvert<$to> for $from {
            fn try_convert(self) -> Option<$to> {
                if self.is_nan() || self.is_infinite() {
                    return None;
                }
                let t = self.trunc() as i128;
                if t < <$to>::MIN as i128 || t > <$to>::MAX as i128 {
                    return None;
                }
                Some(t as $to)
            }
        }
    };
}
impl_f2i!(f32 => u8);
impl_f2i!(f32 => i8);
impl_f2i!(f32 => u16);
impl_f2i!(f32 => i16);
impl_f2i!(f32 => u32);
impl_f2i!(f32 => i32);
impl_f2i!(f32 => u64);
impl_f2i!(f32 => i64);
impl_f2i!(f64 => u8);
impl_f2i!(f64 => i8);
impl_f2i!(f64 => u16);
impl_f2i!(f64 => i16);
impl_f2i!(f64 => u32);
impl_f2i!(f64 => i32);
impl_f2i!(f64 => u64);
impl_f2i!(f64 => i64);

impl MetadataConvert<bool> for f32 {
    fn try_convert(self) -> Option<bool> {
        Some(self != 0.0)
    }
}
impl MetadataConvert<bool> for f64 {
    fn try_convert(self) -> Option<bool> {
        Some(self != 0.0)
    }
}

macro_rules! impl_int2bool {
    ($t:ty) => {
        impl MetadataConvert<bool> for $t {
            fn try_convert(self) -> Option<bool> {
                Some(self != 0)
            }
        }
    };
}
impl_int2bool!(u8);
impl_int2bool!(i8);
impl_int2bool!(u16);
impl_int2bool!(i16);
impl_int2bool!(u32);
impl_int2bool!(i32);
impl_int2bool!(u64);
impl_int2bool!(i64);

impl MetadataConvert<bool> for &str {
    fn try_convert(self) -> Option<bool> {
        match self.to_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        }
    }
}
impl MetadataConvert<bool> for String {
    fn try_convert(self) -> Option<bool> {
        self.as_str().try_convert()
    }
}

macro_rules! impl_str2num {
    ($t:ty) => {
        impl MetadataConvert<$t> for &str {
            fn try_convert(self) -> Option<$t> {
                self.parse().ok()
            }
        }
        impl MetadataConvert<$t> for String {
            fn try_convert(self) -> Option<$t> {
                self.parse().ok()
            }
        }
    };
}
impl_str2num!(u8);
impl_str2num!(i8);
impl_str2num!(u16);
impl_str2num!(i16);
impl_str2num!(u32);
impl_str2num!(i32);
impl_str2num!(u64);
impl_str2num!(i64);
impl_str2num!(f32);
impl_str2num!(f64);

/// Zero-copy typed view over a byte-slice with fixed element size.
pub struct PropertyArrayView<'a, T: PropertyElement> {
    data: &'a [u8],
    count: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: PropertyElement> PropertyArrayView<'a, T> {
    pub fn from_bytes(data: &'a [u8], count: usize) -> Self {
        Self {
            data,
            count,
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
        T::from_le_bytes(&self.data[index * T::byte_size()..])
    }
    pub fn iter(&self) -> PropertyArrayIter<'a, T> {
        PropertyArrayIter {
            data: self.data,
            count: self.count,
            index: 0,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: PropertyElement> IntoIterator for &'_ PropertyArrayView<'a, T> {
    type Item = T;
    type IntoIter = PropertyArrayIter<'a, T>;
    fn into_iter(self) -> PropertyArrayIter<'a, T> {
        self.iter()
    }
}

/// Iterator for [`PropertyArrayView`]. Stores data directly — no borrow of the view needed.
pub struct PropertyArrayIter<'a, T: PropertyElement> {
    data: &'a [u8],
    count: usize,
    index: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: PropertyElement> Iterator for PropertyArrayIter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.index >= self.count {
            return None;
        }
        let v = T::from_le_bytes(&self.data[self.index * T::byte_size()..])?;
        self.index += 1;
        Some(v)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let r = self.count.saturating_sub(self.index);
        (r, Some(r))
    }
}
impl<'a, T: PropertyElement> ExactSizeIterator for PropertyArrayIter<'a, T> {}

/// Owned copy of an array property value.
pub struct PropertyArrayCopy<T: PropertyElement> {
    storage: Vec<u8>,
    count: usize,
    _marker: PhantomData<T>,
}

impl<T: PropertyElement> PropertyArrayCopy<T> {
    pub fn from_values(values: &[T]) -> Self
    where
        T: bytemuck::Pod,
    {
        let storage = bytemuck::cast_slice::<T, u8>(values).to_vec();
        Self {
            storage,
            count: values.len(),
            _marker: PhantomData,
        }
    }
    pub fn as_view(&self) -> PropertyArrayView<'_, T> {
        PropertyArrayView::from_bytes(&self.storage, self.count)
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub fn get(&self, index: usize) -> Option<T> {
        self.as_view().get(index)
    }
}

/// All possible states when constructing a property view — success, empty-with-default,
/// and every error condition across table, attribute, and texture views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyViewStatus {
    Valid,
    /// Property has no buffer data but the schema provides a default value.
    EmptyPropertyWithDefault,
    NonexistentProperty,
    TypeMismatch,
    ComponentTypeMismatch,
    ArrayTypeMismatch,
    InvalidNormalization,
    NormalizationMismatch,
    InvalidOffset,
    InvalidScale,
    InvalidMax,
    InvalidMin,
    InvalidNoDataValue,
    InvalidDefaultValue,
    InvalidPropertyTable,
    InvalidValueBufferView,
    InvalidArrayOffsetBufferView,
    InvalidStringOffsetBufferView,
    InvalidValueBuffer,
    InvalidArrayOffsetBuffer,
    InvalidStringOffsetBuffer,
    BufferViewOutOfBounds,
    BufferViewSizeNotDivisibleByTypeSize,
    BufferViewSizeDoesNotMatchPropertyTableCount,
    ArrayCountAndOffsetBufferCoexist,
    ArrayCountAndOffsetBufferDontExist,
    InvalidArrayOffsetType,
    InvalidStringOffsetType,
    ArrayOffsetsNotSorted,
    StringOffsetsNotSorted,
    ArrayOffsetOutOfBounds,
    StringOffsetOutOfBounds,
    InvalidPropertyAttribute,
    InvalidAccessor,
    InvalidPropertyTexture,
    InvalidTexture,
    InvalidImageIndex,
    InvalidSamplerIndex,
    EmptyImage,
    InvalidBytesPerChannel,
    InvalidChannelCount,
}

impl PropertyViewStatus {
    /// Returns `true` for `Valid` and `EmptyPropertyWithDefault`.
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid | Self::EmptyPropertyWithDefault)
    }
    /// Returns `true` only for `Valid` (data is present and correct).
    pub fn is_ready(&self) -> bool {
        *self == Self::Valid
    }
}

impl fmt::Display for PropertyViewStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::Valid => "valid",
            Self::EmptyPropertyWithDefault => "empty property with default value",
            Self::NonexistentProperty => "property does not exist in the schema",
            Self::TypeMismatch => "property type mismatch",
            Self::ComponentTypeMismatch => "property component type mismatch",
            Self::ArrayTypeMismatch => "property array type mismatch",
            Self::InvalidNormalization => "invalid normalization",
            Self::NormalizationMismatch => "normalization mismatch",
            Self::InvalidOffset => "invalid offset",
            Self::InvalidScale => "invalid scale",
            Self::InvalidMax => "invalid max",
            Self::InvalidMin => "invalid min",
            Self::InvalidNoDataValue => "invalid noData value",
            Self::InvalidDefaultValue => "invalid default value",
            Self::InvalidPropertyTable => "invalid property table",
            Self::InvalidValueBufferView => "invalid value buffer view",
            Self::InvalidArrayOffsetBufferView => "invalid array-offset buffer view",
            Self::InvalidStringOffsetBufferView => "invalid string-offset buffer view",
            Self::InvalidValueBuffer => "invalid value buffer",
            Self::InvalidArrayOffsetBuffer => "invalid array-offset buffer",
            Self::InvalidStringOffsetBuffer => "invalid string-offset buffer",
            Self::BufferViewOutOfBounds => "buffer view out of bounds",
            Self::BufferViewSizeNotDivisibleByTypeSize => {
                "buffer view byte length is not divisible by the element type size"
            }
            Self::BufferViewSizeDoesNotMatchPropertyTableCount => {
                "buffer view size does not match property table count"
            }
            Self::ArrayCountAndOffsetBufferCoexist => {
                "array count and offset buffer cannot both be present"
            }
            Self::ArrayCountAndOffsetBufferDontExist => {
                "variable-length array requires either a fixed count or an offset buffer"
            }
            Self::InvalidArrayOffsetType => "invalid array offset type",
            Self::InvalidStringOffsetType => "invalid string offset type",
            Self::ArrayOffsetsNotSorted => "array offsets are not sorted",
            Self::StringOffsetsNotSorted => "string offsets are not sorted",
            Self::ArrayOffsetOutOfBounds => "array offset out of bounds",
            Self::StringOffsetOutOfBounds => "string offset out of bounds",
            Self::InvalidPropertyAttribute => "invalid property attribute",
            Self::InvalidAccessor => "invalid accessor",
            Self::InvalidPropertyTexture => "invalid property texture",
            Self::InvalidTexture => "invalid texture",
            Self::InvalidImageIndex => "invalid image index",
            Self::InvalidSamplerIndex => "invalid sampler index",
            Self::EmptyImage => "image is empty",
            Self::InvalidBytesPerChannel => "invalid bytes per channel",
            Self::InvalidChannelCount => "invalid channel count",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for PropertyViewStatus {}

#[derive(Debug, Clone, PartialEq)]
pub enum MetadataValue {
    Boolean(bool),
    Int64(i64),
    Uint64(u64),
    Float32(f32),
    Float64(f64),
    String(String),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Mat2([f32; 4]),
    Mat3([f32; 9]),
    Mat4([f32; 16]),
    Array(Vec<MetadataValue>),
    NoData,
}

impl MetadataValue {
    pub fn as_bool(&self) -> Option<bool> {
        if let MetadataValue::Boolean(b) = self {
            Some(*b)
        } else {
            None
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            MetadataValue::Int64(v) => Some(*v),
            MetadataValue::Uint64(v) => i64::try_from(*v).ok(),
            _ => None,
        }
    }
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            MetadataValue::Uint64(v) => Some(*v),
            MetadataValue::Int64(v) if *v >= 0 => Some(*v as u64),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            MetadataValue::Float64(v) => Some(*v),
            MetadataValue::Float32(v) => Some(*v as f64),
            MetadataValue::Int64(v) => Some(*v as f64),
            MetadataValue::Uint64(v) => Some(*v as f64),
            _ => None,
        }
    }
    pub fn as_f32(&self) -> Option<f32> {
        self.as_f64().map(|v| v as f32)
    }
    pub fn as_str(&self) -> Option<&str> {
        if let MetadataValue::String(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_vec3(&self) -> Option<[f32; 3]> {
        if let MetadataValue::Vec3(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    pub fn as_array(&self) -> Option<&[MetadataValue]> {
        if let MetadataValue::Array(a) = self {
            Some(a)
        } else {
            None
        }
    }
}

// ── Variable-length array support ─────────────────────────────────────────────

/// Read a raw `u32` / `u64` element from an offset buffer (UINT32 or UINT64).
fn read_offset(buf: &[u8], index: usize, is_u64: bool) -> Option<usize> {
    if is_u64 {
        let s = index * 8;
        Some(u64::from_le_bytes(buf.get(s..s + 8)?.try_into().ok()?) as usize)
    } else {
        let s = index * 4;
        Some(u32::from_le_bytes(buf.get(s..s + 4)?.try_into().ok()?) as usize)
    }
}

/// A zero-copy view over a variable-length array property column.
///
/// Each row may have a different number of elements. Use [`len_of`] to get
/// the count for a given row, and [`get_row`] to obtain a [`PropertyArrayView`]
/// for that row.
///
/// Mirrors `PropertyArrayView` (variable form) in Cesium's `PropertyTablePropertyView`.
pub struct VariablePropertyArrayView<'a, T: PropertyElement> {
    /// Tightly-packed element values (all rows concatenated).
    values: &'a [u8],
    /// Byte offsets into `values` for each row boundary (length == row_count + 1).
    offsets: &'a [u8],
    row_count: usize,
    /// If `true`, offsets are UINT64; otherwise UINT32.
    offsets_u64: bool,
    _marker: PhantomData<T>,
}

impl<'a, T: PropertyElement> VariablePropertyArrayView<'a, T> {
    /// Construct from raw byte slices.
    ///
    /// * `values`      — the raw element buffer (tightly packed)
    /// * `offsets`     — the array-offset buffer (count+1 entries of UINT32 or UINT64)
    /// * `row_count`   — number of rows in the property table
    /// * `offsets_u64` — `true` if the offset type is UINT64 (default: UINT32)
    pub fn new(values: &'a [u8], offsets: &'a [u8], row_count: usize, offsets_u64: bool) -> Self {
        Self {
            values,
            offsets,
            row_count,
            offsets_u64,
            _marker: PhantomData,
        }
    }

    /// Number of rows.
    pub fn len(&self) -> usize {
        self.row_count
    }

    pub fn is_empty(&self) -> bool {
        self.row_count == 0
    }

    /// Number of elements in the array at `row`.
    pub fn len_of(&self, row: usize) -> Option<usize> {
        if row >= self.row_count {
            return None;
        }
        let start = read_offset(self.offsets, row, self.offsets_u64)?;
        let end = read_offset(self.offsets, row + 1, self.offsets_u64)?;
        let elem = T::byte_size();
        if elem == 0 {
            return Some(0);
        }
        Some((end - start) / elem)
    }

    /// Zero-copy view over the elements in row `row`.
    pub fn get_row(&self, row: usize) -> Option<PropertyArrayView<'a, T>> {
        if row >= self.row_count {
            return None;
        }
        let start = read_offset(self.offsets, row, self.offsets_u64)?;
        let end = read_offset(self.offsets, row + 1, self.offsets_u64)?;
        let elem = T::byte_size();
        let count = if elem > 0 { (end - start) / elem } else { 0 };
        let slice = self.values.get(start..end)?;
        Some(PropertyArrayView::from_bytes(slice, count))
    }
}

// ── Normalized read + offset/scale transform helpers ─────────────────────────

/// Trait for applying offset and scale transforms to property values.
///
/// Mirrors Cesium's `PropertyTransformations` template functions.
pub trait TransformProperty: Sized {
    /// Apply `offset` then `scale` if present: `result = value * scale + offset`.
    fn transform(self, offset: Option<Self>, scale: Option<Self>) -> Self;
}

macro_rules! impl_transform_numeric {
    ($($t:ty),*) => {
        $(impl TransformProperty for $t {
            #[inline]
            fn transform(self, offset: Option<$t>, scale: Option<$t>) -> $t {
                let scaled = scale.map_or(self, |s| self * s);
                offset.map_or(scaled, |o| scaled + o)
            }
        })*
    };
}
impl_transform_numeric!(f32, f64, u8, i8, u16, i16, u32, i32, u64, i64);

macro_rules! impl_transform_array {
    ($n:literal) => {
        impl TransformProperty for [f32; $n] {
            #[inline]
            fn transform(mut self, offset: Option<[f32; $n]>, scale: Option<[f32; $n]>) -> Self {
                if let Some(s) = scale {
                    for (v, sv) in self.iter_mut().zip(s.iter()) {
                        *v *= sv;
                    }
                }
                if let Some(o) = offset {
                    for (v, ov) in self.iter_mut().zip(o.iter()) {
                        *v += ov;
                    }
                }
                self
            }
        }
    };
}
impl_transform_array!(2);
impl_transform_array!(3);
impl_transform_array!(4);
impl_transform_array!(9);
impl_transform_array!(16);
