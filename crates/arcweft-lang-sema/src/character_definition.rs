//! Request-scoped character references and Sans-I/O definition queries.

use arcweft_character::{
    id::{CharacterLookId, CharacterPartId, CharacterVariantId},
    symbol::CharacterSymbolDescriptor,
};
use arcweft_lang_hir::symbol::{
    ProjectSymbolResolutionError, ProjectSymbolRevision, ProjectSymbolTargetId,
    ProjectSymbolWorldId,
};
use arcweft_lang_syntax::{
    ast::{
        items::TypedSyntaxTree,
        module_path::CanonicalModulePath,
        symbol_path::{SpannedProjectSymbolPath, SymbolPath},
    },
    expr::{Expr, parse_expr},
    parser::recovery::ParseError,
};
use arcweft_source::{
    SourceDocument, SourceDocumentIdentity, SourceRange, SourceSpan, identity::SourceSnapshotId,
};
use thiserror::Error;

use crate::{
    check::{TypeCheckReport, TypeJudgmentSubject},
    registration::{
        CharacterDeclarationSource, CharacterDefinitionLimitKind, CharacterDefinitionLimits,
        ExternalOwnerLookupError, RegisteredCharacterResolutionError, RegisteredSemanticWorld,
    },
    types::{CharacterNominalType, EntityKind},
};

/// One request-scoped inventory bound to exact semantic and source identities.
#[derive(Clone, Debug)]
pub struct CharacterReferenceInventory {
    world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    document: SourceDocumentIdentity,
    module: CanonicalModulePath,
    syntax_snapshot: Option<SourceSnapshotId>,
    facts: Vec<CharacterReferenceFact>,
}

/// One parsed character reference and its typed resolution outcome.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CharacterReferenceFact {
    reference_span: SourceSpan,
    selection_span: SourceSpan,
    form: CharacterReferenceForm,
    resolution: CharacterReferenceResolution,
}

/// Authored form retained for definition and future rename policy.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CharacterReferenceForm {
    OwnerPath {
        path: SymbolPath,
    },
    LocalMember {
        spelling: String,
        expected: Option<CharacterNominalType>,
    },
}

/// Typed resolution stored with one current reference fact.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CharacterReferenceResolution {
    Resolved(CharacterSymbolDescriptor),
    Unresolved(CharacterDefinitionIssue),
}

/// Resolved occurrence boundary reserved for the later rename contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterReferenceOccurrence {
    descriptor: CharacterSymbolDescriptor,
    reference_span: SourceSpan,
    selection_span: SourceSpan,
    form: CharacterReferenceForm,
}

/// Exact current analysis values used to collect character references.
#[derive(Clone, Copy)]
pub struct CharacterReferenceInput<'a> {
    document: &'a SourceDocument,
    module: &'a CanonicalModulePath,
    typed_tree: &'a TypedSyntaxTree,
    type_report: &'a TypeCheckReport,
    parse_diagnostics: &'a [ParseError],
    syntax_snapshot: Option<&'a SourceSnapshotId>,
}

impl<'a> CharacterReferenceInput<'a> {
    #[allow(
        clippy::too_many_arguments,
        reason = "the request boundary keeps all independently checked source and semantic identities explicit"
    )]
    pub fn new(
        document: &'a SourceDocument,
        module: &'a CanonicalModulePath,
        typed_tree: &'a TypedSyntaxTree,
        type_report: &'a TypeCheckReport,
        parse_diagnostics: &'a [ParseError],
        syntax_snapshot: Option<&'a SourceSnapshotId>,
    ) -> Self {
        Self {
            document,
            module,
            typed_tree,
            type_report,
            parse_diagnostics,
            syntax_snapshot,
        }
    }
}

impl CharacterReferenceInventory {
    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub const fn symbol_revision(&self) -> &ProjectSymbolRevision {
        &self.symbol_revision
    }

