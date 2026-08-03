use core::num::{NonZeroU32, NonZeroU64};
use std::collections::BTreeSet;
use std::sync::Arc;

use arcweft_lang_syntax::attachment::node::{FunctionBodyKind, LetStatementKind};
use arcweft_lang_syntax::attachment::{
    AttachedExpressionNode, AttachedPatternNode, AttachedRequiredThreadExpressionBody,
    AttachedStyleBody, AttachedStyleMember, AttachedTypeRefNode, DeclarationBodyNode,
    LetInitializerNode, StatementNode, TypedItemNode,
};
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceDocumentIdentity, SourceName, SourceRange,
};
use serde::Deserialize;
use serde::de::{self, IntoDeserializer};

use super::{
    HirExprSourceRole, HirIdRefSourcePart, HirInsertionPoint, HirInsertionPointError,
    HirItemSourceRole, HirPatternFieldSourcePart, HirPatternRestSourcePart, HirPatternSourceRole,
    HirResolvedSourceRole, HirSourceCommitInvariantError, HirSourceIndex,
    HirSourceIndexLookupError, HirSourceOwnerStatus, HirSourcePresence, HirSourceQuery,
    HirSourceQueryError, HirSourceRequirement, HirSourceSite, HirStmtSourceRole, HirStyleBodyPath,
    HirStyleBodySourcePart, HirStyleSourceRole, HirStyleTokenSourcePart, HirThreadBodySourceRole,
    HirThreadFlowItemSourcePart, HirTypeRegionSourcePart, HirTypeSourceRole,
    HirVariantPatternPayloadSourcePart, StagedHirSourceIndex,
};
use crate::arena::{ArenaSnapshot, StagedArena};
use crate::expr::{
    HirExprKind, HirPoisonState, HirThreadBody, HirThreadBodyOwner, HirThreadFlowItem,
};
use crate::identity::{
    ExprId, HirDatabaseId, HirIdKind, HirModuleId, HirRevision, HirTypedId, IdResolveError, ItemId,
    LocalGeneration, LocalId, PatternId, RawHirId, ScopeId, StmtId, SyntheticKey, SyntheticOwner,
    SyntheticRole, TypeId,
};
use crate::leaf::{
    HirEntityReference, HirIdRef, HirIdRefInvariantError, HirIdRefIssue, HirIdRefRecovery,
    HirIdRefShape, HirIdRefValue, HirName, HirPath, HirPathIssue, HirPathRoot, HirPathSegment,
};
use crate::pattern::{
    HirPattern, HirPatternBinding, HirPatternBindingIssue, HirPatternField, HirPatternFieldIssue,
    HirPatternKind, HirPatternRecordPath, HirPatternRecordPathIssue, HirPatternResolver,
    HirPatternSequenceRest, HirPatternSequenceRestIssue, HirVariantPattern, HirVariantPatternHead,
    HirVariantPatternHeadValue, HirVariantPatternName, HirVariantPatternNameIssue,
    HirVariantPatternPayload,
};
use crate::scope::{HirLocal, HirLocalKind, HirScope, HirScopeKind, HirScopeOwner};
use crate::slot::{SlotSnapshot, StagedSlotTransaction};
use crate::stmt::{
    HirStmt, HirStmtKind, HirStmtPoisonState, HirStmtRecoveryIssue, HirUnsafeAudit,
    HirUnsafeLifetimeBody,
};
use crate::type_ref::{HirType, HirTypeKind, HirTypeResolver};

fn document(id: &str, text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new(id).expect("test source ID"),
        SourceName::Generated,
        text,
    )
    .expect("test source")
}

fn module(slot: u32) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::MIN),
        NonZeroU32::new(slot).expect("nonzero module slot"),
    )
}

fn expr(module: HirModuleId, slot: u32) -> ExprId {
    ExprId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("nonzero HIR slot"),
        HirIdKind::Expr,
    ))
}

fn ty(module: HirModuleId, slot: u32) -> TypeId {
    TypeId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("nonzero HIR slot"),
        HirIdKind::Type,
    ))
}

fn pattern(module: HirModuleId, slot: u32) -> PatternId {
    PatternId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("nonzero HIR slot"),
        HirIdKind::Pattern,
    ))
}

fn stmt(module: HirModuleId, slot: u32) -> StmtId {
    StmtId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("nonzero HIR slot"),
        HirIdKind::Stmt,
    ))
}

fn item(module: HirModuleId, slot: u32) -> ItemId {
    ItemId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("nonzero HIR slot"),
        HirIdKind::Item,
    ))
}

fn scope(module: HirModuleId, slot: u32) -> ScopeId {
    ScopeId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("nonzero HIR slot"),
        HirIdKind::Scope,
    ))
}

fn local(module: HirModuleId, slot: u32) -> LocalId {
    LocalId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("nonzero HIR slot"),
        HirIdKind::Local,
    ))
}

fn span(document: &SourceDocument, start: usize, end: usize) -> HirSourceSite {
    HirSourceSite::Span(
        document
            .span(SourceRange::new(start, end))
            .expect("test source span"),
    )
}

fn staged_index(source: &SourceDocument) -> StagedHirSourceIndex {
    let slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    StagedHirSourceIndex::new(source.identity().clone(), &slots)
}

fn empty_index(source: &SourceDocument) -> HirSourceIndex {
    let slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL)
        .prepare()
        .expect("empty slot proposal");
    HirSourceIndex::empty(source.identity().clone(), slots.snapshot())
}

fn parsed_type(document_id: &str, type_source: &str) -> (ParsedSource, AttachedTypeRefNode) {
    let name = SourceName::path(format!("source-index/{document_id}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/source-index/{document_id}.arcw"
            ))
            .expect("attached type document ID"),
            name.clone(),
            format!("source values: {type_source} {{}}\n"),
        )
        .expect("attached type source"),
    );
    let parsed = SyntaxDatabase::try_new()
        .expect("attached type syntax database")
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("attached type parsed source");
    let item = parsed
        .tree()
        .items()
        .expect("attached source item inventory")
        .into_iter()
        .next()
        .expect("source declaration");
    let TypedItemNode::Source(source) = item else {
        panic!("expected source item concrete family");
    };
    let attached = source
        .source_type()
        .expect("source type access")
        .expect("source declaration type")
        .semantic()
        .expect("attached semantic type");
    (parsed, attached)
}

fn parsed_style(document_id: &str, style_source: &str) -> (ParsedSource, TypedItemNode) {
    let name = SourceName::path(format!("source-index/{document_id}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/source-index/{document_id}.arcw"
            ))
            .expect("attached Style document ID"),
            name.clone(),
            style_source,
        )
        .expect("attached Style source"),
    );
    let parsed = SyntaxDatabase::try_new()
        .expect("attached Style syntax database")
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("attached Style parsed source");
    let item = parsed
        .tree()
        .items()
        .expect("attached Style item inventory")
        .into_iter()
        .next()
        .expect("Style item");
    assert!(matches!(&item, TypedItemNode::Style(_)));
    (parsed, item)
}

fn parsed_statement(document_id: &str, statement: &str) -> (ParsedSource, StatementNode) {
    parsed_statement_source(
        document_id,
        &format!("fn audit() {{\n    {statement}\n}}\n"),
    )
}

fn parsed_pattern(document_id: &str, pattern: &str) -> (ParsedSource, AttachedPatternNode) {
    let (parsed, statement) =
        parsed_statement(document_id, &format!("let {pattern} = source_value;"));
    let statement = statement
        .cast::<LetStatementKind>()
        .expect("let statement concrete family");
    let attached = statement
        .pattern()
        .expect("let statement pattern")
        .semantic()
        .expect("attached semantic pattern");
    (parsed, attached)
}

fn parsed_expression(
    document_id: &str,
    expression: &str,
) -> (ParsedSource, AttachedExpressionNode) {
    let (parsed, statement) = parsed_statement(document_id, &format!("let value = {expression};"));
    let statement = statement
        .cast::<LetStatementKind>()
        .expect("let statement concrete family");
    let initializer = statement
        .initializer()
        .expect("let initializer access")
        .expect("authored let initializer");
    let LetInitializerNode::Expression(initializer) = initializer else {
        panic!("test initializer must remain an expression");
    };
    let attached = initializer
        .semantic()
        .expect("attached semantic expression");
    (parsed, attached)
}

fn parsed_statement_source(document_id: &str, source: &str) -> (ParsedSource, StatementNode) {
    let name = SourceName::path(format!("source-index/{document_id}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/source-index/{document_id}.arcw"
            ))
            .expect("attached statement document ID"),
            name.clone(),
            source,
        )
        .expect("attached statement source"),
    );
    let parsed = SyntaxDatabase::try_new()
        .expect("attached statement syntax database")
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("attached statement parsed source");
    let item = parsed
        .tree()
        .items()
        .expect("attached function item inventory")
        .into_iter()
        .next()
        .expect("function item");
    let Some(DeclarationBodyNode::Body(body)) = item.body().expect("function body access") else {
        panic!("test function must retain an authored body");
    };
    let function = body
        .cast::<FunctionBodyKind>()
        .expect("function body concrete family");
    let attached = function
        .block()
        .expect("function computation block")
        .statements()
        .expect("function statement inventory")
        .into_iter()
        .next()
        .expect("test statement");
    (parsed, attached)
}

fn unsafe_stmt_kind(body_scope: ScopeId) -> HirStmtKind {
    HirStmtKind::UnsafeLifetime {
        audit: HirUnsafeAudit::new(
            HirIdRefValue::Resolved(HirIdRef::absolute(
                HirEntityReference::try_new("unsafe.audit".into()).expect("test unsafe audit ID"),
            )),
            None,
            false,
        ),
        body: HirUnsafeLifetimeBody::Block {
            scope: body_scope,
            statements: Box::new([]),
        },
    }
}

fn frozen_unsafe_statement(
    document_id: &str,
) -> (
    ParsedSource,
    Arc<SlotSnapshot>,
    ArenaSnapshot<HirStmt, StmtId>,
    HirSourceIndex,
    StmtId,
) {
    let (parsed, attached) =
        parsed_statement(document_id, "unsafe lifetime @unsafe.audit { value; };");
    let owner_module = module(1);
    let mut slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let mut scopes = StagedArena::<HirScope, ScopeId>::new();
    let module_scope = scopes
        .allocate_source(
            &mut slots,
            parsed.tree().root().id(),
            HirSourceSite::Span(
                parsed
                    .document()
                    .span(SourceRange::new(0, parsed.source().len()))
                    .expect("test source root span"),
            ),
            HirScope::try_new(
                owner_module,
                HirScopeKind::Module,
                None,
                HirScopeOwner::Module(owner_module),
                Box::new([]),
                Box::new([]),
            )
            .expect("test module scope"),
        )
        .expect("test module scope allocation");

    let mut statements = StagedArena::<HirStmt, StmtId>::new();
    let reservation = statements
        .reserve_source(
            &mut slots,
            attached.id(),
            HirSourceSite::Span(attached.source_span()),
        )
        .expect("unsafe statement reservation");
    let owner = reservation.id();
    let body_scope = scopes
        .allocate_source(
            &mut slots,
            attached.id(),
            HirSourceSite::Span(attached.source_span()),
            HirScope::try_new(
                owner_module,
                HirScopeKind::Block,
                Some(module_scope),
                HirScopeOwner::Stmt(owner),
                Box::new([]),
                Box::new([]),
            )
            .expect("test unsafe body scope"),
        )
        .expect("test unsafe body scope allocation");
    let kind = unsafe_stmt_kind(body_scope);
    let statement =
        HirStmt::try_new(module_scope, kind).expect("test clean unsafe statement payload");
    statements
        .finalize(&mut slots, reservation, statement.clone())
        .expect("test unsafe statement finalization");

    let mut source_index = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    source_index
        .stage_attached_stmt(&parsed, owner, &attached, &statement)
        .expect("test unsafe statement source projection");
    let source_index = source_index
        .commit()
        .expect("test unsafe source-index commit");
    let _scopes = scopes
        .into_snapshot(&mut slots)
        .expect("test scope snapshot");
    let statements = statements
        .into_snapshot(&mut slots)
        .expect("test statement snapshot");
    let prepared = slots.prepare().expect("test unsafe slot proposal");
    (
        parsed,
        Arc::clone(prepared.snapshot()),
        statements,
        source_index,
        owner,
    )
}

