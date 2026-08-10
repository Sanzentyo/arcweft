//! Accepted-world projection of source-backed environment inputs.

use std::collections::BTreeMap;

use arcweft_rust_abi::ArcweftRustTypeParameterIndex;
use arcweft_source::SourceSpan;

use crate::{
    env::{
        EnumVariantPayload,
        identity::EnvironmentBindingId,
        nominal::{AcceptedNominalId, AcceptedNominalInstantiationError, AcceptedNominalOwnerId},
        rust_metadata::{
            AcceptedRustStructShape, AcceptedRustTypeMetadata, AcceptedRustTypeMetadataCatalog,
            AcceptedRustTypeMetadataCatalogError, AcceptedRustTypeMetadataKind,
            RustStructMetadataInput, RustTypeMetadataPublicationInput,
            RustTypeMetadataPublicationKind, RustVariantMetadataInput, RustVariantPayloadInput,
        },
    },
    nominal::{NominalAggregationLimits, NominalResolutionLimitKind, NominalResolutionLimits},
    registration::{
        AcceptedNominalWorld, AcceptedNominalWorldLookupError, BoundEnvironmentRegistrationInput,
        EnvironmentCallableLookupInput, EnvironmentCallablePublicationRecordInput,
        EnvironmentParameterGroupInput, EnvironmentParameterInput, EnvironmentParameterTypeInput,
        EnvironmentPublicationItemId, EnvironmentTypeProjectionKind, EnvironmentTypeProjectionNode,
        EnvironmentTypeSite, EnvironmentTypeSiteRoot, EnvironmentTypeSiteStep,
    },
    types::{GenericTypeOwnerId, GenericTypeParameterId, TypeKind},
};

use super::{
    CallableEffectSchema, CallableLookupKey, CallableParameter, CallableParameterGroup,
    CallableParameterType, CallablePublicationError, CallableSignatureSchema,
    EnvironmentCallablePublication, EnvironmentCallablePublicationRecord, ReceiverMethodKey,
};

/// Related source evidence for one environment projection diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentPublicationRelatedSource {
    label: EnvironmentPublicationRelatedLabel,
    source: SourceSpan,
}

/// Role of a source related to an environment projection failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentPublicationRelatedLabel {
    ContainingItem,
    AcceptedDeclaration,
    InaccessibleDeclaration,
    VisibleOwnerDeclaration,
}

/// Typed reason one source-backed environment projection failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentPublicationProjectionErrorKind {
    WorldMismatch,
    UnknownPath {
        requested: AcceptedNominalId,
    },
    InaccessibleExport {
        requested: AcceptedNominalId,
    },
    OwnerMismatch {
        requested: AcceptedNominalId,
        visible: AcceptedNominalId,
    },
    WrongArity {
        nominal: AcceptedNominalId,
        expected: u16,
        actual: usize,
    },
    InvalidAcceptedSemantics {
        nominal: AcceptedNominalId,
    },
    FreeTypeParameterInCallable {
        index: ArcweftRustTypeParameterIndex,
    },
    UnboundMetadataTypeParameter {
        owner: AcceptedNominalId,
        index: ArcweftRustTypeParameterIndex,
    },
    MetadataOwnerMismatch {
        declaration: AcceptedNominalId,
        package: crate::env::nominal::RustPackageId,
    },
    RustMetadataCatalog {
        error: AcceptedRustTypeMetadataCatalogError,
    },
    LimitExceeded {
        kind: NominalResolutionLimitKind,
        observed: u64,
        maximum: u64,
    },
    Callable {
        error: CallablePublicationError,
    },
}

/// One deterministic source-backed environment projection diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentPublicationProjectionDiagnostic {
    item: EnvironmentPublicationItemId,
    site: EnvironmentTypeSite,
    primary: SourceSpan,
    related: Box<[EnvironmentPublicationRelatedSource]>,
    kind: Box<EnvironmentPublicationProjectionErrorKind>,
}

/// Fail-closed aggregate of environment projection diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentPublicationProjectionReport {
    diagnostics: Box<[EnvironmentPublicationProjectionDiagnostic]>,
    omitted_diagnostics: usize,
}