    pub const fn document(&self) -> &SourceDocumentIdentity {
        &self.document
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn syntax_snapshot(&self) -> Option<&SourceSnapshotId> {
        self.syntax_snapshot.as_ref()
    }

    pub fn facts(&self) -> impl ExactSizeIterator<Item = &CharacterReferenceFact> {
        self.facts.iter()
    }

    pub fn resolved_occurrences(&self) -> impl Iterator<Item = CharacterReferenceOccurrence> + '_ {
        self.facts.iter().filter_map(|fact| {
            let CharacterReferenceResolution::Resolved(descriptor) = fact.resolution() else {
                return None;
            };
            Some(CharacterReferenceOccurrence {
                descriptor: descriptor.clone(),
                reference_span: fact.reference_span.clone(),
                selection_span: fact.selection_span.clone(),
                form: fact.form.clone(),
            })
        })
    }
}

impl CharacterReferenceFact {
    pub const fn reference_span(&self) -> &SourceSpan {
        &self.reference_span
    }

    pub const fn selection_span(&self) -> &SourceSpan {
        &self.selection_span
    }

    pub const fn form(&self) -> &CharacterReferenceForm {
        &self.form
    }

    pub const fn resolution(&self) -> &CharacterReferenceResolution {
        &self.resolution
    }
}

impl CharacterReferenceOccurrence {
    pub const fn descriptor(&self) -> &CharacterSymbolDescriptor {
        &self.descriptor
    }

    pub const fn reference_span(&self) -> &SourceSpan {
        &self.reference_span
    }

    pub const fn selection_span(&self) -> &SourceSpan {
        &self.selection_span
    }

    pub const fn form(&self) -> &CharacterReferenceForm {
        &self.form
    }
}

/// Recoverable semantic issue for a syntactically complete character reference.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CharacterDefinitionIssue {
    UnknownOwner {
        reference: SymbolPath,
    },
    AmbiguousAlias {
        reference: SymbolPath,
        candidates: Vec<ProjectSymbolTargetId>,
    },
    WrongOwnerKind {
        reference: SymbolPath,
        actual: ProjectSymbolTargetId,
    },
    UnknownMember {
        spelling: String,
        expected: Option<CharacterNominalType>,
    },
    AmbiguousMember {
        spelling: String,
        candidates: Vec<CharacterSymbolDescriptor>,
    },
    AmbiguousSemanticContext {
        spelling: String,
        candidates: Vec<CharacterNominalType>,
    },
    WrongNominalFamily {
        spelling: String,
        expected: CharacterNominalType,
        candidates: Vec<CharacterSymbolDescriptor>,
    },
    WrongOwningPart {
        spelling: String,
        expected: CharacterNominalType,
        candidates: Vec<CharacterSymbolDescriptor>,
    },
    RecoveredToken {
        source: SourceSpan,
    },
}

