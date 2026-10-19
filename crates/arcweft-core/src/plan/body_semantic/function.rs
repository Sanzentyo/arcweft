//! Producer function signature, actual body root and ordered endpoint paths.
//! Input transport evidence stays distinct from language capture operations.

use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::plan::construction::task_coordinates::RuntimeTaskPlanCoordinateOwner;
use crate::plan::{self, RuntimeFunctionSiteBody};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError, TaskSemanticMeter};

/// Issued only after the complete private producer transcript succeeds.
/// No byte constructor or decoder exists. Upper publication remains Cut 5.
///
/// ```compile_fail
/// use arcweft_core::plan::ProducerFunctionSemanticDigest;
/// let _ = ProducerFunctionSemanticDigest::from_bytes([0; 32]);
/// ```
/// ```compile_fail
/// fn encode<T: serde::Serialize>() {}
/// encode::<arcweft_core::plan::ProducerFunctionSemanticDigest>();
/// ```
/// ```compile_fail
/// fn decode<T: serde::de::DeserializeOwned>() {}
/// decode::<arcweft_core::plan::ProducerFunctionSemanticDigest>();
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProducerFunctionSemanticDigest([u8; 32]);

pub(crate) struct ProducerFunctionSemantic<'a> {
    digest: ProducerFunctionSemanticDigest,
    plan: &'a super::RuntimePlanInventory,
    function: &'a plan::RuntimeFunctionSite,
    function_id: crate::runtime_id::RuntimeFunctionSiteId,
    body_root: blake3::Hash,
    endpoints: Box<[(EndpointKind, blake3::Hash, &'a plan::FlowOp)]>,
}

#[derive(Clone, Copy)]
pub(crate) struct ProducerEndpoint<'a> {
    digest: ProducerFunctionSemanticDigest,
    request: OriginalRequestEndpoint<'a>,
}

/// Original source metadata carries no completed F or Q proof.
#[derive(Clone, Copy)]
pub(crate) struct OriginalRequestEndpoint<'a> {
    owner: OriginalRequestOwner<'a>,
    function_id: crate::runtime_id::RuntimeFunctionSiteId,
    source: ProducerEndpointSource<'a>,
    ordinal: u32,
    kind: EndpointKind,
}

#[derive(Clone, Copy)]
enum OriginalRequestOwner<'a> {
    Inventory(&'a super::RuntimePlanInventory),
    Construction(&'a plan::RuntimePlanBuilder),
}

/// A borrowed, already defined function in its actual aggregate owner.
#[derive(Clone, Copy)]
pub(in crate::plan) struct OriginalFunctionSource<'a> {
    pub(in crate::plan) function: crate::runtime_id::RuntimeFunctionSiteId,
    pub(in crate::plan) body: &'a RuntimeFunctionSiteBody,
    pub(in crate::plan) inputs: &'a [plan::RuntimeFunctionInputBinding],
}

#[derive(Clone, Copy)]
enum ProducerEndpointSource<'a> {
    FunctionRoot(&'a [plan::RuntimeFunctionInputBinding]),
    Operation(&'a plan::FlowOp),
}

impl ProducerEndpoint<'_> {
    pub(crate) const fn producer_digest(&self) -> ProducerFunctionSemanticDigest {
        self.digest
    }
}

impl<'a> std::ops::Deref for ProducerEndpoint<'a> {
    type Target = OriginalRequestEndpoint<'a>;

    fn deref(&self) -> &Self::Target {
        &self.request
    }
}

impl<'a> From<ProducerEndpoint<'a>> for OriginalRequestEndpoint<'a> {
    fn from(endpoint: ProducerEndpoint<'a>) -> Self {
        endpoint.request
    }
}

impl<'a> OriginalRequestEndpoint<'a> {
    pub(in crate::plan) fn from_construction_root(
        builder: &'a plan::RuntimePlanBuilder,
        source: OriginalFunctionSource<'a>,
        meter: &mut TaskSemanticMeter,
        limits: plan::RuntimeTaskPlanSealLimits,
    ) -> Result<Self, RuntimeBodySemanticError> {
        Self::preflight_function_source(&source, meter, limits)?;
        Ok(Self {
            owner: OriginalRequestOwner::Construction(builder),
            function_id: source.function,
            source: ProducerEndpointSource::FunctionRoot(source.inputs),
            ordinal: 0,
            kind: EndpointKind::FunctionRoot,
        })
    }

