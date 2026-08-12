use crate::{
    ArcweftRustIdentityError, ArcweftRustManifest, ArcweftRustOpaqueTypeProducerIdError,
    ArcweftRustStructShape, ArcweftRustTypeKind, ArcweftRustTypePath, ArcweftRustTypeRef,
    ArcweftRustVariantPayload,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Bounded validation policy for one Rust ABI manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArcweftRustAbiLimits {
    type_nodes: usize,
    recursive_depth: usize,
    generic_arguments: usize,
    type_parameters: usize,
}

impl ArcweftRustAbiLimits {
    /// Production limits shared with accepted nominal projection.
    pub const PRODUCTION: Self = Self {
        type_nodes: 4_096,
        recursive_depth: 256,
        generic_arguments: 256,
        type_parameters: 256,
    };

    /// Constructs a bounded policy for focused tests and adapters.
    pub const fn new(
        type_nodes: usize,
        recursive_depth: usize,
        generic_arguments: usize,
        type_parameters: usize,
    ) -> Self {
        Self {
            type_nodes,
            recursive_depth,
            generic_arguments,
            type_parameters,
        }
    }

    pub const fn type_nodes(self) -> usize {
        self.type_nodes
    }

    pub const fn recursive_depth(self) -> usize {
        self.recursive_depth
    }

    pub const fn generic_arguments(self) -> usize {
        self.generic_arguments
    }

    pub const fn type_parameters(self) -> usize {
        self.type_parameters
    }
}

/// The declaration member that owns a validated type-reference tree.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ArcweftRustTypeSiteRoot {
    StructTupleField {
        declaration: usize,
        field: usize,
    },
    StructRecordField {
        declaration: usize,
        field: usize,
    },
    EnumTupleField {
        declaration: usize,
        variant: usize,
        field: usize,
    },
    EnumRecordField {
        declaration: usize,
        variant: usize,
        field: usize,
    },
    NewtypeInner {
        declaration: usize,
    },
    FunctionParameter {
        function: usize,
        parameter: usize,
    },
    FunctionResult {
        function: usize,
    },
}

/// One recursive step from a type-reference root to the failing node.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ArcweftRustTypeSiteStep {
    VecItem,
    SeqItem,
    OptionItem,
    ResultOk,
    ResultError,
    TupleItem(usize),
    NominalArgument(usize),
}

/// Exact location of a node inside the typed Rust ABI model.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArcweftRustTypeSite {
    root: ArcweftRustTypeSiteRoot,
    steps: Box<[ArcweftRustTypeSiteStep]>,
}

impl ArcweftRustTypeSite {
    pub const fn root(&self) -> &ArcweftRustTypeSiteRoot {
        &self.root
    }

    pub fn steps(&self) -> &[ArcweftRustTypeSiteStep] {
        &self.steps
    }
}

/// A structured violation of the final Rust ABI manifest model.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ArcweftRustManifestError {
    #[error("unsupported Rust ABI schema {found}; expected {expected}")]
    UnsupportedSchema { found: u32, expected: u32 },
    #[error("Rust type declaration {declaration} has invalid opaque producer: {error}")]
    InvalidOpaqueProducer {
        declaration: usize,
        error: ArcweftRustOpaqueTypeProducerIdError,
    },
    #[error("invalid Rust package identity: {error}")]
    InvalidPackage { error: ArcweftRustIdentityError },
    #[error("invalid path for Rust type declaration {declaration}: {error}")]
    InvalidTypePath {
        declaration: usize,
        error: ArcweftRustIdentityError,
    },
    #[error("Rust type path {path:?} is declared at both {first} and {duplicate}")]
    DuplicateTypePath {
        path: ArcweftRustTypePath,
        first: usize,
        duplicate: usize,
    },
    #[error("Rust type declaration {declaration} has {observed} parameters, exceeding {maximum}")]
    TypeParameterLimit {
        declaration: usize,
        observed: usize,
        maximum: usize,
    },
    #[error("Rust type declaration {declaration} parameter {parameter} has invalid name: {error}")]
    InvalidTypeParameterName {
        declaration: usize,
        parameter: usize,
        error: ArcweftRustIdentityError,
    },
    #[error("Rust type declaration {declaration} repeats type parameter `{name}`")]
    DuplicateTypeParameterName { declaration: usize, name: String },
    #[error(
        "Rust type declaration {declaration} parameter {parameter} has index {actual}, expected {expected}"
    )]
    NonContiguousTypeParameterIndex {
        declaration: usize,
        parameter: usize,
        expected: usize,
        actual: usize,
    },
    #[error("invalid nominal identity at {site:?}: {error}")]
    InvalidNominalIdentity {
        site: ArcweftRustTypeSite,
        error: ArcweftRustIdentityError,
    },
    #[error("type graph at {site:?} has {observed} nodes, exceeding {maximum}")]
    TypeNodeLimit {
        site: ArcweftRustTypeSite,
        observed: usize,
        maximum: usize,
    },
    #[error("type graph at {site:?} reaches depth {observed}, exceeding {maximum}")]
    RecursiveDepthLimit {
        site: ArcweftRustTypeSite,
        observed: usize,
        maximum: usize,
    },
    #[error("type application at {site:?} has {observed} arguments, exceeding {maximum}")]
    GenericArgumentLimit {
        site: ArcweftRustTypeSite,
        observed: usize,
        maximum: usize,
    },
    #[error("type parameter {index} at {site:?} is not declared by its Rust ADT")]
    UnboundTypeParameter {
        site: ArcweftRustTypeSite,
        index: usize,
    },
    #[error("callable type at {site:?} contains free type parameter {index}")]
    FreeTypeParameterInCallable {
        site: ArcweftRustTypeSite,
        index: usize,
    },
}