/// Failure to bind a reference inventory to one exact registered world.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterReferenceInventoryError {
    #[error("character reference inventory world is stale")]
    StaleWorld {
        expected: ProjectSymbolWorldId,
        actual: ProjectSymbolWorldId,
    },
    #[error("character reference inventory symbol revision is stale")]
    StaleSymbolRevision {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    #[error("character reference inventory document differs from the typed source")]
    DocumentMismatch {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    #[error("character reference inventory resource limit exceeded")]
    Limit {
        kind: CharacterDefinitionLimitKind,
        observed: u64,
        maximum: u64,
    },
    #[error("character reference inventory counter overflowed")]
    ArithmeticOverflow {
        counter: CharacterDefinitionLimitKind,
    },
}

/// Collects typed owner/member references from one current source analysis.
#[allow(
    clippy::result_large_err,
    reason = "the public contract preserves complete typed stale identities without boxing"
)]
pub fn collect_character_references(
    world: &RegisteredSemanticWorld,
    input: CharacterReferenceInput<'_>,
) -> Result<CharacterReferenceInventory, CharacterReferenceInventoryError> {
    ensure_world_integrity(world)?;
    if input.typed_tree.source() != input.document.text() {
        let actual = SourceDocument::try_new(
            input.document.identity().id().clone(),
            input.document.display_name().clone(),
            input.typed_tree.source(),
        )
        .map_err(|_| CharacterReferenceInventoryError::ArithmeticOverflow {
            counter: CharacterDefinitionLimitKind::SourceBytes,
        })?;
        return Err(CharacterReferenceInventoryError::DocumentMismatch {
            expected: input.document.identity().clone(),
            actual: actual.identity().clone(),
        });
    }

    let limits = CharacterDefinitionLimits::PRODUCTION;
    let mut work = 0_u64;
    let mut facts = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for judgment in &input.type_report.judgments {
        let TypeJudgmentSubject::Expr { kind, .. } = &judgment.subject else {
            continue;
        };
        if !matches!(*kind, "entity_ref" | "short_variant") {
            continue;
        }
        let Some(range) = judgment.source_range else {
            continue;
        };
        if !seen.insert((range, *kind)) {
            continue;
        }
        charge_inventory_work(&mut work, limits)?;
        let Some(source) = input.document.text().get(range.as_range()) else {
            return Err(CharacterReferenceInventoryError::DocumentMismatch {
                expected: input.document.identity().clone(),
                actual: input.document.identity().clone(),
            });
        };
        let Ok(expr) = parse_expr(source) else {
            continue;
        };
        let fact = match expr {
            Expr::EntityRef(entity)
                if judgment.ty.is_entity_ref_kind(&EntityKind::Character)
                    || judgment
                        .expected_type()
                        .is_some_and(|ty| ty.is_entity_ref_kind(&EntityKind::Character)) =>
            {
                owner_fact(world, &input, range, &entity.canonical_body(), source)?
            }
            Expr::ShortVariant(name) => local_member_fact(world, &input, range, name.as_str())?,
            _ => None,
        };
        if let Some(fact) = fact {
            let observed = u64::try_from(facts.len())
                .ok()
                .and_then(|count| count.checked_add(1))
                .ok_or(CharacterReferenceInventoryError::ArithmeticOverflow {
                    counter: CharacterDefinitionLimitKind::Candidates,
                })?;
            if observed > limits.candidates() {
                return Err(CharacterReferenceInventoryError::Limit {
                    kind: CharacterDefinitionLimitKind::Candidates,
                    observed,
                    maximum: limits.candidates(),
                });
            }
            facts.push(fact);
        }
    }
    facts.sort();
    facts.dedup();
    Ok(CharacterReferenceInventory {
        world: world.symbols().world().clone(),
        symbol_revision: *world.symbols().revision(),
        document: input.document.identity().clone(),
        module: input.module.clone(),
        syntax_snapshot: input.syntax_snapshot.cloned(),
        facts,
    })
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn owner_fact(
    world: &RegisteredSemanticWorld,
    input: &CharacterReferenceInput<'_>,
    range: arcweft_lang_syntax::ast::common::TextRange,
    canonical_body: &str,
    authored: &str,
) -> Result<Option<CharacterReferenceFact>, CharacterReferenceInventoryError> {
    let Some(body) = authored.strip_prefix('@') else {
        return Ok(None);
    };
    if body.starts_with('<') || body != canonical_body {
        return Ok(None);
    }
    let Ok(spanned) = SpannedProjectSymbolPath::parse_at(body, range.start() + 1) else {
        return Ok(None);
    };
    let Ok(path) = SymbolPath::try_from(spanned.path()) else {
        return Ok(None);
    };
    let Some(selection_range) = spanned.segment_ranges().last().copied() else {
        return Ok(None);
    };
    let reference_span = input
        .document
        .span(SourceRange::new(range.start(), range.end()))
        .expect("a type judgment range belongs to its parsed source");
    let selection_span = input
        .document
        .span(SourceRange::new(
            selection_range.start(),
            selection_range.end(),
        ))
        .expect("a parsed path segment range belongs to its source");
    let form = CharacterReferenceForm::OwnerPath { path: path.clone() };
    let resolution = if intersects_recovery(range, input.parse_diagnostics) {
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::RecoveredToken {
            source: reference_span.clone(),
        })
    } else {
        match world.environment().resolve_character_owner(
            world.symbols(),
            input.module,
            &path,
            &reference_span,
        ) {
            Ok(character) => {
                CharacterReferenceResolution::Resolved(CharacterSymbolDescriptor::Owner {
                    character,
                })
            }
            Err(error) => map_owner_issue(path, error)?,
        }
    };
    Ok(Some(CharacterReferenceFact {
        reference_span,
        selection_span,
        form,
        resolution,
    }))
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn map_owner_issue(
    reference: SymbolPath,
    error: RegisteredCharacterResolutionError,
) -> Result<CharacterReferenceResolution, CharacterReferenceInventoryError> {
    Ok(CharacterReferenceResolution::Unresolved(match error {
        RegisteredCharacterResolutionError::Symbol(
            ProjectSymbolResolutionError::Unknown { .. }
            | ProjectSymbolResolutionError::InvalidPath { .. },
        ) => CharacterDefinitionIssue::UnknownOwner { reference },
        RegisteredCharacterResolutionError::Symbol(ProjectSymbolResolutionError::Ambiguous {
            mut candidates,
            ..
        }) => {
            candidates.sort();
            candidates.dedup();
            CharacterDefinitionIssue::AmbiguousAlias {
                reference,
                candidates,
            }
        }
        RegisteredCharacterResolutionError::Symbol(ProjectSymbolResolutionError::NotCallable {
            actual,
            ..
        })
        | RegisteredCharacterResolutionError::NotExternal { actual } => {
            CharacterDefinitionIssue::WrongOwnerKind { reference, actual }
        }
        RegisteredCharacterResolutionError::Owner(ExternalOwnerLookupError::WrongKind {
            declaration,
            ..
        }) => CharacterDefinitionIssue::WrongOwnerKind {
            reference,
            actual: ProjectSymbolTargetId::External(declaration),
        },
        RegisteredCharacterResolutionError::Owner(ExternalOwnerLookupError::Unknown { .. }) => {
            CharacterDefinitionIssue::UnknownOwner { reference }
        }
        RegisteredCharacterResolutionError::Owner(ExternalOwnerLookupError::Stale {
            expected_world,
            actual_world,
            expected_revision,
            actual_revision,
        }) => {
            if expected_world != actual_world {
                return Err(CharacterReferenceInventoryError::StaleWorld {
                    expected: expected_world,
                    actual: actual_world,
                });
            }
            return Err(CharacterReferenceInventoryError::StaleSymbolRevision {
                expected: expected_revision,
                actual: actual_revision,
            });
        }
    }))
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn local_member_fact(
    world: &RegisteredSemanticWorld,
    input: &CharacterReferenceInput<'_>,
    range: arcweft_lang_syntax::ast::common::TextRange,
    spelling: &str,
) -> Result<Option<CharacterReferenceFact>, CharacterReferenceInventoryError> {
    let Some(selection_start) = range.start().checked_add(1) else {
        return Err(CharacterReferenceInventoryError::ArithmeticOverflow {
            counter: CharacterDefinitionLimitKind::QueryWork,
        });
    };
    if selection_start > range.end() {
        return Ok(None);
    }
    let reference_span = input
        .document
        .span(SourceRange::new(range.start(), range.end()))
        .expect("a type judgment range belongs to its parsed source");
    let selection_span = input
        .document
        .span(SourceRange::new(selection_start, range.end()))
        .expect("a local-member identifier belongs to its parsed source");
    let expected = expected_nominal(input.type_report, selection_span.range());
    let form_expected = match &expected {
        ExpectedNominal::Missing | ExpectedNominal::Ambiguous(_) => None,
        ExpectedNominal::Unique(expected) => Some(expected.clone()),
    };
    let form = CharacterReferenceForm::LocalMember {
        spelling: spelling.to_owned(),
        expected: form_expected,
    };
    let resolution = if intersects_recovery(range, input.parse_diagnostics) {
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::RecoveredToken {
            source: reference_span.clone(),
        })
    } else {
        match expected {
            ExpectedNominal::Missing => resolve_local_member(world, spelling, None),
            ExpectedNominal::Unique(expected) => {
                resolve_local_member(world, spelling, Some(expected))
            }
            ExpectedNominal::Ambiguous(candidates) => CharacterReferenceResolution::Unresolved(
                CharacterDefinitionIssue::AmbiguousSemanticContext {
                    spelling: spelling.to_owned(),
                    candidates,
                },
            ),
        }
    };
    Ok(Some(CharacterReferenceFact {
        reference_span,
        selection_span,
        form,
        resolution,
    }))
}

enum ExpectedNominal {
    Missing,
    Unique(CharacterNominalType),
    Ambiguous(Vec<CharacterNominalType>),
}

fn expected_nominal(report: &TypeCheckReport, selection: SourceRange) -> ExpectedNominal {
    let mut candidates = report
        .judgments
        .iter()
        .filter_map(|judgment| {
            let range = judgment.source_range?;
            (range.start() <= selection.start() && selection.end() <= range.end()).then(|| {
                judgment
                    .expected_type()
                    .and_then(|ty| ty.character_nominal())
                    .or_else(|| judgment.ty.character_nominal())
                    .map(|nominal| (range.end() - range.start(), nominal.clone()))
            })?
        })
        .collect::<Vec<_>>();
    candidates.sort();
    let Some(width) = candidates.first().map(|candidate| candidate.0) else {
        return ExpectedNominal::Missing;
    };
    let mut closest = candidates
        .into_iter()
        .take_while(|(candidate_width, _)| *candidate_width == width)
        .map(|(_, nominal)| nominal)
        .collect::<Vec<_>>();
    closest.sort();
    closest.dedup();
    match closest.as_slice() {
        [expected] => ExpectedNominal::Unique(expected.clone()),
        [] => ExpectedNominal::Missing,
        _ => ExpectedNominal::Ambiguous(closest),
    }
}

fn resolve_local_member(
    world: &RegisteredSemanticWorld,
    spelling: &str,
    expected: Option<CharacterNominalType>,
) -> CharacterReferenceResolution {
    let index = world.character_definition_index();
    let all_candidates = || {
        let mut candidates = Vec::new();
        if let Ok(id) = CharacterLookId::try_new(spelling) {
            candidates.extend_from_slice(index.look_candidates(&id));
        }
        if let Ok(id) = CharacterPartId::try_new(spelling) {
            candidates.extend_from_slice(index.part_candidates(&id));
        }
        if let Ok(id) = CharacterVariantId::try_new(spelling) {
            candidates.extend_from_slice(index.variant_candidates(&id));
        }
        candidates.sort();
        candidates.dedup();
        candidates
    };

    let Some(expected) = expected else {
        let candidates = all_candidates();
        return match candidates.as_slice() {
            [descriptor] => CharacterReferenceResolution::Resolved(descriptor.clone()),
            [] => {
                CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::UnknownMember {
                    spelling: spelling.to_owned(),
                    expected: None,
                })
            }
            _ => CharacterReferenceResolution::Unresolved(
                CharacterDefinitionIssue::AmbiguousMember {
                    spelling: spelling.to_owned(),
                    candidates,
                },
            ),
        };
    };

    let descriptor =
        match &expected {
            CharacterNominalType::Look { character } => CharacterLookId::try_new(spelling)
                .ok()
                .map(|look| CharacterSymbolDescriptor::Look {
                    character: character.clone(),
                    look,
                }),
            CharacterNominalType::Part { character } => CharacterPartId::try_new(spelling)
                .ok()
                .map(|part| CharacterSymbolDescriptor::Part {
                    character: character.clone(),
                    part,
                }),
            CharacterNominalType::Variant { character, part } => {
                CharacterVariantId::try_new(spelling).ok().map(|variant| {
                    CharacterSymbolDescriptor::Variant {
                        character: character.clone(),
                        part: part.clone(),
                        variant,
                    }
                })
            }
        };
    if let Some(descriptor) = descriptor
        && index.declaration(&descriptor).is_some()
    {
        return CharacterReferenceResolution::Resolved(descriptor);
    }

    let candidates = all_candidates();
    if matches!(&expected, CharacterNominalType::Variant { character, part }
        if candidates.iter().any(|candidate| matches!(candidate,
            CharacterSymbolDescriptor::Variant { character: actual_character, part: actual_part, .. }
                if actual_character == character && actual_part != part)))
    {
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::WrongOwningPart {
            spelling: spelling.to_owned(),
            expected,
            candidates,
        })
    } else if !candidates.is_empty() {
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::WrongNominalFamily {
            spelling: spelling.to_owned(),
            expected,
            candidates,
        })
    } else {
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::UnknownMember {
            spelling: spelling.to_owned(),
            expected: Some(expected),
        })
    }
}

