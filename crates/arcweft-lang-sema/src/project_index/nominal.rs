//! Typed project-nominal declarations and reference edges retained by tooling.

use std::collections::BTreeMap;

use arcweft_lang_hir::symbol::{
    ProjectSymbolTable,
    nominal::{ProjectNominalDeclaration, ProjectNominalDeclarationId},
};
use arcweft_lang_syntax::types::{AuthoredTypeRef, TypePath, TypeRefNodePath};
use arcweft_source::{SourceDocument, SourceRange, SourceSpan};

use crate::{
    checker::TypeCheckReport,
    nominal::{
        AliasExpansionFact, NominalResolutionIndex, ResolvedTypeRefOutcome, TypeNameResolution,
        TypePoisonRecord,
    },
    types::TypeKind,
};

use super::ProjectSemanticIndexError;

/// One accepted project nominal declaration and its checked poison evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalIndexRecord {
    declaration: ProjectNominalDeclaration,
    poisons: Box<[TypePoisonRecord]>,
}

/// One exact authored type head resolved to a project nominal declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalReferenceEdge {
    declaration: ProjectNominalDeclarationId,
    root: SourceSpan,
    source: SourceSpan,
    terminal_source: SourceSpan,
    use_path: TypePath,
    node: TypeRefNodePath,
    arguments: Box<[TypeKind]>,
    normalized: TypeKind,
    alias_expansions: Box<[AliasExpansionFact]>,
    poisons: Box<[TypePoisonRecord]>,
}

type CheckedProjectNominalInventory = (
    BTreeMap<ProjectNominalDeclarationId, ProjectNominalIndexRecord>,
    Box<[ProjectNominalReferenceEdge]>,
);

/// Source-bound lookup used when projecting an already checked authored type.
pub(super) struct CheckedTypeProjection<'a> {
    document: &'a SourceDocument,
    resolutions: &'a NominalResolutionIndex,
}

impl ProjectNominalIndexRecord {
    /// Canonical declaration identity shared with HIR and the resolver.
    pub const fn id(&self) -> &ProjectNominalDeclarationId {
        self.declaration.id()
    }

    /// Complete immutable declaration record published by the project symbol table.
    pub const fn declaration(&self) -> &ProjectNominalDeclaration {
        &self.declaration
    }

    /// Resolver poison records whose authored roots belong to this declaration.
    pub fn poisons(&self) -> &[TypePoisonRecord] {
        &self.poisons
    }
}

impl ProjectNominalReferenceEdge {
    /// Declaration selected for this exact authored type head.
    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    /// Exact authored type root whose report owns this edge.
    pub const fn root(&self) -> &SourceSpan {
        &self.root
    }

    /// Exact path or constructor head selected by nominal resolution.
    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    /// Exact final path segment to edit when the selected declaration is renamed.
    pub const fn terminal_source(&self) -> &SourceSpan {
        &self.terminal_source
    }

    /// Validated authored path resolved by this edge.
    pub const fn use_path(&self) -> &TypePath {
        &self.use_path
    }

    /// Structural node address within `root`.
    pub const fn node(&self) -> &TypeRefNodePath {
        &self.node
    }

    /// Checked generic arguments applied at this reference.
    pub fn arguments(&self) -> &[TypeKind] {
        &self.arguments
    }

    /// Checked semantic type after alias normalization.
    pub const fn normalized(&self) -> &TypeKind {
        &self.normalized
    }

    /// Typed alias-expansion trace beginning at this reference.
    pub fn alias_expansions(&self) -> &[AliasExpansionFact] {
        &self.alias_expansions
    }

    /// Poison evidence attached to the owning authored root.
    pub fn poisons(&self) -> &[TypePoisonRecord] {
        &self.poisons
    }
}

impl<'a> CheckedTypeProjection<'a> {
    pub(super) const fn new(document: &'a SourceDocument, report: &'a TypeCheckReport) -> Self {
        Self {
            document,
            resolutions: &report.nominal_resolutions,
        }
    }

    pub(super) fn recovered(
        &self,
        authored: &AuthoredTypeRef,
    ) -> Result<TypeKind, ProjectSemanticIndexError> {
        let range = authored.root_source().whole();
        let root = self
            .document
            .span(SourceRange::new(range.start(), range.end()))
            .map_err(|error| ProjectSemanticIndexError::MissingCheckedType {
                document: self.document.identity().clone(),
                range: (range.start(), range.end()),
                reason: error.to_string(),
            })?;
        self.resolutions
            .recovered_type(&root)
            .cloned()
            .ok_or_else(|| ProjectSemanticIndexError::MissingCheckedType {
                document: self.document.identity().clone(),
                range: (range.start(), range.end()),
                reason: "the accepted type-check report has no fact for this authored root"
                    .to_owned(),
            })
    }
}

