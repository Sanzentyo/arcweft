use std::collections::BTreeSet;
use std::fmt::Write as _;

use arcweft_lang_syntax::expressions::{ExpressionComponentRole, ExpressionProjection};

use super::*;
use crate::expr::HirMatchRecoveryIssue;
use crate::identity::CaptureId;
use crate::pattern::{HirPatternBinding, HirPatternKind};
use crate::scope::{CaptureAccess, HirCapture, LocalLookup};
use crate::source_index::HirMatchArmSourcePart;
use crate::type_ref::HirTypeKind;

fn typed_if(module: &HirModule, owner: ExprId) -> &crate::expr::HirIfExpr {
    let HirExprKind::If(expression) = expression(module, owner).kind() else {
        panic!("E30 owner must retain HirExprKind::If");
    };
    expression
}

fn typed_if_let(module: &HirModule, owner: ExprId) -> &crate::expr::HirIfLetExpr {
    let HirExprKind::IfLet(expression) = expression(module, owner).kind() else {
        panic!("E31 owner must retain HirExprKind::IfLet");
    };
    expression
}

fn typed_match(module: &HirModule, owner: ExprId) -> &crate::expr::HirMatchExpr {
    let HirExprKind::Match(expression) = expression(module, owner).kind() else {
        panic!("E32 owner must retain HirExprKind::Match");
    };
    expression
}

fn typed_closure(module: &HirModule, owner: ExprId) -> &crate::expr::HirClosureExpr {
    let HirExprKind::Closure(expression) = expression(module, owner).kind() else {
        panic!("Closure owner must retain HirExprKind::Closure");
    };
    expression
}

fn staged_closure_captures(
    transaction: &mut StagedHirModuleTransaction<'_>,
    root: ExprId,
) -> (ExprId, Box<[CaptureId]>) {
    let (slots, arenas) = transaction.storage_mut();
    let root = arenas.expressions().resolve_staged(slots, root).unwrap();
    let HirExprKind::ComputationBlock(block) = root.kind() else {
        panic!("capture fixture root must remain a computation block");
    };
    let closure_id = block.tail();
    let closure = arenas
        .expressions()
        .resolve_staged(slots, closure_id)
        .unwrap();
    let HirExprKind::Closure(closure) = closure.kind() else {
        panic!("capture fixture tail must remain a closure");
    };
    (closure_id, closure.captures().into())
}

fn revise_staged_closure_captures(
    transaction: &mut StagedHirModuleTransaction<'_>,
    closure_id: ExprId,
    captures: Box<[CaptureId]>,
) {
    let replacement = {
        let (slots, arenas) = transaction.storage_mut();
        let retained = arenas
            .expressions()
            .resolve_staged(slots, closure_id)
            .unwrap()
            .clone();
        let HirExprKind::Closure(closure) = retained.kind() else {
            panic!("capture graph owner must remain a closure");
        };
        HirExpr::try_new(
            retained.scope(),
            HirExprKind::Closure(HirClosureExpr::new(
                closure.scope(),
                closure.parameters().into(),
                closure.result_type(),
                closure.body(),
                captures,
            )),
            retained.state().clone(),
        )
        .expect("same-module closure capture-list tamper")
    };
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .expressions()
        .revise_finalized(slots, closure_id, replacement)
        .expect("test-only closure capture-list substitution");
}

#[derive(Clone, Copy)]
enum CaptureGraphTamper {
    WrongOwner,
    Reordered,
    Orphan,
    DuplicateLocal,
}

fn capture(module: &HirModule, owner: CaptureId) -> &HirCapture {
    module
        .resolve_capture(owner)
        .expect("published closure capture")
}

fn captured_local_name<'a>(module: &'a HirModule, capture: &HirCapture) -> &'a str {
    module
        .resolve_local(capture.local())
        .expect("captured Local")
        .name()
        .as_str()
}

fn assert_capture_slot(module: &HirModule, closure: ExprId, capture_id: CaptureId, ordinal: u32) {
    let capture = capture(module, capture_id);
    let metadata = module.metadata(capture_id).expect("capture slot metadata");
    assert!(matches!(
        metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Expr(closure)
                && key.role() == SyntheticRole::ClosureCapture
                && key.ordinal() == ordinal
    ));
    assert!(matches!(
        metadata.source_site(),
        HirSourceSite::Insertion(insertion)
            if insertion.source_identity() == capture.first_use().source()
                && insertion.offset() == capture.first_use().range().start()
    ));
}

fn capture_limit_fixture(count: usize) -> String {
    let mut source = String::from("result {");
    for ordinal in 0..count {
        write!(source, " let capture_{ordinal} = 0;").expect("String writes are infallible");
    }
    source.push_str(" || {");
    for ordinal in 0..count {
        write!(source, " capture_{ordinal};").expect("String writes are infallible");
    }
    source.push_str(" () } }");
    source
}

fn capture_arena_limit_fixture(count: usize) -> String {
    const CAPTURES_PER_CLOSURE: usize = 64;

    assert!(count > 0, "Capture arena fixtures require one capture");
    let capture_width = count.min(CAPTURES_PER_CLOSURE);
    let closure_count = count.div_ceil(capture_width);
    let closures_per_chunk = HirLimit::SyntheticDescendantsPerOwner.maximum();
    let chunk_count = closure_count.div_ceil(closures_per_chunk);
    let mut source = String::from("result { let (");
    for ordinal in 0..capture_width {
        if ordinal != 0 {
            source.push_str(", ");
        }
        write!(source, "capture_{ordinal}").expect("String writes are infallible");
    }
    source.push_str(") = (");
    for ordinal in 0..capture_width {
        if ordinal != 0 {
            source.push_str(", ");
        }
        source.push('0');
    }
    source.push_str(");");
    if chunk_count > 1 {
        source.push_str(" (");
    }
    for chunk in 0..chunk_count {
        if chunk != 0 {
            source.push_str(", ");
        }
        let first_closure = chunk * closures_per_chunk;
        let end_closure = (first_closure + closures_per_chunk).min(closure_count);
        if end_closure - first_closure > 1 {
            source.push('(');
        }
        for closure in first_closure..end_closure {
            if closure != first_closure {
                source.push_str(", ");
            }
            source.push_str("|| { ");
            let first_capture = closure * capture_width;
            let closure_capture_count = (count - first_capture).min(capture_width);
            if closure_capture_count != 1 {
                source.push('(');
            }
            for ordinal in 0..closure_capture_count {
                if ordinal != 0 {
                    source.push_str(", ");
                }
                write!(source, "capture_{ordinal}").expect("String writes are infallible");
            }
            if closure_capture_count != 1 {
                source.push(')');
            }
            source.push_str(" }");
        }
        if end_closure - first_closure > 1 {
            source.push(')');
        }
    }
    if chunk_count > 1 {
        source.push(')');
    }
    source.push_str(" }");
    source
}

fn repeated_capture_fixture(count: usize) -> String {
    let mut source = String::from("result { let outer = 0; || {");
    for _ in 0..count {
        source.push_str(" outer;");
    }
    source.push_str(" () } }");
    source
}

fn typed_computation_block(
    module: &HirModule,
    owner: ExprId,
) -> &crate::expr::HirComputationBlockExpr {
    let HirExprKind::ComputationBlock(expression) = expression(module, owner).kind() else {
        panic!("E28 owner must retain HirExprKind::ComputationBlock");
    };
    expression
}

fn typed_named_block(module: &HirModule, owner: ExprId) -> &crate::expr::HirNamedBlockExpr {
    let HirExprKind::NamedBlock(expression) = expression(module, owner).kind() else {
        panic!("E29 owner must retain HirExprKind::NamedBlock");
    };
    expression
}

