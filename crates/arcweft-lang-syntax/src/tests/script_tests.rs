use super::support::*;

#[test]
fn parses_top_level_test_and_bench_items() {
    let tree = parse_ok(
        r#"
test @test.opening scenario {
    start @flow.opening
    expect no_assertion_failures
}

bench @bench.opening {
    setup { let state = fixture<GameState>("opening.json") }
    measure iterations = 10 { opening_choices() }
}
"#,
    );

    assert!(matches!(
        &tree.items()[0],
        Item::Test(item)
            if item.id().body() == "test.opening"
                && item.kind() == &TestKind::Scenario
                && item.body().contains("start @flow.opening")
    ));
    assert!(matches!(
        &tree.items()[1],
        Item::Bench(item)
            if item.id().body() == "bench.opening"
                && item.body().contains("measure iterations")
    ));
}

#[test]
fn lowers_test_and_bench_to_hir_declarations() {
    let tree = parse_ok(
        r#"
test @test.choice visual {
    capture image overlay as "choice.png"
}

bench @bench.choice {
    report { cpu_time }
}
"#,
    );

    let hir = lower_to_hir(&tree).expect("test and bench lower to HIR declarations");
    assert!(matches!(
        &hir.declarations()[0],
        HirTopLevelDecl::Test(item) if item.kind() == &TestKind::Visual
    ));
    assert!(matches!(
        &hir.declarations()[1],
        HirTopLevelDecl::Bench(item) if item.id().body() == "bench.choice"
    ));
}

#[test]
fn parses_family_relative_test_and_bench_ids() {
    let tree = parse_ok(
        r"
test @test:.opening scenario {
    start @flow.opening
}

bench @bench:.opening {
    measure iterations = 1 { opening_choices() }
}
",
    );

    assert!(matches!(
        &tree.items()[0],
        Item::Test(item)
            if item.id().is_relative()
                && item.id().family_relative_ref().is_some_and(|id| id.family() == "test")
                && item.id().body() == "opening"
    ));
    assert!(matches!(
        &tree.items()[1],
        Item::Bench(item)
            if item.id().is_relative()
                && item.id().family_relative_ref().is_some_and(|id| id.family() == "bench")
                && item.id().body() == "opening"
    ));
}

#[test]
fn rejects_test_without_kind() {
    let errors = parse_errors(
        r"
test @test.missing_kind {
}
",
    );

    assert!(errors[0].message().contains("missing a test kind"));
}
