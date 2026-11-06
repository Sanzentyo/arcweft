//! Trust declarations reached through selected source call targets.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::symbol::{CallableDeclarationKey, CallableDeclarationOwner, ProofArtifactId};
use arcweft_lang_sema::{
    callable::{CallAnalysisOutcome, CallableCandidateId, CheckedCallResult},
    final_analysis::FinalSemanticAnalysis,
};

use super::{ProofSummary, ProofTrustSummary, VerificationInputError};

const MAX_DEPENDENCY_WORK: usize = 262_144;

pub(super) fn project(
    semantics: &FinalSemanticAnalysis,
    artifacts: &[ProofArtifactId],
    summaries: &mut [ProofSummary],
) -> Result<(), VerificationInputError> {
    let trusted = artifacts
        .iter()
        .zip(summaries.iter())
        .filter(|(_, summary)| matches!(summary.trust, ProofTrustSummary::Trusted { .. }))
        .map(|(artifact, _)| CallableDeclarationKey::Existing(artifact.declaration().clone()))
        .collect::<BTreeSet<_>>();
    if trusted.is_empty() {
        return Ok(());
    }
    let proof_owners = artifacts
        .iter()
        .map(|artifact| {
            (
                CallableDeclarationKey::Existing(artifact.declaration().clone()),
                artifact,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut remaining = MAX_DEPENDENCY_WORK;
    let edges = source_calls(semantics, &proof_owners, &mut remaining)?;

    for (artifact, summary) in artifacts.iter().zip(summaries) {
        let root = CallableDeclarationKey::Existing(artifact.declaration().clone());
        let mut visited = BTreeSet::from([&root]);
        let mut pending = vec![&root];
        let mut labels = BTreeSet::new();
        while let Some(current) = pending.pop() {
            charge(&mut remaining)?;
            for callee in &edges[current] {
                charge(&mut remaining)?;
                if visited.insert(callee) {
                    if trusted.contains(callee) {
                        labels.insert(proof_owners[callee].declaration().qualified_name());
                    }
                    pending.push(callee);
                }
            }
        }
        summary.trusted_dependencies = labels.into_iter().collect();
    }
    Ok(())
}

fn source_calls(
    semantics: &FinalSemanticAnalysis,
    proof_owners: &BTreeMap<CallableDeclarationKey, &ProofArtifactId>,
    remaining: &mut usize,
) -> Result<
    BTreeMap<CallableDeclarationKey, BTreeSet<CallableDeclarationKey>>,
    VerificationInputError,
> {
    // Borrow the sealed declaration rows once. Resolving every callee via the
    // project-wide declaration scan would do uncharged work for each new node.
    let mut declarations = BTreeMap::new();
    for module in semantics.hir_topology().modules() {
        for entry in module.entries() {
            charge(remaining)?;
            if let Some(body) = entry.body()
                && declarations
                    .insert(body.declaration(), (entry, body))
                    .is_some()
            {
                return Err(VerificationInputError::DuplicateDependencyOwner {
                    owner: entry.item(),
                });
            }
        }
    }

    let mut edges = BTreeMap::new();
    let mut pending = proof_owners.keys().cloned().collect::<Vec<_>>();
    while let Some(key) = pending.pop() {
        charge(remaining)?;
        if edges.contains_key(&key) {
            continue;
        }
        let declaration = declarations.remove(&key);
        if let Some(artifact) = proof_owners.get(&key) {
            let matches_owner = declaration.is_some_and(|(entry, body)| {
                entry.item() == artifact.item() && body.snapshot() == artifact.snapshot()
            });
            if !matches_owner {
                return Err(VerificationInputError::ProofDependencyOwnerMismatch {
                    owner: artifact.item(),
                });
            }
        }

        let mut called = BTreeSet::new();
        if let Some((entry, body)) = declaration {
            let expressions = entry
                .paths()
                .expression_owners()
                .chain(body.paths().expression_owners());
            for expression in expressions {
                charge(remaining)?;
                let Some(CallAnalysisOutcome::Selected(application)) =
                    semantics.call(expression).map(|call| call.outcome())
                else {
                    continue;
                };
                if matches!(application.result(), CheckedCallResult::Continuation(_)) {
                    continue;
                }
                let CallableCandidateId::Project(callee) =
                    application.core().candidates().selected().id()
                else {
                    continue;
                };
                if callee.owner() == CallableDeclarationOwner::Proof
                    && !proof_owners.contains_key(callee)
                {
                    return Err(VerificationInputError::UnregisteredProofDependency { expression });
                }
                if called.insert(callee.clone()) && !edges.contains_key(callee) {
                    pending.push(callee.clone());
                }
            }
        }
        edges.insert(key, called);
    }
    Ok(edges)
}

fn charge(remaining: &mut usize) -> Result<(), VerificationInputError> {
    *remaining =
        remaining
            .checked_sub(1)
            .ok_or(VerificationInputError::ProofDependencyWorkLimit {
                limit: MAX_DEPENDENCY_WORK,
            })?;
    Ok(())
}
