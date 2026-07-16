use std::sync::Arc;

use arcweft_character::{
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    symbol::CharacterSymbolDescriptor,
};
use arcweft_lang_hir::symbol::{ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_source::{SourceName, identity::SourceSnapshotId};

use crate::{
    character_definition::{
        CharacterDefinitionIssue, CharacterDefinitionNotApplicable, CharacterDefinitionQueryResult,
        CharacterDefinitionRequestBudget, CharacterDefinitionResourceError,
        CharacterDefinitionWorkKind, CharacterReferenceFact, CharacterReferenceInventory,
        CharacterReferenceResolution, query_character_definition,
    },
    registration::{CharacterDefinitionLimitKind, RegisteredSemanticWorld},
    test_support::character_project::{CharacterProjectFixture, source_document},
};

fn collect(
    source: &str,
) -> (
    CharacterProjectFixture,
    CharacterReferenceInventory,
    CharacterDefinitionRequestBudget,
) {
    let fixture = CharacterProjectFixture::new(source);
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let inventory = fixture
        .collect(&mut budget)
        .expect("real parser/checker pipeline collects references");
    assert!(budget.consumed() > 0);
    (fixture, inventory, budget)
}

fn assert_resolved(
    source: &str,
    cursor_spelling: &str,
    expected: impl FnOnce(&CharacterSymbolDescriptor) -> bool,
) {
    let (fixture, inventory, _) = collect(source);
    let cursor = fixture
        .source()
        .text()
        .rfind(cursor_spelling)
        .expect("cursor spelling belongs to fixture source");
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let result = query_character_definition(
        fixture.world(),
        &inventory,
        fixture.source().identity(),
        cursor,
        &mut budget,
    );
    let CharacterDefinitionQueryResult::Resolved(definition) = result else {
        panic!(
            "reference must resolve through the registered character index: {result:?}; facts={:?}",
            inventory.facts().collect::<Vec<_>>()
        );
    };
    assert!(expected(definition.descriptor()));
    assert_eq!(definition.declarations().len(), 1);
    assert!(budget.consumed() > 0);
}

fn query(
    fixture: &CharacterProjectFixture,
    inventory: &CharacterReferenceInventory,
    cursor_spelling: &str,
) -> (
    CharacterDefinitionQueryResult,
    CharacterDefinitionRequestBudget,
) {
    let cursor = fixture
        .source()
        .text()
        .rfind(cursor_spelling)
        .expect("cursor spelling belongs to fixture source");
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let result = query_character_definition(
        fixture.world(),
        inventory,
        fixture.source().identity(),
        cursor,
        &mut budget,
    );
    (result, budget)
}

fn only_fact(inventory: &CharacterReferenceInventory) -> &CharacterReferenceFact {
    let mut facts = inventory.facts();
    let fact = facts.next().expect("one character reference fact");
    assert!(
        facts.next().is_none(),
        "fixture must contain exactly one fact"
    );
    fact
}

fn look_candidates(count: usize) -> Vec<CharacterSymbolDescriptor> {
    (0..count)
        .map(|index| CharacterSymbolDescriptor::Look {
            character: CharacterId::try_new(format!("character.c{index:03}"))
                .expect("generated character identity"),
            look: CharacterLookId::try_new("normal").expect("normal look"),
        })
        .collect()
}

fn world_with_member_candidates(
    fixture: &CharacterProjectFixture,
    candidates: impl IntoIterator<Item = CharacterSymbolDescriptor>,
) -> RegisteredSemanticWorld {
    let mut world = fixture.world().clone();
    let index = world
        .character_definition_index()
        .with_member_candidates_for_test(candidates);
    world.character_definitions = Arc::new(index);
    world
}

#[test]
fn owner_canonical_pipeline() {
    assert_resolved(
        "fn main() -> Unit { accept_owner(@character.akane) }\n",
        "akane",
        |descriptor| matches!(descriptor, CharacterSymbolDescriptor::Owner { .. }),
    );
}

#[test]
fn owner_compact_pipeline() {
    assert_resolved(
        "fn main() -> Unit { accept_owner(@akane) }\n",
        "akane",
        |descriptor| matches!(descriptor, CharacterSymbolDescriptor::Owner { .. }),
    );
}

#[test]
fn owner_qualified_pipeline() {
    assert_resolved(
        "fn main() -> Unit { accept_owner(@crate.cast.akane) }\n",
        "akane",
        |descriptor| matches!(descriptor, CharacterSymbolDescriptor::Owner { .. }),
    );
}

#[test]
fn owner_alias_pipeline() {
    assert_resolved(
        "use crate.akane as hero\nfn main() -> Unit { accept_owner(@hero) }\n",
        "hero",
        |descriptor| matches!(descriptor, CharacterSymbolDescriptor::Owner { .. }),
    );
}

#[test]
fn owner_same_target_aliases() {
    let source = "use crate.akane as hero\nuse crate.akane as speaker\nfn main() -> Unit { accept_owner(@hero)\naccept_owner(@speaker) }\n";
    let (_, inventory, budget) = collect(source);
    let descriptors = inventory
        .facts()
        .map(|fact| match fact.resolution() {
            CharacterReferenceResolution::Resolved(descriptor) => descriptor.clone(),
            CharacterReferenceResolution::Unresolved(issue) => {
                panic!("same-target aliases must resolve: {issue:?}")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(descriptors.len(), 2);
    assert_eq!(descriptors[0], descriptors[1]);
    assert_eq!(
        budget
            .transcript_for_test()
            .iter()
            .filter(|kind| **kind == CharacterDefinitionWorkKind::ProjectSymbolCandidate)
            .count(),
        2
    );
}

#[test]
fn owner_unknown_charge() {
    let source = "fn main() -> Unit { accept_owner(@missing) }\n";
    let (fixture, inventory, budget) = collect(source);
    let fact = only_fact(&inventory);
    assert!(matches!(
        fact.resolution(),
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::UnknownOwner { .. })
    ));
    assert!(
        !budget
            .transcript_for_test()
            .contains(&CharacterDefinitionWorkKind::ProjectSymbolCandidate)
    );
    assert_eq!(
        budget
            .transcript_for_test()
            .iter()
            .filter(|kind| **kind == CharacterDefinitionWorkKind::AdmittedErrorCandidate)
            .count(),
        1
    );

    let (result, query_budget) = query(&fixture, &inventory, "missing");
    assert!(matches!(
        result,
        CharacterDefinitionQueryResult::Unresolved(CharacterDefinitionIssue::UnknownOwner { .. })
    ));
    assert_eq!(
        query_budget.transcript_for_test().last().copied(),
        Some(CharacterDefinitionWorkKind::AdmittedErrorCandidate)
    );
}

#[test]
fn owner_wrong_kind_charge() {
    let source = "fn helper() -> Unit { () }\nfn main() -> Unit { accept_owner(@helper) }\n";
    let (_, inventory, budget) = collect(source);
    let fact = only_fact(&inventory);
    assert!(matches!(
        fact.resolution(),
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::WrongOwnerKind { .. })
    ));
    let transcript = budget.transcript_for_test();
    let project = transcript
        .iter()
        .position(|kind| *kind == CharacterDefinitionWorkKind::ProjectSymbolCandidate)
        .expect("resolved wrong-kind candidate is charged");
    let error = transcript
        .iter()
        .position(|kind| *kind == CharacterDefinitionWorkKind::AdmittedErrorCandidate)
        .expect("wrong-kind error envelope is charged");
    assert!(project < error);
}

#[test]
fn inventory_error_admission_exhaustion_has_resource_precedence() {
    let source = "fn main() -> Unit { accept_owner(@missing) }\n";
    let fixture = CharacterProjectFixture::new(source);
    let mut complete = CharacterDefinitionRequestBudget::for_request();
    fixture
        .collect(&mut complete)
        .expect("unknown owner is a retained semantic issue");
    let envelope_index = complete
        .transcript_for_test()
        .iter()
        .position(|kind| *kind == CharacterDefinitionWorkKind::AdmittedErrorCandidate)
        .expect("unknown owner charges one envelope");
    let maximum = u64::try_from(envelope_index).expect("small fixture transcript");
    let mut constrained = CharacterDefinitionRequestBudget::with_maximum_for_test(maximum);
    let error = fixture
        .collect(&mut constrained)
        .expect_err("error envelope one-over must suppress the semantic issue");
    assert_eq!(
        error,
        super::super::CharacterReferenceInventoryError::Limit {
            kind: CharacterDefinitionLimitKind::QueryWork,
            observed: maximum + 1,
            maximum,
        }
    );
}

#[test]
fn pipeline_owner_end_to_end() {
    owner_canonical_pipeline();
}

#[test]
fn pipeline_look_end_to_end() {
    assert_resolved(
        "fn main() -> Unit { accept_look(.normal) }\n",
        "normal",
        |descriptor| matches!(descriptor, CharacterSymbolDescriptor::Look { .. }),
    );
}

#[test]
fn member_look_expected() {
    let (_, _, budget) = collect("fn main() -> Unit { accept_look(.normal) }\n");
    assert!(
        budget
            .transcript_for_test()
            .contains(&CharacterDefinitionWorkKind::TypedMemberCandidate)
    );
}

#[test]
fn pipeline_part_end_to_end() {
    assert_resolved(
        "fn main() -> Unit { accept_part(.body) }\n",
        "body",
        |descriptor| matches!(descriptor, CharacterSymbolDescriptor::Part { .. }),
    );
}

#[test]
fn member_part_expected() {
    let (_, _, budget) = collect("fn main() -> Unit { accept_part(.body) }\n");
    assert!(
        budget
            .transcript_for_test()
            .contains(&CharacterDefinitionWorkKind::TypedMemberCandidate)
    );
}

#[test]
fn pipeline_variant_end_to_end() {
    assert_resolved(
        "fn main() -> Unit { accept_variant(.default) }\n",
        "default",
        |descriptor| matches!(descriptor, CharacterSymbolDescriptor::Variant { .. }),
    );
}

#[test]
fn member_variant_expected() {
    let (_, _, budget) = collect("fn main() -> Unit { accept_variant(.default) }\n");
    assert!(
        budget
            .transcript_for_test()
            .contains(&CharacterDefinitionWorkKind::TypedMemberCandidate)
    );
}

#[test]
fn member_unique_no_context() {
    assert_resolved(
        "fn main() -> Unit { accept_any(.normal) }\n",
        "normal",
        |descriptor| matches!(descriptor, CharacterSymbolDescriptor::Look { .. }),
    );
}

#[test]
fn member_wrong_family() {
    let (_, inventory, budget) = collect("fn main() -> Unit { accept_part(.normal) }\n");
    let fact = only_fact(&inventory);
    assert!(matches!(
        fact.resolution(),
        CharacterReferenceResolution::Unresolved(
            CharacterDefinitionIssue::WrongNominalFamily { .. }
        )
    ));
    assert!(
        budget
            .transcript_for_test()
            .contains(&CharacterDefinitionWorkKind::TypedMemberCandidate)
    );
}

#[test]
fn member_cross_owner_ambiguous() {
    let source = "fn main() -> Unit { accept_any(.normal) }\n";
    let fixture = CharacterProjectFixture::new(source);
    let world = world_with_member_candidates(&fixture, look_candidates(2));
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let inventory = fixture
        .collect_with_world(&world, &mut budget)
        .expect("bounded ambiguous members remain a semantic issue");
    let CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::AmbiguousMember {
        candidates,
        ..
    }) = only_fact(&inventory).resolution()
    else {
        panic!("two untyped look candidates must remain ambiguous");
    };
    assert_eq!(candidates.len(), 2);
    assert_eq!(
        budget
            .transcript_for_test()
            .iter()
            .filter(|kind| **kind == CharacterDefinitionWorkKind::TypedMemberCandidate)
            .count(),
        2
    );
}

#[test]
fn member_wrong_part() {
    let source = "fn main() -> Unit { accept_variant(.alt) }\n";
    let fixture = CharacterProjectFixture::new(source);
    let candidates = [CharacterSymbolDescriptor::Variant {
        character: CharacterId::try_new("character.akane").expect("akane identity"),
        part: CharacterPartId::try_new("head").expect("head part"),
        variant: CharacterVariantId::try_new("alt").expect("alternate variant"),
    }];
    let world = world_with_member_candidates(&fixture, candidates);
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let inventory = fixture
        .collect_with_world(&world, &mut budget)
        .expect("wrong owning part remains a semantic issue");
    assert!(matches!(
        only_fact(&inventory).resolution(),
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::WrongOwningPart { .. })
    ));
}

