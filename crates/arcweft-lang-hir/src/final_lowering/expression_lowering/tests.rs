use core::num::NonZeroU32;
use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::attachment::node::{
    FunctionBodyKind, LetAwaitStatementKind, LetChoiceStatementKind, LetStatementKind,
};
use arcweft_lang_syntax::attachment::{
    AttachedExpressionNode, DeclarationBodyNode, LetInitializerNode, StatementNode,
};
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};

use super::*;
use crate::database::HirDatabase;
use crate::diagnostic::{HirDiagnostic, HirRecoveryPrimary};
use crate::dialogue_application::HirPostfixBracketCandidates;
use crate::expr::HirBlockExpr;
use crate::identity::{
    HirIdKind, HirLimit, HirTypedId, LocalGeneration, LocalId, RawHirId, SyntheticKey,
    SyntheticOwner, SyntheticRole, TypeId,
};
use crate::leaf::{
    HirDurationLiteral, HirIdRefIssue, HirIntegerIssue, HirIntegerLiteral, HirIntegerRadix,
    HirIntegerSuffix, HirLifetimeRegistryScope, HirLiteral, HirNumericSequenceRecovery,
    HirPathSegment,
};
use crate::lowering::{HirLowerFailure, HirModuleKey, LoweringRequest};
use crate::module::{HirModule, HirModuleStatus};
use crate::scope::{HirLocal, HirLocalKind, HirScope, HirScopeKind, HirScopeOwner};
use crate::slot::HirOrigin;
use crate::source_index::{
    HirIdRefSourcePart, HirRecordFieldSourcePart, HirSourceCommitInvariantError,
    HirSourceOwnerStatus, HirSourcePresence, HirSourceQueryError,
};
use crate::stmt::HirStmtKind;
use crate::symbol::CallablePackageId;

#[path = "tests/call.rs"]
mod call;
#[path = "tests/choice.rs"]
mod choice;
#[path = "tests/control.rs"]
mod control;
#[path = "tests/dialogue.rs"]
mod dialogue;
#[path = "tests/dialogue_candidate_block.rs"]
mod dialogue_candidate_block;
#[path = "tests/dialogue_candidate_control.rs"]
mod dialogue_candidate_control;
#[path = "tests/identity.rs"]
mod identity;
#[path = "tests/select.rs"]
mod select;
#[path = "tests/select_limits.rs"]
mod select_limits;
#[path = "tests/statements.rs"]
mod statements;

fn parsed_source(document_id: &str, expressions: &[String]) -> ParsedSource {
    let name = SourceName::path(format!("proof/expression-lowering/{document_id}.arcw"));
    let statements = expressions
        .iter()
        .enumerate()
        .map(|(ordinal, expression)| format!("    let value_{ordinal} = {expression};"))
        .collect::<Vec<_>>()
        .join("\n");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/expression-lowering/{document_id}.arcw"
            ))
            .expect("expression-lowering document ID"),
            name.clone(),
            format!("fn lower_expressions() {{\n{statements}\n}}\n"),
        )
        .expect("expression-lowering source"),
    );
    SyntaxDatabase::try_new()
        .expect("expression-lowering syntax database")
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("attached expression source parses")
}

struct DisagreeingDisplayTypedExpressionBuilder {
    document_id: &'static str,
    display_source: &'static str,
    typed_expression: &'static str,
}

impl DisagreeingDisplayTypedExpressionBuilder {
    fn build(self) -> ParsedSource {
        let name = SourceName::path(self.display_source);
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(self.document_id)
                    .expect("disagreeing-display document ID"),
                name.clone(),
                format!(
                    "fn lower_typed_expression() {{\n    let value = {};\n}}\n",
                    self.typed_expression
                ),
            )
            .expect("disagreeing-display source"),
        );
        SyntaxDatabase::try_new()
            .expect("disagreeing-display syntax database")
            .parse_initial(
                SourceSnapshotId::initial(name),
                document,
                arcweft_lang_syntax::parser::ParseOptions::default(),
            )
            .expect("disagreeing-display attached source parses")
    }
}

fn repeated_numeric_sequence(element: &str, count: usize) -> String {
    let mut source = String::with_capacity(
        element
            .len()
            .checked_mul(count)
            .and_then(|bytes| bytes.checked_add(count.saturating_sub(1)))
            .and_then(|bytes| bytes.checked_add(2))
            .expect("numeric-sequence fixture size fits usize"),
    );
    source.push('[');
    for ordinal in 0..count {
        if ordinal != 0 {
            source.push(',');
        }
        source.push_str(element);
    }
    source.push(']');
    source
}

fn parsed_revisions(document_id: &str, expression: &str) -> (ParsedSource, ParsedSource) {
    let name = SourceName::path(format!("proof/expression-lowering/{document_id}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/expression-lowering/{document_id}.arcw"
            ))
            .expect("expression relowering document ID"),
            name.clone(),
            format!("fn lower_expressions() {{\n    let value = {expression};\n}}\n"),
        )
        .expect("expression relowering source"),
    );
    let mut syntax = SyntaxDatabase::try_new().expect("expression relowering syntax database");
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("initial attached expression source");
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(0, 0))
                    .expect("expression revision insertion"),
                "// retained expression revision\n",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("revised attached expression source");
    (initial, revised)
}

fn statements(parsed: &ParsedSource) -> Vec<StatementNode> {
    let item = parsed
        .items()
        .expect("source item inventory")
        .into_iter()
        .next()
        .expect("function item");
    let Some(DeclarationBodyNode::Body(body)) = item.body().expect("function body access") else {
        panic!("test function must retain an authored body");
    };
    body.cast::<FunctionBodyKind>()
        .expect("function body family")
        .block()
        .expect("function computation block")
        .statements()
        .expect("function statement inventory")
}

fn attached_expressions(parsed: &ParsedSource) -> Vec<AttachedExpressionNode> {
    statements(parsed)
        .into_iter()
        .map(|statement| {
            let initializer = if let Ok(binding) = statement.cast::<LetStatementKind>() {
                match binding
                    .initializer()
                    .expect("initializer access")
                    .expect("let initializer")
                {
                    LetInitializerNode::Expression(initializer) => initializer,
                    LetInitializerNode::Missing(_) => panic!("test initializer is authored"),
                }
            } else if let Ok(binding) = statement.cast::<LetChoiceStatementKind>() {
                return binding
                    .semantics()
                    .expect("let-choice semantics")
                    .expression()
                    .expression_node()
                    .expect("let-choice attached expression");
            } else {
                statement
                    .cast::<LetAwaitStatementKind>()
                    .expect("let-await statement family")
                    .initializer()
                    .expect("let-await initializer access")
            };
            initializer
                .semantic()
                .expect("attached semantic expression")
        })
        .collect()
}

fn module_key(parsed: &ParsedSource) -> HirModuleKey {
    HirModuleKey::new(
        CallablePackageId::try_new("proof-expression-lowering-tests").expect("package ID"),
        CanonicalModulePath::crate_root(),
        parsed.document().identity().clone(),
    )
}

fn stage<'source>(
    database: &HirDatabase,
    parsed: &'source ParsedSource,
) -> StagedHirModuleTransaction<'source> {
    super::super::stage_unpublished_module_for_invariant_test(
        database,
        LoweringRequest::try_new(module_key(parsed), parsed).expect("lowering request"),
        crate::lowering::HirLoweringControl::new(),
    )
    .expect("staged HIR module")
}

fn allocate_module_scope(
    transaction: &mut StagedHirModuleTransaction<'_>,
    parsed: &ParsedSource,
) -> ScopeId {
    let module = transaction.snapshot_id().module();
    let root = parsed.root_syntax();
    let site = HirSourceSite::Span(root.source_span().clone());
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .scopes()
        .allocate_source(
            slots,
            root.id(),
            site,
            HirScope::try_new(
                module,
                HirScopeKind::Module,
                None,
                HirScopeOwner::Module(module),
                Box::new([]),
                Box::new([]),
            )
            .expect("module scope"),
        )
        .expect("module scope allocation")
}

fn lower_and_publish(
    parsed: &ParsedSource,
) -> (Arc<HirModule>, Vec<ExprId>, Vec<AttachedExpressionNode>) {
    let attached = attached_expressions(parsed);
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, parsed);
    let scope = allocate_module_scope(&mut transaction, parsed);
    let owners = attached
        .iter()
        .enumerate()
        .map(|(ordinal, expression)| {
            transaction
                .lower_attached_expression(expression, scope)
                .unwrap_or_else(|error| {
                    panic!("attached expression {ordinal} lowering failed: {error:?}")
                })
        })
        .collect::<Vec<_>>();
    let module = transaction
        .finish(&mut database)
        .expect("expression module publication")
        .into_module();
    (module, owners, attached)
}

fn expression(module: &HirModule, owner: ExprId) -> &HirExpr {
    module
        .arenas()
        .expressions()
        .resolve(module.slots(), owner)
        .expect("published expression")
}

#[test]
fn fx_constants_are_classified_from_accepted_hir() {
    let sources = ["500ms", "-2px", "\"seed\"", ".glyph", "[1, 2]", "\"x\"c"].map(str::to_owned);
    let parsed = parsed_source("fx-constants", &sources);
    let (module, owners, _) = lower_and_publish(&parsed);
    let expected = [
        Some(crate::fx::FxConstKind::Literal),
        Some(crate::fx::FxConstKind::SignedNumber),
        Some(crate::fx::FxConstKind::Literal),
        Some(crate::fx::FxConstKind::Selector),
        Some(crate::fx::FxConstKind::List),
        None,
    ];
    for ((source, owner), expected) in sources.iter().zip(owners).zip(expected) {
        let constant = crate::fx::FxConst::from_expr(&module, owner);
        assert_eq!(constant.map(crate::fx::FxConst::kind), expected, "{source}");
        if let Some(constant) = constant {
            assert_eq!(constant.expr(), owner);
        }
    }
}

#[derive(Clone, Copy)]
enum LocalPayloadTamper {
    Name(&'static str),
    Generation(LocalGeneration),
    Annotation(Option<TypeId>),
    Mutable(bool),
}

fn tamper_local_payload(
    transaction: &mut StagedHirModuleTransaction<'_>,
    local: LocalId,
    tamper: LocalPayloadTamper,
) {
    let payload = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .locals()
            .resolve_staged(slots, local)
            .expect("staged Local payload")
            .clone()
    };
    let mut name = payload.name().clone();
    let mut generation = payload.generation();
    let mut annotation = payload.annotation();
    let mut mutable = payload.is_mutable_binding();
    match tamper {
        LocalPayloadTamper::Name(replacement) => {
            name = crate::leaf::HirName::try_new(replacement.into()).expect("tampered Local name");
        }
        LocalPayloadTamper::Generation(replacement) => generation = replacement,
        LocalPayloadTamper::Annotation(replacement) => annotation = replacement,
        LocalPayloadTamper::Mutable(replacement) => mutable = replacement,
    }
    let replacement = HirLocal::try_new(
        payload.scope(),
        payload.kind(),
        name,
        generation,
        payload.pattern(),
        annotation,
        mutable,
        payload.is_poisoned(),
    )
    .expect("same-module Local payload tamper");
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .locals()
        .revise_finalized(slots, local, replacement)
        .expect("test-only Local payload substitution");
}

