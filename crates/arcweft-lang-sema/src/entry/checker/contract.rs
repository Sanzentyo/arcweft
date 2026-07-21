use std::collections::BTreeSet;

use arcweft_lang_hir::{
    model::{HirFlow, HirFunction, HirModule},
    symbol::CallableDeclarationId,
};
use arcweft_lang_syntax::{
    ast::{flow::ContractClause, items::FunctionKind, module_path::CanonicalModulePath},
    reference::BorrowKind,
    types::{FnReceiverKind, GenericParam, TypeRef},
};

use crate::{
    callable::{
        CallableParameterPassing, CallableParameterPresence, CallableParameterType, CallableRecord,
    },
    check::TypeCheckReport,
    effect_model::CallableId,
    effect_row::EffectRowTail,
    effects::EffectSet,
};

use super::{
    BoundNominalTypeKey, NominalSchemaResolver,
    digest::{
        CanonicalAtomic, CanonicalCallableContract, CanonicalConstructor, CanonicalEffectRow,
        CanonicalFlowContract, CanonicalFlowSuspension, CanonicalGenericParameter,
        CanonicalParameter, CanonicalParameterGroup, CanonicalSignature, CanonicalType,
        CanonicalWherePredicate,
    },
};

pub(super) struct EntryContractBuilder<'a> {
    nominals: &'a NominalSchemaResolver<'a>,
    typecheck: &'a TypeCheckReport,
}

#[derive(Clone, Copy)]
pub(super) struct ReducerContractNominals<'a> {
    pub(super) state: &'a BoundNominalTypeKey,
    pub(super) event: &'a BoundNominalTypeKey,
}

impl<'a> EntryContractBuilder<'a> {
    pub(super) const fn new(
        nominals: &'a NominalSchemaResolver<'a>,
        typecheck: &'a TypeCheckReport,
    ) -> Self {
        Self {
            nominals,
            typecheck,
        }
    }