#[test]
fn member_candidate_permutation() {
    let source = "fn main() -> Unit { accept_any(.normal) }\n";
    let fixture = CharacterProjectFixture::new(source);
    let candidates = look_candidates(3);
    let mut reversed = candidates.clone();
    reversed.reverse();
    let mut first_budget = CharacterDefinitionRequestBudget::for_request();
    let first = fixture
        .collect_with_world(
            &world_with_member_candidates(&fixture, candidates),
            &mut first_budget,
        )
        .expect("first permutation collects");
    let mut second_budget = CharacterDefinitionRequestBudget::for_request();
    let second = fixture
        .collect_with_world(
            &world_with_member_candidates(&fixture, reversed),
            &mut second_budget,
        )
        .expect("reversed permutation collects");
    assert_eq!(
        first.facts().cloned().collect::<Vec<_>>(),
        second.facts().cloned().collect::<Vec<_>>()
    );
    assert_eq!(
        first_budget.transcript_for_test(),
        second_budget.transcript_for_test()
    );
}

#[test]
fn member_candidate_limit_exact() {
    let source = "fn main() -> Unit { accept_any(.normal) }\n";
    let fixture = CharacterProjectFixture::new(source);
    let world = world_with_member_candidates(&fixture, look_candidates(256));
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let inventory = fixture
        .collect_with_world(&world, &mut budget)
        .expect("256 member candidates remain within the existing limit");
    let CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::AmbiguousMember {
        candidates,
        ..
    }) = only_fact(&inventory).resolution()
    else {
        panic!("256 untyped look candidates must remain ambiguous");
    };
    assert_eq!(candidates.len(), 256);
    assert_eq!(
        budget
            .transcript_for_test()
            .iter()
            .filter(|kind| **kind == CharacterDefinitionWorkKind::TypedMemberCandidate)
            .count(),
        256
    );
    assert_eq!(
        budget
            .transcript_for_test()
            .iter()
            .filter(|kind| **kind == CharacterDefinitionWorkKind::AdmittedErrorCandidate)
            .count(),
        257
    );
}

