use std::collections::BTreeMap;

use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::LaunchProfileManifest;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LaunchKeyPath(Vec<String>);

impl LaunchKeyPath {
    pub fn new(segments: Vec<String>) -> Self {
        Self(segments)
    }

    pub fn segments(&self) -> &[String] {
        &self.0
    }

    pub(crate) fn profile_field(&self) -> Option<&str> {
        match self.0.as_slice() {
            [profiles, _, field] if profiles == "profiles" => Some(field),
            _ => None,
        }
    }

    pub(crate) fn extended(&self, segments: impl IntoIterator<Item = String>) -> Self {
        let mut path = self.0.clone();
        path.extend(segments);
        Self(path)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LaunchTokenPath {
    Table {
        path: LaunchKeyPath,
        occurrence: usize,
    },
    Key {
        path: LaunchKeyPath,
        occurrence: usize,
    },
    ArrayElement {
        path: LaunchKeyPath,
        occurrence: usize,
        index: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchToken {
    key: SourceSpan,
    value: Option<SourceSpan>,
    string_content: Option<SourceSpan>,
}

impl LaunchToken {
    pub(crate) const fn new(
        key: SourceSpan,
        value: Option<SourceSpan>,
        string_content: Option<SourceSpan>,
    ) -> Self {
        Self {
            key,
            value,
            string_content,
        }
    }

    pub const fn key(&self) -> &SourceSpan {
        &self.key
    }

    pub const fn value(&self) -> Option<&SourceSpan> {
        self.value.as_ref()
    }

    /// Raw source span inside a quoted TOML string value, excluding delimiters.
    pub const fn string_content(&self) -> Option<&SourceSpan> {
        self.string_content.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchManifestSourceMap {
    document: SourceDocumentIdentity,
    tokens: BTreeMap<LaunchTokenPath, LaunchToken>,
}

impl LaunchManifestSourceMap {
    pub(crate) fn new(
        document: SourceDocumentIdentity,
        tokens: BTreeMap<LaunchTokenPath, LaunchToken>,
    ) -> Self {
        Self { document, tokens }
    }

    pub const fn document(&self) -> &SourceDocumentIdentity {
        &self.document
    }

    pub fn token(&self, path: &LaunchTokenPath) -> Option<&LaunchToken> {
        self.tokens.get(path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBackedLaunchManifest {
    pub(crate) manifest: LaunchProfileManifest,
    pub(crate) source_map: LaunchManifestSourceMap,
}

impl SourceBackedLaunchManifest {
    pub const fn manifest(&self) -> &LaunchProfileManifest {
        &self.manifest
    }

    pub const fn source_map(&self) -> &LaunchManifestSourceMap {
        &self.source_map
    }
}
