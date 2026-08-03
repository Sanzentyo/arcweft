//! Exact source ownership for parser-produced semantic Pattern trees.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arcweft_source::SourceRange;

use crate::id_ref::{SyntaxIdRefPart, SyntaxIdRefSyntax};
use crate::literal::SyntaxLiteralSyntax;
use crate::types::{AuthoredTypeRef, TypeRefNodePath};

use super::{
    PatternBindingSite, PatternNodePath, PatternNodeStep, PatternPathRoot, PatternPathSyntax,
    PatternRecordFieldShape, PatternRecordFieldSyntax, PatternSequenceRestSyntax,
    PatternSyntaxKind, PatternSyntaxNode, PatternUnqualifiedVariantForm, PatternVariantHead,
    PatternVariantHeadSyntax, PatternVariantPayloadSyntax, collect_binding_sites,
    mark_or_binding_mismatches,
};

/// Source component shared by every literal Pattern family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternLiteralPart {
    Body,
    Prefix,
    Suffix,
    Unit,
}

/// Source component of a qualified or shorthand variant head.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VariantPatternHeadPart {
    QualifiedRoot,
    QualifiedSegment { ordinal: u32 },
    DotShorthandMarker,
}

/// Source component of an optional variant payload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VariantPatternPayloadPart {
    Whole,
    OpenDelimiter,
    CloseDelimiter,
}

/// Source component of one record-pattern field.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternFieldPart {
    Whole,
    Name,
    Colon,
    Pattern,
    RestMarker,
    RestBinding,
}

/// Source component of a bracket-sequence rest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternRestPart {
    Whole,
    Marker,
    Binding,
}

/// Typed source role emitted directly by the Pattern grammar transaction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternComponentRole {
    Whole,
    Name,
    MutKeyword,
    Literal(PatternLiteralPart),
    EntityReference(SyntaxIdRefPart),
    VariantHead(VariantPatternHeadPart),
    VariantName,
    VariantPayload(VariantPatternPayloadPart),
    Element { ordinal: u32 },
    RecordPathRoot,
    RecordPathSegment { ordinal: u32 },
    PatternField { field: u32, part: PatternFieldPart },
    SequenceRest(PatternRestPart),
    WholeBindingName,
    NestedPattern,
    TypedBindingColon,
    TypedBindingType,
    Recovery,
}

/// Exact source of one semantic Pattern component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternComponentSource<R> {
    owner: PatternNodePath,
    role: PatternComponentRole,
    range: R,
}

/// Relationship from a typed-binding Pattern to its attached type child.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternTypeChildRelation {
    TypedBinding,
}

/// Exact semantic type projection owned by a Pattern node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatternTypeChildSource {
    owner: PatternNodePath,
    relation: PatternTypeChildRelation,
    tree: u64,
    authored: Arc<AuthoredTypeRef>,
    path: TypeRefNodePath,
}

/// One-to-one source map for every node in an authored Pattern tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternSourceMap<R> {
    nodes: Box<[(PatternNodePath, R)]>,
    components: Box<[PatternComponentSource<R>]>,
    type_children: Box<[PatternTypeChildSource]>,
}

/// Parser-owned semantic Pattern coupled to its exact source map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredPattern {
    value: PatternSyntaxNode,
    source: Box<PatternSourceMap<SourceRange>>,
    binding_sites: Box<[PatternBindingSite]>,
}

/// Invalid relationship between a Pattern tree and its source map.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PatternSourceMapError {
    MissingRoot,
    MissingNode(PatternNodePath),
    ExtraNode(PatternNodePath),
    DuplicateNode(PatternNodePath),
    ChildOutsideParent(PatternNodePath),
    ChildOutOfOrder(PatternNodePath),
    DuplicateComponent {
        owner: PatternNodePath,
        role: PatternComponentRole,
    },
    MissingComponent {
        owner: PatternNodePath,
        role: PatternComponentRole,
    },
    ExtraComponent {
        owner: PatternNodePath,
        role: PatternComponentRole,
    },
    ComponentOutsideOwner {
        owner: PatternNodePath,
        role: PatternComponentRole,
    },
    ComponentOutOfOrder {
        owner: PatternNodePath,
        role: PatternComponentRole,
    },
    MissingTypeChild(PatternNodePath),
    ExtraTypeChild(PatternNodePath),
    DuplicateTypeChild(PatternNodePath),
    TypeChildOutsideOwner(PatternNodePath),
    BindingOrdinalOverflow,
}

