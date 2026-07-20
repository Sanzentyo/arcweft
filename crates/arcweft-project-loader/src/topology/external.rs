//! Admission of verified neutral module metadata into the selected project facts.

use super::{
    ExternalModuleFactsError, LoadedExternalModuleMetadata, ProfileTopologyLoadError,
    TypeReferenceLimitKind, TypeReferenceLimits,
};
use arcweft_adapter_context::manifest::{
    AdapterCallableGroupIndex, AdapterCallableModelError, AdapterCallableName,
    AdapterCallableOverloadIndex, AdapterCallableParameterIndex, AdapterCallablePath,
    AdapterEffectCapability, AdapterFunctionParam, AdapterFunctionSignature, AdapterManifest,
    AdapterParameterGroup, AdapterParameterPassing, AdapterParameterPresence, AdapterSymbol,
    AdapterSymbolPath, AdapterSymbolPathError, AdapterSymbolSegment, AdapterTypeKind,
};
use arcweft_adapter_metadata::{AdapterFunctionExport, FunctionPurity};
use arcweft_launch::resolve::ResolvedLaunchProfile;
use arcweft_manifest_model::{
    ExternalModuleImportId, ManifestVisibility, ModuleMountPath, TypeReference,
};
use std::collections::BTreeSet;

/// Extends the selected host adapter with only the admitted, mounted module surface.
pub(super) fn extend_selected_adapter(
    adapter: AdapterManifest,
    modules: &[LoadedExternalModuleMetadata],
) -> Result<AdapterManifest, ProfileTopologyLoadError> {
    ExternalModuleFactBuilder::new(adapter)
        .extend(modules)
        .map_err(Into::into)
}

/// Confirms every selected abstract Activity binding against verified metadata.
pub(super) fn validate_activity_bindings(
    profile: &ResolvedLaunchProfile,
    modules: &[LoadedExternalModuleMetadata],
) -> Result<(), ProfileTopologyLoadError> {
    for (activity, binding) in profile.activity_bindings() {
        let import_id = &binding.implementation().module;
        let Some(module) = modules
            .iter()
            .find(|module| module.import_id() == import_id)
        else {
            return Err(ExternalModuleFactsError::ActivityImportMissing {
                activity: activity.clone(),
                import: import_id.clone(),
            }
            .into());
        };
        let export_id = &binding.implementation().export;
        let Some(export) = module
            .metadata()
            .metadata()
            .exports
            .activities
            .iter()
            .find(|export| &export.export == export_id)
        else {
            return Err(ExternalModuleFactsError::ActivityExportMissing {
                activity: activity.clone(),
                import: import_id.clone(),
                export: export_id.clone(),
            }
            .into());
        };
        if &export.activity_id != activity {
            return Err(ExternalModuleFactsError::ActivityIdentityMismatch {
                import: import_id.clone(),
                export: export_id.clone(),
                expected: activity.clone(),
                actual: export.activity_id.clone(),
            }
            .into());
        }
    }
    Ok(())
}

struct ExternalModuleFactBuilder {
    adapter: Option<AdapterManifest>,
    mounted_identities: BTreeSet<String>,
}

impl ExternalModuleFactBuilder {
    fn new(adapter: AdapterManifest) -> Self {
        let mut mounted_identities = adapter
            .symbols()
            .iter()
            .map(|symbol| symbol.path().to_string())
            .collect::<BTreeSet<_>>();
        mounted_identities.extend(
            adapter
                .functions()
                .iter()
                .map(|function| callable_path_text(function.path())),
        );
        mounted_identities.extend(
            adapter
                .rust_functions()
                .iter()
                .map(|function| callable_path_text(function.path())),
        );
        Self {
            adapter: Some(adapter),
            mounted_identities,
        }
    }

    fn extend(
        mut self,
        modules: &[LoadedExternalModuleMetadata],
    ) -> Result<AdapterManifest, ExternalModuleFactsError> {
        for module in modules {
            self.add_module(module)?;
        }
        self.adapter
            .ok_or(ExternalModuleFactsError::ProjectionState {
                operation: "finishing the selected adapter",
            })
    }

