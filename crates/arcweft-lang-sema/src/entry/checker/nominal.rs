use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use arcweft_data::{BytesFormat, FieldShape, TypeShape, VariantShape};
use arcweft_lang_hir::{
    identity::TypeId,
    symbol::{
        ProjectSymbolTable,
        nominal::{
            ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalDeclarationId,
            ProjectNominalDeclarationKind,
        },
    },
};

use crate::{
    final_analysis::FinalSemanticAnalysis,
    types::{GenericTypeOwnerId, GenericTypeParameterId, MapKind, TypeKind},
};

/// Entry-owned data-shape projection over already checked nominal types.
///
/// Name selection, imports, aliases, arity, and generic argument validation are
/// owned by the normal semantic nominal resolver. This adapter only projects
/// its accepted `TypeKind` products into the persistence schema required by an
/// entry; it never resolves authored paths itself.
pub(super) struct NominalSchemaExpander<'a> {
    symbols: &'a ProjectSymbolTable,
    analysis: &'a FinalSemanticAnalysis,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NominalSchemaError {
    path: Vec<String>,
    reason: String,
}

impl NominalSchemaError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            path: Vec::new(),
            reason: reason.into(),
        }
    }

    fn within(mut self, segment: impl Into<String>) -> Self {
        self.path.insert(0, segment.into());
        self
    }
}

impl fmt::Display for NominalSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            formatter.write_str(&self.reason)
        } else {
            write!(formatter, "{}: {}", self.path.join(" -> "), self.reason)
        }
    }
}

impl<'a> NominalSchemaExpander<'a> {
    pub(super) const fn new(
        symbols: &'a ProjectSymbolTable,
        analysis: &'a FinalSemanticAnalysis,
    ) -> Self {
        Self { symbols, analysis }
    }

    pub(super) fn schema(
        &self,
        declaration: &ProjectNominalDeclaration,
    ) -> Result<TypeShape, NominalSchemaError> {
        if !declaration.type_parameters().is_empty() {
            return Err(NominalSchemaError::new(format!(
                "generic project type `{}` requires checked type arguments",
                declaration.id().qualified_name()
            )));
        }
        self.schema_with_stack(declaration, &[], &BTreeMap::new(), &mut BTreeSet::new())
    }

