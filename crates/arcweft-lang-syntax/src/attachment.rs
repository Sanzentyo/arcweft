//! Snapshot-bound typed syntax handles and exact source identities.

pub mod access;
mod action;
mod activity;
mod callable;
mod choice;
mod declaration;
mod dialogue_plan;
mod entry;
mod error;
mod expression;
mod extern_capability;
pub mod family;
mod flow;
pub mod item;
mod item_prefix;
mod layer;
mod metric;
pub mod node;
mod nominal;
mod pattern;
mod resource;
mod signal;
mod snapshot;
pub mod source_file;
mod statement;
mod style;
mod test_bench;
mod thread_body;
mod thread_statement;
mod trait_impl;
mod trigger;
mod type_ref;
mod view;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use arcweft_source::SourceDocument;

pub use crate::patterns::{
    PatternComponentRole, PatternFieldPart, PatternLiteralPart, PatternRestPart,
    VariantPatternHeadPart, VariantPatternPayloadPart,
};
pub use access::{
    BlockTailNode, DeclarationBodyNode, IfStatementElseNode, IfStatementHeadNode,
    LetInitializerNode, MatchStatementArmBodyNode, MatchStatementBodyNode,
    MatchStatementExpressionNode, RequiredStatementExpressionNode, UnsafeAuditBodyNode,
    UnsafeAuditIdNode, UnsafeAuditReasonNode,
};
pub use action::{
    AttachedActionDeclaration, AttachedActionForbiddenDefault, AttachedActionParameter,
    AttachedActionSignature, AttachedActionTrailingRecovery,
};
pub use activity::{
    AttachedActivityBody, AttachedActivityContractBody, AttachedActivityContractClause,
    AttachedActivityContractCondition, AttachedActivityContractEntry,
    AttachedActivityContractMember, AttachedActivityDeclaration, AttachedActivityEntry,
    AttachedActivityInputMember, AttachedActivityLifecycle, AttachedActivityLifecycleMember,
    AttachedActivityMode, AttachedActivityModeMember, AttachedActivityOutputMember,
    AttachedActivityPort, AttachedActivityPortBody, AttachedActivitySectionState,
};
pub use callable::{
    AttachedAssertionMode, AttachedAssertionStatement, AttachedCallableContractClause,
    AttachedCallableParameter, AttachedCallableParameterDefault, AttachedCallableParameterKind,
    AttachedCallableReturn, AttachedFixedParameterGroup, AttachedFunctionBody,
    AttachedFunctionDeclaration, AttachedMethodParameter, AttachedMethodParameterGroup,
    AttachedMethodReceiver, AttachedMethodReceiverKind, AttachedPredicateBody,
    AttachedPredicateDeclaration, AttachedPredicateReturnRecovery, AttachedProofBody,
    AttachedProofDeclaration, ProofTrustSyntax, TrustReasonSyntax, TrustReasonSyntaxError,
};
pub use choice::{
    AttachedChoiceBody, AttachedChoiceCompactAction, AttachedChoiceCompactArm,
    AttachedChoiceEntityReference, AttachedChoiceExpression, AttachedChoiceFor, AttachedChoiceIf,
    AttachedChoiceIfBranch, AttachedChoiceItem, AttachedChoiceMatch, AttachedChoiceMatchArm,
    AttachedChoiceMatchArmBody, AttachedChoiceMatchBody, AttachedChoiceOption,
    AttachedChoiceOptionBody, AttachedChoiceOptionField, AttachedChoiceOptionFor,
    AttachedChoicePlan, AttachedChoicePlanAssignment, AttachedChoicePlanBody,
    AttachedChoicePlanCancel, AttachedChoicePlanItem, AttachedChoicePlanKey,
    AttachedChoicePlanOnSelect, AttachedChoicePlanTimeout, AttachedChoiceSelect,
    AttachedChoiceStatement, AttachedChoiceSuiteSource, AttachedChoiceView, AttachedChoiceViewBody,
    AttachedChoiceViewEntry, AttachedLetChoiceStatement, AttachedRequiredChoiceBody,
    AttachedRequiredChoiceEntityReference, AttachedRequiredChoiceMatchBody,
    AttachedRequiredChoiceOptionBody, AttachedRequiredChoicePlanBody,
    AttachedRequiredChoiceViewBody,
};
pub use declaration::{
    AttachedCharacterAssignment, AttachedCharacterBody, AttachedCharacterDeclaration,
    AttachedCharacterDisplayNameMember, AttachedCharacterInitializer, AttachedCharacterMember,
    AttachedCharacterSurfaceAlias, AttachedDeclarationIdentity, AttachedDeclarationPublicId,
    AttachedDeclarationPublicIdIssue, AttachedRetainedHeader, AttachedRetainedName,
};
pub use dialogue_plan::{AttachedDialogueLinePlan, AttachedDialogueLinePlanBody};
pub use entry::{
    AttachedEntryBody, AttachedEntryDeclaration, AttachedEntryHttpMethod, AttachedEntryId,
    AttachedEntryKind, AttachedEntryMember, AttachedEntryName, AttachedEntryPunctuation,
    AttachedEntryRoleBinding, AttachedEntryRouteBinding, AttachedEntryRouteBindings,
    AttachedEntryValue,
};
pub use error::{AttachmentFailure, SyntaxAccessError, SyntaxLookupError};
pub use expression::{
    AttachedAwaitBranch, AttachedAwaitBranchBlock, AttachedAwaitBranchBody, AttachedCallTypeChild,
    AttachedCandidateAssertion, AttachedCandidateAssertionProjection, AttachedCandidateAssignment,
    AttachedCandidateBlockTail, AttachedCandidateClosure, AttachedCandidateClosureParameter,
    AttachedCandidateControlLabel, AttachedCandidateDialogueExpression,
    AttachedCandidateDialogueOwner, AttachedCandidateExpressionChild, AttachedCandidateGraph,
    AttachedCandidateIf, AttachedCandidateIfElse, AttachedCandidateIfHead, AttachedCandidateIfLet,
    AttachedCandidateKeywordStatement, AttachedCandidateMatch, AttachedCandidateMatchArm,
    AttachedCandidateMatchArmBody, AttachedCandidateMatchArmStatement, AttachedCandidateMatchBody,
    AttachedCandidateMatchStatement, AttachedCandidateNode, AttachedCandidateNominalTypeRoot,
    AttachedCandidatePathExpression, AttachedCandidatePathProjection, AttachedCandidatePathSegment,
    AttachedCandidatePatternChild, AttachedCandidatePatternProjection,
    AttachedCandidateRequiredOperand, AttachedCandidateStatement, AttachedCandidateStatementBlock,
    AttachedCandidateStatementExpression, AttachedCandidateTypeChild,
    AttachedCandidateTypeProjection, AttachedCandidateTypeRoot, AttachedCandidateUnsafeAuditId,
    AttachedCandidateUnsafeBody, AttachedCandidateUnsafeLifetime, AttachedCandidateValueBlock,
    AttachedClosureParameter, AttachedExpressionChild, AttachedExpressionComponent,
    AttachedExpressionNode, AttachedMatchArm, AttachedMatchArmComponent,
    AttachedMatchArmExpression,
};
pub use extern_capability::{
    AttachedCapabilityAssociatedType, AttachedCapabilityEffects, AttachedCapabilityFunction,
    AttachedCapabilityMember, AttachedExternCapabilityBody, AttachedExternCapabilityDeclaration,
};
pub use family::{
    AstNodeFamily, AttributeNode, BodyNode, DeclarationPartNode, DelimiterNode, ExprNode, NameNode,
    PathNode, PatternNode, RecoveryNode, RichTextNode, StatementNode, TypeNode,
};
pub use flow::{
    AttachedFlowContractClause, AttachedFlowContractCondition, AttachedFlowContractList,
    AttachedFlowContractMode, AttachedFlowContractOperands, AttachedFlowDeclaration,
    AttachedFlowIdComponent, AttachedFlowIdSyntax, AttachedFlowIdentity, AttachedFlowName,
    AttachedFlowPublicId, AttachedFlowReturnSyntax, AttachedFlowSignature,
    AttachedFlowSignatureRecovery, AttachedRequiredFlowBody,
};
pub use item::TypedItemNode;
pub use item_prefix::{
    AttachedAttributeArgument, AttachedAttributeComponent, AttachedAttributeValue,
    AttachedDocumentation, AttachedInnerAttribute, AttachedItemPrefix, AttachedOuterAttribute,
    AttachedOuterAttributeForm, AttachedOuterAttributeIssue,
};
pub use layer::{
    AttachedLayerBody, AttachedLayerDeclaration, AttachedLayerEntry, AttachedLayerExpression,
    AttachedLayerKind, AttachedLayerMember, AttachedLayerMemberState, AttachedLayerPolicy,
    AttachedLayerReference,
};
pub use metric::{
    AttachedMetricBody, AttachedMetricBucketsMember, AttachedMetricBucketsValue,
    AttachedMetricDeclaration, AttachedMetricEntry, AttachedMetricKind, AttachedMetricLabel,
    AttachedMetricLabelsBody, AttachedMetricLabelsMember, AttachedMetricMemberState,
    AttachedMetricUnitMember, AttachedMetricUnitValue,
};
pub use node::{
    AstKind, AstNode, ExactAstKind, ExpressionFragmentRootKind, PatternFragmentRootKind,
    SourceFileKind, StatementFragmentRootKind, TypeFragmentRootKind,
};
#[cfg(test)]
pub(crate) use node::{PredicateItemKind, ProofItemKind};
pub use nominal::{
    AttachedEnumBody, AttachedEnumDeclaration, AttachedEnumVariant, AttachedGenericParameter,
    AttachedGenericParameterGroup, AttachedNominalDeclaration, AttachedNominalFieldPrefix,
    AttachedRequiredName, AttachedRequiredPunctuation, AttachedStructBody,
    AttachedStructDeclaration, AttachedStructField, AttachedTypeAliasDeclaration,
    AttachedWhereClause, AttachedWherePredicate,
};
pub use pattern::{AttachedPatternChild, AttachedPatternComponent, AttachedPatternNode};
pub use resource::{
    AttachedResourceBody, AttachedResourceDeclaration, AttachedResourceField,
    AttachedResourceInitializer, AttachedResourcePublicId, AttachedResourcePublicIdIssue,
};
pub use signal::AttachedSignalDeclaration;
pub(crate) use snapshot::SyntaxSnapshotData;
pub use snapshot::{
    SyntaxDatabaseId, SyntaxLanguage, SyntaxLineageId, SyntaxNode, SyntaxNodeHandle, SyntaxNodeId,
    SyntaxSnapshotId,
};
pub use source_file::{AttachedPath, AttachedPathRoot};
pub use statement::{
    AttachedBreakStatement, AttachedContinueStatement, AttachedControlLabel,
    AttachedDeferStatement, AttachedGotoStatement, AttachedOutStatement, AttachedSignalStatement,
};
pub use style::{
    AttachedStyleAssignment, AttachedStyleAssignmentState, AttachedStyleBody,
    AttachedStyleDeclaration, AttachedStyleEnvironment, AttachedStyleEnvironmentClause,
    AttachedStyleEnvironmentComparison, AttachedStyleEnvironmentCondition,
    AttachedStyleEnvironmentConditionRecovery, AttachedStyleEnvironmentField,
    AttachedStyleExpression, AttachedStyleId, AttachedStyleMember, AttachedStyleName,
    AttachedStylePredicate, AttachedStyleProperty, AttachedStyleRule, AttachedStyleRuleBody,
    AttachedStyleSelector, AttachedStyleSelectorPart, AttachedStyleSelectorRelation,
    AttachedStyleSelectorSequence, AttachedStyleToken, AttachedStyleTypeAnnotation,
    StyleEnvironmentComparisonKind, StyleEnvironmentConditionIssue, StyleEnvironmentFieldKind,
    StyleIdForm, StylePropertyOperation, StyleSelectorRelation, StyleSyntaxName,
    StyleSyntaxNameIssue,
};
pub use test_bench::{
    AttachedBenchDeclaration, AttachedPlanBody, AttachedPlanId, AttachedTestDeclaration,
    AttachedTestKind,
};
pub use thread_body::{
    AttachedFlowStatementBody, AttachedNestedThreadFlowBody, AttachedRequiredNestedThreadFlowBody,
    AttachedRequiredThreadExpressionBody, AttachedThreadExpressionBody, AttachedThreadFlowItem,
    AttachedThreadFlowItemFamily,
};
pub use thread_statement::{
    AttachedForStatement, AttachedIncludeStatement, AttachedRequiredIncludeTarget,
    AttachedScopeName, AttachedScopeStatement, AttachedSelectBindingName, AttachedSelectBranch,
    AttachedSelectBranchBlock, AttachedSelectStatement, AttachedSelectStatementForm,
    AttachedSourceLocaleStatement, AttachedSourceLocaleValue, AttachedThreadEntityReference,
    AttachedWhileLetStatement, AttachedWhileStatement,
};
pub use trait_impl::{
    AttachedImplAssociatedType, AttachedImplBody, AttachedImplDeclaration, AttachedImplFunction,
    AttachedImplMember, AttachedTraitAssociatedType, AttachedTraitBody, AttachedTraitDeclaration,
    AttachedTraitFunction, AttachedTraitMember,
};
pub use trigger::{
    AttachedExpressionTrigger, AttachedPatternTrigger, AttachedSignalTrigger,
    AttachedTriggerDelimiters, AttachedTriggerPattern,
};
pub use type_ref::{
    AttachedTypeChild, AttachedTypeComponent, AttachedTypeFamily, AttachedTypeRefNode,
};
pub use view::{
    AttachedViewBody, AttachedViewDeclaration, AttachedViewExport, AttachedViewFragment,
    AttachedViewFragmentEntry, AttachedViewPartLocalName, AttachedViewPartModifier,
    AttachedViewPartPath, AttachedViewRequiredKeyword,
};