#[test]
fn member_candidate_limit_one_over() {
    let source = "fn main() -> Unit { accept_any(.normal) }\n";
    let fixture = CharacterProjectFixture::new(source);
    let world = world_with_member_candidates(&fixture, look_candidates(257));
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let error = fixture
        .collect_with_world(&world, &mut budget)
        .expect_err("candidate 257 is rejected before issue payload cloning");
    assert_eq!(
        error,
        super::super::CharacterReferenceInventoryError::Limit {
            kind: CharacterDefinitionLimitKind::Candidates,
            observed: 257,
            maximum: 256,
        }
    );
    assert!(
        !budget
            .transcript_for_test()
            .contains(&CharacterDefinitionWorkKind::AdmittedErrorCandidate)
    );
    assert_eq!(
        budget.transcript_for_test().last(),
        Some(&CharacterDefinitionWorkKind::TypedMemberCandidate)
    );
    assert_eq!(
        budget
            .transcript_for_test()
            .iter()
            .filter(|kind| **kind == CharacterDefinitionWorkKind::TypedMemberCandidate)
            .count(),
        257
    );
}

#[test]
fn member_unknown_expected() {
    let (_, inventory, budget) = collect("fn main() -> Unit { accept_look(.missing) }\n");
    let fact = only_fact(&inventory);
    assert!(matches!(
        fact.resolution(),
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::UnknownMember {
            expected: Some(_),
            ..
        })
    ));
    assert_eq!(
        budget
            .transcript_for_test()
            .iter()
            .filter(|kind| **kind == CharacterDefinitionWorkKind::AdmittedErrorCandidate)
            .count(),
        1
    );
}