impl<R> PatternComponentSource<R> {
    pub(crate) const fn new(owner: PatternNodePath, role: PatternComponentRole, range: R) -> Self {
        Self { owner, role, range }
    }

    pub const fn owner(&self) -> &PatternNodePath {
        &self.owner
    }

    pub const fn role(&self) -> PatternComponentRole {
        self.role
    }

    pub const fn range(&self) -> &R {
        &self.range
    }
}

impl PatternTypeChildSource {
    pub(crate) const fn new(
        owner: PatternNodePath,
        relation: PatternTypeChildRelation,
        tree: u64,
        authored: Arc<AuthoredTypeRef>,
        path: TypeRefNodePath,
    ) -> Self {
        Self {
            owner,
            relation,
            tree,
            authored,
            path,
        }
    }

    pub(crate) const fn owner(&self) -> &PatternNodePath {
        &self.owner
    }

    pub(crate) const fn relation(&self) -> PatternTypeChildRelation {
        self.relation
    }

    pub(crate) const fn tree(&self) -> u64 {
        self.tree
    }

    pub(crate) const fn authored(&self) -> &Arc<AuthoredTypeRef> {
        &self.authored
    }

    pub(crate) const fn path(&self) -> &TypeRefNodePath {
        &self.path
    }
}

impl<R> PatternSourceMap<R> {
    pub fn nodes(&self) -> &[(PatternNodePath, R)] {
        &self.nodes
    }

    pub fn components(&self) -> &[PatternComponentSource<R>] {
        &self.components
    }

    pub fn source_at(&self, path: &PatternNodePath) -> Option<&R> {
        self.nodes
            .binary_search_by(|(candidate, _)| candidate.cmp(path))
            .ok()
            .map(|index| &self.nodes[index].1)
    }

    pub fn component_at(&self, owner: &PatternNodePath, role: PatternComponentRole) -> Option<&R> {
        self.components
            .binary_search_by(|component| (component.owner(), component.role()).cmp(&(owner, role)))
            .ok()
            .map(|index| self.components[index].range())
    }
}

impl PatternSourceMap<SourceRange> {
    fn try_new(
        value: &PatternSyntaxNode,
        nodes: Vec<(PatternNodePath, SourceRange)>,
        mut components: Vec<PatternComponentSource<SourceRange>>,
        type_children: Vec<PatternTypeChildSource>,
    ) -> Result<Self, PatternSourceMapError> {
        let ordered_nodes = validated_nodes(value, nodes)?;
        validate_component_source_order(&components)?;
        components
            .sort_by(|left, right| (left.owner(), left.role()).cmp(&(right.owner(), right.role())));
        validate_components(value, &ordered_nodes, &components)?;
        let type_children = validate_type_children(value, &ordered_nodes, type_children)?;
        Ok(Self {
            nodes: ordered_nodes
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            components: components.into_boxed_slice(),
            type_children: type_children.into_boxed_slice(),
        })
    }

    pub(crate) fn type_child_at(
        &self,
        owner: &PatternNodePath,
        relation: PatternTypeChildRelation,
    ) -> Option<&PatternTypeChildSource> {
        self.type_children
            .binary_search_by(|child| (child.owner(), child.relation()).cmp(&(owner, relation)))
            .ok()
            .map(|index| &self.type_children[index])
    }
}