use crate::grammar::build::{GrammarBuild, GrammarEventPath, UnattachedGrammarEntry};
use crate::grammar::kinds::{AstTag, SyntaxKind, SyntaxRole};

/// Stable grammar identities indexed by exact event path.
#[derive(Clone, Debug, Default)]
pub(crate) struct GrammarIdentityMap {
    by_path: HashMap<GrammarEventPath, SyntaxNodeId>,
}

impl GrammarIdentityMap {
    pub(crate) fn new(by_path: HashMap<GrammarEventPath, SyntaxNodeId>) -> Self {
        Self { by_path }
    }

    pub(crate) fn id_for_path(&self, path: &GrammarEventPath) -> Option<SyntaxNodeId> {
        self.by_path.get(path).copied()
    }

    pub(crate) fn len(&self) -> usize {
        self.by_path.len()
    }

    #[cfg(test)]
    pub(crate) fn remove_path(&mut self, path: &GrammarEventPath) {
        self.by_path.remove(path);
    }
}

/// Builds the immutable node/path/ID attachment for one staged snapshot.
pub(crate) fn attach_typed_tree(
    build: &GrammarBuild,
    identities: &GrammarIdentityMap,
    snapshot: SyntaxSnapshotId,
    document: Arc<SourceDocument>,
) -> Result<Arc<SyntaxSnapshotData>, AttachmentFailure> {
    let root = SyntaxNode::new_root(build.green().clone());
    let inventory =
        AttachmentInventoryBuilder::new(&root, identities, build.index().entries().len())
            .collect(build.index().entries())?;

    let root_id = identities
        .id_for_path(
            build
                .index()
                .entries()
                .first()
                .ok_or(AttachmentFailure::MissingRoot)?
                .path(),
        )
        .ok_or(AttachmentFailure::MissingRoot)?;
    if inventory
        .records
        .get(&root_id)
        .is_none_or(|record| record.kind() != SyntaxKind::SourceFile)
    {
        return Err(AttachmentFailure::MissingRoot);
    }

    let attached = Arc::new(SyntaxSnapshotData::new(
        snapshot,
        document,
        build.green().clone(),
        root_id,
        inventory.records,
        inventory.by_path,
    ));
    validate_snapshot(&attached)?;
    Ok(attached)
}

#[derive(Debug)]
struct AttachmentInventory {
    records: HashMap<SyntaxNodeId, snapshot::AttachedNodeRecord>,
    by_path: BTreeMap<GrammarEventPath, SyntaxNodeId>,
}

#[derive(Debug)]
struct AttachmentInventoryBuilder<'a> {
    root: &'a SyntaxNode,
    identities: &'a GrammarIdentityMap,
    expected_count: usize,
    by_path: BTreeMap<GrammarEventPath, SyntaxNodeId>,
    seen_ids: HashSet<SyntaxNodeId>,
    ancestry: Vec<(GrammarEventPath, SyntaxNodeId, SyntaxKind, SyntaxNode)>,
    pending: Vec<PendingAttachment>,
    children: HashMap<SyntaxNodeId, Vec<SyntaxNodeId>>,
    children_by_role: HashMap<SyntaxNodeId, BTreeMap<SyntaxRole, Vec<SyntaxNodeId>>>,
}

impl<'a> AttachmentInventoryBuilder<'a> {
    fn new(
        root: &'a SyntaxNode,
        identities: &'a GrammarIdentityMap,
        expected_count: usize,
    ) -> Self {
        Self {
            root,
            identities,
            expected_count,
            by_path: BTreeMap::new(),
            seen_ids: HashSet::with_capacity(expected_count),
            ancestry: Vec::new(),
            pending: Vec::with_capacity(expected_count),
            children: HashMap::new(),
            children_by_role: HashMap::new(),
        }
    }

    fn collect(
        mut self,
        entries: &[UnattachedGrammarEntry],
    ) -> Result<AttachmentInventory, AttachmentFailure> {
        let paths = entries
            .iter()
            .map(|entry| entry.path().clone())
            .collect::<HashSet<_>>();
        let mut nodes =
            grammar_nodes_for_paths(self.root, &paths).ok_or(AttachmentFailure::MissingRoot)?;
        for entry in entries {
            let id = self
                .identities
                .id_for_path(entry.path())
                .ok_or(AttachmentFailure::MissingIdentity)?;
            let node = nodes
                .remove(entry.path())
                .ok_or(AttachmentFailure::MissingAttachment { id })?;
            self.attach(entry, id, &node)?;
        }
        self.finish()
    }

    fn attach(
        &mut self,
        entry: &UnattachedGrammarEntry,
        id: SyntaxNodeId,
        node: &SyntaxNode,
    ) -> Result<(), AttachmentFailure> {
        let actual = node.kind();
        let expected = rowan::SyntaxKind(entry.kind() as u16);
        if actual != expected {
            return Err(AttachmentFailure::GrammarKindMismatch {
                id,
                expected: entry.kind(),
                actual,
            });
        }
        let tag = entry
            .kind()
            .ast_tag()
            .ok_or(AttachmentFailure::MissingAstTag {
                id,
                kind: entry.kind(),
            })?;
        while self.ancestry.last().is_some_and(|(candidate, _, _, _)| {
            !strict_path_prefix(candidate.elements(), entry.path().elements())
        }) {
            self.ancestry.pop();
        }
        let parent = self.ancestry.last().map(|(_, id, _, _)| *id);
        if let Some(parent) = parent {
            self.children.entry(parent).or_default().push(id);
            self.children_by_role
                .entry(parent)
                .or_default()
                .entry(entry.role())
                .or_default()
                .push(id);
        }
        if !self.seen_ids.insert(id) || self.by_path.insert(entry.path().clone(), id).is_some() {
            return Err(AttachmentFailure::DuplicateAttachment { id });
        }
        let text_range = node.text_range();
        let range = arcweft_source::SourceRange::new(
            usize::from(text_range.start()),
            usize::from(text_range.end()),
        );
        let parent_component_range = self
            .ancestry
            .last()
            .map_or(Some(range), |(_, _, _, parent)| {
                semantic_component_range(node, parent, range)
            })
            .ok_or(AttachmentFailure::SnapshotInvariant)?;
        let semantic_parent = self
            .ancestry
            .iter()
            .rev()
            .find(|(_, _, kind, _)| kind.is_expression());
        let semantic_parent_id = semantic_parent.map(|(_, id, _, _)| *id);
        let semantic_component_range = semantic_parent
            .map_or(Some(range), |(_, _, _, parent)| {
                semantic_component_range(node, parent, range)
            })
            .ok_or(AttachmentFailure::SnapshotInvariant)?;
        self.pending.push(PendingAttachment {
            id,
            kind: entry.kind(),
            tag,
            role: entry.role(),
            path: entry.path().clone(),
            range,
            parent_component_range,
            semantic_parent: semantic_parent_id,
            semantic_component_range,
            parent,
            expression_projection: entry.expression_projection().cloned(),
            assertion_projection: entry.assertion_projection(),
            keyword_statement_projection: entry.keyword_statement_projection().cloned(),
            type_projection: entry.type_projection().cloned(),
            pattern_projection: entry.pattern_projection().cloned(),
            path_projection: entry.path_projection().cloned(),
            use_projection: entry.use_projection().cloned(),
            visibility_projection: entry.visibility_projection(),
            attribute_projection: entry.attribute_projection().cloned(),
            declaration_header_projection: entry.declaration_header_projection().cloned(),
            character_projection: entry.character_projection().cloned(),
            test_kind_projection: entry.test_kind_projection().cloned(),
            layer_projection: entry.layer_projection().cloned(),
            entry_projection: entry.entry_projection().cloned(),
            style_projection: entry.style_projection().cloned(),
            method_receiver_projection: entry.method_receiver_projection().cloned(),
            contract_clause_projection: entry.contract_clause_projection().cloned(),
            flow_declaration_projection: entry.flow_declaration_projection().cloned(),
            view_export_projection: entry.view_export_projection().cloned(),
            view_fragment_projection: entry.view_fragment_projection().cloned(),
        });
        self.ancestry
            .push((entry.path().clone(), id, entry.kind(), node.clone()));
        Ok(())
    }

