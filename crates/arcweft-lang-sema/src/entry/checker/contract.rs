use arcweft_lang_hir::{
    identity::ItemId,
    item::{HirFlowItem, HirFunctionItem, HirGenericParameter, HirParameterKind, HirRequiredName},
    symbol::CallablePackageId,
};
use arcweft_lang_syntax::reference::BorrowKind;

use crate::{
    callable::{
        CallableParameterPassing, CallableParameterPresence, CallableParameterType,
        CheckedCallableExecution, CheckedCallableFacts, EffectContractOrigin,
    },
    effect_row::EffectRowTail,
    effects::EffectSet,
    final_analysis::{CheckedFunctionExecution, CheckedItemRole, FinalSemanticAnalysis},
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
    analysis: &'a FinalSemanticAnalysis,
    package: &'a CallablePackageId,
}

#[derive(Clone, Copy)]
pub(super) struct ReducerContractNominals<'a> {
    pub(super) state: &'a BoundNominalTypeKey,
    pub(super) event: &'a BoundNominalTypeKey,
}

impl<'a> EntryContractBuilder<'a> {
    pub(super) const fn new(
        analysis: &'a FinalSemanticAnalysis,
        package: &'a CallablePackageId,
    ) -> Self {
        Self { analysis, package }
    }

    pub(super) fn initializer(
        &self,
        function: &HirFunctionItem,
        facts: &CheckedCallableFacts,
        state: &BoundNominalTypeKey,
    ) -> Result<CanonicalCallableContract, String> {
        require_direct_frame(facts, "initializer")?;
        let (contract, effects_explicit) = self.callable(function, facts)?;
        require_no_generics(&contract.signature, "initializer")?;
        require_empty_parameter_group(&contract.signature, "initializer")?;
        require_result(
            &contract.signature,
            &CanonicalType::Nominal(state.clone()),
            "initializer",
        )?;
        require_explicit_empty_effects(&contract, effects_explicit, "initializer")?;
        require_inferred_empty(facts, "initializer")?;
        Ok(contract)
    }

    pub(super) fn reducer(
        &self,
        function: &HirFunctionItem,
        facts: &CheckedCallableFacts,
        nominals: ReducerContractNominals<'_>,
    ) -> Result<CanonicalCallableContract, String> {
        require_direct_frame(facts, "reducer")?;
        let (contract, effects_explicit) = self.callable(function, facts)?;
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
        require_inferred_empty(facts, "reducer")?;
        Ok(contract)
    }

