use super::support::*;
use arcweft_source::DiagnosticLabelStyle;

#[test]
fn typechecks_ordinary_fn_try_await_without_wait_view() {
    let source = r"
fn load_bg_task() -> Result<Image, AssetError> {
    let bg = try await load_bg()
    Ok(bg)
}
";
    let hir = lower_bound_hir("ordinary-fn-try-await", source);
    validate_typecheck_ready(&hir).expect("try await expression is structured");

    let env = TypeCheckEnv::new().with_function(
        "load_bg",
        TypeKind::Need {
            ready: Box::new(TypeKind::Named("Image".to_owned())),
            error: Box::new(TypeKind::Named("AssetError".to_owned())),
        },
    );
    typecheck_hir(&hir, &env).expect("try await unwraps Need<T, E> to T");
}

#[test]
fn plain_await_expression_returns_result_in_ordinary_fn() {
    let tree = parse_ok(
        r"
fn load_bg_result() -> Result<Image, AssetError> {
    await load_bg()
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("ordinary function lowers");
    assert!(matches!(
        hir.functions()[0].value().map(AuthoredExpr::expr),
        Some(Expr::Await(awaited))
            if awaited.propagation() == AwaitPropagation::PreserveResult
    ));

    let env = TypeCheckEnv::new().with_function(
        "load_bg",
        TypeKind::Need {
            ready: Box::new(TypeKind::Named("Image".to_owned())),
            error: Box::new(TypeKind::Named("AssetError".to_owned())),
        },
    );
    typecheck_hir(&hir, &env).expect("plain await returns Result<T, E>");
}

#[test]
fn await_non_need_reports_typed_code_and_exact_operand_range() {
    let source = r"
fn bad() -> Result<i64, ArcError> {
    await 42
}
";
    let hir = lower_bound_hir("await-non-need", source);
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("non-Need await is rejected");
    let error = errors
        .iter()
        .find(|error| error.stable_code() == "sema.await.operand_not_need")
        .expect("typed await operand diagnostic");
    let operand_start = source.find("42").expect("fixture await operand");

    assert!(matches!(
        error.kind(),
        TypeCheckErrorKind::AwaitOperandNotNeed {
            actual: TypeKind::I32,
            ..
        }
    ));
    let diagnostic = error.diagnostic();
    let primary = diagnostic
        .labels()
        .iter()
        .find(|label| label.style() == DiagnosticLabelStyle::Primary)
        .expect("await diagnostic has primary source evidence");
    assert_eq!(
        primary.span().range().as_range(),
        operand_start..operand_start + "42".len()
    );
}

#[test]
fn await_thread_handle_uses_the_non_need_type_rule() {
    let source = r"
fn route_score(state: i32) -> i32 {
    state
}

fn bad(state: i32) -> Result<i64, ArcError> {
    await thread compute { route_score(state) }
}
";
    let hir = lower_bound_hir("await-thread-handle", source);
    let errors =
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("ThreadHandle await is rejected");
    let error = errors
        .iter()
        .find(|error| error.stable_code() == "sema.await.operand_not_need")
        .expect("typed await operand diagnostic");
    let operand = "thread compute { route_score(state) }";
    let operand_start = source.find(operand).expect("fixture thread operand");

    assert!(matches!(
        error.kind(),
        TypeCheckErrorKind::AwaitOperandNotNeed {
            actual: TypeKind::ThreadHandle(inner),
            ..
        } if inner.as_ref() == &TypeKind::Unit
    ));
    let diagnostic = error.diagnostic();
    let primary = diagnostic
        .labels()
        .iter()
        .find(|label| label.style() == DiagnosticLabelStyle::Primary)
        .expect("ThreadHandle diagnostic has primary source evidence");
    assert_eq!(
        primary.span().range().as_range(),
        operand_start..operand_start + operand.len()
    );
}

#[test]
fn await_with_non_need_uses_the_same_typed_diagnostic_without_fabricated_source() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    await 42 with { pending p => p }
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("await-with fixture lowers");
    let errors =
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("non-Need await-with is rejected");
    let error = errors
        .iter()
        .find(|error| error.stable_code() == "sema.await.operand_not_need")
        .expect("typed await operand diagnostic");

    assert!(matches!(
        error.kind(),
        TypeCheckErrorKind::AwaitOperandNotNeed {
            actual: TypeKind::I32,
            operand: None,
        }
    ));
    assert!(error.diagnostic().labels().is_empty());
}

#[test]
fn await_with_keeps_awaited_expression() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    try await load_opening_assets() with { pending p => progress.set(p.ratio) }
}
",
    );

    let Item::Flow(flow) = &tree.typed_tree().items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::AwaitWith(await_with) = &flow.body()[0] else {
        panic!("expected await with");
    };
    assert!(await_with.applies_try());
    assert!(matches!(await_with.expr(), Expr::Call(_)));
    let pending = await_with.pending().expect("pending branch");
    assert_eq!(pending.kind(), AwaitBranchKind::Pending);
    assert!(matches!(
        pending.body()[0],
        FlowItem::Stmt(Stmt::Expr {
            expr: Expr::Call(_),
            ..
        })
    ));
}

