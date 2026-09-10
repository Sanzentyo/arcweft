use std::collections::{BTreeMap, BTreeSet};

use arcweft_id::PublicId;
use arcweft_lang_hir::{
    expr::{
        HirCallArgument, HirCallArgumentOrdinal, HirCallInvocation, HirCallValue, HirRecoveredName,
        HirRequiredTokenState,
    },
    identity::ExprId,
    leaf::{HirPathRoot, HirPathSegment},
    project::HirAnalysisProjectView,
    symbol::{
        ProjectSymbolTable,
        nominal::{ProjectNominalBody, ProjectNominalDeclarationId},
    },
};
use arcweft_source::{Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceSpan};

use crate::{
    callable::{
        CallableArgumentPolicy, CallableAttachedContentParameter, CallableCompileTimeScalarKind,
        CallableContentParameterConsumer, CallableEffectSchema, CallableGenericParameterIssuer,
        CallableGroupIndex, CallableGroupKind, CallableName, CallableParameter,
        CallableParameterAdmission, CallableParameterConsumer, CallableParameterGroup,
        CallableParameterIndex, CallableParameterPassing, CallableParameterPresence,
        CallableSchemaDependency, CallableSchemaDependencyDigest, CallableSchemaError,
        CallableSignatureSchema, CallableValidator, ContentCallableIdentity,
        PRODUCTION_CALLABLE_LIMITS, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
    },
    checked_rich_text::CheckedObjectDepth,
    effect_row::EffectRow,
    effects::EffectSet,
    nominal::{ResolvedTypeRefOutcome, TypeResolutionReport},
    registration::{CompileTimeScalarTypeRoleId, RegisteredCompileTimeScalarTypes},
    types::{
        AcceptedVariantCaseSemanticId, ProjectNominalType, TypeKind, VariantPayloadOwnerFamily,
        VariantPayloadShape,
    },
};

use super::{
    AcceptedRecordFieldSemanticId, CheckedCompileTimeScalar, CheckedCompileTimeScalarEnum,
    CheckedCompileTimeScalarEnumCase, CheckedCompileTimeScalarKind,
    CheckedTextProxyAttributeFamily, CheckedTextProxyAttributeOrigin, CheckedTextProxyCatalog,
    CheckedTextProxyDefinition, CheckedTextProxyFieldDefinition, CheckedTextProxyMetadataDefaults,
};

const MAX_TEXT_PROXY_FIELDS: usize = 256;
const MAX_TEXT_PROXY_DEFINITIONS: usize = 1_024;

/// Pre-publication proxy catalog. Attribute expression roots remain attached
/// until their typed defaults have been reduced and the final generation seal
/// validates every retained `ExprId`.
#[derive(Clone, Debug, Default)]
pub(crate) struct PreparedCheckedTextProxyCatalog {
    definitions: BTreeMap<super::CheckedTextProxyDefinitionId, PreparedCheckedTextProxyDefinition>,
    diagnostics: Vec<Diagnostic>,
    scalar_types: Option<RegisteredCompileTimeScalarTypes>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedCheckedTextProxyDefinition {
    checked: CheckedTextProxyDefinition,
    arguments: Box<[HirCallArgument]>,
    declaration_source: SourceSpan,
}

/// Exact origin retained while a proxy object is being prepared for the
/// content-emission seal. HIR coordinates are generation-bound evidence here;
/// they are not published as final object identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PreparedCheckedTextProxyOrigin {
    AttributeDefault(ExprId),
    Inline {
        argument: HirCallArgumentOrdinal,
        expression: ExprId,
    },
    CanonicalDefault,
    Absent,
}

impl PreparedCheckedTextProxyOrigin {
    pub(crate) const fn inline(argument: HirCallArgumentOrdinal, expression: ExprId) -> Self {
        Self::Inline {
            argument,
            expression,
        }
    }
}

/// One effective prepared value paired with its typed source origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedTextProxyValue<T> {
    value: T,
    origin: PreparedCheckedTextProxyOrigin,
}

impl<T> PreparedCheckedTextProxyValue<T> {
    pub(crate) const fn new(value: T, origin: PreparedCheckedTextProxyOrigin) -> Self {
        Self { value, origin }
    }

    pub(crate) const fn value(&self) -> &T {
        &self.value
    }

    pub(crate) const fn origin(&self) -> PreparedCheckedTextProxyOrigin {
        self.origin
    }

    pub(crate) fn into_parts(self) -> (T, PreparedCheckedTextProxyOrigin) {
        (self.value, self.origin)
    }
}

/// Effective metadata values for one prepared Object proxy.
///
/// Optional role/layer/depth values represent absence after applying the
/// declaration defaults. `hit_test` is always present because the canonical
/// Object schema owns `false` as its absence default.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedTextProxyMetadata {
    role: Option<PreparedCheckedTextProxyValue<PublicId>>,
    layer: Option<PreparedCheckedTextProxyValue<PublicId>>,
    depth: Option<PreparedCheckedTextProxyValue<CheckedObjectDepth>>,
    hit_test: PreparedCheckedTextProxyValue<bool>,
}

impl PreparedCheckedTextProxyMetadata {
    pub(crate) fn new(
        role: Option<PreparedCheckedTextProxyValue<PublicId>>,
        layer: Option<PreparedCheckedTextProxyValue<PublicId>>,
        depth: Option<PreparedCheckedTextProxyValue<CheckedObjectDepth>>,
        hit_test: PreparedCheckedTextProxyValue<bool>,
    ) -> Self {
        Self {
            role,
            layer,
            depth,
            hit_test,
        }
    }

    pub(crate) const fn role(&self) -> Option<&PreparedCheckedTextProxyValue<PublicId>> {
        self.role.as_ref()
    }

    pub(crate) const fn layer(&self) -> Option<&PreparedCheckedTextProxyValue<PublicId>> {
        self.layer.as_ref()
    }

    pub(crate) const fn depth(&self) -> Option<&PreparedCheckedTextProxyValue<CheckedObjectDepth>> {
        self.depth.as_ref()
    }

    pub(crate) const fn hit_test(&self) -> &PreparedCheckedTextProxyValue<bool> {
        &self.hit_test
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Option<PreparedCheckedTextProxyValue<PublicId>>,
        Option<PreparedCheckedTextProxyValue<PublicId>>,
        Option<PreparedCheckedTextProxyValue<CheckedObjectDepth>>,
        PreparedCheckedTextProxyValue<bool>,
    ) {
        (self.role, self.layer, self.depth, self.hit_test)
    }
}

/// One declaration-ordered custom field in a prepared Object proxy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedTextProxyApplicationField {
    declaration_ordinal: u32,
    semantic_id: AcceptedRecordFieldSemanticId,
    value: Option<CheckedCompileTimeScalar>,
    origin: PreparedCheckedTextProxyOrigin,
}

impl PreparedCheckedTextProxyApplicationField {
    pub(crate) const fn new(
        declaration_ordinal: u32,
        semantic_id: AcceptedRecordFieldSemanticId,
        value: Option<CheckedCompileTimeScalar>,
        origin: PreparedCheckedTextProxyOrigin,
    ) -> Self {
        Self {
            declaration_ordinal,
            semantic_id,
            value,
            origin,
        }
    }