    fn preflight_function_source(
        source: &OriginalFunctionSource<'_>,
        meter: &mut TaskSemanticMeter,
        limits: plan::RuntimeTaskPlanSealLimits,
    ) -> Result<(), RuntimeBodySemanticError> {
        meter.status()?;
        let endpoints = RuntimeBodySemanticContext::endpoint_count(
            source.body,
            source.inputs.len(),
            limits.max_function_roles,
        )?;
        let roles = meter.checked_count_sum(source.inputs.len(), endpoints)?;
        if roles > limits.max_function_roles as usize {
            return Err(RuntimeBodySemanticError::FunctionRoles {
                actual: roles,
                maximum: limits.max_function_roles,
            });
        }
        Ok(())
    }

    pub(in crate::plan) fn visit_construction_tasks(
        builder: &'a plan::RuntimePlanBuilder,
        source: OriginalFunctionSource<'a>,
        meter: &mut TaskSemanticMeter,
        limits: plan::RuntimeTaskPlanSealLimits,
        visitor: &mut impl FnMut(Self, &mut TaskSemanticMeter) -> Result<(), RuntimeBodySemanticError>,
    ) -> Result<(), RuntimeBodySemanticError> {
        Self::visit_task_sources(
            OriginalRequestOwner::Construction(builder),
            source,
            meter,
            limits,
            visitor,
        )
    }

    fn visit_task_sources(
        owner: OriginalRequestOwner<'a>,
        source: OriginalFunctionSource<'a>,
        meter: &mut TaskSemanticMeter,
        limits: plan::RuntimeTaskPlanSealLimits,
        visitor: &mut impl FnMut(Self, &mut TaskSemanticMeter) -> Result<(), RuntimeBodySemanticError>,
    ) -> Result<(), RuntimeBodySemanticError> {
        Self::visit_endpoints(owner, source, meter, limits, &mut |endpoint, meter| {
            if endpoint.task_source().is_some() {
                visitor(endpoint, meter)?;
            }
            Ok(())
        })
    }

    fn visit_endpoints(
        owner: OriginalRequestOwner<'a>,
        source: OriginalFunctionSource<'a>,
        meter: &mut TaskSemanticMeter,
        limits: plan::RuntimeTaskPlanSealLimits,
        visitor: &mut impl FnMut(Self, &mut TaskSemanticMeter) -> Result<(), RuntimeBodySemanticError>,
    ) -> Result<(), RuntimeBodySemanticError> {
        Self::preflight_function_source(&source, meter, limits)?;
        let RuntimeFunctionSiteBody::Executable(body) = source.body else {
            return Ok(());
        };
        let mut ordinal = 0u32;
        plan::flow_ops::try_visit_ops_events(body.ops(), &mut |event| {
            meter.charge_work(1)?;
            if let plan::flow_ops::RuntimeFlowTreeEvent::EnterOperation { op, .. } = event {
                for kind in RuntimeBodySemanticContext::endpoint_kinds(op) {
                    meter.charge_work(1)?;
                    let endpoint = Self {
                        owner,
                        function_id: source.function,
                        source: ProducerEndpointSource::Operation(op),
                        ordinal,
                        kind: *kind,
                    };
                    visitor(endpoint, meter)?;
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or(TaskSemanticEncodingError::ArithmeticOverflow)?;
                }
            }
            Ok(())
        })
    }

    pub(in crate::plan) fn task_semantic_binding(
        &self,
    ) -> plan::RuntimeTaskSemanticBinding<plan::RuntimeLineTaskGroupSeedId> {
        use plan::RuntimeTaskSemanticBinding as Binding;
        match self.kind {
            EndpointKind::AwaitManyBase => Binding::AwaitManyBase,
            EndpointKind::AwaitManyChild => Binding::AwaitManyChild,
            EndpointKind::FunctionRoot
            | EndpointKind::HostTask
            | EndpointKind::MakeNeed
            | EndpointKind::LineTask => Binding::Ordinary,
        }
    }

