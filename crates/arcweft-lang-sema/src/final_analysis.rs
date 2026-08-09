//! Generation-bound semantic authority for the final arena HIR.
//!
//! The public report in this module is intentionally assembled from typed
//! semantic passes and published only after every live final-HIR owner has one
//! checked fact.  It does not retain a detached syntax tree, source ranges as
//! identities, a linked `HirModule`, or positional sidecar evidence.

use crate::{
    assertion::{AssertionBuildProfile, AssertionContext, AssertionRuntimePolicy},
    callable::{
        CallCalleeClassificationFact, CallPoison, CallTargetFact, CallTargetFacts,
        CallableArgumentSlotIndex, CallableCandidateId, CallableDiagnosticSubject,
        CallableInstantiation, CheckedCallArgumentSlotSource, CheckedCallableCatalog,
        ResolvedCallable, SignatureOrigin,
    },
    checked_rich_text::CheckedRichTextReport,
    effects::EffectSet,
    env::identity::EnvironmentBindingId,
    nominal::TypeResolutionReport,
    types::{
        CharacterNominalType, GenericTypeOwnerId, GenericTypeParameterId, SemanticTypeDigest,
        TypeKind, TypeParameterSubstitutions,
    },
};
use arcweft_character::id::CharacterId;
use arcweft_id::{DeclarationIdentityFamily, PublicId};
use arcweft_lang_hir::{
    expr::{HirCallArgumentOrdinal, HirExprKind},
    identity::{
        CaptureId, ExprId, HirModuleId, HirSnapshotId, ItemId, LocalId, PatternId, StmtId, TypeId,
    },
    item::{HirFlowIdentity, HirItemFamily, HirItemKind},
    leaf::{HirIdRef, HirLiteral, HirName},
    module::HirModule,
    pattern::HirPatternKind,
    project::HirExecutableProjectView,
    stmt::HirStmtKind,
    symbol::{
        CallableDeclarationKey, CallableDeclarationOwner, ProjectSymbolRevision,
        ProjectSymbolTable, ProjectSymbolWorldId,
        nominal::{ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalDeclarationId},
    },
};
use arcweft_lang_syntax::assertion::AssertionMode;

mod accounting;
#[path = "final_analysis/analyzer.rs"]
mod analyzer;
mod error;
mod input;
mod model;
mod recovery_diagnostics;
mod report;
mod type_rules;
mod validation;

pub(crate) use accounting::{
    CandidateEvaluationPass, CandidateExpectedType, PhysicalArgumentEvaluationKind,
    PhysicalCandidateArgument, PhysicalCandidateArgumentEvaluation,
};
pub use accounting::{FinalSemanticAnalysisControl, FinalSemanticAnalysisWork};
pub use analyzer::{FinalSemanticCatalogs, analyze_final_project};
pub use error::{
    FinalSemanticAnalysisError, PropagationOperator, RecursiveCallableContractEdge,
    SemanticFactFamily,
};
pub(crate) use input::FinalSemanticAnalysisInput;
pub use model::{
    CheckedAssertionDisposition, CheckedBinding, CheckedBindingRole, CheckedBuiltinVariantCase,
    CheckedEntryReference, CheckedExpression, CheckedExpressionResolution,
    CheckedFunctionExecution, CheckedItem, CheckedItemRole, CheckedIteration,
    CheckedIteratorFamily, CheckedPattern, CheckedPatternResolution, CheckedProjectCallable,
    CheckedProjectItem, CheckedProjectItemOwner, CheckedProjectNominal, CheckedSelectResolution,
    CheckedStatement, CheckedStatementRole, CheckedStyleCallee, CheckedSuspensionRole,
    CheckedTraitConformance, CheckedTraitIdentity, CheckedTypeSelection, CheckedValueResolution,
    CheckedVariantOwner, CheckedVariantResolution, CheckedViewCall, CheckedViewCallee,
    PostfixBracketResolution, RegisteredSemanticValueId,
};
pub use recovery_diagnostics::{
    CallableTailRecoveryDiagnostic, CallableTailRecoveryProjectionError,
    project_callable_tail_recovery_diagnostics,
};
pub use report::FinalSemanticAnalysis;

#[cfg(test)]
#[path = "final_analysis/tests.rs"]
mod tests;
