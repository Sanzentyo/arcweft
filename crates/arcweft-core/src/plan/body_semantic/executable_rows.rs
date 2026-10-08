//! Source-ordered preparation of the actual executable tables zero through
//! thirteen. The task image adds table fourteen before a final E can finish.
//! No public digest or caller-provided row hash enters this preparation pass.

use super::executable_roles::ExecutableCallableCodeSemantic;
use super::function::ProducerFunctionSemantic;
use super::pure_rows::PureHelperSemantic;
use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::entry::RuntimeCallableExecutableCode;
use crate::plan::construction::task_coordinates::{
    RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanCoordinateOwner,
};
use crate::plan::{RuntimeNominalRecordDomain, RuntimeTaskPlanSealLimits, RuntimeVariantDomain};
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};
use std::num::NonZeroU32;

enum FunctionState<'plan> {
    Unvisited,
    Visiting,
    Done(ProducerFunctionSemantic<'plan>),
}

impl<'plan> FunctionState<'plan> {
    fn proof(&self) -> Option<&ProducerFunctionSemantic<'plan>> {
        match self {
            Self::Done(proof) => Some(proof),
            Self::Unvisited | Self::Visiting => None,
        }
    }
}

#[derive(Clone, Copy)]
enum RowState {
    Unvisited,
    Visiting,
    Done(blake3::Hash),
}

/// The fixed table key is independent of arena IDs and diagnostic labels.
#[derive(Clone, Copy)]
enum ExecutableTable {
    Types,
    Locals,
    Records,
    Variants,
    Functions,
    Content,
    Entries,
    CallableExecutables,
    FlowExecutables,
    Flows,
    Helpers,
    Methods,
    Lines,
    Streams,
}

impl ExecutableTable {
    const ORDER: [Self; 14] = [
        Self::Types,
        Self::Locals,
        Self::Records,
        Self::Variants,
        Self::Functions,
        Self::Content,
        Self::Entries,
        Self::CallableExecutables,
        Self::FlowExecutables,
        Self::Flows,
        Self::Helpers,
        Self::Methods,
        Self::Lines,
        Self::Streams,
    ];

    const fn tag(self) -> u8 {
        match self {
            Self::Types => 0,
            Self::Locals => 1,
            Self::Records => 2,
            Self::Variants => 3,
            Self::Functions => 4,
            Self::Content => 5,
            Self::Entries => 6,
            Self::CallableExecutables => 7,
            Self::FlowExecutables => 8,
            Self::Flows => 9,
            Self::Helpers => 10,
            Self::Methods => 11,
            Self::Lines => 12,
            Self::Streams => 13,
        }
    }
}

/// Borrows the one candidate inventory. Indexes contain references/proofs of
/// those rows, never cloned executable definitions or independently supplied
/// digests. One Visiting/Done store covers every prepared table coordinate.
pub(super) struct RuntimeExecutableSemanticRows<'plan, 'owner> {
    context: RuntimeBodySemanticContext<'plan>,
    task_owner: &'owner RuntimeTaskPlanCoordinateOwner,
    limits: RuntimeTaskPlanSealLimits,
    rows: [Vec<RowState>; 14],
    records: Vec<&'plan RuntimeNominalRecordDomain>,
    variants: Vec<&'plan RuntimeVariantDomain>,
    functions: Vec<FunctionState<'plan>>,
    helpers: Vec<Option<PureHelperSemantic<'plan>>>,
}

