//! EXT_structural_metadata support — schema types, typed property views.

// use crate::property::{PropertyComponentType, PropertyElement, PropertyType, PropertyViewStatus};

use crate::{
    GltfModel, MetadataValue, PropertyComponentType, PropertyElement, PropertyType,
    PropertyViewStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Range;

/// How feature IDs are associated with a primitive.
#[derive(Debug, Clone)]
pub enum FeatureId {
    /// Feature IDs come from a texture at the given index.
    Texture(usize),
    /// Feature IDs come from a vertex attribute with the given name.
    Attribute(String),
}

impl FeatureId {
    pub fn from_model(model: &GltfModel, _mesh_index: usize) -> Vec<FeatureId> {
        let mut out = Vec::new();
        if let Some(Value::Object(ext)) = model.extensions.get("EXT_structural_metadata") {
            if let Some(Value::Array(arr)) = ext.get("featureIds") {
                for item in arr {
                    if let Value::Object(m) = item {
                        if let Some(Value::Number(n)) = m.get("featureIdTexture") {
                            if let Some(i) = n.as_u64() {
                                out.push(FeatureId::Texture(i as usize));
                            }
                        } else if let Some(Value::String(s)) = m.get("featureIdAttribute") {
                            out.push(FeatureId::Attribute(s.clone()));
                        }
                    }
                }
            }
        }
        out
    }
}

pub const EXT_STRUCTURAL_METADATA: &str = "EXT_structural_metadata";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassProperty {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub r#type: PropertyType,
    #[serde(rename = "componentType", skip_serializing_if = "Option::is_none")]
    pub component_type: Option<PropertyComponentType>,
    #[serde(rename = "enumType", skip_serializing_if = "Option::is_none")]
    pub enum_type: Option<String>,
    #[serde(default)]
    pub array: bool,
    #[serde(rename = "count", skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(default)]
    pub normalized: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<Value>,
    #[serde(rename = "noData", skip_serializing_if = "Option::is_none")]
    pub no_data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Class {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, ClassProperty>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnumValue {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub value: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemaEnum {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "valueType", default = "default_enum_value_type")]
    pub value_type: PropertyComponentType,
    #[serde(default)]
    pub values: Vec<EnumValue>,
}
fn default_enum_value_type() -> PropertyComponentType {
    PropertyComponentType::Uint16
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Schema {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub classes: HashMap<String, Class>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub enums: HashMap<String, SchemaEnum>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertyTableProperty {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<usize>,
    #[serde(rename = "arrayOffsets", skip_serializing_if = "Option::is_none")]
    pub array_offsets: Option<usize>,
    #[serde(rename = "stringOffsets", skip_serializing_if = "Option::is_none")]
    pub string_offsets: Option<usize>,
    #[serde(rename = "arrayOffsetType", default = "default_offset_type")]
    pub array_offset_type: PropertyComponentType,
    #[serde(rename = "stringOffsetType", default = "default_offset_type")]
    pub string_offset_type: PropertyComponentType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<Value>,
}
fn default_offset_type() -> PropertyComponentType {
    PropertyComponentType::Uint32
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertyTable {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub class: String,
    pub count: i64,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, PropertyTableProperty>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertyAttributeProperty {
    pub attribute: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertyAttribute {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub class: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, PropertyAttributeProperty>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertyTextureProperty {
    pub index: usize,
    #[serde(rename = "texCoord", default)]
    pub tex_coord: usize,
    pub channels: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertyTexture {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub class: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, PropertyTextureProperty>,
}

/// Root EXT_structural_metadata extension object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtStructuralMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Schema>,
    #[serde(rename = "schemaUri", skip_serializing_if = "Option::is_none")]
    pub schema_uri: Option<String>,
    #[serde(
        rename = "propertyTables",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub property_tables: Vec<PropertyTable>,
    #[serde(
        rename = "propertyTextures",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub property_textures: Vec<PropertyTexture>,
    #[serde(
        rename = "propertyAttributes",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub property_attributes: Vec<PropertyAttribute>,
}

impl ExtStructuralMetadata {
    pub fn from_model(model: &GltfModel) -> Option<Self> {
        let val = model.extensions.get(EXT_STRUCTURAL_METADATA)?;
        serde_json::from_value(val.clone()).ok()
    }
}

/// Typed view over a column in a `PropertyTable`.
///
/// Constructed via [`PropertyTablePropertyView::new`] which returns
/// `Result<Self, PropertyViewStatus>`.
pub struct PropertyTablePropertyView<'a, T: PropertyElement> {
    data: &'a [u8],
    count: usize,
    element_stride: usize,
    has_data: bool,
    _marker: PhantomData<T>,
}

impl<'a, T: PropertyElement> PropertyTablePropertyView<'a, T> {
    pub fn new(
        model: &'a GltfModel,
        property_table: &PropertyTable,
        property_id: &str,
        class_property: Option<ClassProperty>,
    ) -> Result<Self, PropertyViewStatus> {
        let cp = class_property.ok_or(PropertyViewStatus::NonexistentProperty)?;
        let prop = match property_table.properties.get(property_id) {
            Some(p) => p,
            None => {
                if cp.default.is_some() {
                    return Ok(Self {
                        data: &[],
                        count: property_table.count as usize,
                        element_stride: 0,
                        has_data: false,
                        _marker: PhantomData,
                    });
                }
                return Err(PropertyViewStatus::NonexistentProperty);
            }
        };
        let bv_idx = prop
            .values
            .ok_or(PropertyViewStatus::InvalidValueBufferView)?;
        let bv = model
            .buffer_views
            .get(bv_idx)
            .ok_or(PropertyViewStatus::InvalidValueBufferView)?;
        let buf: &[u8] = &model
            .buffers
            .get(bv.buffer)
            .ok_or(PropertyViewStatus::InvalidValueBuffer)?
            .data;
        let end = bv.byte_offset + bv.byte_length;
        if end > buf.len() {
            return Err(PropertyViewStatus::BufferViewOutOfBounds);
        }
        let count = property_table.count as usize;
        let elem = T::byte_size();
        if bv.byte_length % elem != 0 {
            return Err(PropertyViewStatus::BufferViewSizeNotDivisibleByTypeSize);
        }
        if bv.byte_length / elem < count {
            return Err(PropertyViewStatus::BufferViewSizeDoesNotMatchPropertyTableCount);
        }
        Ok(Self {
            data: &buf[bv.byte_offset..end],
            count,
            element_stride: elem,
            has_data: true,
            _marker: PhantomData,
        })
    }

    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// `false` when the property has no buffer data (schema provides a default value only).
    pub fn has_data(&self) -> bool {
        self.has_data
    }

    pub fn get_raw(&self, row: usize) -> Option<T> {
        if !self.has_data || row >= self.count {
            return None;
        }
        T::from_le_bytes(&self.data[row * self.element_stride..])
    }

    pub fn row_byte_range(&self, row: usize) -> Option<Range<usize>> {
        if !self.has_data || row >= self.count {
            return None;
        }
        let s = row * self.element_stride;
        Some(s..s + self.element_stride)
    }

    pub fn iter(&self) -> PropertyTableIter<'a, T> {
        PropertyTableIter {
            data: self.data,
            count: self.count,
            element_stride: self.element_stride,
            has_data: self.has_data,
            row: 0,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: PropertyElement> IntoIterator for &'_ PropertyTablePropertyView<'a, T> {
    type Item = T;
    type IntoIter = PropertyTableIter<'a, T>;
    fn into_iter(self) -> PropertyTableIter<'a, T> {
        self.iter()
    }
}

/// Iterator for [`PropertyTablePropertyView`]. Stores data directly — no borrow of the view.
pub struct PropertyTableIter<'a, T: PropertyElement> {
    data: &'a [u8],
    count: usize,
    element_stride: usize,
    has_data: bool,
    row: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: PropertyElement> Iterator for PropertyTableIter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if !self.has_data || self.row >= self.count {
            return None;
        }
        let v = T::from_le_bytes(&self.data[self.row * self.element_stride..])?;
        self.row += 1;
        Some(v)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let r = self.count.saturating_sub(self.row);
        (r, Some(r))
    }
}
impl<'a, T: PropertyElement> ExactSizeIterator for PropertyTableIter<'a, T> {}

pub struct PropertyAttributePropertyView<'a, T: PropertyElement> {
    data: &'a [u8],
    count: usize,
    stride: usize,
    byte_offset: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: PropertyElement> PropertyAttributePropertyView<'a, T> {
    pub fn new(
        model: &'a GltfModel,
        property_attribute: &PropertyAttribute,
        property_id: &str,
        class_property: Option<ClassProperty>,
        mesh_index: usize,
        primitive_index: usize,
    ) -> Result<Self, PropertyViewStatus> {
        if class_property.is_none() {
            return Err(PropertyViewStatus::NonexistentProperty);
        }
        let attr_prop = property_attribute
            .properties
            .get(property_id)
            .ok_or(PropertyViewStatus::NonexistentProperty)?;
        let prim = model
            .meshes
            .get(mesh_index)
            .and_then(|m| m.primitives.get(primitive_index))
            .ok_or(PropertyViewStatus::InvalidPropertyAttribute)?;
        let &acc_idx = prim
            .attributes
            .get(&attr_prop.attribute)
            .ok_or(PropertyViewStatus::InvalidAccessor)?;
        let acc = model
            .accessors
            .get(acc_idx)
            .ok_or(PropertyViewStatus::InvalidAccessor)?;
        let bv_idx = acc.buffer_view.ok_or(PropertyViewStatus::InvalidAccessor)?;
        let bv = model
            .buffer_views
            .get(bv_idx)
            .ok_or(PropertyViewStatus::InvalidAccessor)?;
        let buf: &[u8] = &model
            .buffers
            .get(bv.buffer)
            .ok_or(PropertyViewStatus::InvalidAccessor)?
            .data;
        let end = bv.byte_offset + bv.byte_length;
        if end > buf.len() {
            return Err(PropertyViewStatus::InvalidAccessor);
        }
        let stride = bv.byte_stride.unwrap_or_else(|| T::byte_size());
        Ok(Self {
            data: &buf[bv.byte_offset..end],
            count: acc.count,
            stride,
            byte_offset: acc.byte_offset,
            _marker: PhantomData,
        })
    }

    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn get_raw(&self, index: usize) -> Option<T> {
        if index >= self.count {
            return None;
        }
        T::from_le_bytes(&self.data[self.byte_offset + index * self.stride..])
    }

    pub fn iter(&self) -> PropertyAttributeIter<'a, T> {
        PropertyAttributeIter {
            data: self.data,
            count: self.count,
            stride: self.stride,
            byte_offset: self.byte_offset,
            index: 0,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: PropertyElement> IntoIterator for &'_ PropertyAttributePropertyView<'a, T> {
    type Item = T;
    type IntoIter = PropertyAttributeIter<'a, T>;
    fn into_iter(self) -> PropertyAttributeIter<'a, T> {
        self.iter()
    }
}

/// Iterator for [`PropertyAttributePropertyView`]. Stores data directly — no borrow of the view.
pub struct PropertyAttributeIter<'a, T: PropertyElement> {
    data: &'a [u8],
    count: usize,
    stride: usize,
    byte_offset: usize,
    index: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: PropertyElement> Iterator for PropertyAttributeIter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.index >= self.count {
            return None;
        }
        let v = T::from_le_bytes(&self.data[self.byte_offset + self.index * self.stride..])?;
        self.index += 1;
        Some(v)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let r = self.count.saturating_sub(self.index);
        (r, Some(r))
    }
}
impl<'a, T: PropertyElement> ExactSizeIterator for PropertyAttributeIter<'a, T> {}

pub struct PropertyTexturePropertyView<'a, T: PropertyElement> {
    image: &'a crate::image::Image,
    channels: Vec<u8>,
    tex_coord_set: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: PropertyElement> PropertyTexturePropertyView<'a, T> {
    pub fn new(
        model: &'a GltfModel,
        property_texture: &PropertyTexture,
        property_id: &str,
        class_property: Option<ClassProperty>,
    ) -> Result<Self, PropertyViewStatus> {
        if class_property.is_none() {
            return Err(PropertyViewStatus::NonexistentProperty);
        }
        let prop = property_texture
            .properties
            .get(property_id)
            .ok_or(PropertyViewStatus::NonexistentProperty)?;
        if prop.channels.is_empty() || prop.channels.len() > 4 {
            return Err(PropertyViewStatus::InvalidChannelCount);
        }
        let tex = model
            .textures
            .get(prop.index)
            .ok_or(PropertyViewStatus::InvalidTexture)?;
        let img_idx = tex.source.ok_or(PropertyViewStatus::InvalidImageIndex)?;
        let img = &model
            .images
            .get(img_idx)
            .ok_or(PropertyViewStatus::InvalidImageIndex)?
            .pixels;
        if img.data.is_empty() {
            return Err(PropertyViewStatus::EmptyImage);
        }
        if img.bytes_per_channel != 1 {
            return Err(PropertyViewStatus::InvalidBytesPerChannel);
        }
        Ok(Self {
            image: img,
            channels: prop.channels.clone(),
            tex_coord_set: prop.tex_coord,
            _marker: PhantomData,
        })
    }

    pub fn tex_coord_set(&self) -> usize {
        self.tex_coord_set
    }

    pub fn sample(&self, u: f32, v: f32) -> Option<T> {
        let img = self.image;
        let x = ((u.clamp(0.0, 1.0) * img.width as f32) as u32).min(img.width - 1);
        let y = ((v.clamp(0.0, 1.0) * img.height as f32) as u32).min(img.height - 1);
        let px = ((y * img.width + x) * img.channels * img.bytes_per_channel) as usize;
        let mut raw = [0u8; 8];
        for (i, &ch) in self.channels.iter().enumerate() {
            raw[i] = *img.data.get(px + ch as usize)?;
        }
        T::from_le_bytes(&raw[..self.channels.len()])
    }
}

#[derive(Debug, Clone)]
pub enum MetadataViewError {
    /// Model carries no `EXT_structural_metadata` extension.
    SchemaNotFound,
    /// Property table index out of range.
    PropertyTableNotFound(usize),
    /// Property attribute index out of range.
    PropertyAttributeNotFound(usize),
    /// Property texture index out of range.
    PropertyTextureNotFound(usize),
    /// Named property not found in the class schema.
    PropertyNotFound(String),
    /// Schema class referenced by this table/attribute/texture not found.
    ClassNotFound(String),
    /// Low-level property view construction failed.
    InvalidProperty(PropertyViewStatus),
}

impl From<PropertyViewStatus> for MetadataViewError {
    fn from(s: PropertyViewStatus) -> Self {
        Self::InvalidProperty(s)
    }
}

impl std::fmt::Display for MetadataViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaNotFound => write!(f, "EXT_structural_metadata not present in model"),
            Self::PropertyTableNotFound(i) => write!(f, "property table {i} not found"),
            Self::PropertyAttributeNotFound(i) => write!(f, "property attribute {i} not found"),
            Self::PropertyTextureNotFound(i) => write!(f, "property texture {i} not found"),
            Self::PropertyNotFound(n) => write!(f, "property '{n}' not found in schema"),
            Self::ClassNotFound(c) => write!(f, "schema class '{c}' not found"),
            Self::InvalidProperty(s) => write!(f, "property view error: {s}"),
        }
    }
}

