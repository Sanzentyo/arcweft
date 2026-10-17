//! One borrowed admission of the declaration frames used by the original
//! executable inventory. No declaration frame or expected key seals a task.

use super::{Reader, RuntimeTaskSourceCodecError, RuntimeTaskSourceCodecLimits, Writer, limit};
use crate::plan::control_effect::source_codec::{
    PreparedRuntimeControlEncoding, PreparedRuntimeControlSources, RuntimeControlSourceLimits,
};
use crate::plan::local_declarations::RuntimeLocalDeclarationTableBuilder;
use crate::plan::local_declarations::source_codec::{
    PreparedRuntimeLocalEncoding, PreparedRuntimeLocalSources, RuntimeLocalSourceLimits,
};
use crate::plan::type_table::source_codec::domains::{
    PreparedRuntimeNominalEncoding, PreparedRuntimeNominalSources,
};
use crate::plan::type_table::source_codec::{
    PreparedRuntimeTypeTableEncoding, PreparedRuntimeTypeTableSource, RuntimeTypeSourceLimits,
};
use crate::plan::{
    RuntimeControlEffectContractId, RuntimeControlEffectContractTable, RuntimePlanInventory,
    RuntimePlanTypeTable,
};
use crate::task::semantic::TaskSemanticMeter;

#[derive(Clone, Copy)]
struct DeclarationCounts {
    types: usize,
    records: usize,
    variants: usize,
    locals: usize,
    controls: usize,
}

impl DeclarationCounts {
    fn original(inventory: &RuntimePlanInventory) -> Self {
        Self {
            types: inventory.type_table.len(),
            records: inventory.nominal_record_domains.len(),
            variants: inventory.variant_domains.len(),
            locals: inventory.local_declarations.len(),
            controls: inventory.control_effect_contracts.len(),
        }
    }

    fn type_limits(
        self,
        limits: RuntimeTaskSourceCodecLimits,
    ) -> Result<RuntimeTypeSourceLimits, RuntimeTaskSourceCodecError> {
        Ok(RuntimeTypeSourceLimits {
            rows: u32::try_from(self.types).map_err(|_| RuntimeTaskSourceCodecError::Arithmetic)?,
            elements: limits.declaration_elements,
            encoded_bytes: limits.encoded_bytes,
            decoded_bytes: limits.decoded_bytes,
            policy_depth: limits.type_policy_depth,
        })
    }

    fn local_limits(
        self,
        limits: RuntimeTaskSourceCodecLimits,
    ) -> Result<RuntimeLocalSourceLimits, RuntimeTaskSourceCodecError> {
        Ok(RuntimeLocalSourceLimits {
            rows: u32::try_from(self.locals)
                .map_err(|_| RuntimeTaskSourceCodecError::Arithmetic)?,
            encoded_bytes: limits.encoded_bytes,
            decoded_bytes: limits.decoded_bytes,
            type_count: self.types,
        })
    }

    fn nominal_limits(
        self,
        limits: RuntimeTaskSourceCodecLimits,
    ) -> Result<RuntimeTypeSourceLimits, RuntimeTaskSourceCodecError> {
        let rows = self
            .records
            .checked_add(self.variants)
            .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
        Ok(RuntimeTypeSourceLimits {
            rows: u32::try_from(rows).map_err(|_| RuntimeTaskSourceCodecError::Arithmetic)?,
            ..self.type_limits(limits)?
        })
    }

    fn control_limits(
        self,
        limits: RuntimeTaskSourceCodecLimits,
    ) -> Result<RuntimeControlSourceLimits, RuntimeTaskSourceCodecError> {
        Ok(RuntimeControlSourceLimits {
            rows: u32::try_from(self.controls)
                .map_err(|_| RuntimeTaskSourceCodecError::Arithmetic)?,
            elements: limits.declaration_elements as usize,
            encoded_bytes: limits.encoded_bytes,
            decoded_bytes: limits.decoded_bytes,
            type_count: self.types,
        })
    }
}

pub(super) struct PreparedDeclarationEncoding<'a> {
    types: PreparedRuntimeTypeTableEncoding<'a>,
    nominals: PreparedRuntimeNominalEncoding<'a>,
    locals: PreparedRuntimeLocalEncoding<'a>,
    controls: PreparedRuntimeControlEncoding<'a>,
    storage_bytes: usize,
}