fn intersects_recovery(
    reference: arcweft_lang_syntax::ast::common::TextRange,
    diagnostics: &[ParseError],
) -> bool {
    diagnostics.iter().any(|diagnostic| {
        let recovery = diagnostic.range();
        reference.start() < recovery.end() && recovery.start() < reference.end()
    })
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn ensure_world_integrity(
    world: &RegisteredSemanticWorld,
) -> Result<(), CharacterReferenceInventoryError> {
    let expected_world = world.symbols().world();
    for actual in [
        world.environment().world(),
        world.character_definition_index().world(),
    ] {
        if actual != expected_world {
            return Err(CharacterReferenceInventoryError::StaleWorld {
                expected: expected_world.clone(),
                actual: actual.clone(),
            });
        }
    }
    let expected_revision = world.symbols().revision();
    for actual in [
        world.environment().symbol_revision(),
        world.character_definition_index().symbol_revision(),
    ] {
        if actual != expected_revision {
            return Err(CharacterReferenceInventoryError::StaleSymbolRevision {
                expected: *expected_revision,
                actual: *actual,
            });
        }
    }
    Ok(())
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn charge_inventory_work(
    work: &mut u64,
    limits: CharacterDefinitionLimits,
) -> Result<(), CharacterReferenceInventoryError> {
    let observed =
        work.checked_add(1)
            .ok_or(CharacterReferenceInventoryError::ArithmeticOverflow {
                counter: CharacterDefinitionLimitKind::QueryWork,
            })?;
    if observed > limits.query_work() {
        return Err(CharacterReferenceInventoryError::Limit {
            kind: CharacterDefinitionLimitKind::QueryWork,
            observed,
            maximum: limits.query_work(),
        });
    }
    *work = observed;
    Ok(())
}

/// Result of a typed character definition query at one byte cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDefinitionQueryResult {
    Resolved(CharacterDefinition),
    NotApplicable(CharacterDefinitionNotApplicable),
    Unresolved(CharacterDefinitionIssue),
    Stale(CharacterDefinitionStale),
    Exhausted(CharacterDefinitionResourceError),
    Integrity(CharacterDefinitionIntegrityError),
}

/// Owned core definition result before protocol URI/range adaptation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDefinition {
    descriptor: CharacterSymbolDescriptor,
    origin_selection: SourceSpan,
    declarations: Vec<CharacterDeclarationSource>,
}