impl std::error::Error for MetadataViewError {}

fn parse_meta(model: &GltfModel) -> Result<ExtStructuralMetadata, MetadataViewError> {
    ExtStructuralMetadata::from_model(model).ok_or(MetadataViewError::SchemaNotFound)
}

fn dispatch_table_value<'a>(
    model: &'a GltfModel,
    table: &crate::PropertyTable,
    name: &str,
    cp: ClassProperty,
    row: usize,
) -> Option<MetadataValue> {
    use crate::PropertyTablePropertyView as V;
    macro_rules! scalar {
        ($t:ty, $wrap:expr) => {{
            let cp2 = cp.clone();
            V::<$t>::new(model, table, name, Some(cp2))
                .ok()?
                .get_raw(row)
                .map($wrap)
        }};
    }
    match (cp.r#type, cp.component_type.unwrap_or_default()) {
        (PropertyType::Scalar, PropertyComponentType::Uint8) => {
            scalar!(u8, |v| MetadataValue::Uint64(v as u64))
        }
        (PropertyType::Scalar, PropertyComponentType::Int8) => {
            scalar!(i8, |v| MetadataValue::Int64(v as i64))
        }
        (PropertyType::Scalar, PropertyComponentType::Uint16) => {
            scalar!(u16, |v| MetadataValue::Uint64(v as u64))
        }
        (PropertyType::Scalar, PropertyComponentType::Int16) => {
            scalar!(i16, |v| MetadataValue::Int64(v as i64))
        }
        (PropertyType::Scalar, PropertyComponentType::Uint32) => {
            scalar!(u32, |v| MetadataValue::Uint64(v as u64))
        }
        (PropertyType::Scalar, PropertyComponentType::Int32) => {
            scalar!(i32, |v| MetadataValue::Int64(v as i64))
        }
        (PropertyType::Scalar, PropertyComponentType::Uint64) => {
            scalar!(u64, MetadataValue::Uint64)
        }
        (PropertyType::Scalar, PropertyComponentType::Int64) => scalar!(i64, MetadataValue::Int64),
        (PropertyType::Scalar, PropertyComponentType::Float32) => {
            scalar!(f32, MetadataValue::Float32)
        }
        (PropertyType::Scalar, PropertyComponentType::Float64) => {
            scalar!(f64, MetadataValue::Float64)
        }
        (PropertyType::Boolean, _) => scalar!(u8, |v| MetadataValue::Boolean(v != 0)),
        (PropertyType::Vec2, _) => scalar!([f32; 2], MetadataValue::Vec2),
        (PropertyType::Vec3, _) => scalar!([f32; 3], MetadataValue::Vec3),
        (PropertyType::Vec4, _) => scalar!([f32; 4], MetadataValue::Vec4),
        (PropertyType::Mat2, _) => V::<crate::PropertyMat2>::new(model, table, name, Some(cp))
            .ok()?
            .get_raw(row)
            .map(|m| MetadataValue::Mat2(*m)),
        (PropertyType::Mat3, _) => V::<crate::PropertyMat3>::new(model, table, name, Some(cp))
            .ok()?
            .get_raw(row)
            .map(|m| MetadataValue::Mat3(*m)),
        (PropertyType::Mat4, _) => V::<crate::PropertyMat4>::new(model, table, name, Some(cp))
            .ok()?
            .get_raw(row)
            .map(|m| MetadataValue::Mat4(*m)),
        _ => None,
    }
}

