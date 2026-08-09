use core::fmt::Write as _;

use arcweft_lang_syntax::incremental::{ParseFailure, SyntaxLimit};

use super::*;

const SHALLOW_SELECT: &str = "a.b.c.d";
const INVALID_USE: &str = "use a as\n";
const PADDED_SELECT_PREFIX: &str = "fn lower_expressions() {\n    let value = target.member;\n}\n";

fn parse_text(document_id: &str, source: String) -> Result<ParsedSource, ParseFailure> {
    let name = SourceName::path(format!(
        "proof/expression-lowering/select-limits/{document_id}.arcw"
    ));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/expression-lowering/select-limits/{document_id}.arcw"
            ))
            .expect("Select-limit document ID"),
            name.clone(),
            source,
        )
        .expect("Select-limit source document"),
    );
    SyntaxDatabase::try_new()
        .expect("Select-limit syntax database")
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
}

fn shallow_select_source(statement_count: usize, terminal: &str) -> String {
    shallow_select_batch_source(statement_count, SHALLOW_SELECT, terminal)
}

fn shallow_select_batch_source(statement_count: usize, repeated: &str, terminal: &str) -> String {
    assert!(statement_count > 0);
    let estimated_statement_bytes = "    let value = ;\n"
        .len()
        .checked_add(repeated.len())
        .expect("shallow Select statement size");
    let mut source = String::with_capacity(
        estimated_statement_bytes
            .checked_mul(statement_count)
            .and_then(|bytes| bytes.checked_add(40))
            .expect("shallow Select source size"),
    );
    source.push_str("fn lower_expressions() {\n");
    for _ in 1..statement_count {
        writeln!(&mut source, "    let value = {repeated};")
            .expect("writing to a String cannot fail");
    }
    writeln!(&mut source, "    let value = {terminal};").expect("writing to a String cannot fail");
    source.push_str("}\n");
    source
}

fn select_chain(expression_count: usize) -> String {
    assert!(expression_count > 0);
    let mut expression = String::with_capacity(expression_count.saturating_mul(2));
    expression.push('a');
    for _ in 1..expression_count {
        expression.push_str(".b");
    }
    expression
}

fn hir_expression_boundary_fixture(extra_select_depth: usize) -> Vec<String> {
    let statement_maximum = SyntaxLimit::Statements.maximum();
    assert!(statement_maximum >= 2);
    let mut expressions = Vec::with_capacity(statement_maximum);
    expressions.push(select_chain(6 + extra_select_depth));
    expressions.extend(core::iter::repeat_n(
        SHALLOW_SELECT.to_owned(),
        statement_maximum - 2,
    ));
    // Parentheses keep `scope {}` in the ordinary let-initializer route; they
    // are transparent to attached semantic identity and add no expression.
    expressions.push("(scope {})".to_owned());
    expressions
}

fn attached_expression_count(expression: &AttachedExpressionNode) -> usize {
    expression
        .children()
        .iter()
        .filter_map(|child| {
            child
                .authored_semantic()
                .expect("shallow Select child attachment")
        })
        .map(|child| attached_expression_count(&child))
        .sum::<usize>()
        + 1
}

fn expression_inventory_len(module: &HirModule) -> usize {
    module
        .arenas()
        .expressions()
        .try_iter(module.slots())
        .expect("published expression inventory")
        .count()
}

fn recovery_diagnostic_owners(module: &HirModule) -> Vec<SyntheticOwner> {
    module
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            HirDiagnostic::Recovery(diagnostic) => Some(diagnostic.owner()),
            HirDiagnostic::Syntax(_) | HirDiagnostic::LineIdentity(_) => None,
        })
        .collect()
}

fn select_payload(module: &HirModule, owner: ExprId) -> &HirSelectExpr {
    let HirExprKind::Select(select) = expression(module, owner).kind() else {
        panic!("Select-limit fixture must publish a Select expression");
    };
    select
}