#[test]
fn destructuring_binds_left_to_right_after_initializer() {
    let parsed = parsed_source(
        "destructuring-binding-point",
        &[concat!(
            "result { ",
            "let (a, {left: b, right: (c, d), ..rest}, e (f, g)) = source_value; ",
            "(a, b, c, d, rest, e, f, g) ",
            "}"
        )
        .into()],
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "unexpected destructuring diagnostics: {:#?}",
        parsed.diagnostics()
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let block = typed_computation_block(&module, owners[0]);
    let [statement_id] = block.statements() else {
        panic!("destructuring fixture must retain one Let statement");
    };
    let statement = module
        .resolve_stmt(*statement_id)
        .expect("destructuring Let");
    let HirStmtKind::Let {
        pattern,
        initializer,
        locals,
        ..
    } = statement.kind()
    else {
        panic!("destructuring fixture statement must remain a Let");
    };
    assert!(matches!(
        module.resolve_pattern(*pattern).unwrap().kind(),
        HirPatternKind::Tuple { .. }
    ));

    let expected_names = ["a", "b", "c", "d", "rest", "e", "f", "g"];
    assert_eq!(locals.len(), expected_names.len());
    for (local_id, expected_name) in locals.iter().zip(expected_names) {
        let local = module.resolve_local(*local_id).expect("destructured Local");
        assert_eq!(local.scope(), block.scope());
        assert_eq!(local.name().as_str(), expected_name);
        assert_eq!(local.generation(), LocalGeneration::FIRST);
        assert!(!local.is_poisoned());
    }
    assert!(
        locals
            .windows(2)
            .all(|pair| pair[0].raw().slot() < pair[1].raw().slot())
    );

    let statement_end = match module.metadata(*statement_id).unwrap().source_site() {
        HirSourceSite::Span(span) => span.range().end(),
        HirSourceSite::Insertion(_) => panic!("authored Let must own a source span"),
    };
    let initializer_end = match module.metadata(*initializer).unwrap().source_site() {
        HirSourceSite::Span(span) => span.range().end(),
        HirSourceSite::Insertion(_) => panic!("authored initializer must own a source span"),
    };
    assert!(initializer_end < statement_end);
    let at_binding_point = parsed
        .document()
        .span(SourceRange::new(statement_end, statement_end))
        .expect("statement-end lookup point");
    let after_binding_point = parsed
        .document()
        .span(SourceRange::new(statement_end + 1, statement_end + 1))
        .expect("post-statement lookup point");
    for local_id in locals {
        let local = module.resolve_local(*local_id).unwrap();
        assert_eq!(
            module.lookup_local(block.scope(), local.name(), at_binding_point.clone()),
            Ok(LocalLookup::NotFound),
            "{} became visible before the complete Let statement",
            local.name().as_str()
        );
        assert_eq!(
            module.lookup_local(block.scope(), local.name(), after_binding_point.clone()),
            Ok(LocalLookup::Found(*local_id)),
            "{} did not share the post-statement binding point",
            local.name().as_str()
        );
    }
}

#[test]
fn duplicate_pattern_names_poison_all_duplicate_bindings() {
    let parsed = parsed_source(
        "duplicate-pattern-bindings",
        &["result { let (x, x) = pair; Point { x } }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let block = typed_computation_block(&module, owners[0]);
    let [statement_id] = block.statements() else {
        panic!("duplicate fixture must retain one Let statement");
    };
    let statement = module.resolve_stmt(*statement_id).expect("duplicate Let");
    let HirStmtKind::Let {
        pattern, locals, ..
    } = statement.kind()
    else {
        panic!("duplicate fixture statement must remain a Let");
    };
    let HirPatternKind::Tuple { elements } = module.resolve_pattern(*pattern).unwrap().kind()
    else {
        panic!("duplicate fixture must retain its tuple Pattern");
    };
    let [first_pattern, duplicate_pattern] = elements.as_ref() else {
        panic!("duplicate tuple must retain both occurrence PatternIds");
    };
    assert_ne!(first_pattern, duplicate_pattern);
    let [first_local, duplicate_local] = locals.as_ref() else {
        panic!("duplicate tuple must retain both LocalIds");
    };
    for (pattern_id, local_id) in [
        (*first_pattern, *first_local),
        (*duplicate_pattern, *duplicate_local),
    ] {
        let HirPatternKind::Binding(HirPatternBinding::Bound { local, .. }) =
            module.resolve_pattern(pattern_id).unwrap().kind()
        else {
            panic!("duplicate occurrence must remain a binding Pattern");
        };
        assert_eq!(*local, local_id);
    }

    let first = module.resolve_local(*first_local).unwrap();
    let duplicate = module.resolve_local(*duplicate_local).unwrap();
    assert_eq!(first.generation(), LocalGeneration::FIRST);
    assert_eq!(duplicate.generation(), LocalGeneration::FIRST);
    assert!(!first.is_poisoned());
    assert!(duplicate.is_poisoned());
    assert_eq!(first.scope(), duplicate.scope());
    assert_eq!(
        module
            .diagnostics()
            .iter()
            .filter(|diagnostic| matches!(
                diagnostic,
                HirDiagnostic::Recovery(recovery)
                    if recovery.owner() == SyntheticOwner::Local(*duplicate_local)
            ))
            .count(),
        1
    );

    let HirExprKind::Record(tail) = expression(&module, block.tail()).kind() else {
        panic!("duplicate fixture tail must remain a Record shorthand");
    };
    let [field] = tail.fields() else {
        panic!("duplicate fixture tail must retain one field");
    };
    assert_eq!(field.local(), Some(*first_local));
    let use_start = parsed.document().text().rfind('x').unwrap();
    let use_span = parsed
        .document()
        .span(SourceRange::new(use_start, use_start + 1))
        .unwrap();
    assert_eq!(
        module.lookup_local(first.scope(), first.name(), use_span),
        Ok(LocalLookup::Found(*first_local))
    );
}

#[test]
fn poisoned_pattern_does_not_leak_names() {
    let parsed = parsed_source(
        "poisoned-pattern-visibility",
        &["result { let .Some((leaked, _)) = source; leaked }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let block = typed_computation_block(&module, owners[0]);
    let [statement_id] = block.statements() else {
        panic!("poisoned fixture must retain one Let statement");
    };
    let statement = module.resolve_stmt(*statement_id).expect("poisoned Let");
    assert!(statement.state().is_poisoned());
    let HirStmtKind::Let {
        pattern, locals, ..
    } = statement.kind()
    else {
        panic!("poisoned fixture statement must remain a Let");
    };
    assert!(matches!(
        module.resolve_pattern(*pattern).unwrap().kind(),
        HirPatternKind::Variant(_)
    ));
    let [local_id] = locals.as_ref() else {
        panic!("valid nested binding remains queryable as one poisoned Local");
    };
    let local = module.resolve_local(*local_id).unwrap();
    assert_eq!(local.name().as_str(), "leaked");
    assert!(local.is_poisoned());

    let use_start = parsed.document().text().rfind("leaked").unwrap();
    let use_span = parsed
        .document()
        .span(SourceRange::new(use_start, use_start + "leaked".len()))
        .unwrap();
    assert_eq!(
        module.lookup_local(block.scope(), local.name(), use_span),
        Ok(LocalLookup::AmbiguousPoisoned(Box::new([*local_id])))
    );
    assert!(
        matches!(
            expression(&module, block.tail()).kind(),
            HirExprKind::Path(_)
        ),
        "the authored use remains a typed Path without selecting a poisoned Local"
    );
}

#[test]
fn sequential_shadowing_increments_local_generation() {
    let parsed = parsed_source(
        "sequential-shadowing",
        &[concat!(
            "result { ",
            "let x = 1; Point { x }; ",
            "let x = 2; Point { x }; ",
            "let x = 3; Point { x } ",
            "}"
        )
        .into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let block = typed_computation_block(&module, owners[0]);
    let [first_let, first_use, second_let, second_use, third_let] = block.statements() else {
        panic!("shadow fixture must retain three Lets and two intermediate uses");
    };
    let local_from_let = |statement_id| {
        let statement = module.resolve_stmt(statement_id).expect("shadowing Let");
        let HirStmtKind::Let { locals, .. } = statement.kind() else {
            panic!("shadowing binding must remain a Let");
        };
        let [local] = locals.as_ref() else {
            panic!("shadowing Let must retain one Local");
        };
        *local
    };
    let locals = [
        local_from_let(*first_let),
        local_from_let(*second_let),
        local_from_let(*third_let),
    ];
    for (index, local_id) in locals.iter().enumerate() {
        let local = module.resolve_local(*local_id).unwrap();
        assert_eq!(local.name().as_str(), "x");
        assert_eq!(local.scope(), block.scope());
        assert_eq!(local.generation().get(), u32::try_from(index + 1).unwrap());
        assert!(!local.is_poisoned());
    }

    let record_local = |statement_id| {
        let statement = module.resolve_stmt(statement_id).expect("shadowing use");
        let HirStmtKind::Expression { expression: owner } = statement.kind() else {
            panic!("intermediate shadowing use must remain an expression statement");
        };
        let HirExprKind::Record(record) = expression(&module, *owner).kind() else {
            panic!("shadowing use must remain a Record shorthand");
        };
        let [field] = record.fields() else {
            panic!("shadowing Record must retain one field");
        };
        field.local().expect("shadowing shorthand Local")
    };
    assert_eq!(record_local(*first_use), locals[0]);
    assert_eq!(record_local(*second_use), locals[1]);
    let HirExprKind::Record(tail) = expression(&module, block.tail()).kind() else {
        panic!("final shadowing use must remain the block tail Record");
    };
    let [field] = tail.fields() else {
        panic!("final shadowing Record must retain one field");
    };
    assert_eq!(field.local(), Some(locals[2]));
}

#[test]
fn closure_lowers_parameters_result_and_body_into_one_lexical_owner() {
    let parsed = parsed_source(
        "closure-owner",
        &["|item: Label, fallback| -> Text { item.text }".into()],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let root = expression(&module, owner);
    let closure = typed_closure(&module, owner);
    assert_eq!(closure.parameters().len(), 2);
    assert!(closure.parameters()[0].ty().is_some());
    assert!(closure.parameters()[1].ty().is_none());
    assert!(closure.result_type().is_some());
    assert!(closure.captures().is_empty());

    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), closure.scope())
        .expect("closure lexical scope");
    assert_eq!(scope.kind(), HirScopeKind::Closure);
    assert_eq!(scope.parent(), Some(root.scope()));
    assert_eq!(scope.owner(), &HirScopeOwner::Expr(owner));
    assert_eq!(scope.locals().len(), 2);
    assert!(scope.locals().iter().all(|local| {
        module
            .arenas()
            .locals()
            .resolve(module.slots(), *local)
            .is_ok_and(|local| {
                local.scope() == closure.scope() && local.kind() == HirLocalKind::ClosureParameter
            })
    }));
    assert_eq!(expression(&module, closure.body()).scope(), closure.scope());

    for role in [
        HirExprSourceRole::ClosureParameter {
            parameter: 0,
            part: crate::source_index::HirClosureParameterSourcePart::Pattern,
        },
        HirExprSourceRole::ClosureParameter {
            parameter: 0,
            part: crate::source_index::HirClosureParameterSourcePart::Type,
        },
        HirExprSourceRole::ReturnType,
        HirExprSourceRole::Body,
    ] {
        let source = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr { owner, role },
            )
            .expect("closure source query");
        assert_eq!(source.owner_status(), HirSourceOwnerStatus::Clean);
        assert!(matches!(source.presence(), HirSourcePresence::Present(_)));
    }
    assert_eq!(attached[0].closure_parameters().len(), 2);
}

#[test]
fn closure_capture_order_is_first_use_then_local_id() {
    let parsed = parsed_source(
        "closure-capture-order",
        &["result { let left = 1; let right = 2; || right + left + right }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let closure_id = typed_computation_block(&module, owners[0]).tail();
    let closure = typed_closure(&module, closure_id);
    let [right_id, left_id] = closure.captures() else {
        panic!("closure must retain exactly two unique captures");
    };
    assert_eq!(module.captures().count(), 2);

    let right = capture(&module, *right_id);
    let left = capture(&module, *left_id);
    assert_eq!(captured_local_name(&module, right), "right");
    assert_eq!(captured_local_name(&module, left), "left");
    assert_eq!(right.access(), CaptureAccess::Read);
    assert_eq!(left.access(), CaptureAccess::Read);

    let source = parsed.document().text();
    let body_start = source.find("|| right").expect("closure body source");
    let right_start = body_start + "|| ".len();
    let left_start = source[right_start + "right".len()..]
        .find("left")
        .map(|offset| right_start + "right".len() + offset)
        .expect("left capture use");
    assert_eq!(
        right.first_use().range(),
        SourceRange::new(right_start, right_start + "right".len())
    );
    assert_eq!(
        left.first_use().range(),
        SourceRange::new(left_start, left_start + "left".len())
    );
    assert!(right.first_use().range().start() < left.first_use().range().start());
    assert_capture_slot(&module, closure_id, *right_id, 0);
    assert_capture_slot(&module, closure_id, *left_id, 1);
}

#[test]
fn mutable_binding_and_mutable_reference_remain_distinct() {
    let parsed = parsed_source(
        "mutable-binding-versus-reference",
        &[
            "result { let mut x = 0; let r: &mut Int = seed; || { x = x + 1; *r = x; (x, r) } }"
                .into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let outer = typed_computation_block(&module, owners[0]);
    let outer_scope = module
        .resolve_scope(outer.scope())
        .expect("mutable-binding fixture scope");
    let mut x = None;
    let mut r = None;
    for local_id in outer_scope.locals() {
        let local = module.resolve_local(*local_id).unwrap();
        match local.name().as_str() {
            "x" => x = Some((*local_id, local)),
            "r" => r = Some((*local_id, local)),
            name => panic!("unexpected mutable-binding fixture Local {name}"),
        }
    }
    let (x_id, x) = x.expect("mutable x Local");
    let (r_id, r) = r.expect("mutable-reference r Local");
    assert!(x.is_mutable_binding());
    assert!(!r.is_mutable_binding());
    assert!(x.annotation().is_none());
    let r_annotation = r.annotation().expect("r reference annotation");
    assert!(matches!(
        module.resolve_type(r_annotation).unwrap().kind(),
        HirTypeKind::Reference(reference) if reference.kind() == HirBorrowKind::Mutable
    ));

    let closure_id = outer.tail();
    let closure = typed_closure(&module, closure_id);
    let [x_capture, r_capture] = closure.captures() else {
        panic!("mutable-binding fixture must retain x and r exactly once");
    };
    let x_capture = capture(&module, *x_capture);
    let r_capture = capture(&module, *r_capture);
    assert_eq!(x_capture.local(), x_id);
    assert_eq!(x_capture.access(), CaptureAccess::Reassign);
    assert_eq!(r_capture.local(), r_id);
    assert_eq!(r_capture.access(), CaptureAccess::Read);
    assert_capture_slot(&module, closure_id, closure.captures()[0], 0);
    assert_capture_slot(&module, closure_id, closure.captures()[1], 1);
}

#[test]
fn closure_reassignment_joins_access_without_losing_first_use() {
    let parsed = parsed_source(
        "closure-capture-reassign",
        &["result { let mut outer = 0; || { outer; outer = outer + 1; outer } }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let closure_id = typed_computation_block(&module, owners[0]).tail();
    let closure = typed_closure(&module, closure_id);
    let [capture_id] = closure.captures() else {
        panic!("repeated reassigned Local must retain one capture");
    };
    let retained = capture(&module, *capture_id);
    assert_eq!(captured_local_name(&module, retained), "outer");
    assert_eq!(retained.access(), CaptureAccess::Reassign);
    let body_start = parsed
        .document()
        .text()
        .find("|| { outer")
        .expect("closure block source");
    let first_use = body_start + "|| { ".len();
    assert_eq!(
        retained.first_use().range(),
        SourceRange::new(first_use, first_use + "outer".len())
    );
    assert_capture_slot(&module, closure_id, *capture_id, 0);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test validates one complete closure shadowing and capture-isolation scenario"
)]
fn closure_parameter_and_inner_shadow_prevent_capture() {
    let parsed = parsed_source(
        "closure-capture-shadow",
        &[
            "result { let value = 1; || { let value = 2; value } }".into(),
            "result { let value = 1; || { let value = value; value } }".into(),
            "result { let value = 1; |value| { Point { value }; let value = 2; Point { value } } }"
                .into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let shadowed_id = typed_computation_block(&module, owners[0]).tail();
    assert!(typed_closure(&module, shadowed_id).captures().is_empty());

    let pre_binding_id = typed_computation_block(&module, owners[1]).tail();
    let [capture_id] = typed_closure(&module, pre_binding_id).captures() else {
        panic!("pre-binding initializer must capture the visible outer generation");
    };
    let retained = capture(&module, *capture_id);
    assert_eq!(captured_local_name(&module, retained), "value");
    let initializer_start = parsed
        .document()
        .text()
        .find("let value = value")
        .expect("pre-binding initializer source")
        + "let value = ".len();
    assert_eq!(retained.first_use().range().start(), initializer_start);
    assert_capture_slot(&module, pre_binding_id, *capture_id, 0);

    let parameter_id = typed_computation_block(&module, owners[2]).tail();
    let parameter_closure = typed_closure(&module, parameter_id);
    assert!(
        parameter_closure.captures().is_empty(),
        "a closure parameter must hide the same-named outer Local generation"
    );
    let outer_scope = module
        .resolve_scope(typed_computation_block(&module, owners[2]).scope())
        .expect("outer computation scope");
    let parameter_scope = module
        .resolve_scope(parameter_closure.scope())
        .expect("closure parameter scope");
    let HirExprKind::Block(parameter_body) = expression(&module, parameter_closure.body()).kind()
    else {
        panic!("combined parameter/inner-shadow fixture must retain a Block body");
    };
    let inner_scope = module
        .resolve_scope(parameter_body.scope())
        .expect("inner binding scope");
    let [outer_local] = outer_scope.locals() else {
        panic!("combined fixture must retain one outer Local");
    };
    let [parameter_local] = parameter_scope.locals() else {
        panic!("combined fixture must retain one closure-parameter Local");
    };
    let [inner_local] = inner_scope.locals() else {
        panic!("combined fixture must retain one inner-let Local");
    };
    assert_ne!(outer_local, parameter_local);
    assert_ne!(parameter_local, inner_local);
    assert_ne!(outer_local, inner_local);
    for local in [*outer_local, *parameter_local, *inner_local] {
        assert_eq!(
            module.resolve_local(local).unwrap().name().as_str(),
            "value"
        );
    }
    assert_eq!(
        module.resolve_local(*outer_local).unwrap().kind(),
        crate::scope::HirLocalKind::LetBinding
    );
    assert_eq!(
        module.resolve_local(*parameter_local).unwrap().kind(),
        crate::scope::HirLocalKind::ClosureParameter
    );
    assert_eq!(
        module.resolve_local(*inner_local).unwrap().kind(),
        crate::scope::HirLocalKind::LetBinding
    );

    let [parameter_use, inner_let] = parameter_body.statements() else {
        panic!("combined shadow fixture must retain a parameter use and inner Let");
    };
    let parameter_use = module
        .arenas()
        .statements()
        .resolve(module.slots(), *parameter_use)
        .expect("combined shadow parameter-use statement");
    let HirStmtKind::Expression {
        expression: parameter_record,
    } = parameter_use.kind()
    else {
        panic!("combined shadow parameter use must remain an expression statement");
    };
    let HirExprKind::Record(parameter_record) = expression(&module, *parameter_record).kind()
    else {
        panic!("combined shadow parameter use must remain a Record");
    };
    let [parameter_field] = parameter_record.fields() else {
        panic!("combined shadow parameter Record must retain one shorthand field");
    };
    assert_eq!(parameter_field.local(), Some(*parameter_local));

    let statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), *inner_let)
        .expect("combined shadow inner Let");
    let HirStmtKind::Let { locals, .. } = statement.kind() else {
        panic!("combined shadow statement must remain a Let");
    };
    assert_eq!(locals.as_ref(), &[*inner_local]);

    let HirExprKind::Record(tail) = expression(&module, parameter_body.tail()).kind() else {
        panic!("combined shadow tail must remain a Record");
    };
    let [tail_field] = tail.fields() else {
        panic!("combined shadow tail must retain one shorthand field");
    };
    assert_eq!(tail_field.local(), Some(*inner_local));
}

#[test]
fn let_initializer_uses_pre_binding_scope() {
    let parsed = parsed_source(
        "let-initializer-pre-binding-scope",
        &["result { let value = 1; || { let value = value; Point { value } } }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let block = typed_computation_block(&module, owners[0]);
    let [outer_let] = block.statements() else {
        panic!("pre-binding fixture must retain one outer Let statement");
    };
    let outer_statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), *outer_let)
        .expect("outer Let");
    let HirStmtKind::Let {
        locals: outer_locals,
        ..
    } = outer_statement.kind()
    else {
        panic!("outer statement must remain a Let");
    };
    let [outer_local] = outer_locals.as_ref() else {
        panic!("outer Let must retain one Local generation");
    };

    let closure_id = block.tail();
    let closure = typed_closure(&module, closure_id);
    let [capture_id] = closure.captures() else {
        panic!("inner initializer must capture exactly the pre-binding Local");
    };
    let retained = capture(&module, *capture_id);
    assert_eq!(retained.local(), *outer_local);
    let initializer_use = parsed
        .document()
        .text()
        .rfind("let value = value")
        .expect("inner initializer source")
        + "let value = ".len();
    assert_eq!(retained.first_use().range().start(), initializer_use);

    let HirExprKind::Block(closure_body) = expression(&module, closure.body()).kind() else {
        panic!("pre-binding closure body must remain a Block");
    };
    let [inner_let] = closure_body.statements() else {
        panic!("pre-binding closure must retain one inner Let");
    };
    let inner_statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), *inner_let)
        .expect("inner Let");
    let HirStmtKind::Let {
        locals: inner_locals,
        ..
    } = inner_statement.kind()
    else {
        panic!("inner statement must remain a Let");
    };
    let [inner_local] = inner_locals.as_ref() else {
        panic!("inner Let must retain one Local generation");
    };
    assert_ne!(outer_local, inner_local);

    let HirExprKind::Record(tail) = expression(&module, closure_body.tail()).kind() else {
        panic!("post-binding tail must remain a Record");
    };
    let [tail_field] = tail.fields() else {
        panic!("post-binding tail must retain one shorthand field");
    };
    assert_eq!(tail_field.local(), Some(*inner_local));
}

#[test]
fn nested_closure_capture_propagation_stops_at_the_owning_scope() {
    let parsed = parsed_source(
        "closure-capture-nested",
        &["result { let grand = 1; |outer| || outer + grand }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let outer_id = typed_computation_block(&module, owners[0]).tail();
    let outer = typed_closure(&module, outer_id);
    let inner_id = outer.body();
    let inner = typed_closure(&module, inner_id);

    let [outer_capture] = outer.captures() else {
        panic!("outer closure must capture only the grandparent Local");
    };
    assert_eq!(
        captured_local_name(&module, capture(&module, *outer_capture)),
        "grand"
    );
    let [inner_outer, inner_grand] = inner.captures() else {
        panic!("inner closure must capture its parent parameter and grandparent Local");
    };
    assert_eq!(
        captured_local_name(&module, capture(&module, *inner_outer)),
        "outer"
    );
    assert_eq!(
        captured_local_name(&module, capture(&module, *inner_grand)),
        "grand"
    );
    assert_capture_slot(&module, outer_id, *outer_capture, 0);
    assert_capture_slot(&module, inner_id, *inner_outer, 0);
    assert_capture_slot(&module, inner_id, *inner_grand, 1);
}

#[test]
fn record_shorthand_participates_in_closure_capture_discovery() {
    let parsed = parsed_source(
        "closure-capture-record-shorthand",
        &["result { let outer = 1; || Point { outer } }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let closure_id = typed_computation_block(&module, owners[0]).tail();
    let [capture_id] = typed_closure(&module, closure_id).captures() else {
        panic!("record shorthand must retain one closure capture");
    };
    let retained = capture(&module, *capture_id);
    assert_eq!(captured_local_name(&module, retained), "outer");
    assert_eq!(retained.access(), CaptureAccess::Read);
    assert_capture_slot(&module, closure_id, *capture_id, 0);
}

#[test]
fn synthetic_descendant_limit_is_inclusive_and_atomic() {
    let maximum = HirLimit::SyntheticDescendantsPerOwner.maximum();
    let exact = parsed_source(
        "closure-capture-limit-exact",
        &[capture_limit_fixture(maximum)],
    );
    let (module, owners, _) = lower_and_publish(&exact);
    let closure_id = typed_computation_block(&module, owners[0]).tail();
    assert_eq!(typed_closure(&module, closure_id).captures().len(), maximum);
    assert_eq!(module.captures().count(), maximum);

    let one_over = parsed_source(
        "closure-capture-limit-one-over",
        &[capture_limit_fixture(maximum + 1)],
    );
    let attached = attached_expressions(&one_over).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("capture limit database");
    let retry_source = parsed_source("closure-capture-limit-retry", &[capture_limit_fixture(1)]);
    let retry_attached = attached_expressions(&retry_source).pop().unwrap();
    let (control_closure, control_captures) = {
        let mut control = stage(&database, &retry_source);
        let control_scope = allocate_module_scope(&mut control, &retry_source);
        let control_root = control
            .lower_attached_expression(&retry_attached, control_scope)
            .expect("uncontended capture identity control");
        staged_closure_captures(&mut control, control_root)
    };
    let before = database.test_state();
    let mut transaction = stage(&database, &one_over);
    let scope = allocate_module_scope(&mut transaction, &one_over);
    let result = transaction.lower_attached_expression(&attached, scope);
    assert!(
        matches!(
        result,
        Err(HirLowerFailure::Limit(error))
            if error.limit() == HirLimit::SyntheticDescendantsPerOwner
                && error.observed() == maximum + 1
                && error.maximum() == maximum
        ),
        "unexpected one-over result: {result:?}"
    );
    assert!(transaction.finish(&mut database).is_err());
    assert_eq!(database.test_state(), before);

    let mut retry = stage(&database, &retry_source);
    let retry_scope = allocate_module_scope(&mut retry, &retry_source);
    let retry_root = retry
        .lower_attached_expression(&retry_attached, retry_scope)
        .expect("valid capture lowering after rejected one-over transaction");
    let (retry_closure, retry_captures) = staged_closure_captures(&mut retry, retry_root);
    assert_eq!(retry_closure, control_closure);
    assert_eq!(retry_captures, control_captures);
}

#[test]
fn capture_limit_is_inclusive_and_atomic() {
    assert_eq!(HirLimit::Captures.maximum(), 65_536);

    let exact = parsed_source(
        "closure-capture-arena-limit-exact",
        &[capture_limit_fixture(2)],
    );
    let attached = attached_expressions(&exact).pop().unwrap();
    let mut exact_database = HirDatabase::try_new().expect("capture arena exact database");
    let mut exact_transaction = stage(&exact_database, &exact);
    let scope = allocate_module_scope(&mut exact_transaction, &exact);
    exact_transaction
        .storage_mut()
        .1
        .captures()
        .set_maximum_for_test(2);
    let owner = exact_transaction
        .lower_attached_expression(&attached, scope)
        .expect("the exact Capture arena limit must lower");
    let exact_module = exact_transaction
        .finish(&mut exact_database)
        .expect("the exact Capture arena limit must publish")
        .into_module();
    let closure_id = typed_computation_block(&exact_module, owner).tail();
    assert_eq!(typed_closure(&exact_module, closure_id).captures().len(), 2);
    assert_eq!(exact_module.captures().count(), 2);

    let (initial, revised) = parsed_revisions(
        "closure-capture-arena-limit-one-over",
        &capture_limit_fixture(3),
    );
    let initial_attached = attached_expressions(&initial).pop().unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().expect("capture arena rollback database");
    let mut accepted_transaction = stage(&database, &initial);
    let accepted_scope = allocate_module_scope(&mut accepted_transaction, &initial);
    accepted_transaction
        .lower_attached_expression(&initial_attached, accepted_scope)
        .expect("accepted Capture arena baseline");
    let accepted = accepted_transaction
        .finish(&mut database)
        .expect("accepted Capture arena baseline publication")
        .into_module();
    assert_eq!(accepted.captures().count(), 3);

    let revised_attached = attached_expressions(&revised).pop().unwrap();
    let before = database.test_state();
    let (control_closure, control_captures) = {
        let mut control = stage(&database, &revised);
        let control_scope = allocate_module_scope(&mut control, &revised);
        let control_root = control
            .lower_attached_expression(&revised_attached, control_scope)
            .expect("unrestricted Capture arena control lowering");
        staged_closure_captures(&mut control, control_root)
    };

    let mut rejected = stage(&database, &revised);
    let rejected_scope = allocate_module_scope(&mut rejected, &revised);
    rejected.storage_mut().1.captures().set_maximum_for_test(2);
    let result = rejected.lower_attached_expression(&revised_attached, rejected_scope);
    assert!(
        matches!(
            result,
            Err(HirLowerFailure::Limit(error))
                if error.limit() == HirLimit::Captures
                    && error.observed() == 3
                    && error.maximum() == 2
        ),
        "unexpected one-over Capture arena result: {result:?}"
    );
    assert!(rejected.finish(&mut database).is_err());
    assert_eq!(database.test_state(), before);
    let current = database
        .current(&key)
        .expect("accepted module stays current");
    assert!(Arc::ptr_eq(&accepted, &current));
    assert_eq!(current.captures().count(), 3);

    let mut retry = stage(&database, &revised);
    let retry_scope = allocate_module_scope(&mut retry, &revised);
    let retry_root = retry
        .lower_attached_expression(&revised_attached, retry_scope)
        .expect("Capture arena retry after atomic rejection");
    let (retry_closure, retry_captures) = staged_closure_captures(&mut retry, retry_root);
    assert_eq!(retry_closure, control_closure);
    assert_eq!(retry_captures, control_captures);
    let retried = retry
        .finish(&mut database)
        .expect("Capture arena retry publication")
        .into_module();
    assert_eq!(retried.captures().count(), 3);
    assert!(Arc::ptr_eq(
        &retried,
        &database
            .current(&module_key(&revised))
            .expect("retried module is current")
    ));
}

#[test]
fn capture_arena_limit_fixture_avoids_the_statement_limit() {
    for count in [2, HirLimit::SyntheticDescendantsPerOwner.maximum()] {
        let parsed = parsed_source(
            &format!("closure-capture-arena-fixture-{count}"),
            &[capture_arena_limit_fixture(count)],
        );
        assert!(
            parsed.diagnostics().is_empty(),
            "Capture arena fixture {count} must parse cleanly: {:?}",
            parsed.diagnostics()
        );
        let (module, _, _) = lower_and_publish(&parsed);
        assert_eq!(
            module.status(),
            HirModuleStatus::Clean,
            "Capture arena fixture {count} diagnostics: {:?}",
            module.diagnostics()
        );
        assert_eq!(module.captures().count(), count);
    }
}

#[test]
fn repeated_capture_uses_above_the_descendant_limit_charge_once() {
    let parsed = parsed_source(
        "closure-capture-reuse-above-limit",
        &[repeated_capture_fixture(
            HirLimit::SyntheticDescendantsPerOwner.maximum() + 1,
        )],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    let closure_id = typed_computation_block(&module, owners[0]).tail();
    let [capture_id] = typed_closure(&module, closure_id).captures() else {
        panic!("every repeated use must reuse one CaptureId");
    };
    assert_eq!(module.captures().count(), 1);
    assert_eq!(
        captured_local_name(&module, capture(&module, *capture_id)),
        "outer"
    );
}

#[test]
fn closure_capture_freeze_rejects_first_use_owner_divergence() {
    let parsed = parsed_source(
        "closure-capture-first-use-tamper",
        &["result { let outer = 1; || outer + outer }".into()],
    );
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("capture tamper database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid capture prefix");
    let (closure_id, capture_id) = {
        let (slots, arenas) = transaction.storage_mut();
        let root = arenas.expressions().resolve_staged(slots, root).unwrap();
        let HirExprKind::ComputationBlock(block) = root.kind() else {
            panic!("fixture root must remain a Result computation block");
        };
        let closure_id = block.tail();
        let closure = arenas
            .expressions()
            .resolve_staged(slots, closure_id)
            .unwrap();
        let HirExprKind::Closure(closure) = closure.kind() else {
            panic!("fixture tail must remain a Closure");
        };
        (closure_id, closure.captures()[0])
    };
    let retained = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .captures()
            .resolve_staged(slots, capture_id)
            .unwrap()
            .clone()
    };
    let later_start = parsed
        .document()
        .text()
        .rfind("outer")
        .expect("second captured use");
    let later_use = parsed
        .document()
        .span(SourceRange::new(later_start, later_start + "outer".len()))
        .expect("later captured source span");
    let replacement =
        HirCapture::try_new(closure_id, retained.local(), retained.access(), later_use)
            .expect("same-module capture tamper");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .captures()
            .revise_finalized(slots, capture_id, replacement)
            .expect("test-only capture payload substitution");
    }
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleArenaSnapshot
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());
}

#[test]
fn closure_capture_freeze_rejects_a_sibling_scope_local() {
    let parsed = parsed_source(
        "closure-capture-sibling-local-tamper",
        &[
            "result { let captured = 1; || captured }".into(),
            "{ let sibling = 2; sibling }".into(),
        ],
    );
    let attached = attached_expressions(&parsed);
    let mut database = HirDatabase::try_new().expect("capture sibling database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let captured_root = transaction
        .lower_attached_expression(&attached[0], scope)
        .expect("capturing closure prefix");
    let sibling_root = transaction
        .lower_attached_expression(&attached[1], scope)
        .expect("sibling block prefix");
    let sibling = staged_block_let_local(&mut transaction, sibling_root, 0);
    let (closure_id, capture_id, retained) = {
        let (slots, arenas) = transaction.storage_mut();
        let root = arenas
            .expressions()
            .resolve_staged(slots, captured_root)
            .unwrap();
        let HirExprKind::ComputationBlock(block) = root.kind() else {
            panic!("capturing fixture root must remain a Result block");
        };
        let closure_id = block.tail();
        let closure = arenas
            .expressions()
            .resolve_staged(slots, closure_id)
            .unwrap();
        let HirExprKind::Closure(closure) = closure.kind() else {
            panic!("capturing fixture tail must remain a Closure");
        };
        let capture_id = closure.captures()[0];
        let retained = arenas
            .captures()
            .resolve_staged(slots, capture_id)
            .unwrap()
            .clone();
        (closure_id, capture_id, retained)
    };
    let replacement = HirCapture::try_new(
        closure_id,
        sibling,
        retained.access(),
        retained.first_use().clone(),
    )
    .expect("same-module sibling capture tamper");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .captures()
            .revise_finalized(slots, capture_id, replacement)
            .expect("test-only sibling Local substitution");
    }
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleArenaSnapshot
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());
}

#[allow(
    clippy::too_many_lines,
    reason = "this helper executes one closed atomic capture-graph tamper matrix shared by its small public test cases"
)]
fn assert_capture_graph_tamper_rejected(tamper: CaptureGraphTamper) {
    let suffix = match tamper {
        CaptureGraphTamper::WrongOwner => "wrong-owner",
        CaptureGraphTamper::Reordered => "reordered",
        CaptureGraphTamper::Orphan => "orphan",
        CaptureGraphTamper::DuplicateLocal => "duplicate-local",
    };
    let (initial, revised) = parsed_revisions(
        &format!("closure-capture-graph-{suffix}"),
        "result { let left = 1; let right = 2; || right + left }",
    );
    let initial_attached = attached_expressions(&initial).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("capture graph tamper database");
    let mut accepted_transaction = stage(&database, &initial);
    let accepted_scope = allocate_module_scope(&mut accepted_transaction, &initial);
    accepted_transaction
        .lower_attached_expression(&initial_attached, accepted_scope)
        .expect("accepted capture graph baseline");
    let accepted = accepted_transaction
        .finish(&mut database)
        .expect("accepted capture graph publication")
        .into_module();
    assert_eq!(accepted.captures().count(), 2);
    let before = database.test_state();

    let revised_attached = attached_expressions(&revised).pop().unwrap();
    let mut transaction = stage(&database, &revised);
    let scope = allocate_module_scope(&mut transaction, &revised);
    let root = transaction
        .lower_attached_expression(&revised_attached, scope)
        .expect("valid capture graph prefix");
    let (closure_id, captures) = staged_closure_captures(&mut transaction, root);
    assert_eq!(captures.len(), 2);

    match tamper {
        CaptureGraphTamper::WrongOwner => {
            let retained = {
                let (slots, arenas) = transaction.storage_mut();
                arenas
                    .captures()
                    .resolve_staged(slots, captures[0])
                    .unwrap()
                    .clone()
            };
            let replacement = HirCapture::try_new(
                root,
                retained.local(),
                retained.access(),
                retained.first_use().clone(),
            )
            .expect("same-module wrong capture owner");
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .captures()
                .revise_finalized(slots, captures[0], replacement)
                .expect("test-only wrong capture owner substitution");
        }
        CaptureGraphTamper::Reordered => revise_staged_closure_captures(
            &mut transaction,
            closure_id,
            vec![captures[1], captures[0]].into_boxed_slice(),
        ),
        CaptureGraphTamper::Orphan => revise_staged_closure_captures(
            &mut transaction,
            closure_id,
            vec![captures[0]].into_boxed_slice(),
        ),
        CaptureGraphTamper::DuplicateLocal => {
            let (first, retained) = {
                let (slots, arenas) = transaction.storage_mut();
                let first = arenas
                    .captures()
                    .resolve_staged(slots, captures[0])
                    .unwrap()
                    .local();
                let retained = arenas
                    .captures()
                    .resolve_staged(slots, captures[1])
                    .unwrap()
                    .clone();
                (first, retained)
            };
            let replacement = HirCapture::try_new(
                closure_id,
                first,
                retained.access(),
                retained.first_use().clone(),
            )
            .expect("same-module duplicate capture Local");
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .captures()
                .revise_finalized(slots, captures[1], replacement)
                .expect("test-only duplicate capture Local substitution");
        }
    }

    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleArenaSnapshot
        ))
    ));
    assert_eq!(database.test_state(), before);
    assert!(Arc::ptr_eq(
        &accepted,
        &database
            .current(&module_key(&initial))
            .expect("accepted capture graph stays current")
    ));
}

#[test]
fn closure_capture_freeze_rejects_cross_arena_corruption_atomically() {
    for tamper in [
        CaptureGraphTamper::WrongOwner,
        CaptureGraphTamper::Reordered,
        CaptureGraphTamper::Orphan,
        CaptureGraphTamper::DuplicateLocal,
    ] {
        assert_capture_graph_tamper_rejected(tamper);
    }
}

#[test]
fn closure_freeze_rejects_parameter_annotation_and_generation_tampering() {
    assert_expression_freeze_rejects(
        "closure-local-annotation",
        "|item: Label| item",
        |transaction, root| {
            let (local, ty) = {
                let (slots, arenas) = transaction.storage_mut();
                let (scope, ty) = {
                    let payload = arenas.expressions().resolve_staged(slots, root).unwrap();
                    let HirExprKind::Closure(closure) = payload.kind() else {
                        panic!("staged Closure owner")
                    };
                    (closure.scope(), closure.parameters()[0].ty().unwrap())
                };
                let scope = arenas.scopes().resolve_staged(slots, scope).unwrap();
                (scope.locals()[0], ty)
            };
            tamper_local_payload(transaction, local, LocalPayloadTamper::Annotation(Some(ty)));
        },
    );
    assert_expression_local_freeze_rejects(
        "closure-local-generation",
        "|value: First, value: Second| value",
        |transaction, root| {
            let (slots, arenas) = transaction.storage_mut();
            let scope = {
                let payload = arenas.expressions().resolve_staged(slots, root).unwrap();
                let HirExprKind::Closure(closure) = payload.kind() else {
                    panic!("staged Closure owner")
                };
                closure.scope()
            };
            arenas
                .scopes()
                .resolve_staged(slots, scope)
                .unwrap()
                .locals()[1]
        },
        LocalPayloadTamper::Generation(LocalGeneration::FIRST),
    );
}

#[test]
fn e28_computation_blocks_preserve_kind_scope_statements_and_authored_tail() {
    let parsed = parsed_source(
        "computation-authored-tail",
        &[
            "result { let item = 1; item }".into(),
            "task { let item = 1; item }".into(),
            "seq { let item = 1; item }".into(),
            "stream { let item = 1; item }".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let expected = [
        HirComputationBlockKind::Result,
        HirComputationBlockKind::Task,
        HirComputationBlockKind::Seq,
        HirComputationBlockKind::Stream,
    ];
    for (owner, expected) in owners.iter().copied().zip(expected) {
        let root = expression(&module, owner);
        let block = typed_computation_block(&module, owner);
        assert_eq!(block.kind(), expected);
        assert_eq!(block.statements().len(), 1);
        assert_eq!(root.state(), &HirPoisonState::Clean);

        let scope = module
            .arenas()
            .scopes()
            .resolve(module.slots(), block.scope())
            .expect("E28 block scope");
        assert_eq!(scope.kind(), HirScopeKind::Block);
        assert_eq!(scope.parent(), Some(root.scope()));
        assert_eq!(scope.owner(), &HirScopeOwner::Expr(owner));
        assert_eq!(expression(&module, block.tail()).scope(), block.scope());
        assert!(matches!(
            module.slots().resolve(block.tail()).unwrap().origin(),
            HirOrigin::Source(_)
        ));

        for role in [
            HirExprSourceRole::Statement { ordinal: 0 },
            HirExprSourceRole::Tail,
        ] {
            let source = module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Expr { owner, role },
                )
                .expect("E28 source query");
            assert_eq!(source.owner_status(), HirSourceOwnerStatus::Clean);
            assert!(matches!(source.presence(), HirSourcePresence::Present(_)));
        }
    }
}

#[test]
fn e28_computation_omission_selects_required_or_unit_tail_by_kind() {
    let parsed = parsed_source(
        "computation-omitted-tail",
        &[
            "result { let item = 1; }".into(),
            "task { let item = 1; }".into(),
            "seq { let item = 1; }".into(),
            "stream { let item = 1; }".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    for (ordinal, owner) in owners.iter().copied().enumerate() {
        let block = typed_computation_block(&module, owner);
        let tail = expression(&module, block.tail());
        let metadata = module.slots().resolve(block.tail()).expect("E28 tail slot");
        if ordinal < 2 {
            assert_eq!(
                expression(&module, owner).state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
            );
            assert!(matches!(
                (tail.kind(), tail.state(), metadata.origin()),
                (
                    HirExprKind::Error(error),
                    HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail),
                    HirOrigin::Synthetic(key),
                ) if error.issue() == HirGenericExprIssue::TransactionalChildFailure
                    && key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::MissingRequiredTail
                    && key.ordinal() == 0
            ));
        } else {
            assert_eq!(expression(&module, owner).state(), &HirPoisonState::Clean);
            assert!(matches!(tail.kind(), HirExprKind::Unit));
            assert_eq!(tail.state(), &HirPoisonState::Clean);
            assert!(matches!(
                metadata.origin(),
                HirOrigin::Synthetic(key)
                    if key.owner() == SyntheticOwner::Expr(owner)
                        && key.role() == SyntheticRole::ImplicitUnitTail
                        && key.ordinal() == 0
            ));
        }
        assert!(matches!(
            metadata.source_site(),
            HirSourceSite::Insertion(_)
        ));
    }
}

#[test]
fn e29_named_block_preserves_valid_and_invalid_present_name_but_normalizes_omission() {
    let parsed = parsed_source(
        "named-block-name-state",
        &[
            "(scope retry { let item = 1; })".into(),
            "(scope { let item = 1; })".into(),
            "(scope 9bad { let item = 1; })".into(),
        ],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let valid = typed_named_block(&module, owners[0]);
    assert!(matches!(
        valid.name(),
        HirNamedBlockName::Resolved(name) if name.as_str() == "retry"
    ));
    assert_eq!(
        expression(&module, owners[0]).state(),
        &HirPoisonState::Clean
    );

    assert!(matches!(
        expression(&module, owners[1]).kind(),
        HirExprKind::Block(_)
    ));
    assert!(matches!(
        attached[1].projection(),
        ExpressionProjection::Block
    ));
    assert!(
        attached[1]
            .component(ExpressionComponentRole::Name)
            .is_none()
    );

    let invalid = typed_named_block(&module, owners[2]);
    assert!(matches!(
        invalid.name(),
        HirNamedBlockName::InvalidPresent(crate::leaf::HirNameInvariantError::InvalidIdentifier)
    ));
    assert_eq!(
        expression(&module, owners[2]).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidName(
            crate::leaf::HirNameInvariantError::InvalidIdentifier,
        ))
    );
    let name_source = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[2],
                role: HirExprSourceRole::Name,
            },
        )
        .expect("E29 invalid-present name source");
    assert_eq!(name_source.owner_status(), HirSourceOwnerStatus::Poisoned);
    assert!(matches!(
        name_source.presence(),
        HirSourcePresence::Present(_)
    ));

    for (ordinal, owner) in owners.iter().copied().enumerate() {
        let tail = match expression(&module, owner).kind() {
            HirExprKind::Block(block) => block.tail(),
            HirExprKind::NamedBlock(block) => block.tail(),
            _ => panic!("E29 source must lower to Block or NamedBlock"),
        };
        assert!(matches!(
            expression(&module, tail).kind(),
            HirExprKind::Unit
        ));
        assert!(
            matches!(
                module.slots().resolve(tail).unwrap().origin(),
                HirOrigin::Synthetic(key)
                    if key.owner() == SyntheticOwner::Expr(owner)
                        && key.role() == SyntheticRole::ImplicitUnitTail
                        && key.ordinal() == 0
            ),
            "case {ordinal}"
        );
    }
}

#[test]
fn e28_e29_reuse_and_name_limit_failure_are_atomic() {
    let parsed = parsed_source("named-block-reuse", &["(scope retry { })".into()]);
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E29 HIR database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let first = transaction
        .lower_attached_expression(&attached, scope)
        .expect("first E29 lowering");
    let second = transaction
        .lower_attached_expression(&attached, scope)
        .expect("reused E29 lowering");
    assert_eq!(first, second);
    let module = transaction
        .finish(&mut database)
        .expect("E29 identity publication")
        .into_module();
    let tail = typed_named_block(&module, first).tail();
    assert!(matches!(
        module.slots().resolve(tail).unwrap().origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Expr(first)
                && key.role() == SyntheticRole::ImplicitUnitTail
                && key.ordinal() == 0
    ));

    let exact_name = "a".repeat(HirLimit::NameBytes.maximum());
    let exact = parsed_source(
        "named-block-name-exact",
        &[format!("(scope {exact_name} {{ }})")],
    );
    let (module, owners, _) = lower_and_publish(&exact);
    assert_eq!(module.status(), HirModuleStatus::Clean);
    assert!(matches!(
        typed_named_block(&module, owners[0]).name(),
        HirNamedBlockName::Resolved(name) if name.as_str().len() == HirLimit::NameBytes.maximum()
    ));

    let one_over_name = "a".repeat(HirLimit::NameBytes.maximum() + 1);
    let over = parsed_source(
        "named-block-name-one-over",
        &[format!("(scope {one_over_name} {{ }})")],
    );
    let attached = attached_expressions(&over).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E29 limit database");
    let mut transaction = stage(&database, &over);
    let scope = allocate_module_scope(&mut transaction, &over);
    assert!(matches!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Limit(error)) if error.limit() == HirLimit::NameBytes
    ));
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&over)).is_none());
}

