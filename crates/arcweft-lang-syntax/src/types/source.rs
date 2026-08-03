//! Typed type paths and exact source evidence for authored type references.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::ast::{
    common::TextRange,
    module_path::ModulePathRoot,
    symbol_path::{ProjectSymbolPath, ProjectSymbolPathError, ProjectSymbolSegment},
};

use super::TypeRef;

/// A validated project-symbol path used in type position.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypePath(ProjectSymbolPath);

impl From<ProjectSymbolPath> for TypePath {
    /// Promotes an already validated project-symbol path into type position.
    ///
    /// This conversion does not parse presentation text or weaken path
    /// validation. It lets HIR and semantic environment owners publish typed
    /// paths that were constructed through the same `ProjectSymbolPath`
    /// invariants as parser-produced type paths.
    fn from(path: ProjectSymbolPath) -> Self {
        Self(path)
    }
}

/// Parser-owned identity for one recovered type node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeRecoveryId(u32);

/// Structural address of one node inside an authored type reference.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeRefNodePath(Box<[TypeRefNodeStep]>);

/// One edge in a structural type-node address.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeRefNodeStep {
    TupleItem(u16),
    FunctionParameter(u16),
    FunctionReturn,
    ChoiceAlternative(u16),
    GenericArgument(u16),
    TraitArgument(u16),
    AssociatedBinding(u16),
    ProjectionSubject,
    ReferenceReferent,
    SliceItem,
}

/// Diagnostic head represented by one type node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeRefHeadKind {
    Never,
    ConstInt,
    Path,
    Constructor,
    Trait,
    ProjectionMember,
    Recovery,
}

/// Exact source of a node's diagnostic head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefHeadSource<R> {
    kind: TypeRefHeadKind,
    range: R,
    terminal: Option<R>,
}

/// Exact whole-node source and its optional diagnostic head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefNodeSource<R> {
    whole: R,
    head: Option<TypeRefHeadSource<R>>,
}

/// One typed lexical token owned by a structural type node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeRefLexemeKind {
    PathRoot,
    PathSegment { ordinal: u16 },
    PathSeparator { before: u16 },
    TurbofishSeparator,
    OpenAngle,
    ArgumentSeparator { before: u16 },
    TrailingArgumentSeparator,
    CloseAngle,
}

/// Source component of one associated-type binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeRefAssociatedBindingPart {
    Whole,
    Name,
    Equals,
    Value,
}

/// Source component of one named or elided reference region.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeRefRegionPart {
    Whole,
    NamedApostrophe,
    NamedName,
    ElisionInsertion,
}

/// Typed source component exposed by an attached semantic type node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeRefComponentRole {
    Whole,
    NeverMarker,
    ConstInteger,
    PathRoot,
    PathSegment {
        ordinal: u32,
    },
    TupleOpen,
    TupleElement {
        ordinal: u32,
    },
    TupleSeparator {
        ordinal: u32,
    },
    TupleClose,
    FunctionOpen,
    FunctionParameter {
        ordinal: u32,
    },
    FunctionSeparator {
        ordinal: u32,
    },
    FunctionClose,
    FunctionArrow,
    FunctionReturn,
    FunctionEffectOpen,
    FunctionEffect {
        ordinal: u32,
    },
    FunctionEffectClose,
    ChoiceAlternative {
        ordinal: u32,
    },
    ChoiceSeparator {
        ordinal: u32,
    },
    GenericBase,
    GenericOpen,
    GenericArgument {
        ordinal: u32,
    },
    GenericSeparator {
        ordinal: u32,
    },
    GenericClose,
    TraitBase,
    TraitOpen,
    TraitArgument {
        ordinal: u32,
    },
    TraitSeparator {
        ordinal: u32,
    },
    AssociatedBinding {
        ordinal: u32,
        part: TypeRefAssociatedBindingPart,
    },
    TraitClose,
    ProjectionSubject,
    ProjectionSeparator,
    ProjectionName,
    ReferenceAmpersand,
    Region(TypeRefRegionPart),
    ReferenceMutKeyword,
    ReferenceReferent,
    SliceOpen,
    SliceElement,
    SliceClose,
    Recovery,
}

/// Exact source of one semantic type component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefComponentSource<R> {
    owner: TypeRefNodePath,
    role: TypeRefComponentRole,
    range: R,
}

/// Exact source of one typed lexical token in a type reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefLexemeSource<R> {
    owner: TypeRefNodePath,
    kind: TypeRefLexemeKind,
    range: R,
}

