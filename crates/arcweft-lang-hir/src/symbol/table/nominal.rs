//! Nominal declaration publication from final arena HIR.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModuleSegment},
};
use arcweft_source::SourceSpan;

use crate::identity::{ItemId, TypeId};
use crate::item::{
    HirEnumItem, HirEnumVariant, HirGenericParameter, HirRequiredName, HirStructField,
    HirStructItem, HirTypeAliasItem, HirWherePredicate,
};
use crate::module::HirModule;
use crate::proof_return::HirProofReturnHeaderModuleView;
use crate::source_index::{
    HirDeclarationSourceRole, HirItemSourceRole, HirNominalMemberSourcePart, HirSourcePresence,
    HirSourceQuery, HirSourceSite, HirTypeSourceRole,
};
use crate::type_ref::{HirAssociatedTypeBinding, HirType, HirTypeKind};

use super::super::nominal::{
    ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalDeclarationError,
    ProjectNominalDeclarationId, ProjectNominalDeclarationKind, ProjectNominalDeclarationSource,
    ProjectNominalField, ProjectNominalFieldSource, ProjectNominalTypeParameter,
    ProjectNominalTypeParameterSource, ProjectNominalVariant, ProjectNominalVariantSource,
    ProjectNominalWherePredicate,
};
use super::{
    ProjectSymbolLimitKind, ProjectSymbolLimits, ProjectSymbolRevision, ProjectSymbolWorldId,
};