#[test]
fn e28_source_freeze_rejects_computation_kind_substitution() {
    let parsed = parsed_source("computation-kind-substitution", &["result { 1 }".into()]);
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E28 HIR database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid E28 prefix");
    let current = {
        let (slots, arenas) = transaction.storage_mut();
        let payload = arenas
            .expressions()
            .resolve_staged(slots, root)
            .expect("staged E28 root");
        let HirExprKind::ComputationBlock(expression) = payload.kind() else {
            panic!("staged E28 ComputationBlock");
        };
        expression.clone()
    };
    let replacement = HirExpr::try_new(
        scope,
        HirExprKind::ComputationBlock(HirComputationBlockExpr::new(
            HirComputationBlockKind::Seq,
            current.scope(),
            current.statements().to_vec().into_boxed_slice(),
            current.tail(),
        )),
        HirPoisonState::Clean,
    )
    .expect("same-module forged E28 kind");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .revise_finalized(slots, root, replacement)
            .expect("test-only E28 kind substitution");
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
fn e30_if_lowers_authored_and_implicit_else_through_one_typed_owner() {
    let parsed = parsed_source(
        "if-clean-matrix",
        &[
            "if true { 1 } else { 2 }".into(),
            "if false { 3 }".into(),
            "if true { 1 } else if false { 2 } else { 3 }".into(),
        ],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let authored = typed_if(&module, owners[0]);
    for child in [
        authored.condition(),
        authored.then_branch(),
        authored.else_branch(),
    ] {
        assert_eq!(
            expression(&module, child).scope(),
            expression(&module, owners[0]).scope()
        );
        assert!(matches!(
            module.slots().resolve(child).unwrap().origin(),
            HirOrigin::Source(_)
        ));
    }

    let omitted = typed_if(&module, owners[1]);
    let omitted_else = omitted.else_branch();
    let metadata = module
        .slots()
        .resolve(omitted_else)
        .expect("E30 implicit else slot");
    assert!(matches!(
        metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Expr(owners[1])
                && key.role() == SyntheticRole::ImplicitUnitTail
                && key.ordinal() == 0
    ));
    assert!(matches!(
        expression(&module, omitted_else).kind(),
        HirExprKind::Unit
    ));
    assert_eq!(
        expression(&module, omitted_else).state(),
        &HirPoisonState::Clean
    );
    let omitted_component = attached[1]
        .component(ExpressionComponentRole::ElseBranch)
        .expect("E30 omitted else component");
    let HirSourceSite::Insertion(insertion) = metadata.source_site() else {
        panic!("E30 omitted else must own one insertion site");
    };
    assert_eq!(insertion.offset(), omitted_component.range().start());
    let source = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: owners[1],
                role: HirExprSourceRole::ElseBranch,
            },
        )
        .expect("E30 omitted else source query");
    assert_eq!(source.owner_status(), HirSourceOwnerStatus::Clean);
    assert!(matches!(
        source.presence(),
        HirSourcePresence::Present(HirSourceSite::Insertion(point))
            if point.offset() == omitted_component.range().start()
    ));

    let nested = typed_if(&module, owners[2]);
    assert!(matches!(
        expression(&module, nested.else_branch()).kind(),
        HirExprKind::If(_)
    ));
    assert!(matches!(
        attached[2].projection(),
        ExpressionProjection::If {
            else_branch: Some(_),
            ..
        }
    ));
}