fn staged_block_let_local(
    transaction: &mut StagedHirModuleTransaction<'_>,
    root: ExprId,
    ordinal: usize,
) -> LocalId {
    let (slots, arenas) = transaction.storage_mut();
    let statement = {
        let root = arenas
            .expressions()
            .resolve_staged(slots, root)
            .expect("staged block owner");
        let HirExprKind::Block(block) = root.kind() else {
            panic!("fixture root must remain a Block")
        };
        block.statements()[ordinal]
    };
    let statement = arenas
        .statements()
        .resolve_staged(slots, statement)
        .expect("staged Let statement");
    let HirStmtKind::Let { locals, .. } = statement.kind() else {
        panic!("selected statement must remain a Let")
    };
    locals[0]
}

fn assert_expression_freeze_rejects(
    document_id: &str,
    source: &str,
    tamper: impl FnOnce(&mut StagedHirModuleTransaction<'_>, ExprId),
) {
    let parsed = parsed_source(document_id, &[source.into()]);
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &parsed);
    let module_scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, module_scope)
        .expect("valid local-owner prefix");
    tamper(&mut transaction, root);
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());
}

fn assert_expression_source_freeze_rejects(
    document_id: &str,
    source: &str,
    tamper: impl FnOnce(&mut StagedHirModuleTransaction<'_>, ExprId),
) {
    let parsed = parsed_source(document_id, &[source.into()]);
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &parsed);
    let module_scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, module_scope)
        .expect("valid local-owner prefix");
    tamper(&mut transaction, root);
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());
}

fn assert_expression_local_freeze_rejects(
    document_id: &str,
    source: &str,
    select_local: impl FnOnce(&mut StagedHirModuleTransaction<'_>, ExprId) -> LocalId,
    tamper: LocalPayloadTamper,
) {
    assert_expression_freeze_rejects(document_id, source, |transaction, root| {
        let local = select_local(transaction, root);
        tamper_local_payload(transaction, local, tamper);
    });
}

fn stage_expression_with_manifest(
    transaction: &mut StagedHirModuleTransaction<'_>,
    parsed: &ParsedSource,
    attached: &AttachedExpressionNode,
    scope: ScopeId,
    manifest_kind: &HirExprKind,
    payload_kind: HirExprKind,
) -> ExprId {
    let reservation = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .reserve_source(
                slots,
                attached.id(),
                HirSourceSite::Span(attached.whole_source_span()),
            )
            .expect("forged expression reservation")
    };
    let owner = reservation.id();
    transaction
        .source_components()
        .stage_attached_expression(parsed, owner, attached, manifest_kind)
        .expect("the untampered expression manifest stages");
    let payload = HirExpr::try_new(scope, payload_kind, HirPoisonState::Clean)
        .expect("forged clean expression payload");
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .expressions()
        .finalize(slots, reservation, payload)
        .expect("forged expression finalization")
}

fn assert_synthetic_recovery_child(
    module: &HirModule,
    parent: ExprId,
    child: ExprId,
    ordinal: u32,
    role: HirExprSourceRole,
) {
    let metadata = module.slots().resolve(child).expect("recovery child slot");
    assert!(matches!(
        metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Expr(parent)
                && key.role() == SyntheticRole::RecoveryOperand
                && key.ordinal() == ordinal
    ));
    assert!(matches!(
        metadata.source_site(),
        HirSourceSite::Insertion(_)
    ));
    let child = expression(module, child);
    assert!(matches!(
        child.kind(),
        HirExprKind::Error(error)
            if error.issue() == HirGenericExprIssue::TransactionalChildFailure
    ));
    assert_eq!(
        child.state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand { role })
    );
}

fn assert_no_synthetic_recovery_child(module: &HirModule, parent: ExprId) {
    assert!(
        module
            .arenas()
            .expressions()
            .try_iter(module.slots())
            .expect("published expression inventory")
            .all(|(child, _)| !matches!(
                module.slots().resolve(child).expect("expression slot").origin(),
                HirOrigin::Synthetic(key)
                    if key.owner() == SyntheticOwner::Expr(parent)
                        && key.role() == SyntheticRole::RecoveryOperand
            ))
    );
}

fn root_index_candidate(module: &HirModule, owner: ExprId) -> (ExprId, &HirIndexExpr) {
    let HirExprKind::PostfixBracket(postfix) = expression(module, owner).kind() else {
        panic!("fixture root must remain the ambiguous E34 postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
        panic!("ordinary-index candidate must remain typed");
    };
    let HirExprKind::Index(payload) = expression(module, *index).kind() else {
        panic!("ordinary interpretation must retain its E14 Index root");
    };
    (*index, payload)
}

fn assert_accepted_expression_module_unchanged(
    database: &HirDatabase,
    key: &HirModuleKey,
    accepted: &Arc<HirModule>,
    owner: ExprId,
) {
    let current = database
        .current(key)
        .expect("accepted module stays current");
    assert!(Arc::ptr_eq(accepted, &current));
    assert_eq!(current.diagnostics(), accepted.diagnostics());
    assert_eq!(
        current
            .arenas()
            .expressions()
            .try_iter(current.slots())
            .expect("accepted expression inventory")
            .count(),
        1
    );
    let lookup = current
        .source_site(
            accepted.provenance().source_identity(),
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::LiteralBody,
            },
        )
        .expect("accepted literal source remains queryable");
    assert_eq!(lookup.owner_status(), HirSourceOwnerStatus::Clean);
    assert!(matches!(
        lookup.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));
}

#[test]
fn attached_e01_through_e07_leaf_matrix_uses_shared_final_payloads() {
    let parsed = parsed_source(
        "leaf-matrix",
        &[
            "()".into(),
            "42ms".into(),
            "@scene.entry".into(),
            "'line.focus?".into(),
            "game::actor".into(),
            ".Ready".into(),
            "_".into(),
            "^".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);
    assert!(matches!(
        expression(&module, owners[0]).kind(),
        HirExprKind::Unit
    ));
    assert!(matches!(
        expression(&module, owners[1]).kind(),
        HirExprKind::Literal(HirLiteral::Duration(HirDurationLiteral::Value(_)))
    ));
    assert!(matches!(
        expression(&module, owners[2]).kind(),
        HirExprKind::EntityReference(value) if value.as_resolved().is_some()
    ));
    assert!(matches!(
        expression(&module, owners[3]).kind(),
        HirExprKind::LifetimePath(HirLifetimePathValue::Resolved(path))
            if matches!(path.scope(), HirLifetimeRegistryScope::Line)
                && path.segments().len() == 1
                && path.optional()
    ));
    assert!(matches!(
        expression(&module, owners[4]).kind(),
        HirExprKind::Path(HirPathValue::Resolved(path))
            if matches!(path.segments(),
                [HirPathSegment::Identifier(first), HirPathSegment::Identifier(second)]
                if first.as_str() == "game" && second.as_str() == "actor")
    ));
    assert!(matches!(
        expression(&module, owners[5]).kind(),
        HirExprKind::ShortVariant(HirShortVariantName::Resolved(name))
            if name.as_str() == "Ready"
    ));
    assert!(matches!(
        expression(&module, owners[6]).kind(),
        HirExprKind::Placeholder(HirPlaceholderKind::PartialApplication)
    ));
    assert!(matches!(
        expression(&module, owners[7]).kind(),
        HirExprKind::Placeholder(HirPlaceholderKind::PipeLeft)
    ));
}

#[test]
fn attached_e08_through_e11_composites_publish_exact_children_and_idless_numbers() {
    let parsed = parsed_source(
        "composite-matrix",
        &[
            "(1, (2),)".into(),
            "[true, .Ready]".into(),
            "[0xff_u8, 2, 0b11_u8]".into(),
            "[value; 3]".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let HirExprKind::Tuple(tuple) = expression(&module, owners[0]).kind() else {
        panic!("E08 tuple payload");
    };
    assert_eq!(tuple.elements().len(), 2);
    assert!(matches!(
        expression(&module, tuple.elements()[0]).kind(),
        HirExprKind::Literal(HirLiteral::Integer(_))
    ));
    let grouped_child = module
        .slots()
        .resolve(tuple.elements()[1])
        .expect("grouped tuple child");
    let HirSourceSite::Span(grouped_child_site) = grouped_child.source_site() else {
        panic!("authored grouped child remains source-backed");
    };
    assert_eq!(
        grouped_child_site.range().end() - grouped_child_site.range().start(),
        1
    );
    let grouped_component = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[0],
                role: HirExprSourceRole::Element { ordinal: 1 },
            },
        )
        .expect("grouped tuple component");
    assert!(matches!(
        grouped_component.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(span))
            if span.range().end() - span.range().start() == 3
    ));

    let HirExprKind::BracketSequence(sequence) = expression(&module, owners[1]).kind() else {
        panic!("E09 bracket sequence payload");
    };
    assert_eq!(sequence.elements().len(), 2);

    let HirExprKind::NumericBracketSequence(sequence) = expression(&module, owners[2]).kind()
    else {
        panic!("E10 numeric sequence payload");
    };
    assert_eq!(sequence.elements().len(), 3);
    assert_eq!(sequence.common_suffix(), Some(HirIntegerSuffix::U8));
    assert_eq!(sequence.recovery(), &HirNumericSequenceRecovery::Complete);
    assert_eq!(sequence.elements()[0].radix(), HirIntegerRadix::Hexadecimal);
    assert_eq!(sequence.elements()[0].magnitude().limbs_le(), &[0xff]);
    assert_eq!(sequence.elements()[1].radix(), HirIntegerRadix::Decimal);
    assert_eq!(sequence.elements()[2].radix(), HirIntegerRadix::Binary);
    let suffix = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[2],
                role: HirExprSourceRole::NumericCommonSuffix,
            },
        )
        .expect("numeric common suffix component");
    assert!(matches!(suffix.presence(), HirSourcePresence::Present(_)));

    let HirExprKind::ArrayRepeat(repeat) = expression(&module, owners[3]).kind() else {
        panic!("E11 array-repeat payload");
    };
    assert!(matches!(
        expression(&module, repeat.value()).kind(),
        HirExprKind::Path(HirPathValue::Resolved(_))
    ));
    assert!(matches!(
        expression(&module, repeat.length()).kind(),
        HirExprKind::Literal(HirLiteral::Integer(_))
    ));
}

