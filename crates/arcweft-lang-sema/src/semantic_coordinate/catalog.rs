use std::{collections::BTreeMap, sync::Arc};

use arcweft_lang_hir::{
    identity::{ExprId, ItemId, LocalId, PatternId, StmtId},
    project::{
        HirProjectEvaluationTopology, HirSemanticBodyLocation, HirSemanticBodyLocator,
        HirSemanticBodyLookupError, HirSemanticOwnerPath, HirSemanticPathLocation,
        HirSemanticPathLookupError, HirSemanticPathOwnerId, HirSemanticPathRoot,
        HirSemanticPathStep,
    },
    symbol::CallableDeclarationKey,
};
use thiserror::Error;

use crate::final_analysis::{CheckedItem, CheckedItemRole};

use super::{
    AcceptedDeclarationSemanticId, AcceptedItemSemanticId, AcceptedSemanticRoot,
    CheckedBindingCoordinateEvidence, CheckedBodyCoordinateEvidence, CheckedExpressionChildRole,
    CheckedExpressionCoordinateEvidence, CheckedPatternCoordinateEvidence, CheckedSemanticPath,
    CheckedSemanticPathStep, CheckedStatementCoordinateEvidence, HirItemEvaluationEntryRole,
    HirPatternChildRole, HirStatementChildRole, StableCheckedBindingCoordinate,
    StableCheckedBodyCoordinate, StableCheckedPatternOwnerCoordinate,
    StableCheckedStatementCoordinate,
};