/// One-to-one source map for every structural node in a type reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefSourceMap<R> {
    nodes: Box<[(TypeRefNodePath, TypeRefNodeSource<R>)]>,
    lexemes: Box<[TypeRefLexemeSource<R>]>,
    components: Box<[TypeRefComponentSource<R>]>,
}

/// Parsed type structure coupled to its exact syntax source map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredTypeRef {
    value: TypeRef,
    source: Box<TypeRefSourceMap<TextRange>>,
}

/// One path parsed from the active type-grammar transaction together with its
/// exact token evidence.
pub(super) struct ParsedTypePath {
    pub(super) value: TypePath,
    pub(super) head: TypeRefHeadSource<TextRange>,
    pub(super) lexemes: Vec<TypeRefLexemeSource<TextRange>>,
}

/// One identifier token admitted by the type-path grammar.
#[derive(Clone, Copy)]
pub(super) struct TypePathComponent<'a> {
    pub(super) spelling: &'a str,
    pub(super) range: TextRange,
}

/// Invalid relationship between a type tree and its source map.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TypeRefSourceMapError {
    MissingRoot,
    MissingNode(TypeRefNodePath),
    ExtraNode(TypeRefNodePath),
    DuplicateNode(TypeRefNodePath),
    HeadOutsideWhole(TypeRefNodePath),
    ChildOutsideParent(TypeRefNodePath),
    IndexOverflow(TypeRefNodePath),
    MissingLexeme {
        owner: TypeRefNodePath,
        kind: TypeRefLexemeKind,
    },
    ExtraLexeme {
        owner: TypeRefNodePath,
        kind: TypeRefLexemeKind,
    },
    DuplicateLexeme {
        owner: TypeRefNodePath,
        kind: TypeRefLexemeKind,
    },
    LexemeOutsideOwner {
        owner: TypeRefNodePath,
        kind: TypeRefLexemeKind,
    },
    LexemeOutOfOrder {
        owner: TypeRefNodePath,
        kind: TypeRefLexemeKind,
    },
    LexemeOrdinalOverflow(TypeRefNodePath),
    InvalidTurbofishLexeme(TypeRefNodePath),
    DuplicateComponent {
        owner: TypeRefNodePath,
        role: TypeRefComponentRole,
    },
    ComponentOutsideOwner {
        owner: TypeRefNodePath,
        role: TypeRefComponentRole,
    },
}

impl TypePath {
    /// Builds one type path from the identifier/separator tokens already
    /// consumed by the authoritative type grammar transaction.
    pub(super) fn from_token_parts(
        components: &[TypePathComponent<'_>],
        separators: &[TextRange],
        owner: &TypeRefNodePath,
        head_kind: TypeRefHeadKind,
    ) -> Result<ParsedTypePath, ProjectSymbolPathError> {
        let Some(first) = components.first() else {
            return Err(ProjectSymbolPathError::Empty);
        };
        if separators.len() + 1 != components.len() {
            return Err(ProjectSymbolPathError::EmptySegment);
        }
        let (root, first_segment) = match components {
            [
                TypePathComponent {
                    spelling: "crate", ..
                },
                ..,
            ] => (ModulePathRoot::Crate, 1),
            [
                TypePathComponent {
                    spelling: "self", ..
                },
                ..,
            ] => (ModulePathRoot::SelfModule, 1),
            [
                TypePathComponent {
                    spelling: "parent", ..
                },
                ..,
            ] => (ModulePathRoot::Super(1), 1),
            [
                TypePathComponent {
                    spelling: "super", ..
                },
                ..,
            ] => {
                let levels = components
                    .iter()
                    .take_while(|component| component.spelling == "super")
                    .count();
                (ModulePathRoot::Super(levels), levels)
            }
            _ => (ModulePathRoot::ImplicitCrate, 0),
        };
        let segments = components[first_segment..]
            .iter()
            .map(|component| ProjectSymbolSegment::try_new(component.spelling.to_owned()))
            .collect::<Result<Vec<_>, _>>()?;
        let path = Self(ProjectSymbolPath::new(root, segments)?);
        let terminal = components
            .last()
            .map(|component| component.range)
            .expect("non-empty paths retain a terminal component");
        let head = TypeRefHeadSource::with_terminal(
            head_kind,
            TextRange::new(first.range.start(), terminal.end()),
            terminal,
        );

        let mut lexemes = Vec::new();
        if first_segment > 0 {
            lexemes.push(TypeRefLexemeSource::new(
                owner.clone(),
                TypeRefLexemeKind::PathRoot,
                TextRange::new(
                    first.range.start(),
                    components[first_segment - 1].range.end(),
                ),
            ));
        }
        for (ordinal, component) in components.iter().skip(first_segment).enumerate() {
            let ordinal =
                u16::try_from(ordinal).map_err(|_| ProjectSymbolPathError::InvalidSegment {
                    segment: component.spelling.to_owned(),
                })?;
            lexemes.push(TypeRefLexemeSource::new(
                owner.clone(),
                TypeRefLexemeKind::PathSegment { ordinal },
                component.range,
            ));
            if ordinal > 0 || first_segment > 0 {
                let separator_index = first_segment + usize::from(ordinal) - 1;
                lexemes.push(TypeRefLexemeSource::new(
                    owner.clone(),
                    TypeRefLexemeKind::PathSeparator { before: ordinal },
                    separators[separator_index],
                ));
            }
        }

        Ok(ParsedTypePath {
            value: path,
            head,
            lexemes,
        })
    }

    /// Underlying validated project-symbol path.
    pub const fn path(&self) -> &ProjectSymbolPath {
        &self.0
    }

    /// Root behavior selected by the source spelling.
    pub const fn root(&self) -> ModulePathRoot {
        self.0.root()
    }

    /// Validated path segments after the root spelling.
    pub fn segments(&self) -> &[ProjectSymbolSegment] {
        self.0.segments()
    }

    /// Canonical source spelling for presentation only.
    pub fn canonical_string(&self) -> String {
        self.0.to_string()
    }
}

impl fmt::Display for TypePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl TypeRecoveryId {
    pub(crate) const fn from_index(index: u32) -> Self {
        Self(index)
    }