#[test]
fn e30_if_retains_typed_recovery_for_each_required_slot() {
    let parsed = parsed_source(
        "if-recovery-matrix",
        &[
            "if { 1 }".into(),
            "if true".into(),
            "if true { 1 } else".into(),
            "if true { 1 + } else { 2 }".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let cases = [
        (0, HirExprSourceRole::Condition),
        (1, HirExprSourceRole::ThenBranch),
        (2, HirExprSourceRole::ElseBranch),
    ];
    for (case, role) in cases {
        let owner = owners[case];
        let expression = typed_if(&module, owner);
        let child = match case {
            0 => expression.condition(),
            1 => expression.then_branch(),
            2 => expression.else_branch(),
            _ => unreachable!(),
        };
        assert_eq!(
            super::expression(&module, owner).state(),
            &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand { role })
        );
        let metadata = module.slots().resolve(child).expect("E30 recovery slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::RecoveryOperand
                    && key.ordinal() == u32::try_from(case).unwrap()
        ));
        assert!(matches!(
            metadata.source_site(),
            HirSourceSite::Insertion(_)
        ));
        assert!(matches!(
            super::expression(&module, child).kind(),
            HirExprKind::Error(error)
                if error.issue() == HirGenericExprIssue::TransactionalChildFailure
        ));
        let source = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr { owner, role },
            )
            .expect("E30 recovered component source query");
        assert_eq!(source.owner_status(), HirSourceOwnerStatus::Poisoned);
        assert!(matches!(source.presence(), HirSourcePresence::Present(_)));
    }

    let omitted_else = typed_if(&module, owners[1]).else_branch();
    assert!(matches!(
        module.slots().resolve(omitted_else).unwrap().origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Expr(owners[1])
                && key.role() == SyntheticRole::ImplicitUnitTail
    ));
    assert!(matches!(
        super::expression(&module, omitted_else).kind(),
        HirExprKind::Unit
    ));

    assert_eq!(
        super::expression(&module, owners[3]).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::ThenBranch,
            },
        ))
    );
}