#[test]
fn attached_e14_through_e17_publish_exact_children_forms_and_source_roles() {
    let parsed = parsed_source(
        "pratt-composite-matrix",
        &[
            "items[key]".into(),
            "left |> right".into(),
            "try value".into(),
            "value?".into(),
            "await value".into(),
            "try await value".into(),
            "await? value".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let (_, index) = root_index_candidate(&module, owners[0]);
    assert!(matches!(
        expression(&module, index.target()).kind(),
        HirExprKind::Path(_)
    ));
    assert!(matches!(
        expression(&module, index.index()).kind(),
        HirExprKind::Path(_)
    ));

    let HirExprKind::Pipe(pipe) = expression(&module, owners[1]).kind() else {
        panic!("E15 pipe payload");
    };
    assert_ne!(pipe.left(), pipe.right());

    for (ordinal, expected) in [(2, HirTryForm::PrefixTry), (3, HirTryForm::PostfixQuestion)] {
        let HirExprKind::Try(expression) = expression(&module, owners[ordinal]).kind() else {
            panic!("E16 try payload");
        };
        assert_eq!(expression.form(), expected);
    }
    for (ordinal, expected) in [
        (4, HirAwaitPropagation::PreserveResult),
        (5, HirAwaitPropagation::PropagateError),
        (6, HirAwaitPropagation::PropagateError),
    ] {
        let HirExprKind::Await(expression) = expression(&module, owners[ordinal]).kind() else {
            panic!("E17 await payload");
        };
        assert_eq!(expression.propagation(), expected);
    }

    for (owner, roles) in [
        (
            owners[0],
            &[
                HirExprSourceRole::Target,
                HirExprSourceRole::OpenBracket,
                HirExprSourceRole::CloseBracket,
                HirExprSourceRole::Content,
            ][..],
        ),
        (
            owners[1],
            &[
                HirExprSourceRole::LeftOperand,
                HirExprSourceRole::RightOperand,
            ][..],
        ),
        (
            owners[2],
            &[HirExprSourceRole::Operand, HirExprSourceRole::Operator][..],
        ),
        (
            owners[4],
            &[HirExprSourceRole::Operand, HirExprSourceRole::Operator][..],
        ),
    ] {
        for role in roles {
            let source = module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Expr { owner, role: *role },
                )
                .expect("E14-E17 source role");
            assert_eq!(source.owner_status(), HirSourceOwnerStatus::Clean);
            assert!(matches!(
                source.presence(),
                HirSourcePresence::Present(HirSourceSite::Span(_))
            ));
        }
    }

    assert_eq!(
        module.source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[1],
                role: HirExprSourceRole::Operator,
            },
        ),
        Err(HirSourceQueryError::ExprRoleNotApplicable {
            owner: owners[1],
            role: HirExprSourceRole::Operator,
        })
    );
}

#[test]
fn attached_e23_e24_e26_publish_exact_prefix_payloads_and_source_roles() {
    let parsed = parsed_source(
        "prefix-owner-matrix",
        &[
            "& value".into(),
            "& mut value".into(),
            "*value".into(),
            "!value".into(),
            "-value".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    for (ordinal, expected) in [(0, HirBorrowKind::Shared), (1, HirBorrowKind::Mutable)] {
        let HirExprKind::Borrow(borrow) = expression(&module, owners[ordinal]).kind() else {
            panic!("E23 borrow payload");
        };
        assert_eq!(borrow.kind(), expected);
        assert!(matches!(
            expression(&module, borrow.operand()).kind(),
            HirExprKind::Path(_)
        ));
    }

    let HirExprKind::Dereference(dereference) = expression(&module, owners[2]).kind() else {
        panic!("E24 dereference payload");
    };
    assert!(matches!(
        expression(&module, dereference.operand()).kind(),
        HirExprKind::Path(_)
    ));

    for (ordinal, expected) in [(3, HirUnaryOp::Not), (4, HirUnaryOp::Negate)] {
        let HirExprKind::Unary(unary) = expression(&module, owners[ordinal]).kind() else {
            panic!("E26 unary payload");
        };
        assert_eq!(unary.operator(), expected);
    }

    for owner in owners {
        for role in [HirExprSourceRole::Operator, HirExprSourceRole::Operand] {
            let source = module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Expr { owner, role },
                )
                .expect("prefix source component");
            assert_eq!(source.owner_status(), HirSourceOwnerStatus::Clean);
            assert!(matches!(
                source.presence(),
                HirSourcePresence::Present(HirSourceSite::Span(_))
            ));
        }
    }
}

#[test]
fn attached_e19_range_publishes_optional_endpoints_and_exact_source_roles() {
    let parsed = parsed_source(
        "range-owner-matrix",
        &[
            "start..=end".into(),
            "..end".into(),
            "start..".into(),
            "start..=".into(),
            "..".into(),
            "..=".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    for (ordinal, has_start, has_end, inclusive) in [
        (0, true, true, true),
        (1, false, true, false),
        (2, true, false, false),
        (3, true, false, true),
        (4, false, false, false),
        (5, false, false, true),
    ] {
        let owner = owners[ordinal];
        let HirExprKind::Range(range) = expression(&module, owner).kind() else {
            panic!("E19 range payload");
        };
        assert_eq!(range.start().is_some(), has_start);
        assert_eq!(range.end().is_some(), has_end);
        assert_eq!(range.inclusive(), inclusive);
        assert_no_synthetic_recovery_child(&module, owner);

        for (role, present) in [
            (HirExprSourceRole::RangeStart, has_start),
            (HirExprSourceRole::RangeEnd, has_end),
            (HirExprSourceRole::RangeInclusiveMarker, inclusive),
        ] {
            let source = module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Expr { owner, role },
                )
                .expect("E19 applicable source role");
            assert_eq!(source.owner_status(), HirSourceOwnerStatus::Clean);
            if present {
                assert!(matches!(source.presence(), HirSourcePresence::Present(_)));
            } else {
                assert_eq!(source.presence(), HirSourcePresence::AbsentOptional);
            }
        }
    }

    let inclusive_marker = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[0],
                role: HirExprSourceRole::RangeInclusiveMarker,
            },
        )
        .expect("inclusive range marker source");
    assert!(matches!(
        inclusive_marker.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(span))
            if span.range().as_range().len() == 3
    ));
}

#[test]
fn attached_e22_binary_publishes_closed_operators_children_and_source_roles() {
    let cases = [
        ("left => right", HirBinaryOp::Implies),
        ("left || right", HirBinaryOp::Or),
        ("left && right", HirBinaryOp::And),
        ("left in right", HirBinaryOp::In),
        ("left == right", HirBinaryOp::Equal),
        ("left != right", HirBinaryOp::NotEqual),
        ("left >= right", HirBinaryOp::GreaterOrEqual),
        ("left <= right", HirBinaryOp::LessOrEqual),
        ("left > right", HirBinaryOp::Greater),
        ("left < right", HirBinaryOp::Less),
        ("left & right", HirBinaryOp::Merge),
        ("left + right", HirBinaryOp::Add),
        ("left - right", HirBinaryOp::Subtract),
        ("left * right", HirBinaryOp::Multiply),
        ("left / right", HirBinaryOp::Divide),
        ("left % right", HirBinaryOp::Remainder),
    ];
    let parsed = parsed_source(
        "binary-owner-matrix",
        &cases
            .iter()
            .map(|(source, _)| (*source).to_owned())
            .collect::<Vec<_>>(),
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    for ((_, expected), owner) in cases.into_iter().zip(owners) {
        let HirExprKind::Binary(binary) = expression(&module, owner).kind() else {
            panic!("E22 binary payload");
        };
        assert_eq!(binary.operator(), expected);
        assert_ne!(binary.left(), binary.right());
        for role in [
            HirExprSourceRole::LeftOperand,
            HirExprSourceRole::Operator,
            HirExprSourceRole::RightOperand,
        ] {
            let source = module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Expr { owner, role },
                )
                .expect("E22 source component");
            assert_eq!(source.owner_status(), HirSourceOwnerStatus::Clean);
            assert!(matches!(source.presence(), HirSourcePresence::Present(_)));
        }
    }
}

#[test]
fn typed_child_beats_disagreeing_display_source() {
    const DISPLAY_SOURCE: &str = "proof/non-authoritative/false - true.arcw";
    let parsed = DisagreeingDisplayTypedExpressionBuilder {
        document_id: "arcweft-test://lang-hir/typed-child-authority.arcw",
        display_source: DISPLAY_SOURCE,
        typed_expression: "40 + 2",
    }
    .build();
    assert_eq!(
        parsed.document().display_name().display_name(),
        DISPLAY_SOURCE
    );

    let attached = attached_expressions(&parsed)
        .pop()
        .expect("one typed expression owner");
    let ExpressionProjection::Binary { operator, .. } = attached.projection() else {
        panic!("typed fixture must retain its parser-selected Binary family");
    };
    assert_eq!(*operator, SyntaxBinaryOperator::Add);
    let typed_children = attached
        .children()
        .iter()
        .map(|child| {
            child
                .authored_semantic()
                .expect("typed child access")
                .expect("both Binary children are authored")
                .id()
        })
        .collect::<Vec<_>>();
    assert_eq!(typed_children.len(), 2);

    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);
    assert_eq!(
        module.provenance().document().display_name().display_name(),
        DISPLAY_SOURCE,
        "the disagreeing label remains real accepted provenance"
    );
    let HirExprKind::Binary(binary) = expression(&module, owners[0]).kind() else {
        panic!("typed Binary must lower without consulting the display label");
    };
    assert_eq!(binary.operator(), HirBinaryOp::Add);

    for (owner, expected_syntax) in [binary.left(), binary.right()]
        .into_iter()
        .zip(typed_children)
    {
        let metadata = module.slots().resolve(owner).expect("typed child slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Source(source) if source.syntax() == expected_syntax
        ));
    }

    for (owner, expected_magnitude) in [(binary.left(), 40_u32), (binary.right(), 2_u32)] {
        let HirExprKind::Literal(HirLiteral::Integer(HirIntegerLiteral::Value {
            magnitude,
            radix,
            suffix,
        })) = expression(&module, owner).kind()
        else {
            panic!("typed Binary child must retain its integer literal value");
        };
        assert_eq!(magnitude.limbs_le(), &[expected_magnitude]);
        assert_eq!(*radix, HirIntegerRadix::Decimal);
        assert_eq!(*suffix, None);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test is the closed E20/E21 typed record-field, child, shorthand, and source-role matrix"
)]
fn attached_e20_e21_records_publish_typed_fields_children_and_source_roles() {
    let parsed = parsed_source(
        "record-owner-matrix",
        &[
            "Point { x = left, y: right }".into(),
            "{ first = value, second: other }".into(),
            "Point { x = left, y: }".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let HirExprKind::Record(record) = expression(&module, owners[0]).kind() else {
        panic!("E20 record payload");
    };
    assert!(matches!(
        &record.path().segments()[0],
        HirPathSegment::Identifier(name) if name.as_str() == "Point"
    ));
    assert_eq!(record.fields().len(), 2);
    for (field, expected_name) in record.fields().iter().zip(["x", "y"]) {
        assert!(matches!(
            field,
            HirRecordField::Explicit { name, value }
                if name.as_str() == expected_name
                    && matches!(expression(&module, *value).kind(), HirExprKind::Path(_))
        ));
    }
    for role in [
        HirExprSourceRole::RecordPath,
        HirExprSourceRole::RecordField {
            field: 0,
            part: HirRecordFieldSourcePart::Whole,
        },
        HirExprSourceRole::RecordField {
            field: 0,
            part: HirRecordFieldSourcePart::Name,
        },
        HirExprSourceRole::RecordField {
            field: 0,
            part: HirRecordFieldSourcePart::Colon,
        },
        HirExprSourceRole::RecordField {
            field: 0,
            part: HirRecordFieldSourcePart::Value,
        },
    ] {
        let source = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: owners[0],
                    role,
                },
            )
            .expect("E20 source component");
        assert_eq!(source.owner_status(), HirSourceOwnerStatus::Clean);
        assert!(matches!(source.presence(), HirSourcePresence::Present(_)));
    }

    let HirExprKind::RecordLiteral(literal) = expression(&module, owners[1]).kind() else {
        panic!("E21 record-literal payload");
    };
    assert_eq!(literal.fields().len(), 2);
    assert!(literal.fields().iter().all(|field| matches!(
        field,
        HirRecordField::Explicit { value, .. }
            if matches!(expression(&module, *value).kind(), HirExprKind::Path(_))
    )));

    let missing_owner = owners[2];
    let HirExprKind::Record(missing) = expression(&module, missing_owner).kind() else {
        panic!("missing E20 value remains a typed record");
    };
    assert!(matches!(
        &missing.fields()[1],
        HirRecordField::Invalid {
            issue: HirRecordFieldIssue::MissingValue
        }
    ));
    let role = HirExprSourceRole::RecordField {
        field: 1,
        part: HirRecordFieldSourcePart::Value,
    };
    assert_eq!(
        expression(&module, missing_owner).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand { role })
    );
    let key = SyntheticKey::try_new(
        SyntheticOwner::Expr(missing_owner),
        SyntheticRole::RecoveryOperand,
        1,
    )
    .expect("record recovery key");
    let child = module
        .slots()
        .resolve_prepared_synthetic::<ExprId>(key)
        .expect("record recovery lookup")
        .expect("record missing value child");
    assert_synthetic_recovery_child(&module, missing_owner, child, 1, role);
    let source = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: missing_owner,
                role,
            },
        )
        .expect("missing record value source");
    assert_eq!(source.owner_status(), HirSourceOwnerStatus::Poisoned);
    assert!(matches!(
        source.presence(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
    ));
}