    /// Stable parser-local recovery ordinal.
    pub const fn index(self) -> u32 {
        self.0
    }
}

impl TypeRefNodePath {
    /// Root structural path.
    pub fn root() -> Self {
        Self(Box::new([]))
    }

    /// Structural steps from the authored root.
    pub fn steps(&self) -> &[TypeRefNodeStep] {
        &self.0
    }

    pub(crate) fn child(&self, step: TypeRefNodeStep) -> Self {
        let mut steps = self.0.to_vec();
        steps.push(step);
        Self(steps.into_boxed_slice())
    }

    fn parent(&self) -> Option<Self> {
        (!self.0.is_empty()).then(|| Self(self.0[..self.0.len() - 1].into()))
    }
}

impl<R> TypeRefHeadSource<R> {
    pub(super) const fn new(kind: TypeRefHeadKind, range: R) -> Self {
        Self {
            kind,
            range,
            terminal: None,
        }
    }

    pub(super) const fn with_terminal(kind: TypeRefHeadKind, range: R, terminal: R) -> Self {
        Self {
            kind,
            range,
            terminal: Some(terminal),
        }
    }

    /// Kind of type head represented by the range.
    pub const fn kind(&self) -> TypeRefHeadKind {
        self.kind
    }

    /// Exact head source.
    pub const fn range(&self) -> &R {
        &self.range
    }

    /// Exact final path segment selected by name resolution, when the head is a path.
    pub const fn terminal(&self) -> Option<&R> {
        self.terminal.as_ref()
    }
}

impl<R> TypeRefNodeSource<R> {
    pub(super) const fn new(whole: R, head: Option<TypeRefHeadSource<R>>) -> Self {
        Self { whole, head }
    }

    /// Exact source covering the complete structural node.
    pub const fn whole(&self) -> &R {
        &self.whole
    }

    /// Exact diagnostic head, when this node has one.
    pub const fn head(&self) -> Option<&TypeRefHeadSource<R>> {
        self.head.as_ref()
    }
}

impl TypeRefNodeSource<TextRange> {
    pub(super) fn replace_whole(&mut self, whole: TextRange) {
        self.whole = whole;
    }
}

impl<R> TypeRefLexemeSource<R> {
    pub(super) const fn new(owner: TypeRefNodePath, kind: TypeRefLexemeKind, range: R) -> Self {
        Self { owner, kind, range }
    }

    /// Structural type node that owns this token.
    pub const fn owner(&self) -> &TypeRefNodePath {
        &self.owner
    }

    /// Typed lexical role of this token.
    pub const fn kind(&self) -> &TypeRefLexemeKind {
        &self.kind
    }

    /// Exact token source.
    pub const fn range(&self) -> &R {
        &self.range
    }
}

impl<R> TypeRefComponentSource<R> {
    pub(super) const fn new(owner: TypeRefNodePath, role: TypeRefComponentRole, range: R) -> Self {
        Self { owner, role, range }
    }

    /// Structural type node that owns this component.
    pub const fn owner(&self) -> &TypeRefNodePath {
        &self.owner
    }