    pub(crate) const fn declaration_ordinal(&self) -> u32 {
        self.declaration_ordinal
    }

    pub(crate) const fn semantic_id(&self) -> AcceptedRecordFieldSemanticId {
        self.semantic_id
    }

    pub(crate) const fn value(&self) -> Option<&CheckedCompileTimeScalar> {
        self.value.as_ref()
    }

    pub(crate) const fn origin(&self) -> PreparedCheckedTextProxyOrigin {
        self.origin
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        u32,
        AcceptedRecordFieldSemanticId,
        Option<CheckedCompileTimeScalar>,
        PreparedCheckedTextProxyOrigin,
    ) {
        (
            self.declaration_ordinal,
            self.semantic_id,
            self.value,
            self.origin,
        )
    }
}

/// Private owner-local application carrier consumed by the later generic
/// Object mapper. It intentionally carries no legacy checked object span and
/// no HIR-tag-specific carrier; those generation-bound forms cannot
/// authenticate this prepared relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedTextProxyApplication {
    definition_id: super::CheckedTextProxyDefinitionId,
    definition_digest: super::CheckedTextProxyDefinitionDigest,
    id: PreparedCheckedTextProxyValue<PublicId>,
    metadata: PreparedCheckedTextProxyMetadata,
    fields: Box<[PreparedCheckedTextProxyApplicationField]>,
}

impl PreparedCheckedTextProxyApplication {
    pub(crate) fn try_new(
        definition: &PreparedCheckedTextProxyDefinition,
        id: PreparedCheckedTextProxyValue<PublicId>,
        metadata: PreparedCheckedTextProxyMetadata,
        fields: impl Into<Box<[PreparedCheckedTextProxyApplicationField]>>,
    ) -> Option<Self> {
        let fields = fields.into();
        let checked = definition.checked();
        if !matches!(id.origin(), PreparedCheckedTextProxyOrigin::Inline { .. })
            || !prepared_metadata_matches_definition(checked, &metadata)
            || fields.len() != checked.fields().len()
            || fields
                .iter()
                .zip(checked.fields())
                .any(|(field, expected)| {
                    field.declaration_ordinal() != expected.declaration_ordinal()
                        || field.semantic_id() != expected.semantic_id()
                        || !super::scalar_matches_kind(field.value(), expected.kind())
                        || !prepared_field_origin_matches(expected, field)
                })
        {
            return None;
        }
        Some(Self {
            definition_id: checked.id().clone(),
            definition_digest: checked.digest().ok()?,
            id,
            metadata,
            fields,
        })
    }

    pub(crate) const fn definition_id(&self) -> &super::CheckedTextProxyDefinitionId {
        &self.definition_id
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        super::CheckedTextProxyDefinitionId,
        super::CheckedTextProxyDefinitionDigest,
        PreparedCheckedTextProxyValue<PublicId>,
        PreparedCheckedTextProxyMetadata,
        Box<[PreparedCheckedTextProxyApplicationField]>,
    ) {
        (
            self.definition_id,
            self.definition_digest,
            self.id,
            self.metadata,
            self.fields,
        )
    }
}

fn prepared_metadata_matches_definition(
    definition: &super::CheckedTextProxyDefinition,
    metadata: &PreparedCheckedTextProxyMetadata,
) -> bool {
    let role = prepared_optional_metadata_matches(
        metadata.role(),
        definition.metadata_defaults().role_default(),
    );
    let layer = prepared_optional_metadata_matches(
        metadata.layer(),
        definition.metadata_defaults().layer_default(),
    );
    let depth = prepared_optional_metadata_matches(
        metadata.depth(),
        definition.metadata_defaults().depth_default(),
    );
    let hit_test = prepared_metadata_value_matches(
        metadata.hit_test(),
        definition.metadata_defaults().hit_test_default(),
        Some(&false),
    );
    role && layer && depth && hit_test
}

fn prepared_optional_metadata_matches<T: Eq>(
    value: Option<&PreparedCheckedTextProxyValue<T>>,
    default: Option<&super::CheckedTextProxyMetadataDefault<T>>,
) -> bool {
    match (value, default) {
        (Some(value), default) => prepared_metadata_value_matches(value, default, None),
        (None, None) => true,
        (None, Some(_)) => false,
    }
}

fn prepared_metadata_value_matches<T: Eq>(
    value: &PreparedCheckedTextProxyValue<T>,
    default: Option<&super::CheckedTextProxyMetadataDefault<T>>,
    canonical: Option<&T>,
) -> bool {
    match value.origin() {
        PreparedCheckedTextProxyOrigin::AttributeDefault(_) => {
            default.is_some_and(|default| value.value() == default.value())
        }
        PreparedCheckedTextProxyOrigin::Inline { .. } => true,
        PreparedCheckedTextProxyOrigin::CanonicalDefault => {
            default.is_none() && canonical.is_some_and(|canonical| value.value() == canonical)
        }
        PreparedCheckedTextProxyOrigin::Absent => false,
    }
}

fn prepared_field_origin_matches(
    field: &super::CheckedTextProxyFieldDefinition,
    applied: &PreparedCheckedTextProxyApplicationField,
) -> bool {
    match applied.origin() {
        PreparedCheckedTextProxyOrigin::AttributeDefault(_) => field
            .default()
            .is_some_and(|default| applied.value() == Some(default.value())),
        PreparedCheckedTextProxyOrigin::Inline { .. } => applied.value().is_some(),
        PreparedCheckedTextProxyOrigin::CanonicalDefault => false,
        PreparedCheckedTextProxyOrigin::Absent => {
            field.optional() && field.default().is_none() && applied.value().is_none()
        }
    }
}

pub(crate) struct TextProxyFinalSealAuthority<'a> {
    pub(crate) project: HirAnalysisProjectView<'a>,
    pub(crate) symbols: &'a ProjectSymbolTable,
    pub(crate) project_nominals: &'a crate::final_analysis::ProjectNominalSemanticCatalog,
    pub(crate) types: &'a BTreeMap<arcweft_lang_hir::identity::TypeId, TypeKind>,
    pub(crate) type_resolutions:
        &'a BTreeMap<arcweft_lang_hir::identity::TypeId, TypeResolutionReport>,
    pub(crate) expressions:
        &'a BTreeMap<arcweft_lang_hir::identity::ExprId, crate::final_analysis::CheckedExpression>,
    pub(crate) control: crate::final_analysis::FinalSemanticAnalysisControl<'a>,
}

pub(crate) struct SealedCheckedTextProxyCatalog {
    catalog: CheckedTextProxyCatalog,
    diagnostics: Box<[Diagnostic]>,
}

impl SealedCheckedTextProxyCatalog {
    pub(crate) fn into_parts(self) -> (CheckedTextProxyCatalog, Box<[Diagnostic]>) {
        (self.catalog, self.diagnostics)
    }
}