struct RootTypeResolver {
    scope: ScopeId,
}

impl HirTypeResolver for RootTypeResolver {
    fn scope_is_live(&self, scope: ScopeId) -> bool {
        scope == self.scope
    }

    fn resolve_type(&self, _scope: ScopeId, _ty: TypeId) -> Option<&HirType> {
        None
    }
}

struct RootPatternResolver {
    scope: ScopeId,
}

static CLEAN_PATTERN_TYPE_STATE: HirPoisonState = HirPoisonState::Clean;

fn binding_pattern_kind(owner: HirModuleId, name: &str) -> HirPatternKind {
    HirPatternKind::Binding(HirPatternBinding::Bound {
        name: HirName::try_new(name.into()).expect("test binding name"),
        local: local(owner, 2),
    })
}

impl HirPatternResolver for RootPatternResolver {
    fn scope_is_live(&self, scope: ScopeId) -> bool {
        scope == self.scope
    }

    fn local_is_visible(&self, scope: ScopeId, _local: LocalId) -> bool {
        scope == self.scope
    }

    fn resolve_type_state(&self, scope: ScopeId, _ty: TypeId) -> Option<&HirPoisonState> {
        (scope == self.scope).then_some(&CLEAN_PATTERN_TYPE_STATE)
    }

    fn resolve_pattern(&self, _scope: ScopeId, _pattern: PatternId) -> Option<&HirPattern> {
        None
    }
}

fn hir_type_path(name: &str) -> HirPath {
    HirPath::try_new(
        HirPathRoot::ImplicitCrate,
        Box::new([HirPathSegment::Identifier(
            HirName::try_new(name.into()).expect("test HIR type name"),
        )]),
    )
    .expect("test HIR type path")
}

fn frozen_root_path_type(
    document_id: &str,
    source_name: &str,
    payload_name: &str,
) -> (
    ParsedSource,
    Arc<SlotSnapshot>,
    ArenaSnapshot<HirType, TypeId>,
    HirSourceIndex,
    TypeId,
) {
    let (parsed, attached) = parsed_type(document_id, source_name);
    let owner_module = module(1);
    let mut slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let mut scopes = StagedArena::<HirScope, ScopeId>::new();
    let scope_site = HirSourceSite::Span(
        parsed
            .document()
            .span(SourceRange::new(0, parsed.source().len()))
            .expect("test source root span"),
    );
    let scope = scopes
        .allocate_source(
            &mut slots,
            parsed.tree().root().id(),
            scope_site,
            HirScope::try_new(
                owner_module,
                HirScopeKind::Module,
                None,
                HirScopeOwner::Module(owner_module),
                Box::new([]),
                Box::new([]),
            )
            .expect("test module scope"),
        )
        .expect("test scope allocation");

    let mut types = StagedArena::<HirType, TypeId>::new();
    let reservation = types
        .reserve_source(
            &mut slots,
            attached.id(),
            HirSourceSite::Span(attached.whole_source_span()),
        )
        .expect("test type reservation");
    let owner = reservation.id();
    types
        .finalize(
            &mut slots,
            reservation,
            HirType::try_new(
                owner,
                HirTypeKind::Path(hir_type_path(payload_name)),
                scope,
                HirPoisonState::Clean,
                &RootTypeResolver { scope },
            )
            .expect("test root path type"),
        )
        .expect("test type finalization");

    let mut source_index = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    source_index
        .stage_attached_type(&parsed, owner, &attached)
        .expect("test attached type projection");
    let source_index = source_index.commit().expect("test source-index commit");
    let _scopes = scopes
        .into_snapshot(&mut slots)
        .expect("test scope snapshot");
    let types = types.into_snapshot(&mut slots).expect("test type snapshot");
    let prepared = slots.prepare().expect("test slot proposal");
    (
        parsed,
        Arc::clone(prepared.snapshot()),
        types,
        source_index,
        owner,
    )
}

fn frozen_binding_pattern(
    document_id: &str,
    manifest_kind: &HirPatternKind,
    payload_kind: HirPatternKind,
) -> (
    ParsedSource,
    Arc<SlotSnapshot>,
    ArenaSnapshot<HirPattern, PatternId>,
    HirSourceIndex,
    PatternId,
) {
    let (parsed, attached) = parsed_pattern(document_id, "binding");
    let owner_module = module(1);
    let pattern_scope = scope(owner_module, 1);
    let mut slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let mut patterns = StagedArena::<HirPattern, PatternId>::new();
    let reservation = patterns
        .reserve_source(
            &mut slots,
            attached.id(),
            HirSourceSite::Span(attached.whole_source_span()),
        )
        .expect("test pattern reservation");
    let owner = reservation.id();
    let mut locals = StagedArena::<HirLocal, LocalId>::new();
    let bound_local = locals
        .allocate_synthetic(
            &mut slots,
            SyntheticKey::try_new(
                SyntheticOwner::Pattern(owner),
                SyntheticRole::DestructuredBinding,
                0,
            )
            .expect("test binding synthetic key"),
            HirSourceSite::Span(attached.whole_source_span()),
            HirLocal::try_new(
                pattern_scope,
                HirLocalKind::PatternBinding,
                HirName::try_new("binding".into()).expect("test binding Local name"),
                LocalGeneration::FIRST,
                Some(owner),
                None,
                false,
                false,
            )
            .expect("test binding Local payload"),
        )
        .expect("test binding Local allocation");
    assert_eq!(bound_local, local(owner_module, 2));
    patterns
        .finalize(
            &mut slots,
            reservation,
            HirPattern::try_new(
                payload_kind,
                pattern_scope,
                HirPoisonState::Clean,
                &RootPatternResolver {
                    scope: pattern_scope,
                },
            )
            .expect("test binding pattern payload"),
        )
        .expect("test pattern finalization");

    let mut source_index = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    source_index
        .stage_attached_pattern(&parsed, owner, &attached, manifest_kind)
        .expect("test attached pattern projection");
    let source_index = source_index
        .commit()
        .expect("test pattern source-index commit");
    let patterns = patterns
        .into_snapshot(&mut slots)
        .expect("test pattern snapshot");
    let _locals = locals
        .into_snapshot(&mut slots)
        .expect("test Local snapshot");
    let prepared = slots.prepare().expect("test pattern slot proposal");
    (
        parsed,
        Arc::clone(prepared.snapshot()),
        patterns,
        source_index,
        owner,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "the fixture assembles both exact syntax-owned Type and Pattern arenas for freeze parity"
)]
fn frozen_typed_binding_pattern(
    document_id: &str,
    exact_type_owner: bool,
) -> (
    ParsedSource,
    Arc<SlotSnapshot>,
    ArenaSnapshot<HirPattern, PatternId>,
    HirSourceIndex,
) {
    let (parsed, attached) = parsed_pattern(document_id, "binding: Vec");
    let attached_children = attached.children().expect("typed-binding child inventory");
    let attached_type = attached_children
        .iter()
        .find_map(|child| child.type_ref().cloned())
        .unwrap_or_else(|| {
            panic!(
                "typed binding owns one attached Type child; family={:?}, children={attached_children:?}",
                attached.family()
            )
        });
    let owner_module = module(1);
    let pattern_scope = scope(owner_module, 1);
    let mut slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let mut types = StagedArena::<HirType, TypeId>::new();
    let (type_syntax, type_site) = if exact_type_owner {
        (
            attached_type.id(),
            HirSourceSite::Span(attached_type.whole_source_span()),
        )
    } else {
        (
            parsed.tree().root().id(),
            HirSourceSite::Span(
                parsed
                    .document()
                    .span(SourceRange::new(0, parsed.source().len()))
                    .expect("test source root span"),
            ),
        )
    };
    let type_reservation = types
        .reserve_source(&mut slots, type_syntax, type_site)
        .expect("typed-binding Type reservation");
    let ty = type_reservation.id();
    types
        .finalize(
            &mut slots,
            type_reservation,
            HirType::try_new(
                ty,
                HirTypeKind::Path(hir_type_path("Vec")),
                pattern_scope,
                HirPoisonState::Clean,
                &RootTypeResolver {
                    scope: pattern_scope,
                },
            )
            .expect("typed-binding Type payload"),
        )
        .expect("typed-binding Type finalization");

    let mut patterns = StagedArena::<HirPattern, PatternId>::new();
    let pattern_reservation = patterns
        .reserve_source(
            &mut slots,
            attached.id(),
            HirSourceSite::Span(attached.whole_source_span()),
        )
        .expect("typed-binding Pattern reservation");
    let pattern_owner = pattern_reservation.id();
    let mut locals = StagedArena::<HirLocal, LocalId>::new();
    let bound_local = locals
        .allocate_synthetic(
            &mut slots,
            SyntheticKey::try_new(
                SyntheticOwner::Pattern(pattern_owner),
                SyntheticRole::DestructuredBinding,
                0,
            )
            .expect("typed-binding synthetic key"),
            HirSourceSite::Span(attached.whole_source_span()),
            HirLocal::try_new(
                pattern_scope,
                HirLocalKind::PatternBinding,
                HirName::try_new("binding".into()).expect("typed-binding Local name"),
                LocalGeneration::FIRST,
                Some(pattern_owner),
                Some(ty),
                false,
                false,
            )
            .expect("typed-binding Local payload"),
        )
        .expect("typed-binding Local allocation");
    assert_eq!(bound_local, local(owner_module, 3));
    patterns
        .finalize(
            &mut slots,
            pattern_reservation,
            HirPattern::try_new(
                HirPatternKind::TypedBinding {
                    binding: HirPatternBinding::Bound {
                        name: HirName::try_new("binding".into()).expect("typed binding name"),
                        local: local(owner_module, 3),
                    },
                    ty,
                },
                pattern_scope,
                HirPoisonState::Clean,
                &RootPatternResolver {
                    scope: pattern_scope,
                },
            )
            .expect("typed-binding Pattern payload"),
        )
        .expect("typed-binding Pattern finalization");

    let mut source_index = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    source_index
        .stage_attached_pattern(
            &parsed,
            pattern_owner,
            &attached,
            patterns
                .resolve_staged(&slots, pattern_owner)
                .expect("staged typed-binding Pattern")
                .kind(),
        )
        .expect("typed-binding Pattern source projection");
    let source_index = source_index
        .commit()
        .expect("typed-binding Pattern source-index commit");
    let _types = types
        .into_snapshot(&mut slots)
        .expect("typed-binding Type snapshot");
    let patterns = patterns
        .into_snapshot(&mut slots)
        .expect("typed-binding Pattern snapshot");
    let _locals = locals
        .into_snapshot(&mut slots)
        .expect("typed-binding Local snapshot");
    let prepared = slots.prepare().expect("typed-binding slot proposal");
    (
        parsed,
        Arc::clone(prepared.snapshot()),
        patterns,
        source_index,
    )
}