#[test]
fn e20_invalid_fields_remain_typed_and_c05_never_guesses_a_local() {
    let parsed = parsed_source(
        "record-field-recovery",
        &[
            "Point { x = left, x = right }".into(),
            "Point { = value }".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);
    let HirExprKind::Record(duplicate) = expression(&module, owners[0]).kind() else {
        panic!("duplicate field remains E20");
    };
    assert!(matches!(
        &duplicate.fields()[1],
        HirRecordField::Invalid {
            issue: HirRecordFieldIssue::DuplicateName
        }
    ));
    let HirExprKind::Record(missing_name) = expression(&module, owners[1]).kind() else {
        panic!("missing field name remains E20");
    };
    assert!(matches!(
        &missing_name.fields()[0],
        HirRecordField::Invalid {
            issue: HirRecordFieldIssue::MissingName
        }
    ));

    let shorthand = parsed_source("record-shorthand-timeline", &["Point { local }".into()]);
    let attached = attached_expressions(&shorthand).pop().unwrap();
    let database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &shorthand);
    let scope = allocate_module_scope(&mut transaction, &shorthand);
    assert_eq!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidLocalTimeline
        ))
    );
    assert!(database.current(&module_key(&shorthand)).is_none());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test is the complete E27 statement/local/tail ownership matrix for authored and synthetic tails"
)]
fn e27_block_owns_statements_locals_and_authored_or_synthetic_tail() {
    let parsed = parsed_source(
        "block-owner-matrix",
        &[
            "{ let local = 1; Point { local } }".into(),
            "{ let omitted = 2; }".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let HirExprKind::Block(authored) = expression(&module, owners[0]).kind() else {
        panic!("E27 authored-tail block payload");
    };
    assert_eq!(authored.statements().len(), 1);
    let block_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), authored.scope())
        .expect("E27 block scope");
    assert_eq!(block_scope.kind(), HirScopeKind::Block);
    assert_eq!(block_scope.owner(), &HirScopeOwner::Expr(owners[0]));
    let parent = block_scope.parent().expect("E27 enclosing module scope");
    let parent_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), parent)
        .expect("E27 parent scope");
    assert_eq!(parent_scope.kind(), HirScopeKind::Module);
    assert!(parent_scope.children().contains(&authored.scope()));

    let statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), authored.statements()[0])
        .expect("E27 let statement");
    assert_eq!(statement.scope(), authored.scope());
    let HirStmtKind::Let {
        pattern,
        initializer,
        locals,
        ..
    } = statement.kind()
    else {
        panic!("E27 typed Let statement");
    };
    assert_eq!(locals.len(), 1);
    assert!(matches!(
        expression(&module, *initializer).kind(),
        HirExprKind::Literal(HirLiteral::Integer(_))
    ));
    let local = module
        .arenas()
        .locals()
        .resolve(module.slots(), locals[0])
        .expect("E27 let local");
    assert_eq!(local.scope(), authored.scope());
    assert_eq!(local.kind(), HirLocalKind::LetBinding);
    assert_eq!(local.name().as_str(), "local");
    assert_eq!(local.pattern(), Some(*pattern));
    assert_eq!(block_scope.locals(), locals.as_ref());

    let HirExprKind::Record(tail) = expression(&module, authored.tail()).kind() else {
        panic!("E27 authored record tail");
    };
    assert_eq!(
        expression(&module, authored.tail()).scope(),
        authored.scope()
    );
    assert!(matches!(
        tail.fields(),
        [HirRecordField::Shorthand { name, local: shorthand_local }]
            if name.as_str() == "local" && *shorthand_local == locals[0]
    ));
    for role in [
        HirExprSourceRole::Statement { ordinal: 0 },
        HirExprSourceRole::Tail,
    ] {
        let source = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: owners[0],
                    role,
                },
            )
            .expect("E27 authored block component");
        assert_eq!(source.owner_status(), HirSourceOwnerStatus::Clean);
        assert!(matches!(
            source.presence(),
            HirSourcePresence::Present(HirSourceSite::Span(_))
        ));
    }

    let HirExprKind::Block(omitted) = expression(&module, owners[1]).kind() else {
        panic!("E27 omitted-tail block payload");
    };
    let tail_metadata = module
        .slots()
        .resolve(omitted.tail())
        .expect("E27 implicit Unit tail slot");
    assert!(matches!(
        tail_metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Expr(owners[1])
                && key.role() == SyntheticRole::ImplicitUnitTail
                && key.ordinal() == 0
    ));
    assert!(matches!(
        tail_metadata.source_site(),
        HirSourceSite::Insertion(_)
    ));
    assert_eq!(expression(&module, omitted.tail()).scope(), omitted.scope());
    assert!(matches!(
        expression(&module, omitted.tail()).kind(),
        HirExprKind::Unit
    ));
    let tail_source = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[1],
                role: HirExprSourceRole::Tail,
            },
        )
        .expect("E27 omitted-tail insertion");
    assert_eq!(tail_source.owner_status(), HirSourceOwnerStatus::Clean);
    assert!(matches!(
        tail_source.presence(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test validates the complete E27 statement and tail recovery precedence matrix"
)]
fn e27_block_keeps_typed_poison_for_statement_and_tail_recovery() {
    let parsed = parsed_source(
        "block-recovery-matrix",
        &["{ let missing =; }".into(), "{ 1 + }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let HirExprKind::Block(statement_recovery) = expression(&module, owners[0]).kind() else {
        panic!("E27 missing-initializer owner remains Block");
    };
    assert_eq!(statement_recovery.statements().len(), 1);
    assert_eq!(
        expression(&module, owners[0]).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::Statement { ordinal: 0 },
            },
        ))
    );
    let statement_id = statement_recovery.statements()[0];
    let statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), statement_id)
        .expect("E27 recovered Let statement");
    let HirStmtKind::Let { initializer, .. } = statement.kind() else {
        panic!("E27 missing initializer remains a typed Let");
    };
    let initializer_metadata = module
        .slots()
        .resolve(*initializer)
        .expect("E27 missing initializer slot");
    assert!(matches!(
        initializer_metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Stmt(statement_id)
                && key.role() == SyntheticRole::RecoveryOperand
                && key.ordinal() == 0
    ));
    assert!(matches!(
        initializer_metadata.source_site(),
        HirSourceSite::Insertion(_)
    ));
    assert!(matches!(
        expression(&module, *initializer).kind(),
        HirExprKind::Error(error)
            if error.issue() == HirGenericExprIssue::TransactionalChildFailure
    ));
    assert_eq!(
        expression(&module, *initializer).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::Operand,
        })
    );
    let statement_source = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[0],
                role: HirExprSourceRole::Statement { ordinal: 0 },
            },
        )
        .expect("E27 recovered statement component");
    assert_eq!(
        statement_source.owner_status(),
        HirSourceOwnerStatus::Poisoned
    );
    assert!(matches!(
        statement_source.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));

    let HirExprKind::Block(tail_recovery) = expression(&module, owners[1]).kind() else {
        panic!("E27 poisoned-tail owner remains Block");
    };
    assert_eq!(
        expression(&module, owners[1]).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::Tail,
            },
        ))
    );
    let HirExprKind::Binary(tail) = expression(&module, tail_recovery.tail()).kind() else {
        panic!("E27 authored poisoned tail remains Binary");
    };
    assert_eq!(
        expression(&module, tail_recovery.tail()).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::RightOperand,
        })
    );
    assert_synthetic_recovery_child(
        &module,
        tail_recovery.tail(),
        tail.right(),
        1,
        HirExprSourceRole::RightOperand,
    );
    let tail_source = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[1],
                role: HirExprSourceRole::Tail,
            },
        )
        .expect("E27 poisoned tail component");
    assert_eq!(tail_source.owner_status(), HirSourceOwnerStatus::Poisoned);
    assert!(matches!(
        tail_source.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));
}

