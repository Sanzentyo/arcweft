//! Producer function signature, actual body root and ordered endpoint paths.
//! Input transport evidence stays distinct from language capture operations.

use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::plan::construction::task_coordinates::{
    RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanCoordinateOwner,
};
use crate::plan::{self, RuntimeFunctionSiteBody};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

/// Issued only after the complete private producer transcript succeeds.
/// No byte constructor or decoder exists. Upper publication remains Cut 5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProducerFunctionSemanticDigest([u8; 32]);

pub(crate) struct ProducerFunctionSemantic {
    digest: ProducerFunctionSemanticDigest,
    endpoints: Box<[(EndpointKind, blake3::Hash)]>,
}

#[derive(Clone, Copy)]
pub(crate) struct ProducerEndpoint<'a> {
    producer: &'a ProducerFunctionSemantic,
    ordinal: u32,
    kind: EndpointKind,
}

impl ProducerEndpoint<'_> {
    pub(crate) const fn producer_digest(&self) -> ProducerFunctionSemanticDigest {
        self.producer.digest
    }
    pub(crate) const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) const fn kind(&self) -> EndpointKind {
        self.kind
    }
}

impl ProducerFunctionSemantic {
    pub(crate) const fn digest(&self) -> ProducerFunctionSemanticDigest {
        self.digest
    }
    pub(crate) fn endpoint(&self, ordinal: u32) -> Option<ProducerEndpoint<'_>> {
        self.endpoints
            .get(ordinal as usize)
            .map(|(kind, _)| ProducerEndpoint {
                producer: self,
                ordinal,
                kind: *kind,
            })
    }
}
impl ProducerFunctionSemanticDigest {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EndpointKind {
    HostTask,
    MakeNeed,
    AwaitManyBase,
    AwaitManyChild,
    LineTask,
}
impl EndpointKind {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::HostTask => 0,
            Self::AwaitManyBase => 2,
            Self::AwaitManyChild => 3,
            Self::LineTask => 5,
            Self::MakeNeed => 6,
        }
    }
}

#[derive(Clone, Copy)]
enum PathStep {
    Body(plan::RuntimeFlowBodyRole),
    Operation(usize),
    Endpoint(usize),
}

