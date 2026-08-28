//! Generation-bound semantic authority for the final arena HIR.
//!
//! The public report in this module is intentionally assembled from typed
//! semantic passes and published only after every live final-HIR owner has one
//! checked fact.  It does not retain a detached syntax tree, source ranges as
//! identities, a linked `HirModule`, or positional sidecar evidence.

use crate::{
    assertion::{AssertionBuildProfile, AssertionContext, AssertionRuntimePolicy},
    callable::{
        CallAnalysisOutcome, CallCalleeClassificationFact, CallTargetFacts,
        CallableArgumentSlotIndex, CallableCandidateId, CallableDiagnosticSubject,
        CheckedCallArgumentSlotSource, CheckedCallCalleeExecution, CheckedCallResult,
        CheckedCallableCatalog, ResolvedCallable, ResolvedCallableOrigin,
    },
    checked_rich_text::CheckedRichTextReport,
    effects::EffectSet,
    env::identity::EnvironmentBindingId,
    nominal::TypeResolutionReport,
    types::{
        CharacterDialogueCharacterType, CharacterDialogueType, CharacterNominalType,
        GenericParameterOwnerId, GenericTypeParameterId, SemanticTypeDigest, TypeKind,
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
    leaf::{HirIdRef, HirLiteral},
    module::HirModule,
    pattern::HirPatternKind,
    project::HirExecutableProjectView,
    stmt::HirStmtKind,
    symbol::{
        CallableDeclarationKey, CallableDeclarationOwner, ProjectHirSymbolLookupError,
        ProjectSymbolResolutionError, ProjectSymbolTable,
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
mod prepared;
mod recovery_diagnostics;
mod report;
mod semantic_transcript;
mod type_rules;
mod validation;

pub use crate::callable::CharacterDialoguePatchContext;
pub use crate::callable::{
    CallableInstantiationDigest, CheckedCallableJoin, CheckedCallableJoinDigest,
    CheckedCallableJoinError, IntrinsicCallableCandidateTag,
};
pub(crate) use accounting::{
    CandidateEvaluationPass, CandidateExpectedType, PhysicalArgumentEvaluationKind,
    PhysicalCandidateArgument, PhysicalCandidateArgumentEvaluation,
};
pub use accounting::{FinalSemanticAnalysisControl, FinalSemanticAnalysisWork};
pub use analyzer::{FinalSemanticCatalogs, analyze_final_project};
pub use error::{
    CandidateFactTransactionViolation, FinalCallConstraintFailure, FinalCallFrameInvariant,
    FinalCallSealFailure, FinalCallSealLocation, FinalSemanticAnalysisError,
    FinalSemanticProjectError, RecursiveCallableContractEdge, SemanticFactFamily,
};
pub(crate) use input::FinalSemanticAnalysisInput;
pub use match_edges::{
    CheckedChildEdgeError, CheckedExpressionEdgeError, CheckedExpressionEdgeFact,
    CheckedNestedEvidenceRole, NestedPathEvidence,
};
pub use model::{
    CharacterDialogueFieldCoordinate, CheckedAssertionDisposition, CheckedAssignment,
    CheckedAssignmentPlace, CheckedAwait, CheckedAwaitPendingObserver, CheckedBinding,
    CheckedBindingRole, CheckedCapture, CheckedCaptureAuthorityViolation,
    CheckedCharacterDialogueFactory, CheckedCharacterDialoguePatch,
    CheckedCharacterDialoguePatchField, CheckedCharacterDialogueReconfigure,
    CheckedCharacterDialogueTarget, CheckedChoice, CheckedChoiceGoto, CheckedClosure,
    CheckedCoverageDomainDigest, CheckedDialogueEffectOperation, CheckedDialogueEffectSite,
    CheckedDialogueEffectSiteOrdinal, CheckedDialogueEffectTrigger, CheckedDialogueLinePlan,
    CheckedDialogueMarkHandler, CheckedDialogueMarkOrdinal, CheckedDropFade, CheckedDropPolicy,
    CheckedEffectField, CheckedEntryReference, CheckedEvaluatedEffect, CheckedExpression,
    CheckedExpressionRecordField, CheckedExpressionResolution, CheckedExpressionSemanticDigest,
    CheckedFieldSelection, CheckedFunctionExecution, CheckedImplicitCallable,
    CheckedImplicitCaptureUse, CheckedItem, CheckedItemRole, CheckedIteration,
    CheckedIteratorFamily, CheckedMatchArmFact, CheckedMatchFact, CheckedMatchRef,
    CheckedMatchSemanticDigest, CheckedMethodSelection, CheckedOrdinaryFunctionEmission,
    CheckedPatchOperation, CheckedPattern, CheckedPatternResolution, CheckedPatternSemanticDigest,
    CheckedPipe, CheckedProjectCallable, CheckedProjectItem, CheckedProjectItemOwner,
    CheckedProjectNominal, CheckedRecordBindingSource, CheckedRecordExpressionSource,
    CheckedRecordPattern, CheckedRecordPatternField, CheckedRecordPatternOwner,
    CheckedRecordPatternRest, CheckedRecordPatternSource, CheckedRecordPatternSourceRef,
    CheckedRecordValueSource, CheckedSelectResolution, CheckedStageLook, CheckedStatement,
    CheckedStatementRole, CheckedStyleCallee, CheckedSuspensionRole, CheckedSuspensionStatement,
    CheckedTraitConformance, CheckedTraitIdentity, CheckedTry, CheckedTryBoundary,
    CheckedTryCarrier, CheckedTypeSelection, CheckedTypedBinding, CheckedValueResolution,
    CheckedVariantCase, CheckedVariantOwner, CheckedVariantResolution, CheckedViewCall,
    CheckedViewCallee, PostfixBracketResolution, RegisteredSemanticValueId,
};
pub(crate) use nominal_schema::RuntimeNominalProjectionSeal;
pub use nominal_schema::{
    NominalProjectionLimitKind, NominalSchemaPath, NominalSchemaPathStep,
    NominalSchemaProjectionError, RuntimeProjectFieldProjection, RuntimeProjectNominalKind,
    RuntimeProjectNominalProjection, RuntimeProjectVariantCaseProjection,
    project_runtime_type_schema,
};
pub(crate) use prepared::{
    PreparedAssignmentStatement, PreparedEntryExpression, PreparedEntryReference,
    PreparedExpressionFact, PreparedExpressionShell, PreparedMethodExpression, PreparedPatternFact,
    PreparedProjectFieldExpression, PreparedProjectRecordExpression,
    PreparedProjectRecordExpressionField, PreparedProjectVariantExpression,
    PreparedProjectVariantOwnerSeed, PreparedProjectVariantPattern, PreparedRecordPattern,
    PreparedRecordPatternField, PreparedRecordPatternFieldIdentity, PreparedRecordPatternOwner,
    PreparedRecordPatternRest, PreparedRecordPatternSource, PreparedRecordValueSource,
    PreparedStatementFact, PreparedVariantCaseSeed,
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
pub(crate) use semantic_transcript::{TranscriptHasher, write_len};

#[cfg(test)]
#[path = "final_analysis/tests.rs"]
mod tests;