    pub(super) fn agent_controller(
        &self,
        function: &HirFunctionItem,
        facts: &CheckedCallableFacts,
    ) -> Result<(CanonicalCallableContract, EffectSet, EffectSet), String> {
        require_direct_frame(facts, "Agent controller")?;
        let (contract, effects_explicit) = self.callable(function, facts)?;
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
        let inferred = inferred_effects(facts, "Agent controller")?;
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
        owner: ItemId,
        flow: &HirFlowItem,
        state: &BoundNominalTypeKey,
    ) -> Result<CanonicalFlowContract, String> {
        if !flow.generic_parameters().is_empty() || !flow.where_predicates().is_empty() {
            return Err("initial flow must not declare generics or where predicates".to_owned());
        }
        let [parameter] = flow.parameters() else {
            return Err("initial flow must take exactly one owned State parameter".to_owned());
        };
        if parameter.kind() != HirParameterKind::Fixed || parameter.default().is_some() {
            return Err(
                "initial flow State parameter must be fixed, required, and non-receiver".to_owned(),
            );
        }
        let parameter_type = self.canonical_type_id(parameter.ty())?;
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
            result: Some(match flow.result().authored_type() {
                Some(result) => self.canonical_type_id(result)?,
                None => CanonicalType::Atomic(CanonicalAtomic::Unit),
            }),
            where_predicates: Vec::new(),
        };
        let checked = self
            .analysis
            .item(owner)
            .ok_or_else(|| format!("accepted final analysis has no item fact for {owner:?}"))?;
        if !matches!(checked.role(), CheckedItemRole::Flow { .. }) {
            return Err("selected initial-flow item has no checked Flow role".to_owned());
        }
        let contract_effects = checked.effects().clone();
        Ok(CanonicalFlowContract {
            signature: Some(signature),
            contract_effects,
            suspension: CanonicalFlowSuspension::Flow,
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the Entry callable-role matrix validates signature, effects, suspension, and ordinary-function role as one contract"
    )]
    fn callable(
        &self,
        function: &HirFunctionItem,
        facts: &CheckedCallableFacts,
    ) -> Result<(CanonicalCallableContract, bool), String> {
        let schema = facts.signature();
        if schema.groups().len() != function.parameter_groups().len() {
            return Err(
                "accepted callable schema disagrees with source parameter groups".to_owned(),
            );
        }
        let mut groups = Vec::with_capacity(schema.groups().len());
        for (schema_group, source_group) in schema.groups().iter().zip(function.parameter_groups())
        {
            if schema_group.parameters().len() != source_group.parameters().len() {
                return Err("accepted callable schema disagrees with source parameters".to_owned());
            }
            let parameters = schema_group
                .parameters()
                .iter()
                .zip(source_group.parameters())
                .map(|(schema_parameter, source_parameter)| {
                    let CallableParameterType::Exact(schema_type) = schema_parameter.ty() else {
                        return Err(
                            "entry role callable has an unchecked parameter type".to_owned()
                        );
                    };
                    let checked = self.checked_type(source_parameter.ty())?;
                    if checked != schema_type {
                        return Err(
                            "accepted callable schema disagrees with checked source parameter type"
                                .to_owned(),
                        );
                    }
                    let source_rest = source_parameter.kind() == HirParameterKind::RestPositional;
                    let schema_rest = matches!(
                        schema_parameter.passing(),
                        CallableParameterPassing::RestPositional
                            | CallableParameterPassing::RestNamed
                    );
                    if source_rest != schema_rest
                        || source_parameter.default().is_some()
                            != (schema_parameter.presence() == CallableParameterPresence::Defaulted)
                    {
                        return Err(
                            "accepted callable schema disagrees with source parameter arity"
                                .to_owned(),
                        );
                    }
                    Ok(CanonicalParameter {
                        passing: schema_parameter.passing(),
                        presence: schema_parameter.presence(),
                        receiver: 0,
                        ty: self.canonical_type_kind(checked)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            groups.push(CanonicalParameterGroup {
                kind: schema_group.kind(),
                parameters,
            });
        }
        let generics = function
            .generic_parameters()
            .iter()
            .map(|generic| match generic {
                HirGenericParameter::Lifetime { name } => Ok(CanonicalGenericParameter::Lifetime(
                    required_name(name)?.to_owned(),
                )),
                HirGenericParameter::Type { name, bounds } => Ok(CanonicalGenericParameter::Type {
                    name: required_name(name)?.to_owned(),
                    bounds: bounds
                        .iter()
                        .map(|bound| self.canonical_type_id(*bound))
                        .collect::<Result<Vec<_>, _>>()?,
                }),
            })
            .collect::<Result<Vec<_>, String>>()?;
        let where_predicates = function
            .where_predicates()
            .iter()
            .map(|predicate| {
                Ok(CanonicalWherePredicate {
                    subject: self.canonical_type_id(predicate.subject())?,
                    bounds: predicate
                        .bounds()
                        .iter()
                        .map(|bound| self.canonical_type_id(*bound))
                        .collect::<Result<Vec<_>, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let checked_result = match function.return_type() {
            Some(result) => self.checked_type(result)?,
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
        let declared = facts.exposed_row();
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
            facts.effect_contract_origin() == Some(EffectContractOrigin::Authored),
        ))
    }

    fn checked_type(&self, owner: arcweft_lang_hir::identity::TypeId) -> Result<&TypeKind, String> {
        self.analysis.ty(owner).ok_or_else(|| {
            format!("accepted final semantic analysis has no type fact for {owner:?}")
        })
    }

    fn canonical_type_id(
        &self,
        owner: arcweft_lang_hir::identity::TypeId,
    ) -> Result<CanonicalType, String> {
        self.canonical_type_kind(self.checked_type(owner)?)
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
                let name = crate::types::direct_type_name(nominal.declaration().canonical_path())
                    .ok_or_else(|| {
                    format!(
                        "accepted nominal `{}` has no direct entry contract identity",
                        nominal.declaration().canonical_path()
                    )
                })?;
                if nominal.arguments().is_empty() {
                    CanonicalType::Atomic(canonical_atomic(name).ok_or_else(|| {
                        format!("accepted nominal `{name}` has no entry contract atom")
                    })?)
                } else {
                    CanonicalType::Applied {
                        constructor: canonical_constructor(name).ok_or_else(|| {
                            format!("accepted nominal `{name}` has no entry contract constructor")
                        })?,
                        args: nominal
                            .arguments()
                            .iter()
                            .map(|argument| self.canonical_type_kind(argument))
                            .collect::<Result<Vec<_>, _>>()?,
                    }
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

fn required_name(name: &HirRequiredName) -> Result<&str, String> {
    match name {
        HirRequiredName::Resolved(name) => Ok(name.as_str()),
        HirRequiredName::Missing | HirRequiredName::Invalid => {
            Err("accepted executable callable retained a recovered generic name".to_owned())
        }
    }
}

fn require_direct_frame(facts: &CheckedCallableFacts, role: &str) -> Result<(), String> {
    match facts.execution() {
        CheckedCallableExecution::Runtime(CheckedFunctionExecution::DirectFrame) => Ok(()),
        CheckedCallableExecution::Runtime(CheckedFunctionExecution::StreamFactory { .. }) => {
            Err(format!("{role} must be an ordinary direct-frame function"))
        }
        CheckedCallableExecution::DispatchContract => {
            Err(format!("{role} cannot be a bodyless dispatch contract"))
        }
    }
}

fn inferred_effects(facts: &CheckedCallableFacts, role: &str) -> Result<EffectSet, String> {
    let row = facts
        .actual_row()
        .ok_or_else(|| format!("{role} has no checked body-inference effect row"))?;
    if row.tail() != EffectRowTail::Closed {
        return Err(format!(
            "{role} inferred effect row `{}` is not closed",
            row.display_label()
        ));
    }
    Ok(row.concrete().clone())
}

fn require_inferred_empty(facts: &CheckedCallableFacts, role: &str) -> Result<(), String> {
    let inferred = inferred_effects(facts, role)?;
    if inferred.is_empty() {
        Ok(())
    } else {
        Err(format!("{role} must infer no effects, found {inferred}"))
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
        "Reduction" => CanonicalConstructor::Reduction,
        "Ref" => CanonicalConstructor::Ref,
        "Probe" => CanonicalConstructor::Probe,
        "ThreadHandle" => CanonicalConstructor::ThreadHandle,
        "Shared" => CanonicalConstructor::Shared,
        _ => return None,
    })
}