impl EnvironmentPublicationRelatedSource {
    pub const fn label(&self) -> EnvironmentPublicationRelatedLabel {
        self.label
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

impl EnvironmentPublicationProjectionDiagnostic {
    pub const fn item(&self) -> &EnvironmentPublicationItemId {
        &self.item
    }

    pub const fn site(&self) -> &EnvironmentTypeSite {
        &self.site
    }

    pub const fn primary(&self) -> &SourceSpan {
        &self.primary
    }

    pub fn related(&self) -> &[EnvironmentPublicationRelatedSource] {
        &self.related
    }

    pub const fn kind(&self) -> &EnvironmentPublicationProjectionErrorKind {
        &self.kind
    }
}

impl EnvironmentPublicationProjectionReport {
    pub fn diagnostics(&self) -> &[EnvironmentPublicationProjectionDiagnostic] {
        &self.diagnostics
    }

    pub const fn omitted_diagnostics(&self) -> usize {
        self.omitted_diagnostics
    }

    pub(crate) fn into_parts(self) -> (Box<[EnvironmentPublicationProjectionDiagnostic]>, usize) {
        (self.diagnostics, self.omitted_diagnostics)
    }

    fn one(diagnostic: Box<EnvironmentPublicationProjectionDiagnostic>) -> Self {
        Self {
            diagnostics: Box::new([*diagnostic]),
            omitted_diagnostics: 0,
        }
    }

    fn omitted() -> Self {
        Self {
            diagnostics: Box::new([]),
            omitted_diagnostics: 1,
        }
    }
}

impl std::fmt::Display for EnvironmentPublicationProjectionReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "environment publication projection failed with {} diagnostic(s) and {} omitted",
            self.diagnostics.len(),
            self.omitted_diagnostics
        )
    }
}

impl std::error::Error for EnvironmentPublicationProjectionReport {}

impl AcceptedNominalWorld {
    pub(crate) fn try_project_character_dialogue_field_type(
        &self,
        root: &EnvironmentTypeProjectionNode,
        item: &EnvironmentPublicationItemId,
        limits: NominalResolutionLimits,
    ) -> Result<TypeKind, EnvironmentPublicationProjectionReport> {
        self.project_callable_type(
            root,
            item,
            EnvironmentTypeSiteRoot::CharacterDialogueCustomField,
            limits,
        )
    }

    pub(crate) fn try_project_environment_bindings(
        &self,
        input: &BoundEnvironmentRegistrationInput,
        nominal_limits: NominalResolutionLimits,
        _aggregation_limits: NominalAggregationLimits,
    ) -> Result<Vec<(EnvironmentBindingId, TypeKind)>, EnvironmentPublicationProjectionReport> {
        if input.world() != self.world() {
            let Some(binding) = input.input().value_bindings().first() else {
                return Err(EnvironmentPublicationProjectionReport::omitted());
            };
            return Err(EnvironmentPublicationProjectionReport::one(Box::new(
                EnvironmentPublicationProjectionDiagnostic {
                    item: binding.item().clone(),
                    site: EnvironmentTypeSite::new(
                        EnvironmentTypeSiteRoot::SymbolType,
                        Vec::<EnvironmentTypeSiteStep>::new(),
                    ),
                    primary: binding.ty().source().clone(),
                    related: Box::new([]),
                    kind: Box::new(EnvironmentPublicationProjectionErrorKind::WorldMismatch),
                },
            )));
        }

        let mut bindings = Vec::with_capacity(input.input().value_bindings().len());
        for binding in input.input().value_bindings() {
            bindings.push((
                binding.id().clone(),
                self.project_callable_type(
                    binding.ty(),
                    binding.item(),
                    EnvironmentTypeSiteRoot::SymbolType,
                    nominal_limits,
                )?,
            ));
        }
        bindings.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(bindings)
    }

    /// Projects one stable batch of Rust ADT declarations against this exact
    /// accepted nominal world.
    pub fn try_project_rust_metadata(
        &self,
        inputs: &[RustTypeMetadataPublicationInput],
        nominal_limits: NominalResolutionLimits,
        _aggregation_limits: NominalAggregationLimits,
    ) -> Result<AcceptedRustTypeMetadataCatalog, EnvironmentPublicationProjectionReport> {
        let records = inputs
            .iter()
            .map(|input| self.project_rust_metadata_record(input, nominal_limits))
            .collect::<Result<Vec<_>, _>>()?;

        AcceptedRustTypeMetadataCatalog::try_new(records).map_err(|error| {
            let Some(input) = inputs.first() else {
                return EnvironmentPublicationProjectionReport::omitted();
            };
            EnvironmentPublicationProjectionReport::one(type_diagnostic(
                input.item(),
                EnvironmentTypeSiteRoot::RustNewtypeInner,
                &[],
                input.source(),
                EnvironmentPublicationProjectionErrorKind::RustMetadataCatalog { error },
            ))
        })
    }

