use arcweft_lang_hir::{
    model::{HirFlow, HirFunction, HirModule},
    symbol::{CallableDeclarationId, CallablePackageId},
};
use arcweft_lang_syntax::{
    ast::flow::ContractClause,
    reference::BorrowKind,
    types::{AuthoredTypeRef, FnReceiverKind, GenericParam},
};

use crate::{
    callable::{
        CallableParameterPassing, CallableParameterPresence, CallableParameterType, CallableRecord,
    },
    check::TypeCheckReport,
    effect_model::CallableId,
    effect_row::EffectRowTail,
    effects::EffectSet,
    nominal::ResolvedTypeRefOutcome,
    types::{ArrayLength, MapKind, TypeKind},
};

use super::{
    BoundNominalKind, BoundNominalTypeKey,
    digest::{
        CanonicalAtomic, CanonicalCallableContract, CanonicalConstructor, CanonicalEffectRow,
        CanonicalFlowContract, CanonicalFlowSuspension, CanonicalGenericParameter,
        CanonicalParameter, CanonicalParameterGroup, CanonicalSignature, CanonicalType,
        CanonicalWherePredicate,
    },
};

pub(super) struct EntryContractBuilder<'a> {
    typecheck: &'a TypeCheckReport,
    package: &'a CallablePackageId,
}

#[derive(Clone, Copy)]
pub(super) struct ReducerContractNominals<'a> {
    pub(super) state: &'a BoundNominalTypeKey,
    pub(super) event: &'a BoundNominalTypeKey,
}

impl<'a> EntryContractBuilder<'a> {
    pub(super) const fn new(
        typecheck: &'a TypeCheckReport,
        package: &'a CallablePackageId,
    ) -> Self {
        Self { typecheck, package }
    }

    pub(super) fn initializer(
        &self,
        module: &HirModule,
        function: &HirFunction,
        record: &CallableRecord,
        declaration: &CallableDeclarationId,
        state: &BoundNominalTypeKey,
    ) -> Result<CanonicalCallableContract, String> {
        let (contract, effects_explicit) = self.callable(module, function, record)?;
        require_no_generics(&contract.signature, "initializer")?;
        require_empty_parameter_group(&contract.signature, "initializer")?;
        require_result(
            &contract.signature,
            &CanonicalType::Nominal(state.clone()),
            "initializer",
        )?;
        require_explicit_empty_effects(&contract, effects_explicit, "initializer")?;
        self.require_inferred_empty(declaration, "initializer")?;
        Ok(contract)
    }

    pub(super) fn reducer(
        &self,
        module: &HirModule,
        function: &HirFunction,
        record: &CallableRecord,
        declaration: &CallableDeclarationId,
        nominals: ReducerContractNominals<'_>,
    ) -> Result<CanonicalCallableContract, String> {
        let (contract, effects_explicit) = self.callable(module, function, record)?;
        require_no_generics(&contract.signature, "reducer")?;
        let [group] = contract.signature.groups.as_slice() else {
            return Err("reducer must declare exactly one parameter group".to_owned());
        };
        let [state_parameter, event_parameter] = group.parameters.as_slice() else {
            return Err("reducer must take exactly immutable &State and owned Event".to_owned());
        };
        require_fixed_parameter(state_parameter, "reducer state")?;
        require_fixed_parameter(event_parameter, "reducer event")?;
        let expected_state = CanonicalType::Borrow {
            kind: 1,
            lifetime: None,
            inner: Box::new(CanonicalType::Nominal(nominals.state.clone())),
        };
        if state_parameter.ty != expected_state {
            return Err(format!(
                "reducer parameter 0 must be `{}`, found `{}`",
                expected_state.source_label(),
                state_parameter.ty.source_label()
            ));
        }
        let expected_event = CanonicalType::Nominal(nominals.event.clone());
        if event_parameter.ty != expected_event {
            return Err(format!(
                "reducer parameter 1 must be `{}`, found `{}`",
                expected_event.source_label(),
                event_parameter.ty.source_label()
            ));
        }
        require_result(
            &contract.signature,
            &CanonicalType::Applied {
                constructor: CanonicalConstructor::Result,
                args: vec![
                    CanonicalType::Applied {
                        constructor: CanonicalConstructor::Reduction,
                        args: vec![CanonicalType::Nominal(nominals.state.clone())],
                    },
                    CanonicalType::Atomic(CanonicalAtomic::ReducerError),
                ],
            },
            "reducer",
        )?;
        require_explicit_empty_effects(&contract, effects_explicit, "reducer")?;
        self.require_inferred_empty(declaration, "reducer")?;
        Ok(contract)
    }