    /// Semantic role of this component.
    pub const fn role(&self) -> TypeRefComponentRole {
        self.role
    }

    /// Exact component source.
    pub const fn range(&self) -> &R {
        &self.range
    }
}

impl<R> TypeRefSourceMap<R> {
    /// Source entries in canonical structural-path order.
    pub fn nodes(&self) -> &[(TypeRefNodePath, TypeRefNodeSource<R>)] {
        &self.nodes
    }

    /// Typed lexical tokens in source order.
    pub fn lexemes(&self) -> &[TypeRefLexemeSource<R>] {
        &self.lexemes
    }

    /// Semantic components in structural-owner/role order.
    pub fn components(&self) -> &[TypeRefComponentSource<R>] {
        &self.components
    }

    /// Exact source for one semantic component.
    pub fn component_at(&self, owner: &TypeRefNodePath, role: TypeRefComponentRole) -> Option<&R> {
        self.components
            .binary_search_by(|component| (component.owner(), component.role()).cmp(&(owner, role)))
            .ok()
            .map(|index| self.components[index].range())
    }

    /// Source for one structural node.
    pub fn source_at(&self, path: &TypeRefNodePath) -> Option<&TypeRefNodeSource<R>> {
        self.nodes
            .binary_search_by(|(candidate, _)| candidate.cmp(path))
            .ok()
            .map(|index| &self.nodes[index].1)
    }