fn diagnostic_revisions(document_id: &str) -> (ParsedSource, ParsedSource) {
    let name = SourceName::path(format!(
        "proof/expression-lowering/select-limits/{document_id}.arcw"
    ));
    let source = format!(
        "fn lower_expressions() {{\n    let value = target.;\n}}\n{}",
        INVALID_USE.repeat(SyntaxLimit::Diagnostics.maximum())
    );
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/expression-lowering/select-limits/{document_id}.arcw"
            ))
            .expect("diagnostic retry document ID"),
            name.clone(),
            source,
        )
        .expect("diagnostic retry document"),
    );
    let mut syntax = SyntaxDatabase::try_new().expect("diagnostic retry syntax database");
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("exact diagnostic prefill parses");
    let end = initial.document().text().len();
    let start = end
        .checked_sub(INVALID_USE.len())
        .expect("one invalid use row is present");
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(start, end))
                    .expect("last invalid use row span"),
                "",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("smaller diagnostic prefill reparses");
    (initial, revised)
}

fn name_limit_revisions(document_id: &str) -> (ParsedSource, ParsedSource) {
    let maximum = HirLimit::NameBytes.maximum();
    let name = SourceName::path(format!(
        "proof/expression-lowering/select-limits/{document_id}.arcw"
    ));
    let prefix = "fn lower_expressions() {\n    let value = target.";
    let member = "a".repeat(maximum + 1);
    let source = format!("{prefix}{member};\n}}\n");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/expression-lowering/select-limits/{document_id}.arcw"
            ))
            .expect("name retry document ID"),
            name.clone(),
            source,
        )
        .expect("name retry document"),
    );
    let mut syntax = SyntaxDatabase::try_new().expect("name retry syntax database");
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("one-over name remains valid syntax");
    let removed_start = prefix.len().checked_add(maximum).expect("name edit offset");
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(removed_start, removed_start + 1))
                    .expect("last member byte span"),
                "",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("exact name reparses");
    (initial, revised)
}

fn padded_select_source(size: usize) -> String {
    let mut source = PADDED_SELECT_PREFIX.to_owned();
    let padding = size
        .checked_sub(source.len())
        .expect("source-byte fixture exceeds its fixed Select prefix");
    source.push_str(&" ".repeat(padding));
    assert_eq!(source.len(), size);
    source
}

fn expression_revision_source(expressions: &[&str]) -> String {
    let mut source = String::from("fn lower_expressions() {\n");
    for expression in expressions {
        writeln!(&mut source, "    let value = {expression};")
            .expect("writing to a String cannot fail");
    }
    source.push_str("}\n");
    source
}

fn initial_expression_revision(
    document_id: &str,
    expressions: &[&str],
) -> (SyntaxDatabase, ParsedSource) {
    initial_source_revision(document_id, expression_revision_source(expressions))
}

fn initial_source_revision(document_id: &str, source: String) -> (SyntaxDatabase, ParsedSource) {
    let name = SourceName::path(format!(
        "proof/expression-lowering/select-limits/{document_id}.arcw"
    ));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/expression-lowering/select-limits/{document_id}.arcw"
            ))
            .expect("slot-limit document ID"),
            name.clone(),
            source,
        )
        .expect("slot-limit source document"),
    );
    let mut syntax = SyntaxDatabase::try_new().expect("slot-limit syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("initial slot-limit revision parses");
    (syntax, parsed)
}

fn reparse_expression_revision(
    syntax: &mut SyntaxDatabase,
    previous: &ParsedSource,
    expressions: &[&str],
) -> ParsedSource {
    reparse_source_revision(syntax, previous, expression_revision_source(expressions))
}

