//! Checked formatter projection and template identity for one runtime generation.

use super::*;

/// Selected standard formatter call. The checked semantic fact owns parameter
/// identity, display admission, and failure policy. Authored source text is
/// retained for the canonical per-call Content template; its dense template
/// identity is allocated only after all executable instances are known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedFormatCall {
    checked: arcweft_lang_sema::callable::CheckedFmtCall,
    display_witness: arcweft_lang_sema::checked_rich_text::CheckedDisplayWitness,
    project_self_type: Option<RuntimeNormalizedType>,
    call_source: Box<str>,
    value_source: Box<str>,
}

impl RuntimeResolvedFormatCall {
    pub fn new(
        checked: arcweft_lang_sema::callable::CheckedFmtCall,
        display_witness: arcweft_lang_sema::checked_rich_text::CheckedDisplayWitness,
        project_self_type: Option<RuntimeNormalizedType>,
        call_source: impl Into<Box<str>>,
        value_source: impl Into<Box<str>>,
    ) -> Self {
        Self {
            checked,
            display_witness,
            project_self_type,
            call_source: call_source.into(),
            value_source: value_source.into(),
        }
    }

    pub const fn checked(&self) -> &arcweft_lang_sema::callable::CheckedFmtCall {
        &self.checked
    }

    pub fn admits_primary(&self, ty: &RuntimeNormalizedType) -> bool {
        format_witness_admits(&self.display_witness, self.project_self_type.as_ref(), ty)
    }

    pub const fn display_witness(
        &self,
    ) -> &arcweft_lang_sema::checked_rich_text::CheckedDisplayWitness {
        &self.display_witness
    }

    pub const fn project_self_type(&self) -> Option<&RuntimeNormalizedType> {
        self.project_self_type.as_ref()
    }

    pub const fn coordinate(
        &self,
    ) -> &arcweft_lang_sema::semantic_coordinate::StableCheckedValueCoordinate {
        self.checked.call_source().coordinate()
    }

    pub const fn call_source(&self) -> &str {
        &self.call_source
    }

    pub const fn value_source(&self) -> &str {
        &self.value_source
    }
}

/// Owned lexical scope of a checked formatter occurrence. Distinct closed
/// instances and nested closures must never share a generated template ID.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RuntimeFormatExecutableScope {
    Global,
    Program(arcweft_id::runtime_program::RuntimePureProgramId),
    ProjectFunction(RuntimeProjectFunctionInstanceKey),
    Closure(RuntimeClosureInstanceKey),
    TraitMethod(RuntimeTraitMethodInstanceKey),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RuntimeFormatTemplateKey {
    scope: RuntimeFormatExecutableScope,
    coordinate: arcweft_lang_sema::semantic_coordinate::StableCheckedValueCoordinate,
}

impl RuntimeFormatTemplateKey {
    pub fn new(
        scope: RuntimeFormatExecutableScope,
        coordinate: arcweft_lang_sema::semantic_coordinate::StableCheckedValueCoordinate,
    ) -> Self {
        Self { scope, coordinate }
    }

    pub const fn scope(&self) -> &RuntimeFormatExecutableScope {
        &self.scope
    }

    pub const fn coordinate(
        &self,
    ) -> &arcweft_lang_sema::semantic_coordinate::StableCheckedValueCoordinate {
        &self.coordinate
    }

    pub fn for_call(
        scope: RuntimeExecutableSemanticScope<'_>,
        call: &RuntimeResolvedFormatCall,
    ) -> Self {
        Self::new(Self::scope_for_executable(scope), call.coordinate().clone())
    }