impl AuthoredPattern {
    pub(crate) fn try_new(
        value: PatternSyntaxNode,
        nodes: Vec<(PatternNodePath, SourceRange)>,
        components: Vec<PatternComponentSource<SourceRange>>,
        type_children: Vec<PatternTypeChildSource>,
    ) -> Result<Self, PatternSourceMapError> {
        let mut value = value;
        mark_or_binding_mismatches(&mut value)?;
        let source = PatternSourceMap::try_new(&value, nodes, components, type_children)?;
        let mut binding_sites = Vec::new();
        let mut next_ordinal = 0_u32;
        collect_binding_sites(
            &value,
            &PatternNodePath::root(),
            &mut binding_sites,
            &mut next_ordinal,
        )?;
        Ok(Self {
            value,
            source: Box::new(source),
            binding_sites: binding_sites.into_boxed_slice(),
        })
    }

    pub const fn value(&self) -> &PatternSyntaxNode {
        &self.value
    }

    pub const fn source(&self) -> &PatternSourceMap<SourceRange> {
        &self.source
    }

    pub fn binding_sites(&self) -> &[PatternBindingSite] {
        &self.binding_sites
    }

    pub fn value_at(&self, path: &PatternNodePath) -> Option<&PatternSyntaxNode> {
        value_at_path(&self.value, path)
    }

    pub(crate) fn rebase_with_type_children(
        &mut self,
        base: usize,
        mut rebase_type: impl FnMut(&Arc<AuthoredTypeRef>) -> Option<Arc<AuthoredTypeRef>>,
    ) -> Option<()> {
        if base == 0 {
            return Some(());
        }
        for (_, range) in &mut self.source.nodes {
            *range = rebase_range(*range, base);
        }
        for component in &mut self.source.components {
            component.range = rebase_range(component.range, base);
        }
        for child in &mut self.source.type_children {
            child.authored = rebase_type(&child.authored)?;
        }
        Some(())
    }
}

fn validated_nodes(
    value: &PatternSyntaxNode,
    nodes: Vec<(PatternNodePath, SourceRange)>,
) -> Result<BTreeMap<PatternNodePath, SourceRange>, PatternSourceMapError> {
    let mut ordered = BTreeMap::new();
    for (path, range) in nodes {
        if ordered.insert(path.clone(), range).is_some() {
            return Err(PatternSourceMapError::DuplicateNode(path));
        }
    }
    let root = PatternNodePath::root();
    if !ordered.contains_key(&root) {
        return Err(PatternSourceMapError::MissingRoot);
    }
    let mut expected_order = Vec::new();
    collect_expected_paths(value, &root, &mut expected_order)?;
    let expected = expected_order.iter().cloned().collect::<BTreeSet<_>>();
    for path in &expected_order {
        if !ordered.contains_key(path) {
            return Err(PatternSourceMapError::MissingNode(path.clone()));
        }
    }
    for path in ordered.keys() {
        if !expected.contains(path) {
            return Err(PatternSourceMapError::ExtraNode(path.clone()));
        }
    }
    for path in expected_order.iter().skip(1) {
        let parent = path
            .parent()
            .expect("non-root expected Pattern paths have a parent");
        if !contains(ordered[&parent], ordered[path]) {
            return Err(PatternSourceMapError::ChildOutsideParent(path.clone()));
        }
    }
    let mut previous_sibling_start = BTreeMap::<PatternNodePath, usize>::new();
    for path in expected_order.iter().skip(1) {
        let parent = path
            .parent()
            .expect("non-root expected Pattern paths have a parent");
        let start = ordered[path].start();
        if previous_sibling_start
            .insert(parent, start)
            .is_some_and(|previous| start < previous)
        {
            return Err(PatternSourceMapError::ChildOutOfOrder(path.clone()));
        }
    }
    Ok(ordered)
}