#[derive(Debug)]
pub(crate) struct AcceptedSemanticRootCatalog {
    topology: Arc<HirProjectEvaluationTopology>,
    roots: BTreeMap<HirSemanticPathRoot, AcceptedSemanticRoot>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AcceptedSemanticRootCatalogError {
    #[error("accepted root catalog and checked callable catalog have different HIR generations")]
    CatalogGenerationMismatch,
    #[error("accepted root catalog path root is absent from the sealed topology: {root:?}")]
    MissingRoot { root: HirSemanticPathRoot },
    #[error("accepted root catalog has no checked item fact for {item:?}")]
    MissingCheckedItem { item: ItemId },
    #[error("accepted root catalog item {item:?} family disagrees with its checked role")]
    ItemFamilyMismatch { item: ItemId },
    #[error("accepted root catalog contains a duplicate HIR root {root:?}")]
    DuplicateRoot { root: HirSemanticPathRoot },
    #[error("accepted root catalog callable lookup failed for {declaration:?}: {error:?}")]
    CallableLookup {
        declaration: CallableDeclarationKey,
        error: crate::callable::CheckedCallableLookupError,
    },
    #[error("accepted root catalog contains an unrecoverable digest collision for {root:?}")]
    DigestCollision { root: HirSemanticPathRoot },
    #[error("accepted root catalog length does not fit u32")]
    LengthOverflow,
    #[error("accepted root catalog HIR path lookup failed: {0}")]
    HirLookup(#[from] HirSemanticPathLookupError),
    #[error("accepted root catalog HIR body lookup failed: {0}")]
    HirBodyLookup(#[from] HirSemanticBodyLookupError),
}

impl AcceptedSemanticRootCatalog {
    /// Seals all declaration and item roots from one already-built topology.
    /// Checked item facts are borrowed only for the seal; the catalog retains
    /// no second item authority.
    pub(crate) fn seal(
        topology: Arc<HirProjectEvaluationTopology>,
        callables: &crate::callable::CheckedCallableCatalog,
        item_facts: &BTreeMap<ItemId, &CheckedItem>,
    ) -> Result<Self, AcceptedSemanticRootCatalogError> {
        let callable_generation = callables
            .hir_generation()
            .ok_or(AcceptedSemanticRootCatalogError::CatalogGenerationMismatch)?;
        if !Arc::ptr_eq(topology.generation(), callable_generation) {
            return Err(AcceptedSemanticRootCatalogError::CatalogGenerationMismatch);
        }
        let mut roots = BTreeMap::new();
        let mut accepted_to_hir = BTreeMap::new();
        for module in topology.modules() {
            for entry in module.entries() {
                let checked = item_facts.get(&entry.item()).ok_or(
                    AcceptedSemanticRootCatalogError::MissingCheckedItem { item: entry.item() },
                )?;
                if checked.role().family() != entry.family().family() {
                    return Err(AcceptedSemanticRootCatalogError::ItemFamilyMismatch {
                        item: entry.item(),
                    });
                }
                let accepted = AcceptedSemanticRoot::Item(accepted_item_id(
                    topology.package(),
                    module.generation().canonical_path(),
                    entry.entry_ordinal(),
                    entry.role(),
                    checked.role(),
                )?);
                insert_root(
                    &mut roots,
                    &mut accepted_to_hir,
                    entry.paths().root().clone(),
                    accepted,
                )?;
                if let Some(body) = entry.body() {
                    let declaration = body.declaration().clone();
                    let facts = callables.project_callable(&declaration).map_err(|error| {
                        AcceptedSemanticRootCatalogError::CallableLookup {
                            declaration: declaration.clone(),
                            error,
                        }
                    })?;
                    let accepted = AcceptedSemanticRoot::Declaration(accepted_declaration_id(
                        &declaration,
                        facts,
                    ));
                    insert_root(
                        &mut roots,
                        &mut accepted_to_hir,
                        body.paths().root().clone(),
                        accepted,
                    )?;
                }
            }
        }
        Ok(Self { topology, roots })
    }

    pub(crate) const fn topology(&self) -> &Arc<HirProjectEvaluationTopology> {
        &self.topology
    }

    pub(crate) fn root_for_hir(
        &self,
        root: &HirSemanticPathRoot,
    ) -> Result<&AcceptedSemanticRoot, AcceptedSemanticRootCatalogError> {
        self.roots
            .get(root)
            .ok_or_else(|| AcceptedSemanticRootCatalogError::MissingRoot { root: root.clone() })
    }

    pub(crate) fn semantic_path(
        &self,
        owner: HirSemanticPathOwnerId,
    ) -> Result<Option<HirSemanticPathLocation<'_>>, AcceptedSemanticRootCatalogError> {
        self.topology.semantic_path(owner).map_err(Into::into)
    }

    pub(crate) fn semantic_body(
        &self,
        locator: &HirSemanticBodyLocator,
    ) -> Result<Option<HirSemanticBodyLocation<'_>>, AcceptedSemanticRootCatalogError> {
        self.topology.semantic_body(locator).map_err(Into::into)
    }
}

fn insert_root(
    roots: &mut BTreeMap<HirSemanticPathRoot, AcceptedSemanticRoot>,
    accepted_to_hir: &mut BTreeMap<AcceptedSemanticRoot, HirSemanticPathRoot>,
    hir_root: HirSemanticPathRoot,
    accepted: AcceptedSemanticRoot,
) -> Result<(), AcceptedSemanticRootCatalogError> {
    if roots.contains_key(&hir_root) {
        return Err(AcceptedSemanticRootCatalogError::DuplicateRoot { root: hir_root });
    }
    if accepted_to_hir.contains_key(&accepted) {
        return Err(AcceptedSemanticRootCatalogError::DigestCollision { root: hir_root });
    }
    accepted_to_hir.insert(accepted, hir_root.clone());
    roots.insert(hir_root, accepted);
    Ok(())
}

fn accepted_declaration_id(
    declaration: &CallableDeclarationKey,
    facts: &crate::callable::CheckedCallableFacts,
) -> AcceptedDeclarationSemanticId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.lang.accepted-declaration-semantic.v1\0");
    hasher.update(declaration.semantic_digest().as_bytes());
    hasher.update(facts.id().semantic_digest().as_bytes());
    hasher.update(facts.interface_digest().as_bytes());
    AcceptedDeclarationSemanticId::from_bytes(*hasher.finalize().as_bytes())
}

fn accepted_item_id(
    package: &arcweft_lang_hir::symbol::CallablePackageId,
    module: &arcweft_lang_syntax::ast::module_path::CanonicalModulePath,
    entry_ordinal: u32,
    role: HirItemEvaluationEntryRole,
    checked_role: &CheckedItemRole,
) -> Result<AcceptedItemSemanticId, AcceptedSemanticRootCatalogError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.lang.accepted-item-semantic.v1\0");
    write_u32_bytes(&mut hasher, package.as_str().as_bytes())?;
    write_u32(&mut hasher, module.segments().len())?;
    for segment in module.segments() {
        write_u32_bytes(&mut hasher, segment.as_str().as_bytes())?;
    }
    hasher.update(&entry_ordinal.to_le_bytes());
    let family = checked_role.accepted_item_family_tag();
    match role {
        HirItemEvaluationEntryRole::Item => {
            hasher.update(&[0x00, family]);
        }
        HirItemEvaluationEntryRole::InlineMember { member } => {
            hasher.update(&[0x01]);
            hasher.update(&member.to_le_bytes());
            hasher.update(&[family]);
        }
    }
    Ok(AcceptedItemSemanticId::from_bytes(
        *hasher.finalize().as_bytes(),
    ))
}