    fn add_module(
        &mut self,
        module: &LoadedExternalModuleMetadata,
    ) -> Result<(), ExternalModuleFactsError> {
        let exports = &module.metadata().metadata().exports;
        let visible_types = exports
            .types
            .iter()
            .filter(|export| export.visibility != ManifestVisibility::Private)
            .map(|export| export.name.as_str())
            .collect::<BTreeSet<_>>();

        for export in exports
            .types
            .iter()
            .filter(|export| export.visibility != ManifestVisibility::Private)
        {
            let identity = mounted_identity(&module.import().mount, export.name.as_str());
            self.claim_identity(identity.clone())?;
            let path = mounted_symbol_path(&module.import().mount, export.name.as_str()).map_err(
                |source| ExternalModuleFactsError::Symbol {
                    import: module.import_id().clone(),
                    export: export.name.to_string(),
                    source,
                },
            )?;
            let adapter = self
                .adapter
                .take()
                .ok_or(ExternalModuleFactsError::ProjectionState {
                    operation: "inserting a type fact",
                })?;
            self.adapter = Some(
                adapter.with_symbol(AdapterSymbol::new(path, AdapterTypeKind::Named(identity))),
            );
        }

        for export in exports
            .functions
            .iter()
            .filter(|export| export.visibility != ManifestVisibility::Private)
        {
            self.add_function(module, export, &visible_types)?;
        }
        Ok(())
    }

    fn add_function(
        &mut self,
        module: &LoadedExternalModuleMetadata,
        export: &AdapterFunctionExport,
        local_types: &BTreeSet<&str>,
    ) -> Result<(), ExternalModuleFactsError> {
        validate_function_purity(module, export)?;
        let identity = mounted_identity(&module.import().mount, export.name.as_str());
        self.claim_identity(identity)?;
        let path = mounted_callable_path(&module.import().mount, export.name.as_str()).map_err(
            |source| {
                ExternalModuleFactsError::callable(module.import_id(), export.name.as_str(), source)
            },
        )?;
        let signature = mounted_function_signature(module, export, local_types)?;
        let overload = AdapterCallableOverloadIndex::try_from_usize(0).map_err(|source| {
            ExternalModuleFactsError::callable(module.import_id(), export.name.as_str(), source)
        })?;
        let effects = export
            .effects
            .iter()
            .map(|effect| AdapterEffectCapability::new(effect.to_string()))
            .collect::<Vec<_>>();
        let adapter = self
            .adapter
            .take()
            .ok_or(ExternalModuleFactsError::ProjectionState {
                operation: "inserting a callable fact",
            })?;
        self.adapter = Some(adapter.with_function_signature(path, overload, signature, effects));
        Ok(())
    }

    fn claim_identity(&mut self, identity: String) -> Result<(), ExternalModuleFactsError> {
        if self.mounted_identities.insert(identity.clone()) {
            Ok(())
        } else {
            Err(ExternalModuleFactsError::DuplicateMountedIdentity { identity })
        }
    }
}

fn validate_function_purity(
    module: &LoadedExternalModuleMetadata,
    export: &AdapterFunctionExport,
) -> Result<(), ExternalModuleFactsError> {
    let purity = match (export.purity, export.effects.is_empty()) {
        (FunctionPurity::Pure, false) => Some("pure"),
        (FunctionPurity::Effectful, true) => Some("effectful"),
        (FunctionPurity::Pure, true) | (FunctionPurity::Effectful, false) => None,
    };
    if let Some(purity) = purity {
        return Err(ExternalModuleFactsError::FunctionPurity {
            import: module.import_id().clone(),
            function: export.name.to_string(),
            purity,
        });
    }
    Ok(())
}

fn mounted_function_signature(
    module: &LoadedExternalModuleMetadata,
    export: &AdapterFunctionExport,
    local_types: &BTreeSet<&str>,
) -> Result<AdapterFunctionSignature, ExternalModuleFactsError> {
    let parameters = mounted_function_parameters(module, export, local_types)?;
    let group_index = AdapterCallableGroupIndex::try_from_usize(0).map_err(|source| {
        ExternalModuleFactsError::callable(module.import_id(), export.name.as_str(), source)
    })?;
    let group = AdapterParameterGroup::try_new(group_index, parameters).map_err(|source| {
        ExternalModuleFactsError::callable(module.import_id(), export.name.as_str(), source)
    })?;
    let return_type = mounted_type(
        &export.return_type,
        &module.import().mount,
        local_types,
        module.import_id(),
        export.name.as_str(),
    )?;
    AdapterFunctionSignature::try_new(vec![group], return_type).map_err(|source| {
        ExternalModuleFactsError::callable(module.import_id(), export.name.as_str(), source)
    })
}