    /// Maps source carriers without changing structural node identity.
    pub fn try_map<S, E>(
        &self,
        mut map: impl FnMut(&R) -> Result<S, E>,
    ) -> Result<TypeRefSourceMap<S>, E> {
        let mut nodes = Vec::with_capacity(self.nodes.len());
        for (path, source) in &self.nodes {
            let whole = map(&source.whole)?;
            let head = source
                .head
                .as_ref()
                .map(|head| {
                    Ok(TypeRefHeadSource {
                        kind: head.kind,
                        range: map(&head.range)?,
                        terminal: head.terminal.as_ref().map(&mut map).transpose()?,
                    })
                })
                .transpose()?;
            nodes.push((path.clone(), TypeRefNodeSource { whole, head }));
        }
        let mut lexemes = Vec::with_capacity(self.lexemes.len());
        for lexeme in &self.lexemes {
            lexemes.push(TypeRefLexemeSource {
                owner: lexeme.owner.clone(),
                kind: lexeme.kind,
                range: map(&lexeme.range)?,
            });
        }
        let mut components = Vec::with_capacity(self.components.len());
        for component in &self.components {
            components.push(TypeRefComponentSource {
                owner: component.owner.clone(),
                role: component.role,
                range: map(&component.range)?,
            });
        }
        Ok(TypeRefSourceMap {
            nodes: nodes.into_boxed_slice(),
            lexemes: lexemes.into_boxed_slice(),
            components: components.into_boxed_slice(),
        })
    }
}

impl TypeRefSourceMap<TextRange> {
    pub(super) fn try_new(
        value: &TypeRef,
        nodes: Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
        lexemes: Vec<TypeRefLexemeSource<TextRange>>,
        mut components: Vec<TypeRefComponentSource<TextRange>>,
    ) -> Result<Self, TypeRefSourceMapError> {
        let ordered = validated_type_nodes(value, nodes)?;
        validate_type_lexemes(value, &ordered, &lexemes)?;
        components
            .sort_by(|left, right| (left.owner(), left.role()).cmp(&(right.owner(), right.role())));
        validate_type_components(&ordered, &components)?;
        Ok(Self {
            nodes: ordered.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            lexemes: lexemes.into_boxed_slice(),
            components: components.into_boxed_slice(),
        })
    }
}

fn validate_type_components(
    nodes: &BTreeMap<TypeRefNodePath, TypeRefNodeSource<TextRange>>,
    components: &[TypeRefComponentSource<TextRange>],
) -> Result<(), TypeRefSourceMapError> {
    let mut seen = BTreeSet::new();
    for component in components {
        if !seen.insert((component.owner.clone(), component.role)) {
            return Err(TypeRefSourceMapError::DuplicateComponent {
                owner: component.owner.clone(),
                role: component.role,
            });
        }
        let Some(owner) = nodes.get(&component.owner) else {
            return Err(TypeRefSourceMapError::ComponentOutsideOwner {
                owner: component.owner.clone(),
                role: component.role,
            });
        };
        if !contains(owner.whole, component.range) {
            return Err(TypeRefSourceMapError::ComponentOutsideOwner {
                owner: component.owner.clone(),
                role: component.role,
            });
        }
    }
    Ok(())
}

fn validated_type_nodes(
    value: &TypeRef,
    nodes: Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<BTreeMap<TypeRefNodePath, TypeRefNodeSource<TextRange>>, TypeRefSourceMapError> {
    let mut ordered = BTreeMap::new();
    for (path, source) in nodes {
        if ordered.insert(path.clone(), source).is_some() {
            return Err(TypeRefSourceMapError::DuplicateNode(path));
        }
    }
    let root = TypeRefNodePath::root();
    if !ordered.contains_key(&root) {
        return Err(TypeRefSourceMapError::MissingRoot);
    }
    let mut expected = BTreeSet::new();
    collect_expected_paths(value, &root, &mut expected)?;
    for path in &expected {
        if !ordered.contains_key(path) {
            return Err(TypeRefSourceMapError::MissingNode(path.clone()));
        }
    }
    for path in ordered.keys() {
        if !expected.contains(path) {
            return Err(TypeRefSourceMapError::ExtraNode(path.clone()));
        }
    }
    validate_type_node_ranges(&ordered)?;
    Ok(ordered)
}

fn validate_type_node_ranges(
    nodes: &BTreeMap<TypeRefNodePath, TypeRefNodeSource<TextRange>>,
) -> Result<(), TypeRefSourceMapError> {
    for (path, source) in nodes {
        if let Some(head) = &source.head
            && (!contains(source.whole, head.range)
                || head
                    .terminal
                    .is_some_and(|terminal| !contains(head.range, terminal)))
        {
            return Err(TypeRefSourceMapError::HeadOutsideWhole(path.clone()));
        }
        if let Some(parent) = path.parent() {
            let parent_source = nodes
                .get(&parent)
                .expect("validated type path parents are present");
            if !contains(parent_source.whole, source.whole) {
                return Err(TypeRefSourceMapError::ChildOutsideParent(path.clone()));
            }
        }
    }
    Ok(())
}

fn validate_type_lexemes(
    value: &TypeRef,
    nodes: &BTreeMap<TypeRefNodePath, TypeRefNodeSource<TextRange>>,
    lexemes: &[TypeRefLexemeSource<TextRange>],
) -> Result<(), TypeRefSourceMapError> {
    let expected_lexemes = expected_lexeme_keys(value)?;
    let mut actual_lexemes = BTreeSet::new();
    let mut previous_end = None;
    let mut turbofish = BTreeMap::new();
    let mut open_angles = BTreeMap::new();
    for lexeme in lexemes {
        let key = (lexeme.owner.clone(), lexeme.kind);
        if !actual_lexemes.insert(key.clone()) {
            return Err(TypeRefSourceMapError::DuplicateLexeme {
                owner: key.0,
                kind: key.1,
            });
        }
        let Some(owner) = nodes.get(&lexeme.owner) else {
            return Err(TypeRefSourceMapError::ExtraLexeme {
                owner: lexeme.owner.clone(),
                kind: lexeme.kind,
            });
        };
        if !contains(owner.whole, lexeme.range) {
            return Err(TypeRefSourceMapError::LexemeOutsideOwner {
                owner: lexeme.owner.clone(),
                kind: lexeme.kind,
            });
        }
        if previous_end.is_some_and(|end| lexeme.range.start() < end) {
            return Err(TypeRefSourceMapError::LexemeOutOfOrder {
                owner: lexeme.owner.clone(),
                kind: lexeme.kind,
            });
        }
        previous_end = Some(lexeme.range.end());
        match lexeme.kind {
            TypeRefLexemeKind::TurbofishSeparator => {
                turbofish.insert(lexeme.owner.clone(), lexeme.range);
            }
            TypeRefLexemeKind::OpenAngle => {
                open_angles.insert(lexeme.owner.clone(), lexeme.range);
            }
            _ => {}
        }
    }
    for (owner, kind) in &expected_lexemes {
        if !actual_lexemes.contains(&(owner.clone(), *kind)) {
            return Err(TypeRefSourceMapError::MissingLexeme {
                owner: owner.clone(),
                kind: *kind,
            });
        }
    }
    for (owner, kind) in &actual_lexemes {
        if matches!(
            kind,
            TypeRefLexemeKind::TurbofishSeparator | TypeRefLexemeKind::TrailingArgumentSeparator
        ) && matches!(
            value_at_path(value, owner),
            Some(TypeRef::Generic { .. } | TypeRef::TraitBound(_))
        ) {
            continue;
        }
        if !expected_lexemes.contains(&(owner.clone(), *kind)) {
            return Err(TypeRefSourceMapError::ExtraLexeme {
                owner: owner.clone(),
                kind: *kind,
            });
        }
    }
    for (owner, range) in turbofish {
        if !matches!(
            value_at_path(value, &owner),
            Some(TypeRef::Generic { .. } | TypeRef::TraitBound(_))
        ) || open_angles
            .get(&owner)
            .is_none_or(|open| range.end() != open.start())
        {
            return Err(TypeRefSourceMapError::InvalidTurbofishLexeme(owner));
        }
    }

    Ok(())
}

fn value_at_path<'a>(value: &'a TypeRef, path: &TypeRefNodePath) -> Option<&'a TypeRef> {
    let mut value = value;
    for step in path.steps() {
        value = match (value, step) {
            (TypeRef::Tuple(items), TypeRefNodeStep::TupleItem(index))
            | (TypeRef::Choice(items), TypeRefNodeStep::ChoiceAlternative(index))
            | (TypeRef::Generic { args: items, .. }, TypeRefNodeStep::GenericArgument(index)) => {
                items.get(usize::from(*index))?
            }
            (TypeRef::Function { params, .. }, TypeRefNodeStep::FunctionParameter(index)) => {
                params.get(usize::from(*index))?
            }
            (TypeRef::Function { return_type, .. }, TypeRefNodeStep::FunctionReturn) => return_type,
            (TypeRef::TraitBound(bound), TypeRefNodeStep::TraitArgument(index)) => {
                bound.args().get(usize::from(*index))?
            }
            (TypeRef::TraitBound(bound), TypeRefNodeStep::AssociatedBinding(index)) => {
                bound.associated().get(usize::from(*index))?.value()
            }
            (TypeRef::Projection { subject, .. }, TypeRefNodeStep::ProjectionSubject) => subject,
            (TypeRef::Reference(reference), TypeRefNodeStep::ReferenceReferent) => {
                reference.referent()
            }
            (TypeRef::Slice(item), TypeRefNodeStep::SliceItem) => item,
            _ => return None,
        };
    }
    Some(value)
}

