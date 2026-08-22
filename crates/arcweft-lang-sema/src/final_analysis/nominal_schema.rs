use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use arcweft_core::{
    entry::{
        RuntimeBytesFormat, RuntimeEnumRepr, RuntimeEnumTagStyle, RuntimeSchemaError,
        RuntimeSchemaField, RuntimeSchemaVariant, RuntimeTypeSchema, TypeLayoutHash,
    },
    pattern::RuntimeOpaqueTypeProducerId,
};
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
use arcweft_lang_syntax::ast::module_path::ModuleSegment;

use crate::{
    final_analysis::{CheckedProjectNominal, FinalSemanticAnalysis},
    types::{GenericTypeOwnerId, GenericTypeParameterId, MapKind, SemanticTypeDigest, TypeKind},
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalSchemaPathStep {
    Field {
        ordinal: u32,
        name: ModuleSegment,
    },
    VariantPayload {
        ordinal: u32,
        name: ModuleSegment,
    },
    OptionItem,
    SequenceItem,
    MapKey,
    MapValue,
    ResultOk,
    ResultError,
    TupleItem {
        ordinal: u32,
    },
    GenericArgument {
        ordinal: u32,
    },
    NestedNominal {
        declaration: ProjectNominalDeclarationId,
    },
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NominalSchemaPath(Box<[NominalSchemaPathStep]>);

impl NominalSchemaPath {
    pub const fn steps(&self) -> &[NominalSchemaPathStep] {
        &self.0
    }

    fn prepended(&self, step: NominalSchemaPathStep) -> Self {
        let mut steps = Vec::with_capacity(self.0.len() + 1);
        steps.push(step);
        steps.extend_from_slice(&self.0);
        Self(steps.into_boxed_slice())
    }
}

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
    #[error("accepted opaque type has no closed project-nominal schema layout")]
    OpaqueLeaf {
        path: NominalSchemaPath,
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: SemanticTypeDigest,
    },
    #[error("checked type is not a supported closed project-nominal schema leaf")]
    UnsupportedLeaf {
        path: NominalSchemaPath,
        ty: Box<TypeKind>,
    },
    #[error("project-nominal schema contains a cyclic generic substitution")]
    CyclicGenericSubstitution {
        path: NominalSchemaPath,
        parameter: GenericTypeParameterId,
    },
    #[error("{path}: {reason}")]
    InvalidShape { path: String, reason: String },
    #[error("project-nominal runtime schema layout hash failed: {source}")]
    RuntimeLayout {
        #[source]
        source: RuntimeSchemaError,
    },
}

impl NominalSchemaProjectionError {
    fn new(reason: impl Into<String>) -> Self {
        Self::InvalidShape {
            path: "nominal".to_owned(),
            reason: reason.into(),
        }
    }

    fn within_step(self, step: NominalSchemaPathStep) -> Self {
        match self {
            Self::OpaqueLeaf {
                path,
                producer,
                semantic_identity,
            } => Self::OpaqueLeaf {
                path: path.prepended(step),
                producer,
                semantic_identity,
            },
            Self::UnsupportedLeaf { path, ty } => Self::UnsupportedLeaf {
                path: path.prepended(step),
                ty,
            },
            Self::CyclicGenericSubstitution { path, parameter } => {
                Self::CyclicGenericSubstitution {
                    path: path.prepended(step),
                    parameter,
                }
            }
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

    /// Projects one checked project nominal into the runtime schema authority.
    ///
    /// The data-shape projection above remains the semantic source for the
    /// accepted nominal fields and variants. This adapter only translates that
    /// typed product into the core runtime schema; canonical layout bytes and
    /// the resulting hash remain owned by [`RuntimeTypeSchema`].
    pub fn project_runtime_nominal(
        &self,
        symbols: &ProjectSymbolTable,
        nominal: &CheckedProjectNominal,
    ) -> Result<RuntimeTypeSchema, NominalSchemaProjectionError> {
        self.project_nominal_schema(symbols, nominal)
            .map(|shape| runtime_schema(&shape))
    }

    /// Returns the canonical core layout hash for one accepted project nominal.
    pub fn project_runtime_nominal_layout(
        &self,
        symbols: &ProjectSymbolTable,
        nominal: &CheckedProjectNominal,
    ) -> Result<TypeLayoutHash, NominalSchemaProjectionError> {
        self.project_runtime_nominal(symbols, nominal)?
            .try_layout_hash()
            .map_err(|source| NominalSchemaProjectionError::RuntimeLayout { source })
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
                .enumerate()
                .map(|(ordinal, field)| {
                    self.resolved_shape(field.ty(), &substitutions, stack)
                        .map_err(|error| {
                            error.within_step(NominalSchemaPathStep::Field {
                                ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
                                name: field.name().clone(),
                            })
                        })
                        .map(|shape| {
                            FieldShape::new(field.name().as_str(), field.name().as_str(), shape)
                        })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|fields| TypeShape::record(canonical_nominal_name(declaration.id()), fields)),
            ProjectNominalBody::Enum { variants } => variants
                .iter()
                .enumerate()
                .map(|(ordinal, variant)| {
                    let unit = VariantShape::unit(variant.name().as_str(), variant.name().as_str());
                    let Some(payload) = variant.payload() else {
                        return Ok(unit);
                    };
                    self.resolved_shape(payload, &substitutions, stack)
                        .map_err(|error| {
                            error.within_step(NominalSchemaPathStep::VariantPayload {
                                ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
                                name: variant.name().clone(),
                            })
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

    #[expect(
        clippy::too_many_lines,
        reason = "schema projection exhaustively maps the closed checked type vocabulary"
    )]
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
            TypeKind::Option(inner) => TypeShape::option(
                recurse(inner, generic_stack)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::OptionItem))?,
            ),
            TypeKind::Vec(inner) | TypeKind::Seq(inner) => TypeShape::seq(
                recurse(inner, generic_stack)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::SequenceItem))?,
            ),
            TypeKind::Map {
                kind: MapKind::Ordered | MapKind::Sorted | MapKind::BTree,
                key,
                value,
            } => TypeShape::map(
                recurse(key, generic_stack)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::MapKey))?,
                recurse(value, generic_stack)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::MapValue))?,
            ),
            TypeKind::ProjectNominal(nominal) => {
                let declaration = self.symbols.nominal(nominal.declaration()).ok_or_else(|| {
                    NominalSchemaProjectionError::MissingDeclaration {
                        nominal: nominal.declaration().qualified_name(),
                    }
                })?;
                self.schema_with_stack(declaration, nominal.arguments(), substitutions, stack)
                    .map_err(|error| {
                        error.within_step(NominalSchemaPathStep::NestedNominal {
                            declaration: nominal.declaration().clone(),
                        })
                    })?
            }
            TypeKind::AcceptedNominal(nominal) => {
                return Err(NominalSchemaProjectionError::OpaqueLeaf {
                    path: NominalSchemaPath::default(),
                    producer: nominal.runtime_producer().clone(),
                    semantic_identity: ty.semantic_identity_digest(),
                });
            }
            TypeKind::GenericParam(parameter) => {
                if !generic_stack.insert(parameter.clone()) {
                    return Err(NominalSchemaProjectionError::CyclicGenericSubstitution {
                        path: NominalSchemaPath::default(),
                        parameter: parameter.clone(),
                    });
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
            TypeKind::Result { ok, error } => {
                recurse(ok, generic_stack)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::ResultOk))?;
                recurse(error, generic_stack)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::ResultError))?;
                return Err(NominalSchemaProjectionError::UnsupportedLeaf {
                    path: NominalSchemaPath::default(),
                    ty: Box::new(ty.clone()),
                });
            }
            TypeKind::Tuple(items) => {
                for (ordinal, item) in items.iter().enumerate() {
                    recurse(item, generic_stack).map_err(|error| {
                        error.within_step(NominalSchemaPathStep::TupleItem {
                            ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
                        })
                    })?;
                }
                return Err(NominalSchemaProjectionError::UnsupportedLeaf {
                    path: NominalSchemaPath::default(),
                    ty: Box::new(ty.clone()),
                });
            }
            TypeKind::Array { item, .. } | TypeKind::Slice(item) => {
                recurse(item, generic_stack)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::SequenceItem))?;
                return Err(NominalSchemaProjectionError::UnsupportedLeaf {
                    path: NominalSchemaPath::default(),
                    ty: Box::new(ty.clone()),
                });
            }
            unsupported => {
                return Err(NominalSchemaProjectionError::UnsupportedLeaf {
                    path: NominalSchemaPath::default(),
                    ty: Box::new(unsupported.clone()),
                });
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

fn runtime_schema(shape: &TypeShape) -> RuntimeTypeSchema {
    match shape {
        TypeShape::Unit => RuntimeTypeSchema::Unit,
        TypeShape::Bool => RuntimeTypeSchema::Bool,
        TypeShape::I8 => RuntimeTypeSchema::I8,
        TypeShape::I16 => RuntimeTypeSchema::I16,
        TypeShape::I32 => RuntimeTypeSchema::I32,
        TypeShape::I64 => RuntimeTypeSchema::I64,
        TypeShape::I128 => RuntimeTypeSchema::I128,
        TypeShape::Isize => RuntimeTypeSchema::ISize,
        TypeShape::U8 => RuntimeTypeSchema::U8,
        TypeShape::U16 => RuntimeTypeSchema::U16,
        TypeShape::U32 => RuntimeTypeSchema::U32,
        TypeShape::U64 => RuntimeTypeSchema::U64,
        TypeShape::U128 => RuntimeTypeSchema::U128,
        TypeShape::Usize => RuntimeTypeSchema::USize,
        TypeShape::F32 => RuntimeTypeSchema::F32,
        TypeShape::F64 => RuntimeTypeSchema::F64,
        TypeShape::String => RuntimeTypeSchema::String,
        TypeShape::Char => RuntimeTypeSchema::Char,
        TypeShape::Bytes { format } => RuntimeTypeSchema::Bytes {
            format: runtime_bytes_format(*format),
        },
        TypeShape::Option(inner) => RuntimeTypeSchema::Option(Box::new(runtime_schema(inner))),
        TypeShape::Seq(inner) => RuntimeTypeSchema::Seq(Box::new(runtime_schema(inner))),
        TypeShape::Map { key, value } => RuntimeTypeSchema::Map {
            key: Box::new(runtime_schema(key)),
            value: Box::new(runtime_schema(value)),
        },
        TypeShape::Record {
            name,
            fields,
            policy,
        } => RuntimeTypeSchema::Record {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|field| RuntimeSchemaField {
                    rust_name: field.rust_name.clone(),
                    wire_name: field.wire_name.clone(),
                    schema: runtime_schema(&field.shape),
                    has_default: field.has_default,
                    skip: field.skip,
                    bytes_format: field.bytes_format.map(runtime_bytes_format),
                })
                .collect(),
            deny_unknown_fields: policy.deny_unknown_fields,
        },
        TypeShape::Enum {
            name,
            variants,
            tag,
            repr,
        } => RuntimeTypeSchema::Enum {
            name: name.clone(),
            variants: variants
                .iter()
                .map(|variant| RuntimeSchemaVariant {
                    rust_name: variant.rust_name.clone(),
                    wire_name: variant.wire_name.clone(),
                    payload: variant.payload.as_ref().map(runtime_schema),
                    discriminant: variant.discriminant,
                })
                .collect(),
            tag: match tag {
                arcweft_data::EnumTagStyle::External => RuntimeEnumTagStyle::External,
                arcweft_data::EnumTagStyle::Internal { tag } => {
                    RuntimeEnumTagStyle::Internal { tag: tag.clone() }
                }
                arcweft_data::EnumTagStyle::Adjacent { tag, content } => {
                    RuntimeEnumTagStyle::Adjacent {
                        tag: tag.clone(),
                        content: content.clone(),
                    }
                }
            },
            repr: repr.map(runtime_enum_repr),
        },
        TypeShape::Named(name) => RuntimeTypeSchema::Named(name.clone()),
    }
}

