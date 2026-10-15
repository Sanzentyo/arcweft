use crate::pattern::RuntimePattern;
use crate::runtime_id::RuntimePlanTypeId;
use crate::runtime_id::{RuntimeIdError, RuntimeIdFamily, RuntimeIdPath, RuntimePublicLabel};
use crate::task::TaskSequence;
use crate::value::{RuntimeExpr, RuntimePayload, RuntimeValue};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StreamRuntimeId {
    path: RuntimeIdPath,
}

/// Lowered stream transform state machine.
///
/// The core runtime keeps this as deterministic data. Host adapters may execute
/// the state machine or replace it with an equivalent backend implementation,
/// but device acquisition never happens inside this plan.
#[derive(Clone, Debug, PartialEq)]
pub struct StreamPlan {
    id: StreamRuntimeId,
    item_ty: RuntimePlanTypeId,
    error_ty: RuntimePlanTypeId,
    ops: Vec<StreamOp>,
}

impl StreamPlan {
    pub(crate) fn from_admitted_parts(
        id: StreamRuntimeId,
        item_ty: RuntimePlanTypeId,
        error_ty: RuntimePlanTypeId,
        ops: Vec<StreamOp>,
    ) -> Self {
        Self {
            id,
            item_ty,
            error_ty,
            ops,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &StreamRuntimeId {
        &self.id
    }

    #[must_use]
    pub const fn item_ty(&self) -> RuntimePlanTypeId {
        self.item_ty
    }

    #[must_use]
    pub const fn error_ty(&self) -> RuntimePlanTypeId {
        self.error_ty
    }

    #[must_use]
    pub fn ops(&self) -> &[StreamOp] {
        &self.ops
    }
}

/// One operation in a lowered stream transform.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamOp {
    Let {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    ForNext {
        pattern: RuntimePattern,
        source: RuntimeExpr,
        body: Vec<StreamOp>,
    },
    Yield {
        expr: RuntimeExpr,
    },
    If {
        condition: RuntimeExpr,
        then_ops: Vec<StreamOp>,
        else_ops: Vec<StreamOp>,
    },
    Match {
        scrutinee: RuntimeExpr,
        arms: Vec<StreamMatchArm>,
    },
    Close {
        source: RuntimeExpr,
    },
    Return,
}

impl StreamOp {
    pub(crate) const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Let { .. } => 0,
            Self::ForNext { .. } => 1,
            Self::Yield { .. } => 2,
            Self::If { .. } => 3,
            Self::Match { .. } => 4,
            Self::Close { .. } => 5,
            Self::Return => 6,
        }
    }
}

/// One stream `match` arm.
#[derive(Clone, Debug, PartialEq)]
pub struct StreamMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeExpr>,
    pub ops: Vec<StreamOp>,
}

/// Native and Product stream queue storage; only inert queues have a reader.
///
/// ```compile_fail
/// use arcweft_core::stream::StreamRuntimeState;
/// fn copy(value: &StreamRuntimeState) { let _: StreamRuntimeState = value.clone(); }
/// ```
/// ```compile_fail
/// use arcweft_core::stream::StreamRuntimeState;
/// let _: StreamRuntimeState = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(
    serialize = "V: crate::value::InertRuntimeValueStorage",
    deserialize = "V: crate::value::InertRuntimeValueStorage"
))]
pub struct StreamRuntimeState<V = RuntimePayload> {
    pub id: StreamRuntimeId,
    pub queue: VecDeque<V>,
    pub closed: bool,
    pub emitted_count: u64,
}

impl Clone for StreamRuntimeState<crate::value::AwbcRuntimeValueSnapshot> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            queue: self.queue.clone(),
            closed: self.closed,
            emitted_count: self.emitted_count,
        }
    }
}

impl<P> StreamRuntimeState<P> {
    /// Native queues borrow their exact original stream item contract. Local
    /// yield publishes both a queue item and an event, so retained items must
    /// satisfy that same unrestricted ownership law.
    pub(crate) fn admit_native_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        let invalid = |message: &str| crate::value::AwbcRuntimeValueSnapshotError::Message {
            message: message.to_owned(),
        };
        admission.validate_metadata(&(&self.id, self.closed, self.emitted_count))?;
        admission.validate_metadata_collection(self.queue.len())?;
        if (self.closed && !self.queue.is_empty())
            || u64::try_from(self.queue.len()).map_or(true, |len| len > self.emitted_count)
        {
            return Err(invalid(
                "native stream queue disagrees with its closed/emitted state",
            ));
        }
        let expected = {
            let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner()
            else {
                return Err(invalid(
                    "native stream requires its original Plan executable",
                ));
            };
            let stream = plan
                .stream_plans()
                .iter()
                .find(|stream| stream.id() == &self.id)
                .ok_or_else(|| invalid("native stream is absent from its original executable"))?;
            plan.type_table()
                .get(stream.item_ty())
                .ok_or_else(|| invalid("native stream has an unknown original item type"))?
                .semantic_identity()
        };
        for value in &self.queue {
            admission.validate_typed_borrowed(value.borrowed_value(), expected)?;
        }
        Ok(())
    }
}