impl AuthoredTypeRef {
    pub(super) fn try_new(
        value: TypeRef,
        nodes: Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
        lexemes: Vec<TypeRefLexemeSource<TextRange>>,
        components: Vec<TypeRefComponentSource<TextRange>>,
    ) -> Result<Self, TypeRefSourceMapError> {
        let source = TypeRefSourceMap::try_new(&value, nodes, lexemes, components)?;
        Ok(Self {
            value,
            source: Box::new(source),
        })
    }

    /// Parsed type structure.
    pub const fn value(&self) -> &TypeRef {
        &self.value
    }

    /// Complete structural source map.
    pub const fn source(&self) -> &TypeRefSourceMap<TextRange> {
        &self.source
    }

    /// Source of the authored root node.
    ///
    /// # Panics
    ///
    /// Panics only if the internally validated source map has lost its root,
    /// which cannot be constructed through the public parser API.
    pub fn root_source(&self) -> &TypeRefNodeSource<TextRange> {
        self.source
            .source_at(&TypeRefNodePath::root())
            .expect("authored type source maps always contain a root")
    }

    /// Source for one structural node.
    pub fn source_at(&self, path: &TypeRefNodePath) -> Option<&TypeRefNodeSource<TextRange>> {
        self.source.source_at(path)
    }

    /// Parsed type node at one validated structural source-map path.
    pub fn value_at(&self, path: &TypeRefNodePath) -> Option<&TypeRef> {
        let mut value = &self.value;
        for step in path.steps() {
            value = match (value, step) {
                (TypeRef::Tuple(items), TypeRefNodeStep::TupleItem(index))
                | (TypeRef::Choice(items), TypeRefNodeStep::ChoiceAlternative(index))
                | (TypeRef::Generic { args: items, .. }, TypeRefNodeStep::GenericArgument(index)) => {
                    items.get(usize::from(*index))?
                }
                (TypeRef::Function { params, .. }, TypeRefNodeStep::FunctionParameter(index)) => {
                    params.get(usize::from(*index))?
                }
                (TypeRef::Function { return_type, .. }, TypeRefNodeStep::FunctionReturn) => {
                    return_type
                }
                (TypeRef::TraitBound(bound), TypeRefNodeStep::TraitArgument(index)) => {
                    bound.args().get(usize::from(*index))?
                }
                (TypeRef::TraitBound(bound), TypeRefNodeStep::AssociatedBinding(index)) => {
                    bound.associated().get(usize::from(*index))?.value()
                }
                (TypeRef::Projection { subject, .. }, TypeRefNodeStep::ProjectionSubject) => {
                    subject
                }
                (TypeRef::Reference(reference), TypeRefNodeStep::ReferenceReferent) => {
                    reference.referent()
                }
                (TypeRef::Slice(item), TypeRefNodeStep::SliceItem) => item,
                _ => return None,
            };
        }
        Some(value)
    }