pub(super) fn checked_project_nominals(
    symbols: &ProjectSymbolTable,
    report: &TypeCheckReport,
) -> Result<CheckedProjectNominalInventory, ProjectSemanticIndexError> {
    let resolutions = &report.nominal_resolutions;
    let records = symbols
        .nominal_symbols()
        .map(|declaration| {
            let mut poisons = resolutions
                .roots()
                .filter(|root| span_contains(declaration.source().whole(), root))
                .filter_map(|root| resolutions.report(root))
                .flat_map(|report| report.poisons().iter().cloned())
                .collect::<Vec<_>>();
            poisons.sort_by_key(TypePoisonRecord::id);
            poisons.dedup_by_key(|poison| poison.id());
            (
                declaration.id().clone(),
                ProjectNominalIndexRecord {
                    declaration: declaration.clone(),
                    poisons: poisons.into_boxed_slice(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut edges = Vec::new();
    for (key, node) in resolutions.nodes() {
        let (declaration, arguments, alias_expansions) = match node.outcome() {
            TypeNameResolution::Project(project) => (
                project.declaration().clone(),
                project.arguments().to_vec().into_boxed_slice(),
                Vec::new().into_boxed_slice(),
            ),
            TypeNameResolution::Alias(alias) => {
                let report = resolutions.report(key.root()).ok_or_else(|| {
                    missing_reference_evidence(key.root(), key.node(), "its owning report")
                })?;
                (
                    alias.declaration().clone(),
                    alias.arguments().to_vec().into_boxed_slice(),
                    alias_expansion_suffix(
                        report.outcome(),
                        alias.declaration(),
                        alias.use_source().project(),
                    ),
                )
            }
            _ => continue,
        };
        let source =
            node.source().project().cloned().ok_or_else(|| {
                missing_reference_evidence(key.root(), key.node(), "project source")
            })?;
        let terminal_source = node
            .terminal_source()
            .and_then(|source| source.project())
            .cloned()
            .ok_or_else(|| {
                missing_reference_evidence(key.root(), key.node(), "terminal project source")
            })?;
        let use_path = node
            .reference_path()
            .cloned()
            .ok_or_else(|| missing_reference_evidence(key.root(), key.node(), "typed use path"))?;
        let normalized = node
            .recovered()
            .cloned()
            .ok_or_else(|| missing_reference_evidence(key.root(), key.node(), "recovered type"))?;
        let poisons = resolutions
            .report(key.root())
            .ok_or_else(|| missing_reference_evidence(key.root(), key.node(), "its owning report"))?
            .poisons()
            .to_vec()
            .into_boxed_slice();
        edges.push(ProjectNominalReferenceEdge {
            declaration,
            root: key.root().clone(),
            source,
            terminal_source,
            use_path,
            node: key.node().clone(),
            arguments,
            normalized,
            alias_expansions,
            poisons,
        });
    }
    edges.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.declaration.cmp(&right.declaration))
            .then_with(|| left.node.cmp(&right.node))
    });
    edges.dedup();
    Ok((records, edges.into_boxed_slice()))
}

fn missing_reference_evidence(
    root: &SourceSpan,
    node: &TypeRefNodePath,
    reason: &'static str,
) -> ProjectSemanticIndexError {
    ProjectSemanticIndexError::MissingNominalReferenceEvidence {
        root: root.clone(),
        node: format!("{:?}", node.steps()),
        reason,
    }
}

fn alias_expansion_suffix(
    outcome: &ResolvedTypeRefOutcome,
    declaration: &ProjectNominalDeclarationId,
    source: Option<&SourceSpan>,
) -> Box<[AliasExpansionFact]> {
    let aliases = outcome.product().aliases();
    aliases
        .iter()
        .position(|fact| {
            fact.alias() == declaration
                && source.is_some_and(|source| fact.use_source().project() == Some(source))
        })
        .map_or_else(
            || Vec::new().into_boxed_slice(),
            |start| aliases[start..].to_vec().into_boxed_slice(),
        )
}

fn span_contains(owner: &SourceSpan, candidate: &SourceSpan) -> bool {
    owner.source() == candidate.source()
        && owner.range().start() <= candidate.range().start()
        && candidate.range().end() <= owner.range().end()
}

#[cfg(test)]
mod tests {
    use super::span_contains;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

    #[test]
    fn declaration_poison_membership_is_revision_and_range_exact() {
        let first = SourceDocument::try_new(
            SourceDocumentId::try_new("project://nominal").expect("id"),
            SourceName::Generated,
            "struct Boxed { value: Missing }",
        )
        .expect("source");
        let owner = first.span(SourceRange::new(0, 31)).expect("owner");
        let member = first.span(SourceRange::new(22, 29)).expect("member");
        assert!(span_contains(&owner, &member));

        let second = SourceDocument::try_new(
            SourceDocumentId::try_new("project://nominal").expect("id"),
            SourceName::Generated,
            "struct Boxed { value: Present }",
        )
        .expect("source");
        let stale = second.span(SourceRange::new(22, 29)).expect("stale");
        assert!(!span_contains(&owner, &stale));
    }
}