enum IdentityValue {
    Text(String),
    Length(u64),
}

impl IntoDeserializer<'_, de::value::Error> for IdentityValue {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> de::Deserializer<'de> for IdentityValue {
    type Error = de::value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self {
            Self::Text(value) => visitor.visit_string(value),
            Self::Length(value) => visitor.visit_u64(value),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct map
        struct enum identifier ignored_any
    }
}

fn identity_with_length(document: &SourceDocument, source_len: u64) -> SourceDocumentIdentity {
    let fields = [
        (
            "id",
            IdentityValue::Text(document.identity().id().as_str().to_owned()),
        ),
        (
            "revision",
            IdentityValue::Text(document.identity().revision().to_hex()),
        ),
        ("source_len", IdentityValue::Length(source_len)),
    ];
    SourceDocumentIdentity::deserialize(de::value::MapDeserializer::new(fields.into_iter()))
        .expect("test source identity")
}

fn expression_query(owner: ExprId, role: HirExprSourceRole) -> HirSourceQuery {
    HirSourceQuery::Expr { owner, role }
}

#[test]
fn insertion_point_retains_exact_revision_and_checks_utf8_boundaries() {
    let source = document("arcw:/source-index/insertion", "aé");

    let start = HirInsertionPoint::try_new(&source, 0).expect("start insertion");
    let end = HirInsertionPoint::try_new(&source, 3).expect("end insertion");

    assert_eq!(start.source_identity(), source.identity());
    assert_eq!(start.offset(), 0);
    assert_eq!(end.offset(), 3);
    assert_eq!(
        HirInsertionPoint::try_new(&source, 2),
        Err(HirInsertionPointError::NonUtf8Boundary { offset: 2 })
    );
    assert_eq!(
        HirInsertionPoint::try_new(&source, 4),
        Err(HirInsertionPointError::OutOfDocument {
            offset: 4,
            document_len: 3,
        })
    );
}

#[test]
fn transaction_commits_present_and_absent_optional_components() {
    let source = document("arcw:/source-index/components", "123ms");
    let owner = expr(module(1), 1);
    let body = expression_query(owner, HirExprSourceRole::LiteralBody);
    let suffix = expression_query(owner, HirExprSourceRole::LiteralSuffix);
    let unit = expression_query(owner, HirExprSourceRole::LiteralUnit);
    let mut staged = staged_index(&source);
    staged
        .require(&body, HirSourceRequirement::Required)
        .expect("body requirement");
    staged
        .require(&suffix, HirSourceRequirement::Optional)
        .expect("suffix requirement");
    staged
        .require(&unit, HirSourceRequirement::Optional)
        .expect("unit requirement");
    staged
        .stage(&body, span(&source, 0, 3))
        .expect("body component");
    staged
        .stage(&unit, span(&source, 3, 5))
        .expect("unit component");
    let index = staged.commit().expect("source index commit");

    let body_lookup = index
        .lookup(source.identity(), source.identity(), &body, |_| {
            Ok(HirResolvedSourceRole::component(
                HirSourceRequirement::Required,
                HirSourceOwnerStatus::Clean,
            ))
        })
        .expect("body lookup");
    let suffix_lookup = index
        .lookup(source.identity(), source.identity(), &suffix, |_| {
            Ok(HirResolvedSourceRole::component(
                HirSourceRequirement::Optional,
                HirSourceOwnerStatus::Clean,
            ))
        })
        .expect("suffix lookup");
    let unit_lookup = index
        .lookup(source.identity(), source.identity(), &unit, |_| {
            Ok(HirResolvedSourceRole::component(
                HirSourceRequirement::Optional,
                HirSourceOwnerStatus::Poisoned,
            ))
        })
        .expect("unit lookup");

    assert!(matches!(
        body_lookup.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));
    assert_eq!(suffix_lookup.presence(), HirSourcePresence::AbsentOptional);
    assert!(matches!(
        unit_lookup.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));
    assert_eq!(unit_lookup.owner_status(), HirSourceOwnerStatus::Poisoned);
    assert_eq!(index.component_count(), 2);
}

#[test]
fn pattern_optional_and_type_elision_use_the_same_typed_index() {
    let source = document("arcw:/source-index/all-owner-kinds", "&Value");
    let module = module(1);
    let pattern_owner = pattern(module, 1);
    let type_owner = ty(module, 2);
    let pattern_close = HirSourceQuery::Pattern {
        owner: pattern_owner,
        role: HirPatternSourceRole::VariantPayload(
            HirVariantPatternPayloadSourcePart::CloseDelimiter,
        ),
    };
    let elision = HirSourceQuery::Type {
        owner: type_owner,
        role: HirTypeSourceRole::Region(HirTypeRegionSourcePart::ElisionInsertion),
    };
    let mut staged = staged_index(&source);
    staged
        .require(&pattern_close, HirSourceRequirement::Optional)
        .expect("optional pattern component");
    staged
        .require(&elision, HirSourceRequirement::Required)
        .expect("required type component");
    staged
        .stage(
            &elision,
            HirSourceSite::Insertion(
                HirInsertionPoint::try_new(&source, 1).expect("elision insertion"),
            ),
        )
        .expect("elision source component");
    let index = staged.commit().expect("typed source index");

    let pattern_lookup = index
        .lookup(source.identity(), source.identity(), &pattern_close, |_| {
            Ok(HirResolvedSourceRole::component(
                HirSourceRequirement::Optional,
                HirSourceOwnerStatus::Clean,
            ))
        })
        .expect("pattern source lookup");
    let type_lookup = index
        .lookup(source.identity(), source.identity(), &elision, |_| {
            Ok(HirResolvedSourceRole::component(
                HirSourceRequirement::Required,
                HirSourceOwnerStatus::Poisoned,
            ))
        })
        .expect("type source lookup");

    assert_eq!(pattern_lookup.presence(), HirSourcePresence::AbsentOptional);
    assert!(matches!(
        type_lookup.presence(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
    ));
    assert_eq!(type_lookup.owner_status(), HirSourceOwnerStatus::Poisoned);
}

#[test]
fn unsafe_audit_insertion_uses_the_unified_typed_index() {
    let source = document("arcw:/source-index/unsafe-audit", "unsafe lifetime {");
    let owner = stmt(module(1), 1);
    let query = HirSourceQuery::Stmt {
        owner,
        role: HirStmtSourceRole::UnsafeAuditInsertion,
    };
    let insertion = HirSourceSite::Insertion(
        HirInsertionPoint::try_new(&source, source.text().len()).expect("audit insertion"),
    );
    let mut staged = staged_index(&source);
    staged
        .require(&query, HirSourceRequirement::Required)
        .expect("audit insertion requirement");
    staged
        .stage(&query, insertion.clone())
        .expect("audit insertion component");
    let index = staged.commit().expect("source index commit");

    let lookup = index
        .lookup(source.identity(), source.identity(), &query, |_| {
            Ok(HirResolvedSourceRole::component(
                HirSourceRequirement::Required,
                HirSourceOwnerStatus::Clean,
            ))
        })
        .expect("audit insertion lookup");

    assert_eq!(lookup.presence(), HirSourcePresence::Present(&insertion));
    assert_eq!(lookup.owner_status(), HirSourceOwnerStatus::Clean);
}

#[test]
fn whole_is_borrowed_from_slot_owner_and_never_enters_component_map() {
    let source = document("arcw:/source-index/whole", "value");
    let owner = expr(module(1), 1);
    let whole_query = expression_query(owner, HirExprSourceRole::Whole);
    let whole_site = span(&source, 0, 5);
    let index = empty_index(&source);

    let lookup = index
        .lookup(source.identity(), source.identity(), &whole_query, |_| {
            Ok(HirResolvedSourceRole::whole(
                &whole_site,
                HirSourceOwnerStatus::Clean,
            ))
        })
        .expect("whole lookup");
    assert_eq!(lookup.presence(), HirSourcePresence::Present(&whole_site));
    assert_eq!(index.component_count(), 0);

    let mut staged = staged_index(&source);
    assert_eq!(
        staged.stage(&whole_query, whole_site.clone()),
        Err(HirSourceCommitInvariantError::WholeComponent {
            query: whole_query.clone()
        })
    );
    assert_eq!(
        staged.require(&whole_query, HirSourceRequirement::Required),
        Err(HirSourceCommitInvariantError::TransactionPoisoned)
    );
}

#[test]
fn related_role_borrows_an_authoritative_site_without_a_parallel_component_row() {
    let source = document("arcw:/source-index/related", "item");
    let owner = expr(module(1), 1);
    let query = expression_query(owner, HirExprSourceRole::Operand);
    let related_site = span(&source, 0, 4);
    let index = empty_index(&source);

    let lookup = index
        .lookup(source.identity(), source.identity(), &query, |_| {
            Ok(HirResolvedSourceRole::related(
                HirSourcePresence::Present(&related_site),
                HirSourceOwnerStatus::Poisoned,
            ))
        })
        .expect("related lookup");

    assert_eq!(lookup.presence(), HirSourcePresence::Present(&related_site));
    assert_eq!(lookup.owner_status(), HirSourceOwnerStatus::Poisoned);
    assert_eq!(index.component_count(), 0);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the source-manifest fixture proves slot, syntax, delimiter, item, and relational ownership together"
)]
fn thread_expression_body_manifest_stores_only_delimiters_and_item_wholes() {
    let (parsed, attached) =
        parsed_expression("thread-body-manifest", "thread { let inner = unit; }");
    let thread = attached.thread().expect("Thread expression family");
    let AttachedRequiredThreadExpressionBody::Present(attached_body) =
        thread.statement_body().expect("attached Thread body")
    else {
        panic!("authored Thread body must be present");
    };
    let [attached_item] = attached_body.items() else {
        panic!("test Thread body must contain one direct item");
    };

    let mut slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    let expression = slots
        .reserve_source::<ExprId>(
            attached.id(),
            HirSourceSite::Span(attached.whole_source_span()),
            false,
        )
        .expect("Thread expression slot")
        .id();
    slots
        .bind_payload_poison(expression, false)
        .expect("Thread expression payload state");
    let body_scope = slots
        .reserve_source::<ScopeId>(
            attached_body.syntax().id(),
            HirSourceSite::Span(attached_body.syntax().source_span()),
            false,
        )
        .expect("Thread body scope slot")
        .id();
    slots
        .bind_payload_poison(body_scope, false)
        .expect("Thread body scope payload state");
    let statement = slots
        .reserve_source::<StmtId>(
            attached_item.syntax().id(),
            HirSourceSite::Span(attached_item.syntax().source_span()),
            false,
        )
        .expect("Thread body statement slot")
        .id();
    slots
        .bind_payload_poison(statement, false)
        .expect("Thread body statement payload state");

    let owner = HirThreadBodyOwner::ThreadExpression(expression);
    let body = HirThreadBody::try_new(
        owner,
        body_scope,
        Box::new([HirThreadFlowItem::Statement(statement)]),
    )
    .expect("semantic Thread body");
    let mut staged = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    staged
        .bind_syntax_owner(SyntheticOwner::Expr(expression), attached.id())
        .expect("Thread expression syntax owner");
    staged
        .stage_attached_thread_expression_body(
            &parsed,
            owner,
            &AttachedRequiredThreadExpressionBody::Present(attached_body.clone()),
            &body,
        )
        .expect("Thread body source manifest");
    let prepared = slots.prepare().expect("prepared Thread body slots");
    let index = staged.commit().expect("committed Thread body manifest");

    assert!(index.validates_prepared(prepared.snapshot(), parsed.document().identity()));
    assert!(index.validates_attached_thread_expression_body(
        &parsed,
        prepared.snapshot(),
        owner,
        &AttachedRequiredThreadExpressionBody::Present(attached_body),
        &body,
    ));
    assert_eq!(index.component_count(), 3);
    assert_eq!(
        index.requirement(&HirSourceQuery::ThreadBody {
            owner,
            role: HirThreadBodySourceRole::Whole,
        }),
        None,
        "body Whole is a relation to the scope slot",
    );
    assert_eq!(
        index.requirement(&HirSourceQuery::ThreadBody {
            owner,
            role: HirThreadBodySourceRole::Item {
                ordinal: 0,
                part: HirThreadFlowItemSourcePart::ChildWhole,
            },
        }),
        None,
        "ChildWhole must borrow the child slot rather than copy its site",
    );
}