    fn finish(mut self) -> Result<AttachmentInventory, AttachmentFailure> {
        let mut records = HashMap::with_capacity(self.expected_count);
        for node in self.pending {
            let record =
                snapshot::AttachedNodeRecord::from_parts(snapshot::AttachedNodeRecordParts {
                    kind: node.kind,
                    tag: node.tag,
                    role: node.role,
                    path: node.path,
                    range: node.range,
                    parent_component_range: node.parent_component_range,
                    semantic_parent: node.semantic_parent,
                    semantic_component_range: node.semantic_component_range,
                    parent: node.parent,
                    children: self
                        .children
                        .remove(&node.id)
                        .unwrap_or_default()
                        .into_boxed_slice(),
                    children_by_role: self
                        .children_by_role
                        .remove(&node.id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(role, children)| (role, children.into_boxed_slice()))
                        .collect(),
                    expression_projection: node.expression_projection,
                    assertion_projection: node.assertion_projection,
                    keyword_statement_projection: node.keyword_statement_projection,
                    type_projection: node.type_projection,
                    pattern_projection: node.pattern_projection,
                    path_projection: node.path_projection,
                    use_projection: node.use_projection,
                    visibility_projection: node.visibility_projection,
                    attribute_projection: node.attribute_projection,
                    declaration_header_projection: node.declaration_header_projection,
                    character_projection: node.character_projection,
                    test_kind_projection: node.test_kind_projection,
                    layer_projection: node.layer_projection,
                    entry_projection: node.entry_projection,
                    style_projection: node.style_projection,
                    method_receiver_projection: node.method_receiver_projection,
                    contract_clause_projection: node.contract_clause_projection,
                    flow_declaration_projection: node.flow_declaration_projection,
                    view_export_projection: node.view_export_projection,
                    view_fragment_projection: node.view_fragment_projection,
                });
            if records.insert(node.id, record).is_some() {
                return Err(AttachmentFailure::DuplicateAttachment { id: node.id });
            }
        }
        if records.len() != self.identities.len() {
            return Err(AttachmentFailure::IdentityMapMismatch {
                expected: self.expected_count,
                actual: self.identities.len(),
            });
        }
        Ok(AttachmentInventory {
            records,
            by_path: self.by_path,
        })
    }
}

#[derive(Debug)]
struct PendingAttachment {
    id: SyntaxNodeId,
    kind: SyntaxKind,
    tag: AstTag,
    role: SyntaxRole,
    path: GrammarEventPath,
    range: arcweft_source::SourceRange,
    parent_component_range: arcweft_source::SourceRange,
    semantic_parent: Option<SyntaxNodeId>,
    semantic_component_range: arcweft_source::SourceRange,
    parent: Option<SyntaxNodeId>,
    expression_projection: Option<crate::expressions::PendingExpressionProjection>,
    assertion_projection: Option<crate::grammar::assertion_projection::PendingAssertionProjection>,
    keyword_statement_projection:
        Option<crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection>,
    type_projection: Option<crate::grammar::event::PendingTypeProjection>,
    pattern_projection: Option<crate::grammar::event::PendingPatternProjection>,
    path_projection: Option<crate::grammar::source_projection::PendingPathProjection>,
    use_projection: Option<crate::grammar::source_projection::PendingUseProjection>,
    visibility_projection: Option<crate::grammar::source_projection::PendingVisibilityKind>,
    attribute_projection:
        Option<crate::grammar::attribute_projection::PendingOuterAttributeProjection>,
    declaration_header_projection:
        Option<crate::grammar::declaration_projection::PendingDeclarationHeaderProjection>,
    character_projection:
        Option<crate::grammar::declaration_projection::PendingCharacterDeclarationProjection>,
    test_kind_projection: Option<crate::grammar::test_projection::PendingTestKindProjection>,
    layer_projection:
        Option<crate::grammar::declaration_projection::PendingLayerDeclarationProjection>,
    entry_projection: Option<crate::grammar::entry_projection::PendingEntryDeclarationProjection>,
    style_projection: Option<crate::grammar::style_projection::PendingStyleDeclarationProjection>,
    method_receiver_projection:
        Option<crate::grammar::callable_projection::PendingMethodReceiverProjection>,
    contract_clause_projection:
        Option<crate::grammar::contract_projection::PendingFlowContractClauseProjection>,
    flow_declaration_projection:
        Option<crate::grammar::flow_projection::PendingFlowDeclarationProjection>,
    view_export_projection: Option<crate::grammar::view_projection::PendingViewExportProjection>,
    view_fragment_projection:
        Option<crate::grammar::view_projection::PendingViewFragmentProjection>,
}

fn semantic_component_range(
    node: &SyntaxNode,
    identity_parent: &SyntaxNode,
    node_range: arcweft_source::SourceRange,
) -> Option<arcweft_source::SourceRange> {
    let mut outer_group = None;
    for ancestor in node.ancestors().skip(1) {
        if &ancestor == identity_parent {
            return Some(outer_group.unwrap_or(node_range));
        }
        if ancestor.kind().0 == SyntaxKind::DelimitedGroup as u16 {
            let range = ancestor.text_range();
            outer_group = Some(arcweft_source::SourceRange::new(
                usize::from(range.start()),
                usize::from(range.end()),
            ));
        }
    }
    None
}

fn strict_path_prefix(parent: &[u32], child: &[u32]) -> bool {
    parent.len() < child.len() && child.starts_with(parent)
}

/// Resolves every identity-bearing grammar path in one source-order Rowan walk.
///
/// A separate root-to-path replay for each sibling becomes quadratic because a
/// Rowan child index is reached by iterator position. This transaction-local
/// inventory is consumed by the final shape/attachment owners and is never
/// published as another syntax authority.
pub(crate) fn grammar_nodes_for_paths(
    root: &SyntaxNode,
    paths: &HashSet<GrammarEventPath>,
) -> Option<HashMap<GrammarEventPath, SyntaxNode>> {
    let mut nodes = HashMap::with_capacity(paths.len());
    let mut stack = vec![(root.clone(), Vec::<u32>::new())];
    while let Some((node, elements)) = stack.pop() {
        let path = GrammarEventPath::from_elements(elements.clone().into_boxed_slice());
        if paths.contains(&path) {
            nodes.insert(path, node.clone());
        }
        let children = node
            .children_with_tokens()
            .enumerate()
            .filter_map(|(index, element)| {
                let node = element.into_node()?;
                let index = u32::try_from(index).ok()?;
                Some((index, node))
            })
            .collect::<Vec<_>>();
        for (index, child) in children.into_iter().rev() {
            let mut child_elements = elements.clone();
            child_elements.push(index);
            stack.push((child, child_elements));
        }
    }
    (nodes.len() == paths.len()).then_some(nodes)
}

fn grammar_node_at_path(root: &SyntaxNode, path: &GrammarEventPath) -> Option<SyntaxNode> {
    let mut current = root.clone();
    for &element in path.elements() {
        let index = usize::try_from(element).ok()?;
        current = current.children_with_tokens().nth(index)?.into_node()?;
    }
    Some(current)
}

fn validate_snapshot(snapshot: &Arc<SyntaxSnapshotData>) -> Result<(), AttachmentFailure> {
    let root = snapshot.root_handle();
    validate_snapshot_root(snapshot, &root)?;
    for node in snapshot.nodes() {
        validate_snapshot_node(snapshot, &root, &node)?;
    }
    Ok(())
}

fn validate_snapshot_root(
    snapshot: &Arc<SyntaxSnapshotData>,
    root: &SyntaxNodeHandle,
) -> Result<(), AttachmentFailure> {
    let typed_root = snapshot
        .typed_node::<SourceFileKind>(root.id())
        .map_err(|_| AttachmentFailure::SnapshotInvariant)?;
    if typed_root.id() != root.id()
        || typed_root.snapshot_id() != snapshot.snapshot_id()
        || typed_root.syntax() != *root
        || typed_root.range() != root.range()
        || !typed_root.is_same_reconciled_node(&typed_root.clone())
        || root.parent().is_some()
        || root.tag() != AstTag::SourceFile
        || root
            .cast::<SourceFileKind>()
            .map_err(|_| AttachmentFailure::SnapshotInvariant)?
            != typed_root
        || root.rowan().text() != snapshot.document().text()
    {
        return Err(AttachmentFailure::SnapshotInvariant);
    }
    Ok(())
}

fn validate_snapshot_node(
    snapshot: &Arc<SyntaxSnapshotData>,
    root: &SyntaxNodeHandle,
    node: &SyntaxNodeHandle,
) -> Result<(), AttachmentFailure> {
    validate_node_lookup(snapshot, node)?;
    if !node_projection_shape_is_valid(node) {
        return Err(AttachmentFailure::SnapshotInvariant);
    }
    validate_node_semantics(node)?;
    validate_node_projection_origin(node)?;
    if node.id() != root.id() && node.parent().is_none() {
        return Err(AttachmentFailure::SnapshotInvariant);
    }
    validate_node_children(node)?;
    let _ = node.role().class();
    Ok(())
}

fn validate_node_lookup(
    snapshot: &Arc<SyntaxSnapshotData>,
    node: &SyntaxNodeHandle,
) -> Result<(), AttachmentFailure> {
    if snapshot
        .syntax_node(node.id())
        .map_err(|_| AttachmentFailure::SnapshotInvariant)?
        != *node
        || snapshot
            .node_for_path(node.path())
            .is_none_or(|by_path| by_path != *node)
        || snapshot
            .resolve_exact(node)
            .map_err(|_| AttachmentFailure::SnapshotInvariant)?
            != *node
        || node.kind().ast_tag() != Some(node.tag())
        || node.range().end() > snapshot.document().text().len()
    {
        return Err(AttachmentFailure::SnapshotInvariant);
    }
    Ok(())
}

