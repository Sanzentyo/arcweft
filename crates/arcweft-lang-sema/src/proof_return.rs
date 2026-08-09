//! Semantic Proof return classification over the sole nominal resolver result.

use std::{collections::BTreeMap, sync::Arc};

use arcweft_lang_hir::identity::{ItemId, TypeId};
use arcweft_lang_hir::proof_return::{
    HirProofReturnAuthorityError, HirProofReturnCallableHeaderRef, HirProofReturnHeader,
    HirProofReturnHeaderProjectView, HirProofReturnProjectGeneration, HirProofReturnSemanticClass,
    HirProofReturnSemanticFact, HirProofReturnSemanticFactSet,
};
use arcweft_lang_hir::symbol::{ProjectSymbolRevision, ProjectSymbolWorldId};
use thiserror::Error;

use crate::nominal::{
    GenericTypeBinding, GenericTypeScope, NominalResolutionLimits, ResolvedTypeRefOutcome,
    SelfTypeScope, TypeResolutionInput, TypeResolutionInputError, TypeResolutionReport,
    TypeResolutionWorld, TypeSourceEvidence, resolve_type_ref,
};
use crate::registration::AcceptedNominalWorld;
use crate::types::{GenericTypeOwnerId, GenericTypeParameterId, TypeKind};
use arcweft_lang_hir::item::HirGenericParameter;
use arcweft_lang_hir::symbol::ProjectSymbolTable;
use arcweft_lang_syntax::ast::module_path::ModuleSegment;

