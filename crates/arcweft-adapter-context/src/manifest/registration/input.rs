//! Typed sema registration input derived from one validated adapter manifest.

mod digest;
pub(super) mod source;

use std::sync::Arc;

use arcweft_lang_hir::symbol::CallablePackageId;
use arcweft_lang_sema::{
    callable::{
        AdapterPackageId, CallableArgumentPolicy, CallableDocumentation, CallableGroupIndex,
        CallableGroupKind, CallableName, CallableOverloadIndex, CallableParameterDocumentation,
        CallableParameterIndex, CallableParameterPassing, CallableParameterPresence, CallablePath,
        CallableValidator, DocumentationProvenance, EnvironmentCallableKind,
        EnvironmentCallableOwner, EnvironmentDeclarationOrdinal, ProjectCallablePath,
        RustCallableProvenance, RustCallablePurity, RustItemPath, RustPackageProvenance,
        SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
    },
    effect_row::EffectRow,
    effects::EffectSet,
    env::{
        identity::EnvironmentBindingId,
        nominal::{
            AcceptedNominalId, AcceptedNominalOrigin, AcceptedNominalOwnerId, RustPackageId,
        },
        rust_metadata::{
            RustStructMetadataInput, RustTypeMetadataPublicationIdentity,
            RustTypeMetadataPublicationInput, RustTypeMetadataPublicationKind,
            RustTypeParameterPublicationInput, RustVariantMetadataInput, RustVariantPayloadInput,
        },
    },
    registration::{
        AcceptedNominalInputVisibility, AcceptedNominalInventoryInput,
        EnvironmentCallableLookupInput, EnvironmentCallablePublicationMetadataInput,
        EnvironmentCallablePublicationRecordInput, EnvironmentCallableSignatureInput,
        EnvironmentParameterGroupInput, EnvironmentParameterInput,
        EnvironmentParameterMetadataInput, EnvironmentParameterTypeInput,
        EnvironmentPublicationItemId, EnvironmentTypeProjectionKind, EnvironmentTypeProjectionNode,
        EnvironmentTypeSiteRoot, EnvironmentTypeSiteStep, EnvironmentValueBindingInput,
        SourceBackedEnvironmentRegistrationInput,
    },
};
use arcweft_lang_syntax::ast::{
    module_path::{CanonicalModulePath, ModulePathRoot},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment},
};
use arcweft_lang_syntax::types::TypePath;
use arcweft_rust_abi::{
    ArcweftRustField, ArcweftRustPackage, ArcweftRustPackageId, ArcweftRustStructShape,
    ArcweftRustTypeKind, ArcweftRustTypePath, ArcweftRustTypeRef, ArcweftRustVariant,
    ArcweftRustVariantPayload,
};
use arcweft_source::{SourceDocument, SourceSpan};

use crate::manifest::{
    AdapterCallableName, AdapterCallablePath, AdapterEffectCapability, AdapterEnvironmentOwnerId,
    AdapterFreeCallableKind, AdapterFunctionSignature, AdapterManifest, AdapterNominalOwner,
    AdapterNominalPath, AdapterNominalTypeRef, AdapterNominalVisibility, AdapterParameterPassing,
    AdapterParameterPresence, AdapterRustPackageMountTable, AdapterToolingSubject, AdapterTypeKind,
};

use super::AdapterRegistrationFactsError;

struct TypeSource<'a> {
    document: &'a SourceDocument,
    map: &'a source::RegistrationSourceMap,
    item: &'a EnvironmentPublicationItemId,
    root: EnvironmentTypeSiteRoot,
    steps: Box<[EnvironmentTypeSiteStep]>,
}

impl<'a> TypeSource<'a> {
    fn new(
        document: &'a SourceDocument,
        map: &'a source::RegistrationSourceMap,
        item: &'a EnvironmentPublicationItemId,
        root: EnvironmentTypeSiteRoot,
    ) -> Self {
        Self {
            document,
            map,
            item,
            root,
            steps: Box::new([]),
        }
    }

    fn child(&self, step: EnvironmentTypeSiteStep) -> Self {
        let mut steps = self.steps.to_vec();
        steps.push(step);
        Self {
            document: self.document,
            map: self.map,
            item: self.item,
            root: self.root.clone(),
            steps: steps.into_boxed_slice(),
        }
    }

    fn span(&self) -> Result<SourceSpan, AdapterRegistrationFactsError> {
        Ok(self.document.span(
            self.map
                .type_range(self.item, self.root.clone(), &self.steps)?,
        )?)
    }
}

fn item_source(
    document: &SourceDocument,
    source_map: &source::RegistrationSourceMap,
    item: &EnvironmentPublicationItemId,
) -> Result<SourceSpan, AdapterRegistrationFactsError> {
    Ok(document.span(source_map.item_range(item)?)?)
}

pub(super) fn environment_input(
    manifest: &AdapterManifest,
    owner: EnvironmentCallableOwner,
    document: &SourceDocument,
    source_map: &source::RegistrationSourceMap,
) -> Result<SourceBackedEnvironmentRegistrationInput, AdapterRegistrationFactsError> {
    validate_tooling(manifest)?;
    EnvironmentInputProjector::new(manifest, owner, document, source_map)?.project()
}

struct EnvironmentInputProjector<'a> {
    manifest: &'a AdapterManifest,
    owner: EnvironmentCallableOwner,
    document: &'a SourceDocument,
    source_map: &'a source::RegistrationSourceMap,
    environment_owner: AdapterEnvironmentOwnerId,
    semantic_environment_owner: EnvironmentBindingId,
    adapter: AdapterPackageId,
    callable_package: CallablePackageId,
}