impl<'a> PreparedDeclarationEncoding<'a> {
    pub(super) fn prepare(
        inventory: &'a RuntimePlanInventory,
        limits: RuntimeTaskSourceCodecLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, RuntimeTaskSourceCodecError> {
        let counts = DeclarationCounts::original(inventory);
        let types = inventory
            .type_table
            .prepare_sources_encoding(counts.type_limits(limits)?, meter)?;
        let nominals = inventory
            .prepare_nominal_sources_encoding(counts.nominal_limits(limits)?, meter)
            .map_err(RuntimeTaskSourceCodecError::Nominals)?;
        let locals = inventory
            .local_declarations
            .prepare_sources_encoding(counts.local_limits(limits)?, meter)?;
        let controls = inventory
            .control_effect_contracts
            .prepare_sources_encoding(counts.control_limits(limits)?, meter)?;
        let storage_bytes = total_storage(
            [
                types.storage_bytes(),
                nominals.storage_bytes(),
                locals.storage_bytes(),
                controls.storage_bytes(),
            ],
            limits,
        )?;
        total_elements(
            [types.elements(), nominals.elements(), controls.elements()],
            limits,
        )?;
        Ok(Self {
            types,
            nominals,
            locals,
            controls,
            storage_bytes,
        })
    }

    pub(super) fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }

    pub(super) fn measure(
        &self,
        writer: &mut Writer<'_>,
    ) -> Result<(), RuntimeTaskSourceCodecError> {
        for length in [
            self.types.encoded_bytes(),
            self.nominals.encoded_bytes(),
            self.locals.encoded_bytes(),
            self.controls.encoded_bytes(),
        ] {
            writer.count(length)?;
            writer.extent(length)?;
        }
        Ok(())
    }

    pub(super) fn append(self, writer: &mut Writer<'_>) -> Result<(), RuntimeTaskSourceCodecError> {
        writer.frame(self.types.encoded_bytes(), |output| {
            Ok(self.types.append_to(output)?)
        })?;
        writer.frame(self.nominals.encoded_bytes(), |output| {
            self.nominals
                .append_to(output)
                .map_err(RuntimeTaskSourceCodecError::Nominals)
        })?;
        writer.frame(self.locals.encoded_bytes(), |output| {
            Ok(self.locals.append_to(output)?)
        })?;
        writer.frame(self.controls.encoded_bytes(), |output| {
            Ok(self.controls.append_to(output)?)
        })?;
        Ok(())
    }
}

/// Immutable child extents. Reading the framing retains no decoded declaration.
pub(super) struct DeclarationSourceBytes<'a> {
    types: &'a [u8],
    nominals: &'a [u8],
    locals: &'a [u8],
    controls: &'a [u8],
}

impl<'a> DeclarationSourceBytes<'a> {
    pub(super) fn read(reader: &mut Reader<'a>) -> Result<Self, RuntimeTaskSourceCodecError> {
        let types_length = reader.u32()? as usize;
        let types = reader.slice(types_length)?;
        let nominal_length = reader.u32()? as usize;
        let nominals = reader.slice(nominal_length)?;
        let locals_length = reader.u32()? as usize;
        let locals = reader.slice(locals_length)?;
        let controls_length = reader.u32()? as usize;
        let controls = reader.slice(controls_length)?;
        Ok(Self {
            types,
            nominals,
            locals,
            controls,
        })
    }

    pub(super) fn preflight(
        self,
        inventory: &RuntimePlanInventory,
        limits: RuntimeTaskSourceCodecLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedDeclarationSources<'a>, RuntimeTaskSourceCodecError> {
        let counts = DeclarationCounts::original(inventory);
        let types = RuntimePlanTypeTable::preflight_sources(
            self.types,
            counts.type_limits(limits)?,
            meter,
        )?;
        let nominals = RuntimePlanInventory::preflight_nominal_sources(
            self.nominals,
            counts.types,
            counts.nominal_limits(limits)?,
            meter,
        )
        .map_err(RuntimeTaskSourceCodecError::Nominals)?;
        let locals = RuntimeLocalDeclarationTableBuilder::preflight_sources(
            self.locals,
            counts.local_limits(limits)?,
            meter,
        )?;
        let controls = RuntimeControlEffectContractTable::preflight_sources(
            self.controls,
            counts.control_limits(limits)?,
            meter,
        )?;
        for (table, expected, actual) in [
            ("type", counts.types, types.row_count()),
            ("nominal record", counts.records, nominals.record_count()),
            ("nominal variant", counts.variants, nominals.variant_count()),
            ("local", counts.locals, locals.row_count()),
            ("control", counts.controls, controls.row_count()),
        ] {
            if expected != actual {
                return Err(RuntimeTaskSourceCodecError::DeclarationCount {
                    table,
                    expected,
                    actual,
                });
            }
        }
        let storage_bytes = total_storage(
            [
                types.storage_bytes(),
                nominals.storage_bytes(),
                locals.storage_bytes(),
                controls.storage_bytes(),
            ],
            limits,
        )?;
        total_elements(
            [types.elements(), nominals.elements(), controls.elements()],
            limits,
        )?;
        // The admitted executable still belongs to the original declarations.
        // Reserve the bounded source-comparison pass before retaining tables.
        for bytes in [self.types, self.nominals, self.locals, self.controls] {
            super::charge_passes(meter, bytes.len(), 1)?;
        }
        Ok(PreparedDeclarationSources {
            types,
            nominals,
            locals,
            controls,
            storage_bytes,
        })
    }
}

