use arcweft_agent_runner::effect_policy::AgentEffectRegistry;
use arcweft_agent_runner::session::{AgentSession, RagService};
use arcweft_compiler::error::CompileAgentError;
use arcweft_compiler::types::CompiledAgentBundle;
use arcweft_core::bytecode::BytecodeVerificationBudget;
use arcweft_debug_model::sink::DebugEventSink;
use arcweft_lang_sema::project_index::ProjectSemanticIndex;

use crate::binding::{
    ReplBindingInvalidation, ReplBindingRecord, ReplBindingSnapshotKind, ReplBindingStatus,
    live_binding_prelude,
};
use crate::cell::{
    CommittedReplCell, ReplBytecodeStats, ReplCellExecutionStatus, ReplCellFilter, ReplCellId,
    ReplCellInput, ReplCellList, ReplCellRecord, ReplEvaluateOutcome, ReplResetOptions,
    ReplResetOutcome, ReplUndoOptions, ReplUndoOutcome,
};
use crate::error::{ReplTransactionError, ReplTransactionPhase};
use crate::evidence::{ReplBindingEvidence, ReplGenerationEvidence, ReplGenerationId};
use crate::hash::hash_parts;
use crate::runtime::{
    ReplCapabilityReport, ReplCapabilitySet, ReplEvaluationRuntime, execute_committed_cell,
};
use crate::source::ParsedReplCell;
use crate::source::classify_repl_cell;
use crate::tier::{
    ReplExecutableCell, ReplExecutableSnapshot, ReplTierCursor, ReplTierInvalidationReason,
    ReplTierInvalidationToken, ReplTierStatusProjection, ReplTierStatusRecord,
};

/// Base project snapshot compiled cells are checked against.
#[derive(Clone, Debug)]
pub struct ReplBaseSnapshot {
    label: String,
    project: ProjectSemanticIndex,
    program_hash: String,
    generation: ReplGenerationId,
}

/// Session configuration for the transaction substrate.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReplSessionOptions {
    pub capabilities: ReplCapabilitySet,
}

/// Public outcome after replacing the base project or active generation.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplBaseChangeOutcome {
    pub dropped_cells: usize,
    pub invalidated_bindings: usize,
    pub evidence: ReplGenerationEvidence,
}

/// Transactional Agent REPL session state.
#[derive(Debug)]
pub struct ReplSession {
    base: ReplBaseSnapshot,
    options: ReplSessionOptions,
    cells: Vec<CommittedReplCell>,
    bindings: Vec<ReplBindingRecord>,
    next_ordinal: u64,
    overlay_hash: String,
    generation: ReplGenerationId,
    invalidations: Vec<ReplTierInvalidationToken>,
    tier_status: Vec<ReplTierStatusRecord>,
}

struct ValidatedReplCell {
    cell_id: ReplCellId,
    parsed: ParsedReplCell,
    compiled: CompiledAgentBundle,
    bytecode_stats: ReplBytecodeStats,
    verified_effects: Vec<String>,
    commit_hash: String,
}

impl ReplBaseSnapshot {
    #[must_use]
    pub fn from_project(label: impl Into<String>, project: ProjectSemanticIndex) -> Self {
        let program_hash = project.program_hash().as_str().to_owned();
        Self {
            label: label.into(),
            project,
            program_hash,
            generation: ReplGenerationId::base(),
        }
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn project(&self) -> &ProjectSemanticIndex {
        &self.project
    }

    #[must_use]
    pub fn program_hash(&self) -> &str {
        &self.program_hash
    }

    #[must_use]
    pub const fn generation(&self) -> ReplGenerationId {
        self.generation
    }
}

impl ReplSession {
    #[must_use]
    pub fn new(base: ReplBaseSnapshot, options: ReplSessionOptions) -> Self {
        let generation = base.generation();
        let overlay_hash = hash_parts(
            "repl.overlay.empty",
            [
                base.program_hash().to_owned(),
                generation.as_u64().to_string(),
            ],
        );
        Self {
            base,
            options,
            cells: Vec::new(),
            bindings: Vec::new(),
            next_ordinal: 0,
            overlay_hash,
            generation,
            invalidations: Vec::new(),
            tier_status: Vec::new(),
        }
    }