#[test]
fn member_unknown_no_context() {
    let (_, inventory, _) = collect("fn main() -> Unit { accept_any(.missing) }\n");
    let fact = only_fact(&inventory);
    assert!(matches!(
        fact.resolution(),
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::UnknownMember {
            expected: None,
            ..
        })
    ));
}

#[test]
fn cursor_qualification() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, inventory, _) = collect(source);
    let (result, _) = query(&fixture, &inventory, "character");
    assert_eq!(
        result,
        CharacterDefinitionQueryResult::NotApplicable(
            CharacterDefinitionNotApplicable::Qualification
        )
    );
}

#[test]
fn cursor_delimiter() {
    let source = "fn main() -> Unit { accept_look(.normal) }\n";
    let (fixture, inventory, _) = collect(source);
    let cursor = fixture
        .source()
        .text()
        .rfind(".normal")
        .expect("local-member delimiter");
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let result = query_character_definition(
        fixture.world(),
        &inventory,
        fixture.source().identity(),
        cursor,
        &mut budget,
    );
    assert_eq!(
        result,
        CharacterDefinitionQueryResult::NotApplicable(CharacterDefinitionNotApplicable::Delimiter)
    );
}

#[test]
fn cursor_end_boundary() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, inventory, _) = collect(source);
    let end = inventory
        .facts()
        .next()
        .expect("owner fact")
        .selection_span()
        .range()
        .end();
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let result = query_character_definition(
        fixture.world(),
        &inventory,
        fixture.source().identity(),
        end,
        &mut budget,
    );
    assert_eq!(
        result,
        CharacterDefinitionQueryResult::NotApplicable(
            CharacterDefinitionNotApplicable::EndBoundary
        )
    );
}

