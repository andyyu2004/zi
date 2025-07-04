use url::Url;
use ustr::Ustr;
use zi_core::NamespaceId;

use crate::Editor;
use crate::editor::{Resource, Selector};

pub struct Namespace {
    id: NamespaceId,
    name: Ustr,
    url: Url,
}

impl Namespace {
    pub(crate) fn new(id: NamespaceId, name: impl Into<Ustr>) -> Self {
        let name = name.into();
        let url = Url::parse(&format!("{}://{name}", Self::URL_SCHEME)).unwrap();
        Namespace { id, url, name }
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
