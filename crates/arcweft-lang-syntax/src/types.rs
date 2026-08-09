use thiserror::Error;

use crate::ast::{common::TextRange, module_path::ModuleSegment};
use crate::reference::ReferenceType;

mod source;
mod token;

pub use self::source::{
    AuthoredTypeRef, TypePath, TypeRecoveryId, TypeRefAssociatedBindingPart, TypeRefComponentRole,
    TypeRefComponentSource, TypeRefHeadKind, TypeRefHeadSource, TypeRefLexemeKind,
    TypeRefLexemeSource, TypeRefNodePath, TypeRefNodeSource, TypeRefNodeStep, TypeRefRegionPart,
    TypeRefSourceMap, TypeRefSourceMapError,
};
pub(crate) use self::token::{TypeToken, TypeTokenKind, parse_tokens};

/// Lifetime name used in Arcweft type syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifetimeName {
    name: String,
    range: TextRange,
}

/// Type syntax preserved for later borrow and suspension-boundary checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeRef {
    Never,
    ConstInt(usize),
    Path(TypePath),
    Tuple(Vec<TypeRef>),
    Function {
        params: Vec<TypeRef>,
        return_type: Box<TypeRef>,
        effects: Option<TypeEffectRow>,
    },
    Choice(Vec<TypeRef>),
    Generic {
        base: TypePath,
        args: Vec<TypeRef>,
    },
    TraitBound(TraitBound),
    Projection {
        subject: Box<TypeRef>,
        assoc: ModuleSegment,
    },
    Reference(ReferenceType),
    Slice(Box<TypeRef>),
    Recovery(TypeRecoveryId),
}

/// Closed effect row attached to a function type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeEffectRow {
    effects: Vec<String>,
}

/// Function parameter arity role shared with semantic callable metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FnParamKind {
    /// A normal fixed parameter.
    Fixed,
    /// A positional rest parameter declared as `name: ...T`.
    Rest,
}

/// Associated type equality inside a trait bound, such as `Iterator<Item = T>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssociatedTypeBinding {
    name: ModuleSegment,
    value: TypeRef,
}

/// Trait bound syntax preserving associated type equality constraints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitBound {
    path: TypePath,
    args: Vec<TypeRef>,
    associated: Vec<AssociatedTypeBinding>,
}

/// Type syntax parse failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeParseError {
    code: &'static str,
    range: Option<TextRange>,
    message: String,
}

struct ParsedTypeRef {
    value: TypeRef,
    nodes: Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
    lexemes: Vec<TypeRefLexemeSource<TextRange>>,
}

impl ParsedTypeRef {
    fn node(
        value: TypeRef,
        path: &TypeRefNodePath,
        whole: TextRange,
        head: Option<TypeRefHeadSource<TextRange>>,
        lexemes: Vec<TypeRefLexemeSource<TextRange>>,
    ) -> Self {
        Self {
            value,
            nodes: vec![(path.clone(), TypeRefNodeSource::new(whole, head))],
            lexemes,
        }
    }

    fn replace_node_whole(&mut self, path: &TypeRefNodePath, whole: TextRange) {
        let (_, source) = self
            .nodes
            .iter_mut()
            .find(|(candidate, _)| candidate == path)
            .expect("parsed wrapper retains its structural root");
        source.replace_whole(whole);
    }
}

