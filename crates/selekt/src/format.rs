// This module previously contained `NoopResolver` and related hierarchy
// expansion types. The resolver concept has been removed — `ContentLoader::load`
// now returns `NodeContent` directly, including any sub-scene references inline.