impl RuntimeBodySemanticContext<'_> {
    pub(crate) fn producer_function_digest(
        &self,
        meter: &mut TaskSemanticMeter,
        function: crate::runtime_id::RuntimeFunctionSiteId,
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        task_reference: &mut impl FnMut(
            super::flow::RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
        limits: plan::RuntimeTaskPlanSealLimits,
    ) -> Result<ProducerFunctionSemanticDigest, RuntimeBodySemanticError> {
        self.producer_function(meter, function, task_owner, task_reference, limits)
            .map(|row| row.digest)
    }

    pub(crate) fn producer_function(
        &self,
        meter: &mut TaskSemanticMeter,
        function: crate::runtime_id::RuntimeFunctionSiteId,
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        task_reference: &mut impl FnMut(
            super::flow::RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
        limits: plan::RuntimeTaskPlanSealLimits,
    ) -> Result<ProducerFunctionSemantic, RuntimeBodySemanticError> {
        meter.status()?;
        let Some(site) = self.plan.function_sites().get(function) else {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::MissingRow {
                table: "producer functions",
                ordinal: function.get().get() as usize - 1,
            });
        };
        let endpoints =
            Self::endpoint_count(site.body(), site.inputs().len(), limits.max_function_roles)
                .inspect_err(|_| meter.reject_owner())?;
        let roles = meter.checked_count_sum(site.inputs().len(), endpoints)?;
        if roles > limits.max_function_roles as usize {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::FunctionRoles {
                actual: roles,
                maximum: limits.max_function_roles,
            });
        }
        for (input, binding) in site.inputs().iter().enumerate() {
            if !binding.source().accepts_origin(binding.origin())
                || !binding.source().accepts_transfer(binding.transfer())
            {
                meter.reject_owner();
                return Err(RuntimeBodySemanticError::InvalidFunctionInput { input });
            }
        }
        let body = {
            let mut encoder =
                TaskSemanticEncoder::new(b"arcweft.runtime-plan.body-root.v1\0", meter);
            self.write_function_signature(&mut encoder, function)?;
            match site.body() {
                RuntimeFunctionSiteBody::Expression(body) => {
                    encoder.tag(0);
                    self.write_expression(&mut encoder, body)?;
                }
                RuntimeFunctionSiteBody::Executable(body) => {
                    encoder.tag(1);
                    encoder.count(body.effects().len());
                    for effect in body.effects().iter() {
                        encoder.enter_element();
                        encoder.string(effect.as_str());
                    }
                    self.write_flow(&mut encoder, body.ops(), task_owner, task_reference)?;
                }
            }
            encoder.finish()?
        };
        // Each path is completed before the outer F transcript starts because
        // they borrow the same work/byte meter. No sibling gets its own budget.
        let paths = Self::endpoint_paths(site.body(), meter)?;
        let mut encoder = TaskSemanticEncoder::new(
            b"arcweft.runtime-plan.producer-function-semantic.v1\0",
            meter,
        );
        encoder.digest(site.definition().as_bytes());
        encoder.tag(site.role().semantic_tag());
        encoder.count(site.parameter_inputs().count());
        for (ordinal, input) in site.parameter_inputs().enumerate() {
            encoder.enter_element();
            encoder.enter_role();
            encoder.count(ordinal);
            self.write_type(&mut encoder, input.pattern().ty())?;
            let plan::RuntimeFunctionInputSource::Parameter { passing, .. } = input.source() else {
                unreachable!("parameter iterator selects only immediate parameters")
            };
            encoder.tag(passing.semantic_tag());
        }
        encoder.count(site.capture_inputs().count());
        for (ordinal, input) in site.capture_inputs().enumerate() {
            encoder.enter_element();
            encoder.enter_role();
            encoder.count(ordinal);
            input.origin().encode_semantic_origin(&mut encoder);
            self.write_type(&mut encoder, input.pattern().ty())?;
            input.source().encode_semantic_source(&mut encoder);
            input.transfer().encode_semantic_transfer(&mut encoder);
        }
        self.write_type(&mut encoder, site.result())?;
        encoder.digest(body.as_bytes());
        encoder.count(paths.len());
        for (ordinal, (kind, path)) in paths.iter().enumerate() {
            encoder.enter_element();
            encoder.enter_role();
            encoder.count(ordinal);
            encoder.tag(kind.semantic_tag());
            encoder.digest(path.as_bytes());
        }
        let digest = ProducerFunctionSemanticDigest(*encoder.finish()?.as_bytes());
        Ok(ProducerFunctionSemantic {
            digest,
            endpoints: paths.into_boxed_slice(),
        })
    }

    fn endpoint_count(
        body: &RuntimeFunctionSiteBody,
        inputs: usize,
        maximum: u32,
    ) -> Result<usize, RuntimeBodySemanticError> {
        let RuntimeFunctionSiteBody::Executable(body) = body else {
            return Ok(0);
        };
        let mut count = 0usize;
        plan::flow_ops::try_visit_ops_events(body.ops(), &mut |event| {
            if let plan::flow_ops::RuntimeFlowTreeEvent::EnterOperation { op, .. } = event {
                count = count
                    .checked_add(Self::endpoint_kinds(op).len())
                    .ok_or(crate::task::semantic::TaskSemanticEncodingError::ArithmeticOverflow)?;
                let actual = inputs
                    .checked_add(count)
                    .ok_or(crate::task::semantic::TaskSemanticEncodingError::ArithmeticOverflow)?;
                if actual > maximum as usize {
                    return Err(RuntimeBodySemanticError::FunctionRoles { actual, maximum });
                }
            }
            Ok::<(), RuntimeBodySemanticError>(())
        })?;
        Ok(count)
    }

    fn endpoint_paths(
        body: &RuntimeFunctionSiteBody,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Vec<(EndpointKind, blake3::Hash)>, RuntimeBodySemanticError> {
        let RuntimeFunctionSiteBody::Executable(body) = body else {
            return Ok(vec![]);
        };
        let mut path = Vec::new();
        let mut endpoints = Vec::new();
        plan::flow_ops::try_visit_ops_events(body.ops(), &mut |event| {
            meter.status()?;
            match event {
                plan::flow_ops::RuntimeFlowTreeEvent::EnterBody { role, .. } => {
                    meter.charge_work(1)?;
                    path.push(PathStep::Body(role));
                }
                plan::flow_ops::RuntimeFlowTreeEvent::ExitBody
                | plan::flow_ops::RuntimeFlowTreeEvent::ExitOperation => {
                    path.pop();
                }
                plan::flow_ops::RuntimeFlowTreeEvent::EnterOperation { ordinal, op } => {
                    meter.charge_work(1)?;
                    path.push(PathStep::Operation(ordinal));
                    for (ordinal, kind) in Self::endpoint_kinds(op).iter().copied().enumerate() {
                        meter.charge_work(1)?;
                        path.push(PathStep::Endpoint(ordinal));
                        let mut encoder = TaskSemanticEncoder::new(
                            b"arcweft.runtime-plan.producer-endpoint-path.v1\0",
                            meter,
                        );
                        encoder.count(path.len());
                        for step in &path {
                            encoder.enter_element();
                            encoder.enter_role();
                            match step {
                                PathStep::Body(role) => {
                                    encoder.tag(0);
                                    role.encode_semantic_path(&mut encoder);
                                }
                                PathStep::Operation(ordinal) => {
                                    encoder.tag(1);
                                    encoder.count(*ordinal);
                                }
                                PathStep::Endpoint(ordinal) => {
                                    encoder.tag(2);
                                    encoder.count(*ordinal);
                                }
                            }
                        }
                        endpoints.push((kind, encoder.finish()?));
                        path.pop();
                    }
                }
            }
            Ok::<(), RuntimeBodySemanticError>(())
        })?;
        Ok(endpoints)
    }

    fn endpoint_kinds(op: &plan::FlowOp) -> &'static [EndpointKind] {
        use plan::FlowOp as Op;
        match op {
            Op::HostCall { .. } | Op::Thread { .. } => &[EndpointKind::HostTask],
            Op::StartNeedProducer { .. } => &[EndpointKind::MakeNeed],
            Op::AwaitMany { .. } => &[EndpointKind::AwaitManyBase, EndpointKind::AwaitManyChild],
            Op::Dialogue { .. } => &[EndpointKind::LineTask],
            Op::Bind(_)
            | Op::Let { .. }
            | Op::FormatOperandAttempt { .. }
            | Op::CompleteFormatOperand { .. }
            | Op::LetElse { .. }
            | Op::Assign { .. }
            | Op::LineOperation { .. }
            | Op::CommitDialogueResult { .. }
            | Op::SelectDialogueResult { .. }
            | Op::Choice { .. }
            | Op::Await { .. }
            | Op::ProjectCall { .. }
            | Op::ApplyGroup { .. }
            | Op::If { .. }
            | Op::IfLet { .. }
            | Op::Match { .. }
            | Op::Loop { .. }
            | Op::LoopNext { .. }
            | Op::While { .. }
            | Op::WhileNext { .. }
            | Op::WhileLet { .. }
            | Op::WhileLetNext { .. }
            | Op::For { .. }
            | Op::ForNext { .. }
            | Op::Scope { .. }
            | Op::LetScope { .. }
            | Op::Break(_)
            | Op::Continue
            | Op::Goto(_)
            | Op::GotoExpr(_)
            | Op::Return(_)
            | Op::ReturnExpr(_)
            | Op::Effect(_)
            | Op::EvaluatedEffect(_)
            | Op::RegisterDefer { .. }
            | Op::RegisterCleanup { .. }
            | Op::CancelCleanup { .. }
            | Op::EnterScope { .. }
            | Op::ExitScope
            | Op::CompleteAwaitObserver
            | Op::ExitScopeBind { .. }
            | Op::Noop => &[],
        }
    }
}