#[test]
fn await_with_keeps_wait_view_branches() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    await load_avatar() with {
        pending p => progress.set(p.ratio)
        ready img => Image(img)
        error _ => Icon(@asset:.avatar_fallback)
        denied _ => return Ok(FlowExit.Goto(@flow.title))
    }
}
",
    );

    let Item::Flow(flow) = &tree.typed_tree().items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::AwaitWith(await_with) = &flow.body()[0] else {
        panic!("expected await with");
    };
    assert_eq!(await_with.branches().len(), 4);
    assert!(matches!(
        await_with.branches()[0].kind(),
        AwaitBranchKind::Pending
    ));
    assert!(matches!(
        await_with.branches()[1].body()[0],
        FlowItem::Stmt(Stmt::Expr {
            expr: Expr::Call(_),
            ..
        })
    ));

    let hir =
        lower_document_to_hir(tree.document(), tree.typed_tree()).expect("await branches lower");
    assert!(matches!(
        &hir.flows()[0].body()[0],
        HirFlowItem::Await(await_with) if await_with.branches().len() == 4
    ));
}

#[test]
fn try_await_accepts_indented_with_block() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    try await asset.image(@asset:.bg.room) with:
        pending p:
            progress.set(p.ratio)
}
",
    );

    let Item::Flow(flow) = &tree.typed_tree().items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::AwaitWith(await_with) = &flow.body()[0] else {
        panic!("expected await with");
    };
    assert!(await_with.applies_try());
    assert!(matches!(await_with.expr(), Expr::Call(_)));
    let pending = await_with.pending().expect("pending branch");
    assert_eq!(pending.body().len(), 1);
    assert!(matches!(
        pending.body()[0],
        FlowItem::Stmt(Stmt::Expr {
            expr: Expr::Call(_),
            ..
        })
    ));
}

#[test]
fn await_question_prefix_is_try_await_sugar() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    await? asset.image(@asset:.bg.room) with { pending p => scene.show(@scene.loading) }
}
",
    );

    let Item::Flow(flow) = &tree.typed_tree().items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::AwaitWith(await_with) = &flow.body()[0] else {
        panic!("expected await with");
    };
    assert!(await_with.applies_try());
    assert!(matches!(await_with.expr(), Expr::Call(_)));
}

#[test]
fn let_try_await_with_binds_ready_value_and_keeps_wait_view() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    let assets = try await load_opening_assets() with { pending p => p.ratio ready loaded => loaded.ready }
    let count = assets.count
}
",
    );

    let Item::Flow(flow) = &tree.typed_tree().items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::LetAwait {
        pattern,
        await_with,
    }) = &flow.body()[0]
    else {
        panic!("expected let-await statement");
    };
    assert!(matches!(pattern, Pattern::Ident(name) if name == "assets"));
    assert!(await_with.applies_try());
    assert!(await_with.pending().is_some());

    let hir =
        lower_document_to_hir(tree.document(), tree.typed_tree()).expect("bound try-await lowers");
    assert!(matches!(
        &hir.flows()[0].body()[0],
        HirFlowItem::LetAwait {
            pattern: Pattern::Ident(name),
            await_with,
        } if name == "assets" && await_with.applies_try()
    ));
    validate_typecheck_ready(&hir).expect("bound try-await is typecheck-ready");

    let env = TypeCheckEnv::new().with_function(
        "load_opening_assets",
        TypeKind::Need {
            ready: Box::new(TypeKind::Named("OpeningAssets".to_owned())),
            error: Box::new(TypeKind::Named("AssetError".to_owned())),
        },
    );
    typecheck_hir(&hir, &env).expect("ready value and pending progress bind in scope");
}

#[test]
fn let_plain_await_with_binds_result_value() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    let result = await load_opening_assets() with:
        pending p:
            p.ratio
    let display = result
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("bound plain await lowers");

    let env = TypeCheckEnv::new().with_function(
        "load_opening_assets",
        TypeKind::Need {
            ready: Box::new(TypeKind::Named("OpeningAssets".to_owned())),
            error: Box::new(TypeKind::Named("AssetError".to_owned())),
        },
    );
    typecheck_hir(&hir, &env).expect("plain await binds Result<T, E>");
}

