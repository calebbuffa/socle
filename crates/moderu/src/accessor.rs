use crate::{Accessor, Model};

/// Accessor element type as defined by the glTF specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessorType {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
}

impl AccessorType {
    /// The glTF string for this type (e.g. `"VEC3"`).
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

    /// Number of scalar components in this type
    /// (e.g. `Vec3` -> 3, `Mat4` -> 16).
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

    /// Converts a glTF integer ID to a `ComponentType`.
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

    /// Byte size of a single component (e.g. `Short` -> 2, `Float` -> 4).
    pub fn byte_size(self) -> u8 {
        match self {
            Self::Byte | Self::UnsignedByte => 1,
            Self::Short | Self::UnsignedShort => 2,
            Self::Int | Self::UnsignedInt | Self::Float => 4,
            Self::Int64 | Self::UnsignedInt64 | Self::Double => 8,
        }
    }
}

impl Accessor {
    /// Parses this accessor's `type` field into an `AccessorType`.
    pub fn accessor_type(&self) -> Option<AccessorType> {
        self.r#type.as_str()?.parse().ok()
    }

    /// Parses this accessor's `componentType` field into a `ComponentType`.
    pub fn component_type(&self) -> Option<ComponentType> {
        ComponentType::from_id(self.component_type.as_i64()?)
    }

    /// Number of scalar components for this accessor's type.
    pub fn num_components(&self) -> Option<u8> {
        Some(self.accessor_type()?.num_components())
    }

    /// Byte size of a single component in this accessor.
    pub fn component_byte_size(&self) -> Option<u8> {
        Some(self.component_type()?.byte_size())
    }

    /// Bytes per vertex element (components x component size).
    pub fn bytes_per_vertex(&self) -> Option<u64> {
        let nc = self.num_components()? as u64;
        let cs = self.component_byte_size()? as u64;
        Some(nc * cs)
    }

    /// Byte stride for this accessor, falling back to tight packing
    /// when the buffer view does not specify an explicit stride.
    pub fn byte_stride(&self, model: &Model) -> Option<u64> {
        let bv_idx = self.buffer_view? as usize;
        let bv = model.buffer_views.get(bv_idx)?;
        if let Some(stride) = bv.byte_stride {
            if stride > 0 {
                return Some(stride as u64);
            }
        }
        self.bytes_per_vertex()
    }
}