#[test]
fn typed_resolution_and_role_errors_precede_source_identity_checks() {
    let retained = document("arcw:/source-index/retained", "x");
    let foreign_source = document("arcw:/source-index/foreign", "x");
    let expected_module = module(1);
    let foreign_module = module(2);
    let owner = expr(expected_module, 1);
    let foreign_owner = expr(foreign_module, 1);
    let query = expression_query(owner, HirExprSourceRole::Operand);
    let foreign_query = expression_query(foreign_owner, HirExprSourceRole::Operand);
    let index = empty_index(&retained);

    let role_error = HirSourceQueryError::role_not_applicable(&query);
    assert_eq!(
        index.lookup(
            retained.identity(),
            foreign_source.identity(),
            &query,
            |_| Err(role_error.clone()),
        ),
        Err(HirSourceIndexLookupError::Query(role_error))
    );

    let resolve_error = HirSourceQueryError::resolve(
        &foreign_query,
        IdResolveError::WrongModule {
            expected: expected_module,
            actual: foreign_module,
        },
    );
    assert_eq!(
        index.lookup(
            retained.identity(),
            foreign_source.identity(),
            &foreign_query,
            |_| Err(resolve_error.clone()),
        ),
        Err(HirSourceIndexLookupError::Query(resolve_error))
    );

    assert_eq!(
        index.lookup(
            retained.identity(),
            foreign_source.identity(),
            &query,
            |_| {
                Ok(HirResolvedSourceRole::component(
                    HirSourceRequirement::Optional,
                    HirSourceOwnerStatus::Clean,
                ))
            },
        ),
        Err(HirSourceIndexLookupError::Query(
            HirSourceQueryError::WrongSourceDocument {
                expected: retained.identity().id().clone(),
                actual: foreign_source.identity().id().clone(),
            }
        ))
    );
}

#[test]
fn source_revision_is_checked_before_retained_length() {
    let current = document("arcw:/source-index/revision", "current");
    let stale = document("arcw:/source-index/revision", "stale");
    let owner = expr(module(1), 1);
    let query = expression_query(owner, HirExprSourceRole::LiteralBody);
    let index = empty_index(&current);

    assert_eq!(
        index.lookup(current.identity(), stale.identity(), &query, |_| {
            Ok(HirResolvedSourceRole::component(
                HirSourceRequirement::Optional,
                HirSourceOwnerStatus::Clean,
            ))
        },),
        Err(HirSourceIndexLookupError::Query(
            HirSourceQueryError::StaleSourceRevision {
                expected: current.identity().revision(),
                actual: stale.identity().revision(),
            }
        ))
    );
}

#[test]
fn retained_length_mismatch_is_rejected_after_id_and_revision_match() {
    let current = document("arcw:/source-index/length", "current");
    let wrong_length = identity_with_length(
        &current,
        current
            .identity()
            .source_len()
            .checked_add(1)
            .expect("test source length"),
    );
    let owner = expr(module(1), 1);
    let query = expression_query(owner, HirExprSourceRole::LiteralBody);
    let index = empty_index(&current);

    assert_eq!(
        index.lookup(current.identity(), &wrong_length, &query, |_| {
            Ok(HirResolvedSourceRole::component(
                HirSourceRequirement::Optional,
                HirSourceOwnerStatus::Clean,
            ))
        },),
        Err(HirSourceIndexLookupError::Query(
            HirSourceQueryError::SourceLengthMismatch {
                expected: current.identity().source_len(),
                actual: wrong_length.source_len(),
            }
        ))
    );
}

#[test]
fn typed_error_constructors_preserve_owner_role_and_length() {
    let module_id = module(1);
    let foreign_module = module(2);
    let type_id = ty(module_id, 2);
    let type_query = HirSourceQuery::Type {
        owner: type_id,
        role: HirTypeSourceRole::Region(HirTypeRegionSourcePart::NamedName),
    };
    let statement = stmt(module_id, 3);
    let statement_query = HirSourceQuery::Stmt {
        owner: statement,
        role: HirStmtSourceRole::UnsafeAuditInsertion,
    };

    assert_eq!(
        HirSourceQueryError::role_not_applicable(&type_query),
        HirSourceQueryError::TypeRoleNotApplicable {
            owner: type_id,
            role: HirTypeSourceRole::Region(HirTypeRegionSourcePart::NamedName),
        }
    );
    assert_eq!(
        HirSourceQueryError::resolve(
            &statement_query,
            IdResolveError::WrongModule {
                expected: module_id,
                actual: foreign_module,
            },
        ),
        HirSourceQueryError::StmtResolve {
            owner: statement,
            error: IdResolveError::WrongModule {
                expected: module_id,
                actual: foreign_module,
            },
        }
    );
}

#[test]
fn type_payload_rejects_one_over_ordinals_before_manifest_lookup() {
    let owner = ty(module(1), 1);
    let kind = HirTypeKind::Path(hir_type_path("Vec"));

    assert_eq!(
        kind.validate_source_role(owner, HirTypeSourceRole::PathSegment { ordinal: 0 }),
        Ok(())
    );
    assert_eq!(
        kind.validate_source_role(owner, HirTypeSourceRole::PathSegment { ordinal: 1 }),
        Err(HirSourceQueryError::TypeOrdinalOutOfBounds {
            owner,
            role: HirTypeSourceRole::PathSegment { ordinal: 1 },
            length: 1,
        })
    );
    assert_eq!(
        kind.validate_source_role(owner, HirTypeSourceRole::TupleOpen),
        Err(HirSourceQueryError::TypeRoleNotApplicable {
            owner,
            role: HirTypeSourceRole::TupleOpen,
        })
    );
}

#[test]
fn required_role_validation_rejects_missing_and_undeclared_rows() {
    let source = document("arcw:/source-index/requirements", "x");
    let owner = expr(module(1), 1);
    let required = expression_query(owner, HirExprSourceRole::Operand);
    let unexpected = expression_query(owner, HirExprSourceRole::Operator);
    let mut missing = staged_index(&source);
    missing
        .require(&required, HirSourceRequirement::Required)
        .expect("required role");
    assert!(matches!(
        missing.commit(),
        Err(HirSourceCommitInvariantError::MissingRequiredComponent { query })
            if query == required
    ));

    let mut undeclared = staged_index(&source);
    undeclared
        .stage(&unexpected, span(&source, 0, 1))
        .expect("staged component");
    assert!(matches!(
        undeclared.commit(),
        Err(HirSourceCommitInvariantError::UndeclaredComponent { query })
            if query == unexpected
    ));
}

#[test]
fn conflict_poisoning_and_drop_leave_published_snapshot_unchanged() {
    let source = document("arcw:/source-index/rollback", "ab");
    let owner = expr(module(1), 1);
    let query = expression_query(owner, HirExprSourceRole::Operand);
    let mut accepted = staged_index(&source);
    accepted
        .require(&query, HirSourceRequirement::Required)
        .expect("accepted requirement");
    accepted
        .stage(&query, span(&source, 0, 1))
        .expect("accepted component");
    let published = accepted.commit().expect("published source index");

    let mut rejected = staged_index(&source);
    rejected
        .require(&query, HirSourceRequirement::Required)
        .expect("rejected requirement");
    rejected
        .stage(&query, span(&source, 0, 1))
        .expect("first staged view");
    assert_eq!(
        rejected.stage(&query, span(&source, 1, 2)),
        Err(HirSourceCommitInvariantError::ConflictingComponent {
            query: query.clone()
        })
    );
    assert!(matches!(
        rejected.commit(),
        Err(HirSourceCommitInvariantError::TransactionPoisoned)
    ));

    assert_eq!(published.component_count(), 1);
}

#[test]
fn foreign_component_source_poisoning_is_typed_and_atomic() {
    let source = document("arcw:/source-index/source-a", "a");
    let foreign = document("arcw:/source-index/source-b", "b");
    let owner = expr(module(1), 1);
    let query = expression_query(owner, HirExprSourceRole::Operand);
    let mut staged = staged_index(&source);
    staged
        .require(&query, HirSourceRequirement::Required)
        .expect("source requirement");

    assert_eq!(
        staged.stage(&query, span(&foreign, 0, 1)),
        Err(HirSourceCommitInvariantError::WrongSourceDocument {
            expected: source.identity().id().clone(),
            actual: foreign.identity().id().clone(),
        })
    );
    assert!(matches!(
        staged.commit(),
        Err(HirSourceCommitInvariantError::TransactionPoisoned)
    ));
}

#[test]
fn exact_query_key_uses_typed_owner_and_role_not_source_position() {
    let source = document("arcw:/source-index/typed-key", "x");
    let module = module(1);
    let first_owner = expr(module, 1);
    let second_owner = expr(module, 2);
    let first = expression_query(first_owner, HirExprSourceRole::Operand);
    let second = expression_query(second_owner, HirExprSourceRole::Operand);
    let mut staged = staged_index(&source);
    for query in [&first, &second] {
        staged
            .require(query, HirSourceRequirement::Required)
            .expect("typed requirement");
        staged
            .stage(query, span(&source, 0, 1))
            .expect("typed component");
    }
    let index = staged.commit().expect("typed-key commit");

    assert_ne!(first, second);
    assert_eq!(index.component_count(), 2);
}