struct CallableRecordProjection<'a> {
    item: EnvironmentPublicationItemId,
    kind: EnvironmentCallableKind,
    key: EnvironmentCallableLookupInput,
    overload_index: usize,
    signature: &'a AdapterFunctionSignature,
    effects: &'a [AdapterEffectCapability],
    declaration_order: usize,
    subject: &'a AdapterToolingSubject,
    rust: Option<RustCallableProvenance>,
}

impl<'a> EnvironmentInputProjector<'a> {
    fn new(
        manifest: &'a AdapterManifest,
        owner: EnvironmentCallableOwner,
        document: &'a SourceDocument,
        source_map: &'a source::RegistrationSourceMap,
    ) -> Result<Self, AdapterRegistrationFactsError> {
        let environment_owner = AdapterEnvironmentOwnerId::for_adapter(manifest.id());
        Ok(Self {
            manifest,
            owner,
            document,
            source_map,
            semantic_environment_owner: EnvironmentBindingId::try_new(environment_owner.as_str())?,
            environment_owner,
            adapter: AdapterPackageId::try_new(manifest.id().as_str())?,
            callable_package: CallablePackageId::try_new(manifest.id().as_str())?,
        })
    }

    fn project(
        &self,
    ) -> Result<SourceBackedEnvironmentRegistrationInput, AdapterRegistrationFactsError> {
        let mut nominal_inventory = self.adapter_nominals()?;
        let value_bindings = self.value_bindings()?;
        let rust_metadata = self.rust_metadata(&mut nominal_inventory)?;
        let callable_records = self.callable_records()?;
        Ok(SourceBackedEnvironmentRegistrationInput::new(
            self.owner.clone(),
            self.document.identity().clone(),
            digest::manifest_digest(self.manifest),
            nominal_inventory,
            value_bindings,
            rust_metadata,
            callable_records,
        ))
    }

    fn adapter_nominals(
        &self,
    ) -> Result<Vec<AcceptedNominalInventoryInput>, AdapterRegistrationFactsError> {
        let mut declarations = self
            .manifest
            .nominal_declarations()
            .iter()
            .collect::<Vec<_>>();
        declarations.sort_by(|left, right| left.path().segments().cmp(right.path().segments()));
        declarations
            .into_iter()
            .map(|declaration| {
                let path = nominal_path(declaration.path())?;
                let item = EnvironmentPublicationItemId::AdapterNominal {
                    owner: self.owner.clone(),
                    path: path.clone(),
                };
                Ok(AcceptedNominalInventoryInput::new(
                    AcceptedNominalId::new(
                        AcceptedNominalOwnerId::Environment(
                            self.semantic_environment_owner.clone(),
                        ),
                        path,
                    ),
                    declaration.arity(),
                    match declaration.visibility() {
                        AdapterNominalVisibility::Public => AcceptedNominalInputVisibility::Visible,
                        AdapterNominalVisibility::Private => {
                            AcceptedNominalInputVisibility::Inaccessible
                        }
                    },
                    AcceptedNominalOrigin::Adapter,
                    item_source(self.document, self.source_map, &item)?,
                    item,
                ))
            })
            .collect()
    }

    fn value_bindings(
        &self,
    ) -> Result<Vec<EnvironmentValueBindingInput>, AdapterRegistrationFactsError> {
        let mut symbols = self.manifest.symbols().iter().collect::<Vec<_>>();
        symbols.sort_by(|left, right| left.path().cmp(right.path()));
        symbols
            .into_iter()
            .map(|symbol| {
                let item = EnvironmentPublicationItemId::AdapterSymbol {
                    owner: self.owner.clone(),
                    path: project_symbol_path(symbol.path())?,
                };
                Ok(EnvironmentValueBindingInput::new(
                    item.clone(),
                    EnvironmentBindingId::try_new(symbol.path().to_string())?,
                    adapter_type_node(
                        symbol.ty(),
                        &self.environment_owner,
                        &TypeSource::new(
                            self.document,
                            self.source_map,
                            &item,
                            EnvironmentTypeSiteRoot::SymbolType,
                        ),
                    )?,
                ))
            })
            .collect()
    }

    fn rust_metadata(
        &self,
        nominal_inventory: &mut Vec<AcceptedNominalInventoryInput>,
    ) -> Result<Vec<RustTypeMetadataPublicationInput>, AdapterRegistrationFactsError> {
        let mut rust_types = self.manifest.rust_types().iter().collect::<Vec<_>>();
        rust_types.sort_by(|left, right| {
            left.package().id.cmp(&right.package().id).then_with(|| {
                left.decl()
                    .path
                    .segments()
                    .cmp(right.decl().path.segments())
            })
        });
        rust_types
            .into_iter()
            .map(|rust_type| self.rust_metadata_record(rust_type, nominal_inventory))
            .collect()
    }

