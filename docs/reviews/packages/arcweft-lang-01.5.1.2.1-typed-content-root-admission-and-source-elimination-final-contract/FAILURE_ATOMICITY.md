# Failure atomicity

## 1. Candidate-local products

The following objects are never inserted into shared accepted state before the
final commit:

- effective overlay map;
- `SourceBackedManifest` candidate;
- source and generated-metadata documents;
- HIR/link/sema candidate;
- root-resolution and reference candidates;
- partial Character packages;
- absence records;
- candidate `ProjectTopologyRevision`;
- `AcceptedProjectContent`;
- `ProjectSemanticIndex`;
- `LoadedProfileTopology`;
- bundle input/cache namespace/watch inventory.

## 2. Commit object

`Arc<AcceptedProfileProject>` is the single commit object. It validates:

- identical package ID, package version, and profile identity;
- identical topology revision;
- one loaded package for every unique present Character target;
- no package for an absent Character target;
- no loaded package unreferenced by present root facts;
- every loaded manifest/layer appears in the canonical topology inventory;
- every accepted absence appears in both semantic presence and revision input;
- final index content source spans belong to the accepted manifest document.

## 3. Failure effects

For every failure row in `TEST_MATRIX.md`, assert:

- accepted `Arc<AcceptedProfileProject>` pointer unchanged;
- accepted generation unchanged;
- project index pointer unchanged;
- Character catalog/source index pointer unchanged;
- bundle/cache namespace absent for candidate revision;
- watch inventory unchanged;
- no LSP request can select candidate facts under the prior generation.

## 4. Stale concurrent completion

When candidates A and B start from generation G and B commits first, A's final
CAS fails with `StaleCandidate`. A's objects are dropped. It cannot replace B
even if A has a numerically or lexically “later” revision.

## 5. Byte-identical rebuild

A rebuild whose accepted carrier would be semantically and byte-identically the
same may preserve the current pointer/generation. This is a no-op success, not
a partial commit. A different effective topology revision always creates a new
accepted generation.
