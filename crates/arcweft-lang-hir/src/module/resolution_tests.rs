use std::collections::BTreeSet;
use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::attachment::TypedItemNode;
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};

use crate::database::HirDatabase;
use crate::expr::{HirExprKind, HirRecordField};
use crate::final_lowering::stage_unpublished_module_for_invariant_test;
use crate::identity::{HirIdKind, HirTypedId, IdResolveError, LocalGeneration};
use crate::leaf::{HirPathRoot, HirPathSegment, HirPathValue};
use crate::lowering::{HirModuleKey, LoweringRequest};
use crate::scope::{HirLocalKind, LocalLookup};
use crate::slot::HirOrigin;
use crate::source_index::{HirSourceLookupError, HirSourceSite};
use crate::symbol::CallablePackageId;

use super::{HirModule, HirModuleStatus};

const FUNCTION_SOURCE: &str = concat!(
    "pub fn ordered<T: Bound>((left, right): (T, T))(next: Mapper<T>) -> Output\n",
    "where T: Other\n",
    "requires ready(left)\n",
    "ensures result == next(right)\n",
    "{\n",
    "    let chosen = next(left);\n",
    "    chosen\n",
    "}\n",
);

fn parse_source_in(
    syntax: &mut SyntaxDatabase,
    document_id: &str,
    source_name: &str,
    source: &str,
) -> ParsedSource {
    let name = SourceName::path(source_name);
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(document_id).unwrap(),
            name.clone(),
            source,
        )
        .unwrap(),
    );
    syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap()
}

fn parse_in(syntax: &mut SyntaxDatabase, document_id: &str, source_name: &str) -> ParsedSource {
    parse_source_in(syntax, document_id, source_name, FUNCTION_SOURCE)
}

fn module_key(parsed: &ParsedSource) -> HirModuleKey {
    HirModuleKey::new(
        CallablePackageId::try_new("proof-hir-module-resolution-tests").unwrap(),
        CanonicalModulePath::crate_root(),
        parsed.document().identity().clone(),
    )
}

fn revision_key(base: &HirModuleKey, parsed: &ParsedSource) -> HirModuleKey {
    HirModuleKey::new(
        base.package().clone(),
        base.path().clone(),
        parsed.document().identity().clone(),
    )
}

fn lower(database: &mut HirDatabase, parsed: &ParsedSource, key: &HirModuleKey) -> Arc<HirModule> {
    let key = revision_key(key, parsed);
    let mut transaction = stage_unpublished_module_for_invariant_test(
        database,
        LoweringRequest::try_new(key, parsed).unwrap(),
        crate::lowering::HirLoweringControl::new(),
    )
    .unwrap();
    transaction.lower_parsed_source_items(parsed).unwrap();
    transaction.finish(database).unwrap().into_module()
}

fn raw_slots_are_ordered<I: HirTypedId>(ids: &[I]) -> bool {
    ids.windows(2)
        .all(|pair| pair[0].raw().slot() < pair[1].raw().slot())
}

