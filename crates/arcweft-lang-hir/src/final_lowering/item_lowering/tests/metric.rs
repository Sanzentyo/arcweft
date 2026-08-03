use super::*;

use crate::expr::HirExprKind;
use crate::item::{
    HirDeclarationMember, HirDeclarationMemberIssue, HirMetricBucketsValue, HirMetricDeclaration,
    HirMetricKind, HirMetricKindIssue, HirMetricUnitValue,
};
use crate::leaf::{HirLiteral, HirStringLiteral};
use crate::type_ref::HirType;

fn metric(
    module: &HirModule,
    ordinal: usize,
) -> (crate::identity::ItemId, &HirItem, &HirMetricDeclaration) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Metric(metric) = item.kind() else {
        panic!("source-ordered item {ordinal} must be a Metric")
    };
    (owner, item, metric)
}

fn member(module: &HirModule, id: crate::item::HirDeclarationMemberId) -> &HirDeclarationMember {
    module.declaration_members().resolve(id).unwrap()
}

fn metric_named<'module>(
    module: &'module HirModule,
    expected: &str,
) -> (
    crate::identity::ItemId,
    &'module HirItem,
    &'module HirMetricDeclaration,
) {
    (0..module.source_ordered_items().len())
        .find_map(|ordinal| {
            let (owner, item, metric) = metric(module, ordinal);
            matches!(
                metric.header().name(),
                HirRetainedName::Resolved(name) if name.as_str() == expected
            )
            .then_some((owner, item, metric))
        })
        .unwrap_or_else(|| panic!("missing Metric declaration `{expected}`"))
}

fn string_expression(module: &HirModule, expected: &str) -> ExprId {
    module
        .arenas()
        .expressions()
        .try_iter(module.slots())
        .unwrap()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Literal(HirLiteral::String(HirStringLiteral::Value(value)))
                if value.as_ref() == expected =>
            {
                Some(owner)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing Metric string expression `{expected}`"))
}

fn first_metric_label_type(
    module: &HirModule,
    metric: &HirMetricDeclaration,
) -> crate::identity::TypeId {
    let retained = member(module, metric.labels()[0]);
    let HirDeclarationMemberKind::MetricLabel(label) = retained.kind() else {
        panic!("first Metric label payload")
    };
    label.ty()
}

fn metric_bucket_values(module: &HirModule, metric: &HirMetricDeclaration) -> Box<[ExprId]> {
    let retained = member(module, metric.buckets().unwrap());
    let HirDeclarationMemberKind::MetricBuckets(buckets) = retained.kind() else {
        panic!("Metric buckets payload")
    };
    let HirMetricBucketsValue::Sequence(values) = buckets.value() else {
        panic!("Metric bucket sequence")
    };
    values.clone()
}

fn lower_output(
    database: &mut HirDatabase,
    parsed: &ParsedSource,
    key: &HirModuleKey,
) -> crate::database::HirLowerOutput {
    let mut transaction = stage(database, parsed, key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    transaction.finish(database).unwrap()
}

fn assert_expression_scope_and_source(module: &HirModule, owner: ExprId, expected_scope: ScopeId) {
    let expression = module
        .arenas()
        .expressions()
        .resolve(module.slots(), owner)
        .unwrap();
    assert_eq!(expression.scope(), expected_scope);
    assert_source_backed_child(module, owner);
}

fn assert_metric_freeze_rejects(
    case: &str,
    source: &str,
    tamper: impl FnOnce(&mut StagedHirModuleTransaction<'_>, crate::identity::ItemId),
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-metric-{case}"),
        source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    let owner = transaction.source_ordered_items[0];
    tamper(&mut transaction, owner);
    assert!(
        matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidSourceIndex
            ))
        ),
        "Metric freeze accepted {case}"
    );
    assert!(database.current(&key).is_none());
}

fn revise_type_scope(
    transaction: &mut StagedHirModuleTransaction<'_>,
    owner: crate::identity::TypeId,
    scope: ScopeId,
) {
    let (kind, state) = {
        let (slots, arenas) = transaction.storage_mut();
        let original = arenas.types().resolve_staged(slots, owner).unwrap();
        (original.kind().clone(), original.state().clone())
    };
    let replacement = HirType::try_new(owner, kind, scope, state, transaction).unwrap();
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .types()
        .revise_finalized(slots, owner, replacement)
        .unwrap();
}