    pub fn evaluate_cell<S, D, R>(
        &mut self,
        input: &ReplCellInput,
        runtime: ReplEvaluationRuntime<'_, S, D, R>,
    ) -> Result<ReplEvaluateOutcome, ReplTransactionError>
    where
        S: AgentSession,
        D: DebugEventSink,
        R: RagService,
    {
        let cell_id = ReplCellId::new(self.next_ordinal);
        let validated = self.validate_cell(cell_id, input, &mut *runtime.session)?;
        let committed_index = self.commit_validated_cell(validated);
        let launch_policy = self.options.capabilities.runtime_policy();
        let execution =
            execute_committed_cell(&self.cells[committed_index], runtime, launch_policy);
        self.cells[committed_index].record.execution = execution;
        let execution_failed = self.cells[committed_index].record.execution.status
            == ReplCellExecutionStatus::ExecutionFailed;
        let execution_error = self.cells[committed_index].record.execution.error.clone();
        if execution_failed {
            self.push_invalidation(
                ReplTierInvalidationReason::CellExecutionFailed,
                Some(cell_id),
                execution_error,
            );
        }
        Ok(ReplEvaluateOutcome {
            record: self.cells[committed_index].record.clone(),
            committed: true,
        })
    }

    fn validate_cell<S>(
        &self,
        cell_id: ReplCellId,
        input: &ReplCellInput,
        session: &mut S,
    ) -> Result<ValidatedReplCell, ReplTransactionError>
    where
        S: AgentSession,
    {
        let prelude = live_binding_prelude(&self.bindings);
        let parsed = classify_repl_cell(cell_id, input, &prelude)?;
        let compiled = arcweft_compiler::agent::compile_agent_bundle_with_project(
            parsed.synthetic_source.clone(),
            self.base.project(),
        )
        .map_err(map_compile_error)?;
        compiled
            .bundle
            .bytecode
            .program
            .verify(BytecodeVerificationBudget::default())
            .map_err(|error| ReplTransactionError::Verifier {
                message: error.to_string(),
            })?;
        let launch_policy = self.options.capabilities.runtime_policy();
        AgentEffectRegistry::canonical()
            .authorization_for_artifact(
                &compiled.manifest.verified_effects.inferred,
                &launch_policy,
            )
            .map_err(|error| ReplTransactionError::EffectPolicy {
                message: error.to_string(),
            })?;
        let session_info =
            session
                .info()
                .map_err(|error| ReplTransactionError::ProjectBinding {
                    message: error.to_string(),
                })?;
        if session_info.program_hash != self.base.program_hash {
            return Err(ReplTransactionError::ProjectBinding {
                message: format!(
                    "runtime program hash `{}` does not match REPL base `{}`",
                    session_info.program_hash, self.base.program_hash
                ),
            });
        }
        let stats = ReplBytecodeStats::from(compiled.bundle.bytecode.program.stats());
        let verified_effects = compiled
            .manifest
            .verified_effects
            .inferred
            .iter()
            .map(|effect| effect.as_str().to_owned())
            .collect::<Vec<_>>();
        let commit_hash = hash_parts(
            "repl.cell.commit",
            [
                parsed.source_hash.clone(),
                parsed.synthetic_source_hash.clone(),
                self.base.program_hash.clone(),
                self.generation.as_u64().to_string(),
                verified_effects.join(","),
            ],
        );
        Ok(ValidatedReplCell {
            cell_id,
            parsed,
            compiled,
            bytecode_stats: stats,
            verified_effects,
            commit_hash,
        })
    }

    fn commit_validated_cell(&mut self, validated: ValidatedReplCell) -> usize {
        let bytecode = validated.compiled.bundle.bytecode.program.clone();
        let mut record = ReplCellRecord::new(
            validated.cell_id,
            validated.parsed.kind,
            validated.parsed.source,
            validated.parsed.source_hash,
            validated.parsed.synthetic_source_hash,
            validated.parsed.synthetic_agent_id,
            self.base.program_hash.clone(),
            self.generation,
            validated.commit_hash,
            bytecode.entry_flow.clone(),
            validated.bytecode_stats,
            validated.verified_effects,
            validated.parsed.bindings,
        );
        let bundle = validated.compiled.bundle;
        let committed = CommittedReplCell::new(record.clone(), bytecode, bundle);
        self.cells.push(committed);
        self.bindings.extend(record.bindings.clone());
        self.next_ordinal = self.next_ordinal.saturating_add(1);
        self.overlay_hash = self.compute_overlay_hash();
        record.set_overlay_hash(self.overlay_hash.clone());
        let committed_index = self.cells.len().saturating_sub(1);
        self.cells[committed_index].record = record.clone();
        self.push_invalidation(
            ReplTierInvalidationReason::CellCommitted,
            Some(validated.cell_id),
            Some("pre-commit validation passed".to_owned()),
        );
        committed_index
    }

    #[must_use]
    pub fn cells(&self, filter: ReplCellFilter) -> ReplCellList {
        let cells = self
            .cells
            .iter()
            .map(|cell| &cell.record)
            .filter(|record| {
                filter.include_invalidated
                    || record.execution.status != ReplCellExecutionStatus::Invalidated
            })
            .cloned()
            .collect();
        ReplCellList { cells }
    }