fn collect_expected_paths(
    value: &PatternSyntaxNode,
    path: &PatternNodePath,
    output: &mut Vec<PatternNodePath>,
) -> Result<(), PatternSourceMapError> {
    output.push(path.clone());
    match value.kind() {
        PatternSyntaxKind::Variant(variant) => match variant.payload() {
            PatternVariantPayloadSyntax::Resolved(child)
            | PatternVariantPayloadSyntax::Recovered {
                value: Some(child), ..
            } => {
                collect_expected_paths(
                    child,
                    &path.child(PatternNodeStep::VariantPayload),
                    output,
                )?;
            }
            PatternVariantPayloadSyntax::Recovered { value: None, .. }
            | PatternVariantPayloadSyntax::Absent => {}
        },
        PatternSyntaxKind::Tuple(items) | PatternSyntaxKind::Or(items) => {
            collect_elements(items, path, output)?;
        }
        PatternSyntaxKind::Record(record) => {
            for (index, field) in record.fields().iter().enumerate() {
                if let PatternRecordFieldSyntax::Explicit { pattern, .. } = field {
                    let ordinal = u32::try_from(index)
                        .map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
                    collect_expected_paths(
                        pattern,
                        &path.child(PatternNodeStep::RecordField(ordinal)),
                        output,
                    )?;
                }
            }
        }
        PatternSyntaxKind::BracketSequence(sequence) => {
            collect_elements(sequence.elements(), path, output)?;
        }
        PatternSyntaxKind::WholeBinding { pattern, .. } => {
            collect_expected_paths(pattern, &path.child(PatternNodeStep::NestedPattern), output)?;
        }
        PatternSyntaxKind::Binding(_)
        | PatternSyntaxKind::MutableBinding(_)
        | PatternSyntaxKind::Literal(_)
        | PatternSyntaxKind::EntityReference(_)
        | PatternSyntaxKind::Discard
        | PatternSyntaxKind::TypedBinding(_)
        | PatternSyntaxKind::Error => {}
    }
    Ok(())
}

fn collect_elements(
    items: &[PatternSyntaxNode],
    path: &PatternNodePath,
    output: &mut Vec<PatternNodePath>,
) -> Result<(), PatternSourceMapError> {
    for (index, item) in items.iter().enumerate() {
        let ordinal =
            u32::try_from(index).map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
        collect_expected_paths(item, &path.child(PatternNodeStep::Element(ordinal)), output)?;
    }
    Ok(())
}

fn validate_component_source_order(
    components: &[PatternComponentSource<SourceRange>],
) -> Result<(), PatternSourceMapError> {
    let mut previous = BTreeMap::<PatternNodePath, usize>::new();
    for component in components {
        if component.role == PatternComponentRole::Whole {
            continue;
        }
        if previous
            .insert(component.owner.clone(), component.range.start())
            .is_some_and(|start| component.range.start() < start)
        {
            return Err(PatternSourceMapError::ComponentOutOfOrder {
                owner: component.owner.clone(),
                role: component.role,
            });
        }
    }
    Ok(())
}

fn validate_components(
    value: &PatternSyntaxNode,
    nodes: &BTreeMap<PatternNodePath, SourceRange>,
    components: &[PatternComponentSource<SourceRange>],
) -> Result<(), PatternSourceMapError> {
    let mut actual = BTreeSet::new();
    for component in components {
        if !actual.insert((component.owner.clone(), component.role)) {
            return Err(PatternSourceMapError::DuplicateComponent {
                owner: component.owner.clone(),
                role: component.role,
            });
        }
        if nodes
            .get(component.owner())
            .is_none_or(|owner| !contains(*owner, component.range))
        {
            return Err(PatternSourceMapError::ComponentOutsideOwner {
                owner: component.owner.clone(),
                role: component.role,
            });
        }
    }
    let mut expected = BTreeSet::new();
    collect_expected_components(value, &PatternNodePath::root(), &mut expected)?;
    if let Some((owner, role)) = expected.difference(&actual).next() {
        return Err(PatternSourceMapError::MissingComponent {
            owner: owner.clone(),
            role: *role,
        });
    }
    if let Some((owner, role)) = actual.difference(&expected).next() {
        return Err(PatternSourceMapError::ExtraComponent {
            owner: owner.clone(),
            role: *role,
        });
    }
    Ok(())
}