#[test]
fn style_item_components_use_the_same_prepared_revision_bound_index() {
    let (parsed, attached) = parsed_style(
        "style-item-owner",
        "style theme { Button:hover { color = red } }\n",
    );
    let owner_module = module(1);
    let mut slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let reservation = slots
        .reserve_source::<ItemId>(
            attached.id(),
            HirSourceSite::Span(attached.source_span()),
            false,
        )
        .expect("source-backed Style item slot");
    let owner = reservation.id();
    slots
        .bind_payload_poison(owner, false)
        .expect("Style item payload state");

    let required = HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Style(HirStyleSourceRole::ItemId),
    };
    let optional = HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Style(HirStyleSourceRole::Body {
            path: HirStyleBodyPath::root(),
            part: HirStyleBodySourcePart::RulePart {
                rule: 0,
                sequence: 0,
            },
        }),
    };
    let mut staged = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    staged
        .require(&required, HirSourceRequirement::Required)
        .expect("required Style ID role");
    staged
        .require(&optional, HirSourceRequirement::Optional)
        .expect("optional recovered Style role");
    staged
        .stage(&required, span(parsed.document(), 6, 11))
        .expect("Style ID component");

    let prepared = slots.prepare().expect("prepared Style item slot");
    let index = staged.commit().expect("Style item source index");

    assert_eq!(required.owner(), SyntheticOwner::Item(owner));
    assert!(index.validates_prepared(prepared.snapshot(), parsed.document().identity()));
    assert_eq!(
        index.requirement(&required),
        Some(HirSourceRequirement::Required)
    );
    assert_eq!(
        index.requirement(&optional),
        Some(HirSourceRequirement::Optional)
    );
    let required_lookup = index
        .lookup(
            parsed.document().identity(),
            parsed.document().identity(),
            &required,
            |_| {
                Ok(HirResolvedSourceRole::component(
                    HirSourceRequirement::Required,
                    HirSourceOwnerStatus::Clean,
                ))
            },
        )
        .expect("required Style source lookup");
    let optional_lookup = index
        .lookup(
            parsed.document().identity(),
            parsed.document().identity(),
            &optional,
            |_| {
                Ok(HirResolvedSourceRole::component(
                    HirSourceRequirement::Optional,
                    HirSourceOwnerStatus::Clean,
                ))
            },
        )
        .expect("optional Style source lookup");

    assert!(matches!(
        required_lookup.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));
    assert_eq!(required_lookup.owner_status(), HirSourceOwnerStatus::Clean);
    assert_eq!(
        optional_lookup.presence(),
        HirSourcePresence::AbsentOptional
    );
    assert_eq!(optional_lookup.owner_status(), HirSourceOwnerStatus::Clean);
}

#[test]
fn style_item_resolution_and_invalid_role_paths_precede_source_checks() {
    let retained = document("arcw:/source-index/style-retained", "style theme {}\n");
    let foreign_source = document("arcw:/source-index/style-foreign", "style theme {}\n");
    let stale_source = document("arcw:/source-index/style-retained", "style other {}\n");
    let expected_module = module(1);
    let foreign_module = module(2);
    let owner = item(expected_module, 1);
    let foreign_owner = item(foreign_module, 1);
    let wrong_family = HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Style(HirStyleSourceRole::Token {
            ordinal: 0,
            part: HirStyleTokenSourcePart::Key,
        }),
    };
    let invalid_path = HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Style(HirStyleSourceRole::Body {
            path: HirStyleBodyPath::from_ordinals(vec![7, 11].into_boxed_slice()),
            part: HirStyleBodySourcePart::ClauseField {
                environment: 3,
                clause: 5,
            },
        }),
    };
    let foreign_query = HirSourceQuery::Item {
        owner: foreign_owner,
        role: HirItemSourceRole::Style(HirStyleSourceRole::ItemId),
    };
    let index = empty_index(&retained);

    let wrong_family_error = HirSourceQueryError::role_not_applicable(&wrong_family);
    assert_eq!(
        index.lookup(
            retained.identity(),
            foreign_source.identity(),
            &wrong_family,
            |_| Err(wrong_family_error.clone()),
        ),
        Err(HirSourceIndexLookupError::Query(wrong_family_error))
    );

    let invalid_path_error = HirSourceQueryError::role_not_applicable(&invalid_path);
    assert_eq!(
        index.lookup(
            retained.identity(),
            foreign_source.identity(),
            &invalid_path,
            |_| Err(invalid_path_error.clone()),
        ),
        Err(HirSourceIndexLookupError::Query(invalid_path_error.clone()))
    );
    assert_eq!(
        invalid_path_error,
        HirSourceQueryError::ItemRoleNotApplicable {
            owner,
            role: HirItemSourceRole::Style(HirStyleSourceRole::Body {
                path: HirStyleBodyPath::from_ordinals(vec![7, 11].into_boxed_slice()),
                part: HirStyleBodySourcePart::ClauseField {
                    environment: 3,
                    clause: 5,
                },
            }),
        }
    );

    let resolve_error = HirSourceQueryError::resolve(
        &foreign_query,
        IdResolveError::WrongModule {
            expected: expected_module,
            actual: foreign_module,
        },
    );
    assert_eq!(
        resolve_error,
        HirSourceQueryError::ItemResolve {
            owner: foreign_owner,
            error: IdResolveError::WrongModule {
                expected: expected_module,
                actual: foreign_module,
            },
        }
    );
    assert_eq!(
        index.lookup(
            retained.identity(),
            foreign_source.identity(),
            &foreign_query,
            |_| Err(resolve_error.clone()),
        ),
        Err(HirSourceIndexLookupError::Query(resolve_error))
    );

    assert_eq!(
        index.lookup(
            retained.identity(),
            foreign_source.identity(),
            &wrong_family,
            |_| {
                Ok(HirResolvedSourceRole::component(
                    HirSourceRequirement::Optional,
                    HirSourceOwnerStatus::Clean,
                ))
            },
        ),
        Err(HirSourceIndexLookupError::Query(
            HirSourceQueryError::WrongSourceDocument {
                expected: retained.identity().id().clone(),
                actual: foreign_source.identity().id().clone(),
            }
        ))
    );
    assert_eq!(
        index.lookup(
            retained.identity(),
            stale_source.identity(),
            &wrong_family,
            |_| {
                Ok(HirResolvedSourceRole::component(
                    HirSourceRequirement::Optional,
                    HirSourceOwnerStatus::Clean,
                ))
            },
        ),
        Err(HirSourceIndexLookupError::Query(
            HirSourceQueryError::StaleSourceRevision {
                expected: retained.identity().revision(),
                actual: stale_source.identity().revision(),
            }
        ))
    );
}

#[test]
fn style_item_duplicate_rows_are_idempotent_and_conflicts_poison_the_transaction() {
    let source = document("arcw:/source-index/style-conflicts", "style theme {}\n");
    let query = HirSourceQuery::Item {
        owner: item(module(1), 1),
        role: HirItemSourceRole::Style(HirStyleSourceRole::ItemId),
    };
    let site = span(&source, 6, 11);
    let mut accepted = staged_index(&source);
    accepted
        .require(&query, HirSourceRequirement::Required)
        .expect("initial Style requirement");
    accepted
        .require(&query, HirSourceRequirement::Required)
        .expect("idempotent Style requirement");
    accepted
        .stage(&query, site.clone())
        .expect("initial Style component");
    accepted
        .stage(&query, site)
        .expect("idempotent Style component");
    assert_eq!(
        accepted
            .commit()
            .expect("idempotent Style source index")
            .component_count(),
        1
    );

    let mut requirement_conflict = staged_index(&source);
    requirement_conflict
        .require(&query, HirSourceRequirement::Required)
        .expect("initial conflicting Style requirement");
    assert_eq!(
        requirement_conflict.require(&query, HirSourceRequirement::Optional),
        Err(HirSourceCommitInvariantError::ConflictingRequirement {
            query: query.clone(),
        })
    );
    assert!(matches!(
        requirement_conflict.commit(),
        Err(HirSourceCommitInvariantError::TransactionPoisoned)
    ));

    let mut component_conflict = staged_index(&source);
    component_conflict
        .require(&query, HirSourceRequirement::Required)
        .expect("conflicting Style component requirement");
    component_conflict
        .stage(&query, span(&source, 6, 11))
        .expect("first Style component site");
    assert_eq!(
        component_conflict.stage(&query, span(&source, 0, 5)),
        Err(HirSourceCommitInvariantError::ConflictingComponent { query })
    );
    assert!(matches!(
        component_conflict.commit(),
        Err(HirSourceCommitInvariantError::TransactionPoisoned)
    ));
}

#[test]
fn attached_style_manifest_uses_compact_retained_ordinals_and_existing_child_owners() {
    let (parsed, item_node) = parsed_style(
        "style-complete-manifest",
        concat!(
            "style theme {\n",
            "    token color.text: Color = white\n",
            "    Panel Button.primary:hover > .label:active {\n",
            "        background-color = color.text\n",
            "        append opacity = 1\n",
            "    }\n",
            "    when environment(text-scale >= 100%) {\n",
            "        token nested = white\n",
            "        Button { opacity = 1 }\n",
            "    }\n",
            "}\n",
        ),
    );
    let TypedItemNode::Style(style) = item_node else {
        panic!("Style item concrete family")
    };
    let attached = style.semantics().expect("attached Style semantics");
    let owner_module = module(1);
    let mut slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let owner = slots
        .reserve_source::<ItemId>(style.id(), HirSourceSite::Span(style.source_span()), false)
        .expect("Style item slot")
        .id();
    slots
        .bind_payload_poison(owner, false)
        .expect("Style item payload state");
    let mut staged = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    staged
        .stage_attached_style(&parsed, owner, &attached)
        .expect("complete Style manifest");
    let prepared = slots.prepare().expect("prepared Style item");
    let index = staged.commit().expect("committed Style manifest");

    let root = HirStyleBodyPath::root();
    let root_rule_part_absent = HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Style(HirStyleSourceRole::Body {
            path: root.clone(),
            part: HirStyleBodySourcePart::RulePart {
                rule: 0,
                sequence: 0,
            },
        }),
    };
    let root_rule_part_present = HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Style(HirStyleSourceRole::Body {
            path: root,
            part: HirStyleBodySourcePart::RulePart {
                rule: 0,
                sequence: 1,
            },
        }),
    };
    let nested_path = HirStyleBodyPath::from_ordinals(vec![1].into_boxed_slice());
    let nested_recovered_token_hole = HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Style(HirStyleSourceRole::Body {
            path: nested_path.clone(),
            part: HirStyleBodySourcePart::RuleSelector { rule: 0 },
        }),
    };
    let nested_rule_after_hole = HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Style(HirStyleSourceRole::Body {
            path: nested_path,
            part: HirStyleBodySourcePart::RuleSelector { rule: 1 },
        }),
    };
    let retained_token = HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Style(HirStyleSourceRole::Token {
            ordinal: 0,
            part: HirStyleTokenSourcePart::Whole,
        }),
    };

    assert!(index.validates_prepared(prepared.snapshot(), parsed.document().identity()));
    assert_eq!(
        index.requirement(&root_rule_part_absent),
        Some(HirSourceRequirement::Optional)
    );
    assert_eq!(
        index.requirement(&root_rule_part_present),
        Some(HirSourceRequirement::Required)
    );
    assert_eq!(index.requirement(&nested_recovered_token_hole), None);
    assert_eq!(
        index.requirement(&nested_rule_after_hole),
        Some(HirSourceRequirement::Required)
    );
    assert_eq!(
        index.requirement(&retained_token),
        Some(HirSourceRequirement::Required)
    );
    assert!(index.requirements.keys().all(|query| {
        !matches!(
            query,
            HirSourceQuery::Expr { .. } | HirSourceQuery::Type { .. }
        )
    }));
}