#[test]
fn canonical_metric_freezes_closed_kinds_typed_members_and_expression_owners() {
    let source = concat!(
        "pub metric gauge @metric.frame_time frame_time: f32 {\n",
        "    unit = \"ms\"\n",
        "    labels {\n",
        "        scene: String\n",
        "        quality: RenderQuality\n",
        "    }\n",
        "}\n",
        "metric histogram latency: f64 {\n",
        "    buckets = [1.0, 2.0, 4.0]\n",
        "}\n",
        "metric counter hits: u64 {}\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-metric-clean", source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(module.source_ordered_items().len(), 3);

    let (gauge_owner, gauge_item, gauge) = metric(&module, 0);
    assert_eq!(gauge_item.state(), &HirItemPoisonState::Clean);
    assert_eq!(gauge.kind(), HirMetricKind::Gauge);
    assert_source_backed_child(&module, gauge.value_type());
    assert_eq!(gauge_item.members().len(), 3);
    assert_eq!(gauge.unit(), Some(gauge_item.members()[0]));
    assert_eq!(gauge.labels(), &gauge_item.members()[1..]);
    assert!(gauge.buckets().is_none());
    assert_eq!(
        module
            .declaration_members()
            .arena(gauge_owner)
            .unwrap()
            .members()
            .len(),
        3
    );

    let unit = member(&module, gauge.unit().unwrap());
    assert_eq!(unit.state(), HirDeclarationMemberPoisonState::Clean);
    let HirDeclarationMemberKind::MetricUnit(unit) = unit.kind() else {
        panic!("first gauge member must be the unit")
    };
    assert!(matches!(
        unit.value(),
        HirMetricUnitValue::String(HirStringLiteral::Value(value)) if value.as_ref() == "ms"
    ));

    for (id, expected_name) in gauge.labels().iter().copied().zip(["scene", "quality"]) {
        let retained = member(&module, id);
        assert_eq!(retained.state(), HirDeclarationMemberPoisonState::Clean);
        let HirDeclarationMemberKind::MetricLabel(label) = retained.kind() else {
            panic!("gauge label member")
        };
        assert!(matches!(
            label.name(),
            HirRequiredName::Resolved(name) if name.as_str() == expected_name
        ));
        assert!(!label.is_duplicate());
        assert_source_backed_child(&module, label.ty());
    }
    assert_item_slot_whole(&module, &parsed, gauge_owner);

    let (histogram_owner, histogram_item, histogram) = metric(&module, 1);
    assert_eq!(histogram_item.state(), &HirItemPoisonState::Clean);
    assert_eq!(histogram.kind(), HirMetricKind::Histogram);
    assert!(histogram.unit().is_none());
    assert!(histogram.labels().is_empty());
    let bucket_member = member(&module, histogram.buckets().unwrap());
    assert_eq!(
        bucket_member.state(),
        HirDeclarationMemberPoisonState::Clean
    );
    let HirDeclarationMemberKind::MetricBuckets(bucket_member) = bucket_member.kind() else {
        panic!("histogram bucket member")
    };
    let HirMetricBucketsValue::Sequence(bucket_values) = bucket_member.value() else {
        panic!("histogram buckets must retain expression IDs")
    };
    assert_eq!(bucket_values.len(), 3);
    for owner in bucket_values.iter().copied() {
        assert_expression_scope_and_source(&module, owner, histogram_item.scope());
    }
    let parent_sequences = module
        .arenas()
        .expressions()
        .try_iter(module.slots())
        .unwrap()
        .filter(|(_, expression)| {
            matches!(
                expression.kind(),
                HirExprKind::BracketSequence(sequence)
                    if sequence.elements() == bucket_values.as_ref()
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(parent_sequences.len(), 1);
    assert_expression_scope_and_source(&module, parent_sequences[0].0, histogram_item.scope());
    assert_item_slot_whole(&module, &parsed, histogram_owner);

    let (counter_owner, counter_item, counter) = metric(&module, 2);
    assert_eq!(counter_item.state(), &HirItemPoisonState::Clean);
    assert_eq!(counter.kind(), HirMetricKind::Counter);
    assert!(counter_item.members().is_empty());
    assert!(counter.unit().is_none());
    assert!(counter.labels().is_empty());
    assert!(counter.buckets().is_none());
    assert!(module.declaration_members().arena(counter_owner).is_none());
}

#[test]
fn recovered_metric_retains_reachable_values_global_member_order_and_primary_precedence() {
    let source = concat!(
        "metric mystery broken: f32 {\n",
        "    labels {\n",
        "        scene: String\n",
        "        scene bool\n",
        "    }\n",
        "    unit milliseconds\n",
        "    extra = true\n",
        "    buckets = []\n",
        "    buckets = [1.0]\n",
        "}\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-metric-recovery", source);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, metric) = metric(&module, 0);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );
    assert_eq!(
        metric.kind(),
        HirMetricKind::Recovered(HirMetricKindIssue::Invalid)
    );
    assert_eq!(item.members().len(), 5);
    assert_eq!(
        item.members()
            .iter()
            .map(|id| id.ordinal())
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4]
    );
    assert_eq!(metric.labels(), &item.members()[..2]);
    assert_eq!(metric.unit(), Some(item.members()[2]));
    assert_eq!(metric.buckets(), Some(item.members()[3]));

    let first_label = member(&module, item.members()[0]);
    let duplicate_label = member(&module, item.members()[1]);
    assert_eq!(first_label.state(), HirDeclarationMemberPoisonState::Clean);
    assert_eq!(
        duplicate_label.state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::Duplicate)
    );
    let HirDeclarationMemberKind::MetricLabel(duplicate_label) = duplicate_label.kind() else {
        panic!("duplicate label payload")
    };
    assert!(duplicate_label.is_duplicate());

    let unit = member(&module, item.members()[2]);
    assert_eq!(
        unit.state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::MissingAssignment)
    );
    let HirDeclarationMemberKind::MetricUnit(unit) = unit.kind() else {
        panic!("recovered unit payload")
    };
    let HirMetricUnitValue::NonString(unit_expression) = unit.value() else {
        panic!("non-string unit expression must remain typed")
    };
    assert_expression_scope_and_source(&module, *unit_expression, item.scope());

    let empty_buckets = member(&module, item.members()[3]);
    assert_eq!(
        empty_buckets.state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::RecoveredChild)
    );
    let HirDeclarationMemberKind::MetricBuckets(empty_buckets) = empty_buckets.kind() else {
        panic!("empty buckets payload")
    };
    assert!(matches!(
        empty_buckets.value(),
        HirMetricBucketsValue::Sequence(values) if values.is_empty()
    ));

    let duplicate_buckets = member(&module, item.members()[4]);
    assert_eq!(
        duplicate_buckets.state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::Duplicate)
    );
    let HirDeclarationMemberKind::MetricBuckets(duplicate_buckets) = duplicate_buckets.kind()
    else {
        panic!("duplicate buckets payload")
    };
    let HirMetricBucketsValue::Sequence(values) = duplicate_buckets.value() else {
        panic!("duplicate bucket sequence remains typed")
    };
    assert_eq!(values.len(), 1);
    assert_expression_scope_and_source(&module, values[0], item.scope());
    assert_eq!(
        module
            .declaration_members()
            .arena(owner)
            .unwrap()
            .members()
            .len(),
        5,
        "the unknown body entry is recovery evidence, not a fabricated HIR member"
    );
    assert_item_slot_whole(&module, &parsed, owner);
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn missing_metric_components_remain_typed_without_fabricated_members_or_values() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-metric-missing",
        "metric @metric.missing missing\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, metric) = metric(&module, 0);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );
    assert_eq!(
        metric.kind(),
        HirMetricKind::Recovered(HirMetricKindIssue::Missing)
    );
    assert!(
        module
            .slots()
            .resolve(metric.value_type())
            .unwrap()
            .is_poisoned()
    );
    assert!(item.members().is_empty());
    assert!(metric.unit().is_none());
    assert!(metric.labels().is_empty());
    assert!(metric.buckets().is_none());
    assert!(module.declaration_members().arena(owner).is_none());
    assert_eq!(
        module
            .arenas()
            .expressions()
            .try_iter(module.slots())
            .unwrap()
            .count(),
        0
    );
    assert_item_slot_whole(&module, &parsed, owner);
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn metric_freeze_rejects_kind_and_member_projection_tampering() {
    let source = concat!(
        "metric gauge frame_time: f32 {\n",
        "    unit = \"ms\"\n",
        "    labels { scene: String }\n",
        "}\n",
        "action sibling() { return }\n",
    );

    assert_metric_freeze_rejects("kind-tamper", source, |transaction, owner| {
        let (slots, arenas) = transaction.storage_mut();
        let original = arenas.items().resolve_staged(slots, owner).unwrap().clone();
        let HirItemKind::Metric(metric) = original.kind() else {
            panic!("final Metric item")
        };
        let replacement_metric = HirMetricDeclaration::try_new(
            owner,
            metric.header().clone(),
            HirMetricKind::Counter,
            metric.value_type(),
            metric.unit(),
            metric.labels().into(),
            metric.buckets(),
        )
        .unwrap();
        let replacement = HirItem::try_new_with_state(
            owner,
            original.scope(),
            original.prefix().clone(),
            HirItemKind::Metric(replacement_metric),
            original.members().into(),
            *original.state(),
        )
        .unwrap();
        arenas
            .items()
            .revise_finalized(slots, owner, replacement)
            .unwrap();
    });

    assert_metric_freeze_rejects("member-pointer-tamper", source, |transaction, owner| {
        let (slots, arenas) = transaction.storage_mut();
        let original = arenas.items().resolve_staged(slots, owner).unwrap().clone();
        let HirItemKind::Metric(metric) = original.kind() else {
            panic!("final Metric item")
        };
        let replacement_metric = HirMetricDeclaration::try_new(
            owner,
            metric.header().clone(),
            metric.kind(),
            metric.value_type(),
            Some(original.members()[1]),
            Box::new([]),
            metric.buckets(),
        )
        .unwrap();
        let replacement = HirItem::try_new_with_state(
            owner,
            original.scope(),
            original.prefix().clone(),
            HirItemKind::Metric(replacement_metric),
            original.members().into(),
            *original.state(),
        )
        .unwrap();
        arenas
            .items()
            .revise_finalized(slots, owner, replacement)
            .unwrap();
    });

    assert_metric_freeze_rejects("value-type-scope-tamper", source, |transaction, owner| {
        let sibling = transaction.source_ordered_items[1];
        let (value_type, foreign_scope) = {
            let (slots, arenas) = transaction.storage_mut();
            let value_type = {
                let original = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Metric(metric) = original.kind() else {
                    panic!("final Metric item")
                };
                metric.value_type()
            };
            let sibling = arenas.items().resolve_staged(slots, sibling).unwrap();
            let HirItemKind::Action(sibling) = sibling.kind() else {
                panic!("scope donor must be an Action")
            };
            (value_type, sibling.callable_scope())
        };
        revise_type_scope(transaction, value_type, foreign_scope);
    });

    assert_metric_freeze_rejects("label-type-scope-tamper", source, |transaction, owner| {
        let sibling = transaction.source_ordered_items[1];
        let (label_type, foreign_scope) = {
            let (slots, arenas) = transaction.storage_mut();
            let (item_scope, value_type) = {
                let original = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Metric(metric) = original.kind() else {
                    panic!("final Metric item")
                };
                (original.scope(), metric.value_type())
            };
            let label_type = slots
                .live_ids::<crate::identity::TypeId>()
                .find(|candidate| {
                    *candidate != value_type
                        && arenas
                            .types()
                            .resolve_staged(slots, *candidate)
                            .is_ok_and(|ty| ty.scope() == item_scope)
                })
                .expect("Metric label type owner");
            let sibling = arenas.items().resolve_staged(slots, sibling).unwrap();
            let HirItemKind::Action(sibling) = sibling.kind() else {
                panic!("scope donor must be an Action")
            };
            (label_type, sibling.callable_scope())
        };
        revise_type_scope(transaction, label_type, foreign_scope);
    });
}