    pub(super) fn agent_controller(
        &self,
        module: &HirModule,
        function: &HirFunction,
        record: &CallableRecord,
        declaration: &CallableDeclarationId,
    ) -> Result<(CanonicalCallableContract, EffectSet, EffectSet), String> {
        let (contract, effects_explicit) = self.callable(module, function, record)?;
        require_no_generics(&contract.signature, "Agent controller")?;
        require_empty_parameter_group(&contract.signature, "Agent controller")?;
        require_result(
            &contract.signature,
            &CanonicalType::Applied {
                constructor: CanonicalConstructor::Result,
                args: vec![
                    CanonicalType::Atomic(CanonicalAtomic::Unit),
                    CanonicalType::Atomic(CanonicalAtomic::AgentError),
                ],
            },
            "Agent controller",
        )?;
        if !effects_explicit {
            return Err("Agent controller must declare an explicit closed effect row".to_owned());
        }
        let inferred = self.inferred_effects(declaration)?;
        if !inferred
            .effects_not_covered_by(&contract.contract_effects)
            .is_empty()
        {
            return Err(format!(
                "Agent controller inferred effects {inferred} exceed declared policy {}",
                contract.contract_effects
            ));
        }
        Ok((contract.clone(), contract.contract_effects, inferred))
    }