impl CharacterDefinition {
    pub const fn descriptor(&self) -> &CharacterSymbolDescriptor {
        &self.descriptor
    }

    pub const fn origin_selection(&self) -> &SourceSpan {
        &self.origin_selection
    }

    pub fn declarations(&self) -> impl ExactSizeIterator<Item = &CharacterDeclarationSource> {
        self.declarations.iter()
    }
}

/// Cursor classification outside a complete character identifier selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDefinitionNotApplicable {
    Whitespace,
    Delimiter,
    Qualification,
    NonCharacterToken,
    EndBoundary,
    RecoveredSyntax,
}

/// Exact identity mismatch preventing reuse of a request-scoped inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDefinitionStale {
    World {
        expected: ProjectSymbolWorldId,
        actual: ProjectSymbolWorldId,
    },
    SymbolRevision {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    Document {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    SyntaxSnapshot {
        expected: SourceSnapshotId,
        actual: SourceSnapshotId,
    },
}

/// Bounded resource failure while constructing an owned query result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDefinitionResourceError {
    Limit {
        kind: CharacterDefinitionLimitKind,
        observed: u64,
        maximum: u64,
    },
    ArithmeticOverflow {
        counter: CharacterDefinitionLimitKind,
    },
}

/// Impossible accepted-state inconsistency found by a core query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDefinitionIntegrityError {
    MissingDeclaration {
        descriptor: CharacterSymbolDescriptor,
    },
    MissingOwnedDocument {
        source: SourceDocumentIdentity,
    },
    AmbiguousCursorFacts {
        source: SourceSpan,
        candidates: Vec<CharacterSymbolDescriptor>,
    },
    AcceptedWorldInvariant {
        world: ProjectSymbolWorldId,
        revision: ProjectSymbolRevision,
    },
}