    pub fn undo_latest_cell(
        &mut self,
        _options: ReplUndoOptions,
    ) -> Result<ReplUndoOutcome, ReplTransactionError> {
        let removed = self
            .cells
            .pop()
            .ok_or_else(|| ReplTransactionError::Commit {
                message: "no committed REPL cells to undo".to_owned(),
            })?;
        self.bindings
            .retain(|binding| binding.cell_id != removed.record.id);
        self.overlay_hash = self.compute_overlay_hash();
        self.push_invalidation(
            ReplTierInvalidationReason::CellUndone,
            Some(removed.record.id),
            Some("undo removes committed overlay cell; host effects are not reversed".to_owned()),
        );
        Ok(ReplUndoOutcome {
            removed: removed.record,
            remaining_cells: self.cells.len(),
            overlay_hash: self.overlay_hash.clone(),
        })
    }

    pub fn reset_to_base(&mut self, options: ReplResetOptions) -> ReplResetOutcome {
        let removed_cells = self.cells.len();
        self.cells.clear();
        self.bindings.clear();
        self.next_ordinal = 0;
        if !options.preserve_generation {
            self.generation = self.base.generation();
        }
        self.overlay_hash = self.compute_overlay_hash();
        self.push_invalidation(
            ReplTierInvalidationReason::ResetToBase,
            None,
            Some("reset clears overlay state; host effects are not reversed".to_owned()),
        );
        ReplResetOutcome {
            removed_cells,
            retained_generation: self.generation,
            overlay_hash: self.overlay_hash.clone(),
        }
    }

    #[must_use]
    pub fn generation_evidence(&self) -> ReplGenerationEvidence {
        ReplGenerationEvidence {
            active_generation: self.generation,
            base_program_hash: self.base.program_hash.clone(),
            overlay_hash: self.overlay_hash.clone(),
            committed_cells: self.cells.len(),
            invalidation_events: self.invalidations.len(),
        }
    }

    #[must_use]
    pub fn binding_evidence(&self) -> ReplBindingEvidence {
        ReplBindingEvidence {
            base_program_hash: self.base.program_hash.clone(),
            generation: self.generation,
            bindings: self.bindings.clone(),
        }
    }

    #[must_use]
    pub fn capabilities(&self) -> ReplCapabilityReport {
        self.options.capabilities.report()
    }

    pub fn replace_base_snapshot(&mut self, base: ReplBaseSnapshot) -> ReplBaseChangeOutcome {
        let old_program_hash = self.base.program_hash.clone();
        let old_generation = self.generation;
        self.base = base;
        self.generation = self.generation.next();
        let new_program_hash = self.base.program_hash.clone();
        let invalidated_bindings = self.invalidate_project_bound_bindings(
            "base project snapshot changed",
            &old_program_hash,
            old_generation,
            &new_program_hash,
            self.generation,
        );
        let dropped_cells = self.invalidate_cells();
        self.overlay_hash = self.compute_overlay_hash();
        self.push_invalidation(
            ReplTierInvalidationReason::BaseProjectChanged,
            None,
            Some("project-bound REPL evidence invalidated by base snapshot change".to_owned()),
        );
        ReplBaseChangeOutcome {
            dropped_cells,
            invalidated_bindings,
            evidence: self.generation_evidence(),
        }
    }

    pub fn set_active_generation(&mut self, generation: ReplGenerationId) -> ReplBaseChangeOutcome {
        let old_generation = self.generation;
        self.generation = generation;
        let program_hash = self.base.program_hash.clone();
        let invalidated_bindings = self.invalidate_project_bound_bindings(
            "active generation changed",
            &program_hash,
            old_generation,
            &program_hash,
            generation,
        );
        let dropped_cells = self.invalidate_cells();
        self.overlay_hash = self.compute_overlay_hash();
        self.push_invalidation(
            ReplTierInvalidationReason::GenerationChanged,
            None,
            Some("generation-aware REPL evidence invalidated".to_owned()),
        );
        ReplBaseChangeOutcome {
            dropped_cells,
            invalidated_bindings,
            evidence: self.generation_evidence(),
        }
    }

    #[must_use]
    pub fn executable_snapshot(&self) -> ReplExecutableSnapshot {
        let cells = self
            .cells
            .iter()
            .filter(|cell| cell.record.execution.status == ReplCellExecutionStatus::Executed)
            .map(|cell| ReplExecutableCell {
                cell_id: cell.record.id,
                ordinal: cell.record.ordinal,
                commit_hash: cell.record.commit_hash.clone(),
                source_hash: cell.record.source_hash.clone(),
                synthetic_agent_id: cell.record.synthetic_agent_id.clone(),
                entry_flow: cell.record.entry_flow.clone(),
                bytecode: cell.bytecode.clone(),
            })
            .collect();
        ReplExecutableSnapshot {
            base_program_hash: self.base.program_hash.clone(),
            generation: self.generation,
            overlay_hash: self.overlay_hash.clone(),
            cells,
        }
    }

