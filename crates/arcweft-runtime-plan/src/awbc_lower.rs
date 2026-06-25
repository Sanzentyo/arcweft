//! `RuntimePlan` -> canonical AWBC lowering.
//!
//! This module is intentionally compiler-side. It depends on `arcweft-core`'s
//! AWBC schema/codec/verifier contract, but `arcweft-core` does not depend on
//! this crate. Product players consume the emitted `AwbcProgram`; they do not
//! need HIR, sema, syntax, compiler, or this lowerer.

mod expr;
mod flow;
mod frame;
mod inventory;
mod line;
mod pattern;
mod source;
#[cfg(test)]
mod tests;

pub use expr::AwbcExprLowerer;
pub use flow::AwbcFlowLowerer;
pub use frame::{FrameBuilder, FrameSlotKey};
pub use inventory::{AwbcInventory, AwbcLowerDiagnostic, AwbcLowerStats};
pub use line::AwbcLineLowerer;
pub use source::AwbcSourceStreamLowerer;

use arcweft_core::awbc::schema::{AwbcProgram, AwbcSourceMapEntry};
use arcweft_core::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext, AwbcVerifyError};
use arcweft_core::plan::RuntimePlan;
use arcweft_render_text::LineDisplayCatalog;

/// Inputs to the compiler-side AWBC lowerer.
#[derive(Clone, Copy, Debug)]
pub struct AwbcLowerInput<'a> {
    pub plan: &'a RuntimePlan,
    pub display: &'a LineDisplayCatalog,
    pub source_label: &'a str,
    pub options: AwbcLowerOptions,
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
#[derive(Clone, Debug)]
pub enum AwbcLowerError {
    Lowering(Vec<AwbcLowerDiagnostic>),
    Verify(AwbcVerifyError),
}

/// Lowers a runtime plan using default options and no display catalog entries.
pub fn lower_runtime_plan_to_awbc(plan: &RuntimePlan) -> Result<AwbcLowerReport, AwbcLowerError> {
    lower_runtime_plan_to_awbc_with_input(AwbcLowerInput {
        plan,
        display: &LineDisplayCatalog::default(),
        source_label: "<runtime-plan>",
        options: AwbcLowerOptions::default(),
    })
}

/// Lowers a runtime plan to canonical AWBC tables.
pub fn lower_runtime_plan_to_awbc_with_input(
    input: AwbcLowerInput<'_>,
) -> Result<AwbcLowerReport, AwbcLowerError> {
    let mut inventory = AwbcInventory::new(input.source_label, input.options);
    inventory.intern_runtime_primitives();
    inventory.intern_display_catalog(input.display);

    let mut diagnostics = {
        let mut flow_lowerer = AwbcFlowLowerer::new(&mut inventory);
        flow_lowerer.lower_plan(input.plan);
        flow_lowerer.into_diagnostics()
    };
    AwbcSourceStreamLowerer::new(&mut inventory).lower_plan(input.plan);

    diagnostics.extend(inventory.take_diagnostics());
    let mut program = inventory.finish();
    if input.options.emit_source_map && program.source_map.is_empty() {
        let source_file = program
            .strings
            .iter()
            .position(|value| value == input.source_label)
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

    if input.options.verify {
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

pub(crate) fn table_index(value: usize) -> u32 {
    u32::try_from(value).expect("AWBC table index exceeded u32 address space")
}

pub(crate) fn table_range_len(start: u32, end: usize) -> u32 {
    table_index(end)
        .checked_sub(start)
        .expect("AWBC table range end precedes start")
}
