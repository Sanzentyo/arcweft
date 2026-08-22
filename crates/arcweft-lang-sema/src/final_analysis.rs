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
        CharacterDialogueCharacterType, CharacterDialogueType, CharacterNominalType,
        GenericTypeOwnerId, GenericTypeParameterId, SemanticTypeDigest, TypeKind,
        TypeParameterSubstitutions,
    },
};
use arcweft_character::id::CharacterId;
use arcweft_id::{
    DeclarationIdentityFamily, PublicId,
    dialogue::{DialogueLineId, DialogueTextKey},
};
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
        CallableDeclarationKey, CallableDeclarationOwner, ProjectHirSymbolLookupError,
        ProjectSymbolResolutionError, ProjectSymbolRevision, ProjectSymbolTable,
        ProjectSymbolWorldId,
        nominal::{ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalDeclarationId},
    },
};
use arcweft_lang_syntax::assertion::AssertionMode;

mod accounting;
#[path = "final_analysis/analyzer.rs"]
mod analyzer;
mod error;
mod input;
mod match_edges;
mod model;
mod nominal_schema;
mod recovery_diagnostics;
mod report;
mod semantic_transcript;
mod type_rules;
mod validation;

pub use crate::callable::CharacterDialoguePatchContext;
pub use crate::callable::{
    CallableInstantiationDigest, CheckedCallableJoin, CheckedCallableJoinError,
    IntrinsicCallableCandidateTag,
};
pub(crate) use accounting::{
    CandidateEvaluationPass, CandidateExpectedType, PhysicalArgumentEvaluationKind,
    PhysicalCandidateArgument, PhysicalCandidateArgumentEvaluation,
};
pub use accounting::{FinalSemanticAnalysisControl, FinalSemanticAnalysisWork};
pub use analyzer::{FinalSemanticCatalogs, analyze_final_project};
pub use error::{FinalSemanticAnalysisError, RecursiveCallableContractEdge, SemanticFactFamily};
pub(crate) use input::FinalSemanticAnalysisInput;
pub use match_edges::{
    CheckedChildEdgeError, CheckedExpressionChildRole, CheckedExpressionEdgeError,
    CheckedExpressionEdgeFact, CheckedNestedEvidenceRole, CheckedNestedPathError,
    CheckedNestedPathSegmentV1, CheckedNestedPathV1, NestedPathEvidence,
};
pub use model::{
    AcceptedDeclarationSemanticId, CharacterDialogueFieldCoordinate, CheckedAssertionDisposition,
    CheckedAssignment, CheckedAssignmentPlace, CheckedAwait, CheckedAwaitPendingObserver,
    CheckedBinding, CheckedBindingRole, CheckedBuiltinVariantCase, CheckedCharacterDialogueFactory,
    CheckedCharacterDialoguePatch, CheckedCharacterDialoguePatchField,
    CheckedCharacterDialogueReconfigure, CheckedCharacterDialogueTarget, CheckedChoice,
    CheckedChoiceGoto, CheckedCoverageDomainDigest, CheckedEffectField, CheckedEntryReference,
    CheckedEvaluatedEffect, CheckedExpression, CheckedExpressionChildRolePath,
    CheckedExpressionChildRoleStep, CheckedExpressionResolution, CheckedExpressionSemanticDigest,
    CheckedFunctionExecution, CheckedImplicitCallable, CheckedItem, CheckedItemRole,
    CheckedIteration, CheckedIteratorFamily, CheckedMatchArmFact, CheckedMatchFact,
    CheckedMatchRef, CheckedMatchSemanticDigest, CheckedOrdinaryFunctionEmission,
    CheckedPatchOperation, CheckedPattern, CheckedPatternResolution, CheckedPatternSemanticDigest,
    CheckedPipe, CheckedProjectCallable, CheckedProjectItem, CheckedProjectItemOwner,
    CheckedProjectNominal, CheckedSelectResolution, CheckedStatement, CheckedStatementRole,
    CheckedStyleCallee, CheckedSuspensionRole, CheckedSuspensionStatement, CheckedTraitConformance,
    CheckedTraitIdentity, CheckedTry, CheckedTryBoundary, CheckedTryCarrier, CheckedTypeSelection,
    CheckedValueResolution, CheckedVariantOwner, CheckedVariantResolution, CheckedViewCall,
    CheckedViewCallee, PostfixBracketResolution, RegisteredSemanticValueId,
    StableCheckedValueCoordinate, StablePatternCoordinate, StablePatternCoordinateStep,
};
pub use nominal_schema::{
    NominalSchemaPath, NominalSchemaPathStep, NominalSchemaProjectionError,
    RuntimeProjectNominalKind, RuntimeProjectNominalProjection, project_runtime_type_schema,
};
pub use recovery_diagnostics::{
    CallableTailRecoveryDiagnostic, CallableTailRecoveryProjectionError,
    project_callable_tail_recovery_diagnostics,
};
pub use report::FinalSemanticAnalysis;
pub use semantic_transcript::{
    CheckedCoverageWitness, CheckedGuardClass, CheckedMatch, CheckedMatchArm, CheckedMatchBinding,
    CheckedMatchCoverage, CheckedMatchLimits, CheckedUnreachableArm, CheckedUnreachableReason,
    SemanticTranscriptError,
};
pub(crate) use semantic_transcript::{
    TranscriptHasher, accepted_declaration_id, checked_expression_path, write_len,
    write_value_coordinate,
};

#[cfg(test)]
#[path = "final_analysis/tests.rs"]
mod tests;