    pub(super) fn scope_for_executable(
        scope: RuntimeExecutableSemanticScope<'_>,
    ) -> RuntimeFormatExecutableScope {
        match scope {
            RuntimeExecutableSemanticScope::Global => RuntimeFormatExecutableScope::Global,
            RuntimeExecutableSemanticScope::Program(program) => {
                RuntimeFormatExecutableScope::Program(program)
            }
            RuntimeExecutableSemanticScope::ProjectFunction(key) => {
                RuntimeFormatExecutableScope::ProjectFunction(key.clone())
            }
            RuntimeExecutableSemanticScope::Closure(key) => {
                RuntimeFormatExecutableScope::Closure(key.clone())
            }
            RuntimeExecutableSemanticScope::TraitMethod(key) => {
                RuntimeFormatExecutableScope::TraitMethod(key.clone())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFormatTemplateFact {
    key: RuntimeFormatTemplateKey,
    template: arcweft_text_model::DialogueContentFragmentTemplate,
}

impl RuntimeFormatTemplateFact {
    pub fn new(
        key: RuntimeFormatTemplateKey,
        template: arcweft_text_model::DialogueContentFragmentTemplate,
    ) -> Self {
        Self { key, template }
    }

    pub const fn key(&self) -> &RuntimeFormatTemplateKey {
        &self.key
    }

    pub const fn template(&self) -> &arcweft_text_model::DialogueContentFragmentTemplate {
        &self.template
    }
}

fn format_scalar_witness_admits(
    witness: &arcweft_lang_sema::checked_rich_text::CheckedDisplayScalar,
    ty: &RuntimeNormalizedType,
) -> bool {
    use arcweft_lang_sema::checked_rich_text::{
        CheckedDisplayFloatWidth as Float, CheckedDisplayIntegerWidth as Integer,
        CheckedDisplayScalar as Scalar,
    };
    matches!(
        (witness, ty.shape()),
        (Scalar::Unit, RuntimeTypeShape::Unit)
            | (Scalar::Bool, RuntimeTypeShape::Bool)
            | (
                Scalar::SignedInteger(Integer::Bits8),
                RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I8)
            )
            | (
                Scalar::SignedInteger(Integer::Bits16),
                RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I16)
            )
            | (
                Scalar::SignedInteger(Integer::Bits32),
                RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I32)
            )
            | (
                Scalar::SignedInteger(Integer::Bits64),
                RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I64)
            )
            | (
                Scalar::SignedInteger(Integer::Bits128),
                RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I128)
            )
            | (
                Scalar::SignedInteger(Integer::Pointer),
                RuntimeTypeShape::Signed(RuntimeSignedIntWidth::ISize)
            )
            | (
                Scalar::UnsignedInteger(Integer::Bits8),
                RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U8)
            )
            | (
                Scalar::UnsignedInteger(Integer::Bits16),
                RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U16)
            )
            | (
                Scalar::UnsignedInteger(Integer::Bits32),
                RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U32)
            )
            | (
                Scalar::UnsignedInteger(Integer::Bits64),
                RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U64)
            )
            | (
                Scalar::UnsignedInteger(Integer::Bits128),
                RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U128)
            )
            | (
                Scalar::UnsignedInteger(Integer::Pointer),
                RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::USize)
            )
            | (Scalar::Float(Float::Bits32), RuntimeTypeShape::F32)
            | (Scalar::Float(Float::Bits64), RuntimeTypeShape::F64)
            | (Scalar::String, RuntimeTypeShape::String)
            | (Scalar::Char, RuntimeTypeShape::Char)
            | (Scalar::Duration, RuntimeTypeShape::Duration)
            | (Scalar::Reference(_), RuntimeTypeShape::EntityReference)
            | (Scalar::Progress, RuntimeTypeShape::Progress)
    )
}

fn format_witness_admits(
    witness: &arcweft_lang_sema::checked_rich_text::CheckedDisplayWitness,
    project_self_type: Option<&RuntimeNormalizedType>,
    ty: &RuntimeNormalizedType,
) -> bool {
    use arcweft_lang_sema::checked_rich_text::CheckedDisplayWitness;

    match witness {
        CheckedDisplayWitness::Scalar(scalar) => format_scalar_witness_admits(scalar, ty),
        CheckedDisplayWitness::Content => {
            ty.identity()
                == arcweft_core::value::RuntimeDialogueOpaqueRole::Content.semantic_identity()
        }
        CheckedDisplayWitness::Option(scalar) => {
            matches!(ty.shape(), RuntimeTypeShape::Option { item, .. } if format_scalar_witness_admits(scalar, item))
        }
        CheckedDisplayWitness::Project(_) => {
            project_self_type.is_some_and(|expected| expected == ty)
        }
        CheckedDisplayWitness::OptionProject(_) => {
            matches!(ty.shape(), RuntimeTypeShape::Option { item, .. } if project_self_type.is_some_and(|expected| expected == item.as_ref()))
        }
        CheckedDisplayWitness::OptionDeferredGeneric(_)
        | CheckedDisplayWitness::DeferredGeneric(_) => false,
    }
}

/// Direct interpolation whose source is a project value and whose slot result
/// is the Content returned by its selected DisplayText method.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectDisplayValue {
    source_type: RuntimeNormalizedType,
    method: RuntimeTraitMethodInstanceKey,
    template: RuntimeFormatTemplateKey,
}

impl RuntimeProjectDisplayValue {
    pub(super) const fn new(
        source_type: RuntimeNormalizedType,
        method: RuntimeTraitMethodInstanceKey,
        template: RuntimeFormatTemplateKey,
    ) -> Self {
        Self {
            source_type,
            method,
            template,
        }
    }

    pub const fn source_type(&self) -> &RuntimeNormalizedType {
        &self.source_type
    }
    pub const fn method(&self) -> &RuntimeTraitMethodInstanceKey {
        &self.method
    }
    pub const fn template(&self) -> &RuntimeFormatTemplateKey {
        &self.template
    }
}

