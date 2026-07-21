//! Nominal declaration publication from syntax-owned authored type records.

use std::collections::BTreeMap;

use arcweft_lang_syntax::ast::{
    common::{TextRange, Visibility},
    items::{EnumItem, EnumVariant, StructField, StructItem, TypeAliasItem},
    module_path::{CanonicalModulePath, ModuleSegment},
};
use arcweft_lang_syntax::types::{AuthoredTypeRef, GenericParam};
use arcweft_source::SourceSpan;

use crate::model::HirModule;

use super::super::nominal::{
    ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalDeclarationError,
    ProjectNominalDeclarationId, ProjectNominalDeclarationKind, ProjectNominalDeclarationSource,
    ProjectNominalField, ProjectNominalFieldSource, ProjectNominalTypeParameter,
    ProjectNominalTypeParameterSource, ProjectNominalVariant, ProjectNominalVariantSource,
    SourceBackedTypeRef, SourceBackedWherePredicate,
};
use super::{
    ProjectSymbolLimitKind, ProjectSymbolLimits, ProjectSymbolRevision, ProjectSymbolWorldId,
};

#[derive(Clone, Copy)]
pub(super) enum NominalSyntax<'a> {
    Struct(&'a StructItem),
    Enum(&'a EnumItem),
    TypeAlias(&'a TypeAliasItem),
}

impl<'a> NominalSyntax<'a> {
    const fn kind(self) -> ProjectNominalDeclarationKind {
        match self {
            Self::Struct(_) => ProjectNominalDeclarationKind::Struct,
            Self::Enum(_) => ProjectNominalDeclarationKind::Enum,
            Self::TypeAlias(_) => ProjectNominalDeclarationKind::TypeAlias,
        }
    }

    fn name(self) -> &'a str {
        match self {
            Self::Struct(item) => item.name(),
            Self::Enum(item) => item.name(),
            Self::TypeAlias(item) => item.name(),
        }
    }

    const fn name_range(self) -> TextRange {
        match self {
            Self::Struct(item) => *item.name_range(),
            Self::Enum(item) => *item.name_range(),
            Self::TypeAlias(item) => item.name_range(),
        }
    }

    pub(super) const fn range(self) -> TextRange {
        match self {
            Self::Struct(item) => *item.range(),
            Self::Enum(item) => *item.range(),
            Self::TypeAlias(item) => *item.range(),
        }
    }

    const fn visibility(self) -> Option<Visibility> {
        match self {
            Self::Struct(item) => item.visibility(),
            Self::Enum(item) => item.visibility(),
            Self::TypeAlias(item) => item.visibility(),
        }
    }

    fn generic_params(self) -> &'a [GenericParam] {
        match self {
            Self::Struct(item) => item.generic_params(),
            Self::Enum(item) => item.generic_params(),
            Self::TypeAlias(item) => item.generic_params(),
        }
    }

    const fn generic_range(self) -> Option<TextRange> {
        match self {
            Self::Struct(item) => item.generic_range(),
            Self::Enum(item) => item.generic_range(),
            Self::TypeAlias(item) => item.generic_range(),
        }
    }

    fn where_clauses(self) -> &'a [arcweft_lang_syntax::types::WhereClause] {
        match self {
            Self::Struct(item) => item.where_clauses(),
            Self::Enum(item) => item.where_clauses(),
            Self::TypeAlias(item) => item.where_clauses(),
        }
    }

    fn member_count(self) -> usize {
        match self {
            Self::Struct(item) => item.fields().len(),
            Self::Enum(item) => item.variants().len(),
            Self::TypeAlias(_) => 0,
        }
    }

    fn authored_types(self) -> Vec<&'a AuthoredTypeRef> {
        let mut types = Vec::new();
        for parameter in self.generic_params() {
            if let Some(parameter) = parameter.as_type_param() {
                types.extend(parameter.bounds());
            }
        }
        for predicate in self.where_clauses() {
            types.push(predicate.subject());
            types.extend(predicate.bounds());
        }
        match self {
            Self::Struct(item) => types.extend(item.fields().iter().map(StructField::ty)),
            Self::Enum(item) => {
                types.extend(item.variants().iter().filter_map(EnumVariant::payload));
            }
            Self::TypeAlias(item) => types.push(item.target()),
        }
        types
    }

    fn type_node_count(self) -> u64 {
        self.authored_types().into_iter().fold(0_u64, |count, ty| {
            count.saturating_add(u64::try_from(ty.source().nodes().len()).unwrap_or(u64::MAX))
        })
    }

    pub(super) fn link_work_units(self) -> u64 {
        1_u64
            .saturating_add(u64::try_from(self.generic_params().len()).unwrap_or(u64::MAX))
            .saturating_add(u64::try_from(self.member_count()).unwrap_or(u64::MAX))
            .saturating_add(self.type_node_count())
    }
}

