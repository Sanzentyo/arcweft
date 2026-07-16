use std::sync::Arc;

/// Exact validated LSP URI spelling used as an internal map key.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct LspUriKey(Arc<str>);

impl LspUriKey {
    pub(crate) fn from_uri(uri: &lsp_types::Uri) -> Self {
        Self(Arc::from(uri.as_str()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn to_uri(&self) -> lsp_types::Uri {
        self.as_str()
            .parse()
            .expect("LspUriKey is constructed from an already validated LSP URI")
    }
}

impl std::fmt::Display for LspUriKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
