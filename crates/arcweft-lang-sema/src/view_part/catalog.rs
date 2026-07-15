use arcweft_id::PublicId;
use arcweft_lang_syntax::ast::{
    common::{TextRange, Visibility},
    module_path::CanonicalModulePath,
};
use arcweft_view::{ViewLocalPartName, ViewPartName};

use crate::canonicalization::SemanticSourceIdentity;

/// Canonical public identity of one checked View definition.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedViewId(PublicId);

/// Deterministic owner-local compact part identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedViewPartId(u32);

/// Owner-qualified checked private part reference.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedViewPartRef {
    owner: CheckedViewId,
    part: CheckedViewPartId,
}

/// Checked node family for a private local target.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedViewPartTargetKind {
    Element,
    Text,
    Image,
    ViewCall,
}

/// Checked runtime multiplicity shape of one static target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedViewPartOccurrenceShape {
    can_be_absent: bool,
    can_repeat: bool,
}

/// One checked private local target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewLocalPart {
    id: CheckedViewPartId,
    name: ViewLocalPartName,
    target_kind: CheckedViewPartTargetKind,
    occurrence: CheckedViewPartOccurrenceShape,
    name_range: TextRange,
    target_range: TextRange,
}

/// Exact revision-bound source evidence and declaration ranges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewPartExportSource {
    identity: Option<SemanticSourceIdentity>,
    declaration_range: TextRange,
    local_range: TextRange,
    public_range: TextRange,
}

/// One checked public capability mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewPartExport {
    owner: CheckedViewId,
    target: CheckedViewPartRef,
    local_name: ViewLocalPartName,
    public_name: ViewPartName,
    source: CheckedViewPartExportSource,
}

/// Checked catalog for one View definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewPartOwner {
    id: CheckedViewId,
    module: CanonicalModulePath,
    visibility: Option<Visibility>,
    range: TextRange,
    source: Option<SemanticSourceIdentity>,
    local_parts: Vec<CheckedViewLocalPart>,
    exports: Vec<CheckedViewPartExport>,
}

/// Deterministically ordered checked part catalog for a module/project.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CheckedViewPartCatalog {
    owners: Vec<CheckedViewPartOwner>,
}

impl CheckedViewId {
    pub(super) const fn from_public_id(id: PublicId) -> Self {
        Self(id)
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }
}

impl CheckedViewPartId {
    pub(super) const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl CheckedViewPartRef {
    pub(super) fn new(owner: CheckedViewId, part: CheckedViewPartId) -> Self {
        Self { owner, part }
    }

    pub const fn owner(&self) -> &CheckedViewId {
        &self.owner
    }

    pub const fn part(&self) -> CheckedViewPartId {
        self.part
    }
}

impl CheckedViewPartOccurrenceShape {
    pub(super) const fn new(can_be_absent: bool, can_repeat: bool) -> Self {
        Self {
            can_be_absent,
            can_repeat,
        }
    }

    pub const fn can_be_absent(self) -> bool {
        self.can_be_absent
    }

    pub const fn can_repeat(self) -> bool {
        self.can_repeat
    }
}

impl CheckedViewLocalPart {
    pub(super) const fn new(
        id: CheckedViewPartId,
        name: ViewLocalPartName,
        target_kind: CheckedViewPartTargetKind,
        occurrence: CheckedViewPartOccurrenceShape,
        name_range: TextRange,
        target_range: TextRange,
    ) -> Self {
        Self {
            id,
            name,
            target_kind,
            occurrence,
            name_range,
            target_range,
        }
    }

    pub const fn id(&self) -> CheckedViewPartId {
        self.id
    }

    pub const fn name(&self) -> &ViewLocalPartName {
        &self.name
    }

    pub const fn target_kind(&self) -> CheckedViewPartTargetKind {
        self.target_kind
    }

    pub const fn occurrence(&self) -> CheckedViewPartOccurrenceShape {
        self.occurrence
    }

    pub const fn name_range(&self) -> TextRange {
        self.name_range
    }

    pub const fn target_range(&self) -> TextRange {
        self.target_range
    }
}

impl CheckedViewPartExportSource {
    pub(super) const fn new(
        identity: Option<SemanticSourceIdentity>,
        declaration_range: TextRange,
        local_range: TextRange,
        public_range: TextRange,
    ) -> Self {
        Self {
            identity,
            declaration_range,
            local_range,
            public_range,
        }
    }

    pub const fn identity(&self) -> Option<&SemanticSourceIdentity> {
        self.identity.as_ref()
    }

    pub const fn declaration_range(&self) -> TextRange {
        self.declaration_range
    }

    pub const fn local_range(&self) -> TextRange {
        self.local_range
    }

    pub const fn public_range(&self) -> TextRange {
        self.public_range
    }
}

impl CheckedViewPartExport {
    pub(super) const fn new(
        owner: CheckedViewId,
        target: CheckedViewPartRef,
        local_name: ViewLocalPartName,
        public_name: ViewPartName,
        source: CheckedViewPartExportSource,
    ) -> Self {
        Self {
            owner,
            target,
            local_name,
            public_name,
            source,
        }
    }

    pub const fn owner(&self) -> &CheckedViewId {
        &self.owner
    }

    pub const fn target(&self) -> &CheckedViewPartRef {
        &self.target
    }

    pub const fn local_name(&self) -> &ViewLocalPartName {
        &self.local_name
    }

    pub const fn public_name(&self) -> &ViewPartName {
        &self.public_name
    }

    pub const fn source(&self) -> &CheckedViewPartExportSource {
        &self.source
    }
}

impl CheckedViewPartOwner {
    pub(super) const fn new(
        id: CheckedViewId,
        module: CanonicalModulePath,
        visibility: Option<Visibility>,
        range: TextRange,
        source: Option<SemanticSourceIdentity>,
        local_parts: Vec<CheckedViewLocalPart>,
        exports: Vec<CheckedViewPartExport>,
    ) -> Self {
        Self {
            id,
            module,
            visibility,
            range,
            source,
            local_parts,
            exports,
        }
    }

    pub const fn id(&self) -> &CheckedViewId {
        &self.id
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }

    pub const fn source(&self) -> Option<&SemanticSourceIdentity> {
        self.source.as_ref()
    }

    pub fn local_parts(&self) -> &[CheckedViewLocalPart] {
        &self.local_parts
    }

    pub fn exports(&self) -> &[CheckedViewPartExport] {
        &self.exports
    }
}

impl CheckedViewPartCatalog {
    pub(super) fn new(mut owners: Vec<CheckedViewPartOwner>) -> Self {
        owners.sort_by(|left, right| left.id.cmp(&right.id));
        Self { owners }
    }

    pub fn owners(&self) -> &[CheckedViewPartOwner] {
        &self.owners
    }

    pub fn owner(&self, id: &CheckedViewId) -> Option<&CheckedViewPartOwner> {
        self.owners.iter().find(|owner| owner.id() == id)
    }
}
