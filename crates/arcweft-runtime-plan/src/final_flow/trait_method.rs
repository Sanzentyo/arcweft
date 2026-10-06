//! Closed trait-method reservation and body definition.

use super::*;

#[derive(Clone)]
pub(super) struct ReservedTraitMethodDefinition {
    checked: RuntimeTraitMethodFact,
    method: RuntimeTraitMethodSeedId,
}

pub(super) fn reserve_trait_methods(
    project: HirAnalysisProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    closed_locals: &BTreeMap<RuntimeTraitMethodInstanceKey, ProjectFunctionFrameLocals>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<RuntimeTraitMethodInstanceKey, RuntimeTraitMethodSeedId>,
    Vec<ReservedTraitMethodDefinition>,
) {
    let mut methods = BTreeMap::new();
    let mut definitions = Vec::new();
    for (position, checked) in facts.trait_methods().enumerate() {
        let method_locals = if checked.closed_semantics().is_some() {
            let Some(frame) = closed_locals.get(checked.key()) else {
                errors.push(RuntimePlanLowerError::new(format!(
                    "trait-method instance {:?} has no admitted local frame",
                    checked.key(),
                )));
                continue;
            };
            &frame.hir
        } else {
            locals
        };
        match trait_method_declaration(project, facts, method_locals, checked, position).and_then(
            |declaration| {
                builder
                    .reserve_trait_method_seed(declaration)
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))
            },
        ) {
            Ok(method) => {
                if methods
                    .insert(checked.key().clone(), method.clone())
                    .is_some()
                {
                    errors.push(RuntimePlanLowerError::new(
                        "trait-method instance was reserved twice",
                    ));
                    continue;
                }
                definitions.push(ReservedTraitMethodDefinition {
                    checked: checked.clone(),
                    method,
                });
            }
            Err(error) => errors.push(error),
        }
    }
    (methods, definitions)
}

fn trait_method_declaration(
    project: HirAnalysisProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    checked: &RuntimeTraitMethodFact,
    witness: usize,
) -> Result<RuntimeTraitMethodDeclarationSeed, RuntimePlanLowerError> {
    let (module, function) = resolve_trait_method(project, checked)?;
    let method_name = function
        .name()
        .resolved()
        .ok_or_else(|| RuntimePlanLowerError::new("runtime trait method has no resolved name"))?;
    let mut receiver = None;
    let mut inputs = Vec::new();
    let formals = checked.definition().parameters();
    let parameters = function
        .parameter_groups()
        .iter()
        .flat_map(HirMethodParameterGroup::parameters)
        .collect::<Vec<_>>();
    if parameters.len() != formals.len() {
        return Err(RuntimePlanLowerError::new(
            "trait method formal count disagrees with accepted definition",
        ));
    }
    for (parameter, formal) in parameters.into_iter().zip(formals) {
        let (local, pattern, abi) = match parameter {
            HirMethodParameter::Receiver(parameter) => {
                if receiver.is_some() {
                    return Err(RuntimePlanLowerError::new(
                        "runtime trait method has more than one receiver",
                    ));
                }
                receiver = Some(match parameter.kind() {
                    HirMethodReceiverKind::Owned => RuntimeReceiverMode::Owned,
                    HirMethodReceiverKind::SharedReference => RuntimeReceiverMode::SharedRef,
                    HirMethodReceiverKind::MutableReference => RuntimeReceiverMode::MutRef,
                });
                let [local] = parameter.locals() else {
                    return Err(RuntimePlanLowerError::new(
                        "runtime trait receiver requires one binding",
                    ));
                };
                (*local, parameter.pattern(), RuntimePureInputType::Value)
            }
            HirMethodParameter::Typed(parameter) => {
                if parameter.kind() != HirParameterKind::Fixed
                    || parameter.default().is_some()
                    || parameter.locals().len() != 1
                {
                    return Err(RuntimePlanLowerError::new(
                        "runtime trait method requires fixed single-binding parameters",
                    ));
                }
                let ty = match checked.closed_semantics() {
                    Some(semantics) => {
                        semantics.ty(RuntimeProjectFunctionTypeOwner::Type(parameter.ty()))
                    }
                    None => facts.ty(parameter.ty()),
                }
                .ok_or_else(|| {
                    RuntimePlanLowerError::new("runtime trait parameter type fact is missing")
                })?;
                (
                    parameter.locals()[0],
                    parameter.pattern(),
                    runtime_input_type(ty.shape()),
                )
            }
        };
        if formal.pattern() != Some(pattern) || formal.bindings() != [local] {
            return Err(RuntimePlanLowerError::new(
                "trait method local disagrees with accepted whole formal",
            ));
        }
        inputs.push(RuntimeCallableParameterSeed {
            identity: formal.identity().runtime_identity(),
            local: locals
                .get(&local)
                .cloned()
                .ok_or_else(|| RuntimePlanLowerError::new("trait method local is not admitted"))?,
            passing: formal.passing(),
            abi,
        });
    }
    let receiver = receiver
        .ok_or_else(|| RuntimePlanLowerError::new("runtime trait method requires a receiver"))?;
    let body = function_body_expression(
        function
            .body()
            .ok_or_else(|| RuntimePlanLowerError::new("runtime trait method has no body"))?,
    )?;
    let result = match checked.closed_semantics() {
        Some(semantics) => semantics.expression_type(body),
        None => facts.expression_type(body),
    }
    .ok_or_else(|| {
        RuntimePlanLowerError::new("runtime trait method body has no accepted runtime type")
    })?;
    let output_abi = function
        .return_type()
        .and_then(|ty| match checked.closed_semantics() {
            Some(semantics) => semantics.ty(RuntimeProjectFunctionTypeOwner::Type(ty)),
            None => facts.ty(ty),
        })
        .map_or(RuntimePureOutputType::Value, |ty| {
            runtime_output_type(ty.shape())
        });
    let impl_id = project
        .items()
        .position(|item| item.id() == checked.implementation())
        .ok_or_else(|| RuntimePlanLowerError::new("runtime trait Impl owner is absent"))?;
    let (trait_id, trait_name) = lower_runtime_trait_identity(project, checked.trait_identity())?;
    let _ = module;
    Ok(RuntimeTraitMethodDeclarationSeed {
        definition: checked
            .definition()
            .definition_identity()
            .runtime_identity(),
        identity: RuntimeTraitMethodIdentity {
            impl_id,
            trait_id,
            witness: Some(witness),
            trait_name,
            self_type: semantic_type_label(checked.self_type()),
            method_name: method_name.as_str().to_owned(),
            monomorph_label: format!(
                "{}::{}",
                semantic_type_label(checked.self_type()),
                method_name.as_str()
            ),
        },
        receiver,
        inputs: inputs.into_boxed_slice(),
        result: result.identity(),
        output_abi,
    })
}