/// Every child and its combined footprint has been checked. Materialization
/// remains separate so the containing decoder can first inspect all task rows.
pub(super) struct PreparedDeclarationSources<'a> {
    types: PreparedRuntimeTypeTableSource<'a>,
    nominals: PreparedRuntimeNominalSources<'a>,
    locals: PreparedRuntimeLocalSources<'a>,
    controls: PreparedRuntimeControlSources<'a>,
    storage_bytes: usize,
}

impl PreparedDeclarationSources<'_> {
    pub(super) fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }

    pub(super) fn materialize(
        self,
        inventory: &mut RuntimePlanInventory,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), RuntimeTaskSourceCodecError> {
        let types = self.types.materialize(meter)?;
        // These function bodies have already been admitted against the original
        // tables. A source frame cannot reinterpret their existing typed IDs.
        if types != inventory.type_table {
            return Err(RuntimeTaskSourceCodecError::DeclarationMismatch { table: "type" });
        }
        let (records, variants) = self
            .nominals
            .materialize(&types, meter)
            .map_err(RuntimeTaskSourceCodecError::Nominals)?;
        if records != inventory.nominal_record_domains {
            return Err(RuntimeTaskSourceCodecError::DeclarationMismatch {
                table: "nominal record",
            });
        }
        if variants != inventory.variant_domains {
            return Err(RuntimeTaskSourceCodecError::DeclarationMismatch {
                table: "nominal variant",
            });
        }
        let mut locals = self.locals.materialize(&types, meter)?;
        locals.admit_executable_placements(
            inventory.function_sites(),
            inventory.project_call_sites(),
            inventory.pure_helpers(),
            inventory.trait_methods(),
            inventory.line_task_groups(),
            inventory.stream_plans(),
        )?;
        let locals = locals.take_finish()?;
        if locals != inventory.local_declarations {
            return Err(RuntimeTaskSourceCodecError::DeclarationMismatch { table: "local" });
        }
        let controls = self.controls.materialize(&types, meter)?;
        for index in 0..controls.len() {
            let id = RuntimeControlEffectContractId::for_index(index)
                .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
            // Completed proof caches are deliberately excluded: only original
            // source definitions bind the existing executable context.
            if controls.get(id) != inventory.control_effect_contracts.get(id) {
                return Err(RuntimeTaskSourceCodecError::DeclarationMismatch { table: "control" });
            }
        }
        // Replace the owned candidates together. No public inventory or key is
        // exposed if any earlier preflight, type, scope or placement fails.
        inventory.type_table = types;
        inventory.nominal_record_domains = records;
        inventory.variant_domains = variants;
        inventory.local_declarations = locals;
        inventory.control_effect_contracts = controls;
        Ok(())
    }
}

fn total_storage(
    children: [usize; 4],
    limits: RuntimeTaskSourceCodecLimits,
) -> Result<usize, RuntimeTaskSourceCodecError> {
    let total = children
        .into_iter()
        .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
        .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
    limit("decoded bytes", total, limits.decoded_bytes)?;
    Ok(total)
}

fn total_elements(
    children: [usize; 3],
    limits: RuntimeTaskSourceCodecLimits,
) -> Result<(), RuntimeTaskSourceCodecError> {
    let total = children
        .into_iter()
        .try_fold(0usize, |total, count| total.checked_add(count))
        .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
    limit(
        "declaration elements",
        total,
        limits.declaration_elements as usize,
    )
}