fn reparse_source_revision(
    syntax: &mut SyntaxDatabase,
    previous: &ParsedSource,
    source: String,
) -> ParsedSource {
    syntax
        .reparse(
            previous,
            &[SourceEdit::new(
                previous
                    .document()
                    .span(SourceRange::new(0, previous.document().text().len()))
                    .expect("whole prior slot-limit revision"),
                source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("next slot-limit revision reparses")
}

fn publish_slot_limited_revision(
    database: &mut HirDatabase,
    parsed: &ParsedSource,
    total_slot_maximum: usize,
) -> Arc<HirModule> {
    let attached = attached_expressions(parsed);
    let mut transaction = stage(database, parsed);
    transaction
        .storage_mut()
        .0
        .set_total_slot_maximum_for_test(total_slot_maximum);
    let scope = allocate_module_scope(&mut transaction, parsed);
    for expression in &attached {
        transaction
            .lower_attached_expression(expression, scope)
            .expect("bounded real expression batch lowers");
    }
    transaction
        .finish(database)
        .expect("bounded slot revision publishes")
        .into_module()
}

fn publish_production_revision(
    database: &mut HirDatabase,
    parsed: &ParsedSource,
) -> Arc<HirModule> {
    let attached = attached_expressions(parsed);
    let mut transaction = stage(database, parsed);
    let scope = allocate_module_scope(&mut transaction, parsed);
    for expression in &attached {
        transaction
            .lower_attached_expression(expression, scope)
            .expect("production-sized real expression batch lowers");
    }
    transaction
        .finish(database)
        .expect("production-sized slot revision publishes")
        .into_module()
}

fn exercise_lowered_total_slot_case(third_batch: &[&str], prefill: usize, exact: bool) {
    const LOWERED_MAXIMUM: usize = 14;
    let (mut syntax, mut parsed) =
        initial_expression_revision("total-slots-lowered", &["a.b", "c.d"]);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().expect("lowered total-slot database");

    let first = publish_slot_limited_revision(&mut database, &parsed, LOWERED_MAXIMUM);
    assert_eq!(expression_inventory_len(&first), 4);
    assert_eq!(first.slots().committed_slot_count(), 5);
    drop(first);

    parsed = reparse_expression_revision(&mut syntax, &parsed, &[]);
    let first_retired = publish_slot_limited_revision(&mut database, &parsed, LOWERED_MAXIMUM);
    assert_eq!(expression_inventory_len(&first_retired), 0);
    assert_eq!(first_retired.slots().committed_slot_count(), 5);
    drop(first_retired);

    parsed = reparse_expression_revision(&mut syntax, &parsed, &["e.f", "g.h"]);
    let second = publish_slot_limited_revision(&mut database, &parsed, LOWERED_MAXIMUM);
    assert_eq!(expression_inventory_len(&second), 4);
    assert_eq!(second.slots().committed_slot_count(), 9);
    drop(second);

    parsed = reparse_expression_revision(&mut syntax, &parsed, &[]);
    let second_retired = publish_slot_limited_revision(&mut database, &parsed, LOWERED_MAXIMUM);
    assert_eq!(expression_inventory_len(&second_retired), 0);
    assert_eq!(second_retired.slots().committed_slot_count(), 9);
    drop(second_retired);

    parsed = reparse_expression_revision(&mut syntax, &parsed, third_batch);
    let third = publish_slot_limited_revision(&mut database, &parsed, LOWERED_MAXIMUM);
    assert_eq!(third.slots().committed_slot_count(), prefill);
    drop(third);

    parsed = reparse_expression_revision(&mut syntax, &parsed, &[]);
    let retired = publish_slot_limited_revision(&mut database, &parsed, LOWERED_MAXIMUM);
    assert_eq!(expression_inventory_len(&retired), 0);
    assert_eq!(retired.slots().committed_slot_count(), prefill);
    let accepted_before_direct = Arc::clone(&retired);
    drop(retired);

    parsed = reparse_expression_revision(&mut syntax, &parsed, &["target.member"]);
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut transaction = stage(&database, &parsed);
    transaction
        .storage_mut()
        .0
        .set_total_slot_maximum_for_test(LOWERED_MAXIMUM);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    if exact {
        let owner = transaction
            .lower_attached_expression(&attached, scope)
            .expect("two-slot direct Select reaches exact TotalSlotsPerModule");
        let module = transaction
            .finish(&mut database)
            .expect("exact total-slot Select publishes")
            .into_module();
        assert_eq!(module.slots().committed_slot_count(), LOWERED_MAXIMUM);
        assert_eq!(expression_inventory_len(&module), 2);
        assert!(matches!(
            select_payload(&module, owner).member(),
            HirSelectedMember::Name(name) if name.as_str() == "member"
        ));
    } else {
        assert!(matches!(
            transaction.lower_attached_expression(&attached, scope),
            Err(HirLowerFailure::Limit(error))
                if error.limit() == HirLimit::TotalSlotsPerModule
                    && error.observed() == LOWERED_MAXIMUM + 1
                    && error.maximum() == LOWERED_MAXIMUM
        ));
        assert!(transaction.finish(&mut database).is_err());
        let current = database
            .current_lineage(&key)
            .expect("retired revision stays current");
        assert!(Arc::ptr_eq(&current, &accepted_before_direct));
        assert_eq!(current.slots().committed_slot_count(), prefill);
        assert_eq!(expression_inventory_len(&current), 0);
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "this helper owns one production-sized exact/one-over slot-limit transaction and its rollback evidence"
)]
fn exercise_production_total_slot_case(document_id: &str, third_terminal: &str, exact: bool) {
    let expression_maximum = HirLimit::Expressions.maximum();
    let slot_maximum = HirLimit::TotalSlotsPerModule.maximum();
    let statement_maximum = SyntaxLimit::Statements.maximum();
    assert_eq!(
        expression_maximum
            .checked_mul(3)
            .expect("three production expression batches"),
        slot_maximum
    );
    assert_eq!(
        statement_maximum
            .checked_mul(4)
            .expect("four expressions per shallow Select"),
        expression_maximum
    );

    let (mut syntax, mut parsed) = initial_source_revision(
        document_id,
        shallow_select_batch_source(statement_maximum, "a.b.c.d", "a.b.c.d"),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().expect("production total-slot database");

    let first = publish_production_revision(&mut database, &parsed);
    assert_eq!(expression_inventory_len(&first), expression_maximum);
    assert_eq!(first.slots().committed_slot_count(), expression_maximum + 1);
    drop(first);

    parsed = reparse_source_revision(&mut syntax, &parsed, expression_revision_source(&[]));
    let first_retired = publish_production_revision(&mut database, &parsed);
    assert_eq!(expression_inventory_len(&first_retired), 0);
    assert_eq!(
        first_retired.slots().committed_slot_count(),
        expression_maximum + 1
    );
    drop(first_retired);

    parsed = reparse_source_revision(
        &mut syntax,
        &parsed,
        shallow_select_batch_source(statement_maximum, "e.f.g.h", "e.f.g.h"),
    );
    let second = publish_production_revision(&mut database, &parsed);
    assert_eq!(expression_inventory_len(&second), expression_maximum);
    assert_eq!(
        second.slots().committed_slot_count(),
        expression_maximum
            .checked_mul(2)
            .and_then(|slots| slots.checked_add(1))
            .expect("two full batches plus the reused module scope")
    );
    drop(second);

    parsed = reparse_source_revision(&mut syntax, &parsed, expression_revision_source(&[]));
    let second_retired = publish_production_revision(&mut database, &parsed);
    assert_eq!(expression_inventory_len(&second_retired), 0);
    assert_eq!(
        second_retired.slots().committed_slot_count(),
        expression_maximum * 2 + 1
    );
    drop(second_retired);

    parsed = reparse_source_revision(
        &mut syntax,
        &parsed,
        shallow_select_batch_source(statement_maximum, "i.j.k.l", third_terminal),
    );
    let third_attached_count = attached_expressions(&parsed)
        .iter()
        .map(attached_expression_count)
        .sum::<usize>();
    let expected_third = if exact {
        expression_maximum - 3
    } else {
        expression_maximum - 2
    };
    assert_eq!(third_attached_count, expected_third);
    let expected_prefill = if exact {
        slot_maximum - 2
    } else {
        slot_maximum - 1
    };
    let third = publish_production_revision(&mut database, &parsed);
    assert_eq!(expression_inventory_len(&third), expected_third);
    assert_eq!(third.slots().committed_slot_count(), expected_prefill);
    drop(third);

    parsed = reparse_source_revision(&mut syntax, &parsed, expression_revision_source(&[]));
    let retired = publish_production_revision(&mut database, &parsed);
    assert_eq!(expression_inventory_len(&retired), 0);
    assert_eq!(retired.slots().committed_slot_count(), expected_prefill);
    let accepted_before_direct = Arc::clone(&retired);
    drop(retired);

    parsed = reparse_expression_revision(&mut syntax, &parsed, &["target.member"]);
    let attached = attached_expressions(&parsed)
        .pop()
        .expect("direct production Select");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    if exact {
        let owner = transaction
            .lower_attached_expression(&attached, scope)
            .expect("direct Path+Select reaches the exact production total-slot maximum");
        let module = transaction
            .finish(&mut database)
            .expect("exact production total-slot revision publishes")
            .into_module();
        assert_eq!(module.slots().committed_slot_count(), slot_maximum);
        assert_eq!(expression_inventory_len(&module), 2);
        assert!(matches!(
            select_payload(&module, owner).member(),
            HirSelectedMember::Name(name) if name.as_str() == "member"
        ));
    } else {
        assert!(matches!(
            transaction.lower_attached_expression(&attached, scope),
            Err(HirLowerFailure::Limit(error))
                if error.limit() == HirLimit::TotalSlotsPerModule
                    && error.observed() == slot_maximum + 1
                    && error.maximum() == slot_maximum
        ));
        assert!(transaction.finish(&mut database).is_err());
        let current = database
            .current(&key)
            .expect("retired prefill revision stays current");
        assert!(Arc::ptr_eq(&current, &accepted_before_direct));
        assert_eq!(current.slots().committed_slot_count(), expected_prefill);
        assert_eq!(expression_inventory_len(&current), 0);
    }
}

#[test]
fn shallow_select_fixture_accounting_matches_attached_and_hir_counts() {
    let parsed = parsed_source(
        "select-limit-accounting-smoke",
        &[
            select_chain(6),
            SHALLOW_SELECT.to_owned(),
            "(scope {})".to_owned(),
        ],
    );
    let attached = attached_expressions(&parsed);
    assert_eq!(
        attached
            .iter()
            .map(attached_expression_count)
            .sum::<usize>(),
        11
    );

    let (module, _, _) = lower_and_publish(&parsed);
    assert_eq!(expression_inventory_len(&module), 12);
    assert_eq!(module.status(), HirModuleStatus::Clean);
}

#[test]
fn total_slot_limit_is_inclusive_and_atomic() {
    assert_eq!(HirLimit::TotalSlotsPerModule.maximum(), 786_432);
    // One reused module scope plus 4 + 4 + 3/4 fresh expression slots gives
    // prefill 12/13. The direct Path+Select pair then reaches 14 or observes
    // 15 through the ordinary slot allocator; no counter is seeded.
    exercise_lowered_total_slot_case(&["i.j.k"], 12, true);
    exercise_lowered_total_slot_case(&["i.j", "k.l"], 13, false);
}

#[test]
#[ignore = "Tier 2: retains three production expression generations (estimated 4-8 GiB, 5-15 minutes)"]
fn e13_tier2_total_slots_exact_uses_three_retired_production_batches() {
    exercise_production_total_slot_case("total-slots-production-exact", "i", true);
}

#[test]
#[ignore = "Tier 2: retains three production expression generations (estimated 4-8 GiB, 5-15 minutes)"]
fn e13_tier2_total_slots_one_over_rolls_back_the_direct_select() {
    exercise_production_total_slot_case("total-slots-production-one-over", "i.j", false);
}

#[test]
#[ignore = "Tier 2: parses the production Syntax Expressions exact/one-over boundary"]
fn e13_tier2_syntax_expression_limit_uses_a_shallow_select_matrix() {
    let statement_maximum = SyntaxLimit::Statements.maximum();
    let expression_maximum = SyntaxLimit::Expressions.maximum();
    assert_eq!(
        statement_maximum
            .checked_mul(4)
            .expect("Select expression accounting"),
        expression_maximum
    );
    assert!(
        statement_maximum
            .checked_mul(12)
            .expect("conservative identity accounting")
            < SyntaxLimit::IdentityBearingNodes.maximum()
    );

    let exact_source = shallow_select_source(statement_maximum, SHALLOW_SELECT);
    let one_over_source = shallow_select_source(statement_maximum, "a.b.c.d.e");
    assert!(
        one_over_source.len() < HirLimit::SourceDocumentBytes.maximum(),
        "the expression limit must precede the source-byte limit"
    );

    let exact = parse_text("syntax-expressions-exact", exact_source)
        .expect("four shallow expressions per statement reach the inclusive maximum");
    assert_eq!(statements(&exact).len(), statement_maximum);
    assert_eq!(
        attached_expressions(&exact)
            .iter()
            .map(attached_expression_count)
            .sum::<usize>(),
        expression_maximum
    );

    let one_over = parse_text("syntax-expressions-one-over", one_over_source);
    assert!(matches!(
        one_over,
        Err(ParseFailure::LimitExceeded(SyntaxLimit::Expressions))
    ));
}

#[test]
#[ignore = "Tier 2: lowers the production HIR Expressions exact/one-over boundary"]
fn e13_tier2_hir_expression_limit_uses_a_shallow_select_matrix_and_scope_tail() {
    let maximum = HirLimit::Expressions.maximum();
    assert_eq!(maximum, SyntaxLimit::Expressions.maximum());
    let scope_slots = 2_usize; // the module scope plus the authored `scope {}` block scope
    assert!(
        maximum
            .checked_add(1)
            .and_then(|expressions| expressions.checked_add(scope_slots))
            .expect("one-over expressions plus both scopes")
            <= HirLimit::TotalSlotsPerModule.maximum(),
        "the HIR Expressions failure must precede TotalSlotsPerModule"
    );

    let exact_fixture = hir_expression_boundary_fixture(0);
    assert_eq!(exact_fixture.len(), SyntaxLimit::Statements.maximum());
    let exact = parsed_source("hir-expressions-exact", &exact_fixture);
    let exact_authored = attached_expressions(&exact)
        .iter()
        .map(attached_expression_count)
        .sum::<usize>();
    assert_eq!(exact_authored, maximum - 1);
    assert_eq!(
        exact_authored
            .checked_add(1)
            .expect("one implicit scope tail"),
        maximum,
        "authored syntax plus the sole implicit scope tail reaches exact HIR Expressions"
    );
    let (module, _, _) = lower_and_publish(&exact);
    assert_eq!(expression_inventory_len(&module), maximum);

    let one_over_fixture = hir_expression_boundary_fixture(1);
    assert_eq!(one_over_fixture.len(), SyntaxLimit::Statements.maximum());
    let one_over = parsed_source("hir-expressions-one-over", &one_over_fixture);
    let one_over_authored = attached_expressions(&one_over)
        .iter()
        .map(attached_expression_count)
        .sum::<usize>();
    assert_eq!(one_over_authored, SyntaxLimit::Expressions.maximum());
    assert_eq!(
        one_over_authored
            .checked_add(1)
            .expect("one implicit scope tail"),
        maximum + 1,
        "syntax stays exact while the sole implicit scope tail is HIR one-over"
    );
    let attached = attached_expressions(&one_over);
    let mut database = HirDatabase::try_new().expect("HIR expression-limit database");
    let mut transaction = stage(&database, &one_over);
    let scope = allocate_module_scope(&mut transaction, &one_over);
    for expression in &attached[..attached.len() - 1] {
        transaction
            .lower_attached_expression(expression, scope)
            .expect("shallow Select prefix remains inside HIR Expressions");
    }
    assert!(matches!(
        transaction.lower_attached_expression(attached.last().unwrap(), scope),
        Err(HirLowerFailure::Limit(error))
            if error.limit() == HirLimit::Expressions
                && error.observed() == maximum + 1
                && error.maximum() == maximum
    ));
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&one_over)).is_none());
}

#[test]
#[ignore = "Tier 2: parses and lowers an exact 8 MiB Select source snapshot"]
fn e13_tier2_hir_source_byte_preflight_retains_the_select_pipeline() {
    let maximum = HirLimit::SourceDocumentBytes.maximum();
    let exact_source = padded_select_source(maximum);
    let one_over_source = padded_select_source(maximum + 1);
    assert_eq!(
        exact_source.len(),
        PADDED_SELECT_PREFIX.len() + (maximum - PADDED_SELECT_PREFIX.len())
    );
    assert_eq!(
        one_over_source.len(),
        PADDED_SELECT_PREFIX.len() + (maximum + 1 - PADDED_SELECT_PREFIX.len())
    );

    let exact = parse_text("source-bytes-exact", exact_source).expect("exact source bytes parse");
    assert_eq!(exact.document().text().len(), maximum);
    let (module, owners, _) = lower_and_publish(&exact);
    assert_eq!(
        module.provenance().source_identity().source_len(),
        maximum as u64
    );
    assert!(matches!(
        select_payload(&module, owners[0]).member(),
        HirSelectedMember::Name(name) if name.as_str() == "member"
    ));

    let one_over = parse_text("source-bytes-one-over", one_over_source)
        .expect("syntax has no competing source-byte owner");
    let key = module_key(&one_over);
    let database = HirDatabase::try_new().expect("source-byte preflight HIR database");
    assert!(database.current(&key).is_none());
    assert!(matches!(
        LoweringRequest::try_new(key.clone(), &one_over),
        Err(HirLowerFailure::Limit(error))
            if error.limit() == HirLimit::SourceDocumentBytes
                && error.observed() == maximum + 1
                && error.maximum() == maximum
    ));
    assert!(
        database.current(&key).is_none(),
        "request preflight failure cannot create a database generation"
    );
}

#[test]
#[ignore = "Tier 2: parses the production Syntax Diagnostics exact/one-over boundary"]
fn e13_tier2_syntax_diagnostic_limit_uses_real_parser_diagnostics() {
    let maximum = SyntaxLimit::Diagnostics.maximum();
    assert!(maximum + 1 < SyntaxLimit::TopLevelItems.maximum());
    let exact_source = INVALID_USE.repeat(maximum);
    let one_over_source = INVALID_USE.repeat(maximum + 1);
    assert!(one_over_source.len() < HirLimit::SourceDocumentBytes.maximum());
    let exact = parse_text("syntax-diagnostics-exact", exact_source)
        .expect("exact parser diagnostic inventory commits");
    assert_eq!(exact.diagnostics().len(), maximum);
    assert!(matches!(
        parse_text("syntax-diagnostics-one-over", one_over_source),
        Err(ParseFailure::LimitExceeded(SyntaxLimit::Diagnostics))
    ));
}

#[test]
fn e13_name_limit_failure_retries_a_shorter_same_lineage_revision_without_leaks() {
    let (one_over, exact) = name_limit_revisions("name-limit-retry");
    assert_eq!(
        one_over.snapshot_id().lineage(),
        exact.snapshot_id().lineage()
    );
    let maximum = HirLimit::NameBytes.maximum();
    let mut database = HirDatabase::try_new().expect("name-limit retry HIR database");

    let attached = attached_expressions(&one_over).pop().unwrap();
    let mut failed = stage(&database, &one_over);
    let failed_scope = allocate_module_scope(&mut failed, &one_over);
    assert!(matches!(
        failed.lower_attached_expression(&attached, failed_scope),
        Err(HirLowerFailure::Limit(error))
            if error.limit() == HirLimit::NameBytes
                && error.observed() == maximum + 1
                && error.maximum() == maximum
    ));
    assert!(failed.finish(&mut database).is_err());
    assert!(database.current(&module_key(&one_over)).is_none());

    let attached = attached_expressions(&exact).pop().unwrap();
    let mut retry = stage(&database, &exact);
    let retry_scope = allocate_module_scope(&mut retry, &exact);
    let owner = retry
        .lower_attached_expression(&attached, retry_scope)
        .expect("shorter same-lineage retry lowers");
    assert_eq!(
        retry
            .lower_attached_expression(&attached, retry_scope)
            .expect("same retry expression deduplicates"),
        owner
    );
    let module = retry
        .finish(&mut database)
        .expect("shorter same-lineage retry publishes")
        .into_module();
    assert_eq!(expression_inventory_len(&module), 2);
    assert!(matches!(
        select_payload(&module, owner).member(),
        HirSelectedMember::Name(name) if name.as_str().len() == maximum
    ));
    assert!(module.diagnostics().is_empty());
}

#[test]
fn e13_freeze_rejects_a_select_that_drops_the_postfix_try_owner() {
    let parsed = parsed_source("select-drop-try-owner", &["target?.".to_owned()]);
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("Try-owner rejection database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let owner = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid Try plus Select prefix");
    let (try_owner, state) = {
        let (slots, arenas) = transaction.storage_mut();
        let root = arenas
            .expressions()
            .resolve_staged(slots, owner)
            .expect("staged Select root");
        let HirExprKind::Select(select) = root.kind() else {
            panic!("target?. must stage an outer Select");
        };
        (select.target(), root.state().clone())
    };
    let operand = {
        let (slots, arenas) = transaction.storage_mut();
        let tried = arenas
            .expressions()
            .resolve_staged(slots, try_owner)
            .expect("staged postfix Try");
        let HirExprKind::Try(tried) = tried.kind() else {
            panic!("outer Select target must be a postfix Try");
        };
        tried.operand()
    };
    let replacement = HirExpr::try_new(
        scope,
        HirExprKind::Select(HirSelectExpr::new(operand, HirSelectedMember::Missing)),
        state,
    )
    .expect("same-module forged Select payload");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .revise_finalized(slots, owner, replacement)
            .expect("test-only dropped Try owner");
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
fn e13_freeze_rejects_an_outer_target_component_truncated_to_the_try_operand() {
    let parsed = parsed_source("select-truncated-try-source", &["target?.".to_owned()]);
    let attached = attached_expressions(&parsed).pop().unwrap();
    let tried = attached.children()[0]
        .authored_semantic()
        .expect("postfix Try attachment")
        .expect("authored postfix Try");
    let operand = tried.children()[0]
        .authored_semantic()
        .expect("Try operand attachment")
        .expect("authored Try operand");
    assert_ne!(
        tried.whole_source_span().range(),
        operand.whole_source_span().range()
    );

    let mut database = HirDatabase::try_new().expect("Try-source rejection database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let owner = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid Try plus Select prefix");
    let query = HirSourceQuery::Expr {
        owner,
        role: HirExprSourceRole::Target,
    };
    assert_eq!(
        transaction
            .source_components()
            .inject_component_for_test(&query, HirSourceSite::Span(operand.whole_source_span()),),
        Err(HirSourceCommitInvariantError::ConflictingComponent { query })
    );
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());
}

#[test]
fn e13_diagnostic_limit_retry_preserves_the_owner_after_a_smaller_revision() {
    let (exact_prefill, smaller_prefill) = diagnostic_revisions("diagnostic-limit-retry");
    assert_eq!(
        exact_prefill.snapshot_id().lineage(),
        smaller_prefill.snapshot_id().lineage()
    );
    assert_eq!(
        exact_prefill.diagnostics().len(),
        SyntaxLimit::Diagnostics.maximum()
    );
    assert_eq!(
        smaller_prefill.diagnostics().len(),
        SyntaxLimit::Diagnostics.maximum() - 1
    );
    let exact_attached = attached_expressions(&exact_prefill).pop().unwrap();
    let smaller_attached = attached_expressions(&smaller_prefill).pop().unwrap();
    assert_eq!(exact_attached.id(), smaller_attached.id());

    let mut database = HirDatabase::try_new().expect("diagnostic retry HIR database");
    let mut failed = stage(&database, &exact_prefill);
    let failed_scope = allocate_module_scope(&mut failed, &exact_prefill);
    let failed_owner = failed
        .lower_attached_expression(&exact_attached, failed_scope)
        .expect("diagnostic one-over stages before freeze");
    assert!(matches!(
        failed.finish(&mut database),
        Err(HirLowerFailure::Limit(error))
            if error.limit() == HirLimit::Diagnostics
                && error.observed() == HirLimit::Diagnostics.maximum() + 1
                && error.maximum() == HirLimit::Diagnostics.maximum()
    ));
    assert!(database.current(&module_key(&exact_prefill)).is_none());

    let mut retry = stage(&database, &smaller_prefill);
    let retry_scope = allocate_module_scope(&mut retry, &smaller_prefill);
    let retry_owner = retry
        .lower_attached_expression(&smaller_attached, retry_scope)
        .expect("smaller diagnostic prefill lowers");
    assert_eq!(retry_owner, failed_owner);
    let module = retry
        .finish(&mut database)
        .expect("smaller diagnostic prefill publishes")
        .into_module();
    assert_eq!(
        recovery_diagnostic_owners(&module),
        vec![SyntheticOwner::Expr(retry_owner)]
    );
    assert_eq!(module.diagnostics().len(), HirLimit::Diagnostics.maximum());
}
