//! Charged core character-definition cursor query.

use arcweft_source::SourceDocumentIdentity;

use crate::registration::{
    CharacterDefinitionLimitKind, CharacterDefinitionLimits, RegisteredSemanticWorld,
};

use super::{
    CharacterDefinition, CharacterDefinitionIntegrityError, CharacterDefinitionIssue,
    CharacterDefinitionNotApplicable, CharacterDefinitionQueryResult,
    CharacterDefinitionRequestBudget, CharacterDefinitionResourceError, CharacterDefinitionStale,
    CharacterDefinitionWorkKind, CharacterReferenceForm, CharacterReferenceInventory,
    CharacterReferenceResolution, admit_error_payload, admit_nonresource_error,
};

/// Resolves one byte cursor through an exact current reference inventory.
pub fn query_character_definition(
    world: &RegisteredSemanticWorld,
    inventory: &CharacterReferenceInventory,
    document: &SourceDocumentIdentity,
    cursor: usize,
    budget: &mut CharacterDefinitionRequestBudget,
) -> CharacterDefinitionQueryResult {
    query_character_definition_inner(world, inventory, document, cursor, budget)
        .unwrap_or_else(CharacterDefinitionQueryResult::Exhausted)
}

fn query_character_definition_inner(
    world: &RegisteredSemanticWorld,
    inventory: &CharacterReferenceInventory,
    document: &SourceDocumentIdentity,
    cursor: usize,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<CharacterDefinitionQueryResult, CharacterDefinitionResourceError> {
    if let Some(outcome) = query_context_outcome(world, inventory, document, cursor, budget)? {
        return Ok(outcome);
    }

    let fact = match select_cursor_fact(inventory, cursor, budget)? {
        CursorSelection::Selected(fact) => fact,
        CursorSelection::Outcome(outcome) => return Ok(outcome),
    };
    let descriptor = match fact.resolution() {
        CharacterReferenceResolution::Resolved(descriptor) => descriptor,
        CharacterReferenceResolution::Unresolved(issue) => {
            return Ok(CharacterDefinitionQueryResult::Unresolved(admit_issue(
                budget, issue,
            )?));
        }
    };
    definition_for_descriptor(world, fact, descriptor, budget)
}

enum CursorSelection<'a> {
    Selected(&'a super::CharacterReferenceFact),
    Outcome(CharacterDefinitionQueryResult),
}

fn select_cursor_fact<'a>(
    inventory: &'a CharacterReferenceInventory,
    cursor: usize,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<CursorSelection<'a>, CharacterDefinitionResourceError> {
    let maximum_candidates = CharacterDefinitionLimits::PRODUCTION.candidates();
    let mut selected = Vec::new();
    for fact in inventory.facts() {
        budget.charge(CharacterDefinitionWorkKind::CursorFact)?;
        let selection = fact.selection_span().range();
        if selection.start() <= cursor && cursor < selection.end() {
            let observed = u64::try_from(selected.len())
                .ok()
                .and_then(|count| count.checked_add(1))
                .ok_or(CharacterDefinitionResourceError::ArithmeticOverflow {
                    counter: CharacterDefinitionLimitKind::Candidates,
                })?;
            if observed > maximum_candidates {
                return Err(CharacterDefinitionResourceError::Limit {
                    kind: CharacterDefinitionLimitKind::Candidates,
                    observed,
                    maximum: maximum_candidates,
                });
            }
            let Some(selection_width) = selection.end().checked_sub(selection.start()) else {
                admit_nonresource_error(budget, 0)?;
                return Ok(CursorSelection::Outcome(
                    CharacterDefinitionQueryResult::Integrity(
                        CharacterDefinitionIntegrityError::InvalidSourceRange {
                            source: fact.selection_span().clone(),
                        },
                    ),
                ));
            };
            let reference = fact.reference_span().range();
            let Some(reference_width) = reference.end().checked_sub(reference.start()) else {
                admit_nonresource_error(budget, 0)?;
                return Ok(CursorSelection::Outcome(
                    CharacterDefinitionQueryResult::Integrity(
                        CharacterDefinitionIntegrityError::InvalidSourceRange {
                            source: fact.reference_span().clone(),
                        },
                    ),
                ));
            };
            selected.push((fact, selection_width, reference_width));
        }
    }
    selected.sort_by(
        |(left, left_selection, left_reference), (right, right_selection, right_reference)| {
            left_selection
                .cmp(right_selection)
                .then_with(|| left_reference.cmp(right_reference))
                .then_with(|| left.cmp(right))
        },
    );
    if selected.len() > 1 {
        let candidate_refs = selected
            .iter()
            .filter_map(|(fact, _, _)| match fact.resolution() {
                CharacterReferenceResolution::Resolved(descriptor) => Some(descriptor),
                CharacterReferenceResolution::Unresolved(_) => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        admit_nonresource_error(budget, 0)?;
        let candidates = admit_error_payload(budget, candidate_refs.into_iter())?;
        return Ok(CursorSelection::Outcome(
            CharacterDefinitionQueryResult::Integrity(
                CharacterDefinitionIntegrityError::AmbiguousCursorFacts {
                    source: selected[0].0.selection_span().clone(),
                    candidates,
                },
            ),
        ));
    }
    let Some((fact, _, _)) = selected.first().copied() else {
        return classify_unselected_cursor(inventory, cursor, budget).map(CursorSelection::Outcome);
    };
    Ok(CursorSelection::Selected(fact))
}

fn definition_for_descriptor(
    world: &RegisteredSemanticWorld,
    fact: &super::CharacterReferenceFact,
    descriptor: &arcweft_character::symbol::CharacterSymbolDescriptor,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<CharacterDefinitionQueryResult, CharacterDefinitionResourceError> {
    let Some(set) = world.character_definition_index().declaration(descriptor) else {
        admit_nonresource_error(budget, 0)?;
        return Ok(CharacterDefinitionQueryResult::Integrity(
            CharacterDefinitionIntegrityError::MissingDeclaration {
                descriptor: descriptor.clone(),
            },
        ));
    };
    let mut declarations = Vec::new();
    let maximum = CharacterDefinitionLimits::PRODUCTION.declaration_sources_per_descriptor();
    for declaration in set.sources() {
        budget.charge(CharacterDefinitionWorkKind::DeclarationCopy)?;
        let observed = u64::try_from(declarations.len())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or(CharacterDefinitionResourceError::ArithmeticOverflow {
                counter: CharacterDefinitionLimitKind::DeclarationSourcesPerDescriptor,
            })?;
        if observed > maximum {
            return Err(CharacterDefinitionResourceError::Limit {
                kind: CharacterDefinitionLimitKind::DeclarationSourcesPerDescriptor,
                observed,
                maximum,
            });
        }
        declarations.push(declaration.clone());
        budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
        if world
            .character_definition_index()
            .document(declaration.selection_span().source())
            .is_none()
        {
            admit_nonresource_error(budget, 0)?;
            return Ok(CharacterDefinitionQueryResult::Integrity(
                CharacterDefinitionIntegrityError::MissingOwnedDocument {
                    source: declaration.selection_span().source().clone(),
                },
            ));
        }
    }
    Ok(CharacterDefinitionQueryResult::Resolved(
        CharacterDefinition {
            descriptor: descriptor.clone(),
            origin_selection: fact.selection_span().clone(),
            declarations,
        },
    ))
}

fn query_context_outcome(
    world: &RegisteredSemanticWorld,
    inventory: &CharacterReferenceInventory,
    document: &SourceDocumentIdentity,
    cursor: usize,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<Option<CharacterDefinitionQueryResult>, CharacterDefinitionResourceError> {
    let expected_world = world.symbols().world();
    for actual in [
        world.environment().world(),
        world.character_definition_index().world(),
    ] {
        budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
        if actual != expected_world {
            admit_nonresource_error(budget, 0)?;
            return Ok(Some(CharacterDefinitionQueryResult::Integrity(
                CharacterDefinitionIntegrityError::AcceptedWorldInvariant {
                    world: expected_world.clone(),
                    revision: *world.symbols().revision(),
                },
            )));
        }
    }
    let expected_revision = world.symbols().revision();
    for actual in [
        world.environment().symbol_revision(),
        world.character_definition_index().symbol_revision(),
    ] {
        budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
        if actual != expected_revision {
            admit_nonresource_error(budget, 0)?;
            return Ok(Some(CharacterDefinitionQueryResult::Integrity(
                CharacterDefinitionIntegrityError::AcceptedWorldInvariant {
                    world: expected_world.clone(),
                    revision: *expected_revision,
                },
            )));
        }
    }

    budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
    if inventory.world() != world.symbols().world() {
        admit_nonresource_error(budget, 0)?;
        return Ok(Some(CharacterDefinitionQueryResult::Stale(
            CharacterDefinitionStale::World {
                expected: world.symbols().world().clone(),
                actual: inventory.world().clone(),
            },
        )));
    }
    budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
    if inventory.symbol_revision() != world.symbols().revision() {
        admit_nonresource_error(budget, 0)?;
        return Ok(Some(CharacterDefinitionQueryResult::Stale(
            CharacterDefinitionStale::SymbolRevision {
                expected: *world.symbols().revision(),
                actual: *inventory.symbol_revision(),
            },
        )));
    }
    budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
    if inventory.document() != document {
        admit_nonresource_error(budget, 0)?;
        return Ok(Some(CharacterDefinitionQueryResult::Stale(
            CharacterDefinitionStale::Document {
                expected: inventory.document().clone(),
                actual: document.clone(),
            },
        )));
    }
    budget.charge(CharacterDefinitionWorkKind::CursorFact)?;
    let source_len = usize::try_from(document.source_len()).map_err(|_| {
        CharacterDefinitionResourceError::ArithmeticOverflow {
            counter: CharacterDefinitionLimitKind::QueryWork,
        }
    })?;
    if cursor >= source_len {
        return Ok(Some(CharacterDefinitionQueryResult::NotApplicable(
            CharacterDefinitionNotApplicable::EndBoundary,
        )));
    }
    Ok(None)
}

fn classify_unselected_cursor(
    inventory: &CharacterReferenceInventory,
    cursor: usize,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<CharacterDefinitionQueryResult, CharacterDefinitionResourceError> {
    for fact in inventory.facts() {
        budget.charge(CharacterDefinitionWorkKind::CursorFact)?;
        let selection = fact.selection_span().range();
        if cursor == selection.end() {
            return Ok(CharacterDefinitionQueryResult::NotApplicable(
                CharacterDefinitionNotApplicable::EndBoundary,
            ));
        }
        let reference = fact.reference_span().range();
        if reference.start() <= cursor && cursor < reference.end() {
            return Ok(CharacterDefinitionQueryResult::NotApplicable(
                match fact.form() {
                    CharacterReferenceForm::OwnerPath { .. } => {
                        CharacterDefinitionNotApplicable::Qualification
                    }
                    CharacterReferenceForm::LocalMember { .. } => {
                        CharacterDefinitionNotApplicable::Delimiter
                    }
                },
            ));
        }
    }
    Ok(CharacterDefinitionQueryResult::NotApplicable(
        CharacterDefinitionNotApplicable::NonCharacterToken,
    ))
}

fn admit_issue(
    budget: &mut CharacterDefinitionRequestBudget,
    issue: &CharacterDefinitionIssue,
) -> Result<CharacterDefinitionIssue, CharacterDefinitionResourceError> {
    admit_nonresource_error(budget, 0)?;
    match issue {
        CharacterDefinitionIssue::UnknownOwner { reference } => {
            Ok(CharacterDefinitionIssue::UnknownOwner {
                reference: reference.clone(),
            })
        }
        CharacterDefinitionIssue::AmbiguousAlias {
            reference,
            candidates,
        } => Ok(CharacterDefinitionIssue::AmbiguousAlias {
            reference: reference.clone(),
            candidates: admit_error_payload(budget, candidates.iter())?,
        }),
        CharacterDefinitionIssue::WrongOwnerKind { reference, actual } => {
            Ok(CharacterDefinitionIssue::WrongOwnerKind {
                reference: reference.clone(),
                actual: actual.clone(),
            })
        }
        CharacterDefinitionIssue::UnknownMember { spelling, expected } => {
            Ok(CharacterDefinitionIssue::UnknownMember {
                spelling: spelling.clone(),
                expected: expected.clone(),
            })
        }
        CharacterDefinitionIssue::AmbiguousMember {
            spelling,
            candidates,
        } => Ok(CharacterDefinitionIssue::AmbiguousMember {
            spelling: spelling.clone(),
            candidates: admit_error_payload(budget, candidates.iter())?,
        }),
        CharacterDefinitionIssue::AmbiguousSemanticContext {
            spelling,
            candidates,
        } => Ok(CharacterDefinitionIssue::AmbiguousSemanticContext {
            spelling: spelling.clone(),
            candidates: admit_error_payload(budget, candidates.iter())?,
        }),
        CharacterDefinitionIssue::WrongNominalFamily {
            spelling,
            expected,
            candidates,
        } => Ok(CharacterDefinitionIssue::WrongNominalFamily {
            spelling: spelling.clone(),
            expected: expected.clone(),
            candidates: admit_error_payload(budget, candidates.iter())?,
        }),
        CharacterDefinitionIssue::WrongOwningPart {
            spelling,
            expected,
            candidates,
        } => Ok(CharacterDefinitionIssue::WrongOwningPart {
            spelling: spelling.clone(),
            expected: expected.clone(),
            candidates: admit_error_payload(budget, candidates.iter())?,
        }),
        CharacterDefinitionIssue::RecoveredToken { source } => {
            Ok(CharacterDefinitionIssue::RecoveredToken {
                source: source.clone(),
            })
        }
    }
}