    fn rust_metadata_record(
        &self,
        rust_type: &crate::manifest::AdapterRustType,
        nominal_inventory: &mut Vec<AcceptedNominalInventoryInput>,
    ) -> Result<RustTypeMetadataPublicationInput, AdapterRegistrationFactsError> {
        let package = RustPackageId::try_new(rust_type.package().id.as_str())?;
        let accepted_path = nominal_path(rust_type.accepted_path())?;
        let id = AcceptedNominalId::new(
            AcceptedNominalOwnerId::RustPackage(package.clone()),
            accepted_path.clone(),
        );
        let rust_item = RustItemPath::try_new(rust_type.decl().rust_path.as_str())?;
        let item = EnvironmentPublicationItemId::RustType {
            adapter: self.adapter.clone(),
            package: package.clone(),
            rust_item: rust_item.clone(),
            accepted_path,
        };
        let source = item_source(self.document, self.source_map, &item)?;
        nominal_inventory.push(AcceptedNominalInventoryInput::new(
            id.clone(),
            u16::try_from(rust_type.decl().parameters.len()).map_err(|_| {
                AdapterRegistrationFactsError::RustFieldIndexOverflow {
                    value: rust_type.decl().parameters.len(),
                }
            })?,
            AcceptedNominalInputVisibility::Visible,
            AcceptedNominalOrigin::RustExport,
            source.clone(),
            item.clone(),
        ));
        let parameters = rust_type.decl().parameters.iter().map(|parameter| {
            RustTypeParameterPublicationInput::new(
                parameter.index,
                parameter.name.as_str().to_owned(),
                source.clone(),
            )
        });
        Ok(RustTypeMetadataPublicationInput::new(
            RustTypeMetadataPublicationIdentity::new(
                item.clone(),
                id,
                package,
                rust_package_provenance(rust_type.package())?,
                rust_item,
            ),
            parameters.collect::<Vec<_>>(),
            rust_metadata_kind(
                &rust_type.decl().kind,
                self.manifest.rust_package_mounts(),
                self.document,
                self.source_map,
                &item,
                &source,
            )?,
            source,
        ))
    }

    fn callable_records(
        &self,
    ) -> Result<Vec<EnvironmentCallablePublicationRecordInput>, AdapterRegistrationFactsError> {
        let method_count = self.manifest.methods().len();
        let function_count = self.manifest.functions().len();
        let mut records = Vec::with_capacity(
            method_count
                .saturating_add(function_count)
                .saturating_add(self.manifest.rust_functions().len()),
        );
        self.adapter_methods(&mut records)?;
        self.adapter_functions(&mut records, method_count)?;
        self.rust_functions(&mut records, method_count, function_count)?;
        records.sort_by_key(EnvironmentCallablePublicationRecordInput::declaration_order);
        Ok(records)
    }

    fn adapter_functions(
        &self,
        records: &mut Vec<EnvironmentCallablePublicationRecordInput>,
        method_count: usize,
    ) -> Result<(), AdapterRegistrationFactsError> {
        let mut functions = self.manifest.functions().iter().collect::<Vec<_>>();
        functions.sort_by(|left, right| {
            left.path()
                .segments()
                .cmp(right.path().segments())
                .then_with(|| left.overload().cmp(&right.overload()))
        });
        for (index, function) in functions.into_iter().enumerate() {
            let path = project_callable_path(&self.callable_package, function.path())?;
            let item = EnvironmentPublicationItemId::AdapterFunction {
                owner: self.owner.clone(),
                path: path.clone(),
                overload: overload(function.overload().get())?,
            };
            let subject = AdapterToolingSubject::Free {
                kind: AdapterFreeCallableKind::Function,
                path: function.path().clone(),
                overload: function.overload(),
            };
            records.push(self.callable_record(CallableRecordProjection {
                item,
                kind: EnvironmentCallableKind::Function,
                key: EnvironmentCallableLookupInput::Free(path),
                overload_index: function.overload().get(),
                signature: function.signature(),
                effects: function.effects(),
                declaration_order: method_count.saturating_add(index),
                subject: &subject,
                rust: None,
            })?);
        }
        Ok(())
    }

    fn adapter_methods(
        &self,
        records: &mut Vec<EnvironmentCallablePublicationRecordInput>,
    ) -> Result<(), AdapterRegistrationFactsError> {
        let mut methods = self.manifest.methods().iter().collect::<Vec<_>>();
        methods.sort_by(|left, right| {
            digest::type_digest(left.receiver())
                .cmp(&digest::type_digest(right.receiver()))
                .then_with(|| left.callable_name().cmp(right.callable_name()))
                .then_with(|| left.overload().cmp(&right.overload()))
        });
        for (index, method) in methods.into_iter().enumerate() {
            let name = callable_name(method.callable_name())?;
            let declaration_order = ordinal(index)?;
            let item = EnvironmentPublicationItemId::AdapterMethod {
                owner: self.owner.clone(),
                receiver: digest::type_digest(method.receiver()),
                method: name.clone(),
                overload: overload(method.overload().get())?,
                declaration_order,
            };
            let receiver = adapter_type_node(
                method.receiver(),
                &self.environment_owner,
                &TypeSource::new(
                    self.document,
                    self.source_map,
                    &item,
                    EnvironmentTypeSiteRoot::MethodReceiver,
                ),
            )?;
            let subject = AdapterToolingSubject::Method {
                receiver: method.receiver().clone(),
                name: method.callable_name().clone(),
                overload: method.overload(),
            };
            records.push(self.callable_record(CallableRecordProjection {
                item,
                kind: EnvironmentCallableKind::Method,
                key: EnvironmentCallableLookupInput::Method {
                    receiver,
                    method: name,
                },
                overload_index: method.overload().get(),
                signature: method.signature(),
                effects: method.effects(),
                declaration_order: index,
                subject: &subject,
                rust: None,
            })?);
        }
        Ok(())
    }