impl PreparedCheckedTextProxyCatalog {
    pub(crate) fn build(
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        types: &BTreeMap<arcweft_lang_hir::identity::TypeId, TypeKind>,
        reports: &BTreeMap<arcweft_lang_hir::identity::TypeId, TypeResolutionReport>,
        scalars: &RegisteredCompileTimeScalarTypes,
    ) -> Self {
        let items = project
            .items()
            .map(|item| (item.id(), item.item()))
            .collect::<BTreeMap<_, _>>();
        let mut catalog = Self {
            definitions: BTreeMap::new(),
            diagnostics: Vec::new(),
            scalar_types: Some(scalars.clone()),
        };
        for declaration in symbols.nominal_symbols() {
            let Some(item) = items.get(&declaration.owner()).copied() else {
                catalog.push_diagnostic(
                    "sema.rich_text.proxy.invalid_owner",
                    "text-proxy nominal has no exact final-HIR declaration owner",
                    declaration.source().whole().clone(),
                );
                continue;
            };
            let attributes = item
                .prefix()
                .attributes()
                .iter()
                .enumerate()
                .filter_map(|(ordinal, attribute)| {
                    checked_attribute_family(attribute.path()).map(|family| {
                        u32::try_from(ordinal)
                            .map(|ordinal| (ordinal, family, attribute.arguments()))
                    })
                })
                .collect::<Result<Vec<_>, _>>();
            let Ok(attributes) = attributes else {
                catalog.push_diagnostic(
                    "sema.rich_text.proxy.attribute_limit",
                    "text-proxy declaration attribute ordinal exceeds the semantic domain",
                    declaration.source().whole().clone(),
                );
                continue;
            };
            if attributes.is_empty() {
                continue;
            }
            if catalog.definitions.len() >= MAX_TEXT_PROXY_DEFINITIONS {
                catalog.push_diagnostic(
                    "sema.rich_text.proxy.catalog_limit",
                    "project text-proxy definition limit exceeded",
                    declaration.source().whole().clone(),
                );
                continue;
            }
            if attributes.len() != 1 {
                catalog.push_diagnostic(
                    "sema.rich_text.proxy.duplicate_attribute",
                    "a project nominal must have exactly one text-proxy attribute",
                    declaration.source().whole().clone(),
                );
                continue;
            }
            let (attribute_ordinal, attribute, arguments) = attributes[0];
            match build_definition(
                declaration,
                attribute_ordinal,
                attribute,
                arguments,
                symbols,
                types,
                reports,
                scalars,
            ) {
                Ok(definition) => {
                    if catalog
                        .definitions
                        .insert(definition.checked().id().clone(), definition)
                        .is_some()
                    {
                        catalog.push_diagnostic(
                            "sema.rich_text.proxy.duplicate_definition",
                            "one project nominal produced more than one text-proxy definition",
                            declaration.source().whole().clone(),
                        );
                    }
                }
                Err(diagnostics) => catalog.diagnostics.extend(diagnostics),
            }
        }
        catalog
    }

    pub(crate) fn get(
        &self,
        id: &super::CheckedTextProxyDefinitionId,
    ) -> Option<&PreparedCheckedTextProxyDefinition> {
        self.definitions.get(id)
    }

    pub(crate) fn definition_for_type(
        &self,
        ty: &TypeKind,
    ) -> Option<&PreparedCheckedTextProxyDefinition> {
        let TypeKind::ProjectNominal(nominal) = ty else {
            return None;
        };
        if !nominal.arguments().is_empty() {
            return None;
        }
        let id = super::CheckedTextProxyDefinitionId::new(nominal.declaration().clone());
        let identity = ty.semantic_identity_digest().ok()?;
        self.get(&id)
            .filter(|definition| definition.checked.semantic_type == identity)
    }

    pub(crate) fn definition_ids(&self) -> Vec<super::CheckedTextProxyDefinitionId> {
        self.definitions.keys().cloned().collect()
    }

    pub(crate) fn remove(
        &mut self,
        id: &super::CheckedTextProxyDefinitionId,
    ) -> Option<PreparedCheckedTextProxyDefinition> {
        self.definitions.remove(id)
    }

    pub(crate) fn insert(
        &mut self,
        id: super::CheckedTextProxyDefinitionId,
        definition: PreparedCheckedTextProxyDefinition,
    ) -> bool {
        if definition.checked().id() != &id {
            return false;
        }
        self.definitions.insert(id, definition).is_none()
    }