#[test]
fn c05_shorthand_uses_the_source_visible_same_scope_generation() {
    let parsed = parsed_source(
        "record-shorthand-shadow-timeline",
        &["{ let value = 1; Point { value }; let value = 2; Point { value } }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);
    let HirExprKind::Block(block) = expression(&module, owners[0]).kind() else {
        panic!("C05 shadow fixture owns one E27 block");
    };
    assert_eq!(block.statements().len(), 3);

    let first_statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), block.statements()[0])
        .expect("first C05 let");
    let HirStmtKind::Let {
        locals: first_locals,
        ..
    } = first_statement.kind()
    else {
        panic!("first C05 statement is Let");
    };
    let first_local = first_locals[0];

    let record_statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), block.statements()[1])
        .expect("intermediate C05 record statement");
    let HirStmtKind::Expression {
        expression: first_record,
    } = record_statement.kind()
    else {
        panic!("intermediate C05 statement retains its expression");
    };
    let HirExprKind::Record(first_record) = expression(&module, *first_record).kind() else {
        panic!("intermediate C05 record payload");
    };
    assert!(matches!(
        first_record.fields(),
        [HirRecordField::Shorthand { local, .. }] if *local == first_local
    ));

    let second_statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), block.statements()[2])
        .expect("second C05 let");
    let HirStmtKind::Let {
        locals: second_locals,
        ..
    } = second_statement.kind()
    else {
        panic!("second C05 binding is Let");
    };
    let second_local = second_locals[0];
    assert_ne!(first_local, second_local);
    let first_payload = module
        .arenas()
        .locals()
        .resolve(module.slots(), first_local)
        .expect("first C05 local");
    let second_payload = module
        .arenas()
        .locals()
        .resolve(module.slots(), second_local)
        .expect("second C05 local");
    assert_eq!(first_payload.generation(), LocalGeneration::FIRST);
    assert_eq!(
        second_payload.generation(),
        LocalGeneration::FIRST
            .checked_next()
            .expect("second local generation")
    );

    let HirExprKind::Record(tail) = expression(&module, block.tail()).kind() else {
        panic!("tail C05 record payload");
    };
    assert!(matches!(
        tail.fields(),
        [HirRecordField::Shorthand { local, .. }] if *local == second_local
    ));
    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), block.scope())
        .expect("C05 block scope");
    assert_eq!(scope.locals(), &[first_local, second_local]);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test executes one closed tail/shorthand source-freeze tamper matrix atomically"
)]
fn block_freeze_rejects_tail_and_shorthand_local_substitution() {
    let parsed = parsed_source(
        "block-tail-substitution",
        &["{ let value = 1; Point { value } }".into()],
    );
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &parsed);
    let module_scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, module_scope)
        .expect("valid E27 prefix");
    let (block_scope, statements, initializer) = {
        let (slots, arenas) = transaction.storage_mut();
        let root_payload = arenas
            .expressions()
            .resolve_staged(slots, root)
            .expect("staged E27 root");
        let HirExprKind::Block(block) = root_payload.kind() else {
            panic!("staged E27 block");
        };
        let block_scope = block.scope();
        let statements = block.statements().to_vec().into_boxed_slice();
        let statement = arenas
            .statements()
            .resolve_staged(slots, statements[0])
            .expect("staged E27 Let");
        let HirStmtKind::Let { initializer, .. } = statement.kind() else {
            panic!("staged E27 Let payload");
        };
        (block_scope, statements, *initializer)
    };
    let replacement = HirExpr::try_new(
        module_scope,
        HirExprKind::Block(HirBlockExpr::new(block_scope, statements, initializer)),
        HirPoisonState::Clean,
    )
    .expect("same-module forged Block is locally constructible");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .revise_finalized(slots, root, replacement)
            .expect("test-only Block payload substitution");
    }
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());

    let parsed = parsed_source(
        "record-shorthand-local-substitution",
        &["{ let value = 1; let value = 2; Point { value } }".into()],
    );
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &parsed);
    let module_scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, module_scope)
        .expect("valid C05 prefix");
    let (tail, block_scope, path, name, wrong_local) = {
        let (slots, arenas) = transaction.storage_mut();
        let root_payload = arenas
            .expressions()
            .resolve_staged(slots, root)
            .expect("staged C05 root");
        let HirExprKind::Block(block) = root_payload.kind() else {
            panic!("staged C05 block");
        };
        let tail = block.tail();
        let block_scope = block.scope();
        let first_statement_id = block.statements()[0];
        let first_statement = arenas
            .statements()
            .resolve_staged(slots, first_statement_id)
            .expect("staged first Let");
        let HirStmtKind::Let { locals, .. } = first_statement.kind() else {
            panic!("staged first Let payload");
        };
        let wrong_local = locals[0];
        let tail_payload = arenas
            .expressions()
            .resolve_staged(slots, tail)
            .expect("staged C05 record");
        let HirExprKind::Record(record) = tail_payload.kind() else {
            panic!("staged C05 Record payload");
        };
        let name = record.fields()[0]
            .name()
            .expect("staged shorthand name")
            .clone();
        (tail, block_scope, record.path().clone(), name, wrong_local)
    };
    let replacement = HirExpr::try_new(
        block_scope,
        HirExprKind::Record(HirRecordExpr::new(
            path,
            Box::new([HirRecordField::shorthand(name, wrong_local)]),
        )),
        HirPoisonState::Clean,
    )
    .expect("same-module forged shorthand is locally constructible");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .revise_finalized(slots, tail, replacement)
            .expect("test-only shorthand LocalId substitution");
    }
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());
}

#[test]
fn block_let_freeze_rejects_exact_local_payload_tampering() {
    assert_expression_local_freeze_rejects(
        "block-let-local-name",
        "{ let value = 1; value }",
        |transaction, root| staged_block_let_local(transaction, root, 0),
        LocalPayloadTamper::Name("renamed"),
    );
    assert_expression_local_freeze_rejects(
        "block-let-local-generation",
        "{ let value = 1; let value = 2; value }",
        |transaction, root| staged_block_let_local(transaction, root, 1),
        LocalPayloadTamper::Generation(LocalGeneration::FIRST),
    );
    assert_expression_local_freeze_rejects(
        "block-let-local-mutability",
        "{ let value = 1; value }",
        |transaction, root| staged_block_let_local(transaction, root, 0),
        LocalPayloadTamper::Mutable(true),
    );
}