    fn schema_with_stack(
        &self,
        declaration: &ProjectNominalDeclaration,
        arguments: &[TypeKind],
        inherited: &BTreeMap<GenericTypeParameterId, TypeKind>,
        stack: &mut BTreeSet<ProjectNominalDeclarationId>,
    ) -> Result<TypeShape, NominalSchemaError> {
        if !stack.insert(declaration.id().clone()) {
            return Ok(TypeShape::Named(canonical_nominal_name(declaration.id())));
        }

        if declaration.type_parameters().len() != arguments.len() {
            stack.remove(declaration.id());
            return Err(NominalSchemaError::new(format!(
                "checked project type `{}` expected {} argument(s), found {}",
                declaration.id().qualified_name(),
                declaration.type_parameters().len(),
                arguments.len()
            )));
        }

        let mut substitutions = inherited.clone();
        for (parameter, argument) in declaration.type_parameters().iter().zip(arguments) {
            substitutions.insert(
                GenericTypeParameterId::new(
                    GenericTypeOwnerId::Nominal(declaration.id().clone()),
                    parameter.ordinal(),
                ),
                argument.clone(),
            );
        }

        let result = match declaration.body() {
            ProjectNominalBody::Struct { fields } => fields
                .iter()
                .map(|field| {
                    self.resolved_shape(field.ty(), &substitutions, stack)
                        .map_err(|error| error.within(format!("field `{}`", field.name())))
                        .map(|shape| {
                            FieldShape::new(field.name().as_str(), field.name().as_str(), shape)
                        })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|fields| TypeShape::record(canonical_nominal_name(declaration.id()), fields)),
            ProjectNominalBody::Enum { variants } => variants
                .iter()
                .map(|variant| {
                    let unit = VariantShape::unit(variant.name().as_str(), variant.name().as_str());
                    let Some(payload) = variant.payload() else {
                        return Ok(unit);
                    };
                    self.resolved_shape(payload, &substitutions, stack)
                        .map_err(|error| {
                            error.within(format!("variant `{}` payload", variant.name()))
                        })
                        .map(|shape| unit.with_payload(shape))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|variants| {
                    TypeShape::enumeration(canonical_nominal_name(declaration.id()), variants)
                }),
            ProjectNominalBody::TypeAlias { .. } => Err(NominalSchemaError::new(
                "entry data schemas must start from a project struct or enum, not an alias",
            )),
        };

        stack.remove(declaration.id());
        result
    }

    fn resolved_shape(
        &self,
        root: TypeId,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
        stack: &mut BTreeSet<ProjectNominalDeclarationId>,
    ) -> Result<TypeShape, NominalSchemaError> {
        let ty = self.analysis.ty(root).ok_or_else(|| {
            NominalSchemaError::new(format!(
                "accepted final semantic analysis has no type fact for {root:?}"
            ))
        })?;
        self.type_shape(ty, substitutions, stack, &mut BTreeSet::new())
    }

    fn type_shape(
        &self,
        ty: &TypeKind,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
        stack: &mut BTreeSet<ProjectNominalDeclarationId>,
        generic_stack: &mut BTreeSet<GenericTypeParameterId>,
    ) -> Result<TypeShape, NominalSchemaError> {
        let mut recurse = |inner: &TypeKind,
                           generic_stack: &mut BTreeSet<GenericTypeParameterId>|
         -> Result<TypeShape, NominalSchemaError> {
            self.type_shape(inner, substitutions, stack, generic_stack)
        };

        Ok(match ty {
            TypeKind::Unit => TypeShape::Unit,
            TypeKind::Bool => TypeShape::Bool,
            TypeKind::I8 => TypeShape::I8,
            TypeKind::I16 => TypeShape::I16,
            TypeKind::I32 => TypeShape::I32,
            TypeKind::I64 => TypeShape::I64,
            TypeKind::I128 => TypeShape::I128,
            TypeKind::ISize => TypeShape::Isize,
            TypeKind::U8 => TypeShape::U8,
            TypeKind::U16 => TypeShape::U16,
            TypeKind::U32 => TypeShape::U32,
            TypeKind::U64 => TypeShape::U64,
            TypeKind::U128 => TypeShape::U128,
            TypeKind::USize => TypeShape::Usize,
            TypeKind::F32 => TypeShape::F32,
            TypeKind::F64 => TypeShape::F64,
            TypeKind::String => TypeShape::String,
            TypeKind::Char => TypeShape::Char,
            TypeKind::Bytes => TypeShape::Bytes {
                format: BytesFormat::Binary,
            },
            TypeKind::Option(inner) => TypeShape::option(recurse(inner, generic_stack)?),
            TypeKind::Vec(inner) | TypeKind::Seq(inner) => {
                TypeShape::seq(recurse(inner, generic_stack)?)
            }
            TypeKind::Map {
                kind: MapKind::Ordered | MapKind::Sorted | MapKind::BTree,
                key,
                value,
            } => TypeShape::map(
                recurse(key, generic_stack).map_err(|error| error.within("map key"))?,
                recurse(value, generic_stack).map_err(|error| error.within("map value"))?,
            ),
            TypeKind::ProjectNominal(nominal) => {
                let declaration = self.symbols.nominal(nominal.declaration()).ok_or_else(|| {
                    NominalSchemaError::new(format!(
                        "checked project nominal `{}` is absent from its symbol world",
                        nominal.declaration().qualified_name()
                    ))
                })?;
                self.schema_with_stack(declaration, nominal.arguments(), substitutions, stack)?
            }
            TypeKind::GenericParam(parameter) => {
                if !generic_stack.insert(parameter.clone()) {
                    return Err(NominalSchemaError::new(format!(
                        "cyclic generic substitution for parameter #{}",
                        parameter.ordinal()
                    )));
                }
                let replacement = substitutions.get(parameter).ok_or_else(|| {
                    NominalSchemaError::new(format!(
                        "unbound generic parameter #{} in checked data schema",
                        parameter.ordinal()
                    ))
                })?;
                let shape = recurse(replacement, generic_stack)?;
                generic_stack.remove(parameter);
                shape
            }
            TypeKind::Error(poison) => {
                return Err(NominalSchemaError::new(format!(
                    "poisoned type {} cannot define a persisted data schema",
                    poison.index()
                )));
            }
            unsupported => {
                return Err(NominalSchemaError::new(format!(
                    "checked type `{}` is not a canonical persisted data shape",
                    unsupported.source_label()
                )));
            }
        })
    }
}

fn canonical_nominal_name(id: &ProjectNominalDeclarationId) -> String {
    let kind = match id.kind() {
        ProjectNominalDeclarationKind::Struct => "struct",
        ProjectNominalDeclarationKind::Enum => "enum",
        ProjectNominalDeclarationKind::TypeAlias => "type_alias",
    };
    format!(
        "package={};module={};kind={kind};name={}",
        id.world().package(),
        id.module(),
        id.name()
    )
}
