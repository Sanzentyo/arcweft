//! Typed HIR entry declarations and role references.

use arcweft_lang_syntax::{
    ast::{
        common::{TextRange, Visibility},
        ids::EntityRef,
        items::{EntryKind, EntryRouteBinding},
        module_path::CanonicalModulePath,
    },
    expr::{DottedPath, Expr},
    types::TypeRef,
};

/// HIR-owned entry declaration.
///
/// The entry keeps authored ranges and typed role references, but does not
/// retain or clone the syntax-layer entry item. Semantic checking resolves
/// these references to ordinary nominal, callable, and flow declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirEntryDecl {
    module_path: Option<CanonicalModulePath>,
    kind: EntryKind,
    visibility: Option<Visibility>,
    id: EntityRef,
    items: Vec<HirEntryItem>,
    range: TextRange,
}

/// One typed role or adapter member inside a HIR entry declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirEntryItem {
    StateType {
        ty: TypeRef,
        value_range: TextRange,
        range: TextRange,
    },
    Initializer {
        path: DottedPath,
        value_range: TextRange,
        range: TextRange,
    },
    EventType {
        ty: TypeRef,
        value_range: TextRange,
        range: TextRange,
    },
    Reducer {
        path: DottedPath,
        value_range: TextRange,
        range: TextRange,
    },
    Controller {
        path: DottedPath,
        value_range: TextRange,
        range: TextRange,
    },
    Goto(EntityRef),
    Route {
        method: String,
        path: String,
        target: EntityRef,
        bindings: Vec<EntryRouteBinding>,
    },
    Option {
        name: String,
        value: Expr,
    },
    Raw(String),
}

impl HirEntryDecl {
    pub(crate) fn new(
        module_path: Option<CanonicalModulePath>,
        kind: EntryKind,
        visibility: Option<Visibility>,
        id: EntityRef,
        items: Vec<HirEntryItem>,
        range: TextRange,
    ) -> Self {
        Self {
            module_path,
            kind,
            visibility,
            id,
            items,
            range,
        }
    }

    /// Canonical project module that owns this declaration.
    pub const fn module_path(&self) -> Option<&CanonicalModulePath> {
        self.module_path.as_ref()
    }

    pub const fn kind(&self) -> &EntryKind {
        &self.kind
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> &EntityRef {
        &self.id
    }

    pub fn items(&self) -> &[HirEntryItem] {
        &self.items
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    pub(crate) fn bind_project_module(&mut self, module: &CanonicalModulePath) {
        self.module_path = Some(module.clone());
    }
}

impl HirEntryItem {
    /// Exact range of the role value, excluding its member name and `=`.
    pub const fn value_range(&self) -> Option<&TextRange> {
        match self {
            Self::StateType { value_range, .. }
            | Self::Initializer { value_range, .. }
            | Self::EventType { value_range, .. }
            | Self::Reducer { value_range, .. }
            | Self::Controller { value_range, .. } => Some(value_range),
            Self::Goto(_) | Self::Route { .. } | Self::Option { .. } | Self::Raw(_) => None,
        }
    }

    /// Exact range of the complete role member.
    pub const fn range(&self) -> Option<&TextRange> {
        match self {
            Self::StateType { range, .. }
            | Self::Initializer { range, .. }
            | Self::EventType { range, .. }
            | Self::Reducer { range, .. }
            | Self::Controller { range, .. } => Some(range),
            Self::Goto(_) | Self::Route { .. } | Self::Option { .. } | Self::Raw(_) => None,
        }
    }
}