#[test]
fn e22_missing_operands_keep_typed_parents_and_fixed_synthetic_ordinals() {
    let parsed = parsed_source(
        "binary-owner-recovery",
        &["left +".into(), "+ right".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);
    let HirExprKind::Binary(binary) = expression(&module, owners[0]).kind() else {
        panic!("missing binary right operand remains E22");
    };
    assert_eq!(binary.operator(), HirBinaryOp::Add);
    assert_synthetic_recovery_child(
        &module,
        owners[0],
        binary.right(),
        1,
        HirExprSourceRole::RightOperand,
    );

    let HirExprKind::Binary(binary) = expression(&module, owners[1]).kind() else {
        panic!("missing binary left operand remains E22");
    };
    assert_eq!(binary.operator(), HirBinaryOp::Add);
    assert_synthetic_recovery_child(
        &module,
        owners[1],
        binary.left(),
        0,
        HirExprSourceRole::LeftOperand,
    );
}

#[test]
fn e23_e24_e26_missing_operands_keep_typed_parents_and_synthetic_children() {
    let parsed = parsed_source(
        "prefix-owner-recovery",
        &["&".into(), "*".into(), "!".into(), "-".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let operands = owners
        .iter()
        .map(|owner| match expression(&module, *owner).kind() {
            HirExprKind::Borrow(expression) => expression.operand(),
            HirExprKind::Dereference(expression) => expression.operand(),
            HirExprKind::Unary(expression) => expression.operand(),
            _ => panic!("known prefix family remains typed"),
        })
        .collect::<Vec<_>>();
    for (owner, operand) in owners.iter().copied().zip(operands) {
        assert_synthetic_recovery_child(&module, owner, operand, 0, HirExprSourceRole::Operand);
    }
    assert_eq!(
        module
            .diagnostics()
            .iter()
            .filter(|diagnostic| matches!(diagnostic, HirDiagnostic::Recovery(_)))
            .count(),
        4
    );
}

#[test]
fn e15_through_e17_missing_operands_keep_typed_parents_and_synthetic_children() {
    let parsed = parsed_source(
        "pratt-composite-recovery",
        &["left |>".into(), "try".into(), "await".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let HirExprKind::Pipe(pipe) = expression(&module, owners[0]).kind() else {
        panic!("missing pipe operand remains E15");
    };
    assert_synthetic_recovery_child(
        &module,
        owners[0],
        pipe.right(),
        1,
        HirExprSourceRole::RightOperand,
    );

    let HirExprKind::Try(tried) = expression(&module, owners[1]).kind() else {
        panic!("missing try operand remains E16");
    };
    assert_synthetic_recovery_child(
        &module,
        owners[1],
        tried.operand(),
        0,
        HirExprSourceRole::Operand,
    );

    let HirExprKind::Await(awaited) = expression(&module, owners[2]).kind() else {
        panic!("missing await operand remains E17");
    };
    assert_synthetic_recovery_child(
        &module,
        owners[2],
        awaited.operand(),
        0,
        HirExprSourceRole::Operand,
    );

    assert_eq!(
        module
            .diagnostics()
            .iter()
            .filter(|diagnostic| matches!(diagnostic, HirDiagnostic::Recovery(_)))
            .count(),
        3
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test is the closed composite recovery matrix for typed parents and fixed synthetic descendants"
)]
fn composite_recovery_keeps_typed_parents_and_exact_synthetic_children() {
    let parsed = parsed_source(
        "composite-recovery",
        &[
            "(1,,(2))".into(),
            "[true,,false]".into(),
            "[1u8, 2u16]".into(),
            "[1u8,]".into(),
            "[;]".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let HirExprKind::Tuple(tuple) = expression(&module, owners[0]).kind() else {
        panic!("recovered tuple remains E08");
    };
    assert_eq!(tuple.elements().len(), 3);
    assert_eq!(
        expression(&module, owners[0]).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::Element { ordinal: 1 }
        })
    );
    assert_synthetic_recovery_child(
        &module,
        owners[0],
        tuple.elements()[1],
        1,
        HirExprSourceRole::Element { ordinal: 1 },
    );

    let HirExprKind::BracketSequence(bracket) = expression(&module, owners[1]).kind() else {
        panic!("recovered bracket remains E09");
    };
    assert_synthetic_recovery_child(
        &module,
        owners[1],
        bracket.elements()[1],
        1,
        HirExprSourceRole::Element { ordinal: 1 },
    );

    let HirExprKind::NumericBracketSequence(conflict) = expression(&module, owners[2]).kind()
    else {
        panic!("suffix conflict remains E10");
    };
    assert!(matches!(
        conflict.recovery(),
        HirNumericSequenceRecovery::ConflictingSuffix {
            ordinal: 1,
            first: HirIntegerSuffix::U8,
            conflicting: HirIntegerSuffix::U16,
        }
    ));
    assert_eq!(
        expression(&module, owners[2]).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidNumericSequence)
    );

    let HirExprKind::NumericBracketSequence(missing) = expression(&module, owners[3]).kind() else {
        panic!("trailing numeric separator remains E10");
    };
    assert_eq!(
        missing.recovery(),
        &HirNumericSequenceRecovery::MissingFinalElement { ordinal: 1 }
    );
    assert!(
        module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: owners[3],
                    role: HirExprSourceRole::NumericElement { ordinal: 1 },
                },
            )
            .is_ok_and(|site| matches!(
                site.presence(),
                HirSourcePresence::Present(HirSourceSite::Insertion(_))
            ))
    );

    let HirExprKind::ArrayRepeat(repeat) = expression(&module, owners[4]).kind() else {
        panic!("missing repeat operands remain E11");
    };
    assert_synthetic_recovery_child(
        &module,
        owners[4],
        repeat.value(),
        0,
        HirExprSourceRole::RepeatValue,
    );
    assert_synthetic_recovery_child(
        &module,
        owners[4],
        repeat.length(),
        1,
        HirExprSourceRole::RepeatLength,
    );
    assert_eq!(
        expression(&module, owners[4]).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::RepeatValue,
        })
    );

    let recovery_diagnostics = module
        .diagnostics()
        .iter()
        .filter(|diagnostic| matches!(diagnostic, HirDiagnostic::Recovery(_)))
        .count();
    assert_eq!(recovery_diagnostics, 6);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test validates one complete arbitrary-width numeric-sequence payload and late-recovery scenario"
)]
fn numeric_sequence_retains_invalid_elements_late_suffixes_and_arbitrary_width_values() {
    let parsed = parsed_source(
        "numeric-sequence-matrix",
        &[
            "[1, 0x]".into(),
            "[1, 1__2]".into(),
            "[1, 2u16, 3]".into(),
            "[1, 2]".into(),
            "[0o17, 340282366920938463463374607431768211456]".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let HirExprKind::NumericBracketSequence(invalid) = expression(&module, owners[0]).kind() else {
        panic!("malformed compact element remains E10");
    };
    assert_eq!(invalid.elements().len(), 1);
    assert_eq!(
        invalid.recovery(),
        &HirNumericSequenceRecovery::InvalidElement {
            ordinal: 1,
            issue: HirIntegerIssue::MissingDigits,
        }
    );
    assert_eq!(
        expression(&module, owners[0]).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidNumericSequence)
    );
    let invalid_element = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[0],
                role: HirExprSourceRole::NumericElement { ordinal: 1 },
            },
        )
        .expect("invalid numeric element source");
    assert!(matches!(
        invalid_element.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));

    let HirExprKind::NumericBracketSequence(invalid_digit) = expression(&module, owners[1]).kind()
    else {
        panic!("invalid compact element remains E10");
    };
    assert_eq!(invalid_digit.elements().len(), 1);
    assert_eq!(
        invalid_digit.recovery(),
        &HirNumericSequenceRecovery::InvalidElement {
            ordinal: 1,
            issue: HirIntegerIssue::InvalidDigit,
        }
    );
    assert_eq!(
        expression(&module, owners[1]).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidNumericSequence)
    );
    let invalid_digit_element = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[1],
                role: HirExprSourceRole::NumericElement { ordinal: 1 },
            },
        )
        .expect("invalid-digit numeric element source");
    assert!(matches!(
        invalid_digit_element.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));

    let HirExprKind::NumericBracketSequence(late_suffix) = expression(&module, owners[2]).kind()
    else {
        panic!("late explicit suffix remains E10");
    };
    assert_eq!(late_suffix.common_suffix(), Some(HirIntegerSuffix::U16));
    assert_eq!(late_suffix.elements().len(), 3);
    assert!(matches!(
        module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: owners[2],
                    role: HirExprSourceRole::NumericCommonSuffix,
                },
            )
            .expect("late common suffix source")
            .presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));

    let HirExprKind::NumericBracketSequence(unsuffixed) = expression(&module, owners[3]).kind()
    else {
        panic!("unsuffixed compact sequence remains E10");
    };
    assert_eq!(unsuffixed.common_suffix(), None);
    assert_eq!(
        module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: owners[3],
                    role: HirExprSourceRole::NumericCommonSuffix,
                },
            )
            .expect("optional suffix query")
            .presence(),
        HirSourcePresence::AbsentOptional
    );

    let HirExprKind::NumericBracketSequence(wide) = expression(&module, owners[4]).kind() else {
        panic!("arbitrary-width compact sequence remains E10");
    };
    assert_eq!(wide.elements()[0].radix(), HirIntegerRadix::Octal);
    assert_eq!(wide.elements()[0].magnitude().limbs_le(), &[15]);
    assert_eq!(wide.elements()[1].radix(), HirIntegerRadix::Decimal);
    assert_eq!(wide.elements()[1].magnitude().limbs_le(), &[0, 0, 0, 0, 1]);

    assert_eq!(
        module
            .diagnostics()
            .iter()
            .filter(|diagnostic| matches!(diagnostic, HirDiagnostic::Recovery(_)))
            .count(),
        2
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test is one atomic rollback/retry matrix for numeric-sequence recovery diagnostics"
)]
fn numeric_sequence_rollback_and_relowering_publish_one_recovery_diagnostic() {
    let (initial, revised) = parsed_revisions("numeric-sequence-retry", "[1, 0x]");
    let initial_attached = attached_expressions(&initial).pop().unwrap();
    let revised_attached = attached_expressions(&revised).pop().unwrap();
    assert_eq!(initial_attached.id(), revised_attached.id());
    let key = module_key(&initial);
    let revised_key = module_key(&revised);
    assert_eq!(key.package(), revised_key.package());
    assert_eq!(key.path(), revised_key.path());
    assert_ne!(key.source(), revised_key.source());

    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut initial_transaction = stage(&database, &initial);
    let initial_scope = allocate_module_scope(&mut initial_transaction, &initial);
    let owner = initial_transaction
        .lower_attached_expression(&initial_attached, initial_scope)
        .expect("initial E10 recovery lowering");
    let initial_module = initial_transaction
        .finish(&mut database)
        .expect("initial recovered module publication")
        .into_module();
    assert_eq!(
        initial_module
            .diagnostics()
            .iter()
            .filter(|diagnostic| matches!(diagnostic, HirDiagnostic::Recovery(_)))
            .count(),
        1
    );

    let mut rejected = stage(&database, &revised);
    let revised_scope = allocate_module_scope(&mut rejected, &revised);
    assert_eq!(
        rejected
            .lower_attached_expression(&revised_attached, revised_scope)
            .expect("revised E10 recovery lowering"),
        owner
    );
    assert_eq!(
        rejected
            .diagnostics
            .iter()
            .filter(|diagnostic| matches!(diagnostic, HirDiagnostic::Recovery(_)))
            .count(),
        1
    );
    assert_eq!(
        rejected
            .lower_attached_expression(&revised_attached, revised_scope)
            .expect("same-transaction E10 reuse"),
        owner
    );
    assert_eq!(
        rejected
            .diagnostics
            .iter()
            .filter(|diagnostic| matches!(diagnostic, HirDiagnostic::Recovery(_)))
            .count(),
        1
    );
    let dead_scope = ScopeId::from_raw(RawHirId::new(
        rejected.snapshot_id().module(),
        NonZeroU32::MAX,
        HirIdKind::Scope,
    ));
    assert_eq!(
        rejected.lower_attached_expression(&revised_attached, dead_scope),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidArenaCommit
        ))
    );
    assert!(rejected.finish(&mut database).is_err());
    assert!(Arc::ptr_eq(
        &database
            .current(&key)
            .expect("initial module stays current"),
        &initial_module
    ));

    let mut retry = stage(&database, &revised);
    let retry_scope = allocate_module_scope(&mut retry, &revised);
    assert_eq!(
        retry
            .lower_attached_expression(&revised_attached, retry_scope)
            .expect("fresh E10 retry"),
        owner
    );
    let revised_module = retry
        .finish(&mut database)
        .expect("retried revision publication")
        .into_module();
    assert_eq!(
        revised_module
            .diagnostics()
            .iter()
            .filter(|diagnostic| matches!(diagnostic, HirDiagnostic::Recovery(_)))
            .count(),
        1
    );
    assert!(Arc::ptr_eq(
        &database
            .current(&revised_key)
            .expect("revised module is current"),
        &revised_module
    ));

    let invalid_source_start = |module: &HirModule, parsed: &ParsedSource| {
        let source = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner,
                    role: HirExprSourceRole::NumericElement { ordinal: 1 },
                },
            )
            .expect("invalid E10 element source");
        let HirSourcePresence::Present(HirSourceSite::Span(span)) = source.presence() else {
            panic!("invalid numeric element remains an authored span");
        };
        span.range().start()
    };
    assert!(
        invalid_source_start(&revised_module, &revised)
            > invalid_source_start(&initial_module, &initial)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test executes the complete composite source-manifest tamper matrix in one transactional fixture"
)]
fn composite_source_manifest_rejects_reordered_scoped_and_substituted_payloads() {
    let reordered = parsed_source("tuple-reordered", &["(1, 2)".into()]);
    let attached = attached_expressions(&reordered).pop().unwrap();
    let children = attached
        .children()
        .iter()
        .map(|child| {
            child
                .authored_semantic()
                .expect("tuple child projection")
                .expect("authored tuple child")
        })
        .collect::<Vec<_>>();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &reordered);
    let scope = allocate_module_scope(&mut transaction, &reordered);
    let child_ids = children
        .iter()
        .map(|child| {
            transaction
                .lower_attached_expression(child, scope)
                .expect("tuple child lowering")
        })
        .collect::<Vec<_>>();
    let manifest_kind = HirExprKind::Tuple(HirTupleExpr::new(child_ids.clone().into_boxed_slice()));
    stage_expression_with_manifest(
        &mut transaction,
        &reordered,
        &attached,
        scope,
        &manifest_kind,
        HirExprKind::Tuple(HirTupleExpr::new(child_ids.iter().rev().copied().collect())),
    );
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&reordered)).is_none());

    let scoped = parsed_source("tuple-scope-mismatch", &["(1, 2)".into()]);
    let attached = attached_expressions(&scoped).pop().unwrap();
    let children = attached
        .children()
        .iter()
        .map(|child| {
            child
                .authored_semantic()
                .expect("tuple child projection")
                .expect("authored tuple child")
        })
        .collect::<Vec<_>>();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &scoped);
    let scope = allocate_module_scope(&mut transaction, &scoped);
    let foreign_scope = {
        let module = transaction.snapshot_id().module();
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .scopes()
            .allocate_source(
                slots,
                children[1].id(),
                HirSourceSite::Span(children[1].whole_source_span()),
                HirScope::try_new(
                    module,
                    HirScopeKind::Block,
                    Some(scope),
                    HirScopeOwner::Module(module),
                    Box::new([]),
                    Box::new([]),
                )
                .expect("foreign child scope"),
            )
            .expect("foreign child scope allocation")
    };
    let first = transaction
        .lower_attached_expression(&children[0], scope)
        .expect("first tuple child");
    let second = transaction
        .lower_attached_expression(&children[1], foreign_scope)
        .expect("foreign-scope tuple child");
    let kind = HirExprKind::Tuple(HirTupleExpr::new(Box::new([first, second])));
    stage_expression_with_manifest(
        &mut transaction,
        &scoped,
        &attached,
        scope,
        &kind,
        kind.clone(),
    );
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleArenaSnapshot
        ))
    ));
    assert!(database.current(&module_key(&scoped)).is_none());

    let numeric = parsed_source("numeric-payload-substitution", &["[1, 2]".into()]);
    let attached = attached_expressions(&numeric).pop().unwrap();
    let ExpressionProjection::NumericBracketSequence(sequence) = attached.projection() else {
        panic!("numeric fixture remains E10");
    };
    let expected = project_numeric_sequence(sequence).expect("expected numeric payload");
    let substituted = HirNumericSequence::try_new(
        expected.elements().iter().rev().cloned().collect(),
        expected.common_suffix(),
        expected.recovery().clone(),
    )
    .expect("substituted numeric payload is internally valid");
    let manifest_kind = HirExprKind::NumericBracketSequence(expected);
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &numeric);
    let scope = allocate_module_scope(&mut transaction, &numeric);
    stage_expression_with_manifest(
        &mut transaction,
        &numeric,
        &attached,
        scope,
        &manifest_kind,
        HirExprKind::NumericBracketSequence(substituted),
    );
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&numeric)).is_none());

    let repeated = parsed_source("array-repeat-swapped", &["[value; length]".into()]);
    let attached = attached_expressions(&repeated).pop().unwrap();
    let children = attached
        .children()
        .iter()
        .map(|child| {
            child
                .authored_semantic()
                .expect("array-repeat child projection")
                .expect("authored array-repeat child")
        })
        .collect::<Vec<_>>();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &repeated);
    let scope = allocate_module_scope(&mut transaction, &repeated);
    let value = transaction
        .lower_attached_expression(&children[0], scope)
        .expect("repeat value");
    let length = transaction
        .lower_attached_expression(&children[1], scope)
        .expect("repeat length");
    let manifest_kind = HirExprKind::ArrayRepeat(HirArrayRepeatExpr::new(value, length));
    stage_expression_with_manifest(
        &mut transaction,
        &repeated,
        &attached,
        scope,
        &manifest_kind,
        HirExprKind::ArrayRepeat(HirArrayRepeatExpr::new(length, value)),
    );
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&repeated)).is_none());

    assert_expression_freeze_rejects("index-swapped", "target[index]", |transaction, root| {
        let (slots, arenas) = transaction.storage_mut();
        let candidate = {
            let root = arenas
                .expressions()
                .resolve_staged(slots, root)
                .expect("staged E34 root");
            let HirExprKind::PostfixBracket(postfix) = root.kind() else {
                panic!("fixture root must remain the ambiguous E34 postfix");
            };
            let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
                panic!("ordinary-index candidate must remain typed");
            };
            *index
        };
        let replacement = {
            let candidate = arenas
                .expressions()
                .resolve_staged(slots, candidate)
                .expect("staged E14 candidate");
            let HirExprKind::Index(index) = candidate.kind() else {
                panic!("ordinary interpretation must retain its E14 Index root");
            };
            HirExpr::try_new(
                candidate.scope(),
                HirExprKind::Index(HirIndexExpr::new(index.index(), index.target())),
                candidate.state().clone(),
            )
            .expect("same-module swapped Index is locally constructible")
        };
        arenas
            .expressions()
            .revise_finalized(slots, candidate, replacement)
            .expect("test-only E14 payload substitution");
    });
}