    fn project_rust_metadata_record(
        &self,
        input: &RustTypeMetadataPublicationInput,
        nominal_limits: NominalResolutionLimits,
    ) -> Result<AcceptedRustTypeMetadata, EnvironmentPublicationProjectionReport> {
        if input.package_provenance().name() != input.package().as_str()
            || !matches!(
                input.id().owner(),
                AcceptedNominalOwnerId::RustPackage(package) if package == input.package()
            )
        {
            return Err(EnvironmentPublicationProjectionReport::one(
                type_diagnostic(
                    input.item(),
                    EnvironmentTypeSiteRoot::RustNewtypeInner,
                    &[],
                    input.source(),
                    EnvironmentPublicationProjectionErrorKind::MetadataOwnerMismatch {
                        declaration: input.id().clone(),
                        package: input.package().clone(),
                    },
                ),
            ));
        }

        let accepted = self.accepted_record(input.id()).map_err(|error| {
            EnvironmentPublicationProjectionReport::one(nominal_lookup_diagnostic(
                input.item(),
                EnvironmentTypeSiteRoot::RustNewtypeInner,
                &[],
                input.source(),
                error,
            ))
        })?;
        if usize::from(accepted.arity()) != input.parameters().len() {
            return Err(EnvironmentPublicationProjectionReport::one(
                type_diagnostic(
                    input.item(),
                    EnvironmentTypeSiteRoot::RustNewtypeInner,
                    &[],
                    input.source(),
                    EnvironmentPublicationProjectionErrorKind::WrongArity {
                        nominal: input.id().clone(),
                        expected: accepted.arity(),
                        actual: input.parameters().len(),
                    },
                ),
            ));
        }

        let (parameters, binder) = metadata_binder(input)?;
        let kind = self.project_rust_metadata_kind(input, &binder, nominal_limits)?;
        Ok(AcceptedRustTypeMetadata::new(
            input.id().clone(),
            input.package().clone(),
            input.package_provenance().clone(),
            input.rust_item().clone(),
            parameters,
            kind,
            input.source().clone(),
        ))
    }

    fn project_rust_metadata_kind(
        &self,
        input: &RustTypeMetadataPublicationInput,
        binder: &MetadataBinder,
        limits: NominalResolutionLimits,
    ) -> Result<AcceptedRustTypeMetadataKind, EnvironmentPublicationProjectionReport> {
        match input.kind() {
            RustTypeMetadataPublicationKind::Struct { shape } => {
                Ok(AcceptedRustTypeMetadataKind::Struct {
                    shape: self.project_rust_struct_shape(input, shape, binder, limits)?,
                })
            }
            RustTypeMetadataPublicationKind::Enum { variants } => {
                Ok(AcceptedRustTypeMetadataKind::Enum {
                    variants: self.project_rust_enum_variants(input, variants, binder, limits)?,
                })
            }
            RustTypeMetadataPublicationKind::Newtype { inner } => {
                Ok(AcceptedRustTypeMetadataKind::Newtype {
                    inner: self.project_metadata_type(
                        inner,
                        input.item(),
                        EnvironmentTypeSiteRoot::RustNewtypeInner,
                        limits,
                        binder,
                    )?,
                })
            }
        }
    }

    fn project_rust_struct_shape(
        &self,
        input: &RustTypeMetadataPublicationInput,
        shape: &RustStructMetadataInput,
        binder: &MetadataBinder,
        limits: NominalResolutionLimits,
    ) -> Result<AcceptedRustStructShape, EnvironmentPublicationProjectionReport> {
        match shape {
            RustStructMetadataInput::Unit => Ok(AcceptedRustStructShape::Unit),
            RustStructMetadataInput::Tuple(fields) => fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    let field_index = metadata_field_index(
                        input,
                        EnvironmentTypeSiteRoot::RustStructTupleField { field: u16::MAX },
                        field,
                        index,
                    )?;
                    self.project_metadata_type(
                        field,
                        input.item(),
                        EnvironmentTypeSiteRoot::RustStructTupleField { field: field_index },
                        limits,
                        binder,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|fields| AcceptedRustStructShape::Tuple(fields.into_boxed_slice())),
            RustStructMetadataInput::Record(fields) => fields
                .iter()
                .map(|(name, field)| {
                    Ok((
                        name.clone(),
                        self.project_metadata_type(
                            field,
                            input.item(),
                            EnvironmentTypeSiteRoot::RustStructRecordField {
                                field: name.clone(),
                            },
                            limits,
                            binder,
                        )?,
                    ))
                })
                .collect::<Result<Vec<_>, EnvironmentPublicationProjectionReport>>()
                .map(|fields| AcceptedRustStructShape::Record(fields.into_boxed_slice())),
        }
    }

    fn project_rust_enum_variants(
        &self,
        input: &RustTypeMetadataPublicationInput,
        variants: &[RustVariantMetadataInput],
        binder: &MetadataBinder,
        limits: NominalResolutionLimits,
    ) -> Result<BTreeMap<String, EnumVariantPayload>, EnvironmentPublicationProjectionReport> {
        let mut projected = BTreeMap::new();
        for variant in variants {
            let payload = self.project_rust_variant_payload(input, variant, binder, limits)?;
            if projected
                .insert(variant.name().to_owned(), payload)
                .is_some()
            {
                return Err(EnvironmentPublicationProjectionReport::one(
                    type_diagnostic(
                        input.item(),
                        EnvironmentTypeSiteRoot::RustNewtypeInner,
                        &[],
                        variant.source(),
                        EnvironmentPublicationProjectionErrorKind::RustMetadataCatalog {
                            error: AcceptedRustTypeMetadataCatalogError::DuplicateVariant {
                                id: input.id().clone(),
                                variant: variant.name().to_owned(),
                            },
                        },
                    ),
                ));
            }
        }
        Ok(projected)
    }