    pub(super) fn flow(
        &self,
        module: &HirModule,
        flow: &HirFlow,
        state: &BoundNominalTypeKey,
    ) -> Result<CanonicalFlowContract, String> {
        let signature = flow
            .signature()
            .ok_or_else(|| "initial flow must declare one owned State parameter".to_owned())?;
        if !signature.generic_params().is_empty() || !signature.where_clauses().is_empty() {
            return Err("initial flow must not declare generics or where predicates".to_owned());
        }
        let [group] = signature.param_groups() else {
            return Err("initial flow must declare exactly one parameter group".to_owned());
        };
        let [parameter] = group.params() else {
            return Err("initial flow must take exactly one owned State parameter".to_owned());
        };
        if parameter.is_rest()
            || parameter.default().is_some()
            || parameter.receiver_kind().is_some()
        {
            return Err(
                "initial flow State parameter must be fixed, required, and non-receiver".to_owned(),
            );
        }
        let parameter_type = self.canonical_authored_type(
            module,
            parameter
                .ty()
                .ok_or_else(|| "initial flow State parameter must declare a type".to_owned())?,
        )?;
        let expected_state = CanonicalType::Nominal(state.clone());
        if parameter_type != expected_state {
            return Err(format!(
                "initial flow parameter must be `{}`, found `{}`",
                expected_state.source_label(),
                parameter_type.source_label()
            ));
        }
        let signature = CanonicalSignature {
            generics: Vec::new(),
            groups: vec![CanonicalParameterGroup {
                kind: crate::callable::CallableGroupKind::Initial,
                parameters: vec![CanonicalParameter {
                    passing: CallableParameterPassing::PositionalOrNamed,
                    presence: CallableParameterPresence::Required,
                    receiver: 0,
                    ty: parameter_type,
                }],
            }],
            result: Some(match signature.return_type() {
                Some(result) => self.canonical_authored_type(module, result)?,
                None => CanonicalType::Atomic(CanonicalAtomic::Unit),
            }),
            where_predicates: Vec::new(),
        };
        let contract_effects = self.flow_effects(flow)?;
        Ok(CanonicalFlowContract {
            signature: Some(signature),
            contract_effects,
            suspension: CanonicalFlowSuspension::Flow,
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "canonical callable construction keeps accepted schema and authored signature cross-checks in one boundary"
    )]
    fn callable(
        &self,
        module: &HirModule,
        function: &HirFunction,
        record: &CallableRecord,
    ) -> Result<(CanonicalCallableContract, bool), String> {
        let schema = record.schema();
        let surface = function.signature();
        if schema.groups().len() != surface.param_groups().len() {
            return Err(
                "accepted callable schema disagrees with source parameter groups".to_owned(),
            );
        }
        let mut groups = Vec::with_capacity(schema.groups().len());
        for (schema_group, source_group) in schema.groups().iter().zip(surface.param_groups()) {
            if schema_group.parameters().len() != source_group.params().len() {
                return Err("accepted callable schema disagrees with source parameters".to_owned());
            }
            let parameters = schema_group
                .parameters()
                .iter()
                .zip(source_group.params())
                .map(|(schema_parameter, source_parameter)| {
                    let CallableParameterType::Exact(schema_type) = schema_parameter.ty() else {
                        return Err(
                            "entry role callable has an unchecked parameter type".to_owned()
                        );
                    };
                    let authored = source_parameter
                        .ty()
                        .ok_or_else(|| "entry role parameter must declare a type".to_owned())?;
                    let checked = self.checked_authored_type(module, authored)?;
                    if checked != schema_type {
                        return Err(
                            "accepted callable schema disagrees with checked source parameter type"
                                .to_owned(),
                        );
                    }
                    Ok(CanonicalParameter {
                        passing: schema_parameter.passing(),
                        presence: schema_parameter.presence(),
                        receiver: receiver_tag(source_parameter.receiver_kind()),
                        ty: self.canonical_type_kind(checked)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            groups.push(CanonicalParameterGroup {
                kind: schema_group.kind(),
                parameters,
            });
        }
        let generics = surface
            .generic_params()
            .iter()
            .map(|generic| match generic {
                GenericParam::Lifetime(lifetime) => Ok(CanonicalGenericParameter::Lifetime(
                    lifetime.name().to_owned(),
                )),
                GenericParam::Type(parameter) => Ok(CanonicalGenericParameter::Type {
                    name: parameter.name().as_str().to_owned(),
                    bounds: parameter
                        .bounds()
                        .iter()
                        .map(|bound| self.canonical_authored_type(module, bound))
                        .collect::<Result<Vec<_>, _>>()?,
                }),
            })
            .collect::<Result<Vec<_>, String>>()?;
        let where_predicates = surface
            .where_clauses()
            .iter()
            .map(|predicate| {
                Ok(CanonicalWherePredicate {
                    subject: self.canonical_authored_type(module, predicate.subject())?,
                    bounds: predicate
                        .bounds()
                        .iter()
                        .map(|bound| self.canonical_authored_type(module, bound))
                        .collect::<Result<Vec<_>, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let checked_result = match surface.return_type() {
            Some(result) => self.checked_authored_type(module, result)?,
            None if schema.result() == &TypeKind::Unit => schema.result(),
            None => {
                return Err(
                    "accepted callable schema disagrees with an omitted Unit source result"
                        .to_owned(),
                );
            }
        };
        if checked_result != schema.result() {
            return Err(
                "accepted callable schema disagrees with checked source result type".to_owned(),
            );
        }
        let result = Some(self.canonical_type_kind(checked_result)?);
        let declared = schema.effects().declared();
        if declared.tail() != EffectRowTail::Closed {
            return Err("entry role callable effect row must be closed".to_owned());
        }
        Ok((
            CanonicalCallableContract {
                signature: CanonicalSignature {
                    generics,
                    groups,
                    result,
                    where_predicates,
                },
                contract_effects: declared.concrete().clone(),
            },
            function
                .contracts()
                .iter()
                .any(|contract| matches!(contract, ContractClause::Effects(_))),
        ))
    }

    fn flow_effects(&self, flow: &HirFlow) -> Result<EffectSet, String> {
        let id = flow
            .id()
            .ok_or_else(|| "initial flow must retain a canonical source ID".to_owned())?;
        let name = flow
            .name()
            .ok_or_else(|| "initial flow must retain its source-level name".to_owned())?;
        let callable = CallableId::source_flow(name);
        let summary = self
            .typecheck
            .effects
            .effect_rows()
            .summary(&callable)
            .ok_or_else(|| format!("initial flow `{}` has no effect-row evidence", id.body()))?;
        let row = summary.upper_bound().unwrap_or_else(|| summary.inferred());
        self.typecheck
            .effects
            .resolve_effect_row(row)
            .map_err(|error| format!("cannot close initial flow effect contract: {error}"))
    }

    fn require_inferred_empty(
        &self,
        declaration: &CallableDeclarationId,
        role: &str,
    ) -> Result<(), String> {
        let inferred = self.inferred_effects(declaration)?;
        if inferred.is_empty() {
            Ok(())
        } else {
            Err(format!("{role} must infer no effects, found {inferred}"))
        }
    }

    fn inferred_effects(&self, declaration: &CallableDeclarationId) -> Result<EffectSet, String> {
        let callable = CallableId::project_function(declaration);
        let summary = self
            .typecheck
            .effects
            .effect_rows()
            .summary(&callable)
            .ok_or_else(|| {
                format!("accepted callable `{declaration}` has no effect-row evidence")
            })?;
        self.typecheck
            .effects
            .resolve_effect_row(summary.inferred())
            .map_err(|error| format!("cannot close effect row for `{declaration}`: {error}"))
    }

    fn checked_authored_type<'b>(
        &'b self,
        module: &HirModule,
        authored: &AuthoredTypeRef,
    ) -> Result<&'b TypeKind, String> {
        let root = super::source_span(module, *authored.root_source().whole());
        let report = self
            .typecheck
            .nominal_resolutions
            .report(&root)
            .ok_or_else(|| {
                format!("accepted type-check report has no nominal-resolution fact for {root:?}")
            })?;
        match report.outcome() {
            ResolvedTypeRefOutcome::Complete(product) => Ok(product.recovered()),
            ResolvedTypeRefOutcome::Poisoned(poisoned) => Err(format!(
                "entry contract type is poisoned by {} nominal error(s)",
                poisoned.causes().len()
            )),
            ResolvedTypeRefOutcome::Detached(_) => {
                Err("entry contract type was resolved without accepted project evidence".to_owned())
            }
        }
    }

    fn canonical_authored_type(
        &self,
        module: &HirModule,
        authored: &AuthoredTypeRef,
    ) -> Result<CanonicalType, String> {
        self.canonical_type_kind(self.checked_authored_type(module, authored)?)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "entry contract digests exhaustively project the closed semantic type algebra"
    )]
    fn canonical_type_kind(&self, ty: &TypeKind) -> Result<CanonicalType, String> {
        let canonical = match ty {
            TypeKind::Bool => CanonicalType::Atomic(CanonicalAtomic::Bool),
            TypeKind::I8 => CanonicalType::Atomic(CanonicalAtomic::I8),
            TypeKind::I16 => CanonicalType::Atomic(CanonicalAtomic::I16),
            TypeKind::I32 => CanonicalType::Atomic(CanonicalAtomic::I32),
            TypeKind::I64 => CanonicalType::Atomic(CanonicalAtomic::I64),
            TypeKind::I128 => CanonicalType::Atomic(CanonicalAtomic::I128),
            TypeKind::ISize => CanonicalType::Atomic(CanonicalAtomic::ISize),
            TypeKind::U8 => CanonicalType::Atomic(CanonicalAtomic::U8),
            TypeKind::U16 => CanonicalType::Atomic(CanonicalAtomic::U16),
            TypeKind::U32 => CanonicalType::Atomic(CanonicalAtomic::U32),
            TypeKind::U64 => CanonicalType::Atomic(CanonicalAtomic::U64),
            TypeKind::U128 => CanonicalType::Atomic(CanonicalAtomic::U128),
            TypeKind::USize => CanonicalType::Atomic(CanonicalAtomic::USize),
            TypeKind::F32 => CanonicalType::Atomic(CanonicalAtomic::F32),
            TypeKind::F64 => CanonicalType::Atomic(CanonicalAtomic::F64),
            TypeKind::String => CanonicalType::Atomic(CanonicalAtomic::String),
            TypeKind::Char => CanonicalType::Atomic(CanonicalAtomic::Char),
            TypeKind::Bytes => CanonicalType::Atomic(CanonicalAtomic::Bytes),
            TypeKind::TextCluster => CanonicalType::Atomic(CanonicalAtomic::TextCluster),
            TypeKind::Duration => CanonicalType::Atomic(CanonicalAtomic::Duration),
            TypeKind::DebugStatePath => CanonicalType::Atomic(CanonicalAtomic::DebugStatePath),
            TypeKind::ObservationFieldPath => {
                CanonicalType::Atomic(CanonicalAtomic::ObservationFieldPath)
            }
            TypeKind::AgentValue => CanonicalType::Atomic(CanonicalAtomic::AgentValue),
            TypeKind::DataFormat => CanonicalType::Atomic(CanonicalAtomic::DataFormat),
            TypeKind::DataShape => CanonicalType::Atomic(CanonicalAtomic::DataShape),
            TypeKind::Unit => CanonicalType::Atomic(CanonicalAtomic::Unit),
            TypeKind::Never => CanonicalType::Atomic(CanonicalAtomic::Never),
            TypeKind::Named(name) => canonical_atomic(name)
                .map(CanonicalType::Atomic)
                .ok_or_else(|| format!("unsupported entry contract atom `{name}`"))?,
            TypeKind::ProjectNominal(nominal) if nominal.arguments().is_empty() => {
                let declaration = nominal.declaration();
                let kind = match declaration.kind() {
                    arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::Struct => {
                        BoundNominalKind::Struct
                    }
                    arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::Enum => {
                        BoundNominalKind::Enum
                    }
                    arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::TypeAlias => {
                        return Err(
                            "normalized entry contract type retained an alias identity".to_owned()
                        );
                    }
                };
                CanonicalType::Nominal(BoundNominalTypeKey::new(
                    self.package.clone(),
                    declaration.module().clone(),
                    declaration.name().as_str(),
                    kind,
                ))
            }
            TypeKind::ProjectNominal(nominal) => {
                return Err(format!(
                    "generic project nominal `{}` is not a canonical entry role contract",
                    nominal.declaration().qualified_name()
                ));
            }
            TypeKind::AcceptedNominal(nominal) => {
                let constructor =
                    crate::types::direct_type_name(nominal.declaration().canonical_path())
                        .and_then(canonical_constructor)
                        .ok_or_else(|| {
                            format!(
                                "accepted nominal `{}` has no entry contract constructor",
                                nominal.declaration().canonical_path()
                            )
                        })?;
                CanonicalType::Applied {
                    constructor,
                    args: nominal
                        .arguments()
                        .iter()
                        .map(|argument| self.canonical_type_kind(argument))
                        .collect::<Result<Vec<_>, _>>()?,
                }
            }
            TypeKind::Vec(inner) => {
                self.canonical_application(CanonicalConstructor::Vec, [inner.as_ref()])?
            }
            TypeKind::Array { item, len } => {
                let length = match len {
                    ArrayLength::Const(value) => CanonicalType::ConstInt(
                        u64::try_from(*value)
                            .map_err(|_| "array length does not fit canonical u64".to_owned())?,
                    ),
                    ArrayLength::Generic(parameter) => {
                        CanonicalType::Named(format!("generic#{}", parameter.ordinal()))
                    }
                    ArrayLength::Error(poison) => {
                        return Err(format!(
                            "poisoned array length {} is not an entry contract",
                            poison.index()
                        ));
                    }
                    ArrayLength::Inferred => {
                        return Err("inferred array length is not an entry contract".to_owned());
                    }
                };
                CanonicalType::Applied {
                    constructor: CanonicalConstructor::Array,
                    args: vec![self.canonical_type_kind(item)?, length],
                }
            }
            TypeKind::Slice(inner) => {
                self.canonical_application(CanonicalConstructor::Slice, [inner.as_ref()])?
            }
            TypeKind::Seq(inner) => {
                self.canonical_application(CanonicalConstructor::Seq, [inner.as_ref()])?
            }
            TypeKind::Map { kind, key, value } => self.canonical_application(
                match kind {
                    MapKind::Ordered => CanonicalConstructor::OrderedMap,
                    MapKind::Sorted => CanonicalConstructor::SortedMap,
                    MapKind::BTree => CanonicalConstructor::BTreeMap,
                },
                [key.as_ref(), value.as_ref()],
            )?,
            TypeKind::Result { ok, error } => self.canonical_application(
                CanonicalConstructor::Result,
                [ok.as_ref(), error.as_ref()],
            )?,
            TypeKind::Option(inner) => {
                self.canonical_application(CanonicalConstructor::Option, [inner.as_ref()])?
            }
            TypeKind::Need { ready, error } => self.canonical_application(
                CanonicalConstructor::Need,
                [ready.as_ref(), error.as_ref()],
            )?,
            TypeKind::Stream { item, error } => self.canonical_application(
                CanonicalConstructor::Stream,
                [item.as_ref(), error.as_ref()],
            )?,
            TypeKind::Source { item, error } => self.canonical_application(
                CanonicalConstructor::Source,
                [item.as_ref(), error.as_ref()],
            )?,
            TypeKind::Probe(inner) => {
                self.canonical_application(CanonicalConstructor::Probe, [inner.as_ref()])?
            }
            TypeKind::ThreadHandle(inner) => {
                self.canonical_application(CanonicalConstructor::ThreadHandle, [inner.as_ref()])?
            }
            TypeKind::Shared(inner) => {
                self.canonical_application(CanonicalConstructor::Shared, [inner.as_ref()])?
            }
            TypeKind::BorrowRef {
                kind,
                lifetime,
                inner,
            } => CanonicalType::Borrow {
                kind: match kind {
                    BorrowKind::Shared => 1,
                    BorrowKind::Mutable => 2,
                },
                lifetime: lifetime
                    .as_ref()
                    .map(|lifetime| lifetime.as_str().to_owned()),
                inner: Box::new(self.canonical_type_kind(inner)?),
            },
            TypeKind::Function {
                params,
                return_type,
                effects,
            } if effects.tail() == EffectRowTail::Closed => CanonicalType::Function {
                params: params
                    .iter()
                    .map(|parameter| self.canonical_type_kind(parameter))
                    .collect::<Result<Vec<_>, _>>()?,
                result: Box::new(self.canonical_type_kind(return_type)?),
                effects: CanonicalEffectRow {
                    effects: effects.concrete().to_labels(),
                    tail: 0,
                },
            },
            TypeKind::Tuple(items) => CanonicalType::Tuple(
                items
                    .iter()
                    .map(|item| self.canonical_type_kind(item))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            TypeKind::Choice(alternatives) => {
                let mut alternatives = alternatives
                    .iter()
                    .map(|alternative| self.canonical_type_kind(alternative))
                    .collect::<Result<Vec<_>, _>>()?;
                alternatives.sort();
                alternatives.dedup();
                CanonicalType::Choice(alternatives)
            }
            TypeKind::GenericParam(parameter) => {
                CanonicalType::Named(format!("generic#{}", parameter.ordinal()))
            }
            TypeKind::Error(poison) => {
                return Err(format!(
                    "poisoned type {} is not an accepted entry contract",
                    poison.index()
                ));
            }
            unsupported => {
                return Err(format!(
                    "checked type `{}` is not supported by entry role contracts",
                    unsupported.source_label()
                ));
            }
        };
        Ok(canonical)
    }

    fn canonical_application<'b>(
        &self,
        constructor: CanonicalConstructor,
        arguments: impl IntoIterator<Item = &'b TypeKind>,
    ) -> Result<CanonicalType, String> {
        Ok(CanonicalType::Applied {
            constructor,
            args: arguments
                .into_iter()
                .map(|argument| self.canonical_type_kind(argument))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

fn require_no_generics(signature: &CanonicalSignature, role: &str) -> Result<(), String> {
    if signature.generics.is_empty() && signature.where_predicates.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{role} must not declare generics or where predicates"
        ))
    }
}

fn require_empty_parameter_group(signature: &CanonicalSignature, role: &str) -> Result<(), String> {
    match signature.groups.as_slice() {
        [group] if group.parameters.is_empty() => Ok(()),
        _ => Err(format!("{role} must declare one empty parameter group")),
    }
}

fn require_fixed_parameter(parameter: &CanonicalParameter, role: &str) -> Result<(), String> {
    if parameter.passing == CallableParameterPassing::PositionalOrNamed
        && parameter.presence == CallableParameterPresence::Required
        && parameter.receiver == 0
    {
        Ok(())
    } else {
        Err(format!(
            "{role} parameter must be fixed, required, and non-receiver"
        ))
    }
}

fn require_result(
    signature: &CanonicalSignature,
    expected: &CanonicalType,
    role: &str,
) -> Result<(), String> {
    if signature.result.as_ref() == Some(expected) {
        Ok(())
    } else {
        let actual = signature
            .result
            .as_ref()
            .map_or_else(|| "<missing>".to_owned(), CanonicalType::source_label);
        Err(format!(
            "{role} return type must be `{}`, found `{actual}`",
            expected.source_label()
        ))
    }
}

fn require_explicit_empty_effects(
    contract: &CanonicalCallableContract,
    effects_explicit: bool,
    role: &str,
) -> Result<(), String> {
    if effects_explicit && contract.contract_effects.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{role} must declare an explicit, closed, empty effect row"
        ))
    }
}

fn receiver_tag(receiver: Option<FnReceiverKind>) -> u8 {
    match receiver {
        None => 0,
        Some(FnReceiverKind::Owned) => 1,
        Some(FnReceiverKind::SharedRef) => 2,
        Some(FnReceiverKind::MutRef) => 3,
    }
}

fn canonical_atomic(path: &str) -> Option<CanonicalAtomic> {
    Some(match path {
        "bool" => CanonicalAtomic::Bool,
        "i8" => CanonicalAtomic::I8,
        "i16" => CanonicalAtomic::I16,
        "i32" => CanonicalAtomic::I32,
        "i64" => CanonicalAtomic::I64,
        "i128" => CanonicalAtomic::I128,
        "isize" => CanonicalAtomic::ISize,
        "u8" => CanonicalAtomic::U8,
        "u16" => CanonicalAtomic::U16,
        "u32" => CanonicalAtomic::U32,
        "u64" => CanonicalAtomic::U64,
        "u128" => CanonicalAtomic::U128,
        "usize" => CanonicalAtomic::USize,
        "f32" => CanonicalAtomic::F32,
        "f64" => CanonicalAtomic::F64,
        "String" => CanonicalAtomic::String,
        "char" => CanonicalAtomic::Char,
        "Bytes" => CanonicalAtomic::Bytes,
        "Unit" => CanonicalAtomic::Unit,
        "Never" => CanonicalAtomic::Never,
        "DataFormat" => CanonicalAtomic::DataFormat,
        "DataShape" => CanonicalAtomic::DataShape,
        "AgentValue" => CanonicalAtomic::AgentValue,
        "TextCluster" => CanonicalAtomic::TextCluster,
        "Duration" => CanonicalAtomic::Duration,
        "DebugStatePath" => CanonicalAtomic::DebugStatePath,
        "ObservationFieldPath" => CanonicalAtomic::ObservationFieldPath,
        "ReducerError" => CanonicalAtomic::ReducerError,
        "AgentError" => CanonicalAtomic::AgentError,
        "ArcError" => CanonicalAtomic::ArcError,
        _ => return None,
    })
}

fn canonical_constructor(path: &str) -> Option<CanonicalConstructor> {
    Some(match path {
        "Vec" => CanonicalConstructor::Vec,
        "Array" => CanonicalConstructor::Array,
        "Slice" => CanonicalConstructor::Slice,
        "Seq" => CanonicalConstructor::Seq,
        "OrderedMap" => CanonicalConstructor::OrderedMap,
        "SortedMap" => CanonicalConstructor::SortedMap,
        "BTreeMap" => CanonicalConstructor::BTreeMap,
        "Result" => CanonicalConstructor::Result,
        "Option" => CanonicalConstructor::Option,
        "Need" => CanonicalConstructor::Need,
        "Stream" => CanonicalConstructor::Stream,
        "Source" => CanonicalConstructor::Source,
        "Reduction" => CanonicalConstructor::Reduction,
        "Speaker" => CanonicalConstructor::Speaker,
        "SpeakerPreset" => CanonicalConstructor::SpeakerPreset,
        "Ref" => CanonicalConstructor::Ref,
        "Probe" => CanonicalConstructor::Probe,
        "ThreadHandle" => CanonicalConstructor::ThreadHandle,
        "Shared" => CanonicalConstructor::Shared,
        _ => return None,
    })
}