#[test]
fn e30_if_reuses_root_and_implicit_else_identity_in_one_transaction() {
    let parsed = parsed_source("if-reuse", &["if true { 1 }".into()]);
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E30 HIR database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let first = transaction
        .lower_attached_expression(&attached, scope)
        .expect("first E30 lowering");
    let second = transaction
        .lower_attached_expression(&attached, scope)
        .expect("reused E30 lowering");
    assert_eq!(first, second);
    let module = transaction
        .finish(&mut database)
        .expect("E30 identity publication")
        .into_module();
    let implicit = typed_if(&module, first).else_branch();
    let key = SyntheticKey::try_new(
        SyntheticOwner::Expr(first),
        SyntheticRole::ImplicitUnitTail,
        0,
    )
    .unwrap();
    assert!(matches!(
        module.slots().resolve(implicit).unwrap().origin(),
        HirOrigin::Synthetic(actual) if *actual == key
    ));
}

#[test]
fn e30_if_freeze_rejects_child_order_and_implicit_payload_substitution() {
    let parsed = parsed_source(
        "if-child-order-substitution",
        &["if true { 1 } else { 2 }".into()],
    );
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E30 HIR database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid E30 prefix");
    let current = {
        let (slots, arenas) = transaction.storage_mut();
        let payload = arenas
            .expressions()
            .resolve_staged(slots, root)
            .expect("staged E30 root");
        let HirExprKind::If(expression) = payload.kind() else {
            panic!("staged E30 If");
        };
        expression.clone()
    };
    let replacement = HirExpr::try_new(
        scope,
        HirExprKind::If(HirIfExpr::new(
            current.then_branch(),
            current.condition(),
            current.else_branch(),
        )),
        HirPoisonState::Clean,
    )
    .expect("same-module forged E30 child order");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .revise_finalized(slots, root, replacement)
            .expect("test-only E30 child-order substitution");
    }
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());

    let parsed = parsed_source("if-implicit-substitution", &["if true { 1 }".into()]);
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E30 HIR database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid omitted-else E30 prefix");
    let implicit = {
        let (slots, arenas) = transaction.storage_mut();
        let payload = arenas
            .expressions()
            .resolve_staged(slots, root)
            .expect("staged omitted-else E30 root");
        let HirExprKind::If(expression) = payload.kind() else {
            panic!("staged omitted-else E30 If");
        };
        expression.else_branch()
    };
    let replacement = HirExpr::try_new(
        scope,
        HirExprKind::Tuple(HirTupleExpr::new(Box::new([]))),
        HirPoisonState::Clean,
    )
    .expect("same-module forged E30 implicit payload");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .revise_finalized(slots, implicit, replacement)
            .expect("test-only E30 implicit payload substitution");
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
fn e30_if_nested_limit_is_inclusive_and_one_over_is_atomic() {
    let exact_name = "a".repeat(HirLimit::NameBytes.maximum());
    let exact = parsed_source(
        "if-name-exact",
        &[format!("if true {{ .{exact_name} }} else {{ () }}")],
    );
    let (module, owners, _) = lower_and_publish(&exact);
    assert_eq!(module.status(), HirModuleStatus::Clean);
    assert!(matches!(
        expression(&module, typed_if(&module, owners[0]).then_branch()).kind(),
        HirExprKind::Block(_)
    ));

    let one_over_name = "a".repeat(HirLimit::NameBytes.maximum() + 1);
    let one_over = parsed_source(
        "if-name-one-over",
        &[format!("if true {{ .{one_over_name} }} else {{ () }}")],
    );
    let attached = attached_expressions(&one_over).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E30 HIR database");
    let mut transaction = stage(&database, &one_over);
    let scope = allocate_module_scope(&mut transaction, &one_over);
    assert!(matches!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Limit(error)) if error.limit() == HirLimit::NameBytes
    ));
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&one_over)).is_none());
}