    fn rust_functions(
        &self,
        records: &mut Vec<EnvironmentCallablePublicationRecordInput>,
        method_count: usize,
        function_count: usize,
    ) -> Result<(), AdapterRegistrationFactsError> {
        let mut functions = self.manifest.rust_functions().iter().collect::<Vec<_>>();
        functions.sort_by(|left, right| {
            left.package()
                .id
                .cmp(&right.package().id)
                .then_with(|| left.rust_path().cmp(right.rust_path()))
                .then_with(|| left.path().segments().cmp(right.path().segments()))
                .then_with(|| left.overload().cmp(&right.overload()))
        });
        for (index, function) in functions.into_iter().enumerate() {
            let path = project_callable_path(&self.callable_package, function.path())?;
            let package = RustPackageId::try_new(function.package().id.as_str())?;
            let rust_item = RustItemPath::try_new(function.rust_path())?;
            let item = EnvironmentPublicationItemId::RustFunction {
                adapter: self.adapter.clone(),
                package,
                rust_item,
                callable_path: path.clone(),
                overload: overload(function.overload().get())?,
            };
            let subject = AdapterToolingSubject::Free {
                kind: AdapterFreeCallableKind::RustFunction,
                path: function.path().clone(),
                overload: function.overload(),
            };
            records.push(
                self.callable_record(CallableRecordProjection {
                    item,
                    kind: EnvironmentCallableKind::RustFunction,
                    key: EnvironmentCallableLookupInput::Free(path),
                    overload_index: function.overload().get(),
                    signature: function.signature(),
                    effects: function.effects(),
                    declaration_order: method_count
                        .saturating_add(function_count)
                        .saturating_add(index),
                    subject: &subject,
                    rust: Some(rust_provenance(function, &self.adapter)?),
                })?,
            );
        }
        Ok(())
    }

    fn callable_record(
        &self,
        projection: CallableRecordProjection<'_>,
    ) -> Result<EnvironmentCallablePublicationRecordInput, AdapterRegistrationFactsError> {
        Ok(EnvironmentCallablePublicationRecordInput::new(
            projection.item.clone(),
            projection.kind,
            projection.key,
            overload(projection.overload_index)?,
            signature_input(
                projection.signature,
                projection.effects,
                &self.environment_owner,
                self.document,
                self.source_map,
                &projection.item,
            )?,
            ordinal(projection.declaration_order)?,
            EnvironmentCallablePublicationMetadataInput::new(
                documentation(self.manifest, projection.subject, &self.adapter)?,
                None,
                projection.rust,
            ),
        ))
    }
}

fn signature_input(
    signature: &AdapterFunctionSignature,
    effects: &[AdapterEffectCapability],
    environment_owner: &AdapterEnvironmentOwnerId,
    document: &SourceDocument,
    source_map: &source::RegistrationSourceMap,
    item: &EnvironmentPublicationItemId,
) -> Result<EnvironmentCallableSignatureInput, AdapterRegistrationFactsError> {
    let mut has_rest = false;
    let groups = signature
        .groups()
        .iter()
        .map(|group| {
            let index = CallableGroupIndex::try_from_usize(group.index().get())?;
            let parameters = group
                .parameters()
                .iter()
                .map(|parameter| {
                    let parameter_index =
                        CallableParameterIndex::try_from_usize(parameter.index().get())?;
                    let passing = match parameter.passing() {
                        AdapterParameterPassing::PositionalOrNamed => {
                            CallableParameterPassing::PositionalOrNamed
                        }
                        AdapterParameterPassing::PositionalOnly => {
                            CallableParameterPassing::PositionalOnly
                        }
                        AdapterParameterPassing::NamedOnly => CallableParameterPassing::NamedOnly,
                        AdapterParameterPassing::RestPositional => {
                            has_rest = true;
                            CallableParameterPassing::RestPositional
                        }
                        AdapterParameterPassing::RestNamed => {
                            has_rest = true;
                            CallableParameterPassing::RestNamed
                        }
                    };
                    Ok(EnvironmentParameterInput::new(
                        parameter_index,
                        parameter.name().map(callable_name).transpose()?,
                        EnvironmentParameterTypeInput::Exact(adapter_type_node(
                            parameter.ty(),
                            environment_owner,
                            &TypeSource::new(
                                document,
                                source_map,
                                item,
                                EnvironmentTypeSiteRoot::Parameter {
                                    group: index,
                                    parameter: parameter_index,
                                },
                            ),
                        )?),
                        passing,
                        match parameter.presence() {
                            AdapterParameterPresence::Required => {
                                CallableParameterPresence::Required
                            }
                            AdapterParameterPresence::Defaulted => {
                                CallableParameterPresence::Defaulted
                            }
                        },
                        EnvironmentParameterMetadataInput::new(None, None),
                    ))
                })
                .collect::<Result<Vec<_>, AdapterRegistrationFactsError>>()?;
            Ok(EnvironmentParameterGroupInput::new(
                index,
                if group.index().get() == 0 {
                    CallableGroupKind::Initial
                } else {
                    CallableGroupKind::Curried
                },
                parameters,
            ))
        })
        .collect::<Result<Vec<_>, AdapterRegistrationFactsError>>()?;
    let effects = EffectSet::from_labels(effects.iter().map(AdapterEffectCapability::as_str))?;
    Ok(EnvironmentCallableSignatureInput::new(
        groups,
        adapter_type_node(
            signature.return_type(),
            environment_owner,
            &TypeSource::new(document, source_map, item, EnvironmentTypeSiteRoot::Result),
        )?,
        EffectRow::closed(effects),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            if has_rest {
                SpreadArgumentPolicy::TypedRest
            } else {
                SpreadArgumentPolicy::Reject
            },
        ),
        CallableValidator::Ordinary,
    ))
}