#[test]
fn attached_style_missing_body_retains_required_zero_width_body_owner() {
    let (parsed, item_node) = parsed_style("style-missing-body-manifest", "style theme\n");
    let TypedItemNode::Style(style) = item_node else {
        panic!("Style item concrete family")
    };
    let attached = style.semantics().expect("attached Style recovery");
    assert!(matches!(attached.body(), AttachedStyleBody::Missing(_)));
    let owner_module = module(1);
    let mut slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let owner = slots
        .reserve_source::<ItemId>(style.id(), HirSourceSite::Span(style.source_span()), true)
        .expect("recovered Style item slot")
        .id();
    slots
        .bind_payload_poison(owner, true)
        .expect("recovered Style item payload state");
    let mut staged = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    staged
        .stage_attached_style(&parsed, owner, &attached)
        .expect("missing body Style manifest");
    let prepared = slots.prepare().expect("prepared recovered Style item");
    let index = staged.commit().expect("committed recovered Style manifest");
    let body = HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Style(HirStyleSourceRole::Body {
            path: HirStyleBodyPath::root(),
            part: HirStyleBodySourcePart::BodyWhole,
        }),
    };

    assert!(index.validates_prepared(prepared.snapshot(), parsed.document().identity()));
    assert_eq!(
        index.requirement(&body),
        Some(HirSourceRequirement::Required)
    );
    assert!(matches!(
        index.components.get(&body),
        Some(HirSourceSite::Insertion(insertion))
            if insertion.offset() == "style theme\n".len()
    ));
    assert_eq!(
        index
            .requirements
            .keys()
            .filter(|query| query.owner() == SyntheticOwner::Item(owner))
            .count(),
        2
    );
}

#[test]
fn candidate_inventory_admits_only_retained_style_missing_expressions() {
    let (parsed, item_node) = parsed_style(
        "style-missing-expression-candidate-inventory",
        "style theme { token color.text = }\n",
    );
    let TypedItemNode::Style(style) = item_node else {
        panic!("Style item concrete family")
    };
    let attached = style.semantics().expect("attached Style recovery");
    let [AttachedStyleMember::Token(token)] = attached.body().members() else {
        panic!("one Style token")
    };
    let missing = token.value().missing().expect("missing Style token value");
    let owner_module = module(1);
    let mut staged_slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let site = HirSourceSite::from_attached_span(parsed.document(), &missing.source_span())
        .expect("missing Style expression insertion");
    let owner = staged_slots
        .reserve_source::<ExprId>(missing.id(), site, true)
        .expect("source-backed missing Style expression")
        .id();
    staged_slots
        .bind_payload_poison(owner, true)
        .expect("missing Style expression poison state");
    let prepared = staged_slots
        .prepare()
        .expect("prepared missing Style expression");

    assert!(
        super::expression_manifest::candidate_projection::candidate_type_expectations(
            &parsed,
            prepared.snapshot(),
            &BTreeSet::new(),
        )
        .is_none(),
        "an unretained source family must remain terminal"
    );
    let retained = BTreeSet::from([owner]);
    assert!(
        super::expression_manifest::candidate_projection::candidate_type_expectations(
            &parsed,
            prepared.snapshot(),
            &retained,
        )
        .is_some_and(|expectations| expectations.is_empty()),
        "the exact retained Style MissingExpression is not a candidate root"
    );
}

#[test]
fn entity_reference_expression_uses_typed_id_components_not_path_roles() {
    let source = document("arcw:/source-index/entity-reference", "^flow@child");
    let owner = expr(module(1), 1);
    let absolute = expression_query(
        owner,
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::AbsoluteMarker),
    );
    let family = expression_query(
        owner,
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::Family),
    );
    let suffix = expression_query(
        owner,
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 0 }),
    );
    let path = expression_query(owner, HirExprSourceRole::PathSegment { ordinal: 0 });

    assert_ne!(absolute, path);
    assert_ne!(family, path);
    assert_ne!(suffix, path);

    let mut staged = staged_index(&source);
    for (query, start, end) in [(&absolute, 0, 1), (&family, 1, 5), (&suffix, 6, 11)] {
        staged
            .require(query, HirSourceRequirement::Required)
            .expect("entity-reference component requirement");
        staged
            .stage(query, span(&source, start, end))
            .expect("entity-reference component");
    }
    let index = staged.commit().expect("entity-reference source index");

    for query in [&absolute, &family, &suffix] {
        let lookup = index
            .lookup(source.identity(), source.identity(), query, |_| {
                Ok(HirResolvedSourceRole::component(
                    HirSourceRequirement::Required,
                    HirSourceOwnerStatus::Clean,
                ))
            })
            .expect("typed entity-reference component lookup");
        assert!(matches!(
            lookup.presence(),
            HirSourcePresence::Present(HirSourceSite::Span(_))
        ));
    }
}

#[test]
fn recovered_entity_reference_shape_resolves_before_source_identity() {
    let source = document("arcw:/source-index/recovered-entity-reference", "^^@child");
    let foreign = document("arcw:/source-index/foreign", "^^@child");
    let owner = expr(module(1), 1);
    let payload = HirExprKind::EntityReference(HirIdRefValue::Recovered(HirIdRefRecovery::new(
        HirIdRefShape::FamilyRelative {
            parent_depth: 2,
            suffix_segment_count: 1,
        },
        HirIdRefIssue::Invalid(HirIdRefInvariantError::InvalidSuffix),
    )));
    let suffix = expression_query(
        owner,
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 0 }),
    );
    let mut staged = staged_index(&source);
    staged
        .require(&suffix, HirSourceRequirement::Required)
        .expect("recovered entity-reference requirement");
    staged
        .stage(&suffix, span(&source, 3, 8))
        .expect("recovered entity-reference component");
    let index = staged
        .commit()
        .expect("recovered entity-reference source index");

    let lookup = index
        .lookup(source.identity(), source.identity(), &suffix, |query| {
            let HirSourceQuery::Expr { owner, role } = query else {
                unreachable!("expression query")
            };
            payload.validate_source_role(*owner, *role)?;
            Ok(HirResolvedSourceRole::component(
                HirSourceRequirement::Required,
                HirSourceOwnerStatus::Poisoned,
            ))
        })
        .expect("recovered entity-reference lookup");
    assert!(matches!(
        lookup.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));
    assert_eq!(lookup.owner_status(), HirSourceOwnerStatus::Poisoned);

    let one_over = expression_query(
        owner,
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 1 }),
    );
    assert!(matches!(
        index.lookup(source.identity(), foreign.identity(), &one_over, |query| {
            let HirSourceQuery::Expr { owner, role } = query else {
                unreachable!("expression query")
            };
            payload.validate_source_role(*owner, *role)?;
            Ok(HirResolvedSourceRole::component(
                HirSourceRequirement::Required,
                HirSourceOwnerStatus::Poisoned,
            ))
        }),
        Err(HirSourceIndexLookupError::Query(
            HirSourceQueryError::ExprOrdinalOutOfBounds { length: 1, .. }
        ))
    ));
}

#[test]
fn attached_type_projector_owns_required_optional_and_exact_syntax_rows() {
    let (parsed, attached) = parsed_type("generic", "Vec<T>");
    let mut slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    let reservation = slots
        .reserve_source::<TypeId>(
            attached.id(),
            HirSourceSite::Span(attached.whole_source_span()),
            false,
        )
        .expect("source-backed type slot");
    let owner = reservation.id();
    slots
        .bind_payload_poison(owner, false)
        .expect("test type poison binding");
    let mut staged = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    staged
        .stage_attached_type(&parsed, owner, &attached)
        .expect("direct attached type projection");
    let prepared = slots.prepare().expect("prepared type slots");
    let index = staged.commit().expect("attached type source index");

    assert!(index.validates_prepared(prepared.snapshot(), parsed.document().identity(),));

    let base = HirSourceQuery::Type {
        owner,
        role: HirTypeSourceRole::GenericBase,
    };
    let trailing = HirSourceQuery::Type {
        owner,
        role: HirTypeSourceRole::GenericSeparator { ordinal: 0 },
    };
    let base_lookup = index
        .lookup(
            parsed.document().identity(),
            parsed.document().identity(),
            &base,
            |_| {
                Ok(HirResolvedSourceRole::component(
                    HirSourceRequirement::Required,
                    HirSourceOwnerStatus::Clean,
                ))
            },
        )
        .expect("required generic base");
    let trailing_lookup = index
        .lookup(
            parsed.document().identity(),
            parsed.document().identity(),
            &trailing,
            |_| {
                Ok(HirResolvedSourceRole::component(
                    HirSourceRequirement::Optional,
                    HirSourceOwnerStatus::Clean,
                ))
            },
        )
        .expect("optional trailing separator");

    assert!(matches!(
        base_lookup.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));
    assert_eq!(
        trailing_lookup.presence(),
        HirSourcePresence::AbsentOptional
    );
}

#[test]
fn attached_pattern_projector_owns_required_name_and_exact_syntax_rows() {
    let kind = binding_pattern_kind(module(1), "binding");
    let (parsed, slots, patterns, index, owner) =
        frozen_binding_pattern("pattern-binding", &kind, kind.clone());
    let query = HirSourceQuery::Pattern {
        owner,
        role: HirPatternSourceRole::Name,
    };

    assert!(index.validates_prepared(&slots, parsed.document().identity(),));
    assert!(index.validates_attached_patterns(&parsed, &slots, &patterns));
    assert_eq!(
        index.requirement(&query),
        Some(HirSourceRequirement::Required)
    );
    assert!(matches!(
        index.components.get(&query),
        Some(HirSourceSite::Span(_))
    ));
    assert_eq!(
        index
            .syntax_owners
            .get(&SyntheticOwner::Pattern(owner))
            .copied(),
        slots
            .resolve_prepared(owner)
            .ok()
            .and_then(|metadata| match metadata.origin() {
                crate::slot::HirOrigin::Source(source) => Some(source.syntax()),
                crate::slot::HirOrigin::Synthetic(_) => None,
            })
    );
}

