use std::fmt;

/// The possible types of a property in a property table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyType {
    Invalid,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyComponentType {
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
    /// Returns `true` if this is a vector type (Vec2, Vec3, or Vec4).
    pub fn is_vec(self) -> bool {
        matches!(self, Self::Vec2 | Self::Vec3 | Self::Vec4)
    }

    /// Returns `true` if this is a matrix type (Mat2, Mat3, or Mat4).
    pub fn is_mat(self) -> bool {
        matches!(self, Self::Mat2 | Self::Mat3 | Self::Mat4)
    }

    /// Returns the number of dimensions (e.g. Vec4 and Mat4 both return 4).
    pub fn dimensions(self) -> Option<u8> {
        match self {
            Self::Scalar => Some(1),
            Self::Vec2 | Self::Mat2 => Some(2),
            Self::Vec3 | Self::Mat3 => Some(3),
            Self::Vec4 | Self::Mat4 => Some(4),
            _ => None,
        }
    }

    /// Returns the total number of scalar components
    /// (e.g. Vec3 → 3, Mat3 → 9, Mat4 → 16).
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
            Self::Invalid => "INVALID",
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

impl PropertyComponentType {
    /// Returns `true` if this is an integer component type.
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

    /// Returns the byte size of this component type.
    pub fn byte_size(self) -> Option<u8> {
        match self {
            Self::Int8 | Self::Uint8 => Some(1),
            Self::Int16 | Self::Uint16 => Some(2),
            Self::Int32 | Self::Uint32 | Self::Float32 => Some(4),
            Self::Int64 | Self::Uint64 | Self::Float64 => Some(8),
            Self::None => None,
        }
    }

    /// Converts a glTF accessor `componentType` integer ID to a
    /// `PropertyComponentType`.
    pub fn from_accessor_component_type(ct: i64) -> Self {
        match ct {
            5120 => Self::Int8,
            5121 => Self::Uint8,
            5122 => Self::Int16,
            5123 => Self::Uint16,
            5124 => Self::Int32,
            5125 => Self::Uint32,
            5126 => Self::Float32,
            5134 => Self::Int64,
            5135 => Self::Uint64,
            5130 => Self::Float64,
            _ => Self::None,
        }
    }

    /// Converts this component type back to a glTF accessor `componentType`
    /// integer ID, or `None` if not representable.
    pub fn to_accessor_component_type(self) -> Option<i64> {
        match self {
            Self::Int8 => Some(5120),
            Self::Uint8 => Some(5121),
            Self::Int16 => Some(5122),
            Self::Uint16 => Some(5123),
            Self::Int32 => Some(5124),
            Self::Uint32 => Some(5125),
            Self::Float32 => Some(5126),
            Self::Int64 => Some(5134),
            Self::Uint64 => Some(5135),
            Self::Float64 => Some(5130),
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