fn adapter_type_node(
    ty: &AdapterTypeKind,
    expected_environment_owner: &AdapterEnvironmentOwnerId,
    source: &TypeSource<'_>,
) -> Result<EnvironmentTypeProjectionNode, AdapterRegistrationFactsError> {
    let kind = match ty {
        AdapterTypeKind::Unit => EnvironmentTypeProjectionKind::Unit,
        AdapterTypeKind::Bool => EnvironmentTypeProjectionKind::Bool,
        AdapterTypeKind::I8 => EnvironmentTypeProjectionKind::I8,
        AdapterTypeKind::I16 => EnvironmentTypeProjectionKind::I16,
        AdapterTypeKind::I32 => EnvironmentTypeProjectionKind::I32,
        AdapterTypeKind::I64 => EnvironmentTypeProjectionKind::I64,
        AdapterTypeKind::I128 => EnvironmentTypeProjectionKind::I128,
        AdapterTypeKind::ISize => EnvironmentTypeProjectionKind::ISize,
        AdapterTypeKind::U8 => EnvironmentTypeProjectionKind::U8,
        AdapterTypeKind::U16 => EnvironmentTypeProjectionKind::U16,
        AdapterTypeKind::U32 => EnvironmentTypeProjectionKind::U32,
        AdapterTypeKind::U64 => EnvironmentTypeProjectionKind::U64,
        AdapterTypeKind::U128 => EnvironmentTypeProjectionKind::U128,
        AdapterTypeKind::USize => EnvironmentTypeProjectionKind::USize,
        AdapterTypeKind::F32 => EnvironmentTypeProjectionKind::F32,
        AdapterTypeKind::F64 => EnvironmentTypeProjectionKind::F64,
        AdapterTypeKind::String => EnvironmentTypeProjectionKind::String,
        AdapterTypeKind::Char => EnvironmentTypeProjectionKind::Char,
        AdapterTypeKind::Vec { item } => {
            EnvironmentTypeProjectionKind::Vec(Box::new(adapter_type_node(
                item,
                expected_environment_owner,
                &source.child(EnvironmentTypeSiteStep::VecItem),
            )?))
        }
        AdapterTypeKind::Seq { item } => {
            EnvironmentTypeProjectionKind::Seq(Box::new(adapter_type_node(
                item,
                expected_environment_owner,
                &source.child(EnvironmentTypeSiteStep::SeqItem),
            )?))
        }
        AdapterTypeKind::Option { item } => {
            EnvironmentTypeProjectionKind::Option(Box::new(adapter_type_node(
                item,
                expected_environment_owner,
                &source.child(EnvironmentTypeSiteStep::OptionItem),
            )?))
        }
        AdapterTypeKind::Result { ok, error } => EnvironmentTypeProjectionKind::Result {
            ok: Box::new(adapter_type_node(
                ok,
                expected_environment_owner,
                &source.child(EnvironmentTypeSiteStep::ResultOk),
            )?),
            error: Box::new(adapter_type_node(
                error,
                expected_environment_owner,
                &source.child(EnvironmentTypeSiteStep::ResultError),
            )?),
        },
        AdapterTypeKind::Tuple { items } => {
            adapter_tuple_node(items, expected_environment_owner, source)?
        }
        AdapterTypeKind::Need { ready, error } => EnvironmentTypeProjectionKind::Need {
            ready: Box::new(adapter_type_node(
                ready,
                expected_environment_owner,
                &source.child(EnvironmentTypeSiteStep::NeedReady),
            )?),
            error: Box::new(adapter_type_node(
                error,
                expected_environment_owner,
                &source.child(EnvironmentTypeSiteStep::NeedError),
            )?),
        },
        AdapterTypeKind::Nominal { nominal } => {
            adapter_nominal_node(nominal, expected_environment_owner, source)?
        }
    };
    Ok(EnvironmentTypeProjectionNode::new(source.span()?, kind))
}

fn adapter_tuple_node(
    items: &[AdapterTypeKind],
    expected_environment_owner: &AdapterEnvironmentOwnerId,
    source: &TypeSource<'_>,
) -> Result<EnvironmentTypeProjectionKind, AdapterRegistrationFactsError> {
    let items = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            adapter_type_node(
                item,
                expected_environment_owner,
                &source.child(EnvironmentTypeSiteStep::TupleItem(
                    u16::try_from(index).map_err(|_| {
                        AdapterRegistrationFactsError::TypeSiteIndexOverflow { value: index }
                    })?,
                )),
            )
        })
        .collect::<Result<Box<[_]>, _>>()?;
    Ok(EnvironmentTypeProjectionKind::Tuple(items))
}

fn adapter_nominal_node(
    nominal: &AdapterNominalTypeRef,
    expected_environment_owner: &AdapterEnvironmentOwnerId,
    source: &TypeSource<'_>,
) -> Result<EnvironmentTypeProjectionKind, AdapterRegistrationFactsError> {
    let owner = accepted_owner(nominal.owner(), expected_environment_owner)?;
    let arguments = nominal
        .arguments()
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            adapter_type_node(
                argument,
                expected_environment_owner,
                &source.child(EnvironmentTypeSiteStep::NominalArgument(
                    u16::try_from(index).map_err(|_| {
                        AdapterRegistrationFactsError::TypeSiteIndexOverflow { value: index }
                    })?,
                )),
            )
        })
        .collect::<Result<Box<[_]>, _>>()?;
    Ok(EnvironmentTypeProjectionKind::AcceptedNominal {
        id: AcceptedNominalId::new(owner, nominal_path(nominal.path())?),
        arguments,
    })
}