#[test]
fn attached_expression_source_queries_distinguish_presence_and_owner_poison() {
    let parsed = parsed_source("source-query-matrix", &["game::actor".into(), "@".into()]);
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let whole = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[0],
                role: HirExprSourceRole::Whole,
            },
        )
        .expect("path Whole source query");
    assert_eq!(whole.owner_status(), HirSourceOwnerStatus::Clean);
    assert_eq!(
        whole.presence(),
        HirSourcePresence::Present(&HirSourceSite::Span(attached[0].whole_source_span()))
    );

    let segment = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[0],
                role: HirExprSourceRole::PathSegment { ordinal: 0 },
            },
        )
        .expect("path segment source query");
    assert_eq!(segment.owner_status(), HirSourceOwnerStatus::Clean);
    assert!(matches!(
        segment.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));

    let root = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[0],
                role: HirExprSourceRole::PathRoot,
            },
        )
        .expect("implicit path root source query");
    assert_eq!(root.owner_status(), HirSourceOwnerStatus::Clean);
    assert_eq!(root.presence(), HirSourcePresence::AbsentOptional);

    let inapplicable = HirExprSourceRole::PlaceholderMarker;
    assert_eq!(
        module.source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[0],
                role: inapplicable,
            },
        ),
        Err(HirSourceQueryError::ExprRoleNotApplicable {
            owner: owners[0],
            role: inapplicable,
        })
    );

    let poisoned = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[1],
                role: HirExprSourceRole::EntityReference(HirIdRefSourcePart::Whole),
            },
        )
        .expect("known entity recovery source query");
    assert_eq!(poisoned.owner_status(), HirSourceOwnerStatus::Poisoned);
    assert!(matches!(
        poisoned.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));

    let foreign = parsed_source("source-query-foreign", &["game.actor".into()]);
    assert!(matches!(
        module.source_site(
            foreign.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[0],
                role: HirExprSourceRole::Whole,
            },
        ),
        Err(HirSourceQueryError::WrongSourceDocument { .. })
    ));
}

#[test]
fn known_entity_recovery_keeps_exact_leaf_poison() {
    let parsed = parsed_source("entity-recovery", &["@".into()]);
    let (module, owners, _) = lower_and_publish(&parsed);
    let value = expression(&module, owners[0]);
    assert_eq!(module.status(), HirModuleStatus::Recovered);
    assert!(matches!(
        value.kind(),
        HirExprKind::EntityReference(reference) if reference.recovery().is_some()
    ));
    assert_eq!(
        value.state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidId(HirIdRefIssue::Missing))
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test is the closed E35 generic-error and known-parent poison-propagation acceptance matrix"
)]
fn e35_generic_error_is_source_backed_and_known_parents_propagate_typed_poison() {
    let parsed = parsed_source(
        "generic-expression-recovery",
        &[
            ":".into(),
            "value : bad".into(),
            "left + :".into(),
            "start..:".into(),
        ],
    );
    let attached = attached_expressions(&parsed);
    assert!(matches!(
        attached[0].projection(),
        ExpressionProjection::Error
    ));
    assert!(matches!(
        attached[1].projection(),
        ExpressionProjection::Error
    ));
    assert!(attached[0].children().is_empty());
    let [wrapped_prefix] = attached[1].children() else {
        panic!("wrapped E35 recovery must retain one authored prefix");
    };
    assert!(matches!(
        wrapped_prefix
            .authored_semantic()
            .expect("wrapped E35 prefix access")
            .expect("wrapped E35 authored prefix")
            .projection(),
        ExpressionProjection::Path
    ));
    assert!(matches!(
        attached[2].projection(),
        ExpressionProjection::Binary { .. }
    ));
    assert!(matches!(
        attached[2].children()[1]
            .authored_semantic()
            .expect("E35 binary child attachment")
            .expect("authored E35 binary child")
            .projection(),
        ExpressionProjection::Error
    ));
    assert!(matches!(
        attached[3].projection(),
        ExpressionProjection::Range { .. }
    ));
    assert!(matches!(
        attached[3].children()[1]
            .authored_semantic()
            .expect("E35 range child attachment")
            .expect("authored E35 range child")
            .projection(),
        ExpressionProjection::Error
    ));

    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);
    for (owner, attached) in owners[..2].iter().copied().zip(&attached[..2]) {
        let payload = expression(&module, owner);
        assert!(matches!(
            payload.kind(),
            HirExprKind::Error(error)
                if error.issue() == HirGenericExprIssue::UnclassifiedSyntax
        ));
        assert_eq!(
            payload.state(),
            &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::Generic(HirGenericExprIssue::UnclassifiedSyntax),
            ))
        );
        let recovery = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner,
                    role: HirExprSourceRole::Recovery,
                },
            )
            .expect("E35 recovery source query");
        assert_eq!(recovery.owner_status(), HirSourceOwnerStatus::Poisoned);
        assert_eq!(
            recovery.presence(),
            HirSourcePresence::Present(&HirSourceSite::Span(
                attached
                    .component(ExpressionComponentRole::Recovery)
                    .expect("E35 recovery component"),
            ))
        );
        assert_eq!(
            module.source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner,
                    role: HirExprSourceRole::PlaceholderMarker,
                },
            ),
            Err(HirSourceQueryError::ExprRoleNotApplicable {
                owner,
                role: HirExprSourceRole::PlaceholderMarker,
            })
        );
        assert!(module.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            HirDiagnostic::Recovery(recovery)
                if recovery.owner() == SyntheticOwner::Expr(owner)
                    && recovery.primary_role()
                        == HirRecoveryPrimary::query(HirSourceQuery::Expr {
                            owner,
                            role: HirExprSourceRole::Recovery,
                        })
        )));
    }

    let binary_owner = owners[2];
    let HirExprKind::Binary(binary) = expression(&module, binary_owner).kind() else {
        panic!("known E22 parent must survive an authored E35 child");
    };
    assert_eq!(
        expression(&module, binary_owner).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::RightOperand,
            },
        ))
    );
    assert!(matches!(
        expression(&module, binary.right()).kind(),
        HirExprKind::Error(error)
            if error.issue() == HirGenericExprIssue::UnclassifiedSyntax
    ));
    assert!(module.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        HirDiagnostic::Recovery(recovery)
            if recovery.owner() == SyntheticOwner::Expr(binary_owner)
                && recovery.primary_role()
                    == HirRecoveryPrimary::query(HirSourceQuery::Expr {
                        owner: binary_owner,
                        role: HirExprSourceRole::RightOperand,
                    })
    )));

    let range_owner = owners[3];
    let HirExprKind::Range(range) = expression(&module, range_owner).kind() else {
        panic!("known E19 parent must survive an authored E35 child");
    };
    assert_eq!(
        expression(&module, range_owner).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::RangeEnd,
            },
        ))
    );
    assert!(matches!(
        expression(
            &module,
            range.end().expect("authored malformed range endpoint")
        )
        .kind(),
        HirExprKind::Error(error)
            if error.issue() == HirGenericExprIssue::UnclassifiedSyntax
    ));
    assert!(module.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        HirDiagnostic::Recovery(recovery)
            if recovery.owner() == SyntheticOwner::Expr(range_owner)
                && recovery.primary_role()
                    == HirRecoveryPrimary::query(HirSourceQuery::Expr {
                        owner: range_owner,
                        role: HirExprSourceRole::RangeEnd,
                    })
    )));
    assert_eq!(
        module
            .diagnostics()
            .iter()
            .filter(|diagnostic| matches!(diagnostic, HirDiagnostic::Recovery(_)))
            .count(),
        6
    );
}

#[test]
fn synthetic_expression_publishes_no_attached_source_manifest() {
    let parsed = parsed_source("synthetic-no-manifest", &[]);
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let site = HirSourceSite::Span(parsed.root_syntax().source_span().clone());
    let reservation = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .reserve_synthetic(
                slots,
                SyntheticKey::try_new(
                    SyntheticOwner::Scope(scope),
                    SyntheticRole::PostconditionResult,
                    0,
                )
                .expect("synthetic expression key"),
                site,
            )
            .expect("synthetic expression reservation")
    };
    let owner = reservation.id();
    let payload = HirExpr::try_new(scope, HirExprKind::Unit, HirPoisonState::Clean)
        .expect("synthetic expression payload");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .finalize(slots, reservation, payload)
            .expect("synthetic expression finalization");
    }
    let module = transaction
        .finish(&mut database)
        .expect("synthetic expression publication")
        .into_module();

    assert!(matches!(
        module.slots().resolve(owner).expect("synthetic slot").origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Scope(scope)
                && key.role() == SyntheticRole::PostconditionResult
    ));
    for role in [HirExprSourceRole::Whole, HirExprSourceRole::LiteralBody] {
        assert!(
            module
                .source_components()
                .requirement(&HirSourceQuery::Expr { owner, role })
                .is_none(),
            "synthetic expressions must not enter the attached manifest: {role:?}"
        );
    }
    let whole = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::Whole,
            },
        )
        .expect("synthetic Whole remains slot-owned");
    assert_eq!(whole.owner_status(), HirSourceOwnerStatus::Clean);
    assert!(matches!(whole.presence(), HirSourcePresence::Present(_)));
}

