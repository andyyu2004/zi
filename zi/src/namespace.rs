use zi_lsp::lsp_types::Url;

use crate::editor::{Resource, Selector};
use crate::private::Sealed;
use crate::Editor;

slotmap::new_key_type! {
    pub struct NamespaceId;
}

pub struct Namespace {
    id: NamespaceId,
    url: Url,
}

impl Namespace {
    pub(crate) fn new(id: NamespaceId) -> Self {
        let url = Url::parse(&format!("view://{}", id.0.as_ffi())).unwrap();
        Namespace { id, url }
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
