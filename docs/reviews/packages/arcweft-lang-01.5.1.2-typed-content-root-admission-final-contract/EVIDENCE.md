# Repository evidence and decision trace

The detailed machine-readable inventory is `evidence/SOURCE_INVENTORY.csv`.

## Load-bearing findings

1. The topology's resource payload and overlay types are text-only, and its revision is a `SourceSetRevision`. This is a concrete representational gap for PNG layers and absence facts, not a reason to replace the strict manifest decoder.
2. `CharacterPackage` is already the correct Sans-I/O owner and already enforces package membership. Its missing PNG-content validation is the only package-model defect demonstrated by the request/current tests.
3. Character source maps already preserve the exact `character`, `asset`, rectangle, and nested token spans needed for diagnostics.
4. The project manifest source map already knows content-unit/root/profile token locations, so dedicated source accessors are enough; a second decoder or generic TOML value boundary would be a regression.
5. `BuildDigest` is the existing project/cache BLAKE3 identity. A nominal `ProjectTopologyRevision` over it avoids a new hash system while preventing source-only revisions from pretending to cover binary inputs.
6. Sema already retains typed reference facts and the compiler already has the one reachability/content-partition substrate. Admission uses one typed project reference inventory so absent content never survives typechecking; compiler reachability remains the sole partition/bundle walk rather than being duplicated.
7. LSP already has complete candidate construction followed by compare-and-swap publication. The correct atomicity fix is to add the content inventory/revision to that candidate, not invent a parallel state channel.
8. Source `content` remains in AST/HIR/sema. Its deletion must follow manifest-fact injection and remain ordinary current-grammar rejection.