#[test]
fn await_with_variant_pending_pattern_binds_payload() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    try await run_activity() with { pending .Realizing(p) => p.ratio pending .Running(p) => p.ratio }
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("variant pending patterns lower");

    let env = TypeCheckEnv::new().with_function(
        "run_activity",
        TypeKind::Need {
            ready: Box::new(TypeKind::Named("ActivityOutput".to_owned())),
            error: Box::new(TypeKind::Named("ActivityError".to_owned())),
        },
    );
    typecheck_hir(&hir, &env).expect("variant payloads bind in wait-view branches");
}

#[test]
fn let_try_await_with_accepts_multiline_context_before_with() {
    let tree = parse_ok(
        r#"
flow @flow.loading loading {
    let bg = try await asset.image(@asset:.bg.room)
        .context("opening background failed")
    with:
        pending p:
            p.ratio
    let display = bg.id
}
"#,
    );

    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("multiline contextual try-await lowers");
    assert!(matches!(
        &hir.flows()[0].body()[0],
        HirFlowItem::LetAwait { await_with, .. }
            if selected_call_member(await_with.expr()) == Some("context")
    ));
    validate_typecheck_ready(&hir).expect("multiline contextual try-await is typecheck-ready");

    let need_type = TypeKind::Need {
        ready: Box::new(TypeKind::Named("Image".to_owned())),
        error: Box::new(TypeKind::Named("AssetError".to_owned())),
    };
    let env = TypeCheckEnv::new()
        .with_symbol("asset", TypeKind::Named("AssetApi".to_owned()))
        .with_method(
            TypeKind::Named("AssetApi".to_owned()),
            "image",
            need_type.clone(),
        )
        .with_method(need_type.clone(), "context", need_type);
    typecheck_hir(&hir, &env).expect("context-preserved try-await typechecks");
}

#[test]
fn let_parenthesized_await_with_question_is_try_sugar() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    let bg = (await asset.image(@asset:.bg.room) with:
        pending p:
            p.ratio
    )?
    let display = bg.id
}
",
    );

    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("parenthesized await-with lowers");
    assert!(matches!(
        &hir.flows()[0].body()[0],
        HirFlowItem::LetAwait { await_with, .. } if await_with.applies_try()
    ));
    let need_type = TypeKind::Need {
        ready: Box::new(TypeKind::Named("Image".to_owned())),
        error: Box::new(TypeKind::Named("AssetError".to_owned())),
    };
    let env = TypeCheckEnv::new()
        .with_symbol("asset", TypeKind::Named("AssetApi".to_owned()))
        .with_method(TypeKind::Named("AssetApi".to_owned()), "image", need_type);
    typecheck_hir(&hir, &env).expect("parenthesized await-with unwraps Result");
}

#[test]
fn let_parenthesized_await_with_context_after_block_typechecks() {
    let tree = parse_ok(
        r#"
flow @flow.loading loading {
    let bg = (await asset.image(@asset:.bg.room) with:
        pending p:
            p.ratio
    ).context("opening background failed")?
    let display = bg.id
}
"#,
    );

    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("post-await context lowers");
    assert!(matches!(
        &hir.flows()[0].body()[0],
        HirFlowItem::LetAwait { await_with, .. }
            if await_with.applies_try()
                && selected_call_member(await_with.expr()) == Some("context")
    ));
    let need_type = TypeKind::Need {
        ready: Box::new(TypeKind::Named("Image".to_owned())),
        error: Box::new(TypeKind::Named("AssetError".to_owned())),
    };
    let env = TypeCheckEnv::new()
        .with_symbol("asset", TypeKind::Named("AssetApi".to_owned()))
        .with_method(
            TypeKind::Named("AssetApi".to_owned()),
            "image",
            need_type.clone(),
        )
        .with_method(need_type.clone(), "context", need_type);
    typecheck_hir(&hir, &env).expect("post-await context remains structured");
}

#[test]
fn let_try_await_without_wait_view_stays_expression_await() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    let bg = try await load_bg()
}
",
    );
    let Item::Flow(flow) = &tree.typed_tree().items()[0] else {
        panic!("expected flow");
    };
    assert!(matches!(
        &flow.body()[0],
        FlowItem::Stmt(Stmt::Let {
            expr: Expr::Await(awaited),
            ..
        }) if awaited.propagation() == AwaitPropagation::PropagateError
    ));
}