    #[cfg(test)]
    pub(crate) fn into_value(self) -> TypeRef {
        self.value
    }

    pub(crate) fn recovery(index: u32, range: TextRange) -> Self {
        Self::recovery_with_source(index, range, range)
    }

    pub(crate) fn recovery_with_source(index: u32, whole: TextRange, recovery: TextRange) -> Self {
        let id = TypeRecoveryId::from_index(index);
        let path = TypeRefNodePath::root();
        Self::try_new(
            TypeRef::Recovery(id),
            vec![(
                path,
                TypeRefNodeSource::new(
                    whole,
                    Some(TypeRefHeadSource::new(TypeRefHeadKind::Recovery, recovery)),
                ),
            )],
            Vec::new(),
            vec![
                TypeRefComponentSource::new(
                    TypeRefNodePath::root(),
                    TypeRefComponentRole::Whole,
                    whole,
                ),
                TypeRefComponentSource::new(
                    TypeRefNodePath::root(),
                    TypeRefComponentRole::Recovery,
                    recovery,
                ),
            ],
        )
        .expect("one recovery root is a valid type source map")
    }

    pub(crate) fn rebase(&mut self, base: usize) {
        if base == 0 {
            return;
        }
        self.value.rebase_reference_ranges(base);
        for (_, source) in &mut self.source.nodes {
            source.whole = rebase_range(source.whole, base);
            if let Some(head) = &mut source.head {
                head.range = rebase_range(head.range, base);
                if let Some(terminal) = &mut head.terminal {
                    *terminal = rebase_range(*terminal, base);
                }
            }
        }
        for lexeme in &mut self.source.lexemes {
            lexeme.range = rebase_range(lexeme.range, base);
        }
        for component in &mut self.source.components {
            component.range = rebase_range(component.range, base);
        }
    }
}

fn rebase_range(range: TextRange, base: usize) -> TextRange {
    TextRange::new(range.start() + base, range.end() + base)
}

fn contains(parent: TextRange, child: TextRange) -> bool {
    parent.start() <= child.start() && child.end() <= parent.end()
}

fn collect_expected_paths(
    value: &TypeRef,
    path: &TypeRefNodePath,
    output: &mut BTreeSet<TypeRefNodePath>,
) -> Result<(), TypeRefSourceMapError> {
    let mut pending = vec![(path.clone(), value)];
    while let Some((path, value)) = pending.pop() {
        output.insert(path.clone());
        schedule_type_children(
            value,
            &path,
            &mut pending,
            TypeRefSourceMapError::IndexOverflow,
        )?;
    }
    Ok(())
}

fn expected_lexeme_keys(
    value: &TypeRef,
) -> Result<BTreeSet<(TypeRefNodePath, TypeRefLexemeKind)>, TypeRefSourceMapError> {
    let mut output = BTreeSet::new();
    let mut pending = vec![(TypeRefNodePath::root(), value)];
    while let Some((path, value)) = pending.pop() {
        match value {
            TypeRef::Path(ty) => collect_expected_path_lexemes(ty, &path, &mut output)?,
            TypeRef::Generic { base, args } => {
                collect_expected_path_lexemes(base, &path, &mut output)?;
                collect_expected_generic_lexemes(args.len(), &path, &mut output)?;
            }
            TypeRef::TraitBound(bound) => {
                collect_expected_path_lexemes(&bound.path, &path, &mut output)?;
                let count = bound
                    .args()
                    .len()
                    .checked_add(bound.associated().len())
                    .ok_or_else(|| TypeRefSourceMapError::LexemeOrdinalOverflow(path.clone()))?;
                collect_expected_generic_lexemes(count, &path, &mut output)?;
            }
            TypeRef::Never
            | TypeRef::ConstInt(_)
            | TypeRef::Tuple(_)
            | TypeRef::Function { .. }
            | TypeRef::Choice(_)
            | TypeRef::Projection { .. }
            | TypeRef::Reference(_)
            | TypeRef::Slice(_)
            | TypeRef::Recovery(_) => {}
        }
        schedule_type_children(
            value,
            &path,
            &mut pending,
            TypeRefSourceMapError::LexemeOrdinalOverflow,
        )?;
    }
    Ok(output)
}