#[test]
fn e31_if_let_publishes_one_binding_scope_for_pattern_guard_and_then() {
    let parsed = parsed_source(
        "if-let-binding-scope",
        &[
            "if let value = 1 { value } else { 0 }".into(),
            "if let guarded = 2 when true { guarded } else { 0 }".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    for (ordinal, owner) in owners.iter().copied().enumerate() {
        let root = expression(&module, owner);
        let if_let = typed_if_let(&module, owner);
        let binding_scope = module
            .arenas()
            .scopes()
            .resolve(module.slots(), if_let.scope())
            .expect("E31 binding scope");
        assert_eq!(binding_scope.kind(), HirScopeKind::Conditional);
        assert_eq!(binding_scope.parent(), Some(root.scope()));
        assert_eq!(binding_scope.owner(), &HirScopeOwner::Expr(owner));
        assert_eq!(binding_scope.locals().len(), 1);

        let pattern = module
            .arenas()
            .patterns()
            .resolve(module.slots(), if_let.pattern())
            .expect("E31 root pattern");
        assert_eq!(pattern.scope(), if_let.scope());
        let local = module
            .arenas()
            .locals()
            .resolve(module.slots(), binding_scope.locals()[0])
            .expect("E31 pattern local");
        assert_eq!(local.scope(), if_let.scope());
        assert_eq!(local.kind(), HirLocalKind::PatternBinding);
        assert_eq!(local.pattern(), Some(if_let.pattern()));

        assert_eq!(
            expression(&module, if_let.scrutinee()).scope(),
            root.scope()
        );
        assert_eq!(
            expression(&module, if_let.then_branch()).scope(),
            if_let.scope()
        );
        assert_eq!(
            expression(&module, if_let.else_branch()).scope(),
            root.scope()
        );
        match (ordinal, if_let.guard()) {
            (0, None) => {}
            (1, Some(guard)) => {
                assert_eq!(expression(&module, guard).scope(), if_let.scope());
            }
            _ => panic!("E31 guard presence must match authored syntax"),
        }
        assert_eq!(root.state(), &HirPoisonState::Clean);
    }
}

#[test]
fn e31_if_let_omitted_and_authored_missing_else_share_required_tail_identity() {
    let parsed = parsed_source(
        "if-let-required-tail",
        &[
            "if let value = 1 { value }".into(),
            "if let value = 1 { value } else".into(),
        ],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    for (ordinal, owner) in owners.iter().copied().enumerate() {
        let if_let = typed_if_let(&module, owner);
        assert_eq!(
            expression(&module, owner).state(),
            &HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
        );
        let tail = expression(&module, if_let.else_branch());
        assert_eq!(tail.scope(), expression(&module, owner).scope());
        assert!(matches!(
            (tail.kind(), tail.state()),
            (
                HirExprKind::Error(error),
                HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
            ) if error.issue() == HirGenericExprIssue::TransactionalChildFailure
        ));
        let metadata = module
            .slots()
            .resolve(if_let.else_branch())
            .expect("E31 required-tail slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::MissingRequiredTail
                    && key.ordinal() == 0
        ));
        let expected = if ordinal == 0 {
            attached[ordinal]
                .component(ExpressionComponentRole::ElseBranch)
                .expect("E31 omitted-else insertion")
        } else {
            attached[ordinal]
                .children()
                .iter()
                .find(|child| child.ordinal() == 3)
                .expect("E31 authored missing else")
                .source_span()
        };
        assert_eq!(metadata.source_site().source_identity(), expected.source());
        assert!(matches!(
            metadata.source_site(),
            HirSourceSite::Insertion(point) if point.offset() == expected.range().start()
        ));
    }
}

#[test]
fn e31_if_let_recovery_precedence_is_pattern_scrutinee_guard_then_else() {
    let parsed = parsed_source(
        "if-let-recovery-precedence",
        &[
            "if let value { 1 } else { 2 }".into(),
            "if let value = 1 when { 1 } else { 2 }".into(),
            "if let value = 1 else { 2 }".into(),
            "if let value = 1 { 1 } else".into(),
            "if let = { 1 }".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let expected = [
        HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::Scrutinee,
        },
        HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::Guard,
        },
        HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::ThenBranch,
        },
        HirRecoveryIssue::MissingRequiredTail,
    ];
    for (owner, issue) in owners.iter().copied().zip(expected) {
        assert_eq!(
            expression(&module, owner).state(),
            &HirPoisonState::Poisoned(issue)
        );
    }

    let pattern_first = typed_if_let(&module, owners[4]);
    let pattern = module
        .arenas()
        .patterns()
        .resolve(module.slots(), pattern_first.pattern())
        .expect("E31 recovered pattern");
    assert!(matches!(pattern.state(), HirPoisonState::Poisoned(_)));
    assert_eq!(expression(&module, owners[4]).state(), pattern.state());
}

#[test]
fn e31_if_let_freeze_rejects_cross_scope_child_substitution() {
    let parsed = parsed_source(
        "if-let-cross-scope-substitution",
        &["if let value = 1 when true { value } else { 0 }".into()],
    );
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E31 HIR database");
    let mut transaction = stage(&database, &parsed);
    let outer_scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, outer_scope)
        .expect("valid E31 prefix");
    let current = {
        let (slots, arenas) = transaction.storage_mut();
        let payload = arenas
            .expressions()
            .resolve_staged(slots, root)
            .expect("staged E31 root");
        let HirExprKind::IfLet(expression) = payload.kind() else {
            panic!("staged E31 IfLet");
        };
        expression.clone()
    };
    let replacement = HirExpr::try_new(
        outer_scope,
        HirExprKind::IfLet(HirIfLetExpr::new(
            current.scope(),
            current.pattern(),
            current.then_branch(),
            current.guard(),
            current.scrutinee(),
            current.else_branch(),
        )),
        HirPoisonState::Clean,
    )
    .expect("same-module forged E31 cross-scope children");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .revise_finalized(slots, root, replacement)
            .expect("test-only E31 child substitution");
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
fn e31_if_let_freeze_rejects_exact_local_payload_tampering() {
    assert_expression_local_freeze_rejects(
        "if-let-local-name",
        "if let value = 1 { value } else { 0 }",
        |transaction, root| {
            let (slots, arenas) = transaction.storage_mut();
            let scope = {
                let payload = arenas.expressions().resolve_staged(slots, root).unwrap();
                let HirExprKind::IfLet(expression) = payload.kind() else {
                    panic!("staged IfLet owner")
                };
                expression.scope()
            };
            arenas
                .scopes()
                .resolve_staged(slots, scope)
                .unwrap()
                .locals()[0]
        },
        LocalPayloadTamper::Name("renamed"),
    );
}

#[test]
fn e32_match_lowers_ordered_arm_scopes_bindings_and_source_roles() {
    let parsed = parsed_source(
        "match-clean-matrix",
        &["match option { .Some(item) when item > 0 => item, .None => 0 }".into()],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let root = expression(&module, owner);
    let matched = typed_match(&module, owner);
    assert_eq!(matched.arms().len(), 2);
    assert_eq!(
        expression(&module, matched.scrutinee()).scope(),
        root.scope()
    );

    let mut scopes = BTreeSet::new();
    for (arm_index, arm) in matched.arms().iter().enumerate() {
        let arm_index = u32::try_from(arm_index).unwrap();
        assert!(scopes.insert(arm.scope()));
        let scope = module
            .arenas()
            .scopes()
            .resolve(module.slots(), arm.scope())
            .expect("E32 arm scope");
        assert_eq!(scope.kind(), HirScopeKind::MatchArm);
        assert_eq!(scope.parent(), Some(root.scope()));
        assert_eq!(scope.owner(), &HirScopeOwner::Expr(owner));
        assert_eq!(scope.locals(), arm.locals());
        assert_eq!(expression(&module, arm.value()).scope(), arm.scope());
        if let Some(guard) = arm.guard() {
            assert_eq!(expression(&module, guard).scope(), arm.scope());
        }
        for local in arm.locals() {
            let local = module
                .arenas()
                .locals()
                .resolve(module.slots(), *local)
                .expect("E32 pattern local");
            assert_eq!(local.scope(), arm.scope());
            assert_eq!(local.kind(), HirLocalKind::MatchBinding);
            assert!(local.pattern().is_some());
        }

        for part in [
            HirMatchArmSourcePart::Whole,
            HirMatchArmSourcePart::Pattern,
            HirMatchArmSourcePart::Arrow,
            HirMatchArmSourcePart::Value,
        ] {
            let source = module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Expr {
                        owner,
                        role: HirExprSourceRole::MatchArm {
                            arm: arm_index,
                            part,
                        },
                    },
                )
                .expect("E32 required arm source");
            assert_eq!(source.owner_status(), HirSourceOwnerStatus::Clean);
            assert!(matches!(source.presence(), HirSourcePresence::Present(_)));
        }
    }
    assert_eq!(matched.arms()[0].locals().len(), 1);
    assert!(matched.arms()[0].guard().is_some());
    assert!(matched.arms()[1].locals().is_empty());
    assert!(matched.arms()[1].guard().is_none());
    let absent_guard = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::MatchArm {
                    arm: 1,
                    part: HirMatchArmSourcePart::Guard,
                },
            },
        )
        .expect("E32 optional guard query");
    assert!(matches!(
        absent_guard.presence(),
        HirSourcePresence::AbsentOptional
    ));
    assert_eq!(attached[0].match_arms().len(), 2);
}