/// Mismatch between a staged Proof header and its semantic evidence.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProofReturnClassificationError {
    #[error("detached nominal resolution cannot publish a project Proof return fact")]
    DetachedWorld,
    #[error("Proof return header symbol world is stale or foreign")]
    WrongSymbolWorld {
        expected: Box<ProjectSymbolWorldId>,
        actual: Box<ProjectSymbolWorldId>,
    },
    #[error("Proof return header symbol revision is stale")]
    WrongSymbolRevision {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    #[error("nominal report root {actual:?} does not match Proof return type {expected:?}")]
    WrongReturnType { expected: TypeId, actual: TypeId },
    #[error(transparent)]
    GenerationLease(#[from] HirProofReturnAuthorityError),
    #[error("Proof return nominal resolution input is invalid: {reason:?}")]
    Resolution {
        reason: Box<TypeResolutionInputError>,
    },
}

/// Sema fact and the exact nominal report whose diagnostics/work produced it.
#[derive(Clone, Debug)]
pub struct ProofReturnClassification {
    fact: HirProofReturnSemanticFact,
    report: TypeResolutionReport,
}

/// Complete project-wide semantic result produced while all authored Proof
/// bodies are still paused and unpublished.
pub struct ProofReturnProjectClassification {
    facts: Arc<HirProofReturnSemanticFactSet>,
    reports: BTreeMap<ItemId, TypeResolutionReport>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProofReturnProjectClassificationError {
    #[error("staged Proof return header {item:?} has no frozen callable header")]
    MissingCallableHeader { item: ItemId },
    #[error("staged Proof callable header {item:?} has no generation header")]
    UnexpectedCallableHeader { item: ItemId },
    #[error("staged Proof callable header {item:?} has no exact callable symbol")]
    MissingCallableSymbol { item: ItemId },
    #[error("staged Proof callable header {item:?} contains an invalid generic scope")]
    InvalidGenericScope { item: ItemId },
    #[error(transparent)]
    Classification(#[from] ProofReturnClassificationError),
    #[error(transparent)]
    Authority(#[from] HirProofReturnAuthorityError),
}

impl ProofReturnClassification {
    pub const fn fact(&self) -> &HirProofReturnSemanticFact {
        &self.fact
    }

    pub const fn report(&self) -> &TypeResolutionReport {
        &self.report
    }

    pub fn into_parts(self) -> (HirProofReturnSemanticFact, TypeResolutionReport) {
        (self.fact, self.report)
    }
}

impl ProofReturnProjectClassification {
    pub const fn facts(&self) -> &Arc<HirProofReturnSemanticFactSet> {
        &self.facts
    }

    pub fn reports(&self) -> impl ExactSizeIterator<Item = (ItemId, &TypeResolutionReport)> {
        self.reports.iter().map(|(item, report)| (*item, report))
    }

    pub fn into_facts(self) -> Arc<HirProofReturnSemanticFactSet> {
        self.facts
    }
}

/// Classifies every authored Proof return against the same immutable staged
/// project, symbol table, and accepted nominal world. Fact-set construction is
/// complete before the caller can resume or publish any body.
pub fn classify_proof_return_project<'a>(
    generation: Arc<HirProofReturnProjectGeneration>,
    headers: &[HirProofReturnHeader],
    project: HirProofReturnHeaderProjectView<'a, 'a>,
    symbols: &'a ProjectSymbolTable,
    environment: &'a AcceptedNominalWorld,
) -> Result<ProofReturnProjectClassification, ProofReturnProjectClassificationError> {
    let callable_headers = project
        .authored_proof_returns()
        .map(|header| (header.item(), header))
        .collect::<BTreeMap<_, _>>();
    for item in callable_headers.keys().copied() {
        if !headers.iter().any(|header| header.item() == item) {
            return Err(ProofReturnProjectClassificationError::UnexpectedCallableHeader { item });
        }
    }

    let mut facts = Vec::with_capacity(headers.len());
    let mut reports = BTreeMap::new();
    for header in headers {
        let item = header.item();
        let callable = callable_headers
            .get(&item)
            .copied()
            .ok_or(ProofReturnProjectClassificationError::MissingCallableHeader { item })?;
        let generics = proof_generic_scope(callable, symbols)?;
        let input = TypeResolutionInput::accepted_proof_return_header(
            header.return_type(),
            callable.module(),
            project,
            symbols,
            environment,
            &generics,
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .map_err(|reason| ProofReturnClassificationError::Resolution {
            reason: Box::new(reason),
        })?;
        let (fact, report) = classify_proof_return(header.clone(), &input)?.into_parts();
        facts.push(fact);
        reports.insert(item, report);
    }
    let facts = HirProofReturnSemanticFactSet::try_new(generation, headers.iter().cloned(), facts)?;
    Ok(ProofReturnProjectClassification { facts, reports })
}

fn proof_generic_scope(
    header: HirProofReturnCallableHeaderRef<'_, '_>,
    symbols: &ProjectSymbolTable,
) -> Result<GenericTypeScope, ProofReturnProjectClassificationError> {
    let item = header.item();
    let owner = symbols
        .callable_symbols()
        .find(|symbol| symbol.source_item() == item)
        .map(|symbol| GenericTypeOwnerId::Callable(symbol.declaration().clone()))
        .ok_or(ProofReturnProjectClassificationError::MissingCallableSymbol { item })?;
    let mut ordinal = 0_u16;
    let mut bindings = Vec::new();
    for parameter in header.generic_parameters() {
        let HirGenericParameter::Type { name, .. } = parameter else {
            continue;
        };
        let name = name
            .resolved()
            .ok_or(ProofReturnProjectClassificationError::InvalidGenericScope { item })?;
        let name = ModuleSegment::new(name.as_str())
            .map_err(|_| ProofReturnProjectClassificationError::InvalidGenericScope { item })?;
        let id = GenericTypeParameterId::new(owner.clone(), ordinal);
        ordinal = ordinal
            .checked_add(1)
            .ok_or(ProofReturnProjectClassificationError::InvalidGenericScope { item })?;
        bindings.push(GenericTypeBinding::new(
            id,
            name,
            TypeSourceEvidence::accepted(
                header.declaration_source().range(),
                header.declaration_source().clone(),
            ),
        ));
    }
    GenericTypeScope::try_new(bindings)
        .map_err(|_| ProofReturnProjectClassificationError::InvalidGenericScope { item })
}

/// Resolves and classifies one exact staged Proof return through the accepted
/// project input. Alias spelling and source structure are never inspected.
pub fn classify_proof_return(
    header: HirProofReturnHeader,
    input: &TypeResolutionInput<'_>,
) -> Result<ProofReturnClassification, ProofReturnClassificationError> {
    let TypeResolutionWorld::Accepted { symbols, .. } = input.world() else {
        return Err(ProofReturnClassificationError::DetachedWorld);
    };
    let expected_world = header.generation().world();
    if expected_world != symbols.world() {
        return Err(ProofReturnClassificationError::WrongSymbolWorld {
            expected: Box::new(expected_world.clone()),
            actual: Box::new(symbols.world().clone()),
        });
    }
    let expected_revision = header.generation().revision();
    if expected_revision != *symbols.revision() {
        return Err(ProofReturnClassificationError::WrongSymbolRevision {
            expected: expected_revision,
            actual: *symbols.revision(),
        });
    }
    let module = input.module();
    header.generation().validate_module_transaction(
        module.key().package(),
        module.key().path(),
        module.snapshot_id(),
        module.syntax_snapshot(),
        module.source_identity(),
    )?;
    let actual = input.root();
    if actual != header.return_type() {
        return Err(ProofReturnClassificationError::WrongReturnType {
            expected: header.return_type(),
            actual,
        });
    }

    let report =
        resolve_type_ref(input).map_err(|reason| ProofReturnClassificationError::Resolution {
            reason: Box::new(reason),
        })?;
    debug_assert_eq!(report.outcome().product().root(), input.root());
    let class = match report.outcome() {
        ResolvedTypeRefOutcome::Complete(product) => match product.recovered() {
            TypeKind::Unit => HirProofReturnSemanticClass::Unit,
            TypeKind::Tuple(elements) if elements.is_empty() => HirProofReturnSemanticClass::Unit,
            _ => HirProofReturnSemanticClass::NonUnit,
        },
        ResolvedTypeRefOutcome::Poisoned(_) | ResolvedTypeRefOutcome::Detached(_) => {
            HirProofReturnSemanticClass::Poisoned
        }
    };
    let fact = HirProofReturnSemanticFact::new(header, class, report.work_charged());
    Ok(ProofReturnClassification { fact, report })
}

#[cfg(test)]
mod tests;