#[test]
fn await_question_with_is_rejected_as_ambiguous() {
    let errors = parse_errors(
        r"
flow @flow.loading loading {
    await load_opening_assets()? with { pending p => scene.show(@scene.loading) }
}
",
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("await expr? with"))
    );
}

#[test]
fn typecheck_rejects_borrow_across_await_boundary() {
    let tree = parse_ok(
        r"
flow @flow.borrow borrow {
    let pixels: &'asset [Rgba8] = bg.pixels()
    try await load_avatar() with { pending p => progress.set(p.ratio) }
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("borrow across await fixture lowers");
    let env = TypeCheckEnv::standard()
        .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
        .with_method(
            TypeKind::Named("ImageHandle".to_owned()),
            "pixels",
            TypeKind::Named("Pixels".to_owned()),
        )
        .with_function(
            "load_avatar",
            TypeKind::Need {
                ready: Box::new(TypeKind::Unit),
                error: Box::new(TypeKind::Named("AssetError".to_owned())),
            },
        );
    let errors = typecheck_hir(&hir, &env).expect_err("borrow cannot cross await");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("suspension boundary"))
    );
}

#[test]
fn borrow_across_direct_await_reports_typed_code_and_exact_keyword_range() {
    let source = r"
flow @flow.borrow borrow {
    let pixels: &'asset [Rgba8] = bg.pixels()
    let result = await load_avatar()
}
";
    let hir = lower_bound_hir("borrow-across-direct-await", source);
    let pixel_borrow = TypeKind::BorrowRef {
        kind: BorrowKind::Shared,
        lifetime: Some(LifetimeScopeKind::Named("asset".to_owned())),
        inner: Box::new(TypeKind::Slice(Box::new(TypeKind::Named(
            "Rgba8".to_owned(),
        )))),
    };
    let env = TypeCheckEnv::standard()
        .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
        .with_method(
            TypeKind::Named("ImageHandle".to_owned()),
            "pixels",
            pixel_borrow,
        )
        .with_function(
            "load_avatar",
            TypeKind::Need {
                ready: Box::new(TypeKind::Unit),
                error: Box::new(TypeKind::Named("AssetError".to_owned())),
            },
        );
    let errors = typecheck_hir(&hir, &env).expect_err("borrow cannot cross direct await");
    let error = errors
        .iter()
        .find(|error| error.stable_code() == "sema.suspend.borrow_across")
        .expect("typed borrow-across-suspension diagnostic");
    let await_start = source.find("await").expect("fixture await keyword");

    assert!(matches!(
        error.kind(),
        TypeCheckErrorKind::BorrowAcrossSuspension {
            lifetimes,
            boundary,
            ..
        } if lifetimes == &["asset".to_owned()] && boundary == "await suspension boundary"
    ));
    let diagnostic = error.diagnostic();
    let primary = diagnostic
        .labels()
        .iter()
        .find(|label| label.style() == DiagnosticLabelStyle::Primary)
        .expect("borrow diagnostic has primary source evidence");
    assert_eq!(
        primary.span().range().as_range(),
        await_start..await_start + "await".len()
    );
}