pub(super) fn validate_selected_format_catalog(
    facts: &RuntimePlanSemanticFacts,
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
) -> Result<(), RuntimeSemanticFactsError> {
    let mut selected_instance_uses = BTreeSet::new();
    let mut invalid_selected_method = false;
    facts.visit_scoped_calls(&mut |scope, expression, call| {
            let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Format(
                formatted,
            )) = call.dispatch()
            else {
                return;
            };
            let Some(conformance) = formatted.display_witness().project_conformance() else {
                return;
            };
            let Some(self_type) = formatted.project_self_type() else {
                invalid_selected_method = true;
                return;
            };
            let key = RuntimeTraitMethodInstanceKey::new(
                conformance.method_declaration().clone(),
                self_type.identity(),
            );
            if facts.trait_method(&key).is_none_or(|method| {
                method.trait_identity() != &RuntimeTraitIdentity::StandardDisplayText
            }) {
                invalid_selected_method = true;
            }
            if let Some(scope) = RuntimeTraitMethodUseScope::for_executable(scope.scope()) {
                selected_instance_uses
                    .insert((RuntimeTraitMethodInstanceUse::new(scope, expression), key));
            }
        });
    facts.visit_dialogue_content_fragments(&mut |scope, fragment| {
        for value in fragment.values() {
            let Some(project) = value.project_display() else {
                continue;
            };
            if let Some(scope) = RuntimeTraitMethodUseScope::for_executable(scope.scope()) {
                selected_instance_uses.insert((
                    RuntimeTraitMethodInstanceUse::new(scope, value.expression()),
                    project.method().clone(),
                ));
            }
        }
    });
    let mut supplied_instance_uses = BTreeSet::new();
    for method in facts.trait_methods.values() {
        let globally_reachable = runtime_owners.contains_runtime_owner(
            &HirRuntimeExecutableOwner::ImplMethod(method.declaration().clone()),
        );
        if !globally_reachable
            && (method.closed_semantics().is_none() || method.instance_uses().is_empty())
        {
            invalid_selected_method = true;
        }
        for selected in method.instance_uses() {
            let use_key = (selected.clone(), method.key().clone());
            if !selected_instance_uses.contains(&use_key) || !supplied_instance_uses.insert(use_key)
            {
                invalid_selected_method = true;
            }
        }
    }
    if invalid_selected_method {
        return Err(RuntimeSemanticFactsError::InvalidTraitMethodIdentity);
    }
    let mut selected_format_keys = BTreeSet::new();
    let mut invalid_format_template = false;
    facts.visit_scoped_calls(&mut |scope, _, call| {
            let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Format(
                formatted,
            )) = call.dispatch()
            else {
                return;
            };
            if formatted.display_witness().is_deferred_generic() {
                invalid_format_template = true;
                return;
            }
            let key = RuntimeFormatTemplateKey::for_call(scope.scope(), formatted);
            if !selected_format_keys.insert(key.clone()) {
                invalid_format_template = true;
                return;
            }
            let Some(fact) = facts.format_template(&key) else {
                invalid_format_template = true;
                return;
            };
            let canonical = arcweft_text_model::DialogueContentFragmentTemplate::formatted_call(
                fact.template().id(),
                formatted.call_source(),
                formatted.value_source(),
            );
            if !matches!(canonical, Ok(ref template) if template == fact.template()) {
                invalid_format_template = true;
            }
            let mut values = call.operands().iter().filter(|operand| {
                operand
                    .parameter()
                    .is_some_and(|parameter| parameter.group() == 0 && parameter.parameter() == 0)
            });
            let value = values.next();
            if values.next().is_some()
                || !value.is_some_and(|operand| {
                    operand.source()
                        == RuntimeResolvedCallOperandSource::Expression(
                            formatted.checked().value().owner(),
                        )
                        && formatted.admits_primary(operand.ty())
                })
            {
                invalid_format_template = true;
            }
        });
    facts.visit_dialogue_content_fragments(&mut |scope, fragment| {
        for value in fragment.values() {
            let Some(project) = value.project_display() else {
                continue;
            };
            let key = project.template();
            if value.role() != RuntimeDialogueValueRole::Content
                || value.ty().identity()
                    != arcweft_core::value::RuntimeDialogueOpaqueRole::Content.semantic_identity()
                || key.scope() != &RuntimeFormatTemplateKey::scope_for_executable(scope.scope())
                || project.source_type().identity() != project.method().self_type()
                || facts.trait_method(project.method()).is_none_or(|method| {
                    method.trait_identity() != &RuntimeTraitIdentity::StandardDisplayText
                })
                || !selected_format_keys.insert(key.clone())
            {
                invalid_format_template = true;
                continue;
            }
            let Some(template) = facts
                .format_template(key)
                .map(RuntimeFormatTemplateFact::template)
            else {
                invalid_format_template = true;
                continue;
            };
            let [slot] = template.slots() else {
                invalid_format_template = true;
                continue;
            };
            if slot.role() != RuntimeDialogueValueRole::Formatted
                || slot.semantic_type() != value.ty().identity()
                || !template.marks().is_empty()
                || !template.effects().is_empty()
                || !matches!(
                    template.content().nodes.as_slice(),
                    [arcweft_text_model::RichTextNode::FormattedInsert { slot: node_slot, .. }]
                        if *node_slot == slot.slot()
                )
            {
                invalid_format_template = true;
            }
        }
    });
    if invalid_format_template
        || selected_format_keys.len() != facts.format_templates.len()
        || selected_format_keys
            .iter()
            .any(|key| !facts.format_templates.contains_key(key))
    {
        return Err(RuntimeSemanticFactsError::InvalidFormatTemplateCatalog);
    }
    Ok(())
}
