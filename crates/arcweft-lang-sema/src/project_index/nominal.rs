//! Typed project-nominal declarations and reference edges retained by tooling.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    identity::TypeId,
    leaf::HirPath,
    symbol::{
        ProjectSymbolTable,
        nominal::{ProjectNominalDeclaration, ProjectNominalDeclarationId},
    },
};
use arcweft_source::SourceSpan;

use crate::{
    final_analysis::FinalSemanticAnalysis,
    nominal::{AliasExpansionFact, ResolvedTypeRefOutcome, TypeNameResolution, TypePoisonRecord},
    types::TypeKind,
};

use super::ProjectSemanticIndexError;

type CheckedProjectNominalInventory = (
    BTreeMap<ProjectNominalDeclarationId, ProjectNominalIndexRecord>,
    Box<[ProjectNominalReferenceEdge]>,
);

/// One accepted project nominal declaration and its checked poison evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalIndexRecord {
    declaration: ProjectNominalDeclaration,
    poisons: Box<[TypePoisonRecord]>,
}

/// One exact final-HIR type head resolved to a project nominal declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalReferenceEdge {
    declaration: ProjectNominalDeclarationId,
    root: TypeId,
    source: SourceSpan,
    terminal_source: SourceSpan,
    use_path: HirPath,
    node: TypeId,
    arguments: Box<[TypeKind]>,
    normalized: TypeKind,
    alias_expansions: Box<[AliasExpansionFact]>,
    poisons: Box<[TypePoisonRecord]>,
}

impl ProjectNominalIndexRecord {
    pub fn new(
        declaration: ProjectNominalDeclaration,
        poisons: impl Into<Box<[TypePoisonRecord]>>,
    ) -> Self {
        Self {
            declaration,
            poisons: poisons.into(),
        }
    }

    /// Canonical declaration identity shared with HIR and the resolver.
    pub const fn id(&self) -> &ProjectNominalDeclarationId {
        self.declaration.id()
    }

    /// Complete immutable declaration record published by the project symbol table.
    pub const fn declaration(&self) -> &ProjectNominalDeclaration {
        &self.declaration
    }

    /// Resolver poison records whose final-HIR roots belong to this declaration.
    pub fn poisons(&self) -> &[TypePoisonRecord] {
        &self.poisons
    }
}

impl ProjectNominalReferenceEdge {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        declaration: ProjectNominalDeclarationId,
        root: TypeId,
        source: SourceSpan,
        terminal_source: SourceSpan,
        use_path: HirPath,
        node: TypeId,
        arguments: impl Into<Box<[TypeKind]>>,
        normalized: TypeKind,
        alias_expansions: impl Into<Box<[AliasExpansionFact]>>,
        poisons: impl Into<Box<[TypePoisonRecord]>>,
    ) -> Self {
        Self {
            declaration,
            root,
            source,
            terminal_source,
            use_path,
            node,
            arguments: arguments.into(),
            normalized,
            alias_expansions: alias_expansions.into(),
            poisons: poisons.into(),
        }
    }

    /// Declaration selected for this exact final-HIR type head.
    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    /// Exact final-HIR type root whose report owns this edge.
    pub const fn root(&self) -> TypeId {
        self.root
    }

    /// Exact path or constructor head selected by nominal resolution.
    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    /// Exact final path segment to edit when the selected declaration is renamed.
    pub const fn terminal_source(&self) -> &SourceSpan {
        &self.terminal_source
    }

    /// Root-preserving semantic path resolved by this edge.
    pub const fn use_path(&self) -> &HirPath {
        &self.use_path
    }

    /// Structural node address within `root`.
    pub const fn node(&self) -> TypeId {
        self.node
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

    /// Poison evidence attached to the owning final-HIR root.
    pub fn poisons(&self) -> &[TypePoisonRecord] {
        &self.poisons
    }
}

/// Projects tooling inventory from the exact nominal products retained by the
/// final semantic generation. This never re-runs nominal resolution and never
/// reconstructs a type path from source text.
pub(super) fn checked_project_nominals(
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<CheckedProjectNominalInventory, ProjectSemanticIndexError> {
    let records = symbols
        .nominal_symbols()
        .map(|declaration| {
            let mut poisons = analysis
                .type_resolutions()
                .filter_map(|(_, report)| {
                    let product = report.outcome().product();
                    let root = product
                        .nodes()
                        .iter()
                        .find(|node| node.node() == product.root())?;
                    root.source()
                        .project()
                        .is_some_and(|source| span_contains(declaration.source().whole(), source))
                        .then_some(report)
                })
                .flat_map(|report| report.poisons().iter().cloned())
                .collect::<Vec<_>>();
            poisons.sort_by_key(TypePoisonRecord::id);
            poisons.dedup_by_key(|poison| poison.id());
            (
                declaration.id().clone(),
                ProjectNominalIndexRecord::new(declaration.clone(), poisons),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut edges = Vec::new();
    for (_, report) in analysis.type_resolutions() {
        let product = report.outcome().product();
        for node in product.nodes() {
            let (declaration, arguments, alias_expansions) = match node.outcome() {
                TypeNameResolution::Project(project) => (
                    project.declaration().clone(),
                    project.arguments().to_vec().into_boxed_slice(),
                    Vec::<AliasExpansionFact>::new().into_boxed_slice(),
                ),
                TypeNameResolution::Alias(alias) => (
                    alias.declaration().clone(),
                    alias.arguments().to_vec().into_boxed_slice(),
                    alias_expansion_suffix(
                        report.outcome(),
                        alias.declaration(),
                        alias.use_source().project(),
                    ),
                ),
                _ => continue,
            };
            let source = node.source().project().cloned().ok_or_else(|| {
                missing_reference_evidence(product.root(), node.node(), "project source")
            })?;
            let terminal_source = node
                .terminal_source()
                .and_then(|source| source.project())
                .cloned()
                .ok_or_else(|| {
                    missing_reference_evidence(
                        product.root(),
                        node.node(),
                        "terminal project source",
                    )
                })?;
            let use_path = node.reference_path().cloned().ok_or_else(|| {
                missing_reference_evidence(product.root(), node.node(), "typed use path")
            })?;
            let normalized = node.recovered().cloned().ok_or_else(|| {
                missing_reference_evidence(product.root(), node.node(), "recovered type")
            })?;
            edges.push(ProjectNominalReferenceEdge::new(
                declaration,
                product.root(),
                source,
                terminal_source,
                use_path,
                node.node(),
                arguments,
                normalized,
                alias_expansions,
                report.poisons().to_vec(),
            ));
        }
    }
    edges.sort_by(|left, right| {
        left.source()
            .cmp(right.source())
            .then_with(|| left.declaration().cmp(right.declaration()))
            .then_with(|| left.node().cmp(&right.node()))
    });
    edges.dedup();
    Ok((records, edges.into_boxed_slice()))
}

fn missing_reference_evidence(
    root: TypeId,
    node: TypeId,
    reason: &'static str,
) -> ProjectSemanticIndexError {
    ProjectSemanticIndexError::MissingNominalReferenceEvidence { root, node, reason }
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
            || Vec::<AliasExpansionFact>::new().into_boxed_slice(),
            |start| aliases[start..].to_vec().into_boxed_slice(),
        )
}

fn span_contains(owner: &SourceSpan, candidate: &SourceSpan) -> bool {
    owner.source() == candidate.source()
        && owner.range().start() <= candidate.range().start()
        && candidate.range().end() <= owner.range().end()
}