#[cfg(test)]
pub(crate) fn parse_attached_type_for_test(
    source: &str,
) -> Result<AuthoredTypeRef, TypeParseError> {
    use std::sync::Arc;

    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

    use crate::incremental::SyntaxDatabase;
    use crate::parser::{ParseCompletion, ParseOptions, parse_type_fragment};

    let fragment = parse_type_fragment(source, ParseOptions::default());
    if fragment.completion() != &ParseCompletion::Complete {
        if let Some(diagnostic) = fragment.diagnostics().first() {
            return Err(TypeParseError {
                code: diagnostic.code(),
                range: Some(TextRange::new(
                    diagnostic.range().start(),
                    diagnostic.range().end(),
                )),
                message: diagnostic.message().to_owned(),
            });
        }
        return Err(TypeParseError::new(
            "attached type fragment did not complete",
        ));
    }

    let name = SourceName::path("syntax-attached-type-test.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://syntax/attached-type")
                .expect("test document ID"),
            name.clone(),
            source,
        )
        .expect("test type source document"),
    );
    let span = document
        .span(SourceRange::new(0, source.len()))
        .expect("whole type source span");
    let attached = SyntaxDatabase::try_new()
        .expect("test syntax database")
        .attach_fragment(SourceSnapshotId::initial(name), document, span, fragment)
        .map_err(|error| TypeParseError::new_owned(error.to_string()))?;
    let semantic = attached
        .root()
        .semantic()
        .map_err(|error| TypeParseError::new_owned(error.to_string()))?;
    Ok(semantic.authored_for_test().clone())
}

const MAX_TYPE_GENERIC_ARGUMENTS: usize = 256;
const MAX_TYPE_NODES: usize = 4_096;

fn validate_type_ref_limits(root: &TypeRef) -> Result<(), TypeParseError> {
    let mut pending = vec![root];
    let mut nodes = 0usize;
    while let Some(ty) = pending.pop() {
        nodes = nodes
            .checked_add(1)
            .ok_or_else(|| TypeParseError::resource_limit("type node count overflow"))?;
        if nodes > MAX_TYPE_NODES {
            return Err(TypeParseError::node_limit(
                "type exceeds the 4096 node limit",
            ));
        }
        match ty {
            TypeRef::Tuple(items) | TypeRef::Choice(items) => pending.extend(items.iter().rev()),
            TypeRef::Function {
                params,
                return_type,
                ..
            } => {
                pending.push(return_type);
                pending.extend(params.iter().rev());
            }
            TypeRef::Generic { args, .. } => {
                if args.len() > MAX_TYPE_GENERIC_ARGUMENTS {
                    return Err(TypeParseError::generic_argument_limit(
                        "type constructor exceeds the 256 argument limit",
                    ));
                }
                pending.extend(args.iter().rev());
            }
            TypeRef::TraitBound(bound) => {
                let argument_count = bound
                    .args
                    .len()
                    .checked_add(bound.associated.len())
                    .ok_or_else(|| {
                        TypeParseError::resource_limit("trait argument count overflow")
                    })?;
                if argument_count > MAX_TYPE_GENERIC_ARGUMENTS {
                    return Err(TypeParseError::generic_argument_limit(
                        "trait bound exceeds the 256 argument limit",
                    ));
                }
                pending.extend(bound.associated.iter().rev().map(|binding| &binding.value));
                pending.extend(bound.args.iter().rev());
            }
            TypeRef::Projection { subject, .. } | TypeRef::Slice(subject) => {
                pending.push(subject);
            }
            TypeRef::Reference(reference) => pending.push(reference.referent()),
            TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) | TypeRef::Recovery(_) => {}
        }
    }
    Ok(())
}

impl TypeRef {
    /// Deterministic Arcweft spelling used by typed semantic identities.
    pub fn canonical_label(&self) -> String {
        type_ref_parse_label(self)
    }

    /// Nominal path authored at this type's head, when this node has one.
    ///
    /// This is the structural resolver input for path, generic-constructor,
    /// and trait-bound nodes. It never reconstructs a display label.
    pub const fn nominal_path(&self) -> Option<&TypePath> {
        match self {
            Self::Path(path) | Self::Generic { base: path, .. } => Some(path),
            Self::TraitBound(bound) => Some(&bound.path),
            Self::Never
            | Self::ConstInt(_)
            | Self::Tuple(_)
            | Self::Function { .. }
            | Self::Choice(_)
            | Self::Projection { .. }
            | Self::Reference(_)
            | Self::Slice(_)
            | Self::Recovery(_) => None,
        }
    }