fn node_projection_shape_is_valid(node: &SyntaxNodeHandle) -> bool {
    let keyword_projection = node.keyword_statement_projection();
    let keyword_projection_is_valid =
        crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection::kind_requires_projection(
            node.kind(),
        ) == keyword_projection.is_some();
    (!crate::expressions::PendingExpressionProjection::kind_requires_projection(node.kind())
        || node.expression_projection().is_some())
        && ((node.kind() == SyntaxKind::AssertionStatement)
            == node.assertion_projection().is_some())
        && keyword_projection_is_valid
        && keyword_projection.is_none_or(|projection| projection.accepts_kind(node.kind()))
        && node.kind().is_type_node() == node.type_projection().is_some()
        && (node.kind() == SyntaxKind::Path) == node.path_projection().is_some()
        && (node.kind() == SyntaxKind::UseDeclaration) == node.use_projection().is_some()
        && (node.kind() == SyntaxKind::Visibility) == node.visibility_projection().is_some()
        && matches!(
            node.kind(),
            SyntaxKind::InnerAttribute | SyntaxKind::OuterAttribute
        ) == node.attribute_projection().is_some()
        && (node.kind() == SyntaxKind::TestItem) == node.test_kind_projection().is_some()
        && (node.kind() == SyntaxKind::EntryDeclarationItem) == node.entry_projection().is_some()
        && (node.kind() == SyntaxKind::StyleItem) == node.style_projection().is_some()
        && (node.kind() == SyntaxKind::Parameter || node.method_receiver_projection().is_none())
        && (!matches!(
            node.kind(),
            SyntaxKind::InvariantClause
                | SyntaxKind::AssumeClause
                | SyntaxKind::ReadsClause
                | SyntaxKind::EffectsClause
                | SyntaxKind::NoEffectClause
                | SyntaxKind::ModifiesClause
                | SyntaxKind::DecreasesClause
        ) || node.contract_clause_projection().is_some())
        && node
            .contract_clause_projection()
            .is_none_or(|projection| projection.ranges_are_valid_for(node.kind(), node.range()))
        && (node.kind() == SyntaxKind::FlowItem) == node.flow_declaration_projection().is_some()
        && node
            .flow_declaration_projection()
            .is_none_or(|projection| projection.ranges_are_valid_for(node.range()))
        && (node.kind() == SyntaxKind::ViewExportDeclaration)
            == node.view_export_projection().is_some()
        && (node.kind() == SyntaxKind::ViewFragment) == node.view_fragment_projection().is_some()
        && node
            .view_fragment_projection()
            .is_none_or(|projection| projection.ranges_are_valid_for(node.range()))
}

fn validate_node_semantics(node: &SyntaxNodeHandle) -> Result<(), AttachmentFailure> {
    let expression_invalid = node.expression_projection().is_some()
        && expression::AttachedExpressionNode::from_syntax(node.clone()).is_err();
    let attribute_invalid = node.attribute_projection().is_some()
        && match node.kind() {
            SyntaxKind::InnerAttribute => node
                .clone()
                .cast::<node::InnerAttributeKind>()
                .map_or(true, |attribute| attribute.semantics().is_err()),
            SyntaxKind::OuterAttribute => node
                .clone()
                .cast::<node::OuterAttributeKind>()
                .map_or(true, |attribute| attribute.semantics().is_err()),
            _ => true,
        };
    let style_invalid = node.style_projection().is_some()
        && node
            .clone()
            .cast::<node::StyleItemKind>()
            .map_or(true, |style| style.semantics().is_err());
    let flow_invalid = node.flow_declaration_projection().is_some()
        && node
            .clone()
            .cast::<node::FlowItemKind>()
            .map_or(true, |flow| flow.semantics().is_err());
    if expression_invalid || attribute_invalid || style_invalid || flow_invalid {
        return Err(AttachmentFailure::SnapshotInvariant);
    }
    Ok(())
}

fn validate_node_projection_origin(node: &SyntaxNodeHandle) -> Result<(), AttachmentFailure> {
    if let Some(projection) = node.pattern_projection()
        && (projection
            .authored()
            .value_at(projection.path())
            .is_none_or(|value| !pattern::family_accepts_kind(value.family(), node.kind()))
            || projection
                .authored()
                .source()
                .source_at(projection.path())
                .is_none_or(|range| *range != node.range())
            || node
                .pattern_node_for_projection(projection.tree(), projection.path())
                .as_ref()
                != Some(node))
    {
        return Err(AttachmentFailure::SnapshotInvariant);
    }
    if let Some(projection) = node.type_projection()
        && (node
            .type_node_for_projection(projection.tree(), projection.path())
            .as_ref()
            != Some(node)
            || projection
                .authored()
                .source_at(projection.path())
                .is_none_or(|source| {
                    node.range().start() != source.whole().start()
                        || node.range().end() != source.whole().end()
                }))
    {
        return Err(AttachmentFailure::SnapshotInvariant);
    }
    Ok(())
}