fn build_type_parameters(
    syntax: NominalSyntax<'_>,
    module: &HirModule,
) -> Result<Box<[ProjectNominalTypeParameter]>, ProjectNominalDeclarationError> {
    let mut type_parameters = Vec::with_capacity(syntax.generic_params().len());
    let mut names = BTreeMap::<ModuleSegment, SourceSpan>::new();
    for (ordinal, parameter) in syntax.generic_params().iter().enumerate() {
        let parameter = match parameter {
            GenericParam::Lifetime(lifetime) => {
                return Err(
                    ProjectNominalDeclarationError::UnsupportedLifetimeParameter {
                        source: module_span(module, lifetime.range()),
                    },
                );
            }
            GenericParam::Type(parameter) => parameter,
        };
        let parameter_name = parameter.name().clone();
        let parameter_name_source = module_span(module, parameter.name_range());
        if let Some(first) = names.insert(parameter_name.clone(), parameter_name_source.clone()) {
            return Err(ProjectNominalDeclarationError::DuplicateTypeParameter {
                name: parameter_name,
                first,
                duplicate: parameter_name_source,
            });
        }
        type_parameters.push(ProjectNominalTypeParameter {
            ordinal: u16::try_from(ordinal).expect("nominal type-parameter limit fits u16"),
            name: parameter_name,
            bounds: parameter
                .bounds()
                .iter()
                .map(|bound| bind_nominal_type(bound, module))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            source: ProjectNominalTypeParameterSource {
                whole: module_span(module, parameter.range()),
                name: parameter_name_source,
            },
        });
    }
    Ok(type_parameters.into_boxed_slice())
}

