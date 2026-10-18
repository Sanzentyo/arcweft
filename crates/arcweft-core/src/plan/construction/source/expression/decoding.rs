//! Genuine handles enter seeds; the authenticated body construction and common
//! executable seal still own expression admission, scope and result checking.

use super::super::RuntimePlanSourceConstruction;
use super::cursor::{Cursor, Row};
use super::{
    Counts, Error, PreparedRuntimeExpressionSource, RuntimeExpressionSourceLimits, charge,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::{RuntimeOpaqueTypeProducerId, RuntimePattern, RuntimeSemanticTypeId};
use crate::plan::value_admission::PlanValueAuthority;
use crate::plan::*;
use crate::runtime_id::*;
use crate::value::*;

pub(in crate::plan) struct RuntimeExpressionSourceHandles<'a> {
    pub states: &'a [RuntimeCallableStateSeedId],
    pub specializations: &'a [RuntimeCallableSpecializationSeedId],
    pub helpers: &'a [RuntimePureHelperSeedId],
    pub methods: &'a [RuntimeTraitMethodSeedId],
    pub attempts: &'a [RuntimeFormatAttemptSeedId],
}
impl RuntimeExpressionSourceHandles<'_> {
    fn validate(
        &self,
        source: &mut RuntimePlanSourceConstruction,
        limits: RuntimeExpressionSourceLimits,
    ) -> Result<(), Error> {
        for (kind, expected, actual) in [
            ("type", limits.type_count, source.builder.types.len()),
            ("local", limits.local_count, source.locals.len()),
            (
                "state",
                limits.state_count,
                source.builder.callable_states.borrow().states.len(),
            ),
            ("state handles", limits.state_count, self.states.len()),
            (
                "specialization",
                limits.specialization_count,
                source.builder.callable_specializations.len(),
            ),
            (
                "specialization handles",
                limits.specialization_count,
                self.specializations.len(),
            ),
            (
                "helper",
                limits.helper_count,
                source.builder.pure_helpers.len(),
            ),
            ("helper handles", limits.helper_count, self.helpers.len()),
            (
                "method",
                limits.method_count,
                source.builder.trait_methods.len(),
            ),
            ("method handles", limits.method_count, self.methods.len()),
            (
                "template",
                limits.template_count,
                source.builder.dialogue_content.template_count(),
            ),
            (
                "attempt",
                limits.attempt_count,
                source.builder.format_attempts.len(),
            ),
            ("attempt handles", limits.attempt_count, self.attempts.len()),
        ] {
            if expected != actual {
                return Err(Error::Namespace {
                    kind,
                    expected,
                    actual,
                });
            }
        }
        let count = self
            .states
            .len()
            .checked_add(self.specializations.len())
            .and_then(|n| n.checked_add(self.helpers.len()))
            .and_then(|n| n.checked_add(self.methods.len()))
            .and_then(|n| n.checked_add(self.attempts.len()))
            .ok_or(Error::Arithmetic)?;
        source
            .meter
            .charge_work(u64::try_from(count).map_err(|_| Error::Arithmetic)?)?;
        for (index, handle) in self.states.iter().enumerate() {
            if handle
                .resolve(&source.builder.issuer)
                .is_none_or(|id| id.index() != index)
            {
                return Err(Error::Handle { kind: "state" });
            }
        }
        for (index, handle) in self.specializations.iter().enumerate() {
            if source
                .builder
                .resolve_callable_specialization_seed(handle)?
                .0
                .index()
                != index
            {
                return Err(Error::Handle {
                    kind: "specialization",
                });
            }
        }
        for (index, handle) in self.helpers.iter().enumerate() {
            if handle
                .resolve(&source.builder.issuer)
                .is_none_or(|(id, ..)| id.0 != index)
            {
                return Err(Error::Handle { kind: "helper" });
            }
        }
        for (index, handle) in self.methods.iter().enumerate() {
            if handle
                .resolve(&source.builder.issuer)
                .is_none_or(|(id, ..)| id.0 != index)
            {
                return Err(Error::Handle { kind: "method" });
            }
        }
        for (index, handle) in self.attempts.iter().enumerate() {
            if handle
                .resolve(&source.builder.issuer)
                .is_none_or(|id| id.index() != index)
            {
                return Err(Error::Handle { kind: "attempt" });
            }
        }
        Ok(())
    }
}
impl PreparedRuntimeExpressionSource<'_> {
    pub(in crate::plan) fn materialize(
        self,
        source: &mut RuntimePlanSourceConstruction,
        handles: &RuntimeExpressionSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<RuntimeExprSeed, Error> {
        source.meter.status()?;
        let result = (|| {
            handles.validate(source, self.limits)?;
            charge(&mut source.meter, self.bytes.len())?;
            let mut cursor = Cursor::new(self.bytes, self.limits);
            let count = cursor.header()?;
            let mut counts = Counts::new(self.limits, count)?;
            let mut rows = Vec::with_capacity(count);
            for _ in 0..count {
                rows.push(cursor.row(&mut counts, resources, &mut source.meter)?);
            }
            cursor.finish()?;
            debug_assert_eq!(counts.footprint, self.footprint);
            let mut seeds = Vec::with_capacity(count);
            for row in rows.into_iter().rev() {
                if row.children > seeds.len() {
                    return Err(Error::Root);
                }
                let before = seeds.len();
                let mut decoder = Decoder {
                    source,
                    handles,
                    resources,
                    seeds: &mut seeds,
                    limits: self.limits,
                };
                let seed = decoder.row(&row)?;
                debug_assert_eq!(seeds.len(), before - row.children);
                seeds.push(seed);
            }
            if seeds.len() != 1 {
                return Err(Error::Root);
            }
            Ok(seeds.pop().expect("one complete source root"))
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}
struct Decoder<'a, 'b> {
    source: &'a mut RuntimePlanSourceConstruction,
    handles: &'a RuntimeExpressionSourceHandles<'b>,
    resources: &'a mut ValueResourceAdmission,
    seeds: &'a mut Vec<RuntimeExprSeed>,
    limits: RuntimeExpressionSourceLimits,
}
impl Decoder<'_, '_> {
    fn semantic(&self, ty: RuntimePlanTypeId) -> RuntimeSemanticTypeId {
        self.source
            .builder
            .types
            .get(ty)
            .expect("original type namespace checked")
            .semantic_identity()
    }
    fn child(&mut self) -> RuntimeExprSeed {
        self.seeds
            .pop()
            .expect("complete prefix operands preflighted")
    }
    fn boxed(&mut self) -> Box<RuntimeExprSeed> {
        Box::new(self.child())
    }
    fn children(&mut self, n: usize) -> Box<[RuntimeExprSeed]> {
        (0..n).map(|_| self.child()).collect()
    }
    fn local(&self, raw: u32) -> RuntimeLocalSeedId {
        let id = RuntimeLocalDeclarationId::from_accepted_ordinal(
            std::num::NonZeroU32::new(raw).expect("checked local"),
        );
        self.source
            .local(id)
            .expect("original local namespace checked")
            .clone()
    }
    fn fields(&self, cursor: &mut Cursor<'_>) -> Result<Box<[RuntimeRecordFieldSeedId]>, Error> {
        Ok(cursor
            .fields()?
            .chunks_exact(4)
            .map(|field| {
                RuntimeRecordFieldSeedId::from_zero_based(
                    u32::from_le_bytes(field.try_into().expect("field width")) - 1,
                )
            })
            .collect())
    }
    fn place(&self, cursor: &mut Cursor<'_>) -> Result<RuntimeMutablePlaceSeed, Error> {
        let local = self.local(cursor.u32()?);
        let fields = self.fields(cursor)?;
        Ok(if fields.is_empty() {
            RuntimeMutablePlaceSeed::Local(local)
        } else {
            RuntimeMutablePlaceSeed::Fields {
                base: local,
                fields,
            }
        })
    }
    fn arguments(
        &mut self,
        cursor: &mut Cursor<'_>,
    ) -> Result<Box<[RuntimeCallArgumentSeed]>, Error> {
        let n = cursor.count()?;
        (0..n)
            .map(|_| {
                let mode = if cursor.boolean()? {
                    RuntimeCallArgumentMode::Spread
                } else {
                    RuntimeCallArgumentMode::Value
                };
                let position = cursor.u32()?;
                Ok(RuntimeCallArgumentSeed::new(self.child(), mode, position))
            })
            .collect()
    }
    fn literal(
        &mut self,
        cursor: &mut Cursor<'_>,
        ty: RuntimePlanTypeId,
    ) -> Result<RuntimeValue, Error> {
        let inert = AwbcRuntimeValueSnapshot::preflight_literal_source(
            cursor.frame()?,
            self.limits.literal(),
            self.resources,
            &mut self.source.meter,
        )?
        .materialize(&mut self.source.meter)?;
        PlanValueAuthority::Building {
            types: &self.source.builder.types,
            records: &self.source.builder.nominal_record_domains,
            variants: &self.source.builder.variant_domains,
        }
        .materialize_literal_source_value(ty, inert, self.resources)
        .map_err(|error| Error::Literal(RuntimeLiteralSourceError::Value(error)))
    }
    fn pattern(&mut self, cursor: &mut Cursor<'_>) -> Result<RuntimePatternSeed, Error> {
        Ok(RuntimePattern::preflight_sources(
            cursor.frame()?,
            self.limits.pattern(),
            self.resources,
            &mut self.source.meter,
        )?
        .materialize(self.source, self.resources)?)
    }
    fn initialization(cursor: &mut Cursor<'_>) -> Result<RuntimePlaceInitialization, Error> {
        Ok(match cursor.initialization()? {
            0 => RuntimePlaceInitialization::Initialized,
            1 => RuntimePlaceInitialization::Uninitialized,
            _ => RuntimePlaceInitialization::Conditional,
        })
    }
    #[expect(
        clippy::too_many_lines,
        reason = "one complete source algebra maps into existing genuine construction seeds"
    )]
    fn row(&mut self, row: &Row<'_>) -> Result<RuntimeExprSeed, Error> {
        use RuntimeExprSeedKind as K;
        let mut r = Cursor::new(row.payload, self.limits);
        let kind = match row.tag {
            0 => K::Value(self.literal(&mut r, row.ty)?),
            6 => {
                let RuntimeValue::EntityRef(entity) = self.literal(&mut r, row.ty)? else {
                    return Err(r.invalid("entity literal"));
                };
                K::EntityRef(entity)
            }
            1 => K::Agent(self.agent(&mut r)?),
            2 => {
                let local = self.local(r.u32()?);
                let mode = if r.boolean()? {
                    RuntimeLocalReadMode::Move
                } else {
                    RuntimeLocalReadMode::Copy
                };
                let fields = self.fields(&mut r)?;
                K::Local(RuntimeLocalReadSeed::new_place(local, mode, fields))
            }
            3 => K::SequencePopFront {
                place: self.place(&mut r)?,
            },
            4 => K::SequencePush {
                place: self.place(&mut r)?,
                value: self.boxed(),
            },
            5 => K::SequencePopBack {
                place: self.place(&mut r)?,
            },
            7 => K::Let {
                binding: self.local(r.u32()?),
                expr: self.boxed(),
                body: self.boxed(),
            },
            8 => {
                let identity = if r.boolean()? {
                    crate::scope::RuntimeScopeIdentity::Named(arcweft_id::DeclarationName::try_new(
                        r.string()?,
                    )?)
                } else {
                    crate::scope::RuntimeScopeIdentity::Anonymous
                };
                K::Scope {
                    identity,
                    body: self.boxed(),
                }
            }
            9 => {
                let n = r.count()?;
                K::Tuple(self.children(n))
            }
            10 => {
                let template =
                    RuntimeDialogueContentTemplateId::from_zero_based(r.u32()? as usize - 1)
                        .expect("checked template");
                let n = r.count()?;
                let values = self.children(n);
                let n = r.count()?;
                let effects = (0..n)
                    .map(|_| {
                        let site =
                            RuntimeDialogueEffectSiteId::from_zero_based(r.u32()? as usize - 1)
                                .expect("checked site");
                        let state = self.handles.states[r.u32()? as usize - 1].clone();
                        let n = r.count()?;
                        Ok(RuntimeDialogueEffectSiteSeed {
                            site,
                            state,
                            captures: self.children(n),
                        })
                    })
                    .collect::<Result<Box<[_]>, Error>>()?;
                K::DialogueContent {
                    template,
                    values,
                    effects,
                }
            }
            11 => {
                let template =
                    RuntimeDialogueContentTemplateId::from_zero_based(r.u32()? as usize - 1)
                        .expect("checked template");
                let attempt = if r.boolean()? {
                    Some(self.handles.attempts[r.u32()? as usize - 1].clone())
                } else {
                    None
                };
                let project_method = if r.boolean()? {
                    Some(self.handles.methods[r.u32()? as usize - 1].clone())
                } else {
                    None
                };
                let project_option = r.boolean()?;
                let n = r.count()?;
                let operands = (0..n)
                    .map(|_| {
                        let definition = RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                            r.slice(32)?.try_into().expect("definition width"),
                        );
                        let parameter = RuntimeFmtParameterId::from_index(usize::from(r.byte()?))
                            .expect("checked parameter");
                        Ok(RuntimeFormatContentOperandSeed::new(
                            definition,
                            parameter,
                            self.child(),
                        ))
                    })
                    .collect::<Result<Box<[_]>, Error>>()?;
                K::FormatContent {
                    template,
                    attempt,
                    operands,
                    project_method,
                    project_option,
                }
            }
            12 => {
                use arcweft_interaction_model::dialogue::*;
                let operation = if r.boolean()? {
                    CharacterDialogueOperation::Reconfigure
                } else {
                    CharacterDialogueOperation::Factory
                };
                let target = self.boxed();
                let n = r.count()?;
                let fields = (0..n)
                    .map(|_| {
                        use CharacterDialogueFieldCoordinate as F;
                        let coordinate = match r.byte()? {
                            0 => F::Voice,
                            1 => F::Look,
                            2 => F::Stage,
                            3 => F::Portrait,
                            4 => F::Focus,
                            5 => F::Cleanup,
                            6 => F::View,
                            7 => F::SourceLocale,
                            8 => F::Hooks,
                            9 => F::Style,
                            10 => F::RichText,
                            11 => F::InlineFailure,
                            12 => F::Custom(CharacterDialogueCustomFieldId::try_new(r.string()?)?),
                            _ => return Err(r.invalid("dialogue field")),
                        };
                        let operation = if r.boolean()? {
                            CharacterDialoguePatchOperation::Set(self.child())
                        } else {
                            CharacterDialoguePatchOperation::Clear
                        };
                        Ok(CharacterDialoguePatchField {
                            coordinate,
                            operation,
                        })
                    })
                    .collect::<Result<Box<[_]>, Error>>()?;
                K::CharacterDialogue {
                    operation,
                    target,
                    fields,
                }
            }
            13 => {
                let n = r.count()?;
                K::BracketSeq(self.children(n))
            }
            14 => K::RepeatSeq {
                len: r.u32()? as usize,
                value: self.boxed(),
            },
            15 => {
                let start = r.boolean()?;
                let end = r.boolean()?;
                let inclusive = r.boolean()?;
                K::Range {
                    start: start.then(|| self.boxed()),
                    end: end.then(|| self.boxed()),
                    inclusive,
                }
            }
            16 => {
                let n = r.count()?;
                K::NominalRecord(
                    (0..n)
                        .map(|_| {
                            Ok(RuntimeNominalRecordFieldSeed::new(
                                RuntimeRecordFieldSeedId::from_zero_based(r.u32()? - 1),
                                self.child(),
                            ))
                        })
                        .collect::<Result<Box<[_]>, Error>>()?,
                )
            }
            17 => K::Variant {
                ordinal: r.u32()?,
                payload: r.boolean()?.then(|| self.boxed()),
            },
            18 => {
                let target = if r.boolean()? {
                    RuntimeFieldTargetSeed::Inspect(self.place(&mut r)?)
                } else {
                    RuntimeFieldTargetSeed::Value(self.boxed())
                };
                let field = match r.byte()? {
                    0 => RuntimeFieldProjectionSeed::Nominal {
                        owner: self.semantic(r.ty()?),
                        field: RuntimeRecordFieldSeedId::from_zero_based(r.u32()? - 1),
                    },
                    1 => RuntimeFieldProjectionSeed::OpaqueRecord {
                        owner: self.semantic(r.ty()?),
                        producer: RuntimeOpaqueTypeProducerId::try_new(r.string()?)?,
                        field: RuntimeRecordFieldSeedId::from_zero_based(r.u32()? - 1),
                        field_type: self.semantic(row.ty),
                    },
                    2 => {
                        let tag = r.u32()?;
                        RuntimeFieldProjectionSeed::Agent(
                            *RuntimeAgentField::ALL
                                .iter()
                                .find(|field| u32::from(field.semantic_tag()) == tag)
                                .expect("checked Agent field"),
                        )
                    }
                    3 => RuntimeFieldProjectionSeed::EntityReference(match r.byte()? {
                        0 => RuntimeEntityReferenceField::Id,
                        1 => RuntimeEntityReferenceField::Family,
                        _ => RuntimeEntityReferenceField::Name,
                    }),
                    4 => RuntimeFieldProjectionSeed::Progress(if r.byte()? == 0 {
                        RuntimeProgressField::Ratio
                    } else {
                        RuntimeProgressField::Label
                    }),
                    _ => return Err(r.invalid("field projection")),
                };
                K::Field { target, field }
            }
            19 => K::ProjectTuple {
                target: self.boxed(),
                ordinal: r.u32()?,
            },
            20 => K::ProjectRecord {
                target: self.boxed(),
                field: RuntimeRecordFieldSeedId::from_zero_based(r.u32()?),
            },
            21 => {
                let place = self.place(&mut r)?;
                let displacement = if r.boolean()? {
                    let initialization = Self::initialization(&mut r)?;
                    let n = r.count()?;
                    let fields = (0..n)
                        .map(|_| {
                            Ok(RuntimeDisplacedField {
                                fields: self.fields(&mut r)?,
                                initialization: Self::initialization(&mut r)?,
                            })
                        })
                        .collect::<Result<Box<[_]>, Error>>()?;
                    RuntimePlaceDisplacement::Reachable {
                        initialization,
                        fields,
                    }
                } else {
                    RuntimePlaceDisplacement::Unreachable
                };
                K::Assign {
                    place: RuntimeAssignmentSeed {
                        place,
                        displacement,
                    },
                    expr: self.boxed(),
                    body: self.boxed(),
                }
            }
            22 => {
                let callee = self.call_target(&mut r)?;
                K::Call {
                    callee,
                    args: self.arguments(&mut r)?,
                }
            }
            23 => {
                let state = self.handles.states[r.u32()? as usize - 1].clone();
                let n = r.count()?;
                K::MakeCallable {
                    state,
                    captures: self.children(n),
                }
            }
            24 => K::SpecializeCallable {
                specialization: self.handles.specializations[r.u32()? as usize - 1].clone(),
                value: self.boxed(),
            },
            25 => K::Apply {
                callee: self.boxed(),
                args: self.arguments(&mut r)?,
            },
            26 => {
                let callable = self.handles.methods[r.u32()? as usize - 1].clone();
                let mode = r.byte()?;
                let (_, receiver, ..) = callable
                    .resolve(&self.source.builder.issuer)
                    .expect("original method handle checked");
                let expected = match receiver {
                    RuntimeReceiverMode::Owned => 0,
                    RuntimeReceiverMode::SharedRef => 1,
                    RuntimeReceiverMode::MutRef => 2,
                };
                if mode != expected {
                    return Err(r.invalid("method receiver contract"));
                }
                K::TraitCall {
                    callable,
                    receiver: self.boxed(),
                    args: self.arguments(&mut r)?,
                }
            }
            27 => {
                let helper = self.handles.helpers[r.u32()? as usize - 1].clone();
                K::PureCall {
                    helper,
                    args: self.arguments(&mut r)?,
                }
            }
            29 => K::Sum {
                source: self.boxed(),
            },
            30 => K::Unary {
                op: if r.byte()? == 0 {
                    RuntimeUnaryOp::Not
                } else {
                    RuntimeUnaryOp::Neg
                },
                expr: self.boxed(),
            },
            31 => {
                let op = match r.byte()? {
                    0 => RuntimeBinaryOp::Eq,
                    1 => RuntimeBinaryOp::Ne,
                    2 => RuntimeBinaryOp::Lt,
                    3 => RuntimeBinaryOp::Le,
                    4 => RuntimeBinaryOp::Gt,
                    5 => RuntimeBinaryOp::Ge,
                    6 => RuntimeBinaryOp::Add,
                    7 => RuntimeBinaryOp::Sub,
                    8 => RuntimeBinaryOp::Mul,
                    9 => RuntimeBinaryOp::Div,
                    10 => RuntimeBinaryOp::And,
                    _ => RuntimeBinaryOp::Or,
                };
                K::Binary {
                    lhs: self.boxed(),
                    op,
                    rhs: self.boxed(),
                }
            }
            32 => K::If {
                condition: self.boxed(),
                then_expr: self.boxed(),
                else_expr: self.boxed(),
            },
            33 => {
                let pattern = self.pattern(&mut r)?;
                let guard = r.boolean()?;
                K::IfLet {
                    pattern,
                    expr: self.boxed(),
                    guard: guard.then(|| self.boxed()),
                    then_expr: self.boxed(),
                    else_expr: self.boxed(),
                }
            }
            34 => {
                let scrutinee = self.boxed();
                let n = r.count()?;
                let arms = (0..n)
                    .map(|_| {
                        let pattern = self.pattern(&mut r)?;
                        let guard = r.boolean()?.then(|| self.child());
                        Ok(RuntimeExprMatchArmSeed::new(pattern, guard, self.child()))
                    })
                    .collect::<Result<Box<[_]>, Error>>()?;
                K::Match { scrutinee, arms }
            }
            35 => K::ReductionUnchanged {
                state: self.boxed(),
            },
            _ => return Err(r.invalid("expression tag")),
        };
        r.finish()?;
        let guards = row
            .guards
            .chunks_exact(4)
            .map(|raw| self.local(u32::from_le_bytes(raw.try_into().expect("guard width"))))
            .collect::<Vec<_>>();
        Ok(RuntimeExprSeed::new(self.semantic(row.ty), kind).with_guard_copy_locals(guards))
    }
    fn call_target(&self, r: &mut Cursor<'_>) -> Result<RuntimeCallTarget, Error> {
        Ok(match r.byte()? {
            0 => RuntimeCallTarget::Intrinsic(
                RuntimeIntrinsic::from_source_tag(r.byte()?).expect("checked intrinsic"),
            ),
            1 => {
                RuntimeCallTarget::Callable(crate::entry::RuntimeCallableId::try_new(r.string()?)?)
            }
            2 => {
                let identity = crate::entry::RuntimeCallableId::try_new(r.string()?)?;
                let provider = match r.byte()? {
                    0 => RuntimeExternalCallableProvider::Standard(
                        StandardEnvironmentId::from_canonical_tag(r.byte()?)
                            .expect("checked environment"),
                    ),
                    1 => RuntimeExternalCallableProvider::Adapter(r.string()?.to_owned()),
                    _ => return Err(r.invalid("external provider")),
                };
                let export = match r.byte()? {
                    tag @ (0 | 2) => {
                        let n = r.count()?;
                        let path = (0..n)
                            .map(|_| r.string().map(str::to_owned))
                            .collect::<Result<Box<[_]>, Error>>()?;
                        if tag == 0 {
                            RuntimeExternalCallableExport::Function(path)
                        } else {
                            RuntimeExternalCallableExport::RustFunction(path)
                        }
                    }
                    1 => RuntimeExternalCallableExport::Method {
                        receiver: r.slice(32)?.try_into().expect("receiver identity width"),
                        name: r.string()?.to_owned(),
                    },
                    _ => return Err(r.invalid("external export")),
                };
                RuntimeCallTarget::External(RuntimeExternalCallable::new(
                    identity,
                    RuntimeExternalCallableDeclaration::try_new(provider, export, r.u16()?)?,
                ))
            }
            _ => return Err(r.invalid("call target")),
        })
    }
    fn agent(&mut self, r: &mut Cursor<'_>) -> Result<RuntimeAgentExprSeed, Error> {
        use RuntimeAgentConstructor as C;
        use RuntimeAgentExprSeed as A;
        let constructor = C::from_semantic_tag(r.byte()?).expect("checked constructor");
        let choice = if constructor == C::ChoiceAction {
            Some(crate::entry::RuntimeCommandTargetId::try_new(r.string()?)?)
        } else {
            None
        };
        let n = r.count()?;
        Ok(match constructor {
            C::ChoiceAction => A::ChoiceAction {
                choice: choice.expect("choice source identity"),
            },
            C::CaptureViewport => A::CaptureViewport,
            C::CaptureLayer => A::CaptureLayer {
                target: self.boxed(),
            },
            C::CaptureObject => A::CaptureObject {
                target: self.boxed(),
            },
            C::StatePath => A::StatePath { path: self.boxed() },
            C::ObservationPath => A::ObservationPath { path: self.boxed() },
            C::ProbeSignal => A::ProbeSignal {
                target: self.boxed(),
            },
            C::ProbeMetric => A::ProbeMetric {
                target: self.boxed(),
            },
            C::ProbeState => A::ProbeState { path: self.boxed() },
            C::ProbeObservation => A::ProbeObservation { path: self.boxed() },
            C::Diagnostics => A::Diagnostics,
            C::PredicateExists => A::PredicateExists {
                probe: self.boxed(),
            },
            C::PredicateActionEnabled => A::PredicateActionEnabled {
                target: self.boxed(),
            },
            C::PredicateDiagnosticsHasError => A::PredicateDiagnosticsHasError {
                diagnostics: self.boxed(),
            },
            C::PredicateAll => A::PredicateAll {
                predicates: self.children(n),
            },
            C::PredicateAny => A::PredicateAny {
                predicates: self.children(n),
            },
            C::PredicateNot => A::PredicateNot {
                predicate: self.boxed(),
            },
            C::ViewportPoint => A::ViewportPoint {
                x: self.boxed(),
                y: self.boxed(),
            },
            C::PredicateEq
            | C::PredicateNotEq
            | C::PredicateGreater
            | C::PredicateGreaterOrEqual
            | C::PredicateLess
            | C::PredicateLessOrEqual => {
                let op = match constructor {
                    C::PredicateEq => RuntimeAgentCompareOp::Eq,
                    C::PredicateNotEq => RuntimeAgentCompareOp::NotEq,
                    C::PredicateGreater => RuntimeAgentCompareOp::Greater,
                    C::PredicateGreaterOrEqual => RuntimeAgentCompareOp::GreaterOrEqual,
                    C::PredicateLess => RuntimeAgentCompareOp::Less,
                    C::PredicateLessOrEqual => RuntimeAgentCompareOp::LessOrEqual,
                    _ => unreachable!("comparison constructor selected"),
                };
                A::PredicateCompare {
                    probe: self.boxed(),
                    op,
                    value: self.boxed(),
                }
            }
        })
    }
}
