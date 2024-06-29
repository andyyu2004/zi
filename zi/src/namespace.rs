use zi_lsp::lsp_types::Url;

use crate::editor::{Resource, Selector};
use crate::private::Sealed;
use crate::symbol::Symbol;
use crate::Editor;

slotmap::new_key_type! {
    pub struct NamespaceId;
}

pub struct Namespace {
    id: NamespaceId,
    name: Symbol,
    url: Url,
}

impl Namespace {
    pub(crate) fn new(id: NamespaceId, name: impl Into<Symbol>) -> Self {
        let name = name.into();
        let url = Url::parse(&format!("{}://{name}", Self::URL_SCHEME)).unwrap();
        Namespace { id, url, name }
    }

    #[inline]
    pub fn name(&self) -> &Symbol {
        &self.name
    }
}

impl Sealed for NamespaceId {}

impl Selector<NamespaceId> for NamespaceId {
    fn select(&self, _editor: &Editor) -> NamespaceId {
        *self
    }
}

impl Resource for Namespace {
    type Id = NamespaceId;

    const URL_SCHEME: &'static str = "namespace";

    #[inline]
    fn id(&self) -> NamespaceId {
        self.id
    }

    #[inline]
    fn url(&self) -> &Url {
        assert_eq!(self.url.scheme(), Self::URL_SCHEME);
        &self.url
    }
}