    pub(crate) fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub(crate) fn visit_project_nominal_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        for definition in self.definitions.values() {
            definition.checked.visit_project_nominal_types(visitor)?;
        }
        Ok(())
    }

    pub(crate) fn seal(
        self,
        authority: TextProxyFinalSealAuthority<'_>,
    ) -> Result<SealedCheckedTextProxyCatalog, crate::final_analysis::FinalSemanticAnalysisError>
    {
        authority.control.check()?;
        let scalar_types = self
            .scalar_types
            .as_ref()
            .ok_or(crate::final_analysis::FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let rebuilt = Self::build(
            authority.project,
            authority.symbols,
            authority.types,
            authority.type_resolutions,
            scalar_types,
        );
        if rebuilt.diagnostics != self.diagnostics {
            return Err(crate::final_analysis::FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let expected_definitions = rebuilt
            .definitions
            .values()
            .map(|definition| {
                reconstruct_final_definition(definition, authority.expressions)
                    .map(|definition| (definition.id().clone(), definition))
                    .ok_or(crate::final_analysis::FinalSemanticAnalysisError::WrongPayloadFamily)
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        if expected_definitions.len() != self.definitions.len()
            || self.definitions.iter().any(|(key, definition)| {
                expected_definitions.get(key) != Some(definition.checked())
            })
        {
            return Err(crate::final_analysis::FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if !self.definitions.is_empty() {
            for (key, definition) in &self.definitions {
                authority.control.check()?;
                if key != definition.checked().id()
                    || definition.checked().attribute_origin().declaration()
                        != definition.checked().declaration()
                {
                    return Err(
                        crate::final_analysis::FinalSemanticAnalysisError::WrongPayloadFamily,
                    );
                }
                if !definition_origin_matches_hir(
                    key.declaration(),
                    definition,
                    authority.project,
                    authority.symbols,
                ) {
                    return Err(
                        crate::final_analysis::FinalSemanticAnalysisError::WrongPayloadFamily,
                    );
                }
                if !definition_matches_final_nominal(
                    definition.checked(),
                    authority.project_nominals,
                    scalar_types,
                ) {
                    return Err(
                        crate::final_analysis::FinalSemanticAnalysisError::WrongPayloadFamily,
                    );
                }
            }
        }
        let Self {
            definitions,
            diagnostics,
            scalar_types: _,
        } = self;
        Ok(SealedCheckedTextProxyCatalog {
            catalog: CheckedTextProxyCatalog::new(
                definitions
                    .into_iter()
                    .map(|(_, definition)| (definition.checked.id().clone(), definition.checked))
                    .collect(),
            ),
            diagnostics: diagnostics.into_boxed_slice(),
        })
    }

    fn push_diagnostic(&mut self, code: &'static str, message: &'static str, source: SourceSpan) {
        self.diagnostics
            .push(proxy_diagnostic(code, message, source));
    }
}

fn reconstruct_final_definition(
    definition: &PreparedCheckedTextProxyDefinition,
    expressions: &BTreeMap<
        arcweft_lang_hir::identity::ExprId,
        crate::final_analysis::CheckedExpression,
    >,
) -> Option<CheckedTextProxyDefinition> {
    let mut checked = definition.checked.clone();
    let mut seen = BTreeSet::new();
    for argument in definition.arguments() {
        let HirCallArgument::Named {
            name: HirRecoveredName::Valid(name),
            equals: HirRequiredTokenState::Present,
            value: HirCallValue::Present { value: expression },
        } = argument
        else {
            return None;
        };
        if !seen.insert(name.clone()) {
            return None;
        }
        let field = checked
            .fields()
            .iter()
            .position(|field| field.diagnostic_name() == name.as_str());
        let kind = match name.as_str() {
            "role" | "layer" => CheckedCompileTimeScalarKind::PublicId,
            "depth" => CheckedCompileTimeScalarKind::Length,
            "hit_test" => CheckedCompileTimeScalarKind::Bool,
            _ => checked
                .fields()
                .get(field?)
                .map(|field| field.kind().clone())?,
        };
        let fact = expressions
            .get(expression)
            .filter(|fact| fact.effects().is_empty())?;
        let crate::final_analysis::CheckedExpressionResolution::CompileTimeScalar(scalar) =
            fact.resolution()
        else {
            return None;
        };
        let value = scalar.value().clone();
        if !super::scalar_matches_kind(Some(&value), &kind) {
            return None;
        }
        match name.as_str() {
            "role" => {
                let CheckedCompileTimeScalar::PublicId(value) = value else {
                    return None;
                };
                checked.metadata_defaults_mut().set_role(*expression, value);
            }
            "layer" => {
                let CheckedCompileTimeScalar::PublicId(value) = value else {
                    return None;
                };
                checked
                    .metadata_defaults_mut()
                    .set_layer(*expression, value);
            }
            "depth" => {
                let CheckedCompileTimeScalar::Length(value) = value else {
                    return None;
                };
                if value.unit != crate::checked_rich_text::LengthUnit::Px {
                    return None;
                }
                checked.metadata_defaults_mut().set_depth(
                    *expression,
                    crate::checked_rich_text::CheckedObjectDepth::new(value.milli),
                );
            }
            "hit_test" => {
                let CheckedCompileTimeScalar::Bool(value) = value else {
                    return None;
                };
                checked
                    .metadata_defaults_mut()
                    .set_hit_test(*expression, value);
            }
            _ => {
                let field = field?;
                if !checked.fields_mut()[field]
                    .set_default(super::CheckedTextProxyFieldDefault::new(*expression, value))
                {
                    return None;
                }
            }
        }
    }
    Some(checked)
}

fn definition_origin_matches_hir(
    key: &ProjectNominalDeclarationId,
    definition: &PreparedCheckedTextProxyDefinition,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
) -> bool {
    let checked = definition.checked();
    let Some(declaration) = symbols.nominal(key) else {
        return false;
    };
    if declaration.id() != key
        || checked.declaration() != key
        || checked.attribute_origin().declaration() != key
        || checked.diagnostic_name() != declaration.id().name().as_str()
        || definition.declaration_source() != declaration.source().whole()
        || declaration.visibility() != Some(arcweft_lang_syntax::ast::common::Visibility::Public)
        || !declaration.type_parameters().is_empty()
        || !declaration.where_predicates().is_empty()
    {
        return false;
    }
    let ProjectNominalBody::Struct { fields } = declaration.body() else {
        return false;
    };
    if fields.len() != checked.fields().len()
        || fields
            .iter()
            .zip(checked.fields())
            .any(|(hir, accepted)| hir.name().as_str() != accepted.diagnostic_name())
    {
        return false;
    }
    let Some(item) = project
        .items()
        .find_map(|item| (item.id() == declaration.owner()).then_some(item.item()))
    else {
        return false;
    };
    let Ok(attribute_ordinal) = usize::try_from(checked.attribute_origin().ordinal()) else {
        return false;
    };
    let Some(attribute) = item.prefix().attributes().get(attribute_ordinal) else {
        return false;
    };
    checked_attribute_family(attribute.path()) == Some(checked.attribute())
        && attribute.arguments() == definition.arguments()
        && item
            .prefix()
            .attributes()
            .iter()
            .filter(|attribute| checked_attribute_family(attribute.path()).is_some())
            .count()
            == 1
}

fn definition_matches_final_nominal(
    definition: &CheckedTextProxyDefinition,
    project_nominals: &crate::final_analysis::ProjectNominalSemanticCatalog,
    scalar_types: &RegisteredCompileTimeScalarTypes,
) -> bool {
    let Some(crate::final_analysis::ProjectNominalSemanticDefinition::Record {
        nominal,
        fields,
        ..
    }) = project_nominals.get(definition.semantic_type())
    else {
        return false;
    };
    if nominal.declaration() != definition.declaration()
        || fields.len() != definition.fields().len()
    {
        return false;
    }
    fields
        .iter()
        .zip(definition.fields())
        .all(|(accepted, field)| {
            accepted.declaration_ordinal() == field.declaration_ordinal
                && accepted.semantic_id() == field.semantic_id
                && field_type_matches_kind(accepted.ty(), field, project_nominals, scalar_types)
                && field.default.as_ref().is_none_or(|default| {
                    super::scalar_matches_kind(Some(&default.value), &field.kind)
                })
        })
}

fn field_type_matches_kind(
    declared: &TypeKind,
    field: &CheckedTextProxyFieldDefinition,
    project_nominals: &crate::final_analysis::ProjectNominalSemanticCatalog,
    scalar_types: &RegisteredCompileTimeScalarTypes,
) -> bool {
    let (inner, optional) = match declared {
        TypeKind::Option(inner) if !matches!(inner.as_ref(), TypeKind::Option(_)) => {
            (inner.as_ref(), true)
        }
        TypeKind::Option(_) => return false,
        other => (other, false),
    };
    if field.optional != optional {
        return false;
    }
    if let Some(registered) = field.registered_type() {
        let role = match &field.kind {
            CheckedCompileTimeScalarKind::Bool => CompileTimeScalarTypeRoleId::Bool,
            CheckedCompileTimeScalarKind::Int => CompileTimeScalarTypeRoleId::Int,
            CheckedCompileTimeScalarKind::Milli => CompileTimeScalarTypeRoleId::Milli,
            CheckedCompileTimeScalarKind::Ratio => CompileTimeScalarTypeRoleId::Ratio,
            CheckedCompileTimeScalarKind::Length => CompileTimeScalarTypeRoleId::Length,
            CheckedCompileTimeScalarKind::Angle => CompileTimeScalarTypeRoleId::Angle,
            CheckedCompileTimeScalarKind::Duration => CompileTimeScalarTypeRoleId::Duration,
            CheckedCompileTimeScalarKind::PublicId => CompileTimeScalarTypeRoleId::PublicId,
            CheckedCompileTimeScalarKind::Text => CompileTimeScalarTypeRoleId::Text,
            CheckedCompileTimeScalarKind::Color => CompileTimeScalarTypeRoleId::Color,
            CheckedCompileTimeScalarKind::ClosedEnum(_) => return false,
        };
        return registered == scalar_types.row(role) && registered.ty() == inner;
    }
    let (TypeKind::ProjectNominal(nominal), CheckedCompileTimeScalarKind::ClosedEnum(schema)) =
        (inner, &field.kind)
    else {
        return false;
    };
    if !nominal.arguments().is_empty()
        || nominal.declaration() != schema.declaration()
        || inner.semantic_identity_digest() != Ok(schema.semantic_type())
    {
        return false;
    }
    let Some(crate::final_analysis::ProjectNominalSemanticDefinition::Variant {
        nominal: accepted_nominal,
        cases,
        ..
    }) = project_nominals.get(schema.semantic_type())
    else {
        return false;
    };
    accepted_nominal.declaration() == schema.declaration()
        && cases.len() == schema.cases().len()
        && cases.iter().zip(schema.cases()).all(|(accepted, case)| {
            accepted.ordinal() == case.ordinal()
                && accepted.semantic_id() == case.semantic_id()
                && accepted.payload() == &VariantPayloadShape::Unit
                && accepted.diagnostic_name() == case.diagnostic_name()
        })
}

impl PreparedCheckedTextProxyDefinition {
    pub(crate) const fn checked(&self) -> &CheckedTextProxyDefinition {
        &self.checked
    }

    pub(crate) fn checked_mut(&mut self) -> &mut CheckedTextProxyDefinition {
        &mut self.checked
    }

    pub(crate) fn arguments(&self) -> &[HirCallArgument] {
        &self.arguments
    }

    pub(crate) const fn declaration_source(&self) -> &SourceSpan {
        &self.declaration_source
    }

    /// Returns the exact declaration identity owned by this prepared
    /// definition.
    pub(crate) const fn id(&self) -> &super::CheckedTextProxyDefinitionId {
        self.checked.id()
    }

    /// Returns the current typed definition digest. The prepared definition
    /// owns this projection so callers cannot derive a parallel identity from
    /// its HIR attribute arguments.
    pub(crate) fn digest(
        &self,
    ) -> Result<super::CheckedTextProxyDefinitionDigest, super::CheckedTextProxyDefinitionDigestError>
    {
        self.checked.digest()
    }

    /// Returns the callable-layer identity carrier for this exact definition.
    /// The callable crate receives only its closed owner digest projection and
    /// therefore cannot derive an identity from a display name or HIR path.
    pub(crate) fn callable_identity(&self) -> Result<ContentCallableIdentity, CallableSchemaError> {
        let digest = self
            .digest()
            .map_err(|_| CallableSchemaError::FamilyInvariant {
                family: crate::callable::CallableFamily::Content,
                code: crate::callable::CallableFamilyInvariantCode::InvalidOwner,
            })?;
        Ok(ContentCallableIdentity::text_proxy_object(
            self.id().semantic_type(),
            CallableSchemaDependencyDigest::from_bytes(digest.into_bytes()),
        ))
    }

    /// Builds the private application carrier from the ordinary invocation's
    /// already-checked argument facts. This owner method applies declaration
    /// defaults and preserves each effective value's typed origin; it does
    /// not reconstruct a legacy object span or inspect source text.
    pub(crate) fn prepare_application(
        &self,
        invocation: &HirCallInvocation,
        expressions: &BTreeMap<ExprId, crate::final_analysis::PreparedExpressionFact>,
    ) -> Option<PreparedCheckedTextProxyApplication> {
        let mut named = BTreeMap::new();
        for (index, argument) in invocation.arguments().iter().enumerate() {
            let HirCallArgument::Named {
                name: HirRecoveredName::Valid(name),
                equals: HirRequiredTokenState::Present,
                value: HirCallValue::Present { value },
            } = argument
            else {
                return None;
            };
            if named
                .insert(
                    name.as_str().to_owned(),
                    (HirCallArgumentOrdinal::try_from_usize(index).ok()?, *value),
                )
                .is_some()
            {
                return None;
            }
        }

        let (id_argument, id_expression) = named.get("id").copied()?;
        let id_fact = expressions.get(&id_expression)?;
        let id = prepared_scalar(id_fact, &CheckedCompileTimeScalarKind::PublicId)?;
        let CheckedCompileTimeScalar::PublicId(id) = id else {
            return None;
        };
        let id = PreparedCheckedTextProxyValue::new(
            id,
            PreparedCheckedTextProxyOrigin::inline(id_argument, id_expression),
        );

        let metadata = PreparedCheckedTextProxyMetadata::new(
            prepared_metadata_value(
                named.get("role").copied(),
                self.checked.metadata_defaults().role_default(),
                &expressions,
                CheckedCompileTimeScalarKind::PublicId,
            )?,
            prepared_metadata_value(
                named.get("layer").copied(),
                self.checked.metadata_defaults().layer_default(),
                &expressions,
                CheckedCompileTimeScalarKind::PublicId,
            )?,
            prepared_depth_value(
                named.get("depth").copied(),
                self.checked.metadata_defaults().depth_default(),
                &expressions,
            )?,
            prepared_hit_test_value(
                named.get("hit_test").copied(),
                self.checked.metadata_defaults().hit_test_default(),
                &expressions,
            )?,
        );
        let mut fields = Vec::with_capacity(self.checked.fields().len());
        for field in self.checked.fields() {
            let (value, origin) =
                if let Some((argument, expression)) = named.get(field.diagnostic_name()).copied() {
                    let fact = expressions.get(&expression)?;
                    (
                        Some(prepared_scalar(fact, field.kind())?),
                        PreparedCheckedTextProxyOrigin::inline(argument, expression),
                    )
                } else if let Some(default) = field.default() {
                    (
                        Some(default.value().clone()),
                        PreparedCheckedTextProxyOrigin::AttributeDefault(default.expression()),
                    )
                } else {
                    (None, PreparedCheckedTextProxyOrigin::Absent)
                };
            fields.push(PreparedCheckedTextProxyApplicationField::new(
                field.declaration_ordinal(),
                field.semantic_id(),
                value,
                origin,
            ));
        }
        PreparedCheckedTextProxyApplication::try_new(self, id, metadata, fields)
    }

    /// Builds the sole Object callable schema dependent on this exact
    /// prepared definition. Argument mapping and object materialization are
    /// intentionally outside this constructor.
    pub(crate) fn callable_schema(
        &self,
        scalar_types: &RegisteredCompileTimeScalarTypes,
    ) -> Result<CallableSignatureSchema, CallableSchemaError> {
        let checked = self.checked();
        let mut parameters = vec![
            content_parameter(
                0,
                "id",
                compile_time_scalar_admission(
                    0,
                    CallableCompileTimeScalarKind::PublicId,
                    scalar_types
                        .type_for(CompileTimeScalarTypeRoleId::PublicId)
                        .clone(),
                )?,
                CallableParameterPresence::Required,
                CallableContentParameterConsumer::ObjectId,
            )?,
            content_parameter(
                1,
                "type",
                CallableParameterAdmission::text_proxy_nominal(),
                CallableParameterPresence::Required,
                CallableContentParameterConsumer::ObjectType,
            )?,
            content_parameter(
                2,
                "role",
                compile_time_scalar_admission(
                    2,
                    CallableCompileTimeScalarKind::PublicId,
                    scalar_types
                        .type_for(CompileTimeScalarTypeRoleId::PublicId)
                        .clone(),
                )?,
                metadata_presence(checked.metadata_defaults().role_default().is_some(), false),
                CallableContentParameterConsumer::ObjectRole,
            )?,
            content_parameter(
                3,
                "layer",
                compile_time_scalar_admission(
                    3,
                    CallableCompileTimeScalarKind::PublicId,
                    scalar_types
                        .type_for(CompileTimeScalarTypeRoleId::PublicId)
                        .clone(),
                )?,
                metadata_presence(checked.metadata_defaults().layer_default().is_some(), false),
                CallableContentParameterConsumer::ObjectLayer,
            )?,
            content_parameter(
                4,
                "depth",
                compile_time_scalar_admission(
                    4,
                    CallableCompileTimeScalarKind::Length,
                    scalar_types
                        .type_for(CompileTimeScalarTypeRoleId::Length)
                        .clone(),
                )?,
                metadata_presence(checked.metadata_defaults().depth_default().is_some(), false),
                CallableContentParameterConsumer::ObjectDepth,
            )?,
            content_parameter(
                5,
                "hit_test",
                compile_time_scalar_admission(
                    5,
                    CallableCompileTimeScalarKind::Bool,
                    scalar_types
                        .type_for(CompileTimeScalarTypeRoleId::Bool)
                        .clone(),
                )?,
                CallableParameterPresence::Defaulted,
                CallableContentParameterConsumer::ObjectHitTest,
            )?,
        ];
        for field in checked.fields() {
            let actual =
                parameters
                    .len()
                    .checked_add(1)
                    .ok_or(CallableSchemaError::ParameterLimit {
                        actual: usize::MAX,
                        limit: PRODUCTION_CALLABLE_LIMITS.max_parameters_per_callable(),
                    })?;
            let index = CallableParameterIndex::try_from_usize(parameters.len()).map_err(|_| {
                CallableSchemaError::ParameterLimit {
                    actual,
                    limit: PRODUCTION_CALLABLE_LIMITS.max_parameters_per_callable(),
                }
            })?;
            let name = CallableName::try_new(field.diagnostic_name()).map_err(|_| {
                CallableSchemaError::MissingParameterName {
                    group: CallableGroupIndex::ZERO,
                    parameter: index,
                }
            })?;
            let parameter = CallableParameter::for_accepted_project_record_field(
                index,
                Some(name),
                field.semantic_id(),
                CallableParameterAdmission::compile_time_scalar(
                    field.kind().callable_kind(),
                    compile_time_scalar_field_value_type(field).ok_or(
                        CallableSchemaError::InvalidParameterAdmission {
                            group: CallableGroupIndex::ZERO,
                            parameter: index,
                        },
                    )?,
                )
                .ok_or(CallableSchemaError::InvalidParameterAdmission {
                    group: CallableGroupIndex::ZERO,
                    parameter: index,
                })?,
                CallableParameterPassing::NamedOnly,
                field_presence(field),
                None,
                None,
            )?
            .with_consumer(CallableParameterConsumer::Content(
                CallableContentParameterConsumer::ObjectCustomField(field.semantic_id()),
            ));
            parameters.push(parameter);
        }
        let group = CallableParameterGroup::try_new(
            CallableGroupIndex::ZERO,
            CallableGroupKind::Initial,
            parameters,
            &PRODUCTION_CALLABLE_LIMITS,
        )?;
        let definition_digest =
            checked
                .digest()
                .map_err(|_| CallableSchemaError::FamilyInvariant {
                    family: crate::callable::CallableFamily::Content,
                    code: crate::callable::CallableFamilyInvariantCode::InvalidOwner,
                })?;
        let identity = self.callable_identity()?;
        CallableSignatureSchema::try_new_with_dependency(
            vec![group],
            crate::callable::CallableResultSchema::ContentEmission(identity),
            CallableEffectSchema::fixed(EffectRow::closed(EffectSet::new())),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Content(identity),
            Some(CallableAttachedContentParameter::text_proxy_object()),
            CallableSchemaDependency::text_proxy(
                checked.id().semantic_type(),
                CallableSchemaDependencyDigest::from_bytes(definition_digest.into_bytes()),
            ),
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
    }
}

fn prepared_metadata_value(
    authored: Option<(HirCallArgumentOrdinal, ExprId)>,
    default: Option<&super::CheckedTextProxyMetadataDefault<PublicId>>,
    expressions: &BTreeMap<ExprId, crate::final_analysis::PreparedExpressionFact>,
    kind: CheckedCompileTimeScalarKind,
) -> Option<Option<PreparedCheckedTextProxyValue<PublicId>>> {
    if let Some((argument, expression)) = authored {
        let fact = expressions.get(&expression)?;
        let CheckedCompileTimeScalar::PublicId(value) = prepared_scalar(fact, &kind)? else {
            return None;
        };
        return Some(Some(PreparedCheckedTextProxyValue::new(
            value,
            PreparedCheckedTextProxyOrigin::inline(argument, expression),
        )));
    }
    Some(default.map(|default| {
        PreparedCheckedTextProxyValue::new(
            default.value().clone(),
            PreparedCheckedTextProxyOrigin::AttributeDefault(default.expression()),
        )
    }))
}

fn prepared_depth_value(
    authored: Option<(HirCallArgumentOrdinal, ExprId)>,
    default: Option<&super::CheckedTextProxyMetadataDefault<CheckedObjectDepth>>,
    expressions: &BTreeMap<ExprId, crate::final_analysis::PreparedExpressionFact>,
) -> Option<Option<PreparedCheckedTextProxyValue<CheckedObjectDepth>>> {
    if let Some((argument, expression)) = authored {
        let fact = expressions.get(&expression)?;
        let CheckedCompileTimeScalar::Length(value) =
            prepared_scalar(fact, &CheckedCompileTimeScalarKind::Length)?
        else {
            return None;
        };
        if value.unit != crate::checked_rich_text::LengthUnit::Px {
            return None;
        }
        return Some(Some(PreparedCheckedTextProxyValue::new(
            CheckedObjectDepth::new(value.milli),
            PreparedCheckedTextProxyOrigin::inline(argument, expression),
        )));
    }
    Some(default.map(|default| {
        PreparedCheckedTextProxyValue::new(
            *default.value(),
            PreparedCheckedTextProxyOrigin::AttributeDefault(default.expression()),
        )
    }))
}

fn prepared_hit_test_value(
    authored: Option<(HirCallArgumentOrdinal, ExprId)>,
    default: Option<&super::CheckedTextProxyMetadataDefault<bool>>,
    expressions: &BTreeMap<ExprId, crate::final_analysis::PreparedExpressionFact>,
) -> Option<PreparedCheckedTextProxyValue<bool>> {
    if let Some((argument, expression)) = authored {
        let fact = expressions.get(&expression)?;
        let CheckedCompileTimeScalar::Bool(value) =
            prepared_scalar(fact, &CheckedCompileTimeScalarKind::Bool)?
        else {
            return None;
        };
        return Some(PreparedCheckedTextProxyValue::new(
            value,
            PreparedCheckedTextProxyOrigin::inline(argument, expression),
        ));
    }
    default
        .map(|default| {
            PreparedCheckedTextProxyValue::new(
                *default.value(),
                PreparedCheckedTextProxyOrigin::AttributeDefault(default.expression()),
            )
        })
        .or_else(|| {
            Some(PreparedCheckedTextProxyValue::new(
                false,
                PreparedCheckedTextProxyOrigin::CanonicalDefault,
            ))
        })
}

fn prepared_scalar(
    fact: &crate::final_analysis::PreparedExpressionFact,
    expected: &CheckedCompileTimeScalarKind,
) -> Option<CheckedCompileTimeScalar> {
    let crate::final_analysis::PreparedExpressionFact::CompileTimeScalar(scalar) = fact else {
        return None;
    };
    super::scalar_matches_kind(Some(scalar.value()), expected).then(|| scalar.value().clone())
}

fn compile_time_scalar_admission(
    index: usize,
    kind: CallableCompileTimeScalarKind,
    value_type: TypeKind,
) -> Result<CallableParameterAdmission, CallableSchemaError> {
    let actual = index
        .checked_add(1)
        .ok_or(CallableSchemaError::ParameterLimit {
            actual: usize::MAX,
            limit: PRODUCTION_CALLABLE_LIMITS.max_parameters_per_callable(),
        })?;
    let parameter = CallableParameterIndex::try_from_usize(index).map_err(|_| {
        CallableSchemaError::ParameterLimit {
            actual,
            limit: PRODUCTION_CALLABLE_LIMITS.max_parameters_per_callable(),
        }
    })?;
    CallableParameterAdmission::compile_time_scalar(kind, value_type).ok_or(
        CallableSchemaError::InvalidParameterAdmission {
            group: CallableGroupIndex::ZERO,
            parameter,
        },
    )
}

fn compile_time_scalar_field_value_type(
    field: &CheckedTextProxyFieldDefinition,
) -> Option<TypeKind> {
    if let Some(registered) = field.registered_type() {
        return Some(registered.ty().clone());
    }
    let CheckedCompileTimeScalarKind::ClosedEnum(schema) = field.kind() else {
        return None;
    };
    Some(TypeKind::ProjectNominal(ProjectNominalType::new(
        schema.declaration().clone(),
        Box::<[TypeKind]>::default(),
    )))
}

fn content_parameter(
    index: usize,
    name: &str,
    admission: CallableParameterAdmission,
    presence: CallableParameterPresence,
    consumer: CallableContentParameterConsumer,
) -> Result<CallableParameter, CallableSchemaError> {
    let actual = index
        .checked_add(1)
        .ok_or(CallableSchemaError::ParameterLimit {
            actual: usize::MAX,
            limit: PRODUCTION_CALLABLE_LIMITS.max_parameters_per_callable(),
        })?;
    let index = CallableParameterIndex::try_from_usize(index).map_err(|_| {
        CallableSchemaError::ParameterLimit {
            actual,
            limit: PRODUCTION_CALLABLE_LIMITS.max_parameters_per_callable(),
        }
    })?;
    let name =
        CallableName::try_new(name).map_err(|_| CallableSchemaError::MissingParameterName {
            group: CallableGroupIndex::ZERO,
            parameter: index,
        })?;
    CallableParameter::try_new(
        index,
        Some(name),
        admission,
        CallableParameterPassing::NamedOnly,
        presence,
        None,
        None,
    )
    .map(|parameter| parameter.with_consumer(CallableParameterConsumer::Content(consumer)))
}

fn metadata_presence(
    has_declaration_default: bool,
    has_canonical_default: bool,
) -> CallableParameterPresence {
    if has_declaration_default || has_canonical_default {
        CallableParameterPresence::Defaulted
    } else {
        CallableParameterPresence::Optional
    }
}

fn field_presence(field: &CheckedTextProxyFieldDefinition) -> CallableParameterPresence {
    if field.default().is_some() {
        CallableParameterPresence::Defaulted
    } else if field.optional() {
        CallableParameterPresence::Optional
    } else {
        CallableParameterPresence::Required
    }
}

fn build_definition(
    declaration: &arcweft_lang_hir::symbol::nominal::ProjectNominalDeclaration,
    attribute_ordinal: u32,
    attribute: CheckedTextProxyAttributeFamily,
    arguments: &[HirCallArgument],
    symbols: &ProjectSymbolTable,
    types: &BTreeMap<arcweft_lang_hir::identity::TypeId, TypeKind>,
    reports: &BTreeMap<arcweft_lang_hir::identity::TypeId, TypeResolutionReport>,
    scalars: &RegisteredCompileTimeScalarTypes,
) -> Result<PreparedCheckedTextProxyDefinition, Vec<Diagnostic>> {
    let source = declaration.source().whole().clone();
    let mut diagnostics = Vec::new();
    if declaration.visibility() != Some(arcweft_lang_syntax::ast::common::Visibility::Public) {
        diagnostics.push(proxy_diagnostic(
            "sema.rich_text.proxy.not_public",
            "a text-proxy declaration must be public",
            source.clone(),
        ));
    }
    if !declaration.type_parameters().is_empty() || !declaration.where_predicates().is_empty() {
        diagnostics.push(proxy_diagnostic(
            "sema.rich_text.proxy.generic",
            "a text-proxy declaration cannot be generic",
            source.clone(),
        ));
    }
    let ProjectNominalBody::Struct { fields } = declaration.body() else {
        diagnostics.push(proxy_diagnostic(
            "sema.rich_text.proxy.not_struct",
            "a text-proxy declaration must be a project struct",
            source.clone(),
        ));
        return Err(diagnostics);
    };
    if fields.len() > MAX_TEXT_PROXY_FIELDS {
        diagnostics.push(proxy_diagnostic(
            "sema.rich_text.proxy.field_limit",
            "text-proxy field limit exceeded",
            fields[MAX_TEXT_PROXY_FIELDS].source().whole().clone(),
        ));
    }

    let id = super::CheckedTextProxyDefinitionId::new(declaration.id().clone());
    let semantic_type = id.semantic_type();
    let mut checked_fields = Vec::new();
    let mut names = BTreeSet::new();
    for (index, field) in fields.iter().enumerate().take(MAX_TEXT_PROXY_FIELDS) {
        let field_source = field.source().whole().clone();
        if !names.insert(field.name().clone()) {
            diagnostics.push(proxy_diagnostic(
                "sema.rich_text.proxy.duplicate_field",
                "text-proxy field name is duplicated",
                field_source,
            ));
            continue;
        }
        if matches!(
            field.name().as_str(),
            "id" | "type" | "role" | "layer" | "depth" | "hit_test"
        ) {
            diagnostics.push(proxy_diagnostic(
                "sema.rich_text.proxy.reserved_field",
                "text-proxy field conflicts with canonical object metadata",
                field_source,
            ));
            continue;
        }
        let Some(ty) = types.get(&field.ty()) else {
            diagnostics.push(proxy_diagnostic(
                "sema.rich_text.proxy.missing_field_type",
                "text-proxy field has no resolved semantic type",
                field_source,
            ));
            continue;
        };
        let complete = reports.get(&field.ty()).is_some_and(|report| {
            matches!(report.outcome(), ResolvedTypeRefOutcome::Complete(product) if product.recovered() == ty)
        });
        if !complete {
            diagnostics.push(proxy_diagnostic(
                "sema.rich_text.proxy.incomplete_field_type",
                "text-proxy field type is not a complete accepted type resolution",
                field_source,
            ));
            continue;
        }
        let (inner, optional) = match ty {
            TypeKind::Option(inner) if !matches!(inner.as_ref(), TypeKind::Option(_)) => {
                (inner.as_ref(), true)
            }
            TypeKind::Option(_) => {
                diagnostics.push(proxy_diagnostic(
                    "sema.rich_text.proxy.nested_optional",
                    "text-proxy fields permit exactly one Option wrapper",
                    field_source,
                ));
                continue;
            }
            ty => (ty, false),
        };
        let Some((kind, registered_type)) =
            classify_scalar(inner, optional, reports.get(&field.ty()), symbols, scalars)
        else {
            diagnostics.push(proxy_diagnostic(
                "sema.rich_text.proxy.unsupported_field_type",
                "text-proxy field type is outside the registered scalar algebra",
                field_source,
            ));
            continue;
        };
        let ordinal = match u32::try_from(index) {
            Ok(ordinal) => ordinal,
            Err(_) => {
                diagnostics.push(proxy_diagnostic(
                    "sema.rich_text.proxy.field_limit",
                    "text-proxy field ordinal exceeds the semantic domain",
                    field_source,
                ));
                continue;
            }
        };
        let field_type = match ty.semantic_identity_digest() {
            Ok(identity) => identity,
            Err(error) => {
                diagnostics.push(proxy_diagnostic(
                    "sema.rich_text.proxy.invalid_type_scope",
                    format!("text-proxy field has an invalid generic scope: {error}"),
                    field_source,
                ));
                continue;
            }
        };
        checked_fields.push(CheckedTextProxyFieldDefinition::new(
            ordinal,
            AcceptedRecordFieldSemanticId::issue(semantic_type, ordinal, field_type),
            field_type,
            field.name().clone(),
            kind,
            registered_type,
            optional,
            None,
        ));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(PreparedCheckedTextProxyDefinition {
        checked: CheckedTextProxyDefinition::new(
            id,
            CheckedTextProxyAttributeOrigin::new(declaration.id().clone(), attribute_ordinal),
            semantic_type,
            declaration.id().name().clone(),
            attribute,
            checked_fields.into_boxed_slice(),
            CheckedTextProxyMetadataDefaults::default(),
        ),
        arguments: arguments.to_vec().into_boxed_slice(),
        declaration_source: source,
    })
}

fn classify_scalar(
    ty: &TypeKind,
    optional: bool,
    report: Option<&TypeResolutionReport>,
    symbols: &ProjectSymbolTable,
    scalars: &RegisteredCompileTimeScalarTypes,
) -> Option<(
    CheckedCompileTimeScalarKind,
    Option<crate::registration::RegisteredCompileTimeScalarType>,
)> {
    let role = classify_registered_scalar(report?, ty, optional, scalars)?;
    if let Some(role) = role {
        let kind = match role {
            CompileTimeScalarTypeRoleId::Bool => CheckedCompileTimeScalarKind::Bool,
            CompileTimeScalarTypeRoleId::Int => CheckedCompileTimeScalarKind::Int,
            CompileTimeScalarTypeRoleId::Milli => CheckedCompileTimeScalarKind::Milli,
            CompileTimeScalarTypeRoleId::Ratio => CheckedCompileTimeScalarKind::Ratio,
            CompileTimeScalarTypeRoleId::Length => CheckedCompileTimeScalarKind::Length,
            CompileTimeScalarTypeRoleId::Angle => CheckedCompileTimeScalarKind::Angle,
            CompileTimeScalarTypeRoleId::Duration => CheckedCompileTimeScalarKind::Duration,
            CompileTimeScalarTypeRoleId::PublicId => CheckedCompileTimeScalarKind::PublicId,
            CompileTimeScalarTypeRoleId::Color => CheckedCompileTimeScalarKind::Color,
            CompileTimeScalarTypeRoleId::Text => CheckedCompileTimeScalarKind::Text,
        };
        return Some((kind, Some(scalars.row(role).clone())));
    }
    let TypeKind::ProjectNominal(nominal) = ty else {
        return None;
    };
    if !nominal.arguments().is_empty() {
        return None;
    }
    let declaration = symbols.nominal(nominal.declaration())?;
    let ProjectNominalBody::Enum { variants } = declaration.body() else {
        return None;
    };
    if variants.iter().any(|variant| variant.payload().is_some()) {
        return None;
    }
    let semantic_type = ty.semantic_identity_digest().ok()?;
    let cases = variants
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            let ordinal = u32::try_from(index).ok()?;
            Some(CheckedCompileTimeScalarEnumCase::new(
                ordinal,
                AcceptedVariantCaseSemanticId::issue(
                    VariantPayloadOwnerFamily::Project,
                    semantic_type,
                    ordinal,
                    &VariantPayloadShape::Unit,
                ),
                variant.name().clone(),
            ))
        })
        .collect::<Option<Box<[_]>>>()?;
    Some((
        CheckedCompileTimeScalarKind::ClosedEnum(CheckedCompileTimeScalarEnum::new(
            declaration.id().clone(),
            semantic_type,
            cases,
        )),
        None,
    ))
}

fn classify_registered_scalar(
    report: &TypeResolutionReport,
    expected_inner: &TypeKind,
    optional: bool,
    scalars: &RegisteredCompileTimeScalarTypes,
) -> Option<Option<CompileTimeScalarTypeRoleId>> {
    let ResolvedTypeRefOutcome::Complete(product) = report.outcome() else {
        return None;
    };
    let expected_outer = if optional {
        TypeKind::Option(Box::new(expected_inner.clone()))
    } else {
        expected_inner.clone()
    };
    if product.recovered() != &expected_outer {
        return None;
    }
    if optional
        && !product.nodes().iter().any(|node| {
            node.recovered() == Some(&expected_outer)
                && matches!(
                    node.outcome(),
                    crate::nominal::TypeNameResolution::Builtin(
                        crate::nominal::BuiltinTypeConstructor::Option
                    )
                )
        })
    {
        return None;
    }
    let mut selected = None;
    for node in product
        .nodes()
        .iter()
        .filter(|node| node.recovered() == Some(expected_inner))
    {
        let Ok(Some(role)) = scalars.classify_node(node) else {
            continue;
        };
        if selected
            .replace(role)
            .is_some_and(|previous| previous != role)
        {
            return None;
        }
    }
    Some(selected)
}

fn checked_attribute_family(
    path: &arcweft_lang_hir::leaf::HirPath,
) -> Option<CheckedTextProxyAttributeFamily> {
    if path.root() != HirPathRoot::ImplicitCrate {
        return None;
    }
    let [segment] = path.segments() else {
        return None;
    };
    let name = match segment {
        HirPathSegment::Identifier(name) => name.as_str(),
        HirPathSegment::ProjectSymbol(name) => name.as_str(),
    };
    match name {
        "text_proxy" => Some(CheckedTextProxyAttributeFamily::TextProxy),
        "rich_text_proxy" => Some(CheckedTextProxyAttributeFamily::RichTextProxy),
        _ => None,
    }
}

fn proxy_diagnostic(
    code: &'static str,
    message: impl Into<String>,
    source: SourceSpan,
) -> Diagnostic {
    Diagnostic::new(DiagnosticSeverity::Error, message)
        .with_code(code)
        .with_label(DiagnosticLabel::primary(
            source,
            Some("invalid text-proxy declaration".to_owned()),
        ))
}