fn collect_expected_path_lexemes(
    ty: &TypePath,
    owner: &TypeRefNodePath,
    output: &mut BTreeSet<(TypeRefNodePath, TypeRefLexemeKind)>,
) -> Result<(), TypeRefSourceMapError> {
    if !matches!(ty.root(), ModulePathRoot::ImplicitCrate) {
        output.insert((owner.clone(), TypeRefLexemeKind::PathRoot));
    }
    for ordinal in 0..ty.segments().len() {
        let ordinal = u16::try_from(ordinal)
            .map_err(|_| TypeRefSourceMapError::LexemeOrdinalOverflow(owner.clone()))?;
        output.insert((owner.clone(), TypeRefLexemeKind::PathSegment { ordinal }));
        if ordinal > 0 || !matches!(ty.root(), ModulePathRoot::ImplicitCrate) {
            output.insert((
                owner.clone(),
                TypeRefLexemeKind::PathSeparator { before: ordinal },
            ));
        }
    }
    Ok(())
}

fn collect_expected_generic_lexemes(
    argument_count: usize,
    owner: &TypeRefNodePath,
    output: &mut BTreeSet<(TypeRefNodePath, TypeRefLexemeKind)>,
) -> Result<(), TypeRefSourceMapError> {
    output.insert((owner.clone(), TypeRefLexemeKind::OpenAngle));
    for before in 1..argument_count {
        output.insert((
            owner.clone(),
            TypeRefLexemeKind::ArgumentSeparator {
                before: u16::try_from(before)
                    .map_err(|_| TypeRefSourceMapError::LexemeOrdinalOverflow(owner.clone()))?,
            },
        ));
    }
    output.insert((owner.clone(), TypeRefLexemeKind::CloseAngle));
    Ok(())
}

fn schedule_type_children<'a>(
    value: &'a TypeRef,
    path: &TypeRefNodePath,
    pending: &mut Vec<(TypeRefNodePath, &'a TypeRef)>,
    ordinal_error: fn(TypeRefNodePath) -> TypeRefSourceMapError,
) -> Result<(), TypeRefSourceMapError> {
    match value {
        TypeRef::Tuple(items) => schedule_indexed_children(
            items,
            path,
            pending,
            TypeRefNodeStep::TupleItem,
            ordinal_error,
        )?,
        TypeRef::Function {
            params,
            return_type,
            ..
        } => {
            pending.push((path.child(TypeRefNodeStep::FunctionReturn), return_type));
            schedule_indexed_children(
                params,
                path,
                pending,
                TypeRefNodeStep::FunctionParameter,
                ordinal_error,
            )?;
        }
        TypeRef::Choice(items) => schedule_indexed_children(
            items,
            path,
            pending,
            TypeRefNodeStep::ChoiceAlternative,
            ordinal_error,
        )?,
        TypeRef::Generic { args, .. } => schedule_indexed_children(
            args,
            path,
            pending,
            TypeRefNodeStep::GenericArgument,
            ordinal_error,
        )?,
        TypeRef::TraitBound(bound) => {
            for (index, binding) in bound.associated().iter().enumerate().rev() {
                let index = u16::try_from(index).map_err(|_| ordinal_error(path.clone()))?;
                pending.push((
                    path.child(TypeRefNodeStep::AssociatedBinding(index)),
                    binding.value(),
                ));
            }
            schedule_indexed_children(
                bound.args(),
                path,
                pending,
                TypeRefNodeStep::TraitArgument,
                ordinal_error,
            )?;
        }
        TypeRef::Projection { subject, .. } => {
            pending.push((path.child(TypeRefNodeStep::ProjectionSubject), subject));
        }
        TypeRef::Reference(reference) => pending.push((
            path.child(TypeRefNodeStep::ReferenceReferent),
            reference.referent(),
        )),
        TypeRef::Slice(item) => {
            pending.push((path.child(TypeRefNodeStep::SliceItem), item));
        }
        TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) | TypeRef::Recovery(_) => {}
    }
    Ok(())
}

fn schedule_indexed_children<'a>(
    values: &'a [TypeRef],
    path: &TypeRefNodePath,
    pending: &mut Vec<(TypeRefNodePath, &'a TypeRef)>,
    step: fn(u16) -> TypeRefNodeStep,
    ordinal_error: fn(TypeRefNodePath) -> TypeRefSourceMapError,
) -> Result<(), TypeRefSourceMapError> {
    for (index, value) in values.iter().enumerate().rev() {
        let index = u16::try_from(index).map_err(|_| ordinal_error(path.clone()))?;
        pending.push((path.child(step(index)), value));
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