#[test]
fn cursor_non_character() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, inventory, _) = collect(source);
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let result = query_character_definition(
        fixture.world(),
        &inventory,
        fixture.source().identity(),
        0,
        &mut budget,
    );
    assert_eq!(
        result,
        CharacterDefinitionQueryResult::NotApplicable(
            CharacterDefinitionNotApplicable::NonCharacterToken
        )
    );
}

#[test]
fn core_query_budget_exhaustion_is_terminal() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, inventory, _) = collect(source);
    let cursor = fixture.source().text().rfind("akane").expect("owner");
    let mut budget = CharacterDefinitionRequestBudget::with_maximum_for_test(0);
    let result = query_character_definition(
        fixture.world(),
        &inventory,
        fixture.source().identity(),
        cursor,
        &mut budget,
    );
    assert_eq!(
        result,
        CharacterDefinitionQueryResult::Exhausted(CharacterDefinitionResourceError::Limit {
            kind: CharacterDefinitionLimitKind::QueryWork,
            observed: 1,
            maximum: 0,
        })
    );
    assert_eq!(budget.consumed(), 1);
}

#[test]
fn cursor_recovered() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, mut inventory, _) = collect(source);
    let fact = inventory
        .facts
        .first_mut()
        .expect("owner reference belongs to inventory");
    fact.resolution =
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::RecoveredToken {
            source: fact.reference_span.clone(),
        });
    let (result, budget) = query(&fixture, &inventory, "akane");
    assert!(matches!(
        result,
        CharacterDefinitionQueryResult::Unresolved(CharacterDefinitionIssue::RecoveredToken { .. })
    ));
    assert_eq!(
        budget.transcript_for_test().last(),
        Some(&CharacterDefinitionWorkKind::AdmittedErrorCandidate)
    );
}

#[test]
fn cursor_ambiguous_facts() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, mut inventory, _) = collect(source);
    inventory.facts.push(
        inventory
            .facts
            .first()
            .expect("owner reference belongs to inventory")
            .clone(),
    );
    let (result, budget) = query(&fixture, &inventory, "akane");
    let CharacterDefinitionQueryResult::Integrity(
        super::super::CharacterDefinitionIntegrityError::AmbiguousCursorFacts {
            candidates, ..
        },
    ) = result
    else {
        panic!("duplicate selected facts must be rejected");
    };
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        budget
            .transcript_for_test()
            .iter()
            .filter(|kind| **kind == CharacterDefinitionWorkKind::AdmittedErrorCandidate)
            .count(),
        2
    );
}

#[test]
fn query_missing_declaration() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, mut inventory, _) = collect(source);
    inventory
        .facts
        .first_mut()
        .expect("owner reference belongs to inventory")
        .resolution = CharacterReferenceResolution::Resolved(CharacterSymbolDescriptor::Owner {
        character: CharacterId::try_new("character.missing").expect("missing character identity"),
    });
    let (result, budget) = query(&fixture, &inventory, "akane");
    assert!(matches!(
        result,
        CharacterDefinitionQueryResult::Integrity(
            super::super::CharacterDefinitionIntegrityError::MissingDeclaration { .. }
        )
    ));
    assert_eq!(
        budget.transcript_for_test().last(),
        Some(&CharacterDefinitionWorkKind::AdmittedErrorCandidate)
    );
}