    pub(crate) fn rebase_reference_ranges(&mut self, base: usize) {
        match self {
            Self::Tuple(items) | Self::Choice(items) => {
                for item in items {
                    item.rebase_reference_ranges(base);
                }
            }
            Self::Function {
                params,
                return_type,
                ..
            } => {
                for param in params {
                    param.rebase_reference_ranges(base);
                }
                return_type.rebase_reference_ranges(base);
            }
            Self::Generic { args, .. } => {
                for arg in args {
                    arg.rebase_reference_ranges(base);
                }
            }
            Self::TraitBound(bound) => {
                for arg in &mut bound.args {
                    arg.rebase_reference_ranges(base);
                }
                for binding in &mut bound.associated {
                    binding.value.rebase_reference_ranges(base);
                }
            }
            Self::Projection { subject, .. } | Self::Slice(subject) => {
                subject.rebase_reference_ranges(base);
            }
            Self::Reference(reference) => reference.rebase(base),
            Self::Never | Self::ConstInt(_) | Self::Path(_) | Self::Recovery(_) => {}
        }
    }
}

fn type_ref_parse_label(ty: &TypeRef) -> String {
    type_ref_label_in(ty, TypeLabelContext::TopLevel)
}

#[derive(Clone, Copy)]
enum TypeLabelContext {
    TopLevel,
    FunctionParameter,
    FunctionReturn,
    ChoiceAlternative,
    ReferenceReferent,
    ProjectionSubject,
    Delimited,
}

fn type_ref_label_in(ty: &TypeRef, context: TypeLabelContext) -> String {
    let label = type_ref_unparenthesized_label(ty);
    if type_ref_label_needs_parentheses(ty, context) {
        format!("({label})")
    } else {
        label
    }
}

fn type_ref_label_needs_parentheses(ty: &TypeRef, context: TypeLabelContext) -> bool {
    match context {
        TypeLabelContext::TopLevel | TypeLabelContext::Delimited => false,
        TypeLabelContext::FunctionParameter => matches!(ty, TypeRef::Function { .. }),
        TypeLabelContext::FunctionReturn => {
            matches!(
                ty,
                TypeRef::Function {
                    effects: Some(_),
                    ..
                }
            )
        }
        TypeLabelContext::ChoiceAlternative
        | TypeLabelContext::ReferenceReferent
        | TypeLabelContext::ProjectionSubject => {
            matches!(ty, TypeRef::Function { .. } | TypeRef::Choice(_))
        }
    }
}