fn write_u32(
    hasher: &mut blake3::Hasher,
    value: usize,
) -> Result<(), AcceptedSemanticRootCatalogError> {
    let value =
        u32::try_from(value).map_err(|_| AcceptedSemanticRootCatalogError::LengthOverflow)?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}

fn write_u32_bytes(
    hasher: &mut blake3::Hasher,
    value: &[u8],
) -> Result<(), AcceptedSemanticRootCatalogError> {
    write_u32(hasher, value.len())?;
    hasher.update(value);
    Ok(())
}

pub(crate) trait CheckedExpressionEdgeAuthority {
    fn checked_expression_child_role(
        &self,
        parent: ExprId,
        child: ExprId,
    ) -> Option<CheckedExpressionChildRole>;
}

/// Failure while issuing a stable checked semantic coordinate.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum SemanticCoordinateIndexError {
    #[error(transparent)]
    RootCatalog(#[from] AcceptedSemanticRootCatalogError),
    #[error("semantic coordinate owner is absent from the HIR path index: {owner:?}")]
    MissingOwner { owner: HirSemanticPathOwnerId },
    #[error("semantic body coordinate is absent from the HIR body index: {locator:?}")]
    MissingBody { locator: HirSemanticBodyLocator },
    #[error("semantic coordinate local path does not end in a binding edge: {owner:?}")]
    InvalidBindingPath { owner: LocalId },
    #[error("semantic coordinate expression edge evidence is missing")]
    MissingChildEdges,
    #[error("semantic coordinate HIR expression hop role disagrees with the path")]
    ExpressionRoleMismatch,
    #[error("semantic coordinate root/path invariant is invalid")]
    InvalidRootPath,
}

/// Private project-wide issuer for stable coordinates derived from one sealed
/// accepted-root catalog.
pub(crate) struct SemanticCoordinateIndex<'catalog, 'edges> {
    catalog: &'catalog AcceptedSemanticRootCatalog,
    edges: &'edges dyn CheckedExpressionEdgeAuthority,
}

impl<'catalog, 'edges> SemanticCoordinateIndex<'catalog, 'edges> {
    pub(crate) fn new(
        catalog: &'catalog AcceptedSemanticRootCatalog,
        edges: &'edges dyn CheckedExpressionEdgeAuthority,
    ) -> Self {
        Self { catalog, edges }
    }

    pub(crate) fn expression(
        &self,
        owner: ExprId,
    ) -> Result<CheckedSemanticPath, SemanticCoordinateIndexError> {
        self.coordinate(owner.into())
    }

    pub(crate) fn expression_evidence(
        &self,
        owner: ExprId,
    ) -> Result<CheckedExpressionCoordinateEvidence, SemanticCoordinateIndexError> {
        self.expression(owner)
            .map(|coordinate| CheckedExpressionCoordinateEvidence::new(owner, coordinate))
    }