/// Event emitted by an ordinary callable-backed stream.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum StreamEventKind<T, E> {
    Item(T),
    Error(E),
    End,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StreamEvent<T, E> {
    pub stream: StreamRuntimeId,
    pub sequence: TaskSequence,
    pub kind: StreamEventKind<T, E>,
}

pub type RuntimeStreamEvent = StreamEvent<RuntimePayload, RuntimePayload>;

/// Local `yield` publishes one item to both the runtime queue and the typed
/// host stream event. The borrowed proof binds the exact value being copied;
/// a generic stream item has no blanket Copy entitlement.
pub(crate) struct RuntimeStreamYieldCopyProof<'a> {
    value: &'a RuntimeValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeStreamYieldError {
    #[error(
        "local stream yield requires a recursively unrestricted item for queue and host publication"
    )]
    AffineItem,
}

impl<'a> RuntimeStreamYieldCopyProof<'a> {
    /// Inspects Copy custody during the shared live/inert graph traversal.
    /// Descendants consume the resource bank once, before any queue projection.
    pub(crate) fn validate_inputs(
        value: crate::value::RuntimeValueView<'_>,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        value.admit_unrestricted(resources)
    }

    pub(crate) fn inspect(value: &'a RuntimeValue) -> Result<Self, RuntimeStreamYieldError> {
        value
            .ownership()
            .permits_copy()
            .then_some(Self { value })
            .ok_or(RuntimeStreamYieldError::AffineItem)
    }

    #[must_use]
    pub(crate) fn copy(&self) -> RuntimeValue {
        self.value.clone()
    }
}

impl StreamRuntimeId {
    pub fn canonical(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_canonical_str(RuntimeIdFamily::Stream, value).map(|path| Self { path })
    }

    pub fn from_source_entity_body(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_source_entity_body(
            RuntimeIdFamily::Stream,
            value,
            RuntimeIdFamily::Stream.source_families(),
        )
        .map(|path| Self { path })
    }

    pub fn from_runtime_target_value(value: &str) -> Result<Self, RuntimeIdError> {
        let Some((family, _)) = value.split_once('.') else {
            return Self::canonical(value);
        };
        if RuntimeIdFamily::Stream.source_families().contains(&family) {
            Self::from_source_entity_body(value)
        } else {
            Self::canonical(value)
        }
    }

    #[must_use]
    pub const fn path(&self) -> &RuntimeIdPath {
        &self.path
    }

    #[must_use]
    pub fn canonical_label(&self) -> String {
        self.path.label()
    }

    #[must_use]
    pub fn public_label(&self) -> RuntimePublicLabel {
        RuntimePublicLabel::for_family(RuntimeIdFamily::Stream, &self.path)
    }
}

impl StreamRuntimeState {
    pub fn new(id: StreamRuntimeId) -> Self {
        Self {
            id,
            queue: VecDeque::new(),
            closed: false,
            emitted_count: 0,
        }
    }

    pub fn push_item(&mut self, item: RuntimePayload) -> TaskSequence {
        let sequence = TaskSequence(self.emitted_count);
        self.emitted_count += 1;
        if !self.closed {
            self.queue.push_back(item);
        }
        sequence
    }

    pub fn close(&mut self) {
        self.closed = true;
        self.queue.clear();
    }