fn rust_type_node(
    ty: &ArcweftRustTypeRef,
    mounts: &AdapterRustPackageMountTable,
    source: &TypeSource<'_>,
) -> Result<EnvironmentTypeProjectionNode, AdapterRegistrationFactsError> {
    let kind = match ty {
        ArcweftRustTypeRef::Unit => EnvironmentTypeProjectionKind::Unit,
        ArcweftRustTypeRef::Bool => EnvironmentTypeProjectionKind::Bool,
        ArcweftRustTypeRef::I8 => EnvironmentTypeProjectionKind::I8,
        ArcweftRustTypeRef::I16 => EnvironmentTypeProjectionKind::I16,
        ArcweftRustTypeRef::I32 => EnvironmentTypeProjectionKind::I32,
        ArcweftRustTypeRef::I64 => EnvironmentTypeProjectionKind::I64,
        ArcweftRustTypeRef::I128 => EnvironmentTypeProjectionKind::I128,
        ArcweftRustTypeRef::ISize => EnvironmentTypeProjectionKind::ISize,
        ArcweftRustTypeRef::U8 => EnvironmentTypeProjectionKind::U8,
        ArcweftRustTypeRef::U16 => EnvironmentTypeProjectionKind::U16,
        ArcweftRustTypeRef::U32 => EnvironmentTypeProjectionKind::U32,
        ArcweftRustTypeRef::U64 => EnvironmentTypeProjectionKind::U64,
        ArcweftRustTypeRef::U128 => EnvironmentTypeProjectionKind::U128,
        ArcweftRustTypeRef::USize => EnvironmentTypeProjectionKind::USize,
        ArcweftRustTypeRef::F32 => EnvironmentTypeProjectionKind::F32,
        ArcweftRustTypeRef::F64 => EnvironmentTypeProjectionKind::F64,
        ArcweftRustTypeRef::String => EnvironmentTypeProjectionKind::String,
        ArcweftRustTypeRef::Char => EnvironmentTypeProjectionKind::Char,
        ArcweftRustTypeRef::Vec { item } => {
            EnvironmentTypeProjectionKind::Vec(Box::new(rust_type_node(
                item,
                mounts,
                &source.child(EnvironmentTypeSiteStep::VecItem),
            )?))
        }
        ArcweftRustTypeRef::Seq { item } => {
            EnvironmentTypeProjectionKind::Seq(Box::new(rust_type_node(
                item,
                mounts,
                &source.child(EnvironmentTypeSiteStep::SeqItem),
            )?))
        }
        ArcweftRustTypeRef::Option { item } => {
            EnvironmentTypeProjectionKind::Option(Box::new(rust_type_node(
                item,
                mounts,
                &source.child(EnvironmentTypeSiteStep::OptionItem),
            )?))
        }
        ArcweftRustTypeRef::Result { ok, error } => EnvironmentTypeProjectionKind::Result {
            ok: Box::new(rust_type_node(
                ok,
                mounts,
                &source.child(EnvironmentTypeSiteStep::ResultOk),
            )?),
            error: Box::new(rust_type_node(
                error,
                mounts,
                &source.child(EnvironmentTypeSiteStep::ResultError),
            )?),
        },
        ArcweftRustTypeRef::Tuple { items } => rust_tuple_node(items, mounts, source)?,
        ArcweftRustTypeRef::Nominal {
            package,
            path,
            arguments,
        } => rust_nominal_node(package, path, arguments, mounts, source)?,
        ArcweftRustTypeRef::TypeParameter { index } => {
            EnvironmentTypeProjectionKind::TypeParameter { index: *index }
        }
    };
    Ok(EnvironmentTypeProjectionNode::new(source.span()?, kind))
}

fn rust_tuple_node(
    items: &[ArcweftRustTypeRef],
    mounts: &AdapterRustPackageMountTable,
    source: &TypeSource<'_>,
) -> Result<EnvironmentTypeProjectionKind, AdapterRegistrationFactsError> {
    let items = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            rust_type_node(
                item,
                mounts,
                &source.child(EnvironmentTypeSiteStep::TupleItem(
                    u16::try_from(index).map_err(|_| {
                        AdapterRegistrationFactsError::TypeSiteIndexOverflow { value: index }
                    })?,
                )),
            )
        })
        .collect::<Result<Box<[_]>, _>>()?;
    Ok(EnvironmentTypeProjectionKind::Tuple(items))
}

fn rust_nominal_node(
    package: &ArcweftRustPackageId,
    path: &ArcweftRustTypePath,
    arguments: &[ArcweftRustTypeRef],
    mounts: &AdapterRustPackageMountTable,
    source: &TypeSource<'_>,
) -> Result<EnvironmentTypeProjectionKind, AdapterRegistrationFactsError> {
    let prefix = mounts.get(package).ok_or_else(|| {
        crate::manifest::AdapterManifestModelError::MissingRustPackageMount {
            package: package.clone(),
        }
    })?;
    let arguments = arguments
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            rust_type_node(
                argument,
                mounts,
                &source.child(EnvironmentTypeSiteStep::NominalArgument(
                    u16::try_from(index).map_err(|_| {
                        AdapterRegistrationFactsError::TypeSiteIndexOverflow { value: index }
                    })?,
                )),
            )
        })
        .collect::<Result<Box<[_]>, _>>()?;
    Ok(EnvironmentTypeProjectionKind::AcceptedNominal {
        id: AcceptedNominalId::new(
            AcceptedNominalOwnerId::RustPackage(RustPackageId::try_new(package.as_str())?),
            nominal_path(&prefix.join(path)?)?,
        ),
        arguments,
    })
}