impl ArcweftRustManifest {
    /// Validates the entire manifest and every recursive type graph.
    pub fn validate(&self, limits: ArcweftRustAbiLimits) -> Result<(), ArcweftRustManifestError> {
        if self.schema_version != crate::ARCWEFT_RUST_ABI_SCHEMA_VERSION {
            return Err(ArcweftRustManifestError::UnsupportedSchema {
                found: self.schema_version,
                expected: crate::ARCWEFT_RUST_ABI_SCHEMA_VERSION,
            });
        }
        for (declaration, type_declaration) in self.types.iter().enumerate() {
            type_declaration.opaque_producer().as_str();
            crate::ArcweftRustOpaqueTypeProducerId::validate(
                type_declaration.opaque_producer().as_str(),
            )
            .map_err(|error| ArcweftRustManifestError::InvalidOpaqueProducer {
                declaration,
                error,
            })?;
        }
        self.package
            .id
            .validate()
            .map_err(|error| ArcweftRustManifestError::InvalidPackage { error })?;

        let mut paths = BTreeMap::<ArcweftRustTypePath, usize>::new();
        for (declaration_index, declaration) in self.types.iter().enumerate() {
            declaration.path.validate().map_err(|error| {
                ArcweftRustManifestError::InvalidTypePath {
                    declaration: declaration_index,
                    error,
                }
            })?;
            if let Some(first) = paths.insert(declaration.path.clone(), declaration_index) {
                return Err(ArcweftRustManifestError::DuplicateTypePath {
                    path: declaration.path.clone(),
                    first,
                    duplicate: declaration_index,
                });
            }
            validate_parameters(declaration_index, declaration, limits)?;
            let parameter_count = declaration.parameters.len();
            for (root, ty) in declaration_type_roots(declaration_index, &declaration.kind) {
                validate_type_tree(ty, root, Some(parameter_count), limits)?;
            }
        }

        for (function_index, function) in self.functions.iter().enumerate() {
            for (parameter_index, parameter) in function.params.iter().enumerate() {
                validate_type_tree(
                    &parameter.ty,
                    ArcweftRustTypeSiteRoot::FunctionParameter {
                        function: function_index,
                        parameter: parameter_index,
                    },
                    None,
                    limits,
                )?;
            }
            validate_type_tree(
                &function.return_type,
                ArcweftRustTypeSiteRoot::FunctionResult {
                    function: function_index,
                },
                None,
                limits,
            )?;
        }
        Ok(())
    }
}

fn validate_parameters(
    declaration_index: usize,
    declaration: &crate::ArcweftRustTypeDecl,
    limits: ArcweftRustAbiLimits,
) -> Result<(), ArcweftRustManifestError> {
    if declaration.parameters.len() > limits.type_parameters {
        return Err(ArcweftRustManifestError::TypeParameterLimit {
            declaration: declaration_index,
            observed: declaration.parameters.len(),
            maximum: limits.type_parameters,
        });
    }
    let mut names = BTreeSet::new();
    for (parameter_index, parameter) in declaration.parameters.iter().enumerate() {
        parameter.name.validate().map_err(|error| {
            ArcweftRustManifestError::InvalidTypeParameterName {
                declaration: declaration_index,
                parameter: parameter_index,
                error,
            }
        })?;
        if !names.insert(parameter.name.as_str()) {
            return Err(ArcweftRustManifestError::DuplicateTypeParameterName {
                declaration: declaration_index,
                name: parameter.name.as_str().to_owned(),
            });
        }
        if parameter.index.get() != parameter_index {
            return Err(ArcweftRustManifestError::NonContiguousTypeParameterIndex {
                declaration: declaration_index,
                parameter: parameter_index,
                expected: parameter_index,
                actual: parameter.index.get(),
            });
        }
    }
    Ok(())
}

