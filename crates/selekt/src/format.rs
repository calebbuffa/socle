use crate::hierarchy::HierarchyExpansion;
use crate::hierarchy::HierarchyResolver;
use crate::load::HierarchyReference;

/// A no-op [`HierarchyResolver`] for formats that never produce
/// [`Payload::Reference`](crate::load::Payload::Reference).
///
/// This is the default resolver used by [`SelectionEngineBuilder`](crate::engine::SelectionEngineBuilder).
/// Override it with [`SelectionEngineBuilder::with_resolver`](crate::engine::SelectionEngineBuilder::with_resolver)
/// when your format uses external hierarchy references.
#[derive(Default)]
pub struct NoopResolver;

impl HierarchyResolver for NoopResolver {
    type Error = std::convert::Infallible;

    fn resolve_reference(
        &self,
        _bg_context: &orkester::Context,
        _reference: HierarchyReference,
    ) -> orkester::Task<Result<Option<HierarchyExpansion>, Self::Error>> {
        orkester::resolved(Ok(None))
    }
}