#[test]
fn incremental_metric_preserves_reconciled_owners_and_retires_replaced_children() {
    let name = SourceName::path("proof/metric-incremental.arcw");
    let document_id = "arcweft-test://proof/metric-incremental";
    let initial_source = concat!(
        "metric gauge First: f32 {\n",
        "    unit = \"ms\"\n",
        "    labels { scene: String }\n",
        "}\n",
        "metric histogram Second: f64 {\n",
        "    buckets = [1.0, 2.0]\n",
        "}\n",
    );
    let reordered_source = concat!(
        "metric histogram Second: f64 {\n",
        "    buckets = [1.0, 2.0]\n",
        "}\n",
        "metric counter Inserted: u64 {}\n",
        "metric gauge First: f32 {\n",
        "    unit = \"ms\"\n",
        "    labels { scene: String }\n",
        "}\n",
    );
    let modified_source = concat!(
        "metric histogram Second: f64 {\n",
        "    buckets = [1.0, 2.0]\n",
        "}\n",
        "metric counter Inserted: u64 {}\n",
        "metric gauge First: f32 {\n",
        "    unit = \"seconds\"\n",
        "    labels { scene: Bool }\n",
        "}\n",
    );
    let bucket_modified_source = concat!(
        "metric histogram Second: f64 {\n",
        "    buckets = [1.0, 3.0]\n",
        "}\n",
        "metric counter Inserted: u64 {}\n",
        "metric gauge First: f32 {\n",
        "    unit = \"seconds\"\n",
        "    labels { scene: Bool }\n",
        "}\n",
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, initial_source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let first = lower(&mut database, &initial, &key);
    let (first_owner, first_item, first_metric) = metric_named(&first, "First");
    let (second_owner, _, second_metric) = metric_named(&first, "Second");
    let first_members = first_item.members().to_vec();
    let unit_expression = string_expression(&first, "ms");
    let label_type = first_metric_label_type(&first, first_metric);
    let second_buckets = metric_bucket_values(&first, second_metric);

    let reordered = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(0, initial_source.len()))
                    .unwrap(),
                reordered_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let second_output = lower_output(&mut database, &reordered, &key);
    let second = Arc::clone(second_output.module());
    let (reordered_first_owner, reordered_first_item, reordered_first) =
        metric_named(&second, "First");
    let (reordered_second_owner, _, reordered_second) = metric_named(&second, "Second");
    let (inserted_owner, _, _) = metric_named(&second, "Inserted");
    assert_eq!(reordered_first_owner, first_owner);
    assert_eq!(reordered_second_owner, second_owner);
    assert_ne!(inserted_owner, first_owner);
    assert_ne!(inserted_owner, second_owner);
    assert_eq!(
        second_output.invalidations().changed_items(),
        [inserted_owner],
        "reordering stable items stays hot while the inserted item invalidates"
    );
    assert_eq!(reordered_first_item.members(), first_members.as_slice());
    assert_eq!(string_expression(&second, "ms"), unit_expression);
    assert_eq!(
        first_metric_label_type(&second, reordered_first),
        label_type
    );
    assert_eq!(
        metric_bucket_values(&second, reordered_second),
        second_buckets
    );

    let modified = syntax
        .reparse(
            &reordered,
            &[SourceEdit::new(
                reordered
                    .document()
                    .span(SourceRange::new(0, reordered_source.len()))
                    .unwrap(),
                modified_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let third_output = lower_output(&mut database, &modified, &key);
    let third = Arc::clone(third_output.module());
    let (modified_first_owner, modified_first_item, modified_first) = metric_named(&third, "First");
    let (modified_second_owner, _, modified_second) = metric_named(&third, "Second");
    assert_eq!(modified_first_owner, first_owner);
    assert_eq!(modified_second_owner, second_owner);
    assert_eq!(modified_first_item.members(), first_members.as_slice());
    assert_eq!(third_output.invalidations().changed_items(), [first_owner]);
    assert!(third_output.invalidations().symbol_revision_changed());

    let new_unit_expression = string_expression(&third, "seconds");
    let new_label_type = first_metric_label_type(&third, modified_first);
    assert_ne!(new_unit_expression, unit_expression);
    assert_ne!(new_label_type, label_type);
    assert!(
        third
            .arenas()
            .expressions()
            .resolve(third.slots(), unit_expression)
            .is_err()
    );
    assert!(
        third
            .arenas()
            .types()
            .resolve(third.slots(), label_type)
            .is_err()
    );
    assert_eq!(
        metric_bucket_values(&third, modified_second),
        second_buckets,
        "unchanged histogram element owners must survive the sibling edit"
    );
    assert_expression_scope_and_source(&third, new_unit_expression, modified_first_item.scope());
    assert_source_backed_child(&third, new_label_type);

    let bucket_modified = syntax
        .reparse(
            &modified,
            &[SourceEdit::new(
                modified
                    .document()
                    .span(SourceRange::new(0, modified_source.len()))
                    .unwrap(),
                bucket_modified_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let fourth_output = lower_output(&mut database, &bucket_modified, &key);
    let fourth = fourth_output.module();
    let (fourth_second_owner, _, fourth_second) = metric_named(fourth, "Second");
    let fourth_buckets = metric_bucket_values(fourth, fourth_second);
    assert_eq!(fourth_second_owner, second_owner);
    assert_eq!(
        fourth_output.invalidations().changed_items(),
        [second_owner]
    );
    assert_ne!(fourth_buckets, second_buckets);
    assert_eq!(fourth_buckets[0], second_buckets[0]);
    assert!(
        fourth
            .arenas()
            .expressions()
            .resolve(fourth.slots(), second_buckets[1])
            .is_err()
    );
}