fn mounted_function_parameters(
    module: &LoadedExternalModuleMetadata,
    export: &AdapterFunctionExport,
    local_types: &BTreeSet<&str>,
) -> Result<Vec<AdapterFunctionParam>, ExternalModuleFactsError> {
    export
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let ty = mounted_type(
                &parameter.ty,
                &module.import().mount,
                local_types,
                module.import_id(),
                export.name.as_str(),
            )?;
            let index = AdapterCallableParameterIndex::try_from_usize(index).map_err(|source| {
                ExternalModuleFactsError::callable(module.import_id(), export.name.as_str(), source)
            })?;
            let name =
                AdapterCallableName::try_new(parameter.name.to_string()).map_err(|source| {
                    ExternalModuleFactsError::callable(
                        module.import_id(),
                        export.name.as_str(),
                        source,
                    )
                })?;
            AdapterFunctionParam::try_new(
                index,
                Some(name),
                ty,
                AdapterParameterPassing::PositionalOrNamed,
                AdapterParameterPresence::Required,
            )
            .map_err(|source| {
                ExternalModuleFactsError::callable(module.import_id(), export.name.as_str(), source)
            })
        })
        .collect()
}

fn mounted_symbol_path(
    mount: &ModuleMountPath,
    leaf: &str,
) -> Result<AdapterSymbolPath, AdapterSymbolPathError> {
    AdapterSymbolPath::try_new(
        mount
            .as_str()
            .split('.')
            .chain([leaf])
            .map(|segment| AdapterSymbolSegment::try_new(segment.to_owned()))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn mounted_callable_path(
    mount: &ModuleMountPath,
    leaf: &str,
) -> Result<AdapterCallablePath, AdapterCallableModelError> {
    AdapterCallablePath::try_new(
        mount
            .as_str()
            .split('.')
            .chain([leaf])
            .map(|segment| AdapterCallableName::try_new(segment.to_owned()))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn mounted_identity(mount: &ModuleMountPath, leaf: &str) -> String {
    format!("{}.{leaf}", mount.as_str())
}

fn callable_path_text(path: &AdapterCallablePath) -> String {
    path.segments()
        .iter()
        .map(AdapterCallableName::as_str)
        .collect::<Vec<_>>()
        .join(".")
}

fn mounted_type(
    reference: &TypeReference,
    mount: &ModuleMountPath,
    local_types: &BTreeSet<&str>,
    import: &ExternalModuleImportId,
    export: &str,
) -> Result<AdapterTypeKind, ExternalModuleFactsError> {
    TypeReferenceParser::new(reference.as_str(), mount, local_types)
        .parse()
        .map_err(|source| match source {
            TypeReferenceParseError::Invalid => ExternalModuleFactsError::TypeReference {
                import: import.clone(),
                export: export.to_owned(),
                reference: reference.to_string(),
            },
            TypeReferenceParseError::Limit {
                kind,
                observed,
                maximum,
            } => ExternalModuleFactsError::TypeReferenceLimit {
                import: import.clone(),
                export: export.to_owned(),
                kind,
                observed,
                maximum,
            },
        })
}

struct TypeReferenceParser<'a> {
    source: &'a str,
    offset: usize,
    mount: &'a ModuleMountPath,
    local_types: &'a BTreeSet<&'a str>,
    limits: TypeReferenceLimits,
    work: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TypeReferenceParseError {
    Invalid,
    Limit {
        kind: TypeReferenceLimitKind,
        observed: usize,
        maximum: usize,
    },
}

impl<'a> TypeReferenceParser<'a> {
    fn new(
        source: &'a str,
        mount: &'a ModuleMountPath,
        local_types: &'a BTreeSet<&'a str>,
    ) -> Self {
        Self::with_limits(source, mount, local_types, TypeReferenceLimits::PRODUCTION)
    }

    fn with_limits(
        source: &'a str,
        mount: &'a ModuleMountPath,
        local_types: &'a BTreeSet<&'a str>,
        limits: TypeReferenceLimits,
    ) -> Self {
        Self {
            source,
            offset: 0,
            mount,
            local_types,
            limits,
            work: 0,
        }
    }

    fn parse(mut self) -> Result<AdapterTypeKind, TypeReferenceParseError> {
        if self.source.len() > self.limits.bytes() {
            return Err(TypeReferenceParseError::Limit {
                kind: TypeReferenceLimitKind::Bytes,
                observed: self.source.len(),
                maximum: self.limits.bytes(),
            });
        }
        let ty = self.parse_type(1)?;
        self.skip_space();
        if self.offset == self.source.len() {
            Ok(ty)
        } else {
            Err(TypeReferenceParseError::Invalid)
        }
    }

    fn parse_type(&mut self, depth: usize) -> Result<AdapterTypeKind, TypeReferenceParseError> {
        if depth > self.limits.nesting_depth() {
            return Err(TypeReferenceParseError::Limit {
                kind: TypeReferenceLimitKind::NestingDepth,
                observed: depth,
                maximum: self.limits.nesting_depth(),
            });
        }
        self.work = self.work.saturating_add(1);
        if self.work > self.limits.work() {
            return Err(TypeReferenceParseError::Limit {
                kind: TypeReferenceLimitKind::Work,
                observed: self.work,
                maximum: self.limits.work(),
            });
        }

        self.skip_space();
        if self.consume("()") {
            return Ok(AdapterTypeKind::Unit);
        }
        if self.consume("(") {
            let mut elements = vec![self.parse_type(depth + 1)?];
            while self.consume_comma() {
                elements.push(self.parse_type(depth + 1)?);
            }
            self.skip_space();
            if !self.consume(")") {
                return Err(TypeReferenceParseError::Invalid);
            }
            return Ok(AdapterTypeKind::Tuple(elements));
        }

        let name = self.parse_name().ok_or(TypeReferenceParseError::Invalid)?;
        self.skip_space();
        if !self.consume("<") {
            if let Some(primitive) = AdapterTypeKind::primitive_name(name) {
                return Ok(primitive);
            }
            return self
                .local_types
                .contains(name)
                .then(|| AdapterTypeKind::Named(mounted_identity(self.mount, name)))
                .ok_or(TypeReferenceParseError::Invalid);
        }

        let mut arguments = vec![self.parse_type(depth + 1)?];
        while self.consume_comma() {
            arguments.push(self.parse_type(depth + 1)?);
        }
        self.skip_space();
        if !self.consume(">") {
            return Err(TypeReferenceParseError::Invalid);
        }
        match (name, arguments.as_slice()) {
            ("Vec", [element]) => Ok(AdapterTypeKind::Vec(Box::new(element.clone()))),
            ("Seq", [element]) => Ok(AdapterTypeKind::Seq(Box::new(element.clone()))),
            ("Option", [element]) => Ok(AdapterTypeKind::Option(Box::new(element.clone()))),
            ("Result", [ok, error]) => Ok(AdapterTypeKind::Result {
                ok: Box::new(ok.clone()),
                error: Box::new(error.clone()),
            }),
            ("Need", [ready, error]) => Ok(AdapterTypeKind::Need {
                ready: Box::new(ready.clone()),
                error: Box::new(error.clone()),
            }),
            _ => Err(TypeReferenceParseError::Invalid),
        }
    }

    fn parse_name(&mut self) -> Option<&'a str> {
        let start = self.offset;
        while let Some(byte) = self.source.as_bytes().get(self.offset)
            && (byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            self.offset += 1;
        }
        (self.offset > start).then(|| &self.source[start..self.offset])
    }

    fn consume_comma(&mut self) -> bool {
        self.skip_space();
        self.consume(",")
    }

    fn consume(&mut self, token: &str) -> bool {
        if self.source[self.offset..].starts_with(token) {
            self.offset += token.len();
            true
        } else {
            false
        }
    }

    fn skip_space(&mut self) {
        while self
            .source
            .as_bytes()
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExternalModuleFactsError, TypeReferenceParseError, TypeReferenceParser, mounted_type,
    };
    use arcweft_adapter_context::manifest::AdapterTypeKind;
    use arcweft_manifest_model::{ExternalModuleImportId, ModuleMountPath, TypeReference};
    use std::collections::BTreeSet;

    use crate::topology::{TypeReferenceLimitKind, TypeReferenceLimits};

    #[test]
    fn type_reference_parser_mounts_nominal_types_inside_structural_types() {
        let mount = ModuleMountPath::new("mini_games.truck").expect("mount");
        let local_types = BTreeSet::from(["TruckError", "TruckResult"]);

        assert_eq!(
            TypeReferenceParser::new("Need<Vec<TruckResult>, TruckError>", &mount, &local_types,)
                .parse(),
            Ok(AdapterTypeKind::Need {
                ready: Box::new(AdapterTypeKind::Vec(Box::new(AdapterTypeKind::Named(
                    "mini_games.truck.TruckResult".to_owned(),
                )))),
                error: Box::new(AdapterTypeKind::Named(
                    "mini_games.truck.TruckError".to_owned(),
                )),
            })
        );
    }

    #[test]
    fn type_reference_parser_rejects_unknown_or_trailing_input() {
        let mount = ModuleMountPath::new("truck").expect("mount");
        let local_types = BTreeSet::from(["TruckResult"]);

        assert!(
            TypeReferenceParser::new("Unknown", &mount, &local_types)
                .parse()
                .is_err()
        );
        assert!(
            TypeReferenceParser::new("Vec<TruckResult>>", &mount, &local_types)
                .parse()
                .is_err()
        );
    }

    #[test]
    fn type_reference_parser_enforces_byte_depth_and_work_limits() {
        let mount = ModuleMountPath::new("truck").expect("mount");
        let local_types = BTreeSet::from(["TruckResult"]);

        assert_eq!(
            TypeReferenceParser::with_limits(
                "TruckResult",
                &mount,
                &local_types,
                TypeReferenceLimits::new(5, usize::MAX, usize::MAX),
            )
            .parse(),
            Err(TypeReferenceParseError::Limit {
                kind: TypeReferenceLimitKind::Bytes,
                observed: 11,
                maximum: 5,
            })
        );

        assert_eq!(
            TypeReferenceParser::with_limits(
                "Vec<Vec<TruckResult>>",
                &mount,
                &local_types,
                TypeReferenceLimits::new(usize::MAX, 2, usize::MAX),
            )
            .parse(),
            Err(TypeReferenceParseError::Limit {
                kind: TypeReferenceLimitKind::NestingDepth,
                observed: 3,
                maximum: 2,
            })
        );

        assert_eq!(
            TypeReferenceParser::with_limits(
                "(TruckResult, TruckResult)",
                &mount,
                &local_types,
                TypeReferenceLimits::new(usize::MAX, usize::MAX, 2),
            )
            .parse(),
            Err(TypeReferenceParseError::Limit {
                kind: TypeReferenceLimitKind::Work,
                observed: 3,
                maximum: 2,
            })
        );
    }

    #[test]
    fn generated_fact_projection_reports_the_production_depth_limit() {
        let mount = ModuleMountPath::new("truck").expect("mount");
        let import = ExternalModuleImportId::new("truck").expect("import");
        let local_types = BTreeSet::from(["TruckResult"]);
        let nested = (0..TypeReferenceLimits::PRODUCTION.nesting_depth())
            .fold("TruckResult".to_owned(), |inner, _| format!("Vec<{inner}>"));
        let reference = TypeReference::new(nested).expect("visible type reference");

        let error = mounted_type(&reference, &mount, &local_types, &import, "drive_truck")
            .expect_err("the production projector must reject excessive nesting");
        assert!(matches!(
            error,
            ExternalModuleFactsError::TypeReferenceLimit {
                kind: TypeReferenceLimitKind::NestingDepth,
                observed: 65,
                maximum: 64,
                ..
            }
        ));
    }
}