fn collect_expected_components(
    value: &PatternSyntaxNode,
    path: &PatternNodePath,
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
) -> Result<(), PatternSourceMapError> {
    insert(output, path, PatternComponentRole::Whole);
    match value.kind() {
        PatternSyntaxKind::Binding(_) => insert(output, path, PatternComponentRole::Name),
        PatternSyntaxKind::MutableBinding(_) => {
            insert(output, path, PatternComponentRole::MutKeyword);
            insert(output, path, PatternComponentRole::Name);
        }
        PatternSyntaxKind::Literal(literal) => literal_components(path, literal, output),
        PatternSyntaxKind::EntityReference(reference) => {
            id_components(path, reference, output)?;
        }
        PatternSyntaxKind::Variant(variant) => {
            variant_components(path, variant, output)?;
        }
        PatternSyntaxKind::Discard => {}
        PatternSyntaxKind::Tuple(items) | PatternSyntaxKind::Or(items) => {
            element_components(path, items, output)?;
        }
        PatternSyntaxKind::Record(record) => {
            path_components(path, record.path(), false, output)?;
            for (index, field) in record.fields().iter().enumerate() {
                let ordinal = u32::try_from(index)
                    .map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
                record_field_components(path, ordinal, field, output);
                if let PatternRecordFieldSyntax::Explicit { pattern, .. } = field {
                    collect_expected_components(
                        pattern,
                        &path.child(PatternNodeStep::RecordField(ordinal)),
                        output,
                    )?;
                }
            }
        }
        PatternSyntaxKind::BracketSequence(sequence) => {
            element_components(path, sequence.elements(), output)?;
            if !matches!(sequence.rest(), PatternSequenceRestSyntax::Absent) {
                insert(
                    output,
                    path,
                    PatternComponentRole::SequenceRest(PatternRestPart::Whole),
                );
                insert(
                    output,
                    path,
                    PatternComponentRole::SequenceRest(PatternRestPart::Marker),
                );
                if sequence.rest().binding().is_some() {
                    insert(
                        output,
                        path,
                        PatternComponentRole::SequenceRest(PatternRestPart::Binding),
                    );
                }
            }
        }
        PatternSyntaxKind::WholeBinding { pattern, .. } => {
            insert(output, path, PatternComponentRole::WholeBindingName);
            insert(output, path, PatternComponentRole::NestedPattern);
            collect_expected_components(
                pattern,
                &path.child(PatternNodeStep::NestedPattern),
                output,
            )?;
        }
        PatternSyntaxKind::TypedBinding(_) => {
            insert(output, path, PatternComponentRole::Name);
            insert(output, path, PatternComponentRole::TypedBindingColon);
            insert(output, path, PatternComponentRole::TypedBindingType);
        }
        PatternSyntaxKind::Error => insert(output, path, PatternComponentRole::Recovery),
    }
    Ok(())
}

fn literal_components(
    path: &PatternNodePath,
    literal: &SyntaxLiteralSyntax,
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
) {
    insert(
        output,
        path,
        PatternComponentRole::Literal(PatternLiteralPart::Body),
    );
    if literal.shape().has_prefix() {
        insert(
            output,
            path,
            PatternComponentRole::Literal(PatternLiteralPart::Prefix),
        );
    }
    if literal.shape().has_suffix() {
        insert(
            output,
            path,
            PatternComponentRole::Literal(PatternLiteralPart::Suffix),
        );
    }
    if literal.shape().has_unit() {
        insert(
            output,
            path,
            PatternComponentRole::Literal(PatternLiteralPart::Unit),
        );
    }
}