fn build_where_predicates(
    syntax: NominalSyntax<'_>,
    module: &HirModule,
) -> Result<Box<[SourceBackedWherePredicate]>, ProjectNominalDeclarationError> {
    syntax
        .where_clauses()
        .iter()
        .map(|predicate| {
            Ok(SourceBackedWherePredicate {
                subject: bind_nominal_type(predicate.subject(), module)?,
                bounds: predicate
                    .bounds()
                    .iter()
                    .map(|bound| bind_nominal_type(bound, module))
                    .collect::<Result<Vec<_>, ProjectNominalDeclarationError>>()?
                    .into_boxed_slice(),
                whole: module_span(module, predicate.range()),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn build_nominal_body(
    syntax: NominalSyntax<'_>,
    module: &HirModule,
) -> Result<ProjectNominalBody, ProjectNominalDeclarationError> {
    match syntax {
        NominalSyntax::Struct(item) => Ok(ProjectNominalBody::Struct {
            fields: item
                .fields()
                .iter()
                .map(|field| build_struct_field(field, module))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        }),
        NominalSyntax::Enum(item) => Ok(ProjectNominalBody::Enum {
            variants: item
                .variants()
                .iter()
                .map(|variant| build_enum_variant(variant, module))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        }),
        NominalSyntax::TypeAlias(item) => Ok(ProjectNominalBody::TypeAlias {
            target: bind_nominal_type(item.target(), module)?,
        }),
    }
}

fn build_struct_field(
    field: &StructField,
    module: &HirModule,
) -> Result<ProjectNominalField, ProjectNominalDeclarationError> {
    let name_source = module_span(module, field.name_range());
    let name = ModuleSegment::new(field.name()).map_err(|reason| {
        ProjectNominalDeclarationError::InvalidName {
            source: name_source.clone(),
            reason,
        }
    })?;
    Ok(ProjectNominalField {
        name,
        ty: bind_nominal_type(field.ty(), module)?,
        source: ProjectNominalFieldSource {
            whole: module_span(module, field.range()),
            name: name_source,
        },
    })
}

fn build_enum_variant(
    variant: &EnumVariant,
    module: &HirModule,
) -> Result<ProjectNominalVariant, ProjectNominalDeclarationError> {
    let name_source = module_span(module, variant.name_range());
    let name = ModuleSegment::new(variant.name()).map_err(|reason| {
        ProjectNominalDeclarationError::InvalidName {
            source: name_source.clone(),
            reason,
        }
    })?;
    Ok(ProjectNominalVariant {
        name,
        payload: variant
            .payload()
            .map(|payload| bind_nominal_type(payload, module))
            .transpose()?,
        source: ProjectNominalVariantSource {
            whole: module_span(module, variant.range()),
            name: name_source,
            payload: variant
                .payload_range()
                .map(|range| module_span(module, range)),
        },
    })
}

pub(super) fn build_nominal_declaration(
    syntax: NominalSyntax<'_>,
    module: &HirModule,
    module_path: &CanonicalModulePath,
    world: ProjectSymbolWorldId,
    revision: ProjectSymbolRevision,
) -> Result<ProjectNominalDeclaration, ProjectNominalDeclarationError> {
    let source = module_span(module, syntax.range());
    let name_source = module_span(module, syntax.name_range());
    let name = ModuleSegment::new(syntax.name()).map_err(|reason| {
        ProjectNominalDeclarationError::InvalidName {
            source: name_source.clone(),
            reason,
        }
    })?;
    check_nominal_limit(
        ProjectSymbolLimitKind::NominalMembersPerDeclaration,
        syntax.member_count(),
        ProjectSymbolLimits::PRODUCTION.nominal_members_per_declaration(),
        &source,
    )?;
    check_nominal_limit(
        ProjectSymbolLimitKind::NominalTypeParameters,
        syntax.generic_params().len(),
        ProjectSymbolLimits::PRODUCTION.nominal_type_parameters(),
        &source,
    )?;
    check_nominal_limit(
        ProjectSymbolLimitKind::NominalTypeNodesPerDeclaration,
        syntax.type_node_count(),
        ProjectSymbolLimits::PRODUCTION.nominal_type_nodes_per_declaration(),
        &source,
    )?;

    let type_parameters = build_type_parameters(syntax, module)?;
    let where_predicates = build_where_predicates(syntax, module)?;
    let body = build_nominal_body(syntax, module)?;
    Ok(ProjectNominalDeclaration {
        id: ProjectNominalDeclarationId {
            world,
            revision,
            module: module_path.clone(),
            kind: syntax.kind(),
            owner_path: Box::new([]),
            name,
        },
        visibility: syntax.visibility(),
        type_parameters,
        where_predicates,
        body,
        source: ProjectNominalDeclarationSource {
            whole: source,
            name: name_source,
            generics: syntax
                .generic_range()
                .map(|range| module_span(module, range)),
        },
    })
}

fn bind_nominal_type(
    authored: &AuthoredTypeRef,
    module: &HirModule,
) -> Result<SourceBackedTypeRef, ProjectNominalDeclarationError> {
    let source = module_span(module, *authored.root_source().whole());
    let document = module
        .source_document()
        .expect("project symbol publication requires source-bound HIR modules");
    SourceBackedTypeRef::try_bind(authored.clone(), document, document.identity()).map_err(
        |reason| ProjectNominalDeclarationError::SourceMapMismatch {
            source,
            reason: Box::new(reason),
        },
    )
}

fn check_nominal_limit(
    kind: ProjectSymbolLimitKind,
    observed: impl TryInto<u64>,
    maximum: u64,
    source: &SourceSpan,
) -> Result<(), ProjectNominalDeclarationError> {
    let observed = observed.try_into().unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(ProjectNominalDeclarationError::Limit {
            kind,
            observed,
            maximum,
            source: source.clone(),
        });
    }
    Ok(())
}

fn module_span(module: &HirModule, range: TextRange) -> SourceSpan {
    module
        .source_span(range)
        .expect("project symbol publication requires exact admitted HIR ranges")
}
