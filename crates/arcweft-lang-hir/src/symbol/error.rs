use std::fmt;

use arcweft_lang_syntax::ast::{
    module_path::{CanonicalModulePath, ModulePathError},
    symbol_path::SymbolPath,
};
use arcweft_source::SourceSpan;

use super::{CallableDeclarationIdError, ProjectSymbolLimitKind, ProjectSymbolTargetId};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectSymbolLinkError {
    DuplicateDeclaration {
        module: CanonicalModulePath,
        name: String,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSymbolLinkReport {
    pub(super) diagnostics: Vec<ProjectSymbolLinkError>,
    pub(super) omitted_diagnostics: u64,
    pub(super) work_charged: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectSymbolDiagnosticCode {
    DuplicateDeclaration,
    InaccessibleImport,
    VisibilityEscalation,
    AmbiguousImport,
    InvalidImportPath,
    InvalidDeclaration,
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
            Self::InaccessibleImport => "aw.project.symbol.inaccessible_import",
            Self::VisibilityEscalation => "aw.project.symbol.visibility_escalation",
            Self::AmbiguousImport => "aw.project.symbol.ambiguous_import",
            Self::InvalidImportPath => "aw.project.symbol.invalid_import_path",
            Self::InvalidDeclaration => "aw.project.symbol.invalid_declaration",
            Self::Limit => "aw.project.symbol.limit",
            Self::WorkOverflow => "aw.project.symbol.work_overflow",
        }
    }
}

impl ProjectSymbolLinkError {
    pub const fn code(&self) -> ProjectSymbolDiagnosticCode {
        match self {
            Self::DuplicateDeclaration { .. } => ProjectSymbolDiagnosticCode::DuplicateDeclaration,
            Self::InaccessibleImport { .. } => ProjectSymbolDiagnosticCode::InaccessibleImport,
            Self::VisibilityEscalation { .. } => ProjectSymbolDiagnosticCode::VisibilityEscalation,
            Self::AmbiguousImport { .. } => ProjectSymbolDiagnosticCode::AmbiguousImport,
            Self::InvalidImportPath { .. } => ProjectSymbolDiagnosticCode::InvalidImportPath,
            Self::InvalidDeclaration { .. } => ProjectSymbolDiagnosticCode::InvalidDeclaration,
            Self::Limit { .. } => ProjectSymbolDiagnosticCode::Limit,
            Self::WorkOverflow { .. } => ProjectSymbolDiagnosticCode::WorkOverflow,
        }
    }

    pub(super) fn source(&self) -> Option<&SourceSpan> {
        match self {
            Self::DuplicateDeclaration { duplicate, .. } => Some(duplicate),
            Self::InaccessibleImport { source, .. }
            | Self::VisibilityEscalation { source, .. }
            | Self::AmbiguousImport { source, .. }
            | Self::InvalidImportPath { source, .. }
            | Self::InvalidDeclaration { source, .. } => Some(source),
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