    fn project_rust_variant_payload(
        &self,
        input: &RustTypeMetadataPublicationInput,
        variant: &RustVariantMetadataInput,
        binder: &MetadataBinder,
        limits: NominalResolutionLimits,
    ) -> Result<EnumVariantPayload, EnvironmentPublicationProjectionReport> {
        match variant.payload() {
            RustVariantPayloadInput::Unit => Ok(EnumVariantPayload::Unit),
            RustVariantPayloadInput::Tuple(fields) => fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    let field_index = metadata_field_index(
                        input,
                        EnvironmentTypeSiteRoot::RustEnumTupleField {
                            variant: variant.name().to_owned(),
                            field: u16::MAX,
                        },
                        field,
                        index,
                    )?;
                    self.project_metadata_type(
                        field,
                        input.item(),
                        EnvironmentTypeSiteRoot::RustEnumTupleField {
                            variant: variant.name().to_owned(),
                            field: field_index,
                        },
                        limits,
                        binder,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map(EnumVariantPayload::Tuple),
            RustVariantPayloadInput::Record(fields) => fields
                .iter()
                .map(|(name, field)| {
                    Ok((
                        name.clone(),
                        self.project_metadata_type(
                            field,
                            input.item(),
                            EnvironmentTypeSiteRoot::RustEnumRecordField {
                                variant: variant.name().to_owned(),
                                field: name.clone(),
                            },
                            limits,
                            binder,
                        )?,
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, EnvironmentPublicationProjectionReport>>()
                .map(EnumVariantPayload::Record),
        }
    }

    pub(crate) fn try_project_environment_publication(
        &self,
        input: &BoundEnvironmentRegistrationInput,
        nominal_limits: NominalResolutionLimits,
        _aggregation_limits: NominalAggregationLimits,
        callable_limits: &super::CallableLimits,
    ) -> Result<EnvironmentCallablePublication, EnvironmentPublicationProjectionReport> {
        if input.world() != self.world() {
            let Some(record) = input.input().callable_records().first() else {
                return Err(EnvironmentPublicationProjectionReport::omitted());
            };
            return Err(EnvironmentPublicationProjectionReport::one(Box::new(
                EnvironmentPublicationProjectionDiagnostic {
                    item: record.item().clone(),
                    site: EnvironmentTypeSite::new(
                        EnvironmentTypeSiteRoot::Result,
                        Vec::<EnvironmentTypeSiteStep>::new(),
                    ),
                    primary: record.schema().result().source().clone(),
                    related: Box::new([]),
                    kind: Box::new(EnvironmentPublicationProjectionErrorKind::WorldMismatch),
                },
            )));
        }

        let records = input
            .input()
            .callable_records()
            .iter()
            .map(|record| self.project_environment_record(record, nominal_limits, callable_limits))
            .collect::<Result<Vec<_>, _>>()?;
        EnvironmentCallablePublication::try_new_projected(
            input.input().owner().clone(),
            self.stamp(),
            input.input().manifest_digest(),
            records,
            callable_limits,
        )
        .map_err(|error| {
            let Some(record) = input.input().callable_records().first() else {
                return EnvironmentPublicationProjectionReport::omitted();
            };
            callable_diagnostic(
                record.item(),
                EnvironmentTypeSiteRoot::Result,
                record.schema().result().source(),
                error,
            )
        })
    }

    fn project_environment_record(
        &self,
        record: &EnvironmentCallablePublicationRecordInput,
        nominal_limits: NominalResolutionLimits,
        callable_limits: &super::CallableLimits,
    ) -> Result<EnvironmentCallablePublicationRecord, EnvironmentPublicationProjectionReport> {
        let key = match record.key() {
            EnvironmentCallableLookupInput::Free(path) => {
                CallableLookupKey::Free(path.path().clone())
            }
            EnvironmentCallableLookupInput::Method { receiver, method } => {
                let receiver = self.project_callable_type(
                    receiver,
                    record.item(),
                    EnvironmentTypeSiteRoot::MethodReceiver,
                    nominal_limits,
                )?;
                CallableLookupKey::Method(ReceiverMethodKey::new(receiver, method.clone()))
            }
        };
        let groups = record
            .schema()
            .groups()
            .iter()
            .map(|group| {
                self.project_parameter_group(record, group, nominal_limits, callable_limits)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.project_callable_type(
            record.schema().result(),
            record.item(),
            EnvironmentTypeSiteRoot::Result,
            nominal_limits,
        )?;
        let schema = CallableSignatureSchema::try_new(
            groups,
            result,
            CallableEffectSchema::fixed(record.schema().effects().clone()),
            record.schema().argument_policy(),
            record.schema().validator().clone(),
            callable_limits,
        )
        .map_err(CallablePublicationError::from)
        .map_err(|error| callable_record_diagnostic(record, error))?;
        EnvironmentCallablePublicationRecord::try_new(
            record.kind(),
            key,
            record.overload(),
            schema,
            record.documentation().clone(),
            record.source().cloned(),
            record.rust().cloned(),
            record.declaration_order(),
        )
        .map_err(|error| callable_record_diagnostic(record, error))
    }

    fn project_parameter_group(
        &self,
        record: &EnvironmentCallablePublicationRecordInput,
        group: &EnvironmentParameterGroupInput,
        nominal_limits: NominalResolutionLimits,
        callable_limits: &super::CallableLimits,
    ) -> Result<CallableParameterGroup, EnvironmentPublicationProjectionReport> {
        let parameters = group
            .parameters()
            .iter()
            .map(|parameter| self.project_parameter(record, group, parameter, nominal_limits))
            .collect::<Result<Vec<_>, _>>()?;
        CallableParameterGroup::try_new(group.index(), group.kind(), parameters, callable_limits)
            .map_err(CallablePublicationError::from)
            .map_err(|error| callable_record_diagnostic(record, error))
    }

    fn project_parameter(
        &self,
        record: &EnvironmentCallablePublicationRecordInput,
        group: &EnvironmentParameterGroupInput,
        parameter: &EnvironmentParameterInput,
        nominal_limits: NominalResolutionLimits,
    ) -> Result<CallableParameter, EnvironmentPublicationProjectionReport> {
        let site = EnvironmentTypeSiteRoot::Parameter {
            group: group.index(),
            parameter: parameter.index(),
        };
        let ty = match parameter.ty() {
            EnvironmentParameterTypeInput::Exact(ty) => CallableParameterType::Exact(
                self.project_callable_type(ty, record.item(), site.clone(), nominal_limits)?,
            ),
            EnvironmentParameterTypeInput::Unchecked { .. } => CallableParameterType::Unchecked,
        };
        CallableParameter::try_new(
            parameter.index(),
            parameter.name().cloned(),
            ty,
            parameter.passing(),
            parameter.presence(),
            parameter.documentation().map(std::sync::Arc::from),
            parameter.source().cloned(),
        )
        .map_err(CallablePublicationError::from)
        .map_err(|error| callable_diagnostic(record.item(), site, parameter.ty().source(), error))
    }

    fn project_callable_type(
        &self,
        root: &EnvironmentTypeProjectionNode,
        item: &EnvironmentPublicationItemId,
        site_root: EnvironmentTypeSiteRoot,
        limits: NominalResolutionLimits,
    ) -> Result<TypeKind, EnvironmentPublicationProjectionReport> {
        TypeProjector::new(self, item, site_root, limits, None)
            .project(root, 1)
            .map_err(EnvironmentPublicationProjectionReport::one)
    }

    fn project_metadata_type(
        &self,
        root: &EnvironmentTypeProjectionNode,
        item: &EnvironmentPublicationItemId,
        site_root: EnvironmentTypeSiteRoot,
        limits: NominalResolutionLimits,
        binder: &MetadataBinder,
    ) -> Result<TypeKind, EnvironmentPublicationProjectionReport> {
        TypeProjector::new(self, item, site_root, limits, Some(binder))
            .project(root, 1)
            .map_err(EnvironmentPublicationProjectionReport::one)
    }
}

struct TypeProjector<'a> {
    world: &'a AcceptedNominalWorld,
    item: &'a EnvironmentPublicationItemId,
    root: EnvironmentTypeSiteRoot,
    steps: Vec<EnvironmentTypeSiteStep>,
    limits: NominalResolutionLimits,
    meter: ProjectionMeter,
    binder: Option<&'a MetadataBinder>,
}

impl<'a> TypeProjector<'a> {
    fn new(
        world: &'a AcceptedNominalWorld,
        item: &'a EnvironmentPublicationItemId,
        root: EnvironmentTypeSiteRoot,
        limits: NominalResolutionLimits,
        binder: Option<&'a MetadataBinder>,
    ) -> Self {
        Self {
            world,
            item,
            root,
            steps: Vec::new(),
            limits,
            meter: ProjectionMeter::default(),
            binder,
        }
    }

    fn project(
        &mut self,
        node: &EnvironmentTypeProjectionNode,
        depth: u16,
    ) -> Result<TypeKind, Box<EnvironmentPublicationProjectionDiagnostic>> {
        self.charge(node, depth)?;
        let next_depth = depth.saturating_add(1);
        match node.kind() {
            EnvironmentTypeProjectionKind::Unit => Ok(TypeKind::Unit),
            EnvironmentTypeProjectionKind::Bool => Ok(TypeKind::Bool),
            EnvironmentTypeProjectionKind::I8 => Ok(TypeKind::I8),
            EnvironmentTypeProjectionKind::I16 => Ok(TypeKind::I16),
            EnvironmentTypeProjectionKind::I32 => Ok(TypeKind::I32),
            EnvironmentTypeProjectionKind::I64 => Ok(TypeKind::I64),
            EnvironmentTypeProjectionKind::I128 => Ok(TypeKind::I128),
            EnvironmentTypeProjectionKind::ISize => Ok(TypeKind::ISize),
            EnvironmentTypeProjectionKind::U8 => Ok(TypeKind::U8),
            EnvironmentTypeProjectionKind::U16 => Ok(TypeKind::U16),
            EnvironmentTypeProjectionKind::U32 => Ok(TypeKind::U32),
            EnvironmentTypeProjectionKind::U64 => Ok(TypeKind::U64),
            EnvironmentTypeProjectionKind::U128 => Ok(TypeKind::U128),
            EnvironmentTypeProjectionKind::USize => Ok(TypeKind::USize),
            EnvironmentTypeProjectionKind::F32 => Ok(TypeKind::F32),
            EnvironmentTypeProjectionKind::F64 => Ok(TypeKind::F64),
            EnvironmentTypeProjectionKind::String => Ok(TypeKind::String),
            EnvironmentTypeProjectionKind::Char => Ok(TypeKind::Char),
            EnvironmentTypeProjectionKind::Vec(child) => self
                .boxed_child(child, EnvironmentTypeSiteStep::VecItem, next_depth)
                .map(TypeKind::Vec),
            EnvironmentTypeProjectionKind::Seq(child) => self
                .boxed_child(child, EnvironmentTypeSiteStep::SeqItem, next_depth)
                .map(TypeKind::Seq),
            EnvironmentTypeProjectionKind::Option(child) => self
                .boxed_child(child, EnvironmentTypeSiteStep::OptionItem, next_depth)
                .map(TypeKind::Option),
            EnvironmentTypeProjectionKind::Result { ok, error } => {
                let (ok, error) = self.project_pair(
                    ok,
                    EnvironmentTypeSiteStep::ResultOk,
                    error,
                    EnvironmentTypeSiteStep::ResultError,
                    next_depth,
                )?;
                Ok(TypeKind::Result { ok, error })
            }
            EnvironmentTypeProjectionKind::Tuple(items) => {
                self.project_tuple(items, next_depth).map(TypeKind::Tuple)
            }
            EnvironmentTypeProjectionKind::Need { ready, error } => {
                let (ready, error) = self.project_pair(
                    ready,
                    EnvironmentTypeSiteStep::NeedReady,
                    error,
                    EnvironmentTypeSiteStep::NeedError,
                    next_depth,
                )?;
                Ok(TypeKind::Need { ready, error })
            }
            EnvironmentTypeProjectionKind::CharacterNominal(nominal) => {
                Ok(TypeKind::CharacterNominal(nominal.clone()))
            }
            EnvironmentTypeProjectionKind::AcceptedNominal { id, arguments } => {
                self.project_accepted_nominal(node, id, arguments, next_depth)
            }
            EnvironmentTypeProjectionKind::TypeParameter { index } => {
                self.project_type_parameter(node, *index)
            }
        }
    }

    fn charge(
        &mut self,
        node: &EnvironmentTypeProjectionNode,
        depth: u16,
    ) -> Result<(), Box<EnvironmentPublicationProjectionDiagnostic>> {
        self.meter.nodes = self.meter.nodes.saturating_add(1);
        if self.meter.nodes > self.limits.type_nodes_per_reference() {
            return Err(self.diagnostic(
                node,
                EnvironmentPublicationProjectionErrorKind::LimitExceeded {
                    kind: NominalResolutionLimitKind::TypeNodesPerReference,
                    observed: self.meter.nodes,
                    maximum: self.limits.type_nodes_per_reference(),
                },
            ));
        }
        if depth > self.limits.recursive_type_depth() {
            return Err(self.diagnostic(
                node,
                EnvironmentPublicationProjectionErrorKind::LimitExceeded {
                    kind: NominalResolutionLimitKind::RecursiveTypeDepth,
                    observed: u64::from(depth),
                    maximum: u64::from(self.limits.recursive_type_depth()),
                },
            ));
        }
        Ok(())
    }

    fn project_tuple(
        &mut self,
        items: &[EnvironmentTypeProjectionNode],
        depth: u16,
    ) -> Result<Vec<TypeKind>, Box<EnvironmentPublicationProjectionDiagnostic>> {
        items
            .iter()
            .enumerate()
            .map(|(index, child)| {
                self.child(
                    child,
                    EnvironmentTypeSiteStep::TupleItem(u16::try_from(index).unwrap_or(u16::MAX)),
                    depth,
                )
            })
            .collect()
    }

    fn project_pair(
        &mut self,
        first: &EnvironmentTypeProjectionNode,
        first_step: EnvironmentTypeSiteStep,
        second: &EnvironmentTypeProjectionNode,
        second_step: EnvironmentTypeSiteStep,
        depth: u16,
    ) -> Result<(Box<TypeKind>, Box<TypeKind>), Box<EnvironmentPublicationProjectionDiagnostic>>
    {
        let first = self.boxed_child(first, first_step, depth)?;
        let second = self.boxed_child(second, second_step, depth)?;
        Ok((first, second))
    }

    fn project_accepted_nominal(
        &mut self,
        node: &EnvironmentTypeProjectionNode,
        id: &AcceptedNominalId,
        arguments: &[EnvironmentTypeProjectionNode],
        depth: u16,
    ) -> Result<TypeKind, Box<EnvironmentPublicationProjectionDiagnostic>> {
        let record = self.world.accepted_record(id).map_err(|error| {
            nominal_lookup_diagnostic(
                self.item,
                self.root.clone(),
                &self.steps,
                node.source(),
                error,
            )
        })?;
        if arguments.len() != usize::from(record.arity()) {
            return Err(self.diagnostic(
                node,
                EnvironmentPublicationProjectionErrorKind::WrongArity {
                    nominal: id.clone(),
                    expected: record.arity(),
                    actual: arguments.len(),
                },
            ));
        }
        if arguments.len() > usize::from(self.limits.generic_arguments_per_application()) {
            return Err(self.diagnostic(
                node,
                EnvironmentPublicationProjectionErrorKind::LimitExceeded {
                    kind: NominalResolutionLimitKind::GenericArgumentsPerApplication,
                    observed: u64::try_from(arguments.len()).unwrap_or(u64::MAX),
                    maximum: u64::from(self.limits.generic_arguments_per_application()),
                },
            ));
        }
        let arguments = arguments
            .iter()
            .enumerate()
            .map(|(index, child)| {
                self.child(
                    child,
                    EnvironmentTypeSiteStep::NominalArgument(
                        u16::try_from(index).unwrap_or(u16::MAX),
                    ),
                    depth,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        record.try_instantiate(arguments).map_err(|error| {
            nominal_instantiation_diagnostic(
                self.item,
                self.root.clone(),
                &self.steps,
                node.source(),
                id,
                &error,
            )
        })
    }

    fn project_type_parameter(
        &self,
        node: &EnvironmentTypeProjectionNode,
        index: ArcweftRustTypeParameterIndex,
    ) -> Result<TypeKind, Box<EnvironmentPublicationProjectionDiagnostic>> {
        let Some(binder) = self.binder else {
            return Err(self.diagnostic(
                node,
                EnvironmentPublicationProjectionErrorKind::FreeTypeParameterInCallable { index },
            ));
        };
        let Some(parameter) = binder.parameters.get(&index) else {
            return Err(self.diagnostic(
                node,
                EnvironmentPublicationProjectionErrorKind::UnboundMetadataTypeParameter {
                    owner: binder.owner.clone(),
                    index,
                },
            ));
        };
        Ok(TypeKind::GenericParam(parameter.clone()))
    }

    fn boxed_child(
        &mut self,
        node: &EnvironmentTypeProjectionNode,
        step: EnvironmentTypeSiteStep,
        depth: u16,
    ) -> Result<Box<TypeKind>, Box<EnvironmentPublicationProjectionDiagnostic>> {
        self.child(node, step, depth).map(Box::new)
    }

    fn child(
        &mut self,
        node: &EnvironmentTypeProjectionNode,
        step: EnvironmentTypeSiteStep,
        depth: u16,
    ) -> Result<TypeKind, Box<EnvironmentPublicationProjectionDiagnostic>> {
        self.steps.push(step);
        let projected = self.project(node, depth);
        self.steps.pop();
        projected
    }

    fn diagnostic(
        &self,
        node: &EnvironmentTypeProjectionNode,
        kind: EnvironmentPublicationProjectionErrorKind,
    ) -> Box<EnvironmentPublicationProjectionDiagnostic> {
        type_diagnostic(
            self.item,
            self.root.clone(),
            &self.steps,
            node.source(),
            kind,
        )
    }
}

#[derive(Default)]
struct ProjectionMeter {
    nodes: u64,
}

struct MetadataBinder {
    owner: AcceptedNominalId,
    parameters: BTreeMap<ArcweftRustTypeParameterIndex, GenericTypeParameterId>,
}

fn metadata_binder(
    input: &RustTypeMetadataPublicationInput,
) -> Result<(Vec<GenericTypeParameterId>, MetadataBinder), EnvironmentPublicationProjectionReport> {
    let mut parameters = Vec::with_capacity(input.parameters().len());
    let mut by_index = BTreeMap::new();
    for parameter in input.parameters() {
        let error = || {
            EnvironmentPublicationProjectionReport::one(type_diagnostic(
                input.item(),
                EnvironmentTypeSiteRoot::RustNewtypeInner,
                &[],
                parameter.source(),
                EnvironmentPublicationProjectionErrorKind::UnboundMetadataTypeParameter {
                    owner: input.id().clone(),
                    index: parameter.index(),
                },
            ))
        };
        let ordinal = u16::try_from(parameter.index().get()).map_err(|_| error())?;
        let id = GenericTypeParameterId::new(
            GenericTypeOwnerId::AcceptedNominal(input.id().clone()),
            ordinal,
        );
        if by_index.insert(parameter.index(), id.clone()).is_some() {
            return Err(error());
        }
        parameters.push(id);
    }
    Ok((
        parameters,
        MetadataBinder {
            owner: input.id().clone(),
            parameters: by_index,
        },
    ))
}

fn metadata_field_index(
    input: &RustTypeMetadataPublicationInput,
    root: EnvironmentTypeSiteRoot,
    field: &EnvironmentTypeProjectionNode,
    index: usize,
) -> Result<u16, EnvironmentPublicationProjectionReport> {
    u16::try_from(index).map_err(|_| {
        EnvironmentPublicationProjectionReport::one(type_diagnostic(
            input.item(),
            root,
            &[],
            field.source(),
            EnvironmentPublicationProjectionErrorKind::LimitExceeded {
                kind: NominalResolutionLimitKind::TypeNodesPerReference,
                observed: u64::try_from(index).unwrap_or(u64::MAX),
                maximum: u64::from(u16::MAX),
            },
        ))
    })
}

fn callable_record_diagnostic(
    record: &EnvironmentCallablePublicationRecordInput,
    error: CallablePublicationError,
) -> EnvironmentPublicationProjectionReport {
    callable_diagnostic(
        record.item(),
        EnvironmentTypeSiteRoot::Result,
        record.schema().result().source(),
        error,
    )
}

fn type_diagnostic(
    item: &EnvironmentPublicationItemId,
    root: EnvironmentTypeSiteRoot,
    steps: &[EnvironmentTypeSiteStep],
    source: &SourceSpan,
    kind: EnvironmentPublicationProjectionErrorKind,
) -> Box<EnvironmentPublicationProjectionDiagnostic> {
    Box::new(EnvironmentPublicationProjectionDiagnostic {
        item: item.clone(),
        site: EnvironmentTypeSite::new(root, steps.to_vec().into_boxed_slice()),
        primary: source.clone(),
        related: Box::new([]),
        kind: Box::new(kind),
    })
}

fn callable_diagnostic(
    item: &EnvironmentPublicationItemId,
    root: EnvironmentTypeSiteRoot,
    source: &SourceSpan,
    error: CallablePublicationError,
) -> EnvironmentPublicationProjectionReport {
    EnvironmentPublicationProjectionReport::one(type_diagnostic(
        item,
        root,
        &[],
        source,
        EnvironmentPublicationProjectionErrorKind::Callable { error },
    ))
}

fn nominal_lookup_diagnostic(
    item: &EnvironmentPublicationItemId,
    root: EnvironmentTypeSiteRoot,
    steps: &[EnvironmentTypeSiteStep],
    source: &SourceSpan,
    error: AcceptedNominalWorldLookupError,
) -> Box<EnvironmentPublicationProjectionDiagnostic> {
    let kind = match error {
        AcceptedNominalWorldLookupError::Unknown { requested } => {
            EnvironmentPublicationProjectionErrorKind::UnknownPath {
                requested: *requested,
            }
        }
        AcceptedNominalWorldLookupError::Inaccessible { requested } => {
            EnvironmentPublicationProjectionErrorKind::InaccessibleExport {
                requested: *requested,
            }
        }
        AcceptedNominalWorldLookupError::OwnerMismatch { requested, visible } => {
            EnvironmentPublicationProjectionErrorKind::OwnerMismatch {
                requested: *requested,
                visible: *visible,
            }
        }
    };
    type_diagnostic(item, root, steps, source, kind)
}

fn nominal_instantiation_diagnostic(
    item: &EnvironmentPublicationItemId,
    root: EnvironmentTypeSiteRoot,
    steps: &[EnvironmentTypeSiteStep],
    source: &SourceSpan,
    nominal: &AcceptedNominalId,
    error: &AcceptedNominalInstantiationError,
) -> Box<EnvironmentPublicationProjectionDiagnostic> {
    let kind = match error {
        AcceptedNominalInstantiationError::WrongArity {
            expected, actual, ..
        } => EnvironmentPublicationProjectionErrorKind::WrongArity {
            nominal: nominal.clone(),
            expected: *expected,
            actual: *actual,
        },
        AcceptedNominalInstantiationError::InvalidSemantics { .. } => {
            EnvironmentPublicationProjectionErrorKind::InvalidAcceptedSemantics {
                nominal: nominal.clone(),
            }
        }
    };
    type_diagnostic(item, root, steps, source, kind)
}
