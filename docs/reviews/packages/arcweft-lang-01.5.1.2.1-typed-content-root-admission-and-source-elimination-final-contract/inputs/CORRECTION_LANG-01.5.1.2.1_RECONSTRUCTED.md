# Lang-01.5.1.2.1 — content-root family / Source-elimination reconciliation

## Sequence position

This is Lang-01.5.1.2.1. It corrects the returned Lang-01.5.1.2 typed
content-root admission contract after Lang-01.3.1 selected complete removal of
the author-facing `source` declaration and runtime `Source<T, E>` path. It must
be resolved before the closed content-root family, final manifest-to-project
admission, source `content` deletion, bundle, watch, or LSP migration is
implemented.

The returned Lang-01.5.1.2 package remains authoritative for its concrete
binary topology, overlay, `CharacterPackage`, presence, transaction, and
revision decisions except where it treats `Source` as a retained root family.
This correction must not redesign already implemented and verified substrate
without a concrete defect.

## Why this correction is required

The returned family table includes `SourceContentRootFamily::Source`, treats
an authored `source` entity as a valid source-owned content root, and relies on
`EntityKind::Source`. That directly conflicts with the selected Lang-01.3.1
final model:

- external asynchronous sequences are ordinary extern-capability functions
  returning `Stream<T, E>`;
- authored generators are ordinary functions returning `Stream<T, E>` with
  own-scope `yield`;
- the `source` declaration, `Source<T, E>`, and `EntityKind::Source` are
  removed without aliases or compatibility readers.

The safe binary/topology parts can proceed independently, but the final closed
family and admission graph cannot encode a soon-to-be-deleted owner.

## Required decisions

1. Define the final closed `ContentRootFamily` inventory after Source
   elimination, with exact typed identity and crate ownership for every
   retained family.
2. Remove `Source` from accepted families. Define how a reference that names
   a removed/wrong family fails through the ordinary current typed resolver;
   do not add a spelling-specific migration diagnostic.
3. Decide explicitly whether an ordinary/external Stream callable can ever be
   a content root.
   - Prefer no: a live value producer is not packaged content merely because
     it returns `Stream<T, E>`.
   - If a concrete use case requires one, define a new semantic category from
     existing callable identity and manifest policy, not a replacement Source
     entity or function-name heuristic.
4. Define the exact post-Source `AcceptedContentRootTarget`, accepted reference
   inventory, source spans, visibility, demand/presence, and topology revision
   fields.
5. Define manifest-to-`ProjectIndex` admission and atomic failure order after
   source `content` declaration removal. No directory scan or source reparse
   may reconstruct roots.
6. Reconcile generated metadata, typed `res` references, Character packages,
   file-backed roots, optional absence facts, bundle inclusion, watch inputs,
   and LSP publication against the corrected family.
7. Amend the Lang-01.5.1.2 deletion inventory and tests so no Source entity,
   Source root, alias, dual family reader, or provisional family tag remains.

## Required implementation order

1. Land/reuse the Lang-01.5.1.2 binary topology, binary overlay,
   `CharacterPackage`, source accessor, and transaction substrate that does not
   depend on the root-family enum.
2. Complete Lang-01.3.1 Source elimination and consume its final typed symbol
   inventory.
3. Freeze the corrected closed root-family and accepted-target types.
4. Implement project-wide reference collection and atomic admission.
5. Delete source `content` syntax/HIR/sema/tooling ownership in the same
   migration that publishes manifest-owned content facts.
6. Migrate bundle, watch, LSP, fixtures, and diagnostics to the accepted typed
   inventory.
7. Run focused, workspace, Tier 2, and structural validation.

## Tests to specify

- every retained family accepted in each permitted manifest position and
  rejected in every wrong-family position with exact typed evidence;
- removed Source-family references produce no accepted target, project fact,
  bundle entry, watch input, LSP symbol, or compatibility node;
- ordinary Stream passthrough, authored Stream generator, and external Stream
  callable do not become content roots solely from return type or execution
  mode;
- required/optional present, absent-unreferenced, absent-referenced, and
  present-invalid roots under the corrected family table;
- complete Character package with exact PNG bytes, plus missing, duplicate,
  corrupt, unreferenced, and identity-mismatched layers;
- disk and overlay parity for manifest text and binary resources;
- topology revision changes for every source, metadata, manifest, binary,
  package, overlay, presence, and accepted-root mutation;
- project collision, ambiguity, visibility, wrong revision, and stale overlay
  failures publish no partial topology/index/catalog/cache/LSP state;
- source `content` removal is proven through parser/compiler rejection and
  absence of an executable typed node, not a repository source scan;
- bundle/watch/LSP consume the same accepted inventory without rescanning.

## Constraints

- Preserve the strict single manifest decoder, `SourceBackedManifest`, binary
  bytes outside `SourceDocument`, typed topology revision, generated metadata,
  project containment, and Character nominal identity unless a concrete flaw
  is demonstrated.
- Do not restore `source`, `Source<T, E>`, `EntityKind::Source`, source
  `content`, old path arrays, directory inference, last-known-good candidate
  acceptance, compatibility aliases/readers, source gates, CSS, or Takumi.
- Keep core and data-format crates Sans I/O and preserve existing crate
  layering.

## Expected output

Return an independently usable final-contract package with `OPEN_QUESTIONS=0`
containing the corrected closed family and exact Rust ownership, admission and
diagnostic order, ProjectIndex/bundle/watch/LSP projections, deletion order,
compatibility statement, and a complete positive/negative/transaction test
matrix. Include a normative delta identifying every Lang-01.5.1.2 row changed
by Source elimination.