#[test]
fn repeated_attached_leaf_reuses_one_expression_identity() {
    let parsed = parsed_source("reuse", &[".Ready".into()]);
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let first = transaction
        .lower_attached_expression(&attached, scope)
        .unwrap();
    let second = transaction
        .lower_attached_expression(&attached, scope)
        .unwrap();
    assert_eq!(first, second);
    let module = transaction.finish(&mut database).unwrap().into_module();
    assert_eq!(
        module
            .arenas()
            .expressions()
            .try_iter(module.slots())
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn invalid_attached_identity_and_scope_publish_no_partial_expression_state() {
    let (initial, revised) = parsed_revisions("atomic-input-failures", "42");
    let initial_attached = attached_expressions(&initial).pop().unwrap();
    let revised_attached = revised
        .attached_expression(initial_attached.id())
        .expect("retained revised expression");
    assert_eq!(initial_attached.id(), revised_attached.id());
    let key = module_key(&initial);
    let revised_key = module_key(&revised);
    assert_eq!(key.package(), revised_key.package());
    assert_eq!(key.path(), revised_key.path());
    assert_ne!(key.source(), revised_key.source());

    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut accepted_transaction = stage(&database, &initial);
    let accepted_scope = allocate_module_scope(&mut accepted_transaction, &initial);
    let accepted_owner = accepted_transaction
        .lower_attached_expression(&initial_attached, accepted_scope)
        .expect("accepted expression lowering");
    let accepted = accepted_transaction
        .finish(&mut database)
        .expect("accepted expression publication")
        .into_module();
    let accepted_payload = expression(&accepted, accepted_owner).kind().clone();
    assert_accepted_expression_module_unchanged(&database, &key, &accepted, accepted_owner);

    let mut stale = stage(&database, &revised);
    let stale_diagnostic_count = stale.diagnostics.len();
    assert!(matches!(
        stale.lower_attached_expression(&initial_attached, accepted_scope),
        Err(HirLowerFailure::StaleSource { .. })
    ));
    assert_eq!(stale.diagnostics.len(), stale_diagnostic_count);
    assert!(stale.finish(&mut database).is_err());
    assert_accepted_expression_module_unchanged(&database, &key, &accepted, accepted_owner);

    let foreign = parsed_source("atomic-input-foreign", &["42".into()]);
    let foreign_attached = attached_expressions(&foreign).pop().unwrap();
    let mut foreign_source = stage(&database, &revised);
    let revised_owner = foreign_source
        .lower_attached_expression(&revised_attached, accepted_scope)
        .expect("valid prefix before foreign source rejection");
    assert_eq!(revised_owner, accepted_owner);
    let foreign_diagnostic_count = foreign_source.diagnostics.len();
    assert!(matches!(
        foreign_source
            .source_components()
            .stage_attached_expression(
                &foreign,
                revised_owner,
                &foreign_attached,
                &accepted_payload,
            ),
        Err(HirSourceCommitInvariantError::WrongSourceDocument { .. })
    ));
    assert_eq!(foreign_source.diagnostics.len(), foreign_diagnostic_count);
    assert!(foreign_source.finish(&mut database).is_err());
    assert_accepted_expression_module_unchanged(&database, &key, &accepted, accepted_owner);

    let mut dead_scope_transaction = stage(&database, &revised);
    let dead_scope = ScopeId::from_raw(RawHirId::new(
        dead_scope_transaction.snapshot_id().module(),
        NonZeroU32::MAX,
        HirIdKind::Scope,
    ));
    let dead_scope_diagnostic_count = dead_scope_transaction.diagnostics.len();
    assert_eq!(
        dead_scope_transaction.lower_attached_expression(&revised_attached, dead_scope),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidArenaCommit
        ))
    );
    assert_eq!(
        dead_scope_transaction.diagnostics.len(),
        dead_scope_diagnostic_count
    );
    assert!(dead_scope_transaction.finish(&mut database).is_err());
    assert_accepted_expression_module_unchanged(&database, &key, &accepted, accepted_owner);
}

#[test]
fn short_variant_name_limit_is_inclusive_and_one_over_is_atomic() {
    let exact_name = "a".repeat(HirLimit::NameBytes.maximum());
    let exact = parsed_source("name-exact", &[format!(".{exact_name}")]);
    let (module, owners, _) = lower_and_publish(&exact);
    assert!(matches!(
        expression(&module, owners[0]).kind(),
        HirExprKind::ShortVariant(HirShortVariantName::Resolved(name))
            if name.as_str().len() == HirLimit::NameBytes.maximum()
    ));

    let one_over_name = "a".repeat(HirLimit::NameBytes.maximum() + 1);
    let one_over = parsed_source("name-one-over", &[format!(".{one_over_name}")]);
    let attached = attached_expressions(&one_over).pop().unwrap();
    let database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &one_over);
    let scope = allocate_module_scope(&mut transaction, &one_over);
    let error = transaction
        .lower_attached_expression(&attached, scope)
        .unwrap_err();
    assert!(matches!(
        error,
        HirLowerFailure::Limit(error) if error.limit() == HirLimit::NameBytes
    ));
    assert!(database.current(&module_key(&one_over)).is_none());
}

#[test]
fn numeric_digit_limit_is_inclusive_and_one_over_publishes_nothing() {
    let exact_digits = "9".repeat(HirLimit::NumericDigitsPerLiteral.maximum());
    let exact = parsed_source("numeric-digits-exact", &[exact_digits]);
    let (module, owners, _) = lower_and_publish(&exact);
    assert_eq!(module.status(), HirModuleStatus::Clean);
    assert!(matches!(
        expression(&module, owners[0]).kind(),
        HirExprKind::Literal(HirLiteral::Integer(HirIntegerLiteral::Value { .. }))
    ));

    let one_over_digits = "9".repeat(HirLimit::NumericDigitsPerLiteral.maximum() + 1);
    let one_over = parsed_source("numeric-digits-one-over", &[one_over_digits]);
    let attached = attached_expressions(&one_over).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &one_over);
    let scope = allocate_module_scope(&mut transaction, &one_over);
    let diagnostic_count = transaction.diagnostics.len();
    assert!(matches!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Limit(error))
            if error.limit() == HirLimit::NumericDigitsPerLiteral
    ));
    assert_eq!(transaction.diagnostics.len(), diagnostic_count);
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&one_over)).is_none());
}

#[test]
fn numeric_sequence_element_and_total_digit_limits_are_inclusive_and_atomic() {
    let element_limit = HirLimit::NumericSequenceElements;
    let exact_elements_source = repeated_numeric_sequence("0", element_limit.maximum());
    let exact_elements = parsed_source("numeric-sequence-elements-exact", &[exact_elements_source]);
    let (module, owners, _) = lower_and_publish(&exact_elements);
    let HirExprKind::NumericBracketSequence(sequence) = expression(&module, owners[0]).kind()
    else {
        panic!("exact element boundary remains E10");
    };
    assert_eq!(sequence.elements().len(), element_limit.maximum());
    let last_ordinal = u32::try_from(element_limit.maximum() - 1).expect("limit fits u32");
    assert!(matches!(
        module
            .source_site(
                exact_elements.document().identity(),
                HirSourceQuery::Expr {
                    owner: owners[0],
                    role: HirExprSourceRole::NumericElement {
                        ordinal: last_ordinal,
                    },
                },
            )
            .expect("last exact numeric element source")
            .presence(),
        HirSourcePresence::Present(HirSourceSite::Span(_))
    ));

    let one_over_elements = parsed_source(
        "numeric-sequence-elements-one-over",
        &[repeated_numeric_sequence("0", element_limit.maximum() + 1)],
    );
    let attached = attached_expressions(&one_over_elements).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &one_over_elements);
    let scope = allocate_module_scope(&mut transaction, &one_over_elements);
    let diagnostic_count = transaction.diagnostics.len();
    assert!(matches!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Limit(error))
            if error.limit() == element_limit
                && error.observed() == element_limit.maximum() + 1
                && error.maximum() == element_limit.maximum()
    ));
    assert_eq!(transaction.diagnostics.len(), diagnostic_count);
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&one_over_elements)).is_none());

    let per_literal = HirLimit::NumericDigitsPerLiteral.maximum();
    let total_limit = HirLimit::NumericSequenceTotalDigits;
    assert_eq!(total_limit.maximum() % per_literal, 0);
    let exact_digit = format!("0x{}", "f".repeat(per_literal));
    let exact_total_source =
        repeated_numeric_sequence(&exact_digit, total_limit.maximum() / per_literal);
    let exact_total = parsed_source("numeric-sequence-total-digits-exact", &[exact_total_source]);
    let (module, owners, _) = lower_and_publish(&exact_total);
    let HirExprKind::NumericBracketSequence(sequence) = expression(&module, owners[0]).kind()
    else {
        panic!("exact total-digit boundary remains E10");
    };
    assert_eq!(
        sequence.elements().len(),
        total_limit.maximum() / per_literal
    );
    assert!(
        sequence
            .elements()
            .iter()
            .all(|element| element.radix() == HirIntegerRadix::Hexadecimal)
    );

    let mut one_over_total_source =
        repeated_numeric_sequence(&exact_digit, total_limit.maximum() / per_literal);
    assert_eq!(one_over_total_source.pop(), Some(']'));
    one_over_total_source.push_str(",0]");
    let one_over_total = parsed_source(
        "numeric-sequence-total-digits-one-over",
        &[one_over_total_source],
    );
    let attached = attached_expressions(&one_over_total).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &one_over_total);
    let scope = allocate_module_scope(&mut transaction, &one_over_total);
    let diagnostic_count = transaction.diagnostics.len();
    assert!(matches!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Limit(error))
            if error.limit() == total_limit
                && error.observed() == total_limit.maximum() + 1
                && error.maximum() == total_limit.maximum()
    ));
    assert_eq!(transaction.diagnostics.len(), diagnostic_count);
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&one_over_total)).is_none());
}

#[test]
fn path_segment_limit_is_inclusive_and_one_over_publishes_nothing() {
    let exact_path = std::iter::repeat_n("segment", HirLimit::PathSegments.maximum())
        .collect::<Vec<_>>()
        .join("::");
    let exact = parsed_source("path-segments-exact", &[exact_path]);
    let (module, owners, _) = lower_and_publish(&exact);
    assert_eq!(module.status(), HirModuleStatus::Clean);
    assert!(matches!(
        expression(&module, owners[0]).kind(),
        HirExprKind::Path(HirPathValue::Resolved(path))
            if path.segments().len() == HirLimit::PathSegments.maximum()
    ));

    let one_over_path = std::iter::repeat_n("segment", HirLimit::PathSegments.maximum() + 1)
        .collect::<Vec<_>>()
        .join("::");
    let one_over = parsed_source("path-segments-one-over", &[one_over_path]);
    let attached = attached_expressions(&one_over).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &one_over);
    let scope = allocate_module_scope(&mut transaction, &one_over);
    let diagnostic_count = transaction.diagnostics.len();
    assert!(matches!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Limit(error)) if error.limit() == HirLimit::PathSegments
    ));
    assert_eq!(transaction.diagnostics.len(), diagnostic_count);
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&one_over)).is_none());
}

#[test]
fn registry_segment_limit_is_inclusive_and_one_over_is_atomic() {
    let exact_path = format!(
        "'custom.{}",
        std::iter::repeat_n("a", HirLimit::RegistrySegments.maximum())
            .collect::<Vec<_>>()
            .join(".")
    );
    let exact = parsed_source("registry-exact", &[exact_path]);
    let (module, owners, _) = lower_and_publish(&exact);
    assert!(matches!(
        expression(&module, owners[0]).kind(),
        HirExprKind::LifetimePath(HirLifetimePathValue::Resolved(path))
            if path.segments().len() == HirLimit::RegistrySegments.maximum()
    ));

    let one_over_path = format!(
        "'custom.{}",
        std::iter::repeat_n("a", HirLimit::RegistrySegments.maximum() + 1)
            .collect::<Vec<_>>()
            .join(".")
    );
    let one_over = parsed_source("registry-one-over", &[one_over_path]);
    let attached = attached_expressions(&one_over).pop().unwrap();
    let database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &one_over);
    let scope = allocate_module_scope(&mut transaction, &one_over);
    let error = transaction
        .lower_attached_expression(&attached, scope)
        .unwrap_err();
    assert!(matches!(
        error,
        HirLowerFailure::Limit(error) if error.limit() == HirLimit::RegistrySegments
    ));
    assert!(database.current(&module_key(&one_over)).is_none());
}