#[derive(Clone, Copy)]
pub(super) enum NominalHir<'a> {
    Struct(&'a HirStructItem),
    Enum(&'a HirEnumItem),
    TypeAlias(&'a HirTypeAliasItem),
}

#[derive(Clone, Copy)]
pub(super) enum NominalModuleView<'a, 'source> {
    Published(&'a HirModule),
    ProofHeader(HirProofReturnHeaderModuleView<'a, 'source>),
}

impl<'a> NominalModuleView<'a, '_> {
    fn resolve_type(self, owner: TypeId) -> Option<&'a HirTypeKind> {
        match self {
            Self::Published(module) => module.resolve_type(owner).ok().map(HirType::kind),
            Self::ProofHeader(module) => module.resolve_type(owner).ok().map(HirType::kind),
        }
    }

    fn item_source(self, owner: ItemId, role: HirItemSourceRole) -> Option<SourceSpan> {
        match self {
            Self::Published(module) => {
                let lookup = module
                    .source_site(
                        module.provenance().source_identity(),
                        HirSourceQuery::Item { owner, role },
                    )
                    .ok()?;
                match lookup.presence() {
                    HirSourcePresence::Present(HirSourceSite::Span(span)) => Some(span.clone()),
                    HirSourcePresence::Present(HirSourceSite::Insertion(_))
                    | HirSourcePresence::AbsentOptional => None,
                }
            }
            Self::ProofHeader(module) => match module.item_source_site(owner, role)? {
                HirSourceSite::Span(span) => Some(span.clone()),
                HirSourceSite::Insertion(_) => None,
            },
        }
    }

    fn type_source(self, owner: TypeId, role: HirTypeSourceRole) -> Option<SourceSpan> {
        match self {
            Self::Published(module) => {
                let lookup = module
                    .source_site(
                        module.provenance().source_identity(),
                        HirSourceQuery::Type { owner, role },
                    )
                    .ok()?;
                match lookup.presence() {
                    HirSourcePresence::Present(HirSourceSite::Span(span)) => Some(span.clone()),
                    HirSourcePresence::Present(HirSourceSite::Insertion(_))
                    | HirSourcePresence::AbsentOptional => None,
                }
            }
            Self::ProofHeader(module) => match module.type_source_site(owner, role)? {
                HirSourceSite::Span(span) => Some(span.clone()),
                HirSourceSite::Insertion(_) => None,
            },
        }
    }
}

impl<'a> NominalHir<'a> {
    pub(super) const fn kind(self) -> ProjectNominalDeclarationKind {
        match self {
            Self::Struct(_) => ProjectNominalDeclarationKind::Struct,
            Self::Enum(_) => ProjectNominalDeclarationKind::Enum,
            Self::TypeAlias(_) => ProjectNominalDeclarationKind::TypeAlias,
        }
    }

    fn name(self) -> &'a HirRequiredName {
        match self {
            Self::Struct(item) => item.name(),
            Self::Enum(item) => item.name(),
            Self::TypeAlias(item) => item.name(),
        }
    }

    fn generic_parameters(self) -> &'a [HirGenericParameter] {
        match self {
            Self::Struct(item) => item.generic_parameters(),
            Self::Enum(item) => item.generic_parameters(),
            Self::TypeAlias(item) => item.generic_parameters(),
        }
    }

    fn where_predicates(self) -> &'a [HirWherePredicate] {
        match self {
            Self::Struct(item) => item.where_predicates(),
            Self::Enum(item) => item.where_predicates(),
            Self::TypeAlias(item) => item.where_predicates(),
        }
    }

    pub(super) fn member_count(self) -> usize {
        match self {
            Self::Struct(item) => item.fields().len(),
            Self::Enum(item) => item.variants().len(),
            Self::TypeAlias(_) => 0,
        }
    }

    fn root_types(self) -> Vec<TypeId> {
        let mut types = Vec::new();
        for parameter in self.generic_parameters() {
            types.extend_from_slice(parameter.bounds());
        }
        for predicate in self.where_predicates() {
            types.push(predicate.subject());
            types.extend_from_slice(predicate.bounds());
        }
        match self {
            Self::Struct(item) => types.extend(item.fields().iter().map(HirStructField::ty)),
            Self::Enum(item) => {
                types.extend(item.variants().iter().filter_map(HirEnumVariant::payload));
            }
            Self::TypeAlias(item) => types.push(item.target()),
        }
        types
    }

    pub(super) fn link_work_units(self, module: NominalModuleView<'_, '_>) -> u64 {
        1_u64
            .saturating_add(u64::try_from(self.generic_parameters().len()).unwrap_or(u64::MAX))
            .saturating_add(u64::try_from(self.member_count()).unwrap_or(u64::MAX))
            .saturating_add(type_graph_count(module, self.root_types()))
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one nominal publication transaction validates identity, source roles, generics, members, and accounting before construction"
)]
pub(super) fn build_nominal_declaration(
    owner: ItemId,
    hir: NominalHir<'_>,
    visibility: Option<Visibility>,
    module: NominalModuleView<'_, '_>,
    module_path: &CanonicalModulePath,
    world: ProjectSymbolWorldId,
    revision: ProjectSymbolRevision,
) -> Result<ProjectNominalDeclaration, ProjectNominalDeclarationError> {
    let whole = item_span(module, owner);
    let name = required_name(hir.name(), &whole)?;
    let name_source = item_component_span(
        module,
        owner,
        HirItemSourceRole::Declaration(HirDeclarationSourceRole::Name),
    )
    .unwrap_or_else(|| whole.clone());

    check_limit(
        ProjectSymbolLimitKind::NominalTypeParameters,
        hir.generic_parameters().len(),
        ProjectSymbolLimits::PRODUCTION.nominal_type_parameters(),
        &whole,
    )?;
    check_limit(
        ProjectSymbolLimitKind::NominalTypeNodesPerDeclaration,
        type_graph_count(module, hir.root_types()),
        ProjectSymbolLimits::PRODUCTION.nominal_type_nodes_per_declaration(),
        &whole,
    )?;

    let type_parameters = build_type_parameters(hir, module, &whole)?;
    let where_predicates = hir
        .where_predicates()
        .iter()
        .map(|predicate| ProjectNominalWherePredicate {
            subject: predicate.subject(),
            bounds: predicate.bounds().into(),
            whole: type_span(module, predicate.subject()),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let body = match hir {
        NominalHir::Struct(item) => ProjectNominalBody::Struct {
            fields: item
                .fields()
                .iter()
                .enumerate()
                .map(|(ordinal, field)| {
                    let ordinal = u32::try_from(ordinal).expect("nominal member limit fits u32");
                    let whole = nominal_member_span(
                        module,
                        owner,
                        HirDeclarationSourceRole::StructField {
                            field: ordinal,
                            part: HirNominalMemberSourcePart::Whole,
                        },
                    );
                    let name_source = nominal_member_span(
                        module,
                        owner,
                        HirDeclarationSourceRole::StructField {
                            field: ordinal,
                            part: HirNominalMemberSourcePart::Name,
                        },
                    );
                    Ok(ProjectNominalField {
                        name: required_name(field.name(), &name_source)?,
                        ty: field.ty(),
                        source: ProjectNominalFieldSource {
                            whole,
                            name: name_source,
                        },
                    })
                })
                .collect::<Result<Vec<_>, ProjectNominalDeclarationError>>()?
                .into_boxed_slice(),
        },
        NominalHir::Enum(item) => ProjectNominalBody::Enum {
            variants: item
                .variants()
                .iter()
                .enumerate()
                .map(|(ordinal, variant)| {
                    let ordinal = u32::try_from(ordinal).expect("nominal member limit fits u32");
                    let variant_whole = nominal_member_span(
                        module,
                        owner,
                        HirDeclarationSourceRole::EnumVariant {
                            variant: ordinal,
                            part: HirNominalMemberSourcePart::Whole,
                        },
                    );
                    let name_source = nominal_member_span(
                        module,
                        owner,
                        HirDeclarationSourceRole::EnumVariant {
                            variant: ordinal,
                            part: HirNominalMemberSourcePart::Name,
                        },
                    );
                    Ok(ProjectNominalVariant {
                        name: required_name(variant.name(), &name_source)?,
                        payload: variant.payload(),
                        source: ProjectNominalVariantSource {
                            whole: variant_whole,
                            name: name_source,
                            payload: variant.payload().map(|payload| type_span(module, payload)),
                        },
                    })
                })
                .collect::<Result<Vec<_>, ProjectNominalDeclarationError>>()?
                .into_boxed_slice(),
        },
        NominalHir::TypeAlias(item) => ProjectNominalBody::TypeAlias {
            target: item.target(),
        },
    };

    Ok(ProjectNominalDeclaration {
        id: ProjectNominalDeclarationId {
            world,
            revision,
            module: module_path.clone(),
            kind: hir.kind(),
            owner_path: Box::new([]),
            name,
        },
        owner,
        visibility,
        type_parameters,
        where_predicates,
        body,
        source: ProjectNominalDeclarationSource {
            whole,
            name: name_source,
            generics: None,
        },
    })
}

fn build_type_parameters(
    hir: NominalHir<'_>,
    module: NominalModuleView<'_, '_>,
    declaration_source: &SourceSpan,
) -> Result<Box<[ProjectNominalTypeParameter]>, ProjectNominalDeclarationError> {
    let mut parameters = Vec::with_capacity(hir.generic_parameters().len());
    let mut names = BTreeMap::<ModuleSegment, SourceSpan>::new();
    for (ordinal, parameter) in hir.generic_parameters().iter().enumerate() {
        let HirGenericParameter::Type { name, bounds } = parameter else {
            return Err(
                ProjectNominalDeclarationError::UnsupportedLifetimeParameter {
                    source: declaration_source.clone(),
                },
            );
        };
        let source = bounds.first().map_or_else(
            || declaration_source.clone(),
            |bound| type_span(module, *bound),
        );
        let name = required_name(name, &source)?;
        if let Some(first) = names.insert(name.clone(), source.clone()) {
            return Err(ProjectNominalDeclarationError::DuplicateTypeParameter {
                name,
                first,
                duplicate: source,
            });
        }
        parameters.push(ProjectNominalTypeParameter {
            ordinal: u16::try_from(ordinal).expect("nominal generic limit fits u16"),
            name,
            bounds: bounds.clone(),
            source: ProjectNominalTypeParameterSource {
                whole: source.clone(),
                name: source,
            },
        });
    }
    Ok(parameters.into_boxed_slice())
}

fn required_name(
    name: &HirRequiredName,
    source: &SourceSpan,
) -> Result<ModuleSegment, ProjectNominalDeclarationError> {
    let name = name
        .resolved()
        .ok_or_else(|| ProjectNominalDeclarationError::RecoveredName {
            source: source.clone(),
        })?;
    ModuleSegment::new(name.as_str()).map_err(|reason| {
        ProjectNominalDeclarationError::InvalidName {
            source: source.clone(),
            reason,
        }
    })
}

fn item_span(module: NominalModuleView<'_, '_>, owner: ItemId) -> SourceSpan {
    item_component_span(
        module,
        owner,
        HirItemSourceRole::Declaration(HirDeclarationSourceRole::Whole),
    )
    .expect("source-backed final HIR item owns an authored whole span")
}

fn item_component_span(
    module: NominalModuleView<'_, '_>,
    owner: ItemId,
    role: HirItemSourceRole,
) -> Option<SourceSpan> {
    module.item_source(owner, role)
}

fn nominal_member_span(
    module: NominalModuleView<'_, '_>,
    owner: ItemId,
    role: HirDeclarationSourceRole,
) -> SourceSpan {
    item_component_span(module, owner, HirItemSourceRole::Declaration(role))
        .expect("source-backed nominal member owns an exact authored span")
}

fn type_span(module: NominalModuleView<'_, '_>, owner: TypeId) -> SourceSpan {
    module
        .type_source(owner, HirTypeSourceRole::Whole)
        .expect("authored nominal type identity requires a source span")
}

fn type_graph_count(module: NominalModuleView<'_, '_>, roots: Vec<TypeId>) -> u64 {
    let mut seen = BTreeSet::new();
    let mut pending = roots;
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(kind) = module.resolve_type(id) else {
            continue;
        };
        match kind {
            HirTypeKind::Tuple(children) | HirTypeKind::Choice(children) => {
                pending.extend_from_slice(children);
            }
            HirTypeKind::Function(function) => {
                pending.extend_from_slice(function.parameters());
                pending.push(function.return_type());
            }
            HirTypeKind::Generic(generic) => pending.extend_from_slice(generic.arguments()),
            HirTypeKind::TraitBound(bound) => {
                pending.extend_from_slice(bound.arguments());
                pending.extend(
                    bound
                        .associated()
                        .iter()
                        .map(HirAssociatedTypeBinding::value),
                );
            }
            HirTypeKind::Projection(projection) => pending.push(projection.subject()),
            HirTypeKind::Reference(reference) => pending.push(reference.referent()),
            HirTypeKind::Slice(element) => pending.push(*element),
            HirTypeKind::Never
            | HirTypeKind::ConstInt(_)
            | HirTypeKind::Path(_)
            | HirTypeKind::Recovery(_) => {}
        }
    }
    u64::try_from(seen.len()).unwrap_or(u64::MAX)
}

fn check_limit(
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