const fn runtime_bytes_format(format: BytesFormat) -> RuntimeBytesFormat {
    match format {
        BytesFormat::Binary => RuntimeBytesFormat::Binary,
        BytesFormat::Base64 => RuntimeBytesFormat::Base64,
        BytesFormat::Hex => RuntimeBytesFormat::Hex,
        BytesFormat::Array => RuntimeBytesFormat::Array,
    }
}

const fn runtime_enum_repr(repr: arcweft_data::EnumRepr) -> RuntimeEnumRepr {
    match repr {
        arcweft_data::EnumRepr::I8 => RuntimeEnumRepr::I8,
        arcweft_data::EnumRepr::I16 => RuntimeEnumRepr::I16,
        arcweft_data::EnumRepr::I32 => RuntimeEnumRepr::I32,
        arcweft_data::EnumRepr::I64 => RuntimeEnumRepr::I64,
        arcweft_data::EnumRepr::I128 => RuntimeEnumRepr::I128,
        arcweft_data::EnumRepr::Isize => RuntimeEnumRepr::ISize,
        arcweft_data::EnumRepr::U8 => RuntimeEnumRepr::U8,
        arcweft_data::EnumRepr::U16 => RuntimeEnumRepr::U16,
        arcweft_data::EnumRepr::U32 => RuntimeEnumRepr::U32,
        arcweft_data::EnumRepr::U64 => RuntimeEnumRepr::U64,
        arcweft_data::EnumRepr::U128 => RuntimeEnumRepr::U128,
        arcweft_data::EnumRepr::Usize => RuntimeEnumRepr::USize,
    }
}
