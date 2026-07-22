//! Typed type paths and exact source evidence for authored type references.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::ast::{
    common::TextRange,
    module_path::ModulePathRoot,
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SpannedProjectSymbolPath},
};

use super::TypeRef;
use super::{AssociatedTypeBinding, TypeParseError};

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

/// One-to-one source map for every structural node in a type reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefSourceMap<R> {
    nodes: Box<[(TypeRefNodePath, TypeRefNodeSource<R>)]>,
}

/// Parsed type structure coupled to its exact syntax source map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredTypeRef {
    value: TypeRef,
    source: TypeRefSourceMap<TextRange>,
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
}

impl TypePath {
    pub(super) fn parse(
        source: &str,
    ) -> Result<Self, crate::ast::symbol_path::ProjectSymbolPathError> {
        source.parse().map(Self)
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

    pub(super) fn child(&self, step: TypeRefNodeStep) -> Self {
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

impl<R> TypeRefSourceMap<R> {
    /// Source entries in canonical structural-path order.
    pub fn nodes(&self) -> &[(TypeRefNodePath, TypeRefNodeSource<R>)] {
        &self.nodes
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
        Ok(TypeRefSourceMap {
            nodes: nodes.into_boxed_slice(),
        })
    }
}

impl TypeRefSourceMap<TextRange> {
    pub(super) fn try_new(
        value: &TypeRef,
        nodes: Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
    ) -> Result<Self, TypeRefSourceMapError> {
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
        for (path, source) in &ordered {
            if let Some(head) = &source.head
                && !contains(source.whole, head.range)
            {
                return Err(TypeRefSourceMapError::HeadOutsideWhole(path.clone()));
            }
            if let Some(head) = &source.head
                && let Some(terminal) = head.terminal
                && !contains(head.range, terminal)
            {
                return Err(TypeRefSourceMapError::HeadOutsideWhole(path.clone()));
            }
            if let Some(parent) = path.parent() {
                let parent_source = ordered
                    .get(&parent)
                    .expect("validated type path parents are present");
                if !contains(parent_source.whole, source.whole) {
                    return Err(TypeRefSourceMapError::ChildOutsideParent(path.clone()));
                }
            }
        }
        Ok(Self {
            nodes: ordered.into_iter().collect::<Vec<_>>().into_boxed_slice(),
        })
    }
}

impl AuthoredTypeRef {
    pub(super) fn try_new(
        value: TypeRef,
        nodes: Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
    ) -> Result<Self, TypeRefSourceMapError> {
        let source = TypeRefSourceMap::try_new(&value, nodes)?;
        Ok(Self { value, source })
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
        let id = TypeRecoveryId::from_index(index);
        let path = TypeRefNodePath::root();
        Self::try_new(
            TypeRef::Recovery(id),
            vec![(
                path,
                TypeRefNodeSource::new(
                    range,
                    Some(TypeRefHeadSource::new(TypeRefHeadKind::Recovery, range)),
                ),
            )],
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
    }
}

fn rebase_range(range: TextRange, base: usize) -> TextRange {
    TextRange::new(range.start() + base, range.end() + base)
}

pub(super) fn build_type_source_map(
    source: &str,
    value: &TypeRef,
) -> Result<Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>, TypeParseError> {
    let mut nodes = Vec::new();
    map_node(source, 0, value, &TypeRefNodePath::root(), &mut nodes)?;
    Ok(nodes)
}

fn map_node(
    source: &str,
    base: usize,
    value: &TypeRef,
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<(), TypeParseError> {
    let leading = source.len().saturating_sub(source.trim_start().len());
    let source = source.trim();
    let base = base + leading;
    let whole = TextRange::new(base, base + source.len());

    if map_single_argument_generic_chain(source, base, value, path, output)?
        || map_function_syntax(source, base, whole, value, path, output)?
        || map_choice_syntax(source, base, whole, value, path, output)?
        || map_parenthesized_syntax(source, base, whole, value, path, output)?
    {
        return Ok(());
    }
    map_structural_node(source, base, whole, value, path, output)
}

/// Maps unary generic chains iteratively so the source-evidence pass supports
/// every type depth accepted by the semantic resolver without depending on the
/// host thread's call-stack size.
fn map_single_argument_generic_chain(
    source: &str,
    base: usize,
    value: &TypeRef,
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<bool, TypeParseError> {
    let mut source = source;
    let mut base = base;
    let mut value = value;
    let mut path = path.clone();
    let mut mapped = false;

    while let TypeRef::Generic { args, .. } = value {
        if args.len() != 1 {
            break;
        }
        let Some((head, arguments)) = super::split_generic_type(source) else {
            break;
        };
        let fragments = super::split_type_args(arguments);
        if fragments.len() != 1 {
            break;
        }
        let fragment = fragments[0].trim();
        if fragment.is_empty() || super::split_top_level_punctuation_once(fragment, '=').is_some() {
            break;
        }

        let whole = TextRange::new(base, base + source.len());
        output.push((
            path.clone(),
            TypeRefNodeSource::new(
                whole,
                Some(path_head_source(
                    TypeRefHeadKind::Constructor,
                    head,
                    base + super::subslice_offset(source, head),
                )?),
            ),
        ));
        path = path.child(TypeRefNodeStep::GenericArgument(0));
        base = base
            .checked_add(super::subslice_offset(source, fragment))
            .ok_or_else(|| TypeParseError::new("type source offset overflow"))?;
        source = fragment;
        value = &args[0];
        mapped = true;
    }

    if mapped {
        map_node(source, base, value, &path, output)?;
    }
    Ok(mapped)
}

fn map_function_syntax(
    source: &str,
    base: usize,
    whole: TextRange,
    value: &TypeRef,
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<bool, TypeParseError> {
    let (function_source, effects) = super::split_type_effect_row_suffix(source)?;
    if let Some((params_source, return_source)) = super::split_top_level_arrow(function_source)
        && let TypeRef::Function {
            params,
            return_type,
            ..
        } = value
    {
        output.push((path.clone(), TypeRefNodeSource::new(whole, None)));
        let params_source = params_source.trim();
        map_function_parameters(
            params_source,
            base + super::subslice_offset(source, params_source),
            params,
            path,
            output,
        )?;
        let return_source = return_source.trim();
        map_node(
            return_source,
            base + super::subslice_offset(source, return_source),
            return_type,
            &path.child(TypeRefNodeStep::FunctionReturn),
            output,
        )?;
        return Ok(true);
    }
    if effects.is_some()
        && let Some(inner) = super::parenthesized_type(function_source)
        && matches!(value, TypeRef::Function { .. })
    {
        map_node(
            inner,
            base + super::subslice_offset(source, inner),
            value,
            path,
            output,
        )?;
        replace_whole(output, path, whole);
        return Ok(true);
    }
    Ok(false)
}

fn map_choice_syntax(
    source: &str,
    base: usize,
    whole: TextRange,
    value: &TypeRef,
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<bool, TypeParseError> {
    let alternatives = super::split_top_level_punctuation(source, '|');
    if alternatives.len() > 1
        && let TypeRef::Choice(values) = value
    {
        output.push((path.clone(), TypeRefNodeSource::new(whole, None)));
        for (index, (fragment, value)) in alternatives.into_iter().zip(values).enumerate() {
            let fragment = fragment.trim();
            map_node(
                fragment,
                base + super::subslice_offset(source, fragment),
                value,
                &path.child(TypeRefNodeStep::ChoiceAlternative(index_u16(index, path)?)),
                output,
            )?;
        }
        return Ok(true);
    }
    Ok(false)
}

fn map_parenthesized_syntax(
    source: &str,
    base: usize,
    whole: TextRange,
    value: &TypeRef,
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<bool, TypeParseError> {
    if let Some(inner) = super::parenthesized_type(source) {
        let parts = super::split_top_level_punctuation(inner, ',');
        if parts.len() > 1
            && let TypeRef::Tuple(values) = value
        {
            output.push((path.clone(), TypeRefNodeSource::new(whole, None)));
            for (index, (fragment, value)) in parts.into_iter().zip(values).enumerate() {
                let fragment = fragment.trim();
                map_node(
                    fragment,
                    base + super::subslice_offset(source, fragment),
                    value,
                    &path.child(TypeRefNodeStep::TupleItem(index_u16(index, path)?)),
                    output,
                )?;
            }
            return Ok(true);
        }
        map_node(
            inner,
            base + super::subslice_offset(source, inner),
            value,
            path,
            output,
        )?;
        replace_whole(output, path, whole);
        return Ok(true);
    }
    Ok(false)
}

fn map_structural_node(
    source: &str,
    base: usize,
    whole: TextRange,
    value: &TypeRef,
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<(), TypeParseError> {
    match value {
        TypeRef::Never => output.push((
            path.clone(),
            TypeRefNodeSource::new(
                whole,
                Some(TypeRefHeadSource::new(TypeRefHeadKind::Never, whole)),
            ),
        )),
        TypeRef::ConstInt(_) => output.push((
            path.clone(),
            TypeRefNodeSource::new(
                whole,
                Some(TypeRefHeadSource::new(TypeRefHeadKind::ConstInt, whole)),
            ),
        )),
        TypeRef::Path(_) => output.push((
            path.clone(),
            TypeRefNodeSource::new(
                whole,
                Some(path_head_source(TypeRefHeadKind::Path, source, base)?),
            ),
        )),
        TypeRef::Generic { args, .. } => {
            map_generic_node(source, base, whole, args, path, output)?;
        }
        TypeRef::TraitBound(bound) => {
            map_trait_node(source, base, whole, bound, path, output)?;
        }
        TypeRef::Projection { subject, .. } => {
            map_projection_node(source, base, whole, subject, path, output)?;
        }
        TypeRef::Reference(reference) => {
            map_reference_node(source, base, whole, reference, path, output)?;
        }
        TypeRef::Slice(item) => map_slice_node(source, base, whole, item, path, output)?,
        TypeRef::Recovery(_) => output.push((
            path.clone(),
            TypeRefNodeSource::new(
                whole,
                Some(TypeRefHeadSource::new(TypeRefHeadKind::Recovery, whole)),
            ),
        )),
        TypeRef::Tuple(_) | TypeRef::Function { .. } | TypeRef::Choice(_) => {
            return Err(TypeParseError::new(
                "type source no longer matches its parsed structural form",
            ));
        }
    }
    Ok(())
}

fn map_generic_node(
    source: &str,
    base: usize,
    whole: TextRange,
    args: &[TypeRef],
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<(), TypeParseError> {
    let (head, arguments) = super::split_generic_type(source).ok_or_else(|| {
        TypeParseError::new("generic type source no longer matches its parsed structure")
    })?;
    output.push((
        path.clone(),
        TypeRefNodeSource::new(
            whole,
            Some(path_head_source(
                TypeRefHeadKind::Constructor,
                head,
                base + super::subslice_offset(source, head),
            )?),
        ),
    ));
    for (index, (fragment, value)) in super::split_type_args(arguments)
        .into_iter()
        .zip(args)
        .enumerate()
    {
        let fragment = fragment.trim();
        map_node(
            fragment,
            base + super::subslice_offset(source, fragment),
            value,
            &path.child(TypeRefNodeStep::GenericArgument(index_u16(index, path)?)),
            output,
        )?;
    }
    Ok(())
}

fn map_trait_node(
    source: &str,
    base: usize,
    whole: TextRange,
    bound: &super::TraitBound,
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<(), TypeParseError> {
    let (head, arguments) = super::split_generic_type(source).ok_or_else(|| {
        TypeParseError::new("trait-bound source no longer matches its parsed structure")
    })?;
    output.push((
        path.clone(),
        TypeRefNodeSource::new(
            whole,
            Some(path_head_source(
                TypeRefHeadKind::Trait,
                head,
                base + super::subslice_offset(source, head),
            )?),
        ),
    ));
    map_trait_arguments(
        source,
        base,
        arguments,
        bound.args(),
        bound.associated(),
        path,
        output,
    )
}

fn map_projection_node(
    source: &str,
    base: usize,
    whole: TextRange,
    subject: &TypeRef,
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<(), TypeParseError> {
    let (subject_source, member_source) =
        super::split_type_projection(source).ok_or_else(|| {
            TypeParseError::new("projection source no longer matches its parsed structure")
        })?;
    let member_source = member_source.trim();
    output.push((
        path.clone(),
        TypeRefNodeSource::new(
            whole,
            Some(TypeRefHeadSource::with_terminal(
                TypeRefHeadKind::ProjectionMember,
                fragment_range(source, base, member_source),
                fragment_range(source, base, member_source),
            )),
        ),
    ));
    let subject_source = subject_source.trim();
    map_node(
        subject_source,
        base + super::subslice_offset(source, subject_source),
        subject,
        &path.child(TypeRefNodeStep::ProjectionSubject),
        output,
    )
}

fn map_reference_node(
    source: &str,
    base: usize,
    whole: TextRange,
    reference: &super::ReferenceType,
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<(), TypeParseError> {
    output.push((path.clone(), TypeRefNodeSource::new(whole, None)));
    let cursor = super::reference::reference_referent_start(source)?;
    map_node(
        &source[cursor..],
        base + cursor,
        reference.referent(),
        &path.child(TypeRefNodeStep::ReferenceReferent),
        output,
    )
}

fn map_slice_node(
    source: &str,
    base: usize,
    whole: TextRange,
    item: &TypeRef,
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<(), TypeParseError> {
    output.push((path.clone(), TypeRefNodeSource::new(whole, None)));
    let inner = source
        .strip_prefix('[')
        .and_then(|source| source.strip_suffix(']'))
        .ok_or_else(|| {
            TypeParseError::new("slice source no longer matches its parsed structure")
        })?;
    map_node(
        inner,
        base + 1,
        item,
        &path.child(TypeRefNodeStep::SliceItem),
        output,
    )
}

fn map_function_parameters(
    source: &str,
    base: usize,
    values: &[TypeRef],
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<(), TypeParseError> {
    let (parameter_source, parameter_base, fragments) =
        if let Some(inner) = super::parenthesized_type(source) {
            let parts = super::split_top_level_punctuation(inner, ',');
            (
                inner,
                base + super::subslice_offset(source, inner),
                if parts.len() > 1 { parts } else { vec![inner] },
            )
        } else {
            (source, base, vec![source])
        };
    for (index, (fragment, value)) in fragments.into_iter().zip(values).enumerate() {
        let fragment = fragment.trim();
        map_node(
            fragment,
            parameter_base + super::subslice_offset(parameter_source, fragment),
            value,
            &path.child(TypeRefNodeStep::FunctionParameter(index_u16(index, path)?)),
            output,
        )?;
    }
    Ok(())
}

fn map_trait_arguments(
    source: &str,
    base: usize,
    arguments: &str,
    type_arguments: &[TypeRef],
    bindings: &[AssociatedTypeBinding],
    path: &TypeRefNodePath,
    output: &mut Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
) -> Result<(), TypeParseError> {
    let mut type_index = 0usize;
    let mut binding_index = 0usize;
    for fragment in super::split_type_args(arguments) {
        if let Some((_, value_source)) = super::split_top_level_punctuation_once(fragment, '=') {
            let value = bindings
                .get(binding_index)
                .ok_or_else(|| TypeParseError::new("missing parsed associated binding"))?;
            let value_source = value_source.trim();
            map_node(
                value_source,
                base + super::subslice_offset(source, value_source),
                value.value(),
                &path.child(TypeRefNodeStep::AssociatedBinding(index_u16(
                    binding_index,
                    path,
                )?)),
                output,
            )?;
            binding_index += 1;
        } else {
            let value = type_arguments
                .get(type_index)
                .ok_or_else(|| TypeParseError::new("missing parsed trait argument"))?;
            let fragment = fragment.trim();
            map_node(
                fragment,
                base + super::subslice_offset(source, fragment),
                value,
                &path.child(TypeRefNodeStep::TraitArgument(index_u16(type_index, path)?)),
                output,
            )?;
            type_index += 1;
        }
    }
    Ok(())
}

fn fragment_range(parent: &str, base: usize, fragment: &str) -> TextRange {
    let start = base + super::subslice_offset(parent, fragment);
    TextRange::new(start, start + fragment.len())
}

fn path_head_source(
    kind: TypeRefHeadKind,
    source: &str,
    base: usize,
) -> Result<TypeRefHeadSource<TextRange>, TypeParseError> {
    let path = SpannedProjectSymbolPath::parse_at(source, base).map_err(|error| {
        TypeParseError::new_owned(format!(
            "type path source no longer matches its parsed structure: {error}"
        ))
    })?;
    let terminal = path.segment_ranges().last().copied().ok_or_else(|| {
        TypeParseError::new("type path source has no resolvable terminal segment")
    })?;
    Ok(TypeRefHeadSource::with_terminal(
        kind,
        path.range(),
        terminal,
    ))
}

fn replace_whole(
    output: &mut [(TypeRefNodePath, TypeRefNodeSource<TextRange>)],
    path: &TypeRefNodePath,
    whole: TextRange,
) {
    let (_, source) = output
        .iter_mut()
        .find(|(candidate, _)| candidate == path)
        .expect("mapped wrapper contains its structural root");
    source.whole = whole;
}

fn index_u16(index: usize, path: &TypeRefNodePath) -> Result<u16, TypeParseError> {
    u16::try_from(index).map_err(|_| {
        TypeParseError::new_owned(format!(
            "type node at {:?} has too many indexed children",
            path.steps()
        ))
    })
}

fn contains(parent: TextRange, child: TextRange) -> bool {
    parent.start() <= child.start() && child.end() <= parent.end()
}

fn collect_expected_paths(
    value: &TypeRef,
    path: &TypeRefNodePath,
    output: &mut BTreeSet<TypeRefNodePath>,
) -> Result<(), TypeRefSourceMapError> {
    output.insert(path.clone());
    match value {
        TypeRef::Tuple(items) => collect_indexed(items, path, output, TypeRefNodeStep::TupleItem)?,
        TypeRef::Function {
            params,
            return_type,
            ..
        } => {
            collect_indexed(params, path, output, TypeRefNodeStep::FunctionParameter)?;
            collect_expected_paths(
                return_type,
                &path.child(TypeRefNodeStep::FunctionReturn),
                output,
            )?;
        }
        TypeRef::Choice(items) => {
            collect_indexed(items, path, output, TypeRefNodeStep::ChoiceAlternative)?;
        }
        TypeRef::Generic { args, .. } => {
            collect_indexed(args, path, output, TypeRefNodeStep::GenericArgument)?;
        }
        TypeRef::TraitBound(bound) => {
            collect_indexed(bound.args(), path, output, TypeRefNodeStep::TraitArgument)?;
            for (index, binding) in bound.associated().iter().enumerate() {
                let index = u16::try_from(index)
                    .map_err(|_| TypeRefSourceMapError::IndexOverflow(path.clone()))?;
                collect_expected_paths(
                    binding.value(),
                    &path.child(TypeRefNodeStep::AssociatedBinding(index)),
                    output,
                )?;
            }
        }
        TypeRef::Projection { subject, .. } => collect_expected_paths(
            subject,
            &path.child(TypeRefNodeStep::ProjectionSubject),
            output,
        )?,
        TypeRef::Reference(reference) => collect_expected_paths(
            reference.referent(),
            &path.child(TypeRefNodeStep::ReferenceReferent),
            output,
        )?,
        TypeRef::Slice(item) => {
            collect_expected_paths(item, &path.child(TypeRefNodeStep::SliceItem), output)?;
        }
        TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) | TypeRef::Recovery(_) => {}
    }
    Ok(())
}

fn collect_indexed(
    values: &[TypeRef],
    path: &TypeRefNodePath,
    output: &mut BTreeSet<TypeRefNodePath>,
    step: impl Fn(u16) -> TypeRefNodeStep,
) -> Result<(), TypeRefSourceMapError> {
    for (index, value) in values.iter().enumerate() {
        let index =
            u16::try_from(index).map_err(|_| TypeRefSourceMapError::IndexOverflow(path.clone()))?;
        collect_expected_paths(value, &path.child(step(index)), output)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