#[test]
fn typecheck_allows_explicit_drop_before_await_boundary() {
    let tree = parse_ok(
        r"
flow @flow.borrow borrow {
    let pixels: &'asset [Rgba8] = bg.pixels()
    drop(pixels)
    try await load_avatar() with { pending p => progress.set(p.ratio) }
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("explicit drop borrow fixture lowers");
    let pixel_borrow = TypeKind::BorrowRef {
        kind: BorrowKind::Shared,
        lifetime: Some(LifetimeScopeKind::Named("asset".to_owned())),
        inner: Box::new(TypeKind::Slice(Box::new(TypeKind::Named(
            "Rgba8".to_owned(),
        )))),
    };
    let env = TypeCheckEnv::standard()
        .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
        .with_method(
            TypeKind::Named("ImageHandle".to_owned()),
            "pixels",
            pixel_borrow,
        )
        .with_function(
            "load_avatar",
            TypeKind::Need {
                ready: Box::new(TypeKind::Unit),
                error: Box::new(TypeKind::Named("AssetError".to_owned())),
            },
        );

    typecheck_hir(&hir, &env).expect("explicit drop ends borrow before await");
}

#[test]
fn typecheck_rejects_conditional_drop_before_await_boundary() {
    let tree = parse_ok(
        r"
flow @flow.borrow borrow {
    let pixels: &'asset [Rgba8] = bg.pixels()
    if should_drop {
        drop(pixels)
    }
    try await load_avatar() with { pending p => progress.set(p.ratio) }
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("conditional drop borrow fixture lowers");
    let pixel_borrow = TypeKind::BorrowRef {
        kind: BorrowKind::Shared,
        lifetime: Some(LifetimeScopeKind::Named("asset".to_owned())),
        inner: Box::new(TypeKind::Slice(Box::new(TypeKind::Named(
            "Rgba8".to_owned(),
        )))),
    };
    let env = TypeCheckEnv::standard()
        .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
        .with_symbol("should_drop", TypeKind::Bool)
        .with_method(
            TypeKind::Named("ImageHandle".to_owned()),
            "pixels",
            pixel_borrow,
        )
        .with_function(
            "load_avatar",
            TypeKind::Need {
                ready: Box::new(TypeKind::Unit),
                error: Box::new(TypeKind::Named("AssetError".to_owned())),
            },
        );

    let errors = typecheck_hir(&hir, &env).expect_err("conditional drop is not enough");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("suspension boundary"))
    );
}

#[test]
fn typecheck_allows_match_when_every_arm_drops_before_await_boundary() {
    let tree = parse_ok(
        r"
flow @flow.borrow borrow {
    let pixels: &'asset [Rgba8] = bg.pixels()
    match mode {
        .Fast => drop(pixels)
        _ => drop(pixels)
    }
    try await load_avatar() with { pending p => progress.set(p.ratio) }
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("all-arm drop fixture lowers");
    let pixel_borrow = TypeKind::BorrowRef {
        kind: BorrowKind::Shared,
        lifetime: Some(LifetimeScopeKind::Named("asset".to_owned())),
        inner: Box::new(TypeKind::Slice(Box::new(TypeKind::Named(
            "Rgba8".to_owned(),
        )))),
    };
    let env = TypeCheckEnv::standard()
        .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
        .with_symbol("mode", TypeKind::Named("Mode".to_owned()))
        .with_method(
            TypeKind::Named("ImageHandle".to_owned()),
            "pixels",
            pixel_borrow,
        )
        .with_function(
            "load_avatar",
            TypeKind::Need {
                ready: Box::new(TypeKind::Unit),
                error: Box::new(TypeKind::Named("AssetError".to_owned())),
            },
        );

    typecheck_hir(&hir, &env).expect("all match arms end borrow before await");
}

#[test]
fn typecheck_rejects_use_after_explicit_borrow_drop() {
    let tree = parse_ok(
        r"
flow @flow.borrow borrow {
    let pixels: &'asset [Rgba8] = bg.pixels()
    drop(pixels)
    let again = pixels
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("use-after-drop fixture lowers");
    let pixel_borrow = TypeKind::BorrowRef {
        kind: BorrowKind::Shared,
        lifetime: Some(LifetimeScopeKind::Named("asset".to_owned())),
        inner: Box::new(TypeKind::Slice(Box::new(TypeKind::Named(
            "Rgba8".to_owned(),
        )))),
    };
    let env = TypeCheckEnv::standard()
        .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
        .with_method(
            TypeKind::Named("ImageHandle".to_owned()),
            "pixels",
            pixel_borrow,
        );

    let errors = typecheck_hir(&hir, &env).expect_err("dropped borrow local cannot be reused");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("used after it was dropped"))
    );
}

#[test]
fn typechecks_await_wait_view_branches() {
    let tree = parse_ok(
        r"
enum FlowExit { Goto(Ref<Flow>) }
struct AvatarError {}

flow @flow.loading loading() -> Result<FlowExit, AvatarError> {
    try await load_avatar() with {
        pending p => progress.set(p.ratio)
        ready img => Image(img)
        error _ => Icon(@asset:.avatar_fallback)
        denied _ => return Ok(FlowExit.Goto(@flow.title))
    }
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("await branch typecheck fixture lowers");
    let env = TypeCheckEnv::new()
        .with_function(
            "load_avatar",
            TypeKind::Need {
                ready: Box::new(TypeKind::Named("Image".to_owned())),
                error: Box::new(TypeKind::Named("AvatarError".to_owned())),
            },
        )
        .with_function("Image", TypeKind::Named("View".to_owned()))
        .with_function("Icon", TypeKind::Named("View".to_owned()))
        .with_function("FlowExit.Goto", TypeKind::Named("FlowExit".to_owned()))
        .with_symbol("img", TypeKind::Named("Image".to_owned()));

    typecheck_hir(&hir, &env).expect("await wait-view branches typecheck");
}