#[test]
fn e32_match_recovery_keeps_guard_parent_poison_and_scope_owned_missing_value() {
    let parsed = parsed_source(
        "match-recovery-matrix",
        &[
            "match option { .Some(item) when => item }".into(),
            "match option { .None => }".into(),
            "match option".into(),
        ],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let guard_owner = owners[0];
    let guard_match = typed_match(&module, guard_owner);
    assert!(guard_match.arms()[0].guard().is_none());
    let guard_role = HirExprSourceRole::MatchArm {
        arm: 0,
        part: HirMatchArmSourcePart::Guard,
    };
    assert_eq!(
        expression(&module, guard_owner).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::RecoveredChild { role: guard_role },
        ))
    );
    let guard_source = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: guard_owner,
                role: guard_role,
            },
        )
        .expect("E32 missing guard source");
    assert!(matches!(
        guard_source.presence(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
    ));

    let value_owner = owners[1];
    let value_match = typed_match(&module, value_owner);
    let arm = &value_match.arms()[0];
    assert_eq!(
        expression(&module, value_owner).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
    );
    let metadata = module
        .slots()
        .resolve(arm.value())
        .expect("E32 missing arm value slot");
    assert!(matches!(
        metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Scope(arm.scope())
                && key.role() == SyntheticRole::MissingRequiredTail
                && key.ordinal() == 0
    ));
    assert_eq!(expression(&module, arm.value()).scope(), arm.scope());
    assert!(matches!(
        expression(&module, arm.value()).state(),
        HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
    ));

    let missing_body_owner = owners[2];
    assert!(typed_match(&module, missing_body_owner).arms().is_empty());
    assert_eq!(
        expression(&module, missing_body_owner).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidMatch(
            HirMatchRecoveryIssue::MissingBody,
        ))
    );
    assert!(matches!(
        attached[2].projection(),
        ExpressionProjection::Match(projection)
            if matches!(
                projection.terminator(),
                arcweft_lang_syntax::expressions::SyntaxMatchBodyTerminator::MissingBody
            )
    ));
}