fn id_components(
    path: &PatternNodePath,
    reference: &SyntaxIdRefSyntax,
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
) -> Result<(), PatternSourceMapError> {
    insert(
        output,
        path,
        PatternComponentRole::EntityReference(SyntaxIdRefPart::Whole),
    );
    let shape = reference.shape();
    if shape.has_absolute_marker() {
        insert(
            output,
            path,
            PatternComponentRole::EntityReference(SyntaxIdRefPart::AbsoluteMarker),
        );
    }
    if shape.has_family() {
        insert(
            output,
            path,
            PatternComponentRole::EntityReference(SyntaxIdRefPart::Family),
        );
        insert(
            output,
            path,
            PatternComponentRole::EntityReference(SyntaxIdRefPart::FamilySeparator),
        );
    }
    for ordinal in 0..shape.parent_depth() {
        let ordinal =
            u32::try_from(ordinal).map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
        insert(
            output,
            path,
            PatternComponentRole::EntityReference(SyntaxIdRefPart::ParentMarker { ordinal }),
        );
    }
    for ordinal in 0..shape.segment_count() {
        insert(
            output,
            path,
            PatternComponentRole::EntityReference(SyntaxIdRefPart::SuffixSegment { ordinal }),
        );
    }
    Ok(())
}

fn variant_components(
    path: &PatternNodePath,
    variant: &super::PatternVariantSyntax,
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
) -> Result<(), PatternSourceMapError> {
    match variant.head() {
        PatternVariantHeadSyntax::Resolved(PatternVariantHead::Unqualified(
            PatternUnqualifiedVariantForm::DotShorthand,
        )) => insert(
            output,
            path,
            PatternComponentRole::VariantHead(VariantPatternHeadPart::DotShorthandMarker),
        ),
        PatternVariantHeadSyntax::Resolved(PatternVariantHead::Qualified(head)) => {
            resolved_path_components(path, head.root(), head.segments().len(), true, output)?;
        }
        PatternVariantHeadSyntax::Resolved(PatternVariantHead::Unqualified(
            PatternUnqualifiedVariantForm::BareExpectedType,
        ))
        | PatternVariantHeadSyntax::Absent => {}
        PatternVariantHeadSyntax::Recovered(recovery) => {
            recovered_path_components(path, recovery, true, output)?;
        }
    }
    insert(output, path, PatternComponentRole::VariantName);
    match variant.payload() {
        PatternVariantPayloadSyntax::Resolved(child)
        | PatternVariantPayloadSyntax::Recovered {
            value: Some(child), ..
        } => {
            payload_components(path, output);
            collect_expected_components(
                child,
                &path.child(PatternNodeStep::VariantPayload),
                output,
            )?;
        }
        PatternVariantPayloadSyntax::Recovered { value: None, .. } => {
            payload_components(path, output);
        }
        PatternVariantPayloadSyntax::Absent => {}
    }
    Ok(())
}

fn payload_components(
    path: &PatternNodePath,
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
) {
    for part in [
        VariantPatternPayloadPart::Whole,
        VariantPatternPayloadPart::OpenDelimiter,
        VariantPatternPayloadPart::CloseDelimiter,
    ] {
        insert(output, path, PatternComponentRole::VariantPayload(part));
    }
}

fn path_components(
    path: &PatternNodePath,
    value: &PatternPathSyntax,
    variant: bool,
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
) -> Result<(), PatternSourceMapError> {
    match value {
        PatternPathSyntax::Resolved(value) => {
            resolved_path_components(path, value.root(), value.segments().len(), variant, output)?;
        }
        PatternPathSyntax::Recovered(value) => {
            recovered_path_components(path, value, variant, output)?;
        }
        PatternPathSyntax::Absent => {}
    }
    Ok(())
}