/// High-level validated view over an EXT_structural_metadata property table.
pub struct PropertyTableView<'a> {
    model: &'a GltfModel,
    meta: ExtStructuralMetadata,
    table_index: usize,
}

impl<'a> PropertyTableView<'a> {
    pub fn new(model: &'a GltfModel, table_index: usize) -> Result<Self, MetadataViewError> {
        let meta = parse_meta(model)?;
        if table_index >= meta.property_tables.len() {
            return Err(MetadataViewError::PropertyTableNotFound(table_index));
        }
        Ok(Self {
            model,
            meta,
            table_index,
        })
    }

    fn table(&self) -> &crate::PropertyTable {
        &self.meta.property_tables[self.table_index]
    }

    pub fn size(&self) -> i64 {
        self.table().count
    }

    pub fn class_name(&self) -> &str {
        &self.table().class
    }

    pub fn property_names(&self) -> Vec<&str> {
        self.table().properties.keys().map(String::as_str).collect()
    }

    pub fn class_property(&self, name: &str) -> Option<ClassProperty> {
        let class_name = &self.table().class;
        let schema = self.meta.schema.as_ref()?;
        schema
            .classes
            .get(class_name)?
            .properties
            .get(name)
            .cloned()
    }

    pub fn property<T: PropertyElement>(
        &self,
        name: &str,
    ) -> Result<PropertyTablePropertyView<'a, T>, MetadataViewError> {
        let cp = self.class_property(name);
        PropertyTablePropertyView::new(self.model, self.table(), name, cp)
            .map_err(MetadataViewError::from)
    }

    pub fn value(&self, name: &str, row: usize) -> Option<MetadataValue> {
        let cp = self.class_property(name)?;
        dispatch_table_value(self.model, self.table(), name, cp, row)
    }
}