    pub(super) fn initializer(
        &self,
        module_path: &CanonicalModulePath,
        module: &HirModule,
        function: &HirFunction,
        record: &CallableRecord,
        declaration: &CallableDeclarationId,
        state: &BoundNominalTypeKey,
    ) -> Result<CanonicalCallableContract, String> {
        let (contract, effects_explicit) = self.callable(module_path, module, function, record)?;
        require_ordinary_function(function, "initializer")?;
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
        module_path: &CanonicalModulePath,
        module: &HirModule,
        function: &HirFunction,
        record: &CallableRecord,
        declaration: &CallableDeclarationId,
        nominals: ReducerContractNominals<'_>,
    ) -> Result<CanonicalCallableContract, String> {
        let (contract, effects_explicit) = self.callable(module_path, module, function, record)?;
        require_ordinary_function(function, "reducer")?;
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
        module_path: &CanonicalModulePath,
        module: &HirModule,
        function: &HirFunction,
        record: &CallableRecord,
        declaration: &CallableDeclarationId,
    ) -> Result<(CanonicalCallableContract, EffectSet, EffectSet), String> {
        let (contract, effects_explicit) = self.callable(module_path, module, function, record)?;
        require_ordinary_function(function, "Agent controller")?;
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
        module_path: &CanonicalModulePath,
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
        let generic_names = BTreeSet::new();
        let parameter_type = self.canonical_type_ref(
            module_path,
            module,
            parameter
                .ty()
                .ok_or_else(|| "initial flow State parameter must declare a type".to_owned())?
                .value(),
            &generic_names,
            &mut BTreeSet::new(),
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
                Some(result) => self.canonical_type_ref(
                    module_path,
                    module,
                    result.value(),
                    &generic_names,
                    &mut BTreeSet::new(),
                )?,
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
        module_path: &CanonicalModulePath,
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
        let generic_names = surface
            .generic_params()
            .iter()
            .filter_map(GenericParam::as_type)
            .map(|name| name.as_str().to_owned())
            .collect::<BTreeSet<_>>();
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
                    if !matches!(schema_parameter.ty(), CallableParameterType::Exact(_)) {
                        return Err(
                            "entry role callable has an unchecked parameter type".to_owned()
                        );
                    }
                    Ok(CanonicalParameter {
                        passing: schema_parameter.passing(),
                        presence: schema_parameter.presence(),
                        receiver: receiver_tag(source_parameter.receiver_kind()),
                        ty: self.canonical_type_ref(
                            module_path,
                            module,
                            source_parameter
                                .ty()
                                .ok_or_else(|| {
                                    "entry role parameter must declare a type".to_owned()
                                })?
                                .value(),
                            &generic_names,
                            &mut BTreeSet::new(),
                        )?,
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
                        .map(|bound| {
                            self.canonical_type_ref(
                                module_path,
                                module,
                                bound.value(),
                                &generic_names,
                                &mut BTreeSet::new(),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                }),
            })
            .collect::<Result<Vec<_>, String>>()?;
        let where_predicates = surface
            .where_clauses()
            .iter()
            .map(|predicate| {
                Ok(CanonicalWherePredicate {
                    subject: self.canonical_type_ref(
                        module_path,
                        module,
                        predicate.subject().value(),
                        &generic_names,
                        &mut BTreeSet::new(),
                    )?,
                    bounds: predicate
                        .bounds()
                        .iter()
                        .map(|bound| {
                            self.canonical_type_ref(
                                module_path,
                                module,
                                bound.value(),
                                &generic_names,
                                &mut BTreeSet::new(),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let result = Some(match surface.return_type() {
            Some(result) => self.canonical_type_ref(
                module_path,
                module,
                result.value(),
                &generic_names,
                &mut BTreeSet::new(),
            )?,
            None => CanonicalType::Atomic(CanonicalAtomic::Unit),
        });
        let declared = schema.effects().declared();
        if declared.tail() != EffectRowTail::Closed {
            return Err("entry role callable effect row must be closed".to_owned());
        }
        Ok((
            CanonicalCallableContract {
                kind: function.kind(),
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

    #[allow(
        clippy::too_many_lines,
        reason = "the recursive closed TypeRef normalization is one exhaustive canonicalization rule"
    )]
    fn canonical_type_ref(
        &self,
        current: &CanonicalModulePath,
        module: &HirModule,
        ty: &TypeRef,
        generic_names: &BTreeSet<String>,
        alias_stack: &mut BTreeSet<(CanonicalModulePath, String)>,
    ) -> Result<CanonicalType, String> {
        match ty {
            TypeRef::Never => Ok(CanonicalType::Atomic(CanonicalAtomic::Never)),
            TypeRef::ConstInt(value) => u64::try_from(*value)
                .map(CanonicalType::ConstInt)
                .map_err(|_| "const type argument does not fit canonical u64".to_owned()),
            TypeRef::Path(path)
                if crate::types::direct_type_name(path)
                    .is_some_and(|name| generic_names.contains(name)) =>
            {
                Ok(CanonicalType::Named(
                    crate::types::direct_type_name(path)
                        .expect("guard requires a direct generic name")
                        .to_owned(),
                ))
            }
            TypeRef::Path(path) => {
                let path_label = path.canonical_string();
                if let Ok(record) = self.nominals.resolve_nominal(current, module, &path_label) {
                    return Ok(CanonicalType::Nominal(record.key.clone()));
                }
                if let Some((alias_module, alias_source, target, alias_name)) = self
                    .nominals
                    .resolve_alias_target(current, module, &path_label)?
                {
                    let key = (alias_module.clone(), alias_name.clone());
                    if !alias_stack.insert(key.clone()) {
                        return Err(format!("recursive type alias `{alias_name}`"));
                    }
                    let canonical = self.canonical_type_ref(
                        alias_module,
                        alias_source,
                        target.value(),
                        generic_names,
                        alias_stack,
                    );
                    alias_stack.remove(&key);
                    return canonical;
                }
                if let Some(atomic) =
                    crate::types::direct_type_name(path).and_then(canonical_atomic)
                {
                    return Ok(CanonicalType::Atomic(atomic));
                }
                Err(format!("unresolved canonical type `{path_label}`"))
            }
            TypeRef::Tuple(items) => items
                .iter()
                .map(|item| {
                    self.canonical_type_ref(current, module, item, generic_names, alias_stack)
                })
                .collect::<Result<Vec<_>, _>>()
                .map(CanonicalType::Tuple),
            TypeRef::Function {
                params,
                return_type,
                effects,
            } => Ok(CanonicalType::Function {
                params: params
                    .iter()
                    .map(|param| {
                        self.canonical_type_ref(current, module, param, generic_names, alias_stack)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                result: Box::new(self.canonical_type_ref(
                    current,
                    module,
                    return_type,
                    generic_names,
                    alias_stack,
                )?),
                effects: {
                    let effects = effects.as_ref().ok_or_else(|| {
                        "function type in an entry contract must declare a closed effect row"
                            .to_owned()
                    })?;
                    let effects = EffectSet::from_labels(effects.effects())
                        .map_err(|error| error.to_string())?;
                    CanonicalEffectRow {
                        effects: effects.to_labels(),
                        tail: 0,
                    }
                },
            }),
            TypeRef::Choice(alternatives) => {
                let mut alternatives = alternatives
                    .iter()
                    .map(|alternative| {
                        self.canonical_type_ref(
                            current,
                            module,
                            alternative,
                            generic_names,
                            alias_stack,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                alternatives.sort();
                alternatives.dedup();
                Ok(CanonicalType::Choice(alternatives))
            }
            TypeRef::Generic { base, args } => {
                let base_label = base.canonical_string();
                if self
                    .nominals
                    .resolve_nominal(current, module, &base_label)
                    .is_ok()
                    || self
                        .nominals
                        .resolve_alias_target(current, module, &base_label)?
                        .is_some()
                {
                    return Err(format!(
                        "project type `{base_label}` cannot be interpreted as a prelude constructor"
                    ));
                }
                let constructor = crate::types::direct_type_name(base)
                    .and_then(canonical_constructor)
                    .ok_or_else(|| {
                        format!("unresolved canonical type constructor `{base_label}`")
                    })?;
                Ok(CanonicalType::Applied {
                    constructor,
                    args: args
                        .iter()
                        .map(|arg| {
                            self.canonical_type_ref(
                                current,
                                module,
                                arg,
                                generic_names,
                                alias_stack,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                })
            }
            TypeRef::TraitBound(_) => Err(
                "entry role contracts do not accept unresolved trait-bound type syntax".to_owned(),
            ),
            TypeRef::Projection { .. } => Err(
                "entry role contracts do not accept unresolved associated-type projections"
                    .to_owned(),
            ),
            TypeRef::Reference(reference) => Ok(CanonicalType::Borrow {
                kind: match reference.kind() {
                    BorrowKind::Shared => 1,
                    BorrowKind::Mutable => 2,
                },
                lifetime: reference
                    .region()
                    .name()
                    .map(|lifetime| lifetime.name().to_owned()),
                inner: Box::new(self.canonical_type_ref(
                    current,
                    module,
                    reference.referent(),
                    generic_names,
                    alias_stack,
                )?),
            }),
            TypeRef::Slice(inner) => Ok(CanonicalType::Applied {
                constructor: CanonicalConstructor::Slice,
                args: vec![self.canonical_type_ref(
                    current,
                    module,
                    inner,
                    generic_names,
                    alias_stack,
                )?],
            }),
            TypeRef::Recovery(id) => Err(format!(
                "recovered type node {} is not an accepted entry contract",
                id.index()
            )),
        }
    }
}

fn require_ordinary_function(function: &HirFunction, role: &str) -> Result<(), String> {
    if function.kind() == FunctionKind::Function {
        Ok(())
    } else {
        Err(format!(
            "{role} must resolve to an ordinary `fn` declaration"
        ))
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
        "ArcResult" => CanonicalConstructor::ArcResult,
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
