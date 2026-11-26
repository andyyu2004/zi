use rel::Relation;
use ustr::Ustr;
use zi_core::NamespaceId;

use crate::Editor;
use crate::editor::Selector;

#[derive(Debug, Relation)]
#[relation(key = NamespaceId)]
pub struct Namespace {
    id: NamespaceId,
    #[index(unique)]
    name: Ustr,
}

impl Namespace {
    pub(crate) fn new(id: NamespaceId, name: impl Into<Ustr>) -> Self {
        Namespace { id, name: name.into() }
    }

    #[inline]
    pub fn id(&self) -> NamespaceId {
        self.id
    }

    #[inline]
    pub fn name(&self) -> Ustr {
        self.name
    }
}

impl Selector<NamespaceId> for NamespaceId {
    fn select(&self, _editor: &Editor) -> NamespaceId {
        *self
    }
}