/// High-level view over an EXT_structural_metadata property attribute bound to
/// a specific mesh primitive.
pub struct PropertyAttributeView<'a> {
    model: &'a GltfModel,
    meta: ExtStructuralMetadata,
    attr_index: usize,
    mesh_index: usize,
    primitive_index: usize,
}

impl<'a> PropertyAttributeView<'a> {
    pub fn new(
        model: &'a GltfModel,
        attr_index: usize,
        mesh_index: usize,
        primitive_index: usize,
    ) -> Result<Self, MetadataViewError> {
        let meta = parse_meta(model)?;
        if attr_index >= meta.property_attributes.len() {
            return Err(MetadataViewError::PropertyAttributeNotFound(attr_index));
        }
        Ok(Self {
            model,
            meta,
            attr_index,
            mesh_index,
            primitive_index,
        })
    }

    fn attr(&self) -> &crate::PropertyAttribute {
        &self.meta.property_attributes[self.attr_index]
    }

    fn class_property(&self, name: &str) -> Option<ClassProperty> {
        let class_name = &self.attr().class;
        let schema = self.meta.schema.as_ref()?;
        schema
            .classes
            .get(class_name)?
            .properties
            .get(name)
            .cloned()
    }

    pub fn class_name(&self) -> &str {
        &self.attr().class
    }

