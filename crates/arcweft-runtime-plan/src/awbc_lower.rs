//! `RuntimePlan` -> canonical AWBC lowering.
//!
//! This module is intentionally compiler-side. It depends on `arcweft-core`'s
//! AWBC schema/codec/verifier contract, but `arcweft-core` does not depend on
//! this crate. Product players consume the emitted `AwbcProgram`; they do not
//! need HIR, sema, syntax, compiler, or this lowerer.

mod audio;
mod expr;
mod flow;
mod frame;
mod inventory;
mod line;
mod pattern;
#[cfg(test)]
mod tests;
mod trait_method;

pub(crate) use audio::AwbcAudioLowerer;
pub use expr::AwbcExprLowerer;
pub use flow::AwbcFlowLowerer;
pub use frame::{FrameBuilder, FrameSlotKey};
pub use inventory::{AwbcInventory, AwbcLowerDiagnostic, AwbcLowerStats};
pub use line::AwbcLineLowerer;
pub(crate) use trait_method::AwbcTraitMethodLowerer;

use arcweft_core::awbc::schema::{AwbcProgram, AwbcSourceMapEntry};
use arcweft_core::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext, AwbcVerifyError};
use arcweft_core::plan::{EntryRuntimeId, RuntimePlan};
use arcweft_text_model::DialogueContentCatalog;
use thiserror::Error;

/// Compiler-side context for one `RuntimePlan` to AWBC lowering operation.
#[derive(Clone, Copy, Debug)]
pub struct AwbcLowerer<'a> {
    plan: &'a RuntimePlan,
    dialogue_content: &'a DialogueContentCatalog,
    source_label: &'a str,
    entry: Option<&'a EntryRuntimeId>,
    options: AwbcLowerOptions,
}

/// Stable options that affect lowering shape, never runtime semantics.
#[allow(
    clippy::struct_excessive_bools,
    reason = "Lowering feature switches are independent emission flags, not state variants."
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AwbcLowerOptions {
    pub verify: bool,
    pub include_structured_debug_names: bool,
    pub emit_display_map: bool,
    pub emit_source_map: bool,
}

impl Default for AwbcLowerOptions {
    fn default() -> Self {
        Self {
            verify: true,
            include_structured_debug_names: true,
            emit_display_map: true,
            emit_source_map: true,
        }
    }
}

/// Lowering output plus deterministic inventory/debug evidence.
#[derive(Clone, Debug)]
pub struct AwbcLowerReport {
    pub program: AwbcProgram,
    pub stats: AwbcLowerStats,
    pub diagnostics: Vec<AwbcLowerDiagnostic>,
}

/// Top-level failure. Diagnostics remain structured and reusable by CLI/LSP.
#[derive(Clone, Debug, Error)]
pub enum AwbcLowerError {
    #[error("Product AWBC lowering reported structured diagnostics: {0:?}")]
    Lowering(Vec<AwbcLowerDiagnostic>),
    #[error(transparent)]
    Verify(AwbcVerifyError),
}

impl<'a> AwbcLowerer<'a> {
    /// Creates a lowerer with the stable default emission options.
    pub fn new(
        plan: &'a RuntimePlan,
        dialogue_content: &'a DialogueContentCatalog,
        source_label: &'a str,
    ) -> Self {
        Self {
            plan,
            dialogue_content,
            source_label,
            entry: None,
            options: AwbcLowerOptions::default(),
        }
    }

    /// Creates a lowerer for one selected entry and its executable Flow closure.
    ///
    /// Selection is resolved against the complete accepted plan. The lowerer
    /// never reconstructs a partial `RuntimePlan`, so plan-local identities
    /// retain their original owner while Product AWBC tables are allocated
    /// afresh for the selected artifact.
    pub fn for_entry(
        plan: &'a RuntimePlan,
        dialogue_content: &'a DialogueContentCatalog,
        source_label: &'a str,
        entry: &'a EntryRuntimeId,
    ) -> Self {
        Self {
            plan,
            dialogue_content,
            source_label,
            entry: Some(entry),
            options: AwbcLowerOptions::default(),
        }
    }

    /// Replaces the emission options for this lowering operation.
    #[must_use]
    pub const fn with_options(mut self, options: AwbcLowerOptions) -> Self {
        self.options = options;
        self
    }

    /// Lowers the configured runtime plan to canonical AWBC tables.
    pub fn lower(self) -> Result<AwbcLowerReport, AwbcLowerError> {
        let Self {
            plan,
            dialogue_content,
            source_label,
            entry,
            options,
        } = self;
        let mut inventory = AwbcInventory::new(source_label, options);
        inventory.intern_runtime_primitives();
        inventory.intern_dialogue_content_catalog(dialogue_content);

        let mut diagnostics = {
            let mut flow_lowerer = AwbcFlowLowerer::new(&mut inventory, plan);
            if let Some(entry) = entry {
                flow_lowerer.lower_entry_plan(entry);
            } else {
                flow_lowerer.lower_plan();
            }
            flow_lowerer.into_diagnostics()
        };
        expr::lower_pending_closures(&mut inventory, plan);

        diagnostics.extend(inventory.take_diagnostics());
        let mut program = inventory.finish();
        if options.emit_source_map && program.source_map.is_empty() {
            let source_file = program
                .strings
                .iter()
                .position(|value| value == source_label)
                .map(|index| arcweft_core::awbc::schema::AwbcStringId(table_index(index)))
                .unwrap_or_default();
            program.source_map.push(AwbcSourceMapEntry {
                location: arcweft_core::awbc::schema::AwbcCodeLocation::Block(
                    arcweft_core::awbc::schema::AwbcBlockId(0),
                ),
                source_file,
                start: 0,
                end: 0,
                anchor: None,
            });
        }
        program.canonicalize_string_table();

        if options.verify {
            program
                .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
                .map_err(AwbcLowerError::Verify)?;
        }
        let stats = AwbcLowerStats::from_program(&program);
        if diagnostics.iter().any(AwbcLowerDiagnostic::is_error) {
            return Err(AwbcLowerError::Lowering(diagnostics));
        }
        Ok(AwbcLowerReport {
            program,
            stats,
            diagnostics,
        })
    }
}

pub(crate) fn table_index(value: usize) -> u32 {
    u32::try_from(value).expect("AWBC table index exceeded u32 address space")
}

pub(crate) fn table_range_len(start: u32, end: usize) -> u32 {
    table_index(end)
        .checked_sub(start)
        .expect("AWBC table range end precedes start")
}
