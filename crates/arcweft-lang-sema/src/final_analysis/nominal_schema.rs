use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

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
    final_analysis::{CheckedProjectNominal, FinalSemanticAnalysis},
    types::{GenericTypeOwnerId, GenericTypeParameterId, MapKind, TypeKind},
};

/// Generation-bound data-shape projection over already checked nominal types.
///
/// Name selection, imports, aliases, arity, and generic argument validation are
/// owned by the normal semantic nominal resolver. This adapter only projects
/// its accepted `TypeKind` products into the persistence schema required by an
/// entry; it never resolves authored paths itself.
struct NominalSchemaExpander<'a> {
    symbols: &'a ProjectSymbolTable,
    analysis: &'a FinalSemanticAnalysis,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum NominalSchemaProjectionError {
    #[error("checked nominal belongs to a different project-symbol generation")]
    GenerationMismatch,
    #[error("checked project nominal `{nominal}` is absent from its symbol world")]
    MissingDeclaration { nominal: String },
    #[error("checked project nominal `{nominal}` has owner {actual:?}, expected {expected:?}")]
    OwnerMismatch {
        nominal: String,
        expected: arcweft_lang_hir::identity::ItemId,
        actual: arcweft_lang_hir::identity::ItemId,
    },
    #[error("checked project nominal `{nominal}` expected {expected} argument(s), found {actual}")]
    WrongArity {
        nominal: String,
        expected: usize,
        actual: usize,
    },
    #[error("accepted final semantic analysis has no type fact for {ty:?}")]
    MissingTypeFact { ty: TypeId },
    #[error("{path}: {reason}")]
    InvalidShape { path: String, reason: String },
}

impl NominalSchemaProjectionError {
    fn new(reason: impl Into<String>) -> Self {
        Self::InvalidShape {
            path: "nominal".to_owned(),
            reason: reason.into(),
        }
    }

    fn within(self, segment: impl Into<String>) -> Self {
        match self {
            Self::InvalidShape { path, reason } => Self::InvalidShape {
                path: format!("{} -> {path}", segment.into()),
                reason,
            },
            other => other,
        }
    }
}

impl FinalSemanticAnalysis {
    /// Projects one checked project nominal into its canonical data shape.
    pub fn project_nominal_schema(
        &self,
        symbols: &ProjectSymbolTable,
        nominal: &CheckedProjectNominal,
    ) -> Result<TypeShape, NominalSchemaProjectionError> {
        if nominal.declaration().world() != symbols.world()
            || nominal.declaration().revision() != *symbols.revision()
        {
            return Err(NominalSchemaProjectionError::GenerationMismatch);
        }
        let declaration = symbols.nominal(nominal.declaration()).ok_or_else(|| {
            NominalSchemaProjectionError::MissingDeclaration {
                nominal: nominal.declaration().qualified_name(),
            }
        })?;
        if declaration.owner() != nominal.owner() {
            return Err(NominalSchemaProjectionError::OwnerMismatch {
                nominal: nominal.declaration().qualified_name(),
                expected: declaration.owner(),
                actual: nominal.owner(),
            });
        }
        if declaration.type_parameters().len() != nominal.arguments().len() {
            return Err(NominalSchemaProjectionError::WrongArity {
                nominal: nominal.declaration().qualified_name(),
                expected: declaration.type_parameters().len(),
                actual: nominal.arguments().len(),
            });
        }
        NominalSchemaExpander::new(symbols, self).schema_checked(declaration, nominal.arguments())
    }
}

impl<'a> NominalSchemaExpander<'a> {
    const fn new(symbols: &'a ProjectSymbolTable, analysis: &'a FinalSemanticAnalysis) -> Self {
        Self { symbols, analysis }
    }

    fn schema_checked(
        &self,
        declaration: &ProjectNominalDeclaration,
        arguments: &[TypeKind],
    ) -> Result<TypeShape, NominalSchemaProjectionError> {
        self.schema_with_stack(
            declaration,
            arguments,
            &BTreeMap::new(),
            &mut BTreeSet::new(),
        )
    }

    fn schema_with_stack(
        &self,
        declaration: &ProjectNominalDeclaration,
        arguments: &[TypeKind],
        inherited: &BTreeMap<GenericTypeParameterId, TypeKind>,
        stack: &mut BTreeSet<ProjectNominalDeclarationId>,
    ) -> Result<TypeShape, NominalSchemaProjectionError> {
        if !stack.insert(declaration.id().clone()) {
            return Ok(TypeShape::Named(canonical_nominal_name(declaration.id())));
        }

        if declaration.type_parameters().len() != arguments.len() {
            stack.remove(declaration.id());
            return Err(NominalSchemaProjectionError::WrongArity {
                nominal: declaration.id().qualified_name(),
                expected: declaration.type_parameters().len(),
                actual: arguments.len(),
            });
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
            ProjectNominalBody::TypeAlias { .. } => Err(NominalSchemaProjectionError::new(
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
    ) -> Result<TypeShape, NominalSchemaProjectionError> {
        let ty = self
            .analysis
            .ty(root)
            .ok_or(NominalSchemaProjectionError::MissingTypeFact { ty: root })?;
        self.type_shape(ty, substitutions, stack, &mut BTreeSet::new())
    }

    fn type_shape(
        &self,
        ty: &TypeKind,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
        stack: &mut BTreeSet<ProjectNominalDeclarationId>,
        generic_stack: &mut BTreeSet<GenericTypeParameterId>,
    ) -> Result<TypeShape, NominalSchemaProjectionError> {
        let mut recurse = |inner: &TypeKind,
                           generic_stack: &mut BTreeSet<GenericTypeParameterId>|
         -> Result<TypeShape, NominalSchemaProjectionError> {
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
                    NominalSchemaProjectionError::MissingDeclaration {
                        nominal: nominal.declaration().qualified_name(),
                    }
                })?;
                self.schema_with_stack(declaration, nominal.arguments(), substitutions, stack)?
            }
            TypeKind::GenericParam(parameter) => {
                if !generic_stack.insert(parameter.clone()) {
                    return Err(NominalSchemaProjectionError::new(format!(
                        "cyclic generic substitution for parameter #{}",
                        parameter.ordinal()
                    )));
                }
                let replacement = substitutions.get(parameter).ok_or_else(|| {
                    NominalSchemaProjectionError::new(format!(
                        "unbound generic parameter #{} in checked data schema",
                        parameter.ordinal()
                    ))
                })?;
                let shape = recurse(replacement, generic_stack)?;
                generic_stack.remove(parameter);
                shape
            }
            TypeKind::Error(poison) => {
                return Err(NominalSchemaProjectionError::new(format!(
                    "poisoned type {} cannot define a persisted data schema",
                    poison.index()
                )));
            }
            unsupported => {
                return Err(NominalSchemaProjectionError::new(format!(
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