fn declaration_type_roots(
    declaration: usize,
    kind: &ArcweftRustTypeKind,
) -> Vec<(ArcweftRustTypeSiteRoot, &ArcweftRustTypeRef)> {
    match kind {
        ArcweftRustTypeKind::Struct { shape } => match shape {
            ArcweftRustStructShape::Unit => Vec::new(),
            ArcweftRustStructShape::Tuple { fields } => fields
                .iter()
                .enumerate()
                .map(|(field, ty)| {
                    (
                        ArcweftRustTypeSiteRoot::StructTupleField { declaration, field },
                        ty,
                    )
                })
                .collect(),
            ArcweftRustStructShape::Record { fields } => fields
                .iter()
                .enumerate()
                .map(|(field, value)| {
                    (
                        ArcweftRustTypeSiteRoot::StructRecordField { declaration, field },
                        &value.ty,
                    )
                })
                .collect(),
        },
        ArcweftRustTypeKind::Enum { variants } => variants
            .iter()
            .enumerate()
            .flat_map(|(variant, value)| match &value.payload {
                ArcweftRustVariantPayload::Unit => Vec::new(),
                ArcweftRustVariantPayload::Tuple { fields } => fields
                    .iter()
                    .enumerate()
                    .map(|(field, ty)| {
                        (
                            ArcweftRustTypeSiteRoot::EnumTupleField {
                                declaration,
                                variant,
                                field,
                            },
                            ty,
                        )
                    })
                    .collect(),
                ArcweftRustVariantPayload::Record { fields } => fields
                    .iter()
                    .enumerate()
                    .map(|(field, value)| {
                        (
                            ArcweftRustTypeSiteRoot::EnumRecordField {
                                declaration,
                                variant,
                                field,
                            },
                            &value.ty,
                        )
                    })
                    .collect(),
            })
            .collect(),
        ArcweftRustTypeKind::Newtype { inner } => {
            vec![(ArcweftRustTypeSiteRoot::NewtypeInner { declaration }, inner)]
        }
    }
}

fn validate_type_tree(
    root_type: &ArcweftRustTypeRef,
    root: ArcweftRustTypeSiteRoot,
    parameter_count: Option<usize>,
    limits: ArcweftRustAbiLimits,
) -> Result<(), ArcweftRustManifestError> {
    TypeTreeValidator::new(root_type, root, parameter_count, limits).run()
}

struct TypeFrame<'a> {
    ty: &'a ArcweftRustTypeRef,
    depth: usize,
    steps: Vec<ArcweftRustTypeSiteStep>,
}

struct TypeTreeValidator<'a> {
    root: ArcweftRustTypeSiteRoot,
    parameter_count: Option<usize>,
    limits: ArcweftRustAbiLimits,
    stack: Vec<TypeFrame<'a>>,
    visited: usize,
}

impl<'a> TypeTreeValidator<'a> {
    fn new(
        root_type: &'a ArcweftRustTypeRef,
        root: ArcweftRustTypeSiteRoot,
        parameter_count: Option<usize>,
        limits: ArcweftRustAbiLimits,
    ) -> Self {
        Self {
            root,
            parameter_count,
            limits,
            stack: vec![TypeFrame {
                ty: root_type,
                depth: 1,
                steps: Vec::new(),
            }],
            visited: 0,
        }
    }

    fn run(mut self) -> Result<(), ArcweftRustManifestError> {
        while let Some(frame) = self.stack.pop() {
            self.visited = self.visited.saturating_add(1);
            self.validate_limits(&frame)?;
            self.visit(&frame)?;
        }
        Ok(())
    }

    fn validate_limits(&self, frame: &TypeFrame<'_>) -> Result<(), ArcweftRustManifestError> {
        if self.visited > self.limits.type_nodes {
            return Err(ArcweftRustManifestError::TypeNodeLimit {
                site: self.site(&frame.steps),
                observed: self.visited,
                maximum: self.limits.type_nodes,
            });
        }
        if frame.depth > self.limits.recursive_depth {
            return Err(ArcweftRustManifestError::RecursiveDepthLimit {
                site: self.site(&frame.steps),
                observed: frame.depth,
                maximum: self.limits.recursive_depth,
            });
        }
        Ok(())
    }