    #[allow(
        dead_code,
        reason = "the semantic transcript graph consumes this typed owner coordinate before raw pattern IDs are removed"
    )]
    pub(crate) fn pattern(
        &self,
        owner: PatternId,
    ) -> Result<StableCheckedPatternOwnerCoordinate, SemanticCoordinateIndexError> {
        self.coordinate(owner.into())
            .map(StableCheckedPatternOwnerCoordinate::new)
    }

    #[allow(
        dead_code,
        reason = "checked pattern transcript publication consumes this affine evidence"
    )]
    pub(crate) fn pattern_evidence(
        &self,
        owner: PatternId,
    ) -> Result<CheckedPatternCoordinateEvidence, SemanticCoordinateIndexError> {
        self.pattern(owner)
            .map(|coordinate| CheckedPatternCoordinateEvidence::new(owner, coordinate))
    }

    #[allow(
        dead_code,
        reason = "the semantic transcript graph consumes this typed coordinate when statement digests publish"
    )]
    pub(crate) fn statement(
        &self,
        owner: StmtId,
    ) -> Result<StableCheckedStatementCoordinate, SemanticCoordinateIndexError> {
        self.coordinate(owner.into())
            .map(StableCheckedStatementCoordinate::new)
    }

    #[allow(
        dead_code,
        reason = "checked statement transcript publication consumes this affine evidence"
    )]
    pub(crate) fn statement_evidence(
        &self,
        owner: StmtId,
    ) -> Result<CheckedStatementCoordinateEvidence, SemanticCoordinateIndexError> {
        self.statement(owner)
            .map(|coordinate| CheckedStatementCoordinateEvidence::new(owner, coordinate))
    }

    #[allow(
        dead_code,
        reason = "the semantic transcript graph consumes this typed coordinate when body digests publish"
    )]
    pub(crate) fn body(
        &self,
        locator: &HirSemanticBodyLocator,
    ) -> Result<StableCheckedBodyCoordinate, SemanticCoordinateIndexError> {
        let Some(location) = self.catalog.semantic_body(locator)? else {
            return Err(SemanticCoordinateIndexError::MissingBody {
                locator: locator.clone(),
            });
        };
        if location.root() != locator.root() || location.row().owner() != locator.owner() {
            return Err(SemanticCoordinateIndexError::InvalidRootPath);
        }
        let accepted = *self.catalog.root_for_hir(location.root())?;
        let path = checked_path_from_owner_path(
            accepted,
            location.root(),
            location.row().path(),
            self.edges,
        )?;
        Ok(StableCheckedBodyCoordinate::new(
            location.row().owner(),
            location.row().kind(),
            path,
        ))
    }

    #[allow(
        dead_code,
        reason = "checked body transcript publication consumes this affine evidence"
    )]
    pub(crate) fn body_evidence(
        &self,
        locator: HirSemanticBodyLocator,
    ) -> Result<CheckedBodyCoordinateEvidence, SemanticCoordinateIndexError> {
        let coordinate = self.body(&locator)?;
        Ok(CheckedBodyCoordinateEvidence::new(locator, coordinate))
    }

    pub(crate) fn binding(
        &self,
        local: LocalId,
    ) -> Result<StableCheckedBindingCoordinate, SemanticCoordinateIndexError> {
        let owner = HirSemanticPathOwnerId::Local(local);
        let Some(location) = self.catalog.semantic_path(owner)? else {
            return Err(SemanticCoordinateIndexError::MissingOwner { owner });
        };
        if !is_binding_local_path(location.path()) {
            return Err(SemanticCoordinateIndexError::InvalidBindingPath { owner: local });
        }
        let accepted = *self.catalog.root_for_hir(location.root())?;
        let checked_path =
            checked_path_from_owner_path(accepted, location.root(), location.path(), self.edges)?;
        Ok(StableCheckedBindingCoordinate::new(checked_path))
    }

    pub(crate) fn binding_evidence(
        &self,
        local: LocalId,
    ) -> Result<CheckedBindingCoordinateEvidence, SemanticCoordinateIndexError> {
        self.binding(local)
            .map(|coordinate| CheckedBindingCoordinateEvidence::new(local, coordinate))
    }

    fn coordinate(
        &self,
        owner: HirSemanticPathOwnerId,
    ) -> Result<CheckedSemanticPath, SemanticCoordinateIndexError> {
        let Some(location) = self.catalog.semantic_path(owner)? else {
            return Err(SemanticCoordinateIndexError::MissingOwner { owner });
        };
        if location.owner() != owner || location.snapshot().module() != owner.module() {
            return Err(SemanticCoordinateIndexError::InvalidRootPath);
        }
        let accepted = *self.catalog.root_for_hir(location.root())?;
        checked_path_from_owner_path(accepted, location.root(), location.path(), self.edges)
    }
}

fn is_binding_local_path(path: &HirSemanticOwnerPath) -> bool {
    let path = path.steps();
    match path.last() {
        Some(
            HirSemanticPathStep::Statement(HirStatementChildRole::SelectBinding { .. })
            | HirSemanticPathStep::DeclarationMember { .. }
            | HirSemanticPathStep::DeclarationResult,
        ) => true,
        Some(HirSemanticPathStep::Pattern(role)) => matches!(
            role,
            HirPatternChildRole::BindingLocal
                | HirPatternChildRole::MutableBindingLocal
                | HirPatternChildRole::RecordShorthandLocal { .. }
                | HirPatternChildRole::RecordRestLocal { .. }
                | HirPatternChildRole::SequenceRestLocal
                | HirPatternChildRole::WholeBindingLocal
                | HirPatternChildRole::TypedBindingLocal
        ),
        Some(
            HirSemanticPathStep::DeclarationBody(_)
            | HirSemanticPathStep::DeclarationContract(_)
            | HirSemanticPathStep::DeclarationItem(_)
            | HirSemanticPathStep::ExpressionOwned(_)
            | HirSemanticPathStep::Body(_)
            | HirSemanticPathStep::Statement(_)
            | HirSemanticPathStep::StatementBody(_)
            | HirSemanticPathStep::Expression(_)
            | HirSemanticPathStep::MatchPattern { .. }
            | HirSemanticPathStep::ParameterPattern { .. }
            | HirSemanticPathStep::ParameterDefault { .. },
        )
        | None => false,
    }
}