fn type_ref_unparenthesized_label(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Never => "Never".to_owned(),
        TypeRef::ConstInt(value) => value.to_string(),
        TypeRef::Path(path) => path.canonical_string(),
        TypeRef::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(|item| type_ref_label_in(item, TypeLabelContext::Delimited))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::Function {
            params,
            return_type,
            effects,
        } => {
            let params = if params.len() == 1 {
                type_ref_label_in(&params[0], TypeLabelContext::FunctionParameter)
            } else {
                format!(
                    "({})",
                    params
                        .iter()
                        .map(|param| type_ref_label_in(param, TypeLabelContext::Delimited))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let label = format!(
                "{params} -> {}",
                type_ref_label_in(return_type, TypeLabelContext::FunctionReturn)
            );
            type_effect_row_label(effects.as_ref()).map_or(label.clone(), |effects| {
                format!("{label} effects {effects}")
            })
        }
        TypeRef::Choice(alternatives) => alternatives
            .iter()
            .map(|alternative| type_ref_label_in(alternative, TypeLabelContext::ChoiceAlternative))
            .collect::<Vec<_>>()
            .join(" | "),
        TypeRef::Generic { base, args } => format!(
            "{base}<{}>",
            args.iter()
                .map(|arg| type_ref_label_in(arg, TypeLabelContext::Delimited))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::TraitBound(bound) => {
            let mut args = bound
                .args
                .iter()
                .map(|arg| type_ref_label_in(arg, TypeLabelContext::Delimited))
                .collect::<Vec<_>>();
            args.extend(bound.associated.iter().map(|binding| {
                format!(
                    "{} = {}",
                    binding.name,
                    type_ref_label_in(&binding.value, TypeLabelContext::Delimited)
                )
            }));
            format!("{}<{}>", bound.path, args.join(", "))
        }
        TypeRef::Projection { subject, assoc } => {
            format!(
                "{}::{assoc}",
                type_ref_label_in(subject, TypeLabelContext::ProjectionSubject)
            )
        }
        TypeRef::Reference(reference) => {
            let lifetime = reference
                .region()
                .name()
                .map(|lifetime| format!("'{} ", lifetime.name()))
                .unwrap_or_default();
            format!(
                "&{lifetime}{}{}",
                reference.kind().source_qualifier(),
                type_ref_label_in(reference.referent(), TypeLabelContext::ReferenceReferent)
            )
        }
        TypeRef::Slice(inner) => format!(
            "[{}]",
            type_ref_label_in(inner, TypeLabelContext::Delimited)
        ),
        TypeRef::Recovery(id) => format!("<recovered-type:{}>", id.index()),
    }
}

fn type_effect_row_label(effects: Option<&TypeEffectRow>) -> Option<String> {
    effects.map(|effects| {
        if effects.effects().is_empty() {
            "{ }".to_owned()
        } else {
            format!("{{ {} }}", effects.effects().join(", "))
        }
    })
}

fn parse_lifetime_name(source: &str, range: TextRange) -> LifetimeName {
    LifetimeName {
        name: source.trim_start_matches('\'').to_owned(),
        range,
    }
}

impl LifetimeName {
    /// Lifetime name without the leading apostrophe.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Exact source range including the leading apostrophe.
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl TypeEffectRow {
    fn new(effects: Vec<String>) -> Self {
        Self { effects }
    }

    /// Source labels declared in this closed effect row.
    pub fn effects(&self) -> &[String] {
        &self.effects
    }
}

impl AssociatedTypeBinding {
    /// Associated type name constrained by this binding.
    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }

    /// Required associated type value.
    pub const fn value(&self) -> &TypeRef {
        &self.value
    }
}

impl TraitBound {
    /// Trait path used by this bound.
    pub const fn path(&self) -> &TypePath {
        &self.path
    }

    /// Positional type arguments supplied to the trait.
    pub fn args(&self) -> &[TypeRef] {
        &self.args
    }

    /// Associated type equalities supplied to the trait.
    pub fn associated(&self) -> &[AssociatedTypeBinding] {
        &self.associated
    }
}

impl TypeParseError {
    fn new(message: &str) -> Self {
        Self {
            code: "syntax.type.invalid",
            range: None,
            message: message.to_owned(),
        }
    }

    fn new_owned(message: String) -> Self {
        Self {
            code: "syntax.type.invalid",
            range: None,
            message,
        }
    }

    fn resource_limit(message: &str) -> Self {
        Self::without_range("syntax.type.resource_limit", message)
    }

    fn node_limit(message: &str) -> Self {
        Self::without_range("syntax.type.node_limit", message)
    }

    fn generic_argument_limit(message: &str) -> Self {
        Self::without_range("syntax.type.generic_argument_limit", message)
    }

    fn without_range(code: &'static str, message: &str) -> Self {
        Self {
            code,
            range: None,
            message: message.to_owned(),
        }
    }

    pub(super) fn at(code: &'static str, message: &str, range: TextRange) -> Self {
        Self {
            code,
            range: Some(range),
            message: message.to_owned(),
        }
    }

    /// Stable parser diagnostic code.
    pub const fn code(&self) -> &'static str {
        self.code
    }

    /// Type-fragment-relative error range, when exact.
    pub const fn range(&self) -> Option<TextRange> {
        self.range
    }
}