    pub(in crate::plan) fn control_child_coordinate(
        &self,
    ) -> Option<&'a plan::RuntimeTaskPlanBuildCoordinate> {
        match (self.kind, self.source) {
            (
                EndpointKind::AwaitManyBase,
                ProducerEndpointSource::Operation(plan::FlowOp::AwaitMany { target, .. }),
            ) => Some(&target.child.plan),
            _ => None,
        }
    }

    pub(crate) fn task_source(&self) -> Option<super::flow::RuntimeBodyTaskSource<'a>> {
        use super::flow::RuntimeBodyTaskSource as Source;
        let ProducerEndpointSource::Operation(operation) = self.source else {
            return None;
        };
        match self.kind {
            EndpointKind::HostTask => match operation {
                plan::FlowOp::HostCall { target, .. } => Some(Source::Host(target)),
                plan::FlowOp::Thread { producer, .. } => Some(Source::Producer(producer)),
                _ => None,
            },
            EndpointKind::MakeNeed => match operation {
                plan::FlowOp::StartNeedProducer { target, .. } => Some(Source::Start(target)),
                _ => None,
            },
            EndpointKind::AwaitManyBase | EndpointKind::AwaitManyChild => match operation {
                plan::FlowOp::AwaitMany { target, .. } => Some(Source::Producer(
                    if self.kind == EndpointKind::AwaitManyBase {
                        &target.base
                    } else {
                        &target.child
                    },
                )),
                _ => None,
            },
            EndpointKind::LineTask | EndpointKind::FunctionRoot => None,
        }
    }

    pub(crate) const fn request_source(&self) -> super::request::RuntimeTaskRequestSource {
        match self.source {
            ProducerEndpointSource::FunctionRoot(_) => {
                super::request::RuntimeTaskRequestSource::FunctionRoot
            }
            ProducerEndpointSource::Operation(_) => {
                super::request::RuntimeTaskRequestSource::Operation(self.ordinal)
            }
        }
    }

    pub(crate) fn function_inputs(&self) -> Option<&'a [plan::RuntimeFunctionInputBinding]> {
        match self.source {
            ProducerEndpointSource::FunctionRoot(inputs) => Some(inputs),
            ProducerEndpointSource::Operation(_) => None,
        }
    }
    pub(crate) const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) const fn kind(&self) -> EndpointKind {
        self.kind
    }
    pub(crate) fn belongs_to(&self, plan: &super::RuntimePlanInventory) -> bool {
        matches!(self.owner, OriginalRequestOwner::Inventory(original) if std::ptr::eq(original, plan))
    }
    pub(super) fn source_type_is_need(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
    ) -> Result<bool, RuntimeBodySemanticError> {
        match self.owner {
            OriginalRequestOwner::Inventory(plan) => plan
                .type_table()
                .get(ty)
                .map(|definition| {
                    matches!(
                        definition.projection(),
                        plan::RuntimePlanTypeProjection::Need(_)
                    )
                })
                .ok_or(RuntimeBodySemanticError::UnknownType { ty }),
            OriginalRequestOwner::Construction(builder) => builder.source_type_is_need(ty),
        }
    }
    pub(super) fn source_local_is_capture(
        &self,
        local: crate::runtime_id::RuntimeLocalDeclarationId,
    ) -> Result<bool, RuntimeBodySemanticError> {
        match self.owner {
            OriginalRequestOwner::Inventory(plan) => plan
                .local_declarations()
                .get(local)
                .map(|declaration| {
                    declaration
                        .placement()
                        .function_capture_position(self.function_id)
                        .is_some()
                })
                .ok_or(RuntimeBodySemanticError::UnknownLocal { local }),
            OriginalRequestOwner::Construction(builder) => {
                builder.source_local_is_capture(self.function_id, local)
            }
        }
    }
    #[cfg(test)]
    pub(crate) fn host_target(&self) -> Option<&'a plan::RuntimeHostCallTarget> {
        if let ProducerEndpointSource::Operation(plan::FlowOp::HostCall { target, .. }) =
            self.source
        {
            Some(target)
        } else {
            None
        }
    }
    pub(crate) fn host_arguments(&self) -> Option<&'a [crate::task::RuntimeHostArgumentTemplate]> {
        let ProducerEndpointSource::Operation(operation) = self.source else {
            return None;
        };
        match operation {
            plan::FlowOp::HostCall { target, .. } => Some(&target.args),
            plan::FlowOp::StartNeedProducer { target, .. } => Some(target.arguments()),
            plan::FlowOp::Thread { producer, .. } => Some(&producer.request.args),
            plan::FlowOp::AwaitMany { target, .. } => match self.kind {
                EndpointKind::AwaitManyBase => Some(&target.base.request.args),
                EndpointKind::AwaitManyChild => Some(&target.child.request.args),
                _ => None,
            },
            _ => None,
        }
    }
    pub(crate) const fn function_id(&self) -> crate::runtime_id::RuntimeFunctionSiteId {
        self.function_id
    }
}