fn checked_path_from_owner_path(
    accepted: AcceptedSemanticRoot,
    hir_root: &HirSemanticPathRoot,
    owner_path: &HirSemanticOwnerPath,
    edges: &dyn CheckedExpressionEdgeAuthority,
) -> Result<CheckedSemanticPath, SemanticCoordinateIndexError> {
    let path = owner_path.steps();
    let valid_root = match hir_root {
        HirSemanticPathRoot::Declaration(_) => {
            matches!(
                path.first(),
                Some(
                    HirSemanticPathStep::DeclarationBody(_)
                        | HirSemanticPathStep::DeclarationContract(_)
                        | HirSemanticPathStep::ParameterPattern { .. }
                        | HirSemanticPathStep::ParameterDefault { .. }
                        | HirSemanticPathStep::DeclarationResult
                )
            ) && !path.iter().any(|step| {
                matches!(
                    step,
                    HirSemanticPathStep::DeclarationItem(_)
                        | HirSemanticPathStep::DeclarationMember { .. }
                )
            }) && !path.iter().skip(1).any(|step| {
                matches!(
                    step,
                    HirSemanticPathStep::DeclarationBody(_)
                        | HirSemanticPathStep::DeclarationContract(_)
                        | HirSemanticPathStep::ParameterPattern { .. }
                        | HirSemanticPathStep::ParameterDefault { .. }
                        | HirSemanticPathStep::DeclarationResult
                )
            })
        }
        HirSemanticPathRoot::Item { role, .. } => {
            let first = path.first();
            let first_valid = matches!(first, Some(HirSemanticPathStep::DeclarationItem(_)))
                || matches!(
                    (role, first),
                    (
                        HirItemEvaluationEntryRole::Item,
                        Some(HirSemanticPathStep::DeclarationMember { .. })
                    )
                );
            first_valid
                && !path.iter().enumerate().any(|(index, step)| {
                    matches!(
                        step,
                        HirSemanticPathStep::DeclarationBody(_)
                            | HirSemanticPathStep::DeclarationContract(_)
                            | HirSemanticPathStep::ParameterPattern { .. }
                            | HirSemanticPathStep::ParameterDefault { .. }
                            | HirSemanticPathStep::DeclarationResult
                    ) || (index > 0
                        && matches!(
                            step,
                            HirSemanticPathStep::DeclarationItem(_)
                                | HirSemanticPathStep::DeclarationMember { .. }
                        ))
                })
                && (role != &HirItemEvaluationEntryRole::Item
                    || !matches!(first, Some(HirSemanticPathStep::DeclarationMember { .. }))
                    || matches!(
                        path.last(),
                        Some(HirSemanticPathStep::DeclarationMember { .. })
                    ))
        }
    };
    if !valid_root {
        return Err(SemanticCoordinateIndexError::InvalidRootPath);
    }
    let mut hops = owner_path.hops().iter();
    let mut steps = Vec::with_capacity(path.len());
    for step in path {
        steps.push(match step {
            HirSemanticPathStep::DeclarationBody(role) => {
                CheckedSemanticPathStep::DeclarationBody(*role)
            }
            HirSemanticPathStep::DeclarationContract(role) => {
                CheckedSemanticPathStep::DeclarationContract(*role)
            }
            HirSemanticPathStep::DeclarationItem(role) => {
                CheckedSemanticPathStep::DeclarationItem(role.clone())
            }
            HirSemanticPathStep::ExpressionOwned(role) => {
                CheckedSemanticPathStep::ExpressionOwned(role.clone())
            }
            HirSemanticPathStep::Body(role) => CheckedSemanticPathStep::Body(*role),
            HirSemanticPathStep::Statement(role) => CheckedSemanticPathStep::Statement(*role),
            HirSemanticPathStep::StatementBody(role) => {
                CheckedSemanticPathStep::StatementBody(*role)
            }
            HirSemanticPathStep::Expression(raw_role) => {
                let hop = hops
                    .next()
                    .ok_or(SemanticCoordinateIndexError::MissingChildEdges)?;
                if hop.role() != raw_role {
                    return Err(SemanticCoordinateIndexError::ExpressionRoleMismatch);
                }
                let role = edges
                    .checked_expression_child_role(hop.parent(), hop.child())
                    .ok_or(SemanticCoordinateIndexError::MissingChildEdges)?;
                CheckedSemanticPathStep::Expression(role)
            }
            HirSemanticPathStep::MatchPattern { arm } => {
                CheckedSemanticPathStep::MatchPattern { arm: *arm }
            }
            HirSemanticPathStep::Pattern(role) => CheckedSemanticPathStep::Pattern(*role),
            HirSemanticPathStep::ParameterPattern { group, parameter } => {
                CheckedSemanticPathStep::ParameterPattern {
                    group: *group,
                    parameter: *parameter,
                }
            }
            HirSemanticPathStep::ParameterDefault { group, parameter } => {
                CheckedSemanticPathStep::ParameterDefault {
                    group: *group,
                    parameter: *parameter,
                }
            }
            HirSemanticPathStep::DeclarationMember { member } => {
                CheckedSemanticPathStep::DeclarationMember { member: *member }
            }
            HirSemanticPathStep::DeclarationResult => CheckedSemanticPathStep::DeclarationResult,
        });
    }
    if hops.next().is_some() {
        return Err(SemanticCoordinateIndexError::MissingChildEdges);
    }
    Ok(CheckedSemanticPath::new(accepted, steps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::final_analysis::{CheckedFunctionExecution, CheckedItemRole, CheckedSuspensionRole};
    use crate::semantic_coordinate::{
        CHECKED_DECLARATION_BODY_STEP_TAG, CHECKED_DECLARATION_CONTRACT_STEP_TAG,
        CHECKED_DECLARATION_ITEM_STEP_TAG, CHECKED_DECLARATION_MEMBER_STEP_TAG,
        CHECKED_DECLARATION_RESULT_STEP_TAG, CHECKED_EXPRESSION_OWNED_STEP_TAG,
        HirDeclarationBodyRootRole, HirDeclarationContractRootRole, HirDeclarationItemRootRole,
        HirExpressionOwnedBodyRole, HirFlowContractRootFamily, HirStatementBodyRole,
        StableCheckedBodyCoordinate, StableCheckedValueCoordinate, write_len,
    };
    use arcweft_lang_hir::symbol::CallablePackageId;
    use arcweft_lang_hir::{body_edges::HirBodyKind, project::HirSemanticBodyOwner};
    use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};

    fn assert_pairwise_unique(tags: &[u8]) {
        for (index, tag) in tags.iter().enumerate() {
            assert!(!tags[..index].contains(tag), "duplicate tag {tag}");
        }
    }

    fn checked_role(family: u8) -> CheckedItemRole {
        match family {
            0 => CheckedItemRole::Module,
            1 => CheckedItemRole::Use,
            2 => CheckedItemRole::Flow {
                identity: arcweft_lang_hir::item::HirFlowIdentity::Missing,
            },
            3 => CheckedItemRole::Function {
                execution: CheckedFunctionExecution::DirectFrame,
                suspension: CheckedSuspensionRole::NonSuspending,
            },
            4 => CheckedItemRole::Predicate,
            5 => CheckedItemRole::Proof,
            6 => CheckedItemRole::Trait,
            7 => CheckedItemRole::Impl,
            8 => CheckedItemRole::Enum,
            9 => CheckedItemRole::Struct,
            10 => CheckedItemRole::TypeAlias,
            11 => CheckedItemRole::Resource,
            12 => CheckedItemRole::Character,
            13 => CheckedItemRole::View,
            14 => CheckedItemRole::Action,
            15 => CheckedItemRole::Activity,
            16 => CheckedItemRole::Signal,
            17 => CheckedItemRole::Metric,
            18 => CheckedItemRole::Layer,
            19 => CheckedItemRole::Entry,
            20 => CheckedItemRole::ExternCapability,
            21 => CheckedItemRole::Test,
            22 => CheckedItemRole::Bench,
            23 => CheckedItemRole::Style,
            _ => panic!("checked item family tag out of range"),
        }
    }

    fn test_declaration_root() -> AcceptedSemanticRoot {
        AcceptedSemanticRoot::Declaration(AcceptedDeclarationSemanticId::from_bytes([0xa5; 32]))
    }

    #[test]
    fn accepted_item_root_bytes_are_typed_and_family_complete() {
        let package = CallablePackageId::try_new("pkg").expect("package");
        let module = CanonicalModulePath::from_segments([
            ModuleSegment::new("game").expect("module segment")
        ]);
        let ids = (0_u8..24)
            .map(|family| {
                accepted_item_id(
                    &package,
                    &module,
                    7,
                    HirItemEvaluationEntryRole::Item,
                    &checked_role(family),
                )
                .expect("accepted family tag")
            })
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), 24);
        assert_eq!(
            ids[0].as_bytes(),
            &[
                0x77, 0xbc, 0x7b, 0xc7, 0x4d, 0x95, 0x03, 0xdf, 0x7e, 0x95, 0x95, 0xf8, 0x8f, 0xd5,
                0xc9, 0x01, 0x72, 0x0c, 0x7e, 0x3f, 0xec, 0x1f, 0x68, 0xb6, 0x52, 0x05, 0x63, 0x1b,
                0xeb, 0xfd, 0x58, 0xb6,
            ]
        );
        for (index, id) in ids.iter().enumerate() {
            assert!(!ids[..index].contains(id));
        }
        let inline = accepted_item_id(
            &package,
            &module,
            7,
            HirItemEvaluationEntryRole::InlineMember { member: 3 },
            &checked_role(4),
        )
        .expect("inline member role");
        assert_eq!(
            inline.as_bytes(),
            &[
                0xf2, 0x38, 0xca, 0xfb, 0x72, 0xa9, 0x6b, 0xdd, 0x38, 0x69, 0x29, 0xa2, 0x3e, 0x7f,
                0x8b, 0x77, 0x5b, 0x23, 0x39, 0x03, 0x2a, 0x8c, 0x2b, 0xc9, 0x08, 0x95, 0x84, 0x13,
                0xe2, 0x5c, 0x18, 0x31,
            ]
        );
        assert_ne!(inline, ids[4]);

        let path = CheckedSemanticPath::new(
            AcceptedSemanticRoot::Item(ids[0]),
            Box::<[CheckedSemanticPathStep]>::default(),
        );
        let bytes = path.canonical_bytes().unwrap();
        assert_eq!(bytes[0], 0x01);
        assert_eq!(&bytes[33..41], &0_u64.to_le_bytes());
    }

    #[test]
    fn accepted_item_length_encoding_fails_closed_on_u32_overflow() {
        let mut hasher = blake3::Hasher::new();
        assert_eq!(
            write_u32(&mut hasher, usize::MAX),
            Err(AcceptedSemanticRootCatalogError::LengthOverflow)
        );
    }

    fn checked_path_bytes(step: CheckedSemanticPathStep) -> Vec<u8> {
        CheckedSemanticPath::new(test_declaration_root(), [step])
            .canonical_bytes()
            .unwrap()
    }

    #[test]
    fn checked_path_step_tags_are_append_only_and_payload_sensitive() {
        assert_pairwise_unique(&[
            0,
            1,
            2,
            3,
            4,
            5,
            6,
            7,
            CHECKED_DECLARATION_BODY_STEP_TAG,
            CHECKED_EXPRESSION_OWNED_STEP_TAG,
            CHECKED_DECLARATION_CONTRACT_STEP_TAG,
            CHECKED_DECLARATION_ITEM_STEP_TAG,
            CHECKED_DECLARATION_MEMBER_STEP_TAG,
            CHECKED_DECLARATION_RESULT_STEP_TAG,
        ]);
        assert_eq!(CHECKED_DECLARATION_BODY_STEP_TAG, 8);
        assert_eq!(CHECKED_EXPRESSION_OWNED_STEP_TAG, 9);
        assert_eq!(CHECKED_DECLARATION_CONTRACT_STEP_TAG, 10);
        assert_eq!(CHECKED_DECLARATION_ITEM_STEP_TAG, 11);
        assert_eq!(CHECKED_DECLARATION_MEMBER_STEP_TAG, 12);
        assert_eq!(CHECKED_DECLARATION_RESULT_STEP_TAG, 13);

        let declaration = checked_path_bytes(CheckedSemanticPathStep::DeclarationBody(
            HirDeclarationBodyRootRole::FunctionBody,
        ));
        let owned_zero = checked_path_bytes(CheckedSemanticPathStep::ExpressionOwned(
            HirExpressionOwnedBodyRole::AwaitBranchPattern { branch: 0 },
        ));
        let owned_one = checked_path_bytes(CheckedSemanticPathStep::ExpressionOwned(
            HirExpressionOwnedBodyRole::AwaitBranchPattern { branch: 1 },
        ));
        let prior = checked_path_bytes(CheckedSemanticPathStep::StatementBody(
            HirStatementBodyRole::LetElse,
        ));
        assert_ne!(declaration, owned_zero);
        assert_ne!(declaration, prior);
        assert_ne!(owned_zero, prior);
        assert_ne!(owned_zero, owned_one);
    }

    #[test]
    fn declaration_contract_item_member_and_result_bytes_are_exact() {
        let contract = checked_path_bytes(CheckedSemanticPathStep::DeclarationContract(
            HirDeclarationContractRootRole::EffectOperand {
                clause: 2,
                family: HirFlowContractRootFamily::NoEffect,
                operand: 3,
            },
        ));
        assert_eq!(
            &contract[41..],
            &[
                CHECKED_DECLARATION_CONTRACT_STEP_TAG,
                7,
                2,
                0,
                0,
                0,
                5,
                3,
                0,
                0,
                0,
            ]
        );
        let item = checked_path_bytes(CheckedSemanticPathStep::DeclarationItem(
            HirDeclarationItemRootRole::TestBody,
        ));
        assert_eq!(&item[41..], &[CHECKED_DECLARATION_ITEM_STEP_TAG, 10]);
        let member = checked_path_bytes(CheckedSemanticPathStep::DeclarationMember { member: 9 });
        assert_eq!(
            &member[41..],
            &[CHECKED_DECLARATION_MEMBER_STEP_TAG, 9, 0, 0, 0]
        );
        let result = checked_path_bytes(CheckedSemanticPathStep::DeclarationResult);
        assert_eq!(&result[41..], &[CHECKED_DECLARATION_RESULT_STEP_TAG]);
    }

    #[test]
    fn representative_c1_bytes_remain_exact_after_the_owner_move() {
        let path = CheckedSemanticPath::new(
            test_declaration_root(),
            [CheckedSemanticPathStep::DeclarationBody(
                HirDeclarationBodyRootRole::FunctionBody,
            )],
        );
        let root = test_declaration_root();
        let mut expected = vec![root.tag()];
        expected.extend_from_slice(root.as_bytes());
        expected.extend_from_slice(&1_u64.to_le_bytes());
        expected.extend_from_slice(&[8, 0]);
        assert_eq!(path.canonical_bytes().unwrap(), expected);
    }

    #[test]
    fn canonical_lengths_are_fixed_little_endian_u64() {
        let path = CheckedSemanticPath::new(
            test_declaration_root(),
            [CheckedSemanticPathStep::DeclarationBody(
                HirDeclarationBodyRootRole::FunctionBody,
            )],
        );
        let bytes = path.canonical_bytes().unwrap();
        assert_eq!(&bytes[33..41], &1_u64.to_le_bytes());

        let mut raw = Vec::new();
        write_len(&mut raw, usize::from(0x2a_u8)).unwrap();
        assert_eq!(raw, 42_u64.to_le_bytes());
    }

    #[test]
    fn stable_value_coordinates_are_typed_paths_without_raw_or_recursive_ids() {
        let declaration = test_declaration_root();
        let steps_a = vec![CheckedSemanticPathStep::DeclarationBody(
            HirDeclarationBodyRootRole::FunctionBody,
        )]
        .into_boxed_slice();
        let steps_b = vec![CheckedSemanticPathStep::DeclarationBody(
            HirDeclarationBodyRootRole::FunctionBody,
        )]
        .into_boxed_slice();
        let expression_path = CheckedSemanticPath::new(declaration, steps_a);
        let binding_path = CheckedSemanticPath::new(declaration, steps_b);
        let expression = StableCheckedValueCoordinate::Expression(expression_path.clone());
        let binding = StableCheckedValueCoordinate::Binding(StableCheckedBindingCoordinate::new(
            binding_path.clone(),
        ));

        let expression_bytes = expression.canonical_bytes().unwrap();
        let binding_bytes = binding.canonical_bytes().unwrap();
        assert_eq!(
            expression_bytes,
            StableCheckedValueCoordinate::Expression(expression_path)
                .canonical_bytes()
                .unwrap()
        );
        assert_eq!(
            binding_bytes,
            StableCheckedValueCoordinate::Binding(StableCheckedBindingCoordinate::new(
                binding_path.clone(),
            ))
            .canonical_bytes()
            .unwrap()
        );
        assert_ne!(expression_bytes, binding_bytes);
        assert_eq!(expression_bytes[0], 0);
        assert_eq!(binding_bytes[0], 1);
        assert_eq!(
            &binding_bytes[1..],
            binding_path.canonical_bytes().unwrap().as_slice()
        );
    }

    #[test]
    fn checked_body_coordinates_distinguish_owner_family_and_body_kind() {
        let path = CheckedSemanticPath::new(
            test_declaration_root(),
            [CheckedSemanticPathStep::DeclarationBody(
                HirDeclarationBodyRootRole::PredicateBody,
            )],
        );
        let predicate =
            HirSemanticBodyOwner::declaration(HirDeclarationBodyRootRole::PredicateBody);
        let item = HirSemanticBodyOwner::item(HirDeclarationItemRootRole::TestBody);
        let expression =
            StableCheckedBodyCoordinate::new(&predicate, HirBodyKind::Expression, path.clone());
        let ordinary =
            StableCheckedBodyCoordinate::new(&predicate, HirBodyKind::Ordinary, path.clone());
        let item = StableCheckedBodyCoordinate::new(&item, HirBodyKind::Ordinary, path);

        let expression_bytes = expression.canonical_bytes().unwrap();
        assert!(expression_bytes.starts_with(&expression.path().canonical_bytes().unwrap()));
        assert_ne!(expression_bytes, ordinary.canonical_bytes().unwrap());
        assert_ne!(
            ordinary.canonical_bytes().unwrap(),
            item.canonical_bytes().unwrap()
        );
    }
}