fn rust_metadata_kind(
    kind: &ArcweftRustTypeKind,
    mounts: &AdapterRustPackageMountTable,
    document: &SourceDocument,
    source_map: &source::RegistrationSourceMap,
    item: &EnvironmentPublicationItemId,
    item_source: &SourceSpan,
) -> Result<RustTypeMetadataPublicationKind, AdapterRegistrationFactsError> {
    Ok(match kind {
        ArcweftRustTypeKind::Struct { shape } => RustTypeMetadataPublicationKind::Struct {
            shape: match shape {
                ArcweftRustStructShape::Unit => RustStructMetadataInput::Unit,
                ArcweftRustStructShape::Tuple { fields } => RustStructMetadataInput::Tuple(
                    fields
                        .iter()
                        .enumerate()
                        .map(|(index, field)| {
                            rust_type_node(
                                field,
                                mounts,
                                &TypeSource::new(
                                    document,
                                    source_map,
                                    item,
                                    EnvironmentTypeSiteRoot::RustStructTupleField {
                                        field: u16::try_from(index).map_err(|_| {
                                            AdapterRegistrationFactsError::RustFieldIndexOverflow {
                                                value: index,
                                            }
                                        })?,
                                    },
                                ),
                            )
                        })
                        .collect::<Result<Box<[_]>, _>>()?,
                ),
                ArcweftRustStructShape::Record { fields } => RustStructMetadataInput::Record(
                    rust_record_fields(fields, mounts, document, source_map, item, None)?,
                ),
            },
        },
        ArcweftRustTypeKind::Enum { variants } => RustTypeMetadataPublicationKind::Enum {
            variants: variants
                .iter()
                .map(|variant| {
                    rust_variant(variant, mounts, document, source_map, item, item_source)
                })
                .collect::<Result<Box<[_]>, _>>()?,
        },
        ArcweftRustTypeKind::Newtype { inner } => RustTypeMetadataPublicationKind::Newtype {
            inner: rust_type_node(
                inner,
                mounts,
                &TypeSource::new(
                    document,
                    source_map,
                    item,
                    EnvironmentTypeSiteRoot::RustNewtypeInner,
                ),
            )?,
        },
    })
}

fn rust_record_fields(
    fields: &[ArcweftRustField],
    mounts: &AdapterRustPackageMountTable,
    document: &SourceDocument,
    source_map: &source::RegistrationSourceMap,
    item: &EnvironmentPublicationItemId,
    variant: Option<&str>,
) -> Result<Box<[(String, EnvironmentTypeProjectionNode)]>, AdapterRegistrationFactsError> {
    fields
        .iter()
        .map(|field| {
            let root = variant.map_or_else(
                || EnvironmentTypeSiteRoot::RustStructRecordField {
                    field: field.name.clone(),
                },
                |variant| EnvironmentTypeSiteRoot::RustEnumRecordField {
                    variant: variant.to_owned(),
                    field: field.name.clone(),
                },
            );
            Ok((
                field.name.clone(),
                rust_type_node(
                    &field.ty,
                    mounts,
                    &TypeSource::new(document, source_map, item, root),
                )?,
            ))
        })
        .collect()
}

fn rust_variant(
    variant: &ArcweftRustVariant,
    mounts: &AdapterRustPackageMountTable,
    document: &SourceDocument,
    source_map: &source::RegistrationSourceMap,
    item: &EnvironmentPublicationItemId,
    item_source: &SourceSpan,
) -> Result<RustVariantMetadataInput, AdapterRegistrationFactsError> {
    let payload = match &variant.payload {
        ArcweftRustVariantPayload::Unit => RustVariantPayloadInput::Unit,
        ArcweftRustVariantPayload::Tuple { fields } => RustVariantPayloadInput::Tuple(
            fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    rust_type_node(
                        field,
                        mounts,
                        &TypeSource::new(
                            document,
                            source_map,
                            item,
                            EnvironmentTypeSiteRoot::RustEnumTupleField {
                                variant: variant.name.clone(),
                                field: u16::try_from(index).map_err(|_| {
                                    AdapterRegistrationFactsError::RustFieldIndexOverflow {
                                        value: index,
                                    }
                                })?,
                            },
                        ),
                    )
                })
                .collect::<Result<Box<[_]>, _>>()?,
        ),
        ArcweftRustVariantPayload::Record { fields } => {
            RustVariantPayloadInput::Record(rust_record_fields(
                fields,
                mounts,
                document,
                source_map,
                item,
                Some(&variant.name),
            )?)
        }
    };
    Ok(RustVariantMetadataInput::new(
        variant.name.clone(),
        payload,
        item_source.clone(),
    ))
}

fn accepted_owner(
    owner: &AdapterNominalOwner,
    expected_environment_owner: &AdapterEnvironmentOwnerId,
) -> Result<AcceptedNominalOwnerId, AdapterRegistrationFactsError> {
    match owner {
        AdapterNominalOwner::Environment { owner } => {
            if owner != expected_environment_owner {
                return Err(AdapterRegistrationFactsError::EnvironmentOwnerMismatch {
                    expected: expected_environment_owner.clone(),
                    actual: owner.clone(),
                });
            }
            Ok(AcceptedNominalOwnerId::Environment(
                EnvironmentBindingId::try_new(owner.as_str())?,
            ))
        }
        AdapterNominalOwner::RustPackage { package } => Ok(AcceptedNominalOwnerId::RustPackage(
            RustPackageId::try_new(package.as_str())?,
        )),
    }
}

fn nominal_path(path: &AdapterNominalPath) -> Result<TypePath, AdapterRegistrationFactsError> {
    Ok(ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        path.segments()
            .iter()
            .map(|segment| ProjectSymbolSegment::try_new(segment.as_str()))
            .collect::<Result<Vec<_>, _>>()?,
    )?
    .into())
}