impl ProducerFunctionSemantic<'_> {
    pub(super) fn matches_function(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        function: crate::runtime_id::RuntimeFunctionSiteId,
    ) -> bool {
        std::ptr::eq(self.plan, context.plan) && self.function_id == function
    }

    pub(super) fn matches_flow(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        flow: &plan::FlowRuntimeId,
    ) -> bool {
        std::ptr::eq(self.plan, context.plan)
            && context.plan.flows.flow(flow).is_some_and(|row| {
                self.function_id == row.function_site()
                    && std::ptr::eq(self.function, row.function())
                    && self.function.role() == plan::RuntimeFunctionSemanticRole::Flow
            })
    }

    /// Resolves the actual Flow row in constant time. Its accepted runtime
    /// path is semantic identity; its public label also binds dynamic target
    /// selection. Function/parameter indices are validated before reuse.
    pub(crate) fn executable_flow_row_digest(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        meter: &mut TaskSemanticMeter,
        ordinal: usize,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        if !std::ptr::eq(self.plan, context.plan) {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::ForeignFunctionTranscript);
        }
        let row = context.plan.flows().get(ordinal).ok_or_else(|| {
            meter.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "flows",
                ordinal,
            }
        })?;
        if self.function_id != row.function_site()
            || !std::ptr::eq(self.function, row.function())
            || self.function.role() != plan::RuntimeFunctionSemanticRole::Flow
        {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::InvalidFlowProducer { ordinal });
        }
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-row.v1\0", meter);
        encoder.tag(9);
        encoder.tag(0);
        encoder.digest(row.definition().as_bytes());
        row.id.write_executable_identity(&mut encoder);
        encoder.count(row.params.len());
        for (ordinal, parameter) in row.params.iter().enumerate() {
            encoder.enter_element();
            encoder.count(ordinal);
            context.write_local(&mut encoder, *parameter)?;
        }
        encoder.digest(self.body_root.as_bytes());
        encoder.finish().map_err(Into::into)
    }

    /// The executable row reuses this actual completed function transcript;
    /// its body and endpoint paths are never reconstructed or walked again.
    pub(crate) fn executable_row_digest(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        meter: &mut TaskSemanticMeter,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        if !std::ptr::eq(self.plan, context.plan) {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::ForeignFunctionTranscript);
        }
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-row.v1\0", meter);
        encoder.tag(4);
        encoder.tag(self.function.role().semantic_tag());
        context.write_function_signature(&mut encoder, self.function_id)?;
        encoder.digest(self.body_root.as_bytes());
        encoder.count(self.endpoints.len());
        for (ordinal, (kind, path, _)) in self.endpoints.iter().enumerate() {
            encoder.enter_element();
            encoder.enter_role();
            encoder.count(ordinal);
            encoder.tag(kind.semantic_tag());
            encoder.digest(path.as_bytes());
        }
        encoder.finish().map_err(Into::into)
    }

    pub(crate) const fn digest(&self) -> ProducerFunctionSemanticDigest {
        self.digest
    }
    pub(crate) fn request_endpoint(
        &self,
        source: super::request::RuntimeTaskRequestSource,
    ) -> Option<ProducerEndpoint<'_>> {
        match source {
            super::request::RuntimeTaskRequestSource::Operation(ordinal) => self.endpoint(ordinal),
            super::request::RuntimeTaskRequestSource::FunctionRoot => Some(ProducerEndpoint {
                digest: self.digest,
                request: OriginalRequestEndpoint {
                    owner: OriginalRequestOwner::Inventory(self.plan),
                    function_id: self.function_id,
                    source: ProducerEndpointSource::FunctionRoot(self.function.inputs()),
                    ordinal: 0,
                    kind: EndpointKind::FunctionRoot,
                },
            }),
        }
    }
    pub(crate) fn endpoint(&self, ordinal: u32) -> Option<ProducerEndpoint<'_>> {
        self.endpoints
            .get(ordinal as usize)
            .map(|(kind, _, operation)| ProducerEndpoint {
                digest: self.digest,
                request: OriginalRequestEndpoint {
                    owner: OriginalRequestOwner::Inventory(self.plan),
                    function_id: self.function_id,
                    source: ProducerEndpointSource::Operation(operation),
                    ordinal,
                    kind: *kind,
                },
            })
    }
}
impl ProducerFunctionSemanticDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EndpointKind {
    FunctionRoot,
    HostTask,
    MakeNeed,
    AwaitManyBase,
    AwaitManyChild,
    LineTask,
}
impl EndpointKind {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::FunctionRoot => 7,
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

impl<'a> OriginalRequestEndpoint<'a> {
    fn find_source(
        owner: OriginalRequestOwner<'a>,
        source: OriginalFunctionSource<'a>,
        coordinate: &crate::plan::RuntimeTaskPlanBuildCoordinate,
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        meter: &mut TaskSemanticMeter,
        limits: plan::RuntimeTaskPlanSealLimits,
    ) -> Result<Self, RuntimeBodySemanticError> {
        let function = source.function;
        let result = (|| {
            meter.status()?;
            if !task_owner.contains(coordinate) {
                return Err(RuntimeBodySemanticError::ForeignTaskCoordinate);
            }
            let mut found = None;
            Self::visit_task_sources(owner, source, meter, limits, &mut |endpoint, _| {
                if endpoint
                    .task_source()
                    .is_some_and(|source| source.coordinate() == coordinate)
                {
                    if found.is_some() {
                        return Err(RuntimeBodySemanticError::AmbiguousOriginalRequestEndpoint {
                            function,
                        });
                    }
                    found = Some(endpoint);
                }
                Ok::<(), RuntimeBodySemanticError>(())
            })?;
            found.ok_or(RuntimeBodySemanticError::MissingOriginalRequestEndpoint { function })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    pub(in crate::plan) fn from_construction(
        builder: &'a plan::RuntimePlanBuilder,
        source: OriginalFunctionSource<'a>,
        coordinate: &crate::plan::RuntimeTaskPlanBuildCoordinate,
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        meter: &mut TaskSemanticMeter,
        limits: plan::RuntimeTaskPlanSealLimits,
    ) -> Result<Self, RuntimeBodySemanticError> {
        Self::find_source(
            OriginalRequestOwner::Construction(builder),
            source,
            coordinate,
            task_owner,
            meter,
            limits,
        )
    }
}

impl<'a> RuntimeBodySemanticContext<'a> {
    /// Borrows the request grammar from its actual immutable function. Sealing
    /// already proved F; source lookup does not hash or issue that proof again.
    pub(in crate::plan) fn original_request_endpoint(
        &self,
        function: crate::runtime_id::RuntimeFunctionSiteId,
        request: super::request::RuntimeTaskRequestSource,
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        meter: &mut TaskSemanticMeter,
        limits: plan::RuntimeTaskPlanSealLimits,
    ) -> Result<Option<OriginalRequestEndpoint<'a>>, RuntimeBodySemanticError> {
        meter.charge_work(1)?;
        let result = (|| {
            let site = self.plan.function_sites().get(function).ok_or(
                RuntimeBodySemanticError::MissingRow {
                    table: "function sites",
                    ordinal: function.get().get() as usize - 1,
                },
            )?;
            let source = OriginalFunctionSource {
                function,
                body: site.body(),
                inputs: site.inputs(),
            };
            let owner = OriginalRequestOwner::Inventory(self.plan);
            match request {
                super::request::RuntimeTaskRequestSource::FunctionRoot => {
                    OriginalRequestEndpoint::preflight_function_source(&source, meter, limits)?;
                    Ok(Some(OriginalRequestEndpoint {
                        owner,
                        function_id: function,
                        source: ProducerEndpointSource::FunctionRoot(source.inputs),
                        ordinal: 0,
                        kind: EndpointKind::FunctionRoot,
                    }))
                }
                super::request::RuntimeTaskRequestSource::Operation(ordinal) => {
                    let mut found = None;
                    OriginalRequestEndpoint::visit_endpoints(
                        owner,
                        source,
                        meter,
                        limits,
                        &mut |endpoint, _| {
                            if endpoint
                                .task_source()
                                .is_some_and(|source| !task_owner.contains(source.coordinate()))
                            {
                                return Err(RuntimeBodySemanticError::ForeignTaskCoordinate);
                            }
                            if endpoint.ordinal == ordinal {
                                found = Some(endpoint);
                            }
                            Ok(())
                        },
                    )?;
                    Ok(found)
                }
            }
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    /// Projects metadata from one exact original source reference. This does
    /// not hash a body or issue a completed F/Q digest.
    #[cfg(test)]
    pub(in crate::plan) fn original_task_endpoint(
        &self,
        meter: &mut TaskSemanticMeter,
        function: crate::runtime_id::RuntimeFunctionSiteId,
        coordinate: &crate::plan::RuntimeTaskPlanBuildCoordinate,
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        limits: plan::RuntimeTaskPlanSealLimits,
    ) -> Result<OriginalRequestEndpoint<'a>, RuntimeBodySemanticError> {
        meter.status()?;
        if !task_owner.contains(coordinate) {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::ForeignTaskCoordinate);
        }
        let site = self.plan.function_sites().get(function).ok_or_else(|| {
            meter.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "function sites",
                ordinal: function.get().get() as usize - 1,
            }
        })?;
        OriginalRequestEndpoint::find_source(
            OriginalRequestOwner::Inventory(self.plan),
            OriginalFunctionSource {
                function,
                body: site.body(),
                inputs: site.inputs(),
            },
            coordinate,
            task_owner,
            meter,
            limits,
        )
    }

    #[cfg(test)]
    pub(crate) fn producer_function_digest(
        &self,
        meter: &mut TaskSemanticMeter,
        function: crate::runtime_id::RuntimeFunctionSiteId,
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        limits: plan::RuntimeTaskPlanSealLimits,
    ) -> Result<ProducerFunctionSemanticDigest, RuntimeBodySemanticError> {
        self.producer_function(meter, function, task_owner, limits)
            .map(|row| row.digest)
    }

    pub(crate) fn producer_function(
        &self,
        meter: &mut TaskSemanticMeter,
        function: crate::runtime_id::RuntimeFunctionSiteId,
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        limits: plan::RuntimeTaskPlanSealLimits,
    ) -> Result<ProducerFunctionSemantic<'a>, RuntimeBodySemanticError> {
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
                    self.write_flow(&mut encoder, body.ops(), task_owner)?;
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
        for (ordinal, (kind, path, _)) in paths.iter().enumerate() {
            encoder.enter_element();
            encoder.enter_role();
            encoder.count(ordinal);
            encoder.tag(kind.semantic_tag());
            encoder.digest(path.as_bytes());
        }
        let digest = ProducerFunctionSemanticDigest(*encoder.finish()?.as_bytes());
        Ok(ProducerFunctionSemantic {
            digest,
            plan: self.plan,
            function: site,
            function_id: function,
            body_root: body,
            endpoints: paths.into_boxed_slice(),
        })
    }

    pub(super) fn endpoint_count(
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
        body: &'a RuntimeFunctionSiteBody,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Vec<(EndpointKind, blake3::Hash, &'a plan::FlowOp)>, RuntimeBodySemanticError> {
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
                        endpoints.push((kind, encoder.finish()?, op));
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
            | Op::StandardMap(_)
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
            | Op::EnterScheduledScope { .. }
            | Op::ExitScheduledScope { .. }
            | Op::ExitScope
            | Op::CompleteAwaitObserver
            | Op::ExitScopeBind { .. }
            | Op::Noop => &[],
        }
    }
}