fn semantic_type_label(ty: &crate::semantic_facts::RuntimeNormalizedType) -> String {
    let mut label = String::with_capacity(64);
    for byte in ty.identity().as_bytes() {
        use std::fmt::Write as _;
        write!(&mut label, "{byte:02x}").expect("writing to String cannot fail");
    }
    label
}

fn resolve_trait_method<'a>(
    project: HirAnalysisProjectView<'a>,
    checked: &RuntimeTraitMethodFact,
) -> Result<(&'a HirModule, &'a HirImplFunction), RuntimePlanLowerError> {
    let module = project
        .modules()
        .find_map(|(_, module)| {
            (module.module_id() == checked.implementation().module()).then_some(module)
        })
        .ok_or_else(|| RuntimePlanLowerError::new("runtime trait method module is absent"))?;
    let item = module
        .resolve_item(checked.implementation())
        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
    let HirItemKind::Impl(implementation) = item.kind() else {
        return Err(RuntimePlanLowerError::new(
            "checked runtime trait method owner is not an Impl",
        ));
    };
    let Some(HirImplMember::Function(function)) =
        implementation.members().get(usize::from(checked.member()))
    else {
        return Err(RuntimePlanLowerError::new(
            "checked runtime trait method member is not a function",
        ));
    };
    Ok((module, function))
}

pub(super) fn define_trait_methods(
    context: &FinalLoweringContext<'_, '_>,
    definitions: &[ReservedTraitMethodDefinition],
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    for definition in definitions {
        let Ok((module, function)) = resolve_trait_method(context.project, &definition.checked)
        else {
            errors.push(RuntimePlanLowerError::new("trait method owner is absent"));
            continue;
        };
        let Some(body_owner) = function.body() else {
            errors.push(RuntimePlanLowerError::new(
                "runtime trait method has no body",
            ));
            continue;
        };
        let body = match definition.checked.closed_semantics() {
            Some(semantics) => context
                .scoped_expr_lowerer(
                    module,
                    RuntimeScopedExecutableSemanticFactView::trait_method(
                        definition.checked.key(),
                        semantics,
                    ),
                )
                .map_err(|error| error.to_string())
                .and_then(|lowerer| lowerer.lower_function_body(body_owner)),
            None => context.expr_lowerer(module).lower_function_body(body_owner),
        };
        match body.and_then(|body| {
            builder
                .define_trait_method_seed(&definition.method, body)
                .map_err(|error| error.to_string())
        }) {
            Ok(()) => {}
            Err(error) => errors.push(RuntimePlanLowerError::new(error)),
        }
    }
}

fn lower_runtime_trait_identity(
    project: HirAnalysisProjectView<'_>,
    identity: &RuntimeTraitIdentity,
) -> Result<(Option<usize>, Option<String>), RuntimePlanLowerError> {
    Ok(match identity {
        RuntimeTraitIdentity::Project(owner) => {
            let (position, trait_owner) = project
                .items()
                .enumerate()
                .find(|(_, item)| item.id() == *owner)
                .ok_or_else(|| RuntimePlanLowerError::new("runtime Trait owner is absent"))?;
            let trait_item = trait_owner
                .module()
                .resolve_item(*owner)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
            let HirItemKind::Trait(trait_item) = trait_item.kind() else {
                return Err(RuntimePlanLowerError::new(
                    "runtime Trait identity does not own a Trait item",
                ));
            };
            let name = trait_item
                .name()
                .resolved()
                .ok_or_else(|| RuntimePlanLowerError::new("runtime Trait has no resolved name"))?;
            (Some(position), Some(name.as_str().to_owned()))
        }
        RuntimeTraitIdentity::StandardIterator => (None, Some("Iterator".to_owned())),
        RuntimeTraitIdentity::StandardIntoIterator => (None, Some("IntoIterator".to_owned())),
        RuntimeTraitIdentity::StandardDisplayText => (None, Some("DisplayText".to_owned())),
    })
}