fn validate_node_children(node: &SyntaxNodeHandle) -> Result<(), AttachmentFailure> {
    for child in node.children() {
        let same_role = node.children_with_role(child.role());
        if child.parent().as_ref() != Some(node)
            || !same_role.contains(&child)
            || (same_role.len() == 1 && node.child(child.role()).as_ref() != Some(&child))
            || !strict_path_prefix(node.path().elements(), child.path().elements())
        {
            return Err(AttachmentFailure::SnapshotInvariant);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};
    use core::num::NonZeroU64;

    use super::access::{
        BlockTailNode, DeclarationBodyNode, IfStatementElseNode, IfStatementHeadNode,
        LetInitializerNode, MatchStatementArmBodyNode, MatchStatementBodyNode,
        MatchStatementExpressionNode, UnsafeAuditBodyNode, UnsafeAuditIdNode,
        UnsafeAuditReasonNode,
    };
    use super::family::{
        DelimiterFamily, ExpressionFamily, FamilyNode, PatternFamily, RichTextNode, TypeFamily,
    };
    use super::node::{
        AssertionStatementKind, BinaryExpressionKind, CallExpressionKind, CharacterBodyKind,
        DialogueContentKind, ExpressionBodyKind, FixedParameterGroupKind, FunctionBodyKind,
        FunctionTypeKind, GenericApplicationTypeKind, IfStatementKind, LetStatementKind,
        MatchStatementKind, PostfixBracketPayloadKind, PredicateBodyKind, ProofBlockKind,
        ProofBodyKind, ProofCallStatementKind, RecordPatternKind, RichTextArgumentPayloadKind,
        RichTextArgumentValueKind, RichTextConditionPayloadKind, RichTextDialogueCallPayloadKind,
        RichTextEndTagKind, RichTextFxCallPayloadKind, RichTextInvalidArgumentKind,
        RichTextNamedArgumentKind, RichTextTagKind, SourceFileKind, UnsafeLifetimeStatementKind,
        WholeBindingPatternKind,
    };
    use super::{
        AstNode, GrammarIdentityMap, PredicateItemKind, ProofItemKind, SyntaxDatabaseId,
        SyntaxLineageId, SyntaxLookupError, SyntaxNodeId, SyntaxSnapshotId, TypedItemNode,
        attach_typed_tree,
    };
    use crate::grammar::kinds::{AstTag, SyntaxKind, SyntaxRole, SyntaxRoleClass};
    use crate::parser::parse_document;
    use crate::text::{ScannedTagArgument, scan_tag_arguments};

    fn document(text: &str) -> Arc<SourceDocument> {
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcw:/attachment-test").unwrap(),
                SourceName::path("attachment-test.arcw"),
                text,
            )
            .unwrap(),
        )
    }

    fn attach(text: &str) -> Arc<super::SyntaxSnapshotData> {
        attach_at(text, 1, 1)
    }

    fn source_file(snapshot: &Arc<super::SyntaxSnapshotData>) -> AstNode<SourceFileKind> {
        snapshot.root_handle().cast().unwrap()
    }

    fn attached_dialogue_content(
        snapshot: &Arc<super::SyntaxSnapshotData>,
    ) -> AstNode<DialogueContentKind> {
        snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::PostfixBracketPayload)
            .expect("postfix bracket payload")
            .cast::<PostfixBracketPayloadKind>()
            .unwrap()
            .required_exact_child(SyntaxRole::Content)
            .unwrap()
    }

    fn attached_rich_text_start_tags(
        content: &AstNode<DialogueContentKind>,
    ) -> Vec<AstNode<RichTextTagKind>> {
        content
            .syntax()
            .children()
            .into_iter()
            .filter(|child| child.kind() == SyntaxKind::RichTextTag)
            .map(|child| child.cast::<RichTextTagKind>().unwrap())
            .collect()
    }

    fn attached_rich_text_end_tags(
        content: &AstNode<DialogueContentKind>,
    ) -> Vec<AstNode<RichTextEndTagKind>> {
        content
            .syntax()
            .children()
            .into_iter()
            .filter(|child| child.kind() == SyntaxKind::RichTextEndTag)
            .map(|child| child.cast::<RichTextEndTagKind>().unwrap())
            .collect()
    }

    fn attach_at(
        text: &str,
        database_ordinal: u64,
        lineage_ordinal: u64,
    ) -> Arc<super::SyntaxSnapshotData> {
        let document = document(text);
        let build = parse_document(&document, crate::parser::ParseOptions::default()).unwrap();
        let database =
            SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(database_ordinal).unwrap());
        let lineage =
            SyntaxLineageId::from_raw_for_test(database, NonZeroU64::new(lineage_ordinal).unwrap());
        let snapshot = SyntaxSnapshotId::new(
            lineage,
            SourceSnapshotId::initial(document.display_name().clone()),
        );
        let identities = build
            .index()
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                (
                    entry.path().clone(),
                    SyntaxNodeId::new(
                        lineage,
                        NonZeroU64::new(u64::try_from(index).unwrap() + 1).unwrap(),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        attach_typed_tree(
            &build,
            &GrammarIdentityMap::new(identities),
            snapshot,
            document,
        )
        .unwrap()
    }

    #[test]
    fn typed_and_rowan_handles_round_trip_without_range_search() {
        let snapshot = attach("predicate ready() = true\nproof valid() = ()\n");
        let predicate = snapshot
            .nodes()
            .find(|node| node.kind() == crate::grammar::kinds::SyntaxKind::PredicateItem)
            .unwrap();
        let typed = AstNode::<PredicateItemKind>::new(predicate.clone()).unwrap();
        let rebound = snapshot.bind_rowan(typed.syntax().rowan()).unwrap();
        assert_eq!(rebound, predicate);
        assert_eq!(
            snapshot
                .typed_node::<PredicateItemKind>(typed.id())
                .unwrap(),
            typed
        );

        assert!(matches!(
            snapshot.typed_node::<ProofItemKind>(typed.id()),
            Err(SyntaxLookupError::KindMismatch { .. })
        ));
    }

    #[test]
    fn structurally_equal_foreign_rowan_root_is_rejected() {
        let first = attach("proof valid() = ()\n");
        let second = attach("proof valid() = ()\n");
        let foreign = second.root_handle();
        assert!(matches!(
            first.bind_rowan(foreign.rowan()),
            Err(SyntaxLookupError::ForeignRowanRoot { .. })
        ));
    }

    #[test]
    fn typed_handle_cannot_cross_an_immutable_snapshot_lineage() {
        let first = attach_at("proof valid() = ()\n", 1, 1);
        let second = attach_at("proof valid() = ()\n", 1, 2);
        let first_item = source_file(&first).items().unwrap().remove(0);
        assert!(matches!(
            second.resolve_exact(&first_item.syntax()),
            Err(SyntaxLookupError::WrongSnapshot { .. })
        ));
    }

    #[test]
    fn exact_roles_index_nearest_identity_parent_without_structural_wrappers() {
        let snapshot = attach("predicate ready(value: Bool) = check(value)\n");
        let root = snapshot.root_handle();
        let predicate = root
            .child(SyntaxRole::Element(0))
            .expect("item-list wrapper does not become semantic parent");
        assert_eq!(predicate.kind(), SyntaxKind::PredicateItem);
        assert_eq!(predicate.tag(), AstTag::Item);
        assert_eq!(predicate.parent(), Some(root.clone()));
        let element_children = root
            .children()
            .into_iter()
            .filter(|child| child.role().class() == SyntaxRoleClass::Element)
            .collect::<Vec<_>>();
        assert_eq!(
            element_children.as_slice(),
            std::slice::from_ref(&predicate)
        );

        let name = predicate
            .child(SyntaxRole::Name)
            .expect("declaration name is indexed by exact role");
        assert_eq!(name.kind(), SyntaxKind::NameDefinition);
        assert_eq!(name.tag(), AstTag::Name);
        assert_eq!(name.parent(), Some(predicate.clone()));

        let body = predicate
            .child(SyntaxRole::Body)
            .expect("predicate body is indexed by exact role");
        assert_eq!(body.kind(), SyntaxKind::PredicateBody);
        assert_eq!(body.tag(), AstTag::Body);
        let expression_body = body
            .child(SyntaxRole::Body)
            .expect("expression body remains a distinct identity owner");
        let call = expression_body
            .child(SyntaxRole::Body)
            .expect("ordinary expression is attached below its authored body");
        assert_eq!(call.kind(), SyntaxKind::CallExpression);
        assert_eq!(call.tag(), AstTag::Expression);
        assert_eq!(
            call.child(SyntaxRole::Callee)
                .expect("call callee role")
                .kind(),
            SyntaxKind::PathExpression
        );
    }

    #[test]
    fn repeated_exact_roles_remain_ordered_without_claiming_unique_child_access() {
        let snapshot = attach("flow checks {\n    assert.check(true, false)\n}\n");
        let assertion = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::AssertionStatement)
            .expect("assertion node");
        let conditions = assertion.children_with_role(SyntaxRole::Condition);
        assert_eq!(conditions.len(), 2);
        assert!(
            conditions
                .iter()
                .all(|condition| condition.tag() == AstTag::Expression)
        );
        assert_eq!(
            conditions
                .iter()
                .map(|condition| condition.rowan().text().to_string())
                .collect::<Vec<_>>(),
            ["true", "false"]
        );
        assert_eq!(assertion.child(SyntaxRole::Condition), None);
    }

    #[test]
    fn statement_if_let_and_unsafe_audit_anchor_remain_typed_and_snapshot_bound() {
        let snapshot = attach(concat!(
            "fn choose(input: Option<Int>, ready: Bool) {\n",
            "    if let .Some(value) = input when ready { value; } else if ready { 1; } else { 0; };\n",
            "    unsafe lifetime @unsafe.audit reason = \"bounded lifetime\" {\n",
            "        /// SAFETY: the test owns the retained value.\n",
            "        value;\n",
            "    };\n",
            "}\n",
        ));

        let conditional = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::IfStatement)
            .expect("if statement")
            .cast::<IfStatementKind>()
            .unwrap();
        let IfStatementHeadNode::Let {
            pattern,
            scrutinee,
            guard,
        } = conditional.head().unwrap()
        else {
            panic!("statement-form if let must keep its pattern head");
        };
        assert_eq!(pattern.syntax().rowan().text().to_string(), ".Some(value)");
        assert_eq!(scrutinee.syntax().rowan().text().to_string(), "input");
        assert_eq!(guard.unwrap().syntax().rowan().text().to_string(), "ready");
        assert_eq!(
            conditional
                .then_branch()
                .unwrap()
                .statements()
                .unwrap()
                .len(),
            1
        );
        let Some(IfStatementElseNode::If(nested)) = conditional.else_branch().unwrap() else {
            panic!("else if must keep its nested statement identity");
        };
        let nested = nested.cast::<IfStatementKind>().unwrap();
        assert!(matches!(
            nested.head().unwrap(),
            IfStatementHeadNode::Condition(condition)
                if condition.syntax().rowan().text() == "ready"
        ));
        assert!(matches!(
            nested.else_branch().unwrap(),
            Some(IfStatementElseNode::Block(_))
        ));

        let audit = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::UnsafeLifetimeStatement)
            .expect("unsafe lifetime statement")
            .cast::<UnsafeLifetimeStatementKind>()
            .unwrap();
        let UnsafeAuditIdNode::Reference(audit_id) = audit.audit_id().unwrap() else {
            panic!("canonical audit identity must remain source-backed");
        };
        assert!(matches!(
            audit_id.semantic().unwrap().projection(),
            crate::expressions::ExpressionProjection::EntityReference(_)
        ));
        let Some(UnsafeAuditReasonNode::Expression(reason)) = audit.reason().unwrap() else {
            panic!("canonical audit reason must remain source-backed");
        };
        assert_eq!(reason.source_text(), "\"bounded lifetime\"");
        assert_eq!(audit.safety_documentation().unwrap().len(), 1);
        let anchor = audit.audit_insertion_anchor().unwrap();
        assert_eq!(anchor.syntax().rowan().text().to_string(), "{");
        assert_eq!(anchor.snapshot_id(), snapshot.snapshot_id());
        assert_eq!(
            anchor.syntax().parent(),
            Some(audit.body().unwrap().syntax().clone())
        );
        assert_eq!(
            anchor.id(),
            audit.body().unwrap().open_delimiter().unwrap().id()
        );
        assert_eq!(
            audit
                .body()
                .unwrap()
                .close_delimiter()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "}"
        );
    }

    #[test]
    fn statement_match_body_owns_flattened_typed_arms_and_exact_recovery() {
        let snapshot = attach(concat!(
            "fn choose(subject: Int, ready: Bool) {\n",
            "    match subject {\n",
            "        value when ready => consume(value),\n",
            "        _ => { consume(subject); },\n",
            "    };\n",
            "}\n",
        ));
        let statement = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::MatchStatement)
            .expect("Match statement")
            .cast::<MatchStatementKind>()
            .unwrap();
        assert!(matches!(
            statement.scrutinee().unwrap(),
            MatchStatementExpressionNode::Expression(scrutinee)
                if scrutinee.source_text() == "subject"
        ));
        let MatchStatementBodyNode::Block(body) = statement.body_or_missing().unwrap() else {
            panic!("canonical Match must retain its braced body");
        };
        let arms = MatchStatementBodyNode::Block(body.clone()).arms().unwrap();
        assert_eq!(arms.len(), 2);
        assert_eq!(
            body.syntax()
                .children_with_role(SyntaxRole::MatchArm(0))
                .len(),
            1
        );
        assert!(
            snapshot
                .nodes()
                .all(|node| node.kind() != SyntaxKind::MatchArmList),
            "structural MatchArmList must not become a second attached owner"
        );
        assert_eq!(arms[0].pattern().unwrap().source_text(), "value");
        assert!(matches!(
            arms[0].guard().unwrap(),
            Some(MatchStatementExpressionNode::Expression(guard))
                if guard.source_text() == "ready"
        ));
        assert!(matches!(
            arms[0].body().unwrap(),
            MatchStatementArmBodyNode::Expression(body) if body.source_text() == "consume(value)"
        ));
        assert!(matches!(
            arms[1].body().unwrap(),
            MatchStatementArmBodyNode::Block(_)
        ));

        let missing = attach("fn choose() { match subject; }\n");
        let missing = missing
            .nodes()
            .find(|node| node.kind() == SyntaxKind::MatchStatement)
            .unwrap()
            .cast::<MatchStatementKind>()
            .unwrap();
        assert!(matches!(
            missing.body_or_missing().unwrap(),
            MatchStatementBodyNode::Missing(body) if body.range().is_empty()
        ));
    }

    #[test]
    fn missing_if_let_equals_retains_a_typed_missing_scrutinee() {
        let snapshot = attach(concat!(
            "fn choose(input: Option<Int>) {\n",
            "    if let .Some(value) input { value; };\n",
            "}\n",
        ));
        let conditional = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::IfStatement)
            .expect("recovered if statement")
            .cast::<IfStatementKind>()
            .unwrap();
        let IfStatementHeadNode::Let { scrutinee, .. } = conditional.head().unwrap() else {
            panic!("recovered if let must keep its pattern head");
        };
        assert_eq!(scrutinee.kind(), SyntaxKind::MissingExpression);
        assert!(scrutinee.range().is_empty());
        assert_eq!(scrutinee.snapshot_id(), snapshot.snapshot_id());
    }

    #[test]
    fn unsafe_audit_body_recovery_never_fabricates_an_authored_anchor() {
        let missing_body = attach(concat!(
            "fn audit() {\n",
            "    unsafe lifetime @unsafe.audit value;\n",
            "}\n",
        ));
        let audit = missing_body
            .nodes()
            .find(|node| node.kind() == SyntaxKind::UnsafeLifetimeStatement)
            .expect("unsafe lifetime statement")
            .cast::<UnsafeLifetimeStatementKind>()
            .unwrap();
        assert!(audit.body().is_err());
        assert!(matches!(
            audit.body_or_missing().unwrap(),
            UnsafeAuditBodyNode::Missing(missing) if missing.range().is_empty()
        ));
        assert!(audit.audit_insertion_anchor().is_err());

        let unclosed_body = attach(concat!(
            "fn audit() {\n",
            "    unsafe lifetime @unsafe.audit { value;\n",
        ));
        let audit = unclosed_body
            .nodes()
            .find(|node| node.kind() == SyntaxKind::UnsafeLifetimeStatement)
            .expect("recovered unsafe lifetime statement")
            .cast::<UnsafeLifetimeStatementKind>()
            .unwrap();
        let body = audit.body().unwrap();
        assert_eq!(
            audit
                .audit_insertion_anchor()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "{"
        );
        assert!(body.close_delimiter().unwrap().range().is_empty());
    }

    #[test]
    fn unsafe_audit_head_recovery_distinguishes_omission_from_missing_values() {
        let snapshot = attach(concat!(
            "fn audit() {\n",
            "    unsafe lifetime reason { value; };\n",
            "    unsafe lifetime @unsafe.second reason { value; };\n",
            "    unsafe lifetime @unsafe.third { value; };\n",
            "}\n",
        ));
        let audits = snapshot
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::UnsafeLifetimeStatement)
            .map(|node| node.cast::<UnsafeLifetimeStatementKind>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(audits.len(), 3);
        assert!(matches!(
            audits[0].audit_id().unwrap(),
            UnsafeAuditIdNode::Missing(missing) if missing.range().is_empty()
        ));
        assert!(matches!(
            audits[0].reason().unwrap(),
            Some(UnsafeAuditReasonNode::Missing(missing)) if missing.range().is_empty()
        ));
        assert!(matches!(
            audits[1].reason().unwrap(),
            Some(UnsafeAuditReasonNode::Missing(missing)) if missing.range().is_empty()
        ));
        assert_eq!(audits[2].reason().unwrap(), None);
    }

    #[test]
    fn typed_tree_navigates_declaration_prefixes_parameters_and_expression_body() {
        let source = concat!(
            "/// externally reviewed\n",
            "#[verify.trusted(reason = \"reviewed\")]\n",
            "pub proof ordered<'a, T>((left, right): (T, T), cmp: Comparator<T>) ",
            "-> Bool where T: Ord requires cmp.ready() ensures result = left == right\n",
        );
        let snapshot = attach(source);
        let root = source_file(&snapshot);
        assert_eq!(root.range(), SourceRange::new(0, source.len()));
        let items = root.items().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].role(), SyntaxRole::Element(0));

        let TypedItemNode::Proof(proof) = &items[0] else {
            panic!("expected proof item");
        };
        assert_eq!(
            items[0]
                .documentation()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "/// externally reviewed\n"
        );
        assert_eq!(items[0].attributes().unwrap().len(), 1);
        assert_eq!(
            items[0]
                .visibility()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "pub"
        );
        assert_eq!(
            items[0]
                .name()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "ordered"
        );
        assert!(items[0].declaration_header().unwrap().is_none());

        let parameters = proof
            .required_exact_child::<FixedParameterGroupKind>(SyntaxRole::ParameterGroup)
            .unwrap()
            .parameters()
            .unwrap();
        assert_eq!(parameters.len(), 2);
        assert_eq!(
            parameters[0].pattern().unwrap().kind(),
            SyntaxKind::TuplePattern
        );
        assert_eq!(parameters[0].ty().unwrap().kind(), SyntaxKind::TupleType);
        assert_eq!(
            parameters[1].ty().unwrap().kind(),
            SyntaxKind::GenericApplicationType
        );

        let DeclarationBodyNode::Body(proof_body) = items[0].body().unwrap().unwrap() else {
            panic!("proof has an authored body");
        };
        let proof_body = proof_body.cast::<ProofBodyKind>().unwrap();
        let DeclarationBodyNode::Body(expression_body) = proof_body.content().unwrap() else {
            panic!("proof has an expression body");
        };
        let expression_body = expression_body.cast::<ExpressionBodyKind>().unwrap();
        let expression = expression_body.expression().unwrap();
        let binary = expression.cast::<BinaryExpressionKind>().unwrap();
        assert_eq!(
            binary.left().unwrap().syntax().rowan().text().to_string(),
            "left"
        );
        assert_eq!(
            binary.right().unwrap().syntax().rowan().text().to_string(),
            "right"
        );
    }

    #[test]
    fn ordinary_call_accessors_keep_named_and_positional_argument_order() {
        let snapshot = attach("predicate next(value: Int) = outer(named = inner(value), value)\n");
        let item = source_file(&snapshot).items().unwrap().remove(0);
        let DeclarationBodyNode::Body(predicate_body) = item.body().unwrap().unwrap() else {
            panic!("predicate has an authored body");
        };
        let predicate_body = predicate_body.cast::<PredicateBodyKind>().unwrap();
        let DeclarationBodyNode::Body(expression_body) = predicate_body.content().unwrap() else {
            panic!("predicate has an expression body");
        };
        let call = expression_body
            .cast::<ExpressionBodyKind>()
            .unwrap()
            .expression()
            .unwrap()
            .cast::<CallExpressionKind>()
            .unwrap();
        assert_eq!(
            call.callee().unwrap().syntax().rowan().text().to_string(),
            "outer"
        );
        let arguments = call.arguments().unwrap();
        assert_eq!(arguments.len(), 2);
        assert_eq!(arguments[0].role(), SyntaxRole::Argument(0));
        assert_eq!(arguments[1].role(), SyntaxRole::Argument(1));
        assert_eq!(
            arguments[0]
                .name()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "named"
        );
        assert_eq!(
            arguments[0]
                .operand()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "inner(value)"
        );
        assert_eq!(
            arguments[1]
                .operand()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "value"
        );
        assert!(arguments[0].range().end() <= arguments[1].range().start());
    }

    #[test]
    fn retained_declaration_header_owns_prefixes_name_and_body() {
        let source = concat!(
            "/// authored character\n",
            "#[authoring]\n",
            "pub character alice {\n",
            "    display_name = \"Alice\"\n",
            "}\n",
        );
        let snapshot = attach(source);
        let item = source_file(&snapshot).items().unwrap().remove(0);
        let TypedItemNode::Character(character) = &item else {
            panic!("expected character item");
        };
        let header = item
            .declaration_header()
            .unwrap()
            .expect("retained declaration owns an exact header");
        assert_eq!(
            header
                .documentation()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "/// authored character\n"
        );
        assert_eq!(header.attributes().unwrap().len(), 1);
        assert_eq!(
            header
                .name()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "alice"
        );
        assert!(header.visibility().unwrap().is_some());
        assert_eq!(
            character
                .required_exact_child::<CharacterBodyKind>(SyntaxRole::Body)
                .unwrap()
                .kind(),
            SyntaxKind::CharacterBody
        );
    }

    #[test]
    fn proof_block_accessors_preserve_statement_pattern_type_and_tail_identity() {
        let source = concat!(
            "proof verify(value: Result<Int>) {\n",
            "    let current: Int = unwrap(value);\n",
            "    assert.check(current > 0, is_valid(current));\n",
            "    verify_nested(current);\n",
            "}\n",
        );
        let snapshot = attach(source);
        let item = source_file(&snapshot).items().unwrap().remove(0);
        let TypedItemNode::Proof(proof) = item else {
            panic!("expected proof item");
        };
        let proof_body = proof
            .required_exact_child::<ProofBodyKind>(SyntaxRole::Body)
            .unwrap();
        let DeclarationBodyNode::Body(block) = proof_body.content().unwrap() else {
            panic!("proof has a block body");
        };
        let block = block.cast::<ProofBlockKind>().unwrap();
        assert_eq!(
            block.open_delimiter().unwrap().kind(),
            SyntaxKind::OpenBraceNode
        );
        assert_eq!(
            block.close_delimiter().unwrap().kind(),
            SyntaxKind::CloseBraceNode
        );

        let statements = block.statements().unwrap();
        assert_eq!(statements.len(), 3);
        assert_eq!(statements[0].role(), SyntaxRole::Statement(0));
        assert_eq!(statements[1].role(), SyntaxRole::Statement(1));
        assert_eq!(statements[2].role(), SyntaxRole::Statement(2));

        let binding = statements[0].cast::<LetStatementKind>().unwrap();
        assert_eq!(
            binding.pattern().unwrap().kind(),
            SyntaxKind::TypedBindingPattern
        );
        let binding_pattern = binding.pattern().unwrap().semantic().unwrap();
        let binding_type = binding_pattern
            .children()
            .unwrap()
            .into_iter()
            .find_map(|child| child.type_ref().cloned())
            .expect("typed-binding Pattern owns its exact type child");
        assert_eq!(binding_type.syntax().kind(), SyntaxKind::PathType);
        assert!(matches!(
            binding.initializer().unwrap().unwrap(),
            LetInitializerNode::Expression(expression)
                if expression.kind() == SyntaxKind::CallExpression
        ));

        let assertion = statements[1].cast::<AssertionStatementKind>().unwrap();
        let conditions = assertion.conditions().unwrap();
        assert_eq!(conditions.len(), 2);
        assert_eq!(
            conditions
                .iter()
                .map(|condition| condition.syntax().rowan().text().to_string())
                .collect::<Vec<_>>(),
            ["current > 0", "is_valid(current)"]
        );
        assert!(matches!(
            assertion.required_family_child::<ExpressionFamily>(SyntaxRole::Condition),
            Err(super::SyntaxAccessError::AmbiguousChild { count: 2, .. })
        ));

        let proof_call = statements[2].cast::<ProofCallStatementKind>().unwrap();
        assert_eq!(
            proof_call
                .callee()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "verify_nested(current)"
        );
        let BlockTailNode::Omitted(tail) = block.tail().unwrap() else {
            panic!("semicolon-terminated block has an omitted tail");
        };
        assert_eq!(tail.range().start(), tail.range().end());
    }

    #[test]
    fn let_initializer_distinguishes_authored_missing_and_absent_children() {
        let snapshot = attach(concat!(
            "fn initializer_shapes() {\n",
            "    let authored = value;\n",
            "    let missing =;\n",
            "    let absent;\n",
            "}\n",
        ));
        let item = source_file(&snapshot).items().unwrap().remove(0);
        let Some(DeclarationBodyNode::Body(body)) = item.body().unwrap() else {
            panic!("function owns an authored body");
        };
        let statements = body
            .cast::<FunctionBodyKind>()
            .unwrap()
            .block()
            .unwrap()
            .statements()
            .unwrap();
        assert_eq!(statements.len(), 3);

        let authored = statements[0].cast::<LetStatementKind>().unwrap();
        assert!(matches!(
            authored.initializer().unwrap().unwrap(),
            LetInitializerNode::Expression(expression)
                if expression.kind() == SyntaxKind::PathExpression
        ));

        let missing = statements[1].cast::<LetStatementKind>().unwrap();
        assert!(matches!(
            missing.initializer().unwrap().unwrap(),
            LetInitializerNode::Missing(insertion) if insertion.range().is_empty()
        ));

        let absent = statements[2].cast::<LetStatementKind>().unwrap();
        assert_eq!(absent.initializer().unwrap(), None);
    }

    #[test]
    fn missing_and_wrong_kind_paths_fail_without_range_or_text_lookup() {
        let snapshot = attach("proof ()() \n");
        let item = source_file(&snapshot).items().unwrap().remove(0);
        let TypedItemNode::Proof(proof) = &item else {
            panic!("expected proof item");
        };
        assert_eq!(
            item.name().unwrap().unwrap().kind(),
            SyntaxKind::MissingName
        );
        assert_eq!(item.recovery().unwrap().len(), 1);
        assert_eq!(item.recovery().unwrap()[0].kind(), SyntaxKind::ErrorNode);

        let proof_body = proof
            .required_exact_child::<ProofBodyKind>(SyntaxRole::Body)
            .unwrap();
        let DeclarationBodyNode::Missing(missing) = proof_body.content().unwrap() else {
            panic!("missing proof body remains an exact recovery node");
        };
        assert_eq!(missing.kind(), SyntaxKind::MissingBody);
        assert_eq!(missing.range().start(), missing.range().end());
        assert!(!matches!(item, TypedItemNode::Predicate(_)));
        assert!(matches!(
            proof.required_exact_child::<PredicateBodyKind>(SyntaxRole::Body),
            Err(super::SyntaxAccessError::Lookup(
                SyntaxLookupError::KindMismatch { .. }
            ))
        ));
        assert!(matches!(
            FamilyNode::<TypeFamily>::new(item.syntax()),
            Err(super::SyntaxAccessError::FamilyMismatch { .. })
        ));

        let call_snapshot = attach("predicate broken() = outer(value\n");
        let item = source_file(&call_snapshot).items().unwrap().remove(0);
        let TypedItemNode::Predicate(predicate) = item else {
            panic!("expected predicate item");
        };
        let body = predicate
            .required_exact_child::<PredicateBodyKind>(SyntaxRole::Body)
            .unwrap();
        let DeclarationBodyNode::Body(expression_body) = body.content().unwrap() else {
            panic!("predicate retains expression body");
        };
        let call = expression_body
            .cast::<ExpressionBodyKind>()
            .unwrap()
            .expression()
            .unwrap()
            .cast::<CallExpressionKind>()
            .unwrap();
        let close = call
            .required_family_child::<DelimiterFamily>(SyntaxRole::CloseDelimiter)
            .unwrap();
        assert_eq!(close.kind(), SyntaxKind::CloseParenNode);
        assert_eq!(close.range().start(), close.range().end());
    }

    #[test]
    fn dialogue_rich_text_owns_ordered_ranged_attached_descendants() {
        let source = concat!(
            "flow opening {\n",
            "    let line = alice[本文。",
            "[transform .offset x=4px pattern==value label='二 px' missing= bad=\\q]",
            "[fx warning(accent=\"urgent\")]",
            "[call flash(level=2)]",
            "[! blink(level=3)]",
            "[if player.ready]",
            "[.sparkle]",
            "[/]]\n",
            "}\n",
        );
        let snapshot = attach(source);
        let dialogue = attached_dialogue_content(&snapshot);
        assert_eq!(snapshot.root_handle().rowan().text().to_string(), source);

        let tags = attached_rich_text_start_tags(&dialogue);
        assert_eq!(tags.len(), 6);
        for (ordinal, tag) in tags.iter().enumerate() {
            assert_eq!(
                tag.role(),
                SyntaxRole::RichTextTag(u32::try_from(ordinal).unwrap())
            );
        }

        assert_rich_text_argument_descendants(source, &tags[0]);
        assert_rich_text_expression_payloads(&tags[1..5]);
        let sparkle = &tags[5];
        assert_eq!(
            sparkle.name().unwrap().syntax().rowan().text().to_string(),
            ".sparkle"
        );
        assert!(sparkle.payload().unwrap().is_none());
        let end_tags = attached_rich_text_end_tags(&dialogue);
        assert_eq!(end_tags.len(), 1);
        assert!(end_tags[0].name().unwrap().is_none());
    }

    fn assert_rich_text_argument_descendants(source: &str, tag: &AstNode<RichTextTagKind>) {
        assert_eq!(
            tag.name().unwrap().syntax().rowan().text().to_string(),
            "transform"
        );
        let payload = tag
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextArgumentPayloadKind>()
            .unwrap();
        let arguments = payload.arguments().unwrap();
        assert_eq!(arguments.len(), 6);
        for (ordinal, argument) in arguments.iter().enumerate() {
            assert_eq!(
                argument.role(),
                SyntaxRole::Argument(u16::try_from(ordinal).unwrap())
            );
        }

        assert_split_equals_argument(&arguments[2]);
        assert_quoted_argument(source, &arguments[3]);
        assert_missing_and_invalid_arguments(source, &arguments[4], &arguments[5]);
    }

    fn assert_split_equals_argument(argument: &RichTextNode) {
        let split_equals = argument.cast::<RichTextNamedArgumentKind>().unwrap();
        let equals = split_equals.equals().unwrap();
        assert_eq!(equals.syntax().rowan().text().to_string(), "=");
        assert_eq!(
            equals.syntax().rowan().first_token().unwrap().kind().0,
            SyntaxKind::PunctuationToken as u16
        );
        assert_eq!(
            split_equals
                .value()
                .unwrap()
                .cast::<RichTextArgumentValueKind>()
                .unwrap()
                .token()
                .unwrap()
                .content()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "=value"
        );
    }

    #[test]
    fn dialogue_only_canonical_tag_surfaces_gain_rich_text_identity() {
        let source = concat!(
            "flow opening {\n",
            "    let line = alice[本文。",
            "\\[effect .wave]",
            "#[score]",
            "$([effect .wave])",
            "|[base](ruby)",
            "[raw]literal [p][/raw]",
            "[raw: [p]x]",
            "[em:夢]",
            "[color #a8:night]",
            "[ruby rt=x]base[/ruby]",
            "[effect .wave]",
            "]\n",
            "}\n",
        );
        let snapshot = attach(source);
        let dialogue = attached_dialogue_content(&snapshot);
        let tags = attached_rich_text_start_tags(&dialogue);

        assert_eq!(tags.len(), 3);
        assert_eq!(
            tags.iter()
                .map(|tag| tag.name().unwrap().syntax().rowan().text().to_string())
                .collect::<Vec<_>>(),
            ["em", "color", "effect"]
        );
        for (ordinal, tag) in tags.iter().enumerate() {
            assert_eq!(
                tag.role(),
                SyntaxRole::RichTextTag(u32::try_from(ordinal).unwrap())
            );
        }
        assert_eq!(attached_rich_text_end_tags(&dialogue).len(), 2);
        assert_eq!(snapshot.root_handle().rowan().text().to_string(), source);
    }

    fn assert_quoted_argument(source: &str, argument: &RichTextNode) {
        let quoted = argument.cast::<RichTextNamedArgumentKind>().unwrap();
        assert_eq!(
            quoted.key().unwrap().syntax().rowan().text().to_string(),
            "label"
        );
        assert_eq!(
            quoted.equals().unwrap().syntax().rowan().text().to_string(),
            "="
        );
        let value = quoted
            .value()
            .unwrap()
            .cast::<RichTextArgumentValueKind>()
            .unwrap();
        assert_eq!(&source[value.range().as_range()], "'二 px'");
        let token = value.token().unwrap();
        assert_eq!(
            token.content().unwrap().syntax().rowan().text().to_string(),
            "二 px"
        );
        assert_eq!(
            token
                .opening_quote()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "'"
        );
        assert_eq!(
            token
                .closing_quote()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "'"
        );
    }

    fn assert_missing_and_invalid_arguments(
        source: &str,
        missing: &RichTextNode,
        invalid: &RichTextNode,
    ) {
        let missing = missing.cast::<RichTextInvalidArgumentKind>().unwrap();
        assert_eq!(&source[missing.range().as_range()], "missing=");
        let missing_issue = missing.issue().unwrap();
        assert_eq!(
            missing_issue.kind(),
            SyntaxKind::RichTextInvalidArgumentIssue
        );
        assert!(missing_issue.range().is_empty());
        assert_eq!(missing_issue.range().start(), missing.range().end());

        let invalid = invalid.cast::<RichTextInvalidArgumentKind>().unwrap();
        assert_eq!(&source[invalid.range().as_range()], "bad=\\q");
        assert_eq!(
            invalid.issue().unwrap().syntax().rowan().text().to_string(),
            "\\q"
        );
    }

    fn assert_rich_text_expression_payloads(tags: &[AstNode<RichTextTagKind>]) {
        let fx = tags[0]
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextFxCallPayloadKind>()
            .unwrap();
        assert_eq!(
            fx.expression().unwrap().syntax().rowan().text().to_string(),
            "warning(accent=\"urgent\")"
        );
        let call = tags[1]
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextDialogueCallPayloadKind>()
            .unwrap();
        assert_eq!(
            call.expression()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "flash(level=2)"
        );
        let bang = &tags[2];
        assert_eq!(
            bang.name().unwrap().syntax().rowan().text().to_string(),
            "!"
        );
        let bang = bang
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextDialogueCallPayloadKind>()
            .unwrap();
        assert_eq!(
            bang.expression()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "blink(level=3)"
        );
        let condition = tags[3]
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextConditionPayloadKind>()
            .unwrap();
        assert_eq!(
            condition
                .expression()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "player.ready"
        );
    }

    #[test]
    fn attached_rich_text_ranges_match_the_canonical_argument_scanner() {
        let tag_source = "[effect .wave\u{3000}amp=2 label=\"游 ゴシック\"]";
        let attrs = ".wave\u{3000}amp=2 label=\"游 ゴシック\"";
        let source =
            format!("flow opening {{\r\n    let line = alice[本文。{tag_source}]\r\n}}\r\n");
        let snapshot = attach(&source);
        assert_eq!(snapshot.root_handle().rowan().text().to_string(), source);
        let dialogue = attached_dialogue_content(&snapshot);
        let private_tag = attached_rich_text_start_tags(&dialogue).remove(0);
        let payload = private_tag
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextArgumentPayloadKind>()
            .unwrap();
        let private_arguments = payload.arguments().unwrap();

        let tag_start = source.find(tag_source).unwrap();
        let attrs_start = source.find(attrs).unwrap();
        let scanned = scan_tag_arguments(attrs, attrs_start, 32);
        assert!(
            scanned.diagnostics().is_empty(),
            "{:?}",
            scanned.diagnostics()
        );
        assert_eq!(
            private_tag.range(),
            SourceRange::new(tag_start, tag_start + tag_source.len())
        );
        assert_eq!(private_arguments.len(), scanned.entries().len());
        for (private, scanned) in private_arguments.iter().zip(scanned.entries()) {
            assert_eq!(
                private.range(),
                SourceRange::new(scanned.range().start(), scanned.range().end())
            );
        }

        let private_label = private_arguments[2]
            .cast::<RichTextNamedArgumentKind>()
            .unwrap();
        let ScannedTagArgument::Named {
            name_range,
            equals_range,
            value,
            ..
        } = &scanned.entries()[2]
        else {
            panic!("label remains a named RichText argument");
        };
        assert_eq!(
            private_label.key().unwrap().range(),
            SourceRange::new(name_range.start(), name_range.end())
        );
        assert_eq!(
            private_label.equals().unwrap().range(),
            SourceRange::new(equals_range.start(), equals_range.end())
        );
        let private_value = private_label
            .value()
            .unwrap()
            .cast::<RichTextArgumentValueKind>()
            .unwrap();
        let private_token = private_value.token().unwrap();
        assert_eq!(
            private_token.range(),
            SourceRange::new(value.token_range().start(), value.token_range().end())
        );
        assert_eq!(
            private_token.content().unwrap().range(),
            SourceRange::new(value.content_range().start(), value.content_range().end())
        );
        assert_eq!(
            private_token.opening_quote().unwrap().unwrap().range(),
            SourceRange::new(
                value.opening_quote_range().unwrap().start(),
                value.opening_quote_range().unwrap().end(),
            )
        );
        assert_eq!(
            private_token.closing_quote().unwrap().unwrap().range(),
            SourceRange::new(
                value.closing_quote_range().unwrap().start(),
                value.closing_quote_range().unwrap().end(),
            )
        );
    }

    #[test]
    fn equal_range_rich_text_recovery_nodes_keep_distinct_path_identity() {
        let source = concat!(
            "flow opening {\n",
            "    let line = alice[本文。[effect \\q]]\n",
            "}\n",
        );
        let snapshot = attach(source);
        let invalid = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::RichTextInvalidArgument)
            .unwrap()
            .cast::<RichTextInvalidArgumentKind>()
            .unwrap();
        let issue = invalid.issue().unwrap();

        assert_eq!(invalid.range(), issue.range());
        assert_ne!(invalid.id(), issue.id());
        assert_eq!(
            snapshot.bind_rowan(invalid.syntax().rowan()).unwrap().id(),
            invalid.id()
        );
        assert_eq!(
            snapshot.bind_rowan(issue.syntax().rowan()).unwrap().id(),
            issue.id()
        );
        assert_eq!(snapshot.root_handle().rowan().text().to_string(), source);
    }

    #[test]
    fn nested_pattern_and_type_accessors_keep_exact_child_roles() {
        let source = "proof nested((head, [first, ..rest], TruckResult { score, rank: mut r, .. }, ev .Choice(value)): (&'a mut Comparator<Option<(Int, String)> | [U8]>) -> Result<Bool, Error>, .Some(left) | .None: Option<Int>) where Comparator<Option<Int>>: Callable<(Int, String)> + Send = true\n";
        let snapshot = attach(source);

        let whole = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::WholeBindingPattern)
            .unwrap()
            .cast::<WholeBindingPatternKind>()
            .unwrap();
        assert_eq!(whole.pattern().unwrap().kind(), SyntaxKind::VariantPattern);

        let record = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::RecordPattern)
            .unwrap()
            .cast::<RecordPatternKind>()
            .unwrap();
        let fields = record.fields().unwrap();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].role(), SyntaxRole::Field(0));
        assert_eq!(fields[1].role(), SyntaxRole::Field(1));
        assert_eq!(fields[2].role(), SyntaxRole::Field(2));
        let shorthand = fields[0]
            .cast::<super::node::RecordPatternFieldKind>()
            .unwrap();
        let named = fields[1]
            .cast::<super::node::RecordPatternFieldKind>()
            .unwrap();
        assert!(shorthand.pattern().unwrap().is_none());
        assert_eq!(
            named.pattern().unwrap().unwrap().kind(),
            SyntaxKind::MutableBindingPattern
        );
        assert_eq!(fields[2].kind(), SyntaxKind::RestPattern);
        assert!(
            snapshot
                .nodes()
                .filter(|node| matches!(
                    node.kind(),
                    SyntaxKind::RecordPatternField | SyntaxKind::RestPattern
                ))
                .all(|node| node.pattern_projection().is_none())
        );

        let function = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::FunctionType)
            .unwrap()
            .cast::<FunctionTypeKind>()
            .unwrap();
        assert_eq!(function.parameters().unwrap().len(), 1);
        assert_eq!(
            function.parameters().unwrap()[0].kind(),
            SyntaxKind::ReferenceType
        );
        assert_eq!(
            function.result().unwrap().kind(),
            SyntaxKind::GenericApplicationType
        );

        let generic = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::GenericApplicationType)
            .unwrap()
            .cast::<GenericApplicationTypeKind>()
            .unwrap();
        let arguments = generic.arguments().unwrap();
        assert_eq!(arguments.len(), 1);
        assert_eq!(arguments[0].role(), SyntaxRole::Argument(0));
        assert_eq!(arguments[0].ty().unwrap().kind(), SyntaxKind::SumType);
    }

    #[test]
    fn attached_error_pattern_owns_the_thirteenth_semantic_family() {
        let snapshot = attach("proof invalid(+: I32) = true\n");
        let error = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::ErrorPattern)
            .expect("invalid Pattern remains attached as typed recovery");
        let semantic = FamilyNode::<PatternFamily>::new(error)
            .unwrap()
            .semantic()
            .unwrap();
        assert_eq!(
            semantic.family(),
            crate::patterns::PatternSyntaxFamily::Error
        );
        assert!(
            semantic
                .component(super::PatternComponentRole::Recovery)
                .is_some()
        );
    }

    #[test]
    fn attached_typed_binding_exposes_its_exact_type_child() {
        let snapshot = attach("fn inspect() { let binding: Vec = source_value; }\n");
        let typed = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::TypedBindingPattern)
            .expect("typed binding Pattern");
        let typed = FamilyNode::<PatternFamily>::new(typed)
            .unwrap()
            .semantic()
            .unwrap();
        let children = typed.children().unwrap();
        assert_eq!(children.len(), 1);
        let child = children[0].type_ref().expect("typed-binding type child");
        assert_eq!(child.syntax().source_text(), "Vec");
        assert_eq!(child.path(), &crate::types::TypeRefNodePath::root());
    }

    #[test]
    fn attached_type_projection_covers_final_families_components_and_recovery() {
        use super::AttachedTypeFamily;
        use crate::types::{TypeRefComponentRole, TypeRefRegionPart};

        let source = "proof types(a: Never, b: 32, c: crate.foo.Bar, d: (A, B), e: (A, B) -> C effects {read, write}, f: A | B, g: Vec<A>, h: Iterator<Item = A>, i: Vec<A>::Item, j: &'a mut A, k: [A]) = true\n";
        let snapshot = attach(source);
        let semantic = snapshot
            .nodes()
            .filter(|node| node.kind().is_type_node())
            .map(|node| {
                FamilyNode::<TypeFamily>::new(node)
                    .unwrap()
                    .semantic()
                    .unwrap()
            })
            .collect::<Vec<_>>();

        assert_final_type_families(&semantic);

        let function = semantic
            .iter()
            .find(|node| node.family() == AttachedTypeFamily::Function)
            .unwrap();
        assert_eq!(function.children().unwrap().len(), 3);
        let function_components = function.components();
        for (role, spelling) in [
            (TypeRefComponentRole::FunctionArrow, "->"),
            (TypeRefComponentRole::FunctionEffectOpen, "{"),
            (TypeRefComponentRole::FunctionEffect { ordinal: 0 }, "read"),
            (TypeRefComponentRole::FunctionEffect { ordinal: 1 }, "write"),
            (TypeRefComponentRole::FunctionEffectClose, "}"),
        ] {
            let span = function.component(role).expect("typed component source");
            assert_eq!(&source[span.range().as_range()], spelling);
            let projected = function_components
                .iter()
                .find(|component| component.role() == role)
                .expect("complete component inventory retains the role");
            assert_eq!(projected.source_span(), &span);
        }
        assert!(
            function_components
                .iter()
                .all(|component| component.source_span().source() == snapshot.document().identity())
        );

        let reference = semantic
            .iter()
            .find(|node| node.family() == AttachedTypeFamily::Reference)
            .unwrap();
        for (role, spelling) in [
            (TypeRefComponentRole::ReferenceAmpersand, "&"),
            (
                TypeRefComponentRole::Region(TypeRefRegionPart::NamedApostrophe),
                "'",
            ),
            (
                TypeRefComponentRole::Region(TypeRefRegionPart::NamedName),
                "a",
            ),
            (TypeRefComponentRole::ReferenceMutKeyword, "mut"),
            (TypeRefComponentRole::ReferenceReferent, "A"),
        ] {
            let span = reference
                .component(role)
                .expect("typed reference component");
            assert_eq!(&source[span.range().as_range()], spelling);
        }

        let invalid = attach("proof bad(a: [A; 32], b: _, c: 'a) = true\n");
        let recoveries = invalid
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::ErrorType)
            .map(|node| {
                FamilyNode::<TypeFamily>::new(node)
                    .unwrap()
                    .semantic()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(recoveries.len(), 3);
        assert!(
            recoveries
                .iter()
                .all(|node| node.family() == AttachedTypeFamily::Recovery
                    && node.component(TypeRefComponentRole::Recovery).is_some())
        );
        assert!(
            invalid
                .nodes()
                .filter(|node| node.kind().is_type_node())
                .all(|node| {
                    matches!(node.kind(), SyntaxKind::ErrorType | SyntaxKind::MissingType)
                })
        );
    }

    fn assert_final_type_families(types: &[super::AttachedTypeRefNode]) {
        for family in [
            super::AttachedTypeFamily::Never,
            super::AttachedTypeFamily::ConstInt,
            super::AttachedTypeFamily::Path,
            super::AttachedTypeFamily::Tuple,
            super::AttachedTypeFamily::Function,
            super::AttachedTypeFamily::Choice,
            super::AttachedTypeFamily::Generic,
            super::AttachedTypeFamily::TraitBound,
            super::AttachedTypeFamily::Projection,
            super::AttachedTypeFamily::Reference,
            super::AttachedTypeFamily::Slice,
        ] {
            assert!(
                types.iter().any(|node| node.family() == family),
                "missing {family:?}"
            );
        }
    }

    #[test]
    fn attached_type_projection_rejects_stale_and_foreign_identity() {
        use super::AttachedTypeFamily;

        let source = "proof typed(value: Vec<I32>) = true\n";
        let first = attach_at(source, 1, 1);
        let stale = attach_at(source, 1, 2);
        let foreign = attach_at(source, 2, 1);
        let semantic = |snapshot: &Arc<super::SyntaxSnapshotData>| {
            FamilyNode::<TypeFamily>::new(
                snapshot
                    .nodes()
                    .find(|node| node.kind() == SyntaxKind::GenericApplicationType)
                    .unwrap(),
            )
            .unwrap()
            .semantic()
            .unwrap()
        };
        let first_type = semantic(&first);
        let stale_type = semantic(&stale);
        let foreign_type = semantic(&foreign);
        assert_eq!(first_type.family(), AttachedTypeFamily::Generic);
        assert_eq!(first_type.children().unwrap().len(), 1);
        assert!(matches!(
            first.resolve_exact(&stale_type.syntax()),
            Err(SyntaxLookupError::WrongSnapshot { .. })
        ));
        assert!(matches!(
            first.syntax_node(foreign_type.id()),
            Err(SyntaxLookupError::WrongDatabase { .. })
        ));
    }
}
