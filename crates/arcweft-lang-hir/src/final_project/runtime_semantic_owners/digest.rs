use sha2::{Digest, Sha256};

use super::{
    BTreeSet, CallableDeclarationKey, HirModuleId, HirRuntimeEmissionMode,
    HirRuntimeExecutableOwner, HirRuntimeReachabilityDigest, HirRuntimeReachabilityEdge,
    HirRuntimeReachabilityEdgeKind, HirRuntimeReachabilityRoot, HirRuntimeReachabilityRootKind,
    HirRuntimeReachabilitySite, HirSnapshotId, LocalId, ProjectSymbolRevision,
    ProjectSymbolWorldId, StructuralOwners,
};
use crate::identity::HirTypedId;

#[expect(
    clippy::too_many_arguments,
    reason = "the digest transcript binds every accepted reachability authority exactly once"
)]
pub(super) fn reachability_digest(
    mode: HirRuntimeEmissionMode,
    module_snapshots: &[(HirModuleId, HirSnapshotId)],
    symbol_world: &ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    roots: &[HirRuntimeReachabilityRoot],
    edges: &[HirRuntimeReachabilityEdge],
    executables: &BTreeSet<HirRuntimeExecutableOwner>,
    locals: &[LocalId],
    owners: &StructuralOwners,
) -> HirRuntimeReachabilityDigest {
    let mut hasher = Sha256::new();
    hasher.update(b"arcweft.runtime-reachability\0");
    hasher.update([1, mode_tag(mode)]);
    digest_len(&mut hasher, module_snapshots.len());
    for (_, snapshot) in module_snapshots {
        hasher.update(snapshot.cache_fingerprint_input());
    }
    digest_string(&mut hasher, symbol_world.package().as_str());
    digest_string(&mut hasher, symbol_world.root_document().as_str());
    digest_string(&mut hasher, symbol_world.profile());
    hasher.update(symbol_revision.as_source_set().as_bytes());
    digest_len(&mut hasher, roots.len());
    for root in roots {
        hasher.update([root_kind_tag(root.kind)]);
        digest_executable(&mut hasher, &root.owner);
    }
    digest_len(&mut hasher, edges.len());
    for edge in edges {
        digest_site(&mut hasher, edge.source);
        digest_edge_kind(&mut hasher, &edge.kind);
        digest_executable(&mut hasher, &edge.target);
    }
    digest_len(&mut hasher, executables.len());
    for executable in executables {
        digest_executable(&mut hasher, executable);
    }
    digest_typed_ids(&mut hasher, locals.iter().copied());
    digest_typed_ids(&mut hasher, owners.expressions.iter().copied());
    digest_typed_ids(&mut hasher, owners.statements.iter().copied());
    digest_typed_ids(&mut hasher, owners.types.iter().copied());
    digest_typed_ids(&mut hasher, owners.patterns.iter().copied());
    digest_typed_ids(&mut hasher, owners.captures.iter().copied());
    HirRuntimeReachabilityDigest(hasher.finalize().into())
}

fn digest_typed_ids<T: HirTypedId>(hasher: &mut Sha256, ids: impl Iterator<Item = T>) {
    let ids = ids.collect::<Vec<_>>();
    digest_len(hasher, ids.len());
    for id in ids {
        hasher.update(id.raw().cache_fingerprint_input());
    }
}

fn digest_executable(hasher: &mut Sha256, owner: &HirRuntimeExecutableOwner) {
    match owner {
        HirRuntimeExecutableOwner::Item(owner) => {
            hasher.update([0]);
            hasher.update(owner.raw().cache_fingerprint_input());
        }
        HirRuntimeExecutableOwner::ImplMethod(owner) => {
            hasher.update([1]);
            let declaration = CallableDeclarationKey::ImplMethod(owner.clone());
            hasher.update(declaration.semantic_digest().as_bytes());
        }
        HirRuntimeExecutableOwner::Closure(owner) => {
            hasher.update([2]);
            hasher.update(owner.raw().cache_fingerprint_input());
        }
    }
}

fn digest_site(hasher: &mut Sha256, site: HirRuntimeReachabilitySite) {
    match site {
        HirRuntimeReachabilitySite::Item(owner) => {
            hasher.update([0]);
            hasher.update(owner.raw().cache_fingerprint_input());
        }
        HirRuntimeReachabilitySite::Expression(owner) => {
            hasher.update([1]);
            hasher.update(owner.raw().cache_fingerprint_input());
        }
        HirRuntimeReachabilitySite::Statement(owner) => {
            hasher.update([2]);
            hasher.update(owner.raw().cache_fingerprint_input());
        }
    }
}

fn digest_edge_kind(hasher: &mut Sha256, kind: &HirRuntimeReachabilityEdgeKind) {
    match kind {
        HirRuntimeReachabilityEdgeKind::CheckedProjectCall { call, declaration } => {
            hasher.update([0]);
            hasher.update(call.raw().cache_fingerprint_input());
            hasher.update(declaration.semantic_digest().as_bytes());
        }
        HirRuntimeReachabilityEdgeKind::CheckedTraitMethodCall {
            call,
            implementation,
            method,
        } => {
            hasher.update([1]);
            hasher.update(call.raw().cache_fingerprint_input());
            hasher.update(implementation.raw().cache_fingerprint_input());
            let declaration = CallableDeclarationKey::ImplMethod(method.clone());
            hasher.update(declaration.semantic_digest().as_bytes());
        }
        HirRuntimeReachabilityEdgeKind::CheckedIteratorWitnessMethod {
            role,
            implementation,
            member,
            method,
        } => {
            hasher.update([2, role.digest_tag()]);
            hasher.update(implementation.raw().cache_fingerprint_input());
            hasher.update(member.to_le_bytes());
            let declaration = CallableDeclarationKey::ImplMethod(method.clone());
            hasher.update(declaration.semantic_digest().as_bytes());
        }
        HirRuntimeReachabilityEdgeKind::CheckedClosureExecution { closure } => {
            hasher.update([5]);
            hasher.update(closure.raw().cache_fingerprint_input());
        }
        HirRuntimeReachabilityEdgeKind::CheckedFlowTransfer {
            source,
            declaration,
        } => {
            hasher.update([3]);
            digest_site(hasher, *source);
            hasher.update(declaration.semantic_digest().as_bytes());
        }
        HirRuntimeReachabilityEdgeKind::CheckedEntryBinding { entry, declaration } => {
            hasher.update([4]);
            hasher.update(entry.raw().cache_fingerprint_input());
            hasher.update(declaration.semantic_digest().as_bytes());
        }
    }
}

const fn mode_tag(mode: HirRuntimeEmissionMode) -> u8 {
    match mode {
        HirRuntimeEmissionMode::CheckAll => 0,
        HirRuntimeEmissionMode::SelectedEntry => 1,
    }
}

const fn root_kind_tag(kind: HirRuntimeReachabilityRootKind) -> u8 {
    match kind {
        HirRuntimeReachabilityRootKind::CheckedFlow => 0,
        HirRuntimeReachabilityRootKind::CheckedEntry => 1,
        HirRuntimeReachabilityRootKind::SelectedEntry => 2,
        HirRuntimeReachabilityRootKind::CheckedViewValueProgram => 3,
    }
}

fn digest_len(hasher: &mut Sha256, length: usize) {
    hasher.update(u32::try_from(length).unwrap_or(u32::MAX).to_le_bytes());
}

fn digest_string(hasher: &mut Sha256, value: &str) {
    digest_len(hasher, value.len());
    hasher.update(value.as_bytes());
}