fn project_symbol_path(
    path: &crate::manifest::AdapterSymbolPath,
) -> Result<ProjectSymbolPath, AdapterRegistrationFactsError> {
    Ok(ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        path.segments()
            .iter()
            .map(|segment| ProjectSymbolSegment::try_new(segment.as_str()))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}

fn project_callable_path(
    package: &CallablePackageId,
    path: &AdapterCallablePath,
) -> Result<ProjectCallablePath, AdapterRegistrationFactsError> {
    Ok(ProjectCallablePath::new(
        package.clone(),
        CanonicalModulePath::crate_root(),
        callable_path(path)?,
    ))
}

fn callable_path(
    path: &AdapterCallablePath,
) -> Result<CallablePath, AdapterRegistrationFactsError> {
    Ok(CallablePath::try_new(
        path.segments()
            .iter()
            .map(callable_name)
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}

fn callable_name(
    name: &AdapterCallableName,
) -> Result<CallableName, AdapterRegistrationFactsError> {
    Ok(CallableName::try_new(name.as_str())?)
}

fn overload(value: usize) -> Result<CallableOverloadIndex, AdapterRegistrationFactsError> {
    Ok(CallableOverloadIndex::try_from_usize(value)?)
}

fn ordinal(value: usize) -> Result<EnvironmentDeclarationOrdinal, AdapterRegistrationFactsError> {
    Ok(EnvironmentDeclarationOrdinal::try_from_usize(value)?)
}

fn rust_provenance(
    function: &crate::manifest::AdapterRustFunction,
    adapter: &AdapterPackageId,
) -> Result<RustCallableProvenance, AdapterRegistrationFactsError> {
    let package = rust_package_provenance(function.package())?;
    let purity = match function.purity() {
        arcweft_rust_abi::ArcweftRustPurity::External => RustCallablePurity::External,
        arcweft_rust_abi::ArcweftRustPurity::Pure => RustCallablePurity::Pure,
        arcweft_rust_abi::ArcweftRustPurity::Task => RustCallablePurity::Task,
    };
    Ok(RustCallableProvenance::try_new(
        adapter.clone(),
        package,
        RustItemPath::try_new(function.rust_path())?,
        purity,
    )?)
}

fn rust_package_provenance(
    package: &ArcweftRustPackage,
) -> Result<RustPackageProvenance, AdapterRegistrationFactsError> {
    Ok(RustPackageProvenance::try_new(
        package.id.as_str(),
        package.version.as_str(),
        package.metadata_hash.as_deref().map(Arc::<str>::from),
    )?)
}

fn documentation(
    manifest: &AdapterManifest,
    subject: &AdapterToolingSubject,
    package: &AdapterPackageId,
) -> Result<CallableDocumentation, AdapterRegistrationFactsError> {
    let Some(doc) = manifest
        .tooling_docs()
        .iter()
        .find(|doc| doc.subject() == subject)
    else {
        return Ok(CallableDocumentation::missing());
    };
    let parameters = doc
        .parameters()
        .iter()
        .map(|parameter| {
            Ok(CallableParameterDocumentation::try_new(
                CallableGroupIndex::try_from_usize(parameter.group().get())?,
                CallableParameterIndex::try_from_usize(parameter.parameter().get())?,
                Arc::<str>::from(parameter.text()),
            )?)
        })
        .collect::<Result<Vec<_>, AdapterRegistrationFactsError>>()?;
    Ok(CallableDocumentation::try_new(
        doc.summary().map(Arc::<str>::from),
        doc.details().map(Arc::<str>::from),
        parameters,
        DocumentationProvenance::AdapterTooling {
            package: package.clone(),
        },
    )?)
}

fn validate_tooling(manifest: &AdapterManifest) -> Result<(), AdapterRegistrationFactsError> {
    let mut seen = Vec::<&AdapterToolingSubject>::new();
    for doc in manifest.tooling_docs() {
        if seen.iter().any(|subject| *subject == doc.subject()) {
            return Err(
                crate::manifest::AdapterCallableModelError::DuplicateToolingSubject {
                    subject: doc.subject().clone(),
                }
                .into(),
            );
        }
        seen.push(doc.subject());
        let Some(signature) = subject_signature(manifest, doc.subject()) else {
            return Err(
                crate::manifest::AdapterCallableModelError::ToolingParameterOutOfBounds {
                    subject: doc.subject().clone(),
                    group: 0,
                    parameter: 0,
                }
                .into(),
            );
        };
        for parameter in doc.parameters() {
            if signature
                .groups()
                .get(parameter.group().get())
                .and_then(|group| group.parameters().get(parameter.parameter().get()))
                .is_none()
            {
                return Err(
                    crate::manifest::AdapterCallableModelError::ToolingParameterOutOfBounds {
                        subject: doc.subject().clone(),
                        group: parameter.group().get(),
                        parameter: parameter.parameter().get(),
                    }
                    .into(),
                );
            }
        }
    }
    Ok(())
}

fn subject_signature<'a>(
    manifest: &'a AdapterManifest,
    subject: &AdapterToolingSubject,
) -> Option<&'a AdapterFunctionSignature> {
    match subject {
        AdapterToolingSubject::Free {
            kind: AdapterFreeCallableKind::Function,
            path,
            overload,
        } => manifest
            .functions()
            .iter()
            .find(|function| function.path() == path && function.overload() == *overload)
            .map(crate::manifest::AdapterFunction::signature),
        AdapterToolingSubject::Free {
            kind: AdapterFreeCallableKind::RustFunction,
            path,
            overload,
        } => manifest
            .rust_functions()
            .iter()
            .find(|function| function.path() == path && function.overload() == *overload)
            .map(crate::manifest::AdapterRustFunction::signature),
        AdapterToolingSubject::Method {
            receiver,
            name,
            overload,
        } => manifest
            .methods()
            .iter()
            .find(|method| {
                method.receiver() == receiver
                    && method.callable_name() == name
                    && method.overload() == *overload
            })
            .map(crate::manifest::AdapterMethod::signature),
    }
}