/// Resolves one byte cursor through an exact current reference inventory.
pub fn query_character_definition(
    world: &RegisteredSemanticWorld,
    inventory: &CharacterReferenceInventory,
    document: &SourceDocumentIdentity,
    cursor: usize,
) -> CharacterDefinitionQueryResult {
    if let Some(outcome) = query_context_outcome(world, inventory, document, cursor) {
        return outcome;
    }

    let mut selected = inventory
        .facts()
        .filter(|fact| {
            fact.selection_span().range().start() <= cursor
                && cursor < fact.selection_span().range().end()
        })
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| {
        span_width(left.selection_span())
            .cmp(&span_width(right.selection_span()))
            .then_with(|| {
                span_width(left.reference_span()).cmp(&span_width(right.reference_span()))
            })
            .then_with(|| left.cmp(right))
    });
    if selected.len() > 1 {
        let mut candidates = selected
            .iter()
            .filter_map(|fact| match fact.resolution() {
                CharacterReferenceResolution::Resolved(descriptor) => Some(descriptor.clone()),
                CharacterReferenceResolution::Unresolved(_) => None,
            })
            .collect::<Vec<_>>();
        candidates.sort();
        candidates.dedup();
        return CharacterDefinitionQueryResult::Integrity(
            CharacterDefinitionIntegrityError::AmbiguousCursorFacts {
                source: selected[0].selection_span().clone(),
                candidates,
            },
        );
    }
    let Some(fact) = selected.first().copied() else {
        return classify_unselected_cursor(inventory, cursor);
    };
    let descriptor = match fact.resolution() {
        CharacterReferenceResolution::Resolved(descriptor) => descriptor,
        CharacterReferenceResolution::Unresolved(issue) => {
            return CharacterDefinitionQueryResult::Unresolved(issue.clone());
        }
    };
    let Some(set) = world.character_definition_index().declaration(descriptor) else {
        return CharacterDefinitionQueryResult::Integrity(
            CharacterDefinitionIntegrityError::MissingDeclaration {
                descriptor: descriptor.clone(),
            },
        );
    };
    let declarations = set.sources().cloned().collect::<Vec<_>>();
    let observed = u64::try_from(declarations.len()).unwrap_or(u64::MAX);
    let maximum = CharacterDefinitionLimits::PRODUCTION.declaration_sources_per_descriptor();
    if observed > maximum {
        return CharacterDefinitionQueryResult::Exhausted(
            CharacterDefinitionResourceError::Limit {
                kind: CharacterDefinitionLimitKind::DeclarationSourcesPerDescriptor,
                observed,
                maximum,
            },
        );
    }
    if let Some(missing) = declarations.iter().find(|declaration| {
        world
            .character_definition_index()
            .document(declaration.selection_span().source())
            .is_none()
    }) {
        return CharacterDefinitionQueryResult::Integrity(
            CharacterDefinitionIntegrityError::MissingOwnedDocument {
                source: missing.selection_span().source().clone(),
            },
        );
    }
    CharacterDefinitionQueryResult::Resolved(CharacterDefinition {
        descriptor: descriptor.clone(),
        origin_selection: fact.selection_span().clone(),
        declarations,
    })
}

