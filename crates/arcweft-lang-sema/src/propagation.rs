//! Lexical return-boundary evidence for `try` and propagating `await`.

use arcweft_lang_hir::symbol::CallableDeclarationId;
use arcweft_source::SourceSpan;

use crate::types::TypeKind;

/// Source-language owner that establishes a return-propagation boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PropagationBoundaryKind {
    Function,
    Closure,
    Method,
    Flow,
}

/// Checked return type available at a lexical propagation boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedReturnType {
    Known(TypeKind),
    Unconstrained,
}

/// Accepted type and source evidence for one lexical return boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropagationBoundaryEvidence {
    kind: PropagationBoundaryKind,
    declaration: Option<CallableDeclarationId>,
    checked_return: CheckedReturnType,
    header: SourceSpan,
    result: Option<SourceSpan>,
}

impl PropagationBoundaryEvidence {
    pub(crate) fn new(
        kind: PropagationBoundaryKind,
        declaration: Option<CallableDeclarationId>,
        checked_return: CheckedReturnType,
        header: SourceSpan,
        result: Option<SourceSpan>,
    ) -> Self {
        Self {
            kind,
            declaration,
            checked_return,
            header,
            result,
        }
    }

    pub const fn kind(&self) -> PropagationBoundaryKind {
        self.kind
    }

    pub const fn declaration(&self) -> Option<&CallableDeclarationId> {
        self.declaration.as_ref()
    }

    pub const fn checked_return(&self) -> &CheckedReturnType {
        &self.checked_return
    }

    pub const fn header(&self) -> &SourceSpan {
        &self.header
    }

    pub const fn result(&self) -> Option<&SourceSpan> {
        self.result.as_ref()
    }

    pub fn related(&self) -> &SourceSpan {
        self.result.as_ref().unwrap_or(&self.header)
    }
}

/// Source evidence for an already classified generator terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropagationBarrierEvidence {
    owner: SourceSpan,
}

impl PropagationBarrierEvidence {
    pub(crate) const fn new(owner: SourceSpan) -> Self {
        Self { owner }
    }

    pub const fn owner(&self) -> &SourceSpan {
        &self.owner
    }
}

/// Nearest lexical propagation target retained in a structured diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropagationTargetEvidence {
    Boundary(PropagationBoundaryEvidence),
    GeneratorTerminal(PropagationBarrierEvidence),
}

impl PropagationTargetEvidence {
    /// Exact source span used as the related owner label.
    pub fn related(&self) -> &SourceSpan {
        match self {
            Self::Boundary(boundary) => boundary.related(),
            Self::GeneratorTerminal(barrier) => barrier.owner(),
        }
    }
}

/// Result/Option envelope supplied to a general Try expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TryPropagationOperand {
    Result { actual_error: TypeKind },
    Option,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReturnPropagationFrame {
    Boundary(PropagationBoundaryEvidence),
    SourceLessBoundary(CheckedReturnType),
    GeneratorTerminal(PropagationBarrierEvidence),
}

impl ReturnPropagationFrame {
    pub(crate) fn target(&self) -> Option<PropagationTargetEvidence> {
        match self {
            Self::Boundary(boundary) => Some(PropagationTargetEvidence::Boundary(boundary.clone())),
            Self::SourceLessBoundary(_) => None,
            Self::GeneratorTerminal(barrier) => Some(PropagationTargetEvidence::GeneratorTerminal(
                barrier.clone(),
            )),
        }
    }

    pub(crate) const fn checked_return(&self) -> Option<&TypeKind> {
        match self {
            Self::Boundary(PropagationBoundaryEvidence {
                checked_return: CheckedReturnType::Known(ty),
                ..
            })
            | Self::SourceLessBoundary(CheckedReturnType::Known(ty)) => Some(ty),
            Self::Boundary(_)
            | Self::SourceLessBoundary(CheckedReturnType::Unconstrained)
            | Self::GeneratorTerminal(_) => None,
        }
    }
}