    pub fn property_names(&self) -> Vec<&str> {
        self.attr().properties.keys().map(String::as_str).collect()
    }

    pub fn property<T: PropertyElement>(
        &self,
        name: &str,
    ) -> Result<PropertyAttributePropertyView<'a, T>, MetadataViewError> {
        let cp = self.class_property(name);
        PropertyAttributePropertyView::new(
            self.model,
            self.attr(),
            name,
            cp,
            self.mesh_index,
            self.primitive_index,
        )
        .map_err(MetadataViewError::from)
    }
}

/// High-level view over an EXT_structural_metadata property texture.
pub struct PropertyTextureView<'a> {
    model: &'a GltfModel,
    meta: ExtStructuralMetadata,
    texture_index: usize,
}

impl<'a> PropertyTextureView<'a> {
    pub fn new(model: &'a GltfModel, texture_index: usize) -> Result<Self, MetadataViewError> {
        let meta = parse_meta(model)?;
        if texture_index >= meta.property_textures.len() {
            return Err(MetadataViewError::PropertyTextureNotFound(texture_index));
        }
        Ok(Self {
            model,
            meta,
            texture_index,
        })
    }

    fn ptexture(&self) -> &crate::PropertyTexture {
        &self.meta.property_textures[self.texture_index]
    }

    fn class_property(&self, name: &str) -> Option<ClassProperty> {
        let class_name = &self.ptexture().class;
        let schema = self.meta.schema.as_ref()?;
        schema
            .classes
            .get(class_name)?
            .properties
            .get(name)
            .cloned()
    }

    pub fn class_name(&self) -> &str {
        &self.ptexture().class
    }

    pub fn property_names(&self) -> Vec<&str> {
        self.ptexture()
            .properties
            .keys()
            .map(String::as_str)
            .collect()
    }

    pub fn property<T: PropertyElement>(
        &self,
        name: &str,
    ) -> Result<PropertyTexturePropertyView<'a, T>, MetadataViewError> {
        let cp = self.class_property(name);
        PropertyTexturePropertyView::new(self.model, self.ptexture(), name, cp)
            .map_err(MetadataViewError::from)
    }
}