fn query_context_outcome(
    world: &RegisteredSemanticWorld,
    inventory: &CharacterReferenceInventory,
    document: &SourceDocumentIdentity,
    cursor: usize,
) -> Option<CharacterDefinitionQueryResult> {
    if ensure_world_integrity(world).is_err() {
        return Some(CharacterDefinitionQueryResult::Integrity(
            CharacterDefinitionIntegrityError::AcceptedWorldInvariant {
                world: world.symbols().world().clone(),
                revision: *world.symbols().revision(),
            },
        ));
    }
    if inventory.world() != world.symbols().world() {
        return Some(CharacterDefinitionQueryResult::Stale(
            CharacterDefinitionStale::World {
                expected: world.symbols().world().clone(),
                actual: inventory.world().clone(),
            },
        ));
    }
    if inventory.symbol_revision() != world.symbols().revision() {
        return Some(CharacterDefinitionQueryResult::Stale(
            CharacterDefinitionStale::SymbolRevision {
                expected: *world.symbols().revision(),
                actual: *inventory.symbol_revision(),
            },
        ));
    }
    if inventory.document() != document {
        return Some(CharacterDefinitionQueryResult::Stale(
            CharacterDefinitionStale::Document {
                expected: inventory.document().clone(),
                actual: document.clone(),
            },
        ));
    }
    let Ok(source_len) = usize::try_from(document.source_len()) else {
        return Some(CharacterDefinitionQueryResult::Exhausted(
            CharacterDefinitionResourceError::ArithmeticOverflow {
                counter: CharacterDefinitionLimitKind::QueryWork,
            },
        ));
    };
    if cursor >= source_len {
        return Some(CharacterDefinitionQueryResult::NotApplicable(
            CharacterDefinitionNotApplicable::EndBoundary,
        ));
    }
    None
}

fn classify_unselected_cursor(
    inventory: &CharacterReferenceInventory,
    cursor: usize,
) -> CharacterDefinitionQueryResult {
    for fact in inventory.facts() {
        let selection = fact.selection_span().range();
        if cursor == selection.end() {
            return CharacterDefinitionQueryResult::NotApplicable(
                CharacterDefinitionNotApplicable::EndBoundary,
            );
        }
        let reference = fact.reference_span().range();
        if reference.start() <= cursor && cursor < reference.end() {
            return CharacterDefinitionQueryResult::NotApplicable(match fact.form() {
                CharacterReferenceForm::OwnerPath { .. } => {
                    CharacterDefinitionNotApplicable::Qualification
                }
                CharacterReferenceForm::LocalMember { .. } => {
                    CharacterDefinitionNotApplicable::Delimiter
                }
            });
        }
    }
    CharacterDefinitionQueryResult::NotApplicable(
        CharacterDefinitionNotApplicable::NonCharacterToken,
    )
}

fn span_width(span: &SourceSpan) -> usize {
    span.range().end().saturating_sub(span.range().start())
}