    #[must_use]
    pub fn tier_invalidation_tokens_since(
        &self,
        cursor: ReplTierCursor,
    ) -> Vec<ReplTierInvalidationToken> {
        self.invalidations
            .iter()
            .filter(|token| token.cursor > cursor)
            .cloned()
            .collect()
    }

    pub fn record_tier_status(&mut self, status: ReplTierStatusRecord) {
        self.tier_status.push(status);
        self.push_invalidation(
            ReplTierInvalidationReason::TierStatusRecorded,
            None,
            Some("tier status projection updated".to_owned()),
        );
    }

    #[must_use]
    pub fn tier_status(&self) -> ReplTierStatusProjection {
        ReplTierStatusProjection {
            records: self.tier_status.clone(),
        }
    }

    fn compute_overlay_hash(&self) -> String {
        hash_parts(
            "repl.overlay",
            std::iter::once(self.base.program_hash.clone())
                .chain(std::iter::once(self.generation.as_u64().to_string()))
                .chain(
                    self.cells
                        .iter()
                        .map(|cell| cell.record.commit_hash.clone()),
                )
                .chain(self.bindings.iter().map(|binding| {
                    format!(
                        "{}:{}:{}",
                        binding.name,
                        binding.snapshot_kind.as_str(),
                        binding.status_label()
                    )
                })),
        )
    }

    fn push_invalidation(
        &mut self,
        reason: ReplTierInvalidationReason,
        cell_id: Option<ReplCellId>,
        detail: Option<String>,
    ) {
        let cursor = self
            .invalidations
            .last()
            .map_or(ReplTierCursor::new(1), |token| token.cursor.next());
        self.invalidations.push(ReplTierInvalidationToken {
            cursor,
            reason,
            generation: self.generation,
            overlay_hash: self.overlay_hash.clone(),
            cell_id,
            detail,
        });
    }

    fn invalidate_cells(&mut self) -> usize {
        self.cells
            .iter_mut()
            .filter(|cell| cell.record.execution.status != ReplCellExecutionStatus::Invalidated)
            .map(|cell| {
                cell.record.mark_invalidated();
                1_usize
            })
            .sum()
    }

    fn invalidate_project_bound_bindings(
        &mut self,
        reason: &str,
        old_program_hash: &str,
        old_generation: ReplGenerationId,
        new_program_hash: &str,
        new_generation: ReplGenerationId,
    ) -> usize {
        self.bindings
            .iter_mut()
            .filter(|binding| {
                binding.status == ReplBindingStatus::Active
                    && (binding.project_bound
                        || binding.snapshot_kind != ReplBindingSnapshotKind::Literal)
            })
            .map(|binding| {
                binding.invalidate(ReplBindingInvalidation {
                    reason: reason.to_owned(),
                    old_program_hash: old_program_hash.to_owned(),
                    new_program_hash: new_program_hash.to_owned(),
                    old_generation,
                    new_generation,
                });
                1_usize
            })
            .sum()
    }
}

impl ReplBindingRecord {
    fn status_label(&self) -> &'static str {
        match self.status {
            ReplBindingStatus::Active => "active",
            ReplBindingStatus::Invalidated => "invalidated",
        }
    }
}

fn map_compile_error(error: CompileAgentError) -> ReplTransactionError {
    match error {
        CompileAgentError::Parse(errors) => ReplTransactionError::Compile {
            phase: ReplTransactionPhase::ClassifyParse,
            message: format!("{errors:?}"),
        },
        CompileAgentError::Hir(errors) => ReplTransactionError::Compile {
            phase: ReplTransactionPhase::HirLowering,
            message: format!("{errors:?}"),
        },
        CompileAgentError::Resolve(errors) => ReplTransactionError::Compile {
            phase: ReplTransactionPhase::SemanticEffectChecks,
            message: format!("{errors:?}"),
        },
        CompileAgentError::Readiness(errors) => ReplTransactionError::Compile {
            phase: ReplTransactionPhase::SemanticEffectChecks,
            message: format!("{errors:?}"),
        },
        CompileAgentError::Type(errors) => ReplTransactionError::Compile {
            phase: ReplTransactionPhase::SemanticEffectChecks,
            message: format!("{errors:?}"),
        },
        other => ReplTransactionError::Compile {
            phase: ReplTransactionPhase::SemanticEffectChecks,
            message: other.to_string(),
        },
    }
}