fn resolved_path_components(
    path: &PatternNodePath,
    root: PatternPathRoot,
    segment_count: usize,
    variant: bool,
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
) -> Result<(), PatternSourceMapError> {
    if !matches!(root, PatternPathRoot::ImplicitCrate) {
        insert(
            output,
            path,
            if variant {
                PatternComponentRole::VariantHead(VariantPatternHeadPart::QualifiedRoot)
            } else {
                PatternComponentRole::RecordPathRoot
            },
        );
    }
    for index in 0..segment_count {
        let ordinal =
            u32::try_from(index).map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
        insert(
            output,
            path,
            if variant {
                PatternComponentRole::VariantHead(VariantPatternHeadPart::QualifiedSegment {
                    ordinal,
                })
            } else {
                PatternComponentRole::RecordPathSegment { ordinal }
            },
        );
    }
    Ok(())
}

fn recovered_path_components(
    path: &PatternNodePath,
    value: &super::PatternPathRecovery,
    variant: bool,
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
) -> Result<(), PatternSourceMapError> {
    resolved_path_components(
        path,
        value.root().unwrap_or(PatternPathRoot::ImplicitCrate),
        value.segments().len(),
        variant,
        output,
    )
}

fn element_components(
    path: &PatternNodePath,
    values: &[PatternSyntaxNode],
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
) -> Result<(), PatternSourceMapError> {
    for (index, value) in values.iter().enumerate() {
        let ordinal =
            u32::try_from(index).map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
        insert(output, path, PatternComponentRole::Element { ordinal });
        collect_expected_components(
            value,
            &path.child(PatternNodeStep::Element(ordinal)),
            output,
        )?;
    }
    Ok(())
}

fn record_field_components(
    path: &PatternNodePath,
    ordinal: u32,
    field: &PatternRecordFieldSyntax,
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
) {
    match field {
        PatternRecordFieldSyntax::Explicit { .. } => {
            field_shape_components(path, ordinal, PatternRecordFieldShape::explicit(), output);
        }
        PatternRecordFieldSyntax::Shorthand(_) => {
            field_shape_components(path, ordinal, PatternRecordFieldShape::shorthand(), output);
        }
        PatternRecordFieldSyntax::Rest(binding) => field_shape_components(
            path,
            ordinal,
            PatternRecordFieldShape::rest(binding.is_some()),
            output,
        ),
        PatternRecordFieldSyntax::Invalid(invalid) => {
            field_shape_components(path, ordinal, invalid.shape(), output);
        }
    }
}

fn field_shape_components(
    path: &PatternNodePath,
    field: u32,
    shape: PatternRecordFieldShape,
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
) {
    insert_field(output, path, field, PatternFieldPart::Whole);
    if shape.name() {
        insert_field(output, path, field, PatternFieldPart::Name);
    }
    if shape.colon() {
        insert_field(output, path, field, PatternFieldPart::Colon);
    }
    if shape.pattern() {
        insert_field(output, path, field, PatternFieldPart::Pattern);
    }
    if shape.rest_marker() {
        insert_field(output, path, field, PatternFieldPart::RestMarker);
    }
    if shape.rest_binding() {
        insert_field(output, path, field, PatternFieldPart::RestBinding);
    }
}

fn insert_field(
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
    path: &PatternNodePath,
    field: u32,
    part: PatternFieldPart,
) {
    insert(
        output,
        path,
        PatternComponentRole::PatternField { field, part },
    );
}

fn insert(
    output: &mut BTreeSet<(PatternNodePath, PatternComponentRole)>,
    path: &PatternNodePath,
    role: PatternComponentRole,
) {
    output.insert((path.clone(), role));
}