    fn visit(&mut self, frame: &TypeFrame<'a>) -> Result<(), ArcweftRustManifestError> {
        match frame.ty {
            ArcweftRustTypeRef::Vec { item } => {
                self.push(item, frame, ArcweftRustTypeSiteStep::VecItem);
            }
            ArcweftRustTypeRef::Seq { item } => {
                self.push(item, frame, ArcweftRustTypeSiteStep::SeqItem);
            }
            ArcweftRustTypeRef::Option { item } => {
                self.push(item, frame, ArcweftRustTypeSiteStep::OptionItem);
            }
            ArcweftRustTypeRef::Result { ok, error } => {
                self.push(error, frame, ArcweftRustTypeSiteStep::ResultError);
                self.push(ok, frame, ArcweftRustTypeSiteStep::ResultOk);
            }
            ArcweftRustTypeRef::Tuple { items } => {
                self.push_sequence(items, frame, ArcweftRustTypeSiteStep::TupleItem)?;
            }
            ArcweftRustTypeRef::Nominal {
                package,
                path,
                arguments,
            } => {
                self.validate_nominal_identity(package.validate(), &frame.steps)?;
                self.validate_nominal_identity(path.validate(), &frame.steps)?;
                self.push_sequence(arguments, frame, ArcweftRustTypeSiteStep::NominalArgument)?;
            }
            ArcweftRustTypeRef::TypeParameter { index } => {
                self.validate_type_parameter(index.get(), &frame.steps)?;
            }
            ArcweftRustTypeRef::Unit
            | ArcweftRustTypeRef::Bool
            | ArcweftRustTypeRef::I8
            | ArcweftRustTypeRef::I16
            | ArcweftRustTypeRef::I32
            | ArcweftRustTypeRef::I64
            | ArcweftRustTypeRef::I128
            | ArcweftRustTypeRef::ISize
            | ArcweftRustTypeRef::U8
            | ArcweftRustTypeRef::U16
            | ArcweftRustTypeRef::U32
            | ArcweftRustTypeRef::U64
            | ArcweftRustTypeRef::U128
            | ArcweftRustTypeRef::USize
            | ArcweftRustTypeRef::F32
            | ArcweftRustTypeRef::F64
            | ArcweftRustTypeRef::String
            | ArcweftRustTypeRef::Char => {}
        }
        Ok(())
    }

    fn push(
        &mut self,
        child: &'a ArcweftRustTypeRef,
        parent: &TypeFrame<'_>,
        step: ArcweftRustTypeSiteStep,
    ) {
        let mut steps = parent.steps.clone();
        steps.push(step);
        self.stack.push(TypeFrame {
            ty: child,
            depth: parent.depth.saturating_add(1),
            steps,
        });
    }

    fn push_sequence(
        &mut self,
        children: &'a [ArcweftRustTypeRef],
        parent: &TypeFrame<'_>,
        step: impl Fn(usize) -> ArcweftRustTypeSiteStep,
    ) -> Result<(), ArcweftRustManifestError> {
        self.ensure_argument_limit(children.len(), &parent.steps)?;
        for (index, child) in children.iter().enumerate().rev() {
            self.push(child, parent, step(index));
        }
        Ok(())
    }

    fn validate_nominal_identity(
        &self,
        result: Result<(), ArcweftRustIdentityError>,
        steps: &[ArcweftRustTypeSiteStep],
    ) -> Result<(), ArcweftRustManifestError> {
        result.map_err(|error| ArcweftRustManifestError::InvalidNominalIdentity {
            site: self.site(steps),
            error,
        })
    }

    fn validate_type_parameter(
        &self,
        index: usize,
        steps: &[ArcweftRustTypeSiteStep],
    ) -> Result<(), ArcweftRustManifestError> {
        match self.parameter_count {
            Some(count) if index < count => Ok(()),
            Some(_) => Err(ArcweftRustManifestError::UnboundTypeParameter {
                site: self.site(steps),
                index,
            }),
            None => Err(ArcweftRustManifestError::FreeTypeParameterInCallable {
                site: self.site(steps),
                index,
            }),
        }
    }

    fn ensure_argument_limit(
        &self,
        observed: usize,
        steps: &[ArcweftRustTypeSiteStep],
    ) -> Result<(), ArcweftRustManifestError> {
        if observed > self.limits.generic_arguments {
            return Err(ArcweftRustManifestError::GenericArgumentLimit {
                site: self.site(steps),
                observed,
                maximum: self.limits.generic_arguments,
            });
        }
        Ok(())
    }

    fn site(&self, steps: &[ArcweftRustTypeSiteStep]) -> ArcweftRustTypeSite {
        ArcweftRustTypeSite {
            root: self.root,
            steps: steps.to_vec().into_boxed_slice(),
        }
    }
}