    pub fn close_with_sequence(&mut self) -> Option<TaskSequence> {
        if self.closed {
            return None;
        }
        let sequence = TaskSequence(self.emitted_count);
        self.emitted_count += 1;
        self.close();
        Some(sequence)
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn native_queue() -> (
        crate::task::RuntimeProgramOwner,
        StreamRuntimeState,
        StreamRuntimeState<crate::value::AwbcRuntimeValueSnapshot>,
    ) {
        use crate::plan::{
            RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
            RuntimeStreamOpSeed, RuntimeStreamPlanSeed,
        };
        let boolean = crate::pattern::RuntimeSemanticTypeId::from_bytes([91; 32]);
        let unit = crate::pattern::RuntimeSemanticTypeId::from_bytes([92; 32]);
        let id = StreamRuntimeId::canonical("owned").unwrap();
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                ],
                [],
            )
            .unwrap();
        builder
            .push_stream_plan_seed(RuntimeStreamPlanSeed {
                id: id.clone(),
                item_ty: boolean,
                error_ty: unit,
                ops: vec![RuntimeStreamOpSeed::Return],
            })
            .unwrap();
        let owner =
            crate::task::RuntimeProgramOwner::Plan(std::sync::Arc::new(builder.finish().unwrap()));
        let mut live = StreamRuntimeState::new(id.clone());
        for _ in 0..64 {
            live.push_item(RuntimePayload::new(RuntimeValue::Bool(true)));
        }
        let inert = StreamRuntimeState {
            id,
            queue: live
                .queue
                .iter()
                .map(|value| {
                    crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                        value.value(),
                        &owner,
                    )
                    .unwrap()
                })
                .collect(),
            closed: false,
            emitted_count: 64,
        };
        (owner, live, inert)
    }

    #[test]
    fn native_queue_admits_live_and_inert_items_with_exact_shared_limits() {
        let (owner, live, inert) = native_queue();
        let before = inert.clone();
        let limits = crate::entry::RuntimeSchemaLimits {
            max_sequence_items: 64,
            ..crate::entry::RuntimeSchemaLimits::engine_default()
        };
        let mut exact =
            crate::value::RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        live.admit_native_retained(&mut exact).unwrap();
        inert.admit_native_retained(&mut exact).unwrap();
        let mut short = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits {
                max_sequence_items: 63,
                ..limits
            },
        );
        let error = live.admit_native_retained(&mut short).unwrap_err();
        assert!(matches!(
            error,
            crate::value::AwbcRuntimeValueSnapshotError::Resource(
                crate::entry::RuntimeSchemaError::BudgetExceeded {
                    budget: "sequence_items"
                }
            )
        ));
        assert_eq!(inert.admit_native_retained(&mut short).unwrap_err(), error);
        let mut short = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner,
            crate::entry::RuntimeSchemaLimits {
                max_sequence_items: 63,
                ..limits
            },
        );
        assert_eq!(inert.admit_native_retained(&mut short).unwrap_err(), error);
        assert_eq!(inert, before);
        assert_eq!(live.queue.len(), 64);
    }

    #[test]
    fn native_queue_rejects_foreign_items_ids_and_inconsistent_progress() {
        let (owner, mut live, inert) = native_queue();
        let admission = || {
            crate::value::RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                crate::entry::RuntimeSchemaLimits::engine_default(),
            )
        };
        *live.queue.front_mut().unwrap() = RuntimePayload::new(RuntimeValue::Unit);
        assert!(live.admit_native_retained(&mut admission()).is_err());
        let mut wrong_item = inert.clone();
        *wrong_item.queue.front_mut().unwrap() =
            crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                &RuntimeValue::Unit,
                &owner,
            )
            .unwrap();
        assert!(wrong_item.admit_native_retained(&mut admission()).is_err());
        for (closed, emitted_count) in [(true, 64), (false, 63)] {
            let mut wrong = inert.clone();
            wrong.closed = closed;
            wrong.emitted_count = emitted_count;
            assert!(
                matches!(wrong.admit_native_retained(&mut admission()), Err(crate::value::AwbcRuntimeValueSnapshotError::Message { message }) if message == "native stream queue disagrees with its closed/emitted state")
            );
        }
        let mut foreign = inert;
        foreign.id = StreamRuntimeId::canonical("foreign").unwrap();
        assert!(
            matches!(foreign.admit_native_retained(&mut admission()), Err(crate::value::AwbcRuntimeValueSnapshotError::Message { message }) if message == "native stream is absent from its original executable")
        );
    }

    #[test]
    fn deeply_nested_stream_body_is_encoded_without_recursive_descent() {
        let semantic = crate::pattern::RuntimeSemanticTypeId::from_bytes([11; 32]);
        let mut builder = crate::plan::RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [crate::plan::RuntimePlanTypeSeed::new(
                    semantic,
                    crate::plan::RuntimePlanTypeProjection::Bool,
                )],
                [],
            )
            .unwrap();
        let plan = builder.finish().unwrap();
        let ty =
            crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(std::num::NonZeroU32::MIN);
        let mut ops = vec![crate::stream::StreamOp::Return];
        for _ in 0..20_000 {
            ops = vec![crate::stream::StreamOp::If {
                condition: crate::value::RuntimeExpr::from_admitted_parts(
                    ty,
                    crate::value::RuntimeExprKind::Value(crate::value::RuntimeValue::Bool(true)),
                ),
                then_ops: ops,
                else_ops: vec![],
            }];
        }
        let stream = crate::stream::StreamPlan::from_admitted_parts(
            crate::stream::StreamRuntimeId::canonical("deep").unwrap(),
            ty,
            ty,
            ops,
        );
        let mut meter = crate::task::semantic::TaskSemanticMeter::new(2_000_000, 10_000_000);
        let mut encoder =
            crate::task::semantic::TaskSemanticEncoder::new(b"stream-body.v1\0", &mut meter);
        crate::plan::body_semantic::RuntimeBodySemanticContext::new(&plan)
            .write_stream(&mut encoder, &stream)
            .unwrap();
        assert!(encoder.finish().is_ok());
        let mut work = stream.ops;
        while let Some(op) = work.pop() {
            if let StreamOp::If {
                then_ops, else_ops, ..
            } = op
            {
                work.extend(then_ops);
                work.extend(else_ops);
            }
        }
    }
}
