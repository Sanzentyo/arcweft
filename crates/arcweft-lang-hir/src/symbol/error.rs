use std::fmt;

use arcweft_id::{IdError, PublicId};
use arcweft_lang_syntax::ast::{
    module_path::{CanonicalModulePath, ModulePathError},
    symbol_path::SymbolPath,
};
use arcweft_source::SourceSpan;

use crate::leaf::HirIdRef;

use super::{
    CallableDeclarationIdError, ProjectSymbolLimitKind, ProjectSymbolTargetId,
    nominal::ProjectNominalDeclarationError,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectSymbolLinkError {
    DuplicateDeclaration {
        module: CanonicalModulePath,
        name: String,
        sites: Box<[SourceSpan]>,
    },
    DuplicatePublicId {
        public_id: PublicId,
        first: SourceSpan,
        duplicate: SourceSpan,
    },
    InaccessibleImport {
        module: CanonicalModulePath,
        import: SymbolPath,
        source: SourceSpan,
    },
    VisibilityEscalation {
        module: CanonicalModulePath,
        import: SymbolPath,
        source: SourceSpan,
    },
    AmbiguousImport {
        module: CanonicalModulePath,
        import: SymbolPath,
        source: SourceSpan,
        candidates: Vec<ProjectSymbolTargetId>,
    },
    InvalidImportPath {
        module: CanonicalModulePath,
        source: SourceSpan,
        reason: ModulePathError,
    },
    InvalidDeclaration {
        source: SourceSpan,
        reason: CallableDeclarationIdError,
    },
    UnknownImport {
        module: CanonicalModulePath,
        import: SymbolPath,
        source: SourceSpan,
    },
    CyclicImport {
        module: CanonicalModulePath,
        import: SymbolPath,
        source: SourceSpan,
        related: Box<[SourceSpan]>,
    },
    ReservedTypeName {
        module: CanonicalModulePath,
        name: String,
        source: SourceSpan,
    },
    InvalidNominalDeclaration {
        source: SourceSpan,
        reason: Box<ProjectNominalDeclarationError>,
    },
    Limit {
        kind: ProjectSymbolLimitKind,
        observed: u64,
        maximum: u64,
        source: Option<SourceSpan>,
    },
    WorkOverflow {
        attempted: u64,
        maximum: u64,
        source: Option<SourceSpan>,
    },
}

/// Terminal project-table failure for one typed entity reference.
///
/// Retained HIR declarations and registered external declarations are selected
/// by one project-symbol transaction.  The error therefore retains unified
/// target identities instead of exposing a retained-only fallback contract.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectEntityReferenceLookupError {
    #[error("entity reference is unknown")]
    Unknown {
        reference: HirIdRef,
        reference_span: SourceSpan,
    },
    #[error("entity reference is ambiguous")]
    Ambiguous {
        reference: HirIdRef,
        reference_span: SourceSpan,
        candidates: Box<[ProjectSymbolTargetId]>,
    },
    #[error("entity reference is inaccessible")]
    Inaccessible {
        reference: HirIdRef,
        reference_span: SourceSpan,
        candidates: Box<[ProjectSymbolTargetId]>,
    },
    #[error("bare relative ID references have no declaration-family anchor")]
    RelativeRequiresFamily {
        reference: HirIdRef,
        reference_span: SourceSpan,
    },
    #[error("family-relative parent traversal is not admitted by this project identity domain")]
    UnsupportedParentDepth {
        reference: HirIdRef,
        reference_span: SourceSpan,
        parent_depth: usize,
    },
    #[error("entity reference has an invalid public identity")]
    InvalidIdentity {
        reference: HirIdRef,
        reference_span: SourceSpan,
        reason: IdError,
    },
    #[error("entity reference cannot be represented by the typed project-symbol path domain")]
    InvalidReferencePath {
        reference: HirIdRef,
        reference_span: SourceSpan,
    },
    #[error("entity reference has an invalid module path")]
    InvalidModulePath {
        reference: HirIdRef,
        reference_span: SourceSpan,
        reason: ModulePathError,
    },
    #[error("asset identity belongs to the package catalog, not an authored HIR item")]
    CatalogOwned {
        reference: HirIdRef,
        reference_span: SourceSpan,
    },
    #[error("entity reference resolves to a recovered retained declaration")]
    Poisoned {
        reference: HirIdRef,
        reference_span: SourceSpan,
        declaration: SourceSpan,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSymbolLinkReport {
    pub(super) diagnostics: Vec<ProjectSymbolLinkError>,
    pub(super) omitted_diagnostics: u64,
    pub(super) work_charged: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectSymbolDiagnosticCode {
    DuplicateDeclaration,
    DuplicatePublicId,
    InaccessibleImport,
    VisibilityEscalation,
    AmbiguousImport,
    InvalidImportPath,
    InvalidDeclaration,
    UnknownImport,
    CyclicImport,
    ReservedTypeName,
    InvalidNominalDeclaration,
    Limit,
    WorkOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectSymbolResolutionError {
    Unknown {
        module: CanonicalModulePath,
        reference: SymbolPath,
        source: SourceSpan,
    },
    Ambiguous {
        module: CanonicalModulePath,
        reference: SymbolPath,
        source: SourceSpan,
        candidates: Vec<ProjectSymbolTargetId>,
    },
    NotCallable {
        reference: SymbolPath,
        source: SourceSpan,
        actual: ProjectSymbolTargetId,
    },
    InvalidPath {
        source: SourceSpan,
        reason: ModulePathError,
    },
}

impl ProjectSymbolDiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateDeclaration => "aw.project.symbol.duplicate_declaration",
            Self::DuplicatePublicId => "aw.project.symbol.duplicate_public_id",
            Self::InaccessibleImport => "aw.project.symbol.inaccessible_import",
            Self::VisibilityEscalation => "aw.project.symbol.visibility_escalation",
            Self::AmbiguousImport => "aw.project.symbol.ambiguous_import",
            Self::InvalidImportPath => "aw.project.symbol.invalid_import_path",
            Self::InvalidDeclaration => "aw.project.symbol.invalid_declaration",
            Self::UnknownImport => "aw.project.symbol.unknown_import",
            Self::CyclicImport => "aw.project.symbol.cyclic_import",
            Self::ReservedTypeName => "aw.project.symbol.reserved_type_name",
            Self::InvalidNominalDeclaration => "aw.project.symbol.invalid_nominal_declaration",
            Self::Limit => "aw.project.symbol.limit",
            Self::WorkOverflow => "aw.project.symbol.work_overflow",
        }
    }
}

impl ProjectSymbolLinkError {
    pub(super) fn duplicate_declaration(
        module: CanonicalModulePath,
        name: String,
        first: SourceSpan,
        duplicate: SourceSpan,
    ) -> Self {
        Self::DuplicateDeclaration {
            module,
            name,
            sites: Box::new([first, duplicate]),
        }
    }

    /// Returns every declaration site participating in one grouped duplicate
    /// name diagnostic, in deterministic source order.
    pub fn duplicate_declaration_sites(&self) -> Option<&[SourceSpan]> {
        match self {
            Self::DuplicateDeclaration { sites, .. } => Some(sites),
            _ => None,
        }
    }

    pub const fn code(&self) -> ProjectSymbolDiagnosticCode {
        match self {
            Self::DuplicateDeclaration { .. } => ProjectSymbolDiagnosticCode::DuplicateDeclaration,
            Self::DuplicatePublicId { .. } => ProjectSymbolDiagnosticCode::DuplicatePublicId,
            Self::InaccessibleImport { .. } => ProjectSymbolDiagnosticCode::InaccessibleImport,
            Self::VisibilityEscalation { .. } => ProjectSymbolDiagnosticCode::VisibilityEscalation,
            Self::AmbiguousImport { .. } => ProjectSymbolDiagnosticCode::AmbiguousImport,
            Self::InvalidImportPath { .. } => ProjectSymbolDiagnosticCode::InvalidImportPath,
            Self::InvalidDeclaration { .. } => ProjectSymbolDiagnosticCode::InvalidDeclaration,
            Self::UnknownImport { .. } => ProjectSymbolDiagnosticCode::UnknownImport,
            Self::CyclicImport { .. } => ProjectSymbolDiagnosticCode::CyclicImport,
            Self::ReservedTypeName { .. } => ProjectSymbolDiagnosticCode::ReservedTypeName,
            Self::InvalidNominalDeclaration { .. } => {
                ProjectSymbolDiagnosticCode::InvalidNominalDeclaration
            }
            Self::Limit { .. } => ProjectSymbolDiagnosticCode::Limit,
            Self::WorkOverflow { .. } => ProjectSymbolDiagnosticCode::WorkOverflow,
        }
    }

    pub(super) fn source(&self) -> Option<&SourceSpan> {
        match self {
            Self::DuplicateDeclaration { sites, .. } => sites.last(),
            Self::DuplicatePublicId { duplicate, .. } => Some(duplicate),
            Self::InaccessibleImport { source, .. }
            | Self::VisibilityEscalation { source, .. }
            | Self::AmbiguousImport { source, .. }
            | Self::InvalidImportPath { source, .. }
            | Self::InvalidDeclaration { source, .. }
            | Self::UnknownImport { source, .. }
            | Self::CyclicImport { source, .. }
            | Self::ReservedTypeName { source, .. }
            | Self::InvalidNominalDeclaration { source, .. } => Some(source),
            Self::Limit { source, .. } | Self::WorkOverflow { source, .. } => source.as_ref(),
        }
    }
}

impl fmt::Display for ProjectSymbolLinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDeclaration { module, name, .. } => {
                write!(
                    formatter,
                    "module `{module}` declares `{name}` more than once"
                )
            }
            Self::DuplicatePublicId { public_id, .. } => {
                write!(
                    formatter,
                    "project declares public ID `{public_id}` more than once"
                )
            }
            Self::InaccessibleImport { module, import, .. } => {
                write!(
                    formatter,
                    "module `{module}` cannot access import `{import}`"
                )
            }
            Self::VisibilityEscalation { module, .. } => {
                write!(
                    formatter,
                    "module `{module}` cannot widen import visibility"
                )
            }
            Self::AmbiguousImport { module, .. } => {
                write!(formatter, "module `{module}` imports an ambiguous symbol")
            }
            Self::InvalidImportPath { module, .. } => {
                write!(formatter, "module `{module}` has an invalid import path")
            }
            Self::InvalidDeclaration { .. } => {
                formatter.write_str("callable declaration identity is invalid")
            }
            Self::UnknownImport { module, import, .. } => {
                write!(
                    formatter,
                    "module `{module}` cannot resolve import `{import}`"
                )
            }
            Self::CyclicImport { module, import, .. } => {
                write!(
                    formatter,
                    "module `{module}` has an unanchored import cycle through `{import}`"
                )
            }
            Self::ReservedTypeName { module, name, .. } => {
                write!(
                    formatter,
                    "module `{module}` declares reserved type name `{name}`"
                )
            }
            Self::InvalidNominalDeclaration { .. } => {
                formatter.write_str("nominal declaration is invalid")
            }
            Self::Limit { .. } => formatter.write_str("project symbol limit exceeded"),
            Self::WorkOverflow { .. } => {
                formatter.write_str("project symbol work counter overflowed")
            }
        }
    }
}

impl std::error::Error for ProjectSymbolLinkError {}

impl fmt::Display for ProjectSymbolResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown { .. } => {
                formatter.write_str("symbol is not visible from the requested module")
            }
            Self::Ambiguous { .. } => formatter.write_str("symbol reference is ambiguous"),
            Self::NotCallable { .. } => formatter.write_str("symbol does not name a callable"),
            Self::InvalidPath { .. } => formatter.write_str("symbol reference path is invalid"),
        }
    }
}

impl std::error::Error for ProjectSymbolResolutionError {}

impl ProjectSymbolLinkReport {
    pub fn diagnostics(&self) -> &[ProjectSymbolLinkError] {
        &self.diagnostics
    }

    pub const fn omitted_diagnostics(&self) -> u64 {
        self.omitted_diagnostics
    }

    pub const fn work_charged(&self) -> u64 {
        self.work_charged
    }
}