#[test]
fn query_missing_owned_document() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, inventory, _) = collect(source);
    let descriptor = match only_fact(&inventory).resolution() {
        CharacterReferenceResolution::Resolved(descriptor) => descriptor,
        CharacterReferenceResolution::Unresolved(issue) => {
            panic!("owner reference must resolve before index tampering: {issue:?}")
        }
    };
    let declaration = fixture
        .world()
        .character_definition_index()
        .declaration(descriptor)
        .expect("owner declaration belongs to accepted index")
        .sources()
        .next()
        .expect("accepted declaration set is non-empty");
    let mut world = fixture.world().clone();
    world.character_definitions = Arc::new(
        world
            .character_definition_index()
            .without_owned_document_for_test(declaration.selection_span().source()),
    );
    let cursor = fixture.source().text().rfind("akane").expect("owner");
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let result = query_character_definition(
        &world,
        &inventory,
        fixture.source().identity(),
        cursor,
        &mut budget,
    );
    assert!(matches!(
        result,
        CharacterDefinitionQueryResult::Integrity(
            super::super::CharacterDefinitionIntegrityError::MissingOwnedDocument { .. }
        )
    ));
    let transcript = budget.transcript_for_test();
    let declaration_copy = transcript
        .iter()
        .position(|kind| *kind == CharacterDefinitionWorkKind::DeclarationCopy)
        .expect("declaration copy is charged");
    assert_eq!(
        &transcript[declaration_copy..],
        &[
            CharacterDefinitionWorkKind::DeclarationCopy,
            CharacterDefinitionWorkKind::IdentityCheck,
            CharacterDefinitionWorkKind::AdmittedErrorCandidate,
        ]
    );
}

#[test]
fn declarations_exact_64() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, inventory, _) = collect(source);
    let descriptor = match only_fact(&inventory).resolution() {
        CharacterReferenceResolution::Resolved(descriptor) => descriptor,
        CharacterReferenceResolution::Unresolved(issue) => {
            panic!("owner reference must resolve before index tampering: {issue:?}")
        }
    };
    let mut world = fixture.world().clone();
    world.character_definitions = Arc::new(
        world
            .character_definition_index()
            .with_declaration_source_count_for_test(descriptor, 64),
    );
    let cursor = fixture.source().text().rfind("akane").expect("owner");
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let result = query_character_definition(
        &world,
        &inventory,
        fixture.source().identity(),
        cursor,
        &mut budget,
    );
    let CharacterDefinitionQueryResult::Resolved(definition) = result else {
        panic!("64 declaration copies must remain within the existing limit");
    };
    assert_eq!(definition.declarations().len(), 64);
    assert_eq!(
        budget
            .transcript_for_test()
            .iter()
            .filter(|kind| **kind == CharacterDefinitionWorkKind::DeclarationCopy)
            .count(),
        64
    );
    let transcript = budget.transcript_for_test();
    let first_copy = transcript
        .iter()
        .position(|kind| *kind == CharacterDefinitionWorkKind::DeclarationCopy)
        .expect("first declaration copy is charged");
    assert_eq!(transcript.len() - first_copy, 128);
    assert!(transcript[first_copy..].chunks_exact(2).all(|pair| pair
        == [
            CharacterDefinitionWorkKind::DeclarationCopy,
            CharacterDefinitionWorkKind::IdentityCheck,
        ]));
}