impl<'plan, 'owner> RuntimeExecutableSemanticRows<'plan, 'owner> {
    pub(super) fn new(
        plan: &'plan super::RuntimePlanInventory,
        task_owner: &'owner RuntimeTaskPlanCoordinateOwner,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, RuntimeBodySemanticError> {
        let counts = Self::table_counts(plan);
        let mut total = 0;
        for count in counts {
            total = meter.checked_count_sum(total, count)?;
        }
        if total > limits.max_executable_rows as usize {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::ExecutableRows {
                actual: total,
                maximum: limits.max_executable_rows,
            });
        }
        Ok(Self {
            context: RuntimeBodySemanticContext::new(plan),
            task_owner,
            limits,
            rows: counts.map(|count| vec![RowState::Unvisited; count]),
            records: plan.nominal_record_domains().domains().collect(),
            variants: plan.variant_domains().domains().collect(),
            functions: (0..counts[4]).map(|_| FunctionState::Unvisited).collect(),
            helpers: (0..counts[10]).map(|_| None).collect(),
        })
    }

    pub(super) fn table_counts(plan: &super::RuntimePlanInventory) -> [usize; 14] {
        [
            plan.type_table().len(),
            plan.local_declarations().len(),
            plan.nominal_record_domains().len(),
            plan.variant_domains().len(),
            plan.function_sites().len(),
            plan.dialogue_content().rows().len(),
            plan.entries().len(),
            plan.callable_executables().len(),
            plan.flow_executables().len(),
            plan.flows().len(),
            plan.pure_helpers().len(),
            plan.trait_methods().len(),
            plan.line_task_groups().len(),
            plan.stream_plans().len(),
        ]
    }

    /// Appends only the fixed E0..E13 prefix. The caller must append the actual
    /// inline task table before completing the enclosing executable transcript.
    pub(super) fn write_tables(
        &mut self,
        encoder: &mut TaskSemanticEncoder<'_>,
        task_reference: &mut impl FnMut(
            super::flow::RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
    ) -> Result<(), RuntimeBodySemanticError> {
        for table in ExecutableTable::ORDER {
            encoder.status()?;
            encoder.enter_role(); // fixed table header visit
            encoder.tag(table.tag());
            let count = self.rows[table.tag() as usize].len();
            encoder.count(count);
            for ordinal in 0..count {
                encoder.enter_element();
                encoder.count(ordinal);
                encoder.tag(self.kind(table, ordinal));
                let digest = encoder
                    .owner_child(|meter| self.complete(table, ordinal, meter, task_reference))?;
                encoder.digest(digest.as_bytes());
            }
        }
        encoder.status().map_err(Into::into)
    }

    fn kind(&self, table: ExecutableTable, ordinal: usize) -> u8 {
        let plan = self.context.plan;
        match table {
            ExecutableTable::Types => plan
                .type_table()
                .get(RuntimePlanTypeId::from_accepted_ordinal(Self::id(ordinal)))
                .expect("preflighted type row")
                .projection()
                .executable_semantic_kind(),
            ExecutableTable::Records => self.records[ordinal].shape().semantic_tag(),
            ExecutableTable::Functions => plan
                .function_sites()
                .get(RuntimeFunctionSiteId::from_accepted_ordinal(Self::id(
                    ordinal,
                )))
                .expect("preflighted function row")
                .role()
                .semantic_tag(),
            ExecutableTable::Entries => plan.entries()[ordinal].kind.canonical_tag(),
            ExecutableTable::CallableExecutables => {
                plan.callable_executables()[ordinal].code.semantic_tag()
            }
            ExecutableTable::Locals
            | ExecutableTable::Variants
            | ExecutableTable::Content
            | ExecutableTable::FlowExecutables
            | ExecutableTable::Flows
            | ExecutableTable::Helpers
            | ExecutableTable::Methods
            | ExecutableTable::Lines
            | ExecutableTable::Streams => 0,
        }
    }

    fn id(ordinal: usize) -> NonZeroU32 {
        NonZeroU32::new(u32::try_from(ordinal).expect("preflighted u32 row count") + 1)
            .expect("one-based row ordinal")
    }

    fn complete(
        &mut self,
        table: ExecutableTable,
        ordinal: usize,
        meter: &mut TaskSemanticMeter,
        task_reference: &mut impl FnMut(
            super::flow::RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        match self.rows[table.tag() as usize].get(ordinal) {
            Some(RowState::Done(digest)) => return Ok(*digest),
            Some(RowState::Visiting) => {
                meter.reject_owner();
                return Err(RuntimeBodySemanticError::ExecutableCycle {
                    table: table.tag(),
                    ordinal,
                });
            }
            Some(RowState::Unvisited) => {}
            None => {
                meter.reject_owner();
                return Err(RuntimeBodySemanticError::MissingRow {
                    table: "executable rows",
                    ordinal,
                });
            }
        }
        meter.charge_work(1)?; // first executable row visit, before owner work
        self.rows[table.tag() as usize][ordinal] = RowState::Visiting;
        let digest = self
            .complete_owner(table, ordinal, meter, task_reference)
            .inspect_err(|_| meter.reject_owner())?;
        self.rows[table.tag() as usize][ordinal] = RowState::Done(digest);
        Ok(digest)
    }

    fn complete_owner(
        &mut self,
        table: ExecutableTable,
        ordinal: usize,
        meter: &mut TaskSemanticMeter,
        task_reference: &mut impl FnMut(
            super::flow::RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        let plan = self.context.plan;
        match table {
            ExecutableTable::Types => plan
                .type_table()
                .get(RuntimePlanTypeId::from_accepted_ordinal(Self::id(ordinal)))
                .expect("preflighted type row")
                .executable_semantic_row_digest(&self.context, meter),
            ExecutableTable::Locals => self.context.local_row_digest(
                meter,
                RuntimeLocalDeclarationId::from_accepted_ordinal(Self::id(ordinal)),
            ),
            ExecutableTable::Records => {
                self.records[ordinal].executable_semantic_row_digest(&self.context, meter)
            }
            ExecutableTable::Variants => {
                self.variants[ordinal].executable_semantic_row_digest(&self.context, meter)
            }
            ExecutableTable::Functions => {
                self.prepare_function(
                    RuntimeFunctionSiteId::from_accepted_ordinal(Self::id(ordinal)),
                    meter,
                    task_reference,
                )?;
                self.functions[ordinal]
                    .proof()
                    .expect("completed producer function")
                    .executable_row_digest(&self.context, meter)
            }
            ExecutableTable::Content => self.context.dialogue_content_row_digest(meter, ordinal),
            ExecutableTable::Entries => self.context.entry_row_digest(meter, ordinal),
            ExecutableTable::CallableExecutables => {
                self.callable_row_digest(ordinal, meter, task_reference)
            }
            ExecutableTable::FlowExecutables => {
                let index = plan
                    .flows
                    .flow(&plan.flow_executables()[ordinal].flow)
                    .ok_or_else(|| {
                        meter.reject_owner();
                        RuntimeBodySemanticError::InvalidFlowExecutableProducer { ordinal }
                    })?
                    .function_site()
                    .get()
                    .get() as usize
                    - 1;
                self.complete(ExecutableTable::Functions, index, meter, task_reference)?;
                self.context.flow_executable_row_digest(
                    meter,
                    ordinal,
                    self.functions[index]
                        .proof()
                        .expect("completed function proof"),
                )
            }
            ExecutableTable::Flows => {
                let index = plan.flows()[ordinal].function_site().get().get() as usize - 1;
                self.complete(ExecutableTable::Functions, index, meter, task_reference)?;
                self.functions[index]
                    .proof()
                    .expect("completed function proof")
                    .executable_flow_row_digest(&self.context, meter, ordinal)
            }
            ExecutableTable::Helpers => {
                let proof =
                    plan.pure_helpers()[ordinal].executable_semantic(&self.context, meter)?;
                let digest = proof.digest();
                self.helpers[ordinal] = Some(proof);
                Ok(digest)
            }
            ExecutableTable::Methods => {
                plan.trait_methods()[ordinal].executable_semantic_row_digest(&self.context, meter)
            }
            ExecutableTable::Lines => self
                .context
                .line_semantic(
                    meter,
                    crate::runtime_id::RuntimeLineTaskGroupId::from_zero_based(ordinal)
                        .expect("preflighted Line row"),
                    self.task_owner,
                    task_reference,
                    self.limits,
                )?
                .executable_row_digest(&self.context, meter),
            ExecutableTable::Streams => self.context.stream_row_digest(meter, ordinal),
        }
    }

    /// Completes F before a task row's Q/C without completing or emitting E4.
    /// Both task-child resolution and E rows use this same function proof.
    pub(super) fn prepare_function(
        &mut self,
        function: RuntimeFunctionSiteId,
        meter: &mut TaskSemanticMeter,
        task_reference: &mut impl FnMut(
            super::flow::RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
    ) -> Result<&ProducerFunctionSemantic<'plan>, RuntimeBodySemanticError> {
        meter.status()?;
        let ordinal = function.get().get() as usize - 1;
        match self.functions.get(ordinal) {
            Some(FunctionState::Done(_)) => {}
            Some(FunctionState::Visiting) => {
                meter.reject_owner();
                return Err(RuntimeBodySemanticError::ExecutableCycle {
                    table: ExecutableTable::Functions.tag(),
                    ordinal,
                });
            }
            Some(FunctionState::Unvisited) => {
                self.functions[ordinal] = FunctionState::Visiting;
                let proof = self
                    .context
                    .producer_function(
                        meter,
                        function,
                        self.task_owner,
                        task_reference,
                        self.limits,
                    )
                    .inspect_err(|_| meter.reject_owner())?;
                self.functions[ordinal] = FunctionState::Done(proof);
            }
            None => {
                meter.reject_owner();
                return Err(RuntimeBodySemanticError::MissingRow {
                    table: "producer functions",
                    ordinal,
                });
            }
        }
        Ok(self.functions[ordinal].proof().expect("completed F proof"))
    }

    /// Resolves the admitted callable substrate through the common row memo.
    fn callable_row_digest(
        &mut self,
        ordinal: usize,
        meter: &mut TaskSemanticMeter,
        task_reference: &mut impl FnMut(
            super::flow::RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        let plan = self.context.plan;
        let code = &plan.callable_executables()[ordinal].code;
        match code {
            RuntimeCallableExecutableCode::PureHelper(helper) => {
                self.complete(ExecutableTable::Helpers, helper.0, meter, task_reference)?;
                self.context.callable_executable_row_digest(
                    meter,
                    ordinal,
                    ExecutableCallableCodeSemantic::PureHelper(
                        self.helpers[helper.0]
                            .as_ref()
                            .expect("completed helper proof"),
                    ),
                )
            }
            RuntimeCallableExecutableCode::FunctionSite(function) => {
                let index = function.get().get() as usize - 1;
                self.complete(ExecutableTable::Functions, index, meter, task_reference)?;
                self.context.callable_executable_row_digest(
                    meter,
                    ordinal,
                    ExecutableCallableCodeSemantic::FunctionSite(
                        self.functions[index]
                            .proof()
                            .expect("completed function proof"),
                    ),
                )
            }
            RuntimeCallableExecutableCode::ControllerFlow(flow) => {
                let index = plan
                    .flows
                    .flow(flow)
                    .ok_or_else(|| {
                        meter.reject_owner();
                        RuntimeBodySemanticError::InvalidCallableExecutableProducer { ordinal }
                    })?
                    .function_site()
                    .get()
                    .get() as usize
                    - 1;
                self.complete(ExecutableTable::Functions, index, meter, task_reference)?;
                self.context.callable_executable_row_digest(
                    meter,
                    ordinal,
                    ExecutableCallableCodeSemantic::ControllerFlow(
                        self.functions[index]
                            .proof()
                            .expect("completed function proof"),
                    ),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(in crate::plan::body_semantic) fn fixture_prefix(
    plan: &super::RuntimePlanInventory,
) -> blake3::Hash {
    let owner = crate::plan::RuntimePlanBuilder::new().task_coordinate_owner(0);
    let limits = RuntimeTaskPlanSealLimits::default();
    let mut meter = TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes);
    let mut rows = RuntimeExecutableSemanticRows::new(plan, &owner, limits, &mut meter).unwrap();
    let mut encoder = TaskSemanticEncoder::new(b"test-executable-prefix.v1\0", &mut meter);
    rows.write_tables(&mut encoder, &mut |_| panic!("fixture has no task edges"))
        .unwrap();
    encoder.finish().unwrap()
}
