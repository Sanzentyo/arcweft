//! Generation-bound semantic authority for the final arena HIR.
//!
//! The public report in this module is intentionally assembled from typed
//! semantic passes and published only after every live final-HIR owner has one
//! checked fact.  It does not retain a detached syntax tree, source ranges as
//! identities, a linked `HirModule`, or positional sidecar evidence.

use crate::{
    assertion::{AssertionBuildProfile, AssertionContext},
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
        CharacterDialogueCharacterType, CharacterDialogueType, GenericParameterOwnerId,
        GenericTypeParameterId, SemanticTypeDigest, TypeKind, TypeParameterSubstitutions,
    },
};
use arcweft_character::id::CharacterId;
use arcweft_id::{
    DeclarationIdentityFamily, PublicId,
    dialogue::{DialogueLineId, DialogueTextKey},
};
use arcweft_lang_hir::{
    expr::{HirCallArgumentOrdinal, HirExprKind},
    identity::{CaptureId, ExprId, HirModuleId, ItemId, LocalId, PatternId, StmtId, TypeId},
    item::{HirFlowIdentity, HirItemFamily, HirItemKind},
    leaf::{HirIdRef, HirLiteral},
    module::HirModule,
    pattern::HirPatternKind,
    project::HirAnalysisProjectView,
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
mod canonical_literal;
mod error;
mod execution_plan;
mod fx_application;
mod input;
mod match_coverage;
mod match_edges;
mod match_transaction;
mod model;
mod nominal_schema;
mod nominal_semantic;
mod owner_bound_resolution;
mod prepared;
mod recovery_diagnostics;
mod report;
mod semantic_shapes;
mod semantic_transcript;
mod statement_effects;
mod statement_seal;
mod transcript_writer;
mod type_rules;
mod validation;

pub use crate::callable::CharacterDialoguePatchContext;
pub use crate::callable::{
    CallableInstantiationDigest, CheckedCallableJoin, CheckedCallableJoinDigest,
    CheckedCallableJoinError, IntrinsicCallableCandidateTag,
};
pub use crate::checked_compile_time::{
    CheckedCompileTimeScalar, CheckedCompileTimeScalarEnum, CheckedCompileTimeScalarEnumCase,
    CheckedCompileTimeScalarEnumValue, CheckedCompileTimeScalarKind,
};
pub use crate::checked_text_proxy::{
    CheckedTextProxyApplication, CheckedTextProxyApplicationField,
    CheckedTextProxyApplicationMetadata, CheckedTextProxyApplicationSemanticDigest,
    CheckedTextProxyApplicationValue, CheckedTextProxyApplicationView,
    CheckedTextProxyAttributeFamily, CheckedTextProxyAttributeOrigin, CheckedTextProxyCatalog,
    CheckedTextProxyDefinition, CheckedTextProxyDefinitionDigest,
    CheckedTextProxyDefinitionDigestError, CheckedTextProxyDefinitionId,
    CheckedTextProxyFieldDefault, CheckedTextProxyFieldDefinition, CheckedTextProxyMetadataDefault,
    CheckedTextProxyMetadataDefaults, CheckedTextProxyValueOrigin, CompileTimeScalarReductionError,
    TextProxyDeclarationDiagnostic, TextProxyDeclarationDiagnosticCause,
    TextProxyDefaultExpectation, TextProxyMetadataRole,
};
pub(crate) use accounting::{
    CandidateEvaluationPass, CandidateExpectedType, PhysicalArgumentEvaluationKind,
    PhysicalCandidateArgument, PhysicalCandidateArgumentEvaluation,
};
pub use accounting::{FinalSemanticAnalysisControl, FinalSemanticAnalysisWork};
pub use analyzer::{FinalSemanticCatalogs, analyze_final_project};
pub(crate) use analyzer::{
    PreparedEntryIngressSeal, PreparedExecutableIngressFacts, PreparedExecutableIngressSeal,
    PreparedStatementIngressSeal,
};
pub use error::{
    CandidateFactTransactionViolation, FinalCallConstraintFailure, FinalCallFrameInvariant,
    FinalCallSealFailure, FinalCallSealLocation, FinalSemanticAnalysisError,
    FinalSemanticProjectError, RecursiveCallableContractEdge, SemanticFactFamily,
};
pub use fx_application::{
    CheckedContentFxApplication, CheckedContentFxBinding, CheckedFxApplicationOrdinal,
    CheckedFxApplicationSemanticDigest, CheckedFxArgument, CheckedFxBindingDecision, CheckedFxBody,
    CheckedFxBodyCall, CheckedFxConstant, CheckedFxConstructorArgument,
    CheckedFxConstructorArgumentValue, CheckedFxConstructorCall, CheckedFxDefinition,
    CheckedFxDefinitionCatalog, CheckedFxDefinitionCatalogError, CheckedFxDefinitionRef,
    CheckedFxDefinitionSealError, CheckedFxGraphExpression, CheckedFxSourceParameter,
    CheckedFxSymbolicValue, CheckedProjectFxDefinition, CheckedSymbolicFxBinding,
    CheckedViewFxApplication, CheckedViewFxBinding, CheckedViewValueInput, CheckedViewValueProgram,
    CheckedViewValueProgramSealError, SealedFxEdgePlanError,
};
pub(crate) use input::FinalSemanticAnalysisInput;
pub use match_coverage::CheckedMatchLimits;
pub use match_edges::{
    CheckedChildEdgeError, CheckedExpressionChildEdge, CheckedExpressionEdgeError,
    CheckedExpressionEdgeFact, CheckedNestedEvidenceRole, NestedPathEvidence,
};
pub(crate) use model::CheckedImplicitCallableIdentityEvidence;
pub(crate) use model::CheckedMatchRef;
pub use model::{
    CharacterDialogueFieldCoordinate, CheckedAssertionDisposition, CheckedAssignment,
    CheckedAssignmentPlace, CheckedAwait, CheckedAwaitPendingObserver, CheckedBinding,
    CheckedBindingRole, CheckedCaptureAuthorityViolation, CheckedCharacterDialogueFactory,
    CheckedCharacterDialoguePatch, CheckedCharacterDialoguePatchField,
    CheckedCharacterDialogueReconfigure, CheckedCharacterDialogueTarget, CheckedChoice,
    CheckedChoiceGoto, CheckedClosure, CheckedCompileTimeCallee,
    CheckedCompileTimeScalarExpression, CheckedCompileTimeValue, CheckedCompileTimeVector,
    CheckedContentApplication, CheckedContentApplicationEdges, CheckedCoverageDomainDigest,
    CheckedDialogueEffectCapture, CheckedDialogueEffectPlan, CheckedDialogueEffectSite,
    CheckedDialogueEffectSiteOrdinal, CheckedDialogueEffectTrigger, CheckedDropFade,
    CheckedDropFadeOperand, CheckedDropInvocation, CheckedDropPolicySource, CheckedEffectField,
    CheckedEntryReference, CheckedEvaluatedEffect, CheckedEvaluatedEffectOperand,
    CheckedEvaluatedEffectOperation, CheckedEvaluatedEffectRole, CheckedExecutableControlRole,
    CheckedExplicitDropPolicy, CheckedExpression, CheckedExpressionCallCallee,
    CheckedExpressionExecutionPlan, CheckedExpressionRecordField, CheckedExpressionResolution,
    CheckedExpressionResult, CheckedExpressionSemanticDigest, CheckedFieldSelection,
    CheckedFunctionExecution, CheckedImplicitCallable, CheckedImplicitCallableBody,
    CheckedImplicitCallableIdentity, CheckedImplicitCapture, CheckedImplicitCaptureOccurrence,
    CheckedImplicitParameter, CheckedImplicitParameterOccurrence, CheckedIncludeFlowTarget,
    CheckedItem, CheckedItemRole, CheckedIteration, CheckedIteratorFamily, CheckedMatchArmFact,
    CheckedMatchFact, CheckedMatchSemanticDigest, CheckedMethodSelection,
    CheckedNonValueExpressionResult, CheckedOrdinaryFunctionEmission, CheckedPatchOperation,
    CheckedPattern, CheckedPatternResolution, CheckedPatternSemanticDigest, CheckedPipe,
    CheckedPipeBindingIdentity, CheckedPipeLeft, CheckedPipeLeftOccurrence, CheckedProjectCallable,
    CheckedProjectItem, CheckedProjectItemOwner, CheckedProjectNominal, CheckedRecordBindingSource,
    CheckedRecordExpressionSource, CheckedRecordPattern, CheckedRecordPatternField,
    CheckedRecordPatternOwner, CheckedRecordPatternRest, CheckedRecordPatternSource,
    CheckedRecordPatternSourceRef, CheckedRecordValueSource, CheckedRuntimeValueDisposition,
    CheckedScopeIdentity, CheckedSelectBranchHead, CheckedSelectResolution, CheckedSelectStatement,
    CheckedSelectStatementView, CheckedStageLook, CheckedStatement, CheckedStatementPayload,
    CheckedStructuralExecutionReason, CheckedSuspensionRole, CheckedSuspensionStatement,
    CheckedTraitConformance, CheckedTraitIdentity, CheckedTrigger, CheckedTriggerView, CheckedTry,
    CheckedTryBoundary, CheckedTryBoundaryOwner, CheckedTryCallableBoundary, CheckedTryCarrier,
    CheckedTryExpressionBoundary, CheckedTryFunctionSite, CheckedTryOperand,
    CheckedTryOperandAuthorityViolation, CheckedTypeSelection, CheckedTypeValue,
    CheckedTypedBinding, CheckedTypedExpressionResult, CheckedUnsafeAudit, CheckedValueResolution,
    CheckedVariantCase, CheckedVariantOwner, CheckedVariantOwnerError, CheckedVariantOwnerKind,
    CheckedVariantResolution, CheckedViewCall, PostfixBracketResolution, RegisteredSemanticValueId,
};
pub(crate) use nominal_schema::RuntimeNominalProjectionSeal;
pub use nominal_schema::{
    NominalProjectionLimitKind, NominalSchemaPath, NominalSchemaPathStep,
    NominalSchemaProjectionError, RuntimeProjectFieldProjection, RuntimeProjectNominalKind,
    RuntimeProjectNominalProjection, RuntimeProjectVariantCaseProjection,
    project_runtime_type_schema,
};
pub(crate) use nominal_semantic::{
    ProjectNominalSemanticCatalog, ProjectNominalSemanticDefinition,
};
pub(crate) use prepared::{
    PreparedAssignmentStatement, PreparedCompileTimeScalarExpression, PreparedEntryExpression,
    PreparedEntryReference, PreparedEventScrutineeProof, PreparedExpressionFact,
    PreparedExpressionShell, PreparedImplicitCallableBody, PreparedIncludeFlowProof,
    PreparedMethodExpression, PreparedOwnerBoundExpression, PreparedOwnerBoundResolution,
    PreparedPatternFact, PreparedProjectFieldExpression, PreparedProjectNominalTypeValueExpression,
    PreparedProjectRecordExpression, PreparedProjectRecordExpressionField, PreparedRecordPattern,
    PreparedRecordPatternField, PreparedRecordPatternFieldCoordinate, PreparedRecordPatternOwner,
    PreparedRecordPatternRest, PreparedRecordPatternSource, PreparedRecordValueSource,
    PreparedSelectBranchHeadProof, PreparedSelectScrutineeProof, PreparedStatementPayload,
    PreparedStatementScrutineeProof, PreparedTriggerScrutineeProof, PreparedTryBoundary,
    PreparedVariantCaseSeed, PreparedVariantExpression, PreparedVariantOwnerSeed,
    PreparedVariantPattern,
};
pub(crate) use prepared::{
    PreparedContentApplication, PreparedContentEmission, PreparedDialogueApplication,
    PreparedDialogueEffectPlan, PreparedDialogueEffectSite, PreparedEvaluatedEffect,
};
pub use recovery_diagnostics::{
    CallableTailRecoveryDiagnostic, CallableTailRecoveryProjectionError,
    project_callable_tail_recovery_diagnostics,
};
pub use report::{
    CheckedCallExecutionCallee, CheckedExecutableRuntimeExpressionFactFamily,
    CheckedExecutableRuntimeExpressionFactOwner, CheckedExecutableRuntimeFactPartition,
    CheckedExecutableRuntimePatternFactFamily, CheckedExecutableRuntimePatternFactOwner,
    CheckedExecutableRuntimeStatementFactFamily, CheckedExecutableRuntimeStatementFactOwner,
    CheckedExpressionExecution, FinalAnalysisClosureExecution, FinalAnalysisExecutionProjection,
    FinalAnalysisExecutionProjectionError, FinalAnalysisImplicitCallableBody,
    FinalAnalysisImplicitCallableView, FinalAnalysisPipeView, FinalAnalysisTryView,
    FinalSemanticAnalysis,
};
pub(crate) use semantic_shapes::AcceptedSemanticShapeCatalog;
pub(crate) use semantic_transcript::write_len;
pub(crate) use transcript_writer::{
    CheckedTranscriptByteBudget, TranscriptHasher, TranscriptWriteError,
};

#[cfg(test)]
#[path = "final_analysis/tests.rs"]
pub(crate) mod tests;
