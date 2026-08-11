# Limits, invalidation, and cache contract

## 1. Fixed production limits

| Limit | Exact value / owner |
|---|---:|
| line-ID UTF-8 bytes | 256, `arcweft-id::dialogue::MAX_DIALOGUE_ID_BYTES` |
| text-key UTF-8 bytes | 256, same owner |
| line candidates per module | 262,144, `HirLimit::Expressions.maximum()` |
| line candidates per project | 262,144 |
| generated candidates per exact prefix | 262,144 |
| named scopes / parent traversal | 16,384, `HirLimit::Scopes.maximum()` |
| module line diagnostics | 1,024, `HirLimit::Diagnostics.maximum()` |
| project line diagnostics | 1,024, same |
| labels per line diagnostic | 2 |
| project line-acceptance work | 786,432 units |

The project cap equals the existing per-module expression ceiling to prevent a
multi-module unbounded scan. The work cap is exactly three checked units per
maximum project candidate. Limits are compile-time production policy; no CLI,
manifest, profile, LSP, or test client can configure them.

## 2. Work accounting

Module candidate work charges one unit for each of:

- site/source validation;
- coordinate classification;
- owner/prefix/relative resolution;
- ID construction; and
- text-key validation/derivation.

It remains bounded by existing HIR expression/diagnostic transactions.

Project work charges at most three units per candidate:

1. candidate/source validation;
2. line-ID map lookup/insertion; and
3. accepted index construction.

Sorting uses bounded vectors already capped by candidate count. Every count,
byte append, ordinal, index, and work addition uses checked arithmetic.

## 3. One-over behavior

- authored ID/key over 256: AW-CD-025, no candidate;
- candidate 262,145 in one module: fatal module limit, no snapshot;
- project candidate 262,145: fatal project limit, no project;
- generated ordinal 262,145 under one prefix: AW-CD-025/fatal candidate build,
  no counter commit;
- named scope 16,385: existing HIR scope limit, no snapshot;
- diagnostic 1,025: fatal transaction limit, no claim of complete diagnostics;
- work 786,433: fatal project limit, no project.

## 4. Module invalidation

The candidate inventory is an immutable child of one `HirModule` snapshot and
is keyed by:

```text
HirModuleKey(package, module path, SourceDocumentIdentity)
HirSnapshotId(module ID, HirRevision)
```

There is no independent mutable candidate cache. A no-op parse/lower operation
returns the exact same HIR snapshot and candidate Arc. A byte-changing source
revision creates a new SourceDocumentIdentity/HIR revision and recomputes only
that module's candidates.

## 5. Project invalidation

The project-builder cache key is a private ordered tuple:

```text
root package
[(HirPackageModuleKey, HirModuleKey.source, HirSnapshotId), ...]
```

An identical tuple may reuse the exact `Arc<HirProject>`. Any changed module key
or snapshot invalidates project collision acceptance because one changed
candidate can collide with any module. Unchanged module candidate Arcs remain
reused as input.

No public `DialogueLineRevision` is introduced. Existing
`ProjectSymbolRevision`, `SourceSetRevision`, accepted generation, and exact
project Arc remain the lifecycle authority.

## 6. Accepted lifecycle

Existing accepted publication behavior is retained:

- identical source bytes with metadata/protocol version changes may publish a
  metadata-only generation reusing project/world Arcs;
- changed/incomplete/mismatched/rejected candidate publication leaves the prior
  accepted generation and caches untouched;
- request leases pin exact accepted document, module, HIR project, semantic
  world, and generation; and
- stale requests cannot redirect to a newer inventory.

The line inventory is accessed through the pinned project Arc.

## 7. Source-span freshness

Module construction validates every site/component span against its exact
SourceDocument. Project construction revalidates module/source-registry
correlation. A stale or foreign span is AW-CD-026/fatal source mismatch; it is
never rebased by offset or display URI.

## 8. Lookup freshness

`ExprId` lookup uses existing HIR stale-ID rules. An ExprId from another module,
future revision, or retired snapshot cannot locate an accepted line. Runtime,
LSP, Agent, and tooling lookups additionally require their accepted-generation
lease.

## 9. Deterministic equality

Accepted records and diagnostics implement structural Eq/Ord on typed fields.
The crate-private cache fingerprint is BLAKE3 domain-separated as
`arcweft.hir.dialogue-line-inventory.v1` over length-prefixed canonical fields.
It is not a wire version and never replaces project/source revision authority.

Input module permutations therefore yield equal records, indexes, diagnostics,
and fingerprint after canonical sorting.