#[test]
fn local_lookup_uses_the_post_statement_binding_point() {
    const SOURCE: &str = concat!(
        "fn lookup(value: I32) -> I32 {\n",
        "    let value = value;\n",
        "    value\n",
        "}\n",
    );

    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_source_in(
        &mut syntax,
        "arcweft-test://proof/hir-module-local-lookup",
        "proof/hir-module-local-lookup.arcw",
        SOURCE,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let parameter = module
        .locals()
        .find(|(_, local)| {
            local.kind() == HirLocalKind::Parameter && local.name().as_str() == "value"
        })
        .expect("parameter local");
    let binding = module
        .locals()
        .find(|(_, local)| {
            local.kind() == HirLocalKind::LetBinding && local.name().as_str() == "value"
        })
        .expect("let-binding local");
    let initializer_start = SOURCE.find("= value").unwrap() + "= ".len();
    let tail_start = SOURCE.rfind("value").unwrap();
    let initializer_use = parsed
        .document()
        .span(SourceRange::new(
            initializer_start,
            initializer_start + "value".len(),
        ))
        .unwrap();
    let tail_use = parsed
        .document()
        .span(SourceRange::new(tail_start, tail_start + "value".len()))
        .unwrap();

    assert_eq!(
        module.lookup_local(binding.1.scope(), binding.1.name(), initializer_use),
        Ok(LocalLookup::Found(parameter.0))
    );
    assert_eq!(
        module.lookup_local(binding.1.scope(), binding.1.name(), tail_use),
        Ok(LocalLookup::Found(binding.0))
    );
}

#[test]
fn local_lookup_does_not_fall_through_an_only_poisoned_latest_binding() {
    const SOURCE: &str = concat!(
        "predicate lookup(input: Bool) {\n",
        "    let value = input;\n",
        "    let mut value = input;\n",
        "    value\n",
        "}\n",
    );

    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_source_in(
        &mut syntax,
        "arcweft-test://proof/hir-module-poisoned-local-lookup",
        "proof/hir-module-poisoned-local-lookup.arcw",
        SOURCE,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(module.status(), HirModuleStatus::Recovered);
    let mut bindings = module
        .locals()
        .filter(|(_, local)| {
            local.kind() == HirLocalKind::LetBinding && local.name().as_str() == "value"
        })
        .collect::<Vec<_>>();
    bindings.sort_by_key(|(_, local)| local.generation());
    let [first, poisoned] = bindings.as_slice() else {
        panic!("same-scope poisoned-shadow fixture retains two Let locals");
    };
    assert_eq!(first.1.generation(), LocalGeneration::FIRST);
    assert_eq!(
        poisoned.1.generation(),
        LocalGeneration::FIRST.checked_next().unwrap()
    );
    assert!(!first.1.is_poisoned());
    assert!(poisoned.1.is_poisoned());
    assert_eq!(first.1.scope(), poisoned.1.scope());

    let use_start = SOURCE.rfind("value").unwrap();
    let use_span = parsed
        .document()
        .span(SourceRange::new(use_start, use_start + "value".len()))
        .unwrap();
    let tail = module
        .expressions()
        .find(|(id, _)| {
            matches!(
                module.metadata(*id).unwrap().source_site(),
                HirSourceSite::Span(span) if span.range().start() == use_start
            )
        })
        .expect("source-backed predicate tail");
    let HirExprKind::Path(HirPathValue::Resolved(path)) = tail.1.kind() else {
        panic!("poisoned-shadow tail remains a resolved typed Path");
    };
    assert_eq!(path.root(), HirPathRoot::ImplicitCrate);
    assert!(matches!(
        path.segments(),
        [HirPathSegment::Identifier(name)] if name.as_str() == "value"
    ));
    assert_eq!(
        module.lookup_local(poisoned.1.scope(), poisoned.1.name(), use_span),
        Ok(LocalLookup::AmbiguousPoisoned(Box::new([poisoned.0])))
    );
}

#[test]
fn ordinary_refutable_let_poisons_the_local_and_blocks_lookup() {
    const SOURCE: &str = concat!(
        "fn refutable(input: Option<I32>) {\n",
        "    let .Some(value) = input;\n",
        "    value\n",
        "}\n",
    );

    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_source_in(
        &mut syntax,
        "arcweft-test://proof/hir-module-refutable-let",
        "proof/hir-module-refutable-let.arcw",
        SOURCE,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let binding = module
        .locals()
        .find(|(_, local)| {
            local.kind() == HirLocalKind::LetBinding && local.name().as_str() == "value"
        })
        .expect("ordinary refutable Let local");
    assert_eq!(binding.1.generation(), LocalGeneration::FIRST);
    assert!(binding.1.is_poisoned());

    let use_start = SOURCE.rfind("value").unwrap();
    let use_span = parsed
        .document()
        .span(SourceRange::new(use_start, use_start + "value".len()))
        .unwrap();
    let tail = module
        .expressions()
        .find(|(id, _)| {
            matches!(
                module.metadata(*id).unwrap().source_site(),
                HirSourceSite::Span(span) if span.range().start() == use_start
            )
        })
        .expect("source-backed ordinary function tail");
    let HirExprKind::Path(HirPathValue::Resolved(path)) = tail.1.kind() else {
        panic!("ordinary refutable Let tail remains a resolved typed Path");
    };
    assert_eq!(path.root(), HirPathRoot::ImplicitCrate);
    assert!(matches!(
        path.segments(),
        [HirPathSegment::Identifier(name)] if name.as_str() == "value"
    ));
    assert_eq!(
        module.lookup_local(binding.1.scope(), binding.1.name(), use_span),
        Ok(LocalLookup::AmbiguousPoisoned(Box::new([binding.0])))
    );
}

#[test]
fn duplicate_recovered_shorthand_uses_the_clean_same_generation_member() {
    const SOURCE: &str = concat!(
        "fn duplicate(input: (I32, I32)) {\n",
        "    let (value, value) = input;\n",
        "    Point { value }\n",
        "}\n",
    );

    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_source_in(
        &mut syntax,
        "arcweft-test://proof/hir-module-duplicate-shorthand",
        "proof/hir-module-duplicate-shorthand.arcw",
        SOURCE,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(module.status(), HirModuleStatus::Recovered);
    let bindings = module
        .locals()
        .filter(|(_, local)| {
            local.kind() == HirLocalKind::LetBinding && local.name().as_str() == "value"
        })
        .collect::<Vec<_>>();
    let [left, right] = bindings.as_slice() else {
        panic!("duplicate tuple pattern retains two Let locals");
    };
    let (first, duplicate) = if left.1.is_poisoned() {
        (right, left)
    } else {
        (left, right)
    };
    assert_eq!(first.1.generation(), LocalGeneration::FIRST);
    assert_eq!(duplicate.1.generation(), LocalGeneration::FIRST);
    assert!(!first.1.is_poisoned());
    assert!(duplicate.1.is_poisoned());
    assert_eq!(first.1.scope(), duplicate.1.scope());

    let record = module
        .expressions()
        .find_map(|(_, expression)| match expression.kind() {
            HirExprKind::Record(record) => Some(record),
            _ => None,
        })
        .expect("duplicate shorthand tail Record");
    assert!(matches!(
        record.fields(),
        [HirRecordField::Shorthand { local, .. }] if *local == first.0
    ));

    let use_start = SOURCE.rfind("value").unwrap();
    let use_span = parsed
        .document()
        .span(SourceRange::new(use_start, use_start + "value".len()))
        .unwrap();
    assert_eq!(
        module.lookup_local(first.1.scope(), first.1.name(), use_span),
        Ok(LocalLookup::Found(first.0))
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one accepted-module matrix resolves and iterates every typed arena owner"
)]
fn accepted_module_resolves_and_iterates_all_eight_typed_arenas() {
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_in(
        &mut syntax,
        "arcweft-test://proof/hir-module-resolution",
        "proof/hir-module-resolution.arcw",
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let provenance = module.provenance();
    assert_eq!(provenance.syntax_snapshot(), parsed.snapshot_id());
    assert_eq!(provenance.source_snapshot(), parsed.source_snapshot_id());
    assert_eq!(provenance.source_identity(), parsed.document().identity());
    assert!(Arc::ptr_eq(provenance.document(), parsed.document_lease()));

    let items = module.items().collect::<Vec<_>>();
    assert_eq!(module.items().len(), items.len());
    assert!(!items.is_empty());
    assert!(raw_slots_are_ordered(
        &items.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    ));
    for &(id, value) in &items {
        assert!(std::ptr::eq(module.resolve_item(id).unwrap(), value));
        assert_eq!(module.metadata(id).unwrap().kind(), HirIdKind::Item);
    }

    let scopes = module.scopes().collect::<Vec<_>>();
    assert_eq!(module.scopes().len(), scopes.len());
    assert!(!scopes.is_empty());
    assert!(raw_slots_are_ordered(
        &scopes.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    ));
    for &(id, value) in &scopes {
        assert!(std::ptr::eq(module.resolve_scope(id).unwrap(), value));
        assert_eq!(module.metadata(id).unwrap().kind(), HirIdKind::Scope);
    }

    let locals = module.locals().collect::<Vec<_>>();
    assert_eq!(module.locals().len(), locals.len());
    assert!(!locals.is_empty());
    assert!(raw_slots_are_ordered(
        &locals.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    ));
    for &(id, value) in &locals {
        assert!(std::ptr::eq(module.resolve_local(id).unwrap(), value));
        assert_eq!(module.metadata(id).unwrap().kind(), HirIdKind::Local);
    }

    let expressions = module.expressions().collect::<Vec<_>>();
    assert_eq!(module.expressions().len(), expressions.len());
    assert!(!expressions.is_empty());
    assert!(raw_slots_are_ordered(
        &expressions.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    ));
    for &(id, value) in &expressions {
        assert!(std::ptr::eq(module.resolve_expr(id).unwrap(), value));
        assert_eq!(module.metadata(id).unwrap().kind(), HirIdKind::Expr);
    }

    let statements = module.statements().collect::<Vec<_>>();
    assert_eq!(module.statements().len(), statements.len());
    assert!(!statements.is_empty());
    assert!(raw_slots_are_ordered(
        &statements.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    ));
    for &(id, value) in &statements {
        assert!(std::ptr::eq(module.resolve_stmt(id).unwrap(), value));
        assert_eq!(module.metadata(id).unwrap().kind(), HirIdKind::Stmt);
    }

    let types = module.types().collect::<Vec<_>>();
    assert_eq!(module.types().len(), types.len());
    assert!(!types.is_empty());
    assert!(raw_slots_are_ordered(
        &types.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    ));
    for &(id, value) in &types {
        assert!(std::ptr::eq(module.resolve_type(id).unwrap(), value));
        assert_eq!(module.metadata(id).unwrap().kind(), HirIdKind::Type);
    }

    let patterns = module.patterns().collect::<Vec<_>>();
    assert_eq!(module.patterns().len(), patterns.len());
    assert!(!patterns.is_empty());
    assert!(raw_slots_are_ordered(
        &patterns.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    ));
    for &(id, value) in &patterns {
        assert!(std::ptr::eq(module.resolve_pattern(id).unwrap(), value));
        assert_eq!(module.metadata(id).unwrap().kind(), HirIdKind::Pattern);
    }

    let captures = module.captures().collect::<Vec<_>>();
    assert_eq!(module.captures().len(), captures.len());
    assert!(raw_slots_are_ordered(
        &captures.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    ));
    for &(id, value) in &captures {
        assert!(std::ptr::eq(module.resolve_capture(id).unwrap(), value));
        assert_eq!(module.metadata(id).unwrap().kind(), HirIdKind::Capture);
    }

    let current = database.current(&key).unwrap();
    let retained = database.snapshot(module.snapshot_id()).unwrap();
    assert!(Arc::ptr_eq(&module, &current));
    assert!(Arc::ptr_eq(&module, &retained));

    let mut foreign_database = HirDatabase::try_new().unwrap();
    let foreign = lower(&mut foreign_database, &parsed, &key);
    let foreign_item = foreign.items().next().unwrap().0;
    assert!(matches!(
        module.resolve_item(foreign_item),
        Err(IdResolveError::WrongModule { expected, actual })
            if expected == module.module_id() && actual == foreign.module_id()
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one source-backed-node matrix pairs every typed arena family with its exact HIR kind"
)]
fn every_source_backed_node_maps_to_exact_hir_kind() {
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_in(
        &mut syntax,
        "arcweft-test://proof/hir-module-source-owner",
        "proof/hir-module-source-owner.arcw",
    );
    let sibling = parse_in(
        &mut syntax,
        "arcweft-test://proof/hir-module-source-owner-sibling",
        "proof/hir-module-source-owner-sibling.arcw",
    );
    let mut foreign_syntax = SyntaxDatabase::try_new().unwrap();
    let foreign = parse_in(
        &mut foreign_syntax,
        "arcweft-test://proof/hir-module-source-owner-foreign",
        "proof/hir-module-source-owner-foreign.arcw",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let mut source_items = 0;
    for (id, _) in module.items() {
        if let HirOrigin::Source(source) = module.metadata(id).unwrap().origin() {
            assert_eq!(module.item_for_syntax(source.syntax()), Ok(id));
            source_items += 1;
        }
    }
    let mut source_scopes = 0;
    for (id, _) in module.scopes() {
        if let HirOrigin::Source(source) = module.metadata(id).unwrap().origin() {
            assert_eq!(module.scope_for_syntax(source.syntax()), Ok(id));
            source_scopes += 1;
        }
    }
    let mut source_expressions = 0;
    for (id, _) in module.expressions() {
        if let HirOrigin::Source(source) = module.metadata(id).unwrap().origin() {
            assert_eq!(module.expr_for_syntax(source.syntax()), Ok(id));
            source_expressions += 1;
        }
    }
    let mut source_statements = 0;
    for (id, _) in module.statements() {
        if let HirOrigin::Source(source) = module.metadata(id).unwrap().origin() {
            assert_eq!(module.stmt_for_syntax(source.syntax()), Ok(id));
            source_statements += 1;
        }
    }
    let mut source_types = 0;
    for (id, _) in module.types() {
        if let HirOrigin::Source(source) = module.metadata(id).unwrap().origin() {
            assert_eq!(module.type_for_syntax(source.syntax()), Ok(id));
            source_types += 1;
        }
    }
    let mut source_patterns = 0;
    for (id, _) in module.patterns() {
        if let HirOrigin::Source(source) = module.metadata(id).unwrap().origin() {
            assert_eq!(module.pattern_for_syntax(source.syntax()), Ok(id));
            source_patterns += 1;
        }
    }
    assert!(source_items > 0);
    assert!(source_scopes > 0);
    assert!(source_expressions > 0);
    assert!(source_statements > 0);
    assert!(source_types > 0);
    assert!(source_patterns > 0);

    let item_syntax = parsed.items().unwrap()[0].id();
    assert!(matches!(
        module.local_for_syntax(item_syntax),
        Err(HirSourceLookupError::KindMismatch {
            syntax,
            expected: HirIdKind::Local,
            actual: HirIdKind::Item,
        }) if syntax == item_syntax
    ));
    assert!(matches!(
        module.expr_for_syntax(item_syntax),
        Err(HirSourceLookupError::KindMismatch {
            syntax,
            expected: HirIdKind::Expr,
            actual: HirIdKind::Item,
        }) if syntax == item_syntax
    ));

    let TypedItemNode::Function(function) = &parsed.items().unwrap()[0] else {
        panic!("fixture must contain one Function")
    };
    let name_syntax = function.semantics().unwrap().name().syntax().id();
    assert!(matches!(
        module.expr_for_syntax(name_syntax),
        Err(HirSourceLookupError::NotLowered {
            syntax,
            expected: HirIdKind::Expr,
        }) if syntax == name_syntax
    ));

    let sibling_syntax = sibling.items().unwrap()[0].id();
    assert!(matches!(
        module.item_for_syntax(sibling_syntax),
        Err(HirSourceLookupError::WrongSyntaxLineage { expected, actual })
            if expected == parsed.snapshot_id().lineage()
                && actual == sibling.snapshot_id().lineage()
    ));

    let foreign_syntax = foreign.items().unwrap()[0].id();
    assert!(matches!(
        module.item_for_syntax(foreign_syntax),
        Err(HirSourceLookupError::WrongSyntaxDatabase { expected, actual })
            if expected == parsed.snapshot_id().lineage().database()
                && actual == foreign.snapshot_id().lineage().database()
    ));
}

#[test]
fn same_line_hir_nodes_do_not_collide() {
    const SOURCE: &str =
        "fn same_line((left, right): (I32, I32)) -> I32 { let sum = left + right; sum }\n";

    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_source_in(
        &mut syntax,
        "arcweft-test://proof/hir-module-same-line-identities",
        "proof/hir-module-same-line-identities.arcw",
        SOURCE,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let mut source_backed = Vec::new();
    macro_rules! collect_source_backed {
        ($nodes:expr) => {
            for (id, _) in $nodes {
                if matches!(module.metadata(id).unwrap().origin(), HirOrigin::Source(_)) {
                    source_backed.push(id.raw());
                }
            }
        };
    }
    collect_source_backed!(module.items());
    collect_source_backed!(module.scopes());
    collect_source_backed!(module.locals());
    collect_source_backed!(module.expressions());
    collect_source_backed!(module.statements());
    collect_source_backed!(module.types());
    collect_source_backed!(module.patterns());
    collect_source_backed!(module.captures());

    assert!(
        source_backed.len() > 12,
        "fixture must exercise same-line density"
    );
    assert_eq!(
        source_backed.iter().copied().collect::<BTreeSet<_>>().len(),
        source_backed.len(),
        "source-backed nodes sharing one physical line must retain distinct typed identities"
    );
}

#[test]
fn activity_port_local_round_trips_its_existing_source_owner() {
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_source_in(
        &mut syntax,
        "arcweft-test://proof/hir-module-source-local",
        "proof/hir-module-source-local.arcw",
        "activity LocalOwner { input { value: I32 } }\n",
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let source_locals = module
        .locals()
        .filter_map(|(id, local)| match module.metadata(id).unwrap().origin() {
            HirOrigin::Source(source) => Some((id, local, source.syntax())),
            HirOrigin::Synthetic(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(source_locals.len(), 1);
    let (local_id, local, syntax) = source_locals[0];
    assert_eq!(local.name().as_str(), "value");
    assert_eq!(module.local_for_syntax(syntax), Ok(local_id));
    assert!(std::ptr::eq(module.resolve_local(local_id).unwrap(), local));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one revision matrix proves stable, new, and retired identity resolution across the same lineage"
)]
fn same_lineage_revisions_resolve_stable_new_and_retired_item_identity_exactly() {
    const INITIAL_SOURCE: &str = "fn stable() {}\nactivity Retired {}\n";
    const REVISED_ITEM: &str = "fn created() {}\n";

    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = parse_source_in(
        &mut syntax,
        "arcweft-test://proof/hir-module-resolution-revisions",
        "proof/hir-module-resolution-revisions.arcw",
        INITIAL_SOURCE,
    );
    assert!(
        initial.diagnostics().is_empty(),
        "{:?}",
        initial.diagnostics()
    );
    let initial_items = initial.items().unwrap();
    assert_eq!(initial_items.len(), 2);
    let stable_syntax = initial_items[0].id();
    let retired_syntax = initial_items[1].id();

    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let old = lower(&mut database, &initial, &key);
    let stable_item = old.item_for_syntax(stable_syntax).unwrap();
    let retired_item = old.item_for_syntax(retired_syntax).unwrap();
    let old_snapshot = old.snapshot_id();

    let edit_start = INITIAL_SOURCE.find("activity Retired {}").unwrap();
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(edit_start, INITIAL_SOURCE.len()))
                    .unwrap(),
                REVISED_ITEM,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    assert!(
        revised.diagnostics().is_empty(),
        "{:?}",
        revised.diagnostics()
    );
    assert_eq!(
        initial.snapshot_id().lineage(),
        revised.snapshot_id().lineage()
    );
    assert_ne!(initial.snapshot_id(), revised.snapshot_id());
    let revised_items = revised.items().unwrap();
    assert_eq!(revised_items.len(), 2);
    assert_eq!(revised_items[0].id(), stable_syntax);
    let created_syntax = revised_items[1].id();
    assert_ne!(created_syntax, retired_syntax);

    let current = lower(&mut database, &revised, &key);
    let created_item = current.item_for_syntax(created_syntax).unwrap();
    assert_eq!(current.item_for_syntax(stable_syntax), Ok(stable_item));
    assert_ne!(created_item, retired_item);

    let old_provenance = old.provenance();
    assert_eq!(old_provenance.syntax_snapshot(), initial.snapshot_id());
    assert_eq!(
        old_provenance.source_snapshot(),
        initial.source_snapshot_id()
    );
    assert_eq!(
        old_provenance.source_identity(),
        initial.document().identity()
    );
    assert!(Arc::ptr_eq(
        old_provenance.document(),
        initial.document_lease()
    ));
    let current_provenance = current.provenance();
    assert_eq!(current_provenance.syntax_snapshot(), revised.snapshot_id());
    assert_eq!(
        current_provenance.source_snapshot(),
        revised.source_snapshot_id()
    );
    assert_eq!(
        current_provenance.source_identity(),
        revised.document().identity()
    );
    assert!(Arc::ptr_eq(
        current_provenance.document(),
        revised.document_lease()
    ));
    assert_ne!(
        old_provenance.source_identity(),
        current_provenance.source_identity()
    );

    assert!(old.resolve_item(stable_item).is_ok());
    assert!(current.resolve_item(stable_item).is_ok());
    let old_stable_metadata = old.metadata(stable_item).unwrap();
    let current_stable_metadata = current.metadata(stable_item).unwrap();
    assert_eq!(old_stable_metadata.born(), current_stable_metadata.born());
    assert_eq!(old_stable_metadata.retired_at(), None);
    assert_eq!(current_stable_metadata.retired_at(), None);
    assert_eq!(
        old_stable_metadata.source_site().source_identity(),
        initial.document().identity()
    );
    assert_eq!(
        current_stable_metadata.source_site().source_identity(),
        revised.document().identity()
    );

    assert!(old.resolve_item(retired_item).is_ok());
    assert!(old.metadata(retired_item).is_ok());
    assert!(matches!(
        current.resolve_item(retired_item),
        Err(IdResolveError::Retired {
            id,
            snapshot,
            retired_at,
        }) if id == retired_item.raw().view()
            && snapshot == current.snapshot_id()
            && retired_at == current.snapshot_id().revision()
    ));
    assert!(matches!(
        current.metadata(retired_item),
        Err(IdResolveError::Retired {
            id,
            snapshot,
            retired_at,
        }) if id == retired_item.raw().view()
            && snapshot == current.snapshot_id()
            && retired_at == current.snapshot_id().revision()
    ));
    assert!(matches!(
        current.item_for_syntax(retired_syntax),
        Err(HirSourceLookupError::NotLowered {
            syntax,
            expected: HirIdKind::Item,
        }) if syntax == retired_syntax
    ));

    assert!(current.resolve_item(created_item).is_ok());
    assert!(current.metadata(created_item).is_ok());
    assert!(matches!(
        old.resolve_item(created_item),
        Err(IdResolveError::NotYetLive { id, snapshot, born })
            if id == created_item.raw().view()
                && snapshot == old.snapshot_id()
                && born == current.snapshot_id().revision()
    ));
    assert!(matches!(
        old.metadata(created_item),
        Err(IdResolveError::NotYetLive { id, snapshot, born })
            if id == created_item.raw().view()
                && snapshot == old.snapshot_id()
                && born == current.snapshot_id().revision()
    ));
    assert!(matches!(
        old.item_for_syntax(created_syntax),
        Err(HirSourceLookupError::NotLowered {
            syntax,
            expected: HirIdKind::Item,
        }) if syntax == created_syntax
    ));

    let retained_old = database.snapshot(old_snapshot).unwrap();
    let retained_current = database.snapshot(current.snapshot_id()).unwrap();
    let current_lookup = database.current(&revision_key(&key, &revised)).unwrap();
    assert!(Arc::ptr_eq(&old, &retained_old));
    assert!(Arc::ptr_eq(&current, &retained_current));
    assert!(Arc::ptr_eq(&current, &current_lookup));
    assert!(!Arc::ptr_eq(&old, &current));
}
