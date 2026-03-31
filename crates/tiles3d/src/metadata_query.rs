//! [`MetadataQuery`] — search 3D Tiles metadata schemas by semantic.
//!
//! Mirrors `Cesium3DTiles::MetadataQuery` from cesium-native.
//!
//! # Example
//!
//! ```
//! use tiles3d::{MetadataQuery, Schema, MetadataEntity};
//! use std::collections::HashMap;
//!
//! let schema = Schema::default();
//! let entity = MetadataEntity { class: "Building".into(), ..Default::default() };
//! let hit = MetadataQuery::find_first_property_with_semantic(&schema, &entity, "NAME");
//! assert!(hit.is_none()); // empty schema → no match
//! ```

use crate::generated::{Class, ClassProperty, MetadataEntity, Schema};

/// A property found by [`MetadataQuery::find_first_property_with_semantic`].
#[derive(Debug)]
pub struct FoundMetadataProperty<'a> {
    /// The id of the class the property belongs to.
    pub class_id: &'a str,
    /// The class definition.
    pub class_def: &'a Class,
    /// The id of the matching property within the class.
    pub property_id: &'a str,
    /// The property definition.
    pub property_def: &'a ClassProperty,
    /// The value stored for this property in the entity, if present.
    pub value: Option<&'a serde_json::Value>,
}

/// Static helpers for querying 3D Tiles metadata.
pub struct MetadataQuery;

impl MetadataQuery {
    /// Find the first property in `entity`'s class that has the given
    /// `semantic` string, searching `schema`.
    ///
    /// Returns `None` if:
    /// - `schema.classes` doesn't contain `entity.class`,
    /// - no property in that class has the requested semantic, or
    /// - `schema` is empty.
    pub fn find_first_property_with_semantic<'a>(
        schema: &'a Schema,
        entity: &'a MetadataEntity,
        semantic: &str,
    ) -> Option<FoundMetadataProperty<'a>> {
        let class_id = entity.class.as_str();
        let class_def = schema.classes.get(class_id)?;

        for (property_id, property_def) in &class_def.properties {
            if property_def
                .semantic
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case(semantic))
            {
                let value = entity.properties.as_ref().and_then(|p| p.get(property_id.as_str()));
                return Some(FoundMetadataProperty {
                    class_id,
                    class_def,
                    property_id: property_id.as_str(),
                    property_def,
                    value,
                });
            }
        }
        None
    }

    /// Find all properties across all classes in `schema` that have the given
    /// `semantic`. Unlike [`find_first_property_with_semantic`](Self::find_first_property_with_semantic)
    /// this is schema-wide and does not filter by entity class.
    pub fn find_all_properties_with_semantic<'a>(
        schema: &'a Schema,
        semantic: &str,
    ) -> Vec<(&'a str, &'a str, &'a ClassProperty)> {
        let mut results = Vec::new();
        for (class_id, class_def) in &schema.classes {
            for (property_id, property_def) in &class_def.properties {
                if property_def
                    .semantic
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case(semantic))
                {
                    results.push((class_id.as_str(), property_id.as_str(), property_def));
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Class, ClassProperty};
    use std::collections::HashMap;

    fn schema_with_semantic(semantic: &str) -> Schema {
        let prop = ClassProperty {
            semantic: Some(semantic.into()),
            name: Some("my_prop".into()),
            ..Default::default()
        };
        let mut props = HashMap::new();
        props.insert("myProp".into(), prop);

        let class = Class {
            properties: props,
            ..Default::default()
        };
        let mut classes = HashMap::new();
        classes.insert("Building".into(), class);

        Schema {
            classes,
            ..Default::default()
        }
    }

    fn entity(class: &str) -> MetadataEntity {
        MetadataEntity {
            class: class.into(),
            ..Default::default()
        }
    }

    #[test]
    fn finds_property_by_semantic() {
        let schema = schema_with_semantic("NAME");
        let e = entity("Building");
        let hit = MetadataQuery::find_first_property_with_semantic(&schema, &e, "NAME");
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.class_id, "Building");
        assert_eq!(hit.property_id, "myProp");
    }

    #[test]
    fn semantic_match_is_case_insensitive() {
        let schema = schema_with_semantic("NAME");
        let e = entity("Building");
        assert!(MetadataQuery::find_first_property_with_semantic(&schema, &e, "name").is_some());
    }

    #[test]
    fn returns_none_for_missing_class() {
        let schema = schema_with_semantic("NAME");
        let e = entity("Road");
        assert!(MetadataQuery::find_first_property_with_semantic(&schema, &e, "NAME").is_none());
    }

    #[test]
    fn returns_none_for_wrong_semantic() {
        let schema = schema_with_semantic("NAME");
        let e = entity("Building");
        assert!(MetadataQuery::find_first_property_with_semantic(&schema, &e, "HEIGHT").is_none());
    }

    #[test]
    fn find_all_schema_wide() {
        let schema = schema_with_semantic("NAME");
        let hits = MetadataQuery::find_all_properties_with_semantic(&schema, "Name");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "Building");
    }
}