#[test]
fn declarations_one_over_65() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, inventory, _) = collect(source);
    let descriptor = match only_fact(&inventory).resolution() {
        CharacterReferenceResolution::Resolved(descriptor) => descriptor,
        CharacterReferenceResolution::Unresolved(issue) => {
            panic!("owner reference must resolve before index tampering: {issue:?}")
        }
    };
    let mut world = fixture.world().clone();
    world.character_definitions = Arc::new(
        world
            .character_definition_index()
            .with_declaration_source_count_for_test(descriptor, 65),
    );
    let cursor = fixture.source().text().rfind("akane").expect("owner");
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let result = query_character_definition(
        &world,
        &inventory,
        fixture.source().identity(),
        cursor,
        &mut budget,
    );
    assert_eq!(
        result,
        CharacterDefinitionQueryResult::Exhausted(CharacterDefinitionResourceError::Limit {
            kind: CharacterDefinitionLimitKind::DeclarationSourcesPerDescriptor,
            observed: 65,
            maximum: 64,
        })
    );
    assert_eq!(
        budget
            .transcript_for_test()
            .iter()
            .filter(|kind| **kind == CharacterDefinitionWorkKind::DeclarationCopy)
            .count(),
        65
    );
    let transcript = budget.transcript_for_test();
    let first_copy = transcript
        .iter()
        .position(|kind| *kind == CharacterDefinitionWorkKind::DeclarationCopy)
        .expect("first declaration copy is charged");
    let suffix = &transcript[first_copy..];
    assert_eq!(suffix.len(), 129);
    assert!(suffix[..128].chunks_exact(2).all(|pair| pair
        == [
            CharacterDefinitionWorkKind::DeclarationCopy,
            CharacterDefinitionWorkKind::IdentityCheck,
        ]));
    assert_eq!(
        suffix.last(),
        Some(&CharacterDefinitionWorkKind::DeclarationCopy)
    );
}

#[test]
fn stale_world() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, mut inventory, _) = collect(source);
    let current = fixture.world().symbols().world();
    inventory.world = ProjectSymbolWorldId::try_new(
        current.package().clone(),
        current.root_document().clone(),
        "stale-character-definition",
    )
    .expect("different valid world identity");
    let (result, budget) = query(&fixture, &inventory, "akane");
    assert!(matches!(
        result,
        CharacterDefinitionQueryResult::Stale(super::super::CharacterDefinitionStale::World { .. })
    ));
    assert_eq!(
        budget.transcript_for_test().last(),
        Some(&CharacterDefinitionWorkKind::AdmittedErrorCandidate)
    );
}

#[test]
fn stale_symbol_revision() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, mut inventory, _) = collect(source);
    let other = source_document(
        "arcweft-project://registration-tests/src/other.arcw",
        "fn other() -> Unit { () }\n",
    );
    inventory.symbol_revision = ProjectSymbolRevision::try_for_documents([other.identity()])
        .expect("different valid source-set revision");
    assert_ne!(
        inventory.symbol_revision,
        *fixture.world().symbols().revision()
    );
    let (result, budget) = query(&fixture, &inventory, "akane");
    assert!(matches!(
        result,
        CharacterDefinitionQueryResult::Stale(
            super::super::CharacterDefinitionStale::SymbolRevision { .. }
        )
    ));
    assert_eq!(
        budget.transcript_for_test().last(),
        Some(&CharacterDefinitionWorkKind::AdmittedErrorCandidate)
    );
}

#[test]
fn stale_document() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, inventory, _) = collect(source);
    let other = source_document(
        "arcweft-project://registration-tests/src/other.arcw",
        source,
    );
    let cursor = fixture.source().text().rfind("akane").expect("owner");
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    let result = query_character_definition(
        fixture.world(),
        &inventory,
        other.identity(),
        cursor,
        &mut budget,
    );
    assert!(matches!(
        result,
        CharacterDefinitionQueryResult::Stale(
            super::super::CharacterDefinitionStale::Document { .. }
        )
    ));
    assert_eq!(
        budget.transcript_for_test().last(),
        Some(&CharacterDefinitionWorkKind::AdmittedErrorCandidate)
    );
}

#[test]
fn snapshot_not_identity_dependency() {
    let source = "fn main() -> Unit { accept_owner(@character.akane) }\n";
    let (fixture, inventory, _) = collect(source);
    let (without_snapshot, without_budget) = query(&fixture, &inventory, "akane");
    let mut with_snapshot_inventory = inventory.clone();
    with_snapshot_inventory.syntax_snapshot = Some(SourceSnapshotId::initial(SourceName::path(
        "unrelated-editor-lineage.arcw",
    )));
    let (with_snapshot, with_budget) = query(&fixture, &with_snapshot_inventory, "akane");
    assert_eq!(with_snapshot, without_snapshot);
    assert_eq!(
        with_budget.transcript_for_test(),
        without_budget.transcript_for_test()
    );
}