#[test]
fn missing_variant_name_is_a_required_zero_width_source_component() {
    let (parsed, attached) = parsed_pattern("variant-missing-name", "Choice.");
    let owner_module = module(1);
    let owner = pattern(owner_module, 1);
    let pattern_scope = scope(owner_module, 1);
    let variant = HirVariantPattern::try_new(
        HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(hir_type_path(
            "Choice",
        ))),
        HirVariantPatternName::Recovered(HirVariantPatternNameIssue::Missing),
        HirVariantPatternPayload::Absent,
        pattern_scope,
        &RootPatternResolver {
            scope: pattern_scope,
        },
    )
    .expect("known Variant family with missing required name");
    let slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let mut staged = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    staged
        .stage_attached_pattern(&parsed, owner, &attached, &HirPatternKind::Variant(variant))
        .expect("missing-name Variant source projection");
    let index = staged.commit().expect("missing-name Variant manifest");
    let query = HirSourceQuery::Pattern {
        owner,
        role: HirPatternSourceRole::VariantName,
    };

    assert_eq!(
        index.requirement(&query),
        Some(HirSourceRequirement::Required)
    );
    assert!(matches!(
        index.components.get(&query),
        Some(HirSourceSite::Insertion(_))
    ));
}

#[test]
fn record_path_manifest_distinguishes_valid_absence_from_recovered_authored_path() {
    let owner = pattern(module(1), 1);

    let (absent_parsed, absent_attached) = parsed_pattern("record-path-absent", "{}");
    let absent_slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    let mut absent_index =
        StagedHirSourceIndex::new(absent_parsed.document().identity().clone(), &absent_slots);
    absent_index
        .stage_attached_pattern(
            &absent_parsed,
            owner,
            &absent_attached,
            &HirPatternKind::Record {
                path: HirPatternRecordPath::Absent,
                fields: Box::new([]),
            },
        )
        .expect("pathless record Pattern projection");
    let absent_index = absent_index
        .commit()
        .expect("pathless record Pattern source manifest");

    let (recovered_parsed, recovered_attached) = parsed_pattern("record-path-recovered", "crate{}");
    let recovered_slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    let mut recovered_index = StagedHirSourceIndex::new(
        recovered_parsed.document().identity().clone(),
        &recovered_slots,
    );
    recovered_index
        .stage_attached_pattern(
            &recovered_parsed,
            owner,
            &recovered_attached,
            &HirPatternKind::Record {
                path: HirPatternRecordPath::Recovered(HirPatternRecordPathIssue::new(
                    HirPathIssue::InvalidSegment { ordinal: 0 },
                    1,
                )),
                fields: Box::new([]),
            },
        )
        .expect("recovered record-path projection");
    let recovered_index = recovered_index
        .commit()
        .expect("recovered record-path source manifest");

    let root = HirSourceQuery::Pattern {
        owner,
        role: HirPatternSourceRole::RecordPathRoot,
    };
    assert_eq!(
        absent_index.requirement(&root),
        Some(HirSourceRequirement::Optional)
    );
    assert!(!absent_index.components.contains_key(&root));
    assert_eq!(
        recovered_index.requirement(&root),
        Some(HirSourceRequirement::Optional)
    );
    assert!(!recovered_index.components.contains_key(&root));

    let recovered_segment = HirSourceQuery::Pattern {
        owner,
        role: HirPatternSourceRole::RecordPathSegment { ordinal: 0 },
    };
    assert_eq!(absent_index.requirement(&recovered_segment), None);
    assert_eq!(
        recovered_index.requirement(&recovered_segment),
        Some(HirSourceRequirement::Required)
    );
    assert!(matches!(
        recovered_index.components.get(&recovered_segment),
        Some(HirSourceSite::Span(_))
    ));
}

#[test]
fn record_cross_field_recovery_keeps_authored_source_on_the_invalid_later_field() {
    let owner_module = module(1);
    let owner = pattern(owner_module, 1);
    let first_local = local(owner_module, 2);
    let cases = [
        (
            "record-duplicate-name",
            "{field, field}",
            vec![
                HirPatternField::Shorthand {
                    name: HirName::try_new("field".into()).expect("record field name"),
                    local: first_local,
                },
                HirPatternField::Invalid {
                    issue: HirPatternFieldIssue::DuplicateName,
                },
            ]
            .into_boxed_slice(),
            HirPatternFieldSourcePart::Name,
        ),
        (
            "record-multiple-rest",
            "{..first, ..second}",
            vec![
                HirPatternField::Rest {
                    binding: Some(first_local),
                },
                HirPatternField::Invalid {
                    issue: HirPatternFieldIssue::MultipleRest,
                },
            ]
            .into_boxed_slice(),
            HirPatternFieldSourcePart::RestBinding,
        ),
    ];

    for (document_id, source, fields, later_part) in cases {
        let (parsed, attached) = parsed_pattern(document_id, source);
        let slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
        let mut staged = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
        staged
            .stage_attached_pattern(
                &parsed,
                owner,
                &attached,
                &HirPatternKind::Record {
                    path: HirPatternRecordPath::Absent,
                    fields,
                },
            )
            .expect("record cross-field recovery source projection");
        let index = staged
            .commit()
            .expect("record cross-field recovery source manifest");
        let first_whole = HirSourceQuery::Pattern {
            owner,
            role: HirPatternSourceRole::PatternField {
                field: 0,
                part: HirPatternFieldSourcePart::Whole,
            },
        };
        let later_component = HirSourceQuery::Pattern {
            owner,
            role: HirPatternSourceRole::PatternField {
                field: 1,
                part: later_part,
            },
        };

        assert_eq!(
            index.requirement(&first_whole),
            Some(HirSourceRequirement::Required)
        );
        assert_eq!(
            index.requirement(&later_component),
            Some(HirSourceRequirement::Optional)
        );
        assert!(matches!(
            index.components.get(&later_component),
            Some(HirSourceSite::Span(_))
        ));
    }
}

#[test]
fn sequence_rest_manifest_distinguishes_all_semantic_presence_states() {
    let owner_module = module(1);
    let owner = pattern(owner_module, 1);
    let rest_local = local(owner_module, 2);
    let recovered_issue = HirPatternSequenceRestIssue::InvalidBinding(
        HirPatternBindingIssue::UnexpectedTrailingInput { token_count: 1 },
    );
    let cases = [
        (
            "sequence-rest-absent",
            "[]",
            HirPatternSequenceRest::Absent,
            HirSourceRequirement::Optional,
            HirSourceRequirement::Optional,
        ),
        (
            "sequence-rest-unbound",
            "[..]",
            HirPatternSequenceRest::Unbound,
            HirSourceRequirement::Required,
            HirSourceRequirement::Optional,
        ),
        (
            "sequence-rest-bound",
            "[..tail]",
            HirPatternSequenceRest::Bound(rest_local),
            HirSourceRequirement::Required,
            HirSourceRequirement::Required,
        ),
        (
            "sequence-rest-recovered",
            "[..tail]",
            HirPatternSequenceRest::Recovered(recovered_issue),
            HirSourceRequirement::Required,
            HirSourceRequirement::Required,
        ),
    ];

    for (document_id, source, rest, authored_rest, authored_binding) in cases {
        let (parsed, attached) = parsed_pattern(document_id, source);
        let slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
        let mut staged = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
        staged
            .stage_attached_pattern(
                &parsed,
                owner,
                &attached,
                &HirPatternKind::BracketSequence {
                    elements: Box::new([]),
                    rest,
                },
            )
            .expect("sequence-rest source projection");
        let index = staged.commit().expect("sequence-rest source manifest");

        for part in [
            HirPatternRestSourcePart::Whole,
            HirPatternRestSourcePart::Marker,
        ] {
            assert_eq!(
                index.requirement(&HirSourceQuery::Pattern {
                    owner,
                    role: HirPatternSourceRole::SequenceRest(part),
                }),
                Some(authored_rest)
            );
        }
        assert_eq!(
            index.requirement(&HirSourceQuery::Pattern {
                owner,
                role: HirPatternSourceRole::SequenceRest(HirPatternRestSourcePart::Binding,),
            }),
            Some(authored_binding)
        );
    }
}

#[test]
fn typed_binding_freeze_requires_the_exact_attached_type_child() {
    let (parsed, slots, patterns, index) =
        frozen_typed_binding_pattern("pattern-typed-child", true);
    assert!(index.validates_prepared(&slots, parsed.document().identity(),));
    assert!(index.validates_attached_patterns(&parsed, &slots, &patterns));

    let (parsed, slots, patterns, index) =
        frozen_typed_binding_pattern("pattern-typed-child-substitution", false);
    assert!(index.validates_prepared(&slots, parsed.document().identity(),));
    assert!(!index.validates_attached_patterns(&parsed, &slots, &patterns));
}

#[test]
fn pattern_freeze_rejects_a_semantic_family_substituted_under_valid_source_rows() {
    let (parsed, slots, patterns, index, _) = frozen_binding_pattern(
        "pattern-payload-mismatch",
        &binding_pattern_kind(module(1), "binding"),
        HirPatternKind::Discard,
    );

    assert!(index.validates_prepared(&slots, parsed.document().identity(),));
    assert!(!index.validates_attached_patterns(&parsed, &slots, &patterns));
}

#[test]
fn attached_pattern_projector_rejects_foreign_snapshot_and_family_atomically() {
    let (parsed, attached) = parsed_pattern("pattern-current", "binding");
    let (_, foreign) = parsed_pattern("pattern-foreign", "binding");
    let owner = pattern(module(1), 1);
    let slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    let mut stale = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);

    assert!(matches!(
        stale.stage_attached_pattern(
            &parsed,
            owner,
            &foreign,
            &binding_pattern_kind(module(1), "binding")
        ),
        Err(HirSourceCommitInvariantError::WrongSyntaxSnapshot { .. })
    ));
    assert!(matches!(
        stale.commit(),
        Err(HirSourceCommitInvariantError::TransactionPoisoned)
    ));

    let slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    let mut mismatch = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    assert_eq!(
        mismatch.stage_attached_pattern(&parsed, owner, &attached, &HirPatternKind::Discard,),
        Err(
            HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                owner: SyntheticOwner::Pattern(owner),
            }
        )
    );
    assert!(matches!(
        mismatch.commit(),
        Err(HirSourceCommitInvariantError::TransactionPoisoned)
    ));
}

#[test]
fn type_freeze_rederives_the_exact_attached_payload_and_manifest() {
    let (parsed, slots, types, index, owner) = frozen_root_path_type("freeze-exact", "Vec", "Vec");
    let items = ArenaSnapshot::empty(&slots);

    assert!(index.validates_prepared(&slots, parsed.document().identity(),));
    assert!(index.validates_attached_types(&parsed, &slots, &items, &types));
    let retained_syntax =
        slots
            .resolve_prepared(owner)
            .ok()
            .and_then(|metadata| match metadata.origin() {
                crate::slot::HirOrigin::Source(source) => Some(source.syntax()),
                crate::slot::HirOrigin::Synthetic(_) => None,
            });
    assert_eq!(
        index
            .syntax_owners
            .get(&SyntheticOwner::Type(owner))
            .copied(),
        retained_syntax
    );
}

#[test]
fn type_freeze_rejects_a_semantic_payload_substituted_under_valid_source_rows() {
    let (parsed, slots, types, index, _) =
        frozen_root_path_type("freeze-payload-mismatch", "Vec", "String");
    let items = ArenaSnapshot::empty(&slots);

    assert!(index.validates_prepared(&slots, parsed.document().identity(),));
    assert!(!index.validates_attached_types(&parsed, &slots, &items, &types));
}