fn validate_type_children(
    value: &PatternSyntaxNode,
    nodes: &BTreeMap<PatternNodePath, SourceRange>,
    mut children: Vec<PatternTypeChildSource>,
) -> Result<Vec<PatternTypeChildSource>, PatternSourceMapError> {
    children.sort_by(|left, right| {
        (left.owner(), left.relation()).cmp(&(right.owner(), right.relation()))
    });
    let mut actual = BTreeSet::new();
    for child in &children {
        if !actual.insert((child.owner.clone(), child.relation)) {
            return Err(PatternSourceMapError::DuplicateTypeChild(
                child.owner.clone(),
            ));
        }
        let Some(owner) = nodes.get(child.owner()) else {
            return Err(PatternSourceMapError::ExtraTypeChild(child.owner.clone()));
        };
        if !matches!(
            value_at_path(value, child.owner()).map(PatternSyntaxNode::kind),
            Some(PatternSyntaxKind::TypedBinding(_))
        ) || child.authored.value_at(&child.path).is_none()
            || !contains_text_range(*owner, child.authored.root_source().whole())
        {
            return Err(PatternSourceMapError::TypeChildOutsideOwner(
                child.owner.clone(),
            ));
        }
    }
    let mut expected = BTreeSet::new();
    collect_typed_binding_paths(value, &PatternNodePath::root(), &mut expected)?;
    let actual_paths = actual
        .iter()
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    if let Some(path) = expected.difference(&actual_paths).next() {
        return Err(PatternSourceMapError::MissingTypeChild(path.clone()));
    }
    if let Some((path, _)) = actual.iter().find(|(path, _)| !expected.contains(path)) {
        return Err(PatternSourceMapError::ExtraTypeChild(path.clone()));
    }
    Ok(children)
}

fn collect_typed_binding_paths(
    value: &PatternSyntaxNode,
    path: &PatternNodePath,
    output: &mut BTreeSet<PatternNodePath>,
) -> Result<(), PatternSourceMapError> {
    if matches!(value.kind(), PatternSyntaxKind::TypedBinding(_)) {
        output.insert(path.clone());
    }
    let mut paths = Vec::new();
    collect_expected_paths(value, path, &mut paths)?;
    for child in paths.into_iter().skip(1) {
        if matches!(
            value_at_path(value, &child).map(PatternSyntaxNode::kind),
            Some(PatternSyntaxKind::TypedBinding(_))
        ) {
            output.insert(child);
        }
    }
    Ok(())
}

fn value_at_path<'a>(
    root: &'a PatternSyntaxNode,
    path: &PatternNodePath,
) -> Option<&'a PatternSyntaxNode> {
    let mut value = root;
    for step in path.steps() {
        value = match (value.kind(), step) {
            (PatternSyntaxKind::Variant(variant), PatternNodeStep::VariantPayload) => {
                match variant.payload() {
                    PatternVariantPayloadSyntax::Resolved(child)
                    | PatternVariantPayloadSyntax::Recovered {
                        value: Some(child), ..
                    } => child,
                    PatternVariantPayloadSyntax::Recovered { value: None, .. }
                    | PatternVariantPayloadSyntax::Absent => return None,
                }
            }
            (
                PatternSyntaxKind::Tuple(items) | PatternSyntaxKind::Or(items),
                PatternNodeStep::Element(index),
            ) => items.get(usize::try_from(*index).ok()?)?,
            (PatternSyntaxKind::BracketSequence(sequence), PatternNodeStep::Element(index)) => {
                sequence.elements().get(usize::try_from(*index).ok()?)?
            }
            (PatternSyntaxKind::Record(record), PatternNodeStep::RecordField(index)) => {
                match record.fields().get(usize::try_from(*index).ok()?)? {
                    PatternRecordFieldSyntax::Explicit { pattern, .. } => pattern,
                    PatternRecordFieldSyntax::Shorthand(_)
                    | PatternRecordFieldSyntax::Rest(_)
                    | PatternRecordFieldSyntax::Invalid(_) => return None,
                }
            }
            (PatternSyntaxKind::WholeBinding { pattern, .. }, PatternNodeStep::NestedPattern) => {
                pattern
            }
            _ => return None,
        };
    }
    Some(value)
}

const fn contains(parent: SourceRange, child: SourceRange) -> bool {
    parent.start() <= child.start() && child.end() <= parent.end()
}

fn contains_text_range(parent: SourceRange, child: &crate::ast::common::TextRange) -> bool {
    parent.start() <= child.start() && child.end() <= parent.end()
}

fn rebase_range(range: SourceRange, base: usize) -> SourceRange {
    SourceRange::new(range.start() + base, range.end() + base)
}