#[test]
fn match_multiple_missing_arm_values_use_distinct_scope_tail_owners_and_rollback() {
    let parsed = parsed_source(
        "match-multiple-missing-arm-values",
        &["match option { .Some =>, .None => }".into()],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);
    assert_eq!(attached[0].match_arms().len(), 2);

    let owner = owners[0];
    let root = expression(&module, owner);
    let [first, second] = typed_match(&module, owner).arms() else {
        panic!("recovered Match must retain two typed arms");
    };
    let mut tail_ids = BTreeSet::new();
    for arm in [first, second] {
        let scope = module.resolve_scope(arm.scope()).expect("Match-arm scope");
        assert_eq!(scope.kind(), HirScopeKind::MatchArm);
        assert_eq!(scope.parent(), Some(root.scope()));
        assert_eq!(scope.owner(), &HirScopeOwner::Expr(owner));

        let metadata = module
            .slots()
            .resolve(arm.value())
            .expect("missing arm-value slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Scope(arm.scope())
                    && key.role() == SyntheticRole::MissingRequiredTail
                    && key.ordinal() == 0
        ));
        assert_eq!(expression(&module, arm.value()).scope(), arm.scope());
        assert!(matches!(
            expression(&module, arm.value()).state(),
            HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
        ));
        assert!(tail_ids.insert(arm.value()));
    }
    assert_ne!(first.scope(), second.scope());
    assert_eq!(tail_ids.len(), 2);

    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("rollback HIR database");
    let before = database.test_state();
    let mut transaction = stage(&database, &parsed);
    let outer_scope = allocate_module_scope(&mut transaction, &parsed);
    transaction.storage_mut().1.scopes().set_maximum_for_test(2);
    assert!(matches!(
        transaction.lower_attached_expression(&attached, outer_scope),
        Err(HirLowerFailure::Limit(error)) if error.limit() == HirLimit::Scopes
    ));
    assert!(transaction.finish(&mut database).is_err());
    assert_eq!(database.test_state(), before);
    assert!(database.current(&module_key(&parsed)).is_none());
}

#[test]
fn match_arm_scope_identity_survives_reverse_production_lookup_order() {
    let parsed = parsed_source(
        "match-arm-reverse-scope-lookup",
        &["match option { .Some =>, .None => }".into()],
    );
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("Match lookup-order HIR database");
    let mut transaction = stage(&database, &parsed);
    let outer_scope = allocate_module_scope(&mut transaction, &parsed);
    let owner = transaction
        .lower_attached_expression(&attached, outer_scope)
        .expect("source-ordered Match prefix");
    let (source_order_scopes, staged_tails) = {
        let (slots, arenas) = transaction.storage_mut();
        let payload = arenas
            .expressions()
            .resolve_staged(slots, owner)
            .expect("staged Match owner");
        let HirExprKind::Match(matched) = payload.kind() else {
            panic!("staged Match payload")
        };
        (
            matched
                .arms()
                .iter()
                .map(HirMatchArm::scope)
                .collect::<Vec<_>>(),
            matched
                .arms()
                .iter()
                .map(HirMatchArm::value)
                .collect::<Vec<_>>(),
        )
    };

    // Match lowering has no internal arm work queue or diagnostic map to
    // reverse. Reusing the production scope allocator in reverse attached-arm
    // order is the strongest real perturbation: identity must come from each
    // arm record, without changing the authored order stored in HirMatchExpr.
    let reverse_lookup_scopes = attached
        .match_arms()
        .iter()
        .rev()
        .map(|arm| {
            transaction
                .allocate_match_arm_scope(arm, owner, outer_scope)
                .expect("reverse production Match-arm scope lookup")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reverse_lookup_scopes,
        source_order_scopes
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>()
    );

    let module = transaction
        .finish(&mut database)
        .expect("reverse Match-arm lookup publishes atomically")
        .into_module();
    let matched = typed_match(&module, owner);
    assert_eq!(
        matched
            .arms()
            .iter()
            .map(HirMatchArm::scope)
            .collect::<Vec<_>>(),
        source_order_scopes
    );
    assert_eq!(
        matched
            .arms()
            .iter()
            .map(HirMatchArm::value)
            .collect::<Vec<_>>(),
        staged_tails
    );

    let mut distinct_scopes = BTreeSet::new();
    let mut distinct_tails = BTreeSet::new();
    for (arm, attached_arm) in matched.arms().iter().zip(attached.match_arms()) {
        assert!(distinct_scopes.insert(arm.scope()));
        assert!(distinct_tails.insert(arm.value()));
        let scope = module.resolve_scope(arm.scope()).expect("Match-arm scope");
        assert_eq!(scope.kind(), HirScopeKind::MatchArm);
        assert_eq!(scope.parent(), Some(outer_scope));
        assert_eq!(scope.owner(), &HirScopeOwner::Expr(owner));
        assert_eq!(
            module.slots().resolve(arm.scope()).unwrap().source_site(),
            &HirSourceSite::Span(attached_arm.whole_source_span())
        );
        assert!(matches!(
            module.slots().resolve(arm.value()).unwrap().origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Scope(arm.scope())
                    && key.role() == SyntheticRole::MissingRequiredTail
                    && key.ordinal() == 0
        ));
    }
    assert_eq!(distinct_scopes.len(), 2);
    assert_eq!(distinct_tails.len(), 2);
}

#[test]
fn e32_match_freeze_rejects_arm_order_substitution() {
    let parsed = parsed_source(
        "match-arm-order-substitution",
        &["match option { .Some(item) => item, .None => 0 }".into()],
    );
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E32 HIR database");
    let mut transaction = stage(&database, &parsed);
    let outer_scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, outer_scope)
        .expect("valid E32 prefix");
    let current = {
        let (slots, arenas) = transaction.storage_mut();
        let payload = arenas
            .expressions()
            .resolve_staged(slots, root)
            .expect("staged E32 root");
        let HirExprKind::Match(expression) = payload.kind() else {
            panic!("staged E32 Match");
        };
        expression.clone()
    };
    let mut reversed = current.arms().to_vec();
    reversed.reverse();
    let replacement = HirExpr::try_new(
        outer_scope,
        HirExprKind::Match(
            crate::expr::HirMatchExpr::try_new(current.scrutinee(), reversed.into_boxed_slice())
                .expect("same-module reversed E32 arms"),
        ),
        HirPoisonState::Clean,
    )
    .expect("same-module forged E32 arm order");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .revise_finalized(slots, root, replacement)
            .expect("test-only E32 arm-order substitution");
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
fn e32_match_freeze_rejects_exact_local_payload_tampering() {
    assert_expression_local_freeze_rejects(
        "match-local-mutability",
        "match subject { value => value }",
        |transaction, root| {
            let (slots, arenas) = transaction.storage_mut();
            let payload = arenas.expressions().resolve_staged(slots, root).unwrap();
            let HirExprKind::Match(expression) = payload.kind() else {
                panic!("staged Match owner")
            };
            expression.arms()[0].locals()[0]
        },
        LocalPayloadTamper::Mutable(true),
    );
}

#[test]
fn e32_match_scope_limit_exact_and_one_over_are_atomic() {
    assert!(require_match_arm_scope_limit(HirLimit::Scopes.maximum()).is_ok());
    assert!(matches!(
        require_match_arm_scope_limit(HirLimit::Scopes.maximum() + 1),
        Err(HirLowerFailure::Limit(error)) if error.limit() == HirLimit::Scopes
    ));

    let exact = parsed_source(
        "match-scope-limit-exact",
        &["match value { _ => 0 }".into()],
    );
    let attached = attached_expressions(&exact).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E32 HIR database");
    let mut transaction = stage(&database, &exact);
    let scope = allocate_module_scope(&mut transaction, &exact);
    transaction.storage_mut().1.scopes().set_maximum_for_test(2);
    let owner = transaction
        .lower_attached_expression(&attached, scope)
        .expect("one E32 arm fits the remaining test scope capacity");
    let module = transaction
        .finish(&mut database)
        .expect("exact E32 scope-capacity publication")
        .into_module();
    assert_eq!(module.status(), HirModuleStatus::Clean);
    assert_eq!(typed_match(&module, owner).arms().len(), 1);

    let one_over = parsed_source(
        "match-scope-limit-one-over",
        &["match value { _ => 0, _ => 1 }".into()],
    );
    let attached = attached_expressions(&one_over).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E32 HIR database");
    let mut transaction = stage(&database, &one_over);
    let scope = allocate_module_scope(&mut transaction, &one_over);
    transaction.storage_mut().1.scopes().set_maximum_for_test(2);
    assert!(matches!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Limit(error)) if error.limit() == HirLimit::Scopes
    ));
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&one_over)).is_none());
}