#[test]
fn candidate_type_freeze_rejects_a_missing_preorder_type_id() {
    let (parsed, attached) = parsed_expression(
        "candidate-type-missing",
        "items[Vec<I32>::with_capacity(8)]",
    );

    let mut staged_slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    let outer = staged_slots
        .reserve_source::<ExprId>(
            attached.id(),
            HirSourceSite::Span(attached.whole_source_span()),
            false,
        )
        .expect("source-backed E34 owner")
        .id();
    staged_slots
        .bind_payload_poison(outer, false)
        .expect("source-backed E34 payload state");
    let prepared = staged_slots.prepare().expect("missing-type slot proposal");
    let slots = prepared.snapshot();
    let items = ArenaSnapshot::empty(slots);
    let types = ArenaSnapshot::empty(slots);
    let index = HirSourceIndex::empty(parsed.document().identity().clone(), slots);

    assert!(index.validates_prepared(slots, parsed.document().identity()));
    assert!(!index.validates_attached_types(&parsed, slots, &items, &types));
}

#[test]
fn candidate_type_freeze_rejects_an_excess_type_id_without_a_candidate_node() {
    let (parsed, attached) = parsed_expression("candidate-type-excess", "value");
    assert!(attached.ambiguous_index_candidate().is_none());

    let owner_module = module(1);
    let mut staged_slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let outer = staged_slots
        .reserve_source::<ExprId>(
            attached.id(),
            HirSourceSite::Span(attached.whole_source_span()),
            false,
        )
        .expect("source-backed expression owner")
        .id();
    staged_slots
        .bind_payload_poison(outer, false)
        .expect("source-backed expression payload state");
    let key = SyntheticKey::try_new(
        SyntheticOwner::Expr(outer),
        SyntheticRole::PostfixIndexCandidateExpression,
        0,
    )
    .expect("candidate type key");
    let mut staged_types = StagedArena::<HirType, TypeId>::new();
    let reservation = staged_types
        .reserve_synthetic(
            &mut staged_slots,
            key,
            HirSourceSite::Span(attached.whole_source_span()),
        )
        .expect("excess candidate type reservation");
    let owner = reservation.id();
    let type_scope = scope(owner_module, 1);
    staged_types
        .finalize(
            &mut staged_slots,
            reservation,
            HirType::try_new(
                owner,
                HirTypeKind::Never,
                type_scope,
                HirPoisonState::Clean,
                &RootTypeResolver { scope: type_scope },
            )
            .expect("excess candidate type payload"),
        )
        .expect("excess candidate type finalization");
    let types = staged_types
        .into_snapshot(&mut staged_slots)
        .expect("excess candidate type arena");
    let prepared = staged_slots.prepare().expect("excess-type slot proposal");
    let slots = prepared.snapshot();
    let items = ArenaSnapshot::empty(slots);
    let index = HirSourceIndex::empty(parsed.document().identity().clone(), slots);

    assert!(index.validates_prepared(slots, parsed.document().identity()));
    assert!(!index.validates_attached_types(&parsed, slots, &items, &types));
}

#[test]
fn candidate_expression_freeze_rejects_a_missing_expected_preorder_id() {
    let staged_slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL)
        .prepare()
        .expect("missing candidate expression slot proposal");
    let expected = BTreeSet::from([expr(module(1), 42)]);

    assert!(
        !super::expression_manifest::candidate_projection::candidate_expression_slots_match(
            staged_slots.snapshot(),
            &expected,
        )
    );
}

#[test]
fn candidate_expression_freeze_rejects_an_excess_preorder_id() {
    let (_parsed, attached) = parsed_expression("candidate-expression-excess", "value");
    let mut staged_slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    let outer = staged_slots
        .reserve_source::<ExprId>(
            attached.id(),
            HirSourceSite::Span(attached.whole_source_span()),
            false,
        )
        .expect("source-backed expression owner")
        .id();
    staged_slots
        .bind_payload_poison(outer, false)
        .expect("source-backed expression payload state");
    let candidate = staged_slots
        .reserve_synthetic::<ExprId>(
            SyntheticKey::try_new(
                SyntheticOwner::Expr(outer),
                SyntheticRole::PostfixIndexCandidateExpression,
                0,
            )
            .expect("candidate expression key"),
            HirSourceSite::Span(attached.whole_source_span()),
            false,
        )
        .expect("candidate expression reservation")
        .id();
    staged_slots
        .bind_payload_poison(candidate, false)
        .expect("candidate expression payload state");
    let prepared = staged_slots
        .prepare()
        .expect("excess candidate expression slot proposal");

    assert!(
        !super::expression_manifest::candidate_projection::candidate_expression_slots_match(
            prepared.snapshot(),
            &BTreeSet::new(),
        )
    );
}

#[test]
fn attached_type_projector_rejects_a_foreign_syntax_snapshot_before_staging() {
    let (parsed, _) = parsed_type("snapshot-current", "Vec<T>");
    let (_, foreign) = parsed_type("snapshot-foreign", "Vec<T>");
    let owner = ty(module(1), 1);
    let slots = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    let mut staged = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);

    assert!(matches!(
        staged.stage_attached_type(&parsed, owner, &foreign),
        Err(HirSourceCommitInvariantError::WrongSyntaxSnapshot { .. })
    ));
    assert!(matches!(
        staged.commit(),
        Err(HirSourceCommitInvariantError::TransactionPoisoned)
    ));
}

#[test]
fn unsafe_audit_projector_owns_the_exact_inner_block_insertion() {
    let (parsed, slots, statements, index, owner) = frozen_unsafe_statement("unsafe-exact");
    let metadata = slots
        .resolve_prepared(owner)
        .expect("prepared unsafe statement slot");
    let crate::slot::HirOrigin::Source(source) = metadata.origin() else {
        panic!("unsafe statement must remain source-backed");
    };
    let attached = parsed
        .statement_node(source.syntax())
        .expect("exact attached unsafe statement");
    let audit = attached
        .cast::<arcweft_lang_syntax::attachment::node::UnsafeLifetimeStatementKind>()
        .expect("unsafe statement concrete family");
    let expected_offset = audit
        .audit_insertion_anchor()
        .expect("authored unsafe opening brace")
        .range()
        .end();
    let query = HirSourceQuery::Stmt {
        owner,
        role: HirStmtSourceRole::UnsafeAuditInsertion,
    };

    assert!(index.validates_prepared(&slots, parsed.document().identity(),));
    assert!(index.validates_attached_statements(&parsed, &slots, &statements));
    assert_eq!(
        index.requirement(&query),
        Some(HirSourceRequirement::Required)
    );
    assert!(matches!(
        index.components.get(&query),
        Some(HirSourceSite::Insertion(insertion)) if insertion.offset() == expected_offset
    ));
}

#[test]
fn unsafe_audit_projector_retains_recovery_without_fabricating_an_insertion() {
    let (unclosed, unclosed_stmt) = parsed_statement_source(
        "unsafe-unclosed",
        "fn audit() {\n    unsafe lifetime @unsafe.audit { value;\n",
    );
    let owner_module = module(1);
    let owner = stmt(owner_module, 1);
    let kind = unsafe_stmt_kind(scope(owner_module, 2));
    let statement = HirStmt::try_new_with_state(
        scope(owner_module, 3),
        kind,
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::UnclosedBody),
    )
    .expect("typed unclosed unsafe statement");
    let slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let mut incomplete = StagedHirSourceIndex::new(unclosed.document().identity().clone(), &slots);

    incomplete
        .stage_attached_stmt(&unclosed, owner, &unclosed_stmt, &statement)
        .expect("unclosed unsafe statement retains an optional absent insertion");
    let incomplete = incomplete.commit().expect("typed recovery source manifest");
    let query = HirSourceQuery::Stmt {
        owner,
        role: HirStmtSourceRole::UnsafeAuditInsertion,
    };
    assert_eq!(
        incomplete.requirement(&query),
        Some(HirSourceRequirement::Optional)
    );
    assert!(!incomplete.components.contains_key(&query));

    let (ordinary, ordinary_stmt) = parsed_statement("unsafe-family-mismatch", "continue;");
    let ordinary_slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let mut mismatch =
        StagedHirSourceIndex::new(ordinary.document().identity().clone(), &ordinary_slots);
    let clean = HirStmt::try_new(
        scope(owner_module, 3),
        unsafe_stmt_kind(scope(owner_module, 2)),
    )
    .expect("clean unsafe statement");
    assert_eq!(
        mismatch.stage_attached_stmt(&ordinary, owner, &ordinary_stmt, &clean),
        Err(
            HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                owner: SyntheticOwner::Stmt(owner),
            }
        )
    );
    assert!(matches!(
        mismatch.commit(),
        Err(HirSourceCommitInvariantError::TransactionPoisoned)
    ));
}

#[test]
fn recovered_unsafe_statement_publishes_no_fabricated_edit_row() {
    let (parsed, attached) = parsed_statement(
        "unsafe-missing-body",
        "unsafe lifetime @unsafe.audit value;",
    );
    let owner_module = module(1);
    let owner = stmt(owner_module, 1);
    let slots = StagedSlotTransaction::new(owner_module, HirRevision::INITIAL);
    let mut staged = StagedHirSourceIndex::new(parsed.document().identity().clone(), &slots);
    let statement = HirStmt::try_new_with_state(
        scope(owner_module, 2),
        HirStmtKind::UnsafeLifetime {
            audit: HirUnsafeAudit::new(
                HirIdRefValue::Resolved(HirIdRef::absolute(
                    HirEntityReference::try_new("unsafe.audit".into())
                        .expect("test unsafe audit ID"),
                )),
                None,
                false,
            ),
            body: HirUnsafeLifetimeBody::Missing,
        },
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::MissingBody),
    )
    .expect("typed missing unsafe body");

    staged
        .stage_attached_stmt(&parsed, owner, &attached, &statement)
        .expect("poisoned statement carries an optional absent component");
    let index = staged.commit().expect("typed recovery source manifest");

    let semantic_owner = SyntheticOwner::Stmt(owner);
    assert_eq!(
        index.syntax_owners.get(&semantic_owner),
        Some(&attached.id())
    );
    let query = HirSourceQuery::Stmt {
        owner,
        role: HirStmtSourceRole::UnsafeAuditInsertion,
    };
    assert_eq!(
        index.requirement(&query),
        Some(HirSourceRequirement::Optional)
    );
    assert_eq!(index.component_count(), 0);
}

#[test]
fn statement_role_applicability_is_resolved_from_the_semantic_payload_first() {
    let owner_module = module(1);
    let owner = stmt(owner_module, 1);
    let unsafe_kind = unsafe_stmt_kind(scope(owner_module, 2));

    assert_eq!(
        unsafe_kind.validate_source_role(owner, HirStmtSourceRole::Whole),
        Ok(())
    );
    assert_eq!(
        unsafe_kind.validate_source_role(owner, HirStmtSourceRole::UnsafeAuditInsertion),
        Ok(())
    );
    assert_eq!(
        HirStmtKind::Error.validate_source_role(owner, HirStmtSourceRole::UnsafeAuditInsertion),
        Err(HirSourceQueryError::StmtRoleNotApplicable {
            owner,
            role: HirStmtSourceRole::UnsafeAuditInsertion,
        })
    );
}
