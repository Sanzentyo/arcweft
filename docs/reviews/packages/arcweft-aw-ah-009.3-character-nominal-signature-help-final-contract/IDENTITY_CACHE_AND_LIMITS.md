# Identity, cache, limits, and errors

## 1. Selected identity branch

The query uses:

```text
SourceDocumentIdentity
+ exact parser-retained CallExpressionSyntax / ArgumentListSyntax ranges
+ exact document-bound HIR
+ accepted project symbol world and revisions
```

It does not use `SourceSnapshotId` or `SyntaxNodeId`. No conversion, parsing,
serialization, hashing bridge, or derivation exists between
`SourceDocumentIdentity` and `SourceSnapshotId`.

The range branch is safe because:

1. `lower_document_to_hir` accepts only a typed tree whose source text equals
   the supplied `SourceDocument`;
2. HIR retains that exact `SourceDocumentIdentity`;
3. parser-owned call ranges are validated against the same text length and
   UTF-8 boundaries;
4. `ProjectSymbolTable` records the identity of every canonical module;
5. the query verifies document, HIR, module, symbol world, and symbol revision
   before work;
6. the LSP layer verifies its document and accepted environment again before
   cache publication.

Proof 01.1.1 may add stable typed-node identity later without changing this
query's correctness. This implementation does not wait for it.

## 2. LSP request stamp

`arcweft-lsp::features::signature` captures one immutable request stamp:

```rust
pub(crate) struct SignatureRequestStamp {
    profile_state: std::sync::Arc<LspProfileState>,
    accepted: std::sync::Arc<AcceptedProfileEnvironment>,
    generation: AcceptedEnvironmentGeneration,
    world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    character_revision: CharacterInventoryRevision,
    character_digest: CharacterInventoryDigest,
    source: SourceDocumentIdentity,
    lsp_version: Option<i32>,
    byte_offset: usize,
}
```

The constructor reads all semantic fields from `accepted.world()` and all
source fields from one cloned `DocumentSnapshot`. It never reads a candidate
profile rebuild.

Before returning or inserting a cache value, the feature handler:

1. re-reads the URI's current `DocumentSnapshot` and compares identity and LSP
   version;
2. re-reads the URI's profile state and requires pointer equality with
   `profile_state`;
3. calls `profile_state.current()` and requires pointer equality with
   `accepted`;
4. compares generation, world, symbol revision, character revision, and
   character digest;
5. verifies the selected source identity is still the project table's identity
   for the HIR module.

Any mismatch discards the computed value and returns the matching typed stale
error.

## 3. Cache owner and key

The placeholder string cache in `AcceptedProfileEnvironment` is replaced in
place; no compatibility cache remains:

```rust
struct ProfileSemanticCaches {
    signature_help: std::sync::Mutex<SignatureHelpCache>,
}

struct SignatureHelpCache {
    entries: std::collections::BTreeMap<SignatureCacheKey, SignatureCacheEntry>,
    access_clock: u64,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct SignatureCacheKey {
    generation: AcceptedEnvironmentGeneration,
    world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    character_revision: CharacterInventoryRevision,
    character_digest: CharacterInventoryDigest,
    source: SourceDocumentIdentity,
    lsp_version: Option<i32>,
    byte_offset: usize,
}

struct SignatureCacheEntry {
    value: CacheableSignatureOutcome,
    last_access: u64,
}

enum CacheableSignatureOutcome {
    Help(SemanticSignatureHelp),
    NotApplicable(SignatureNotApplicable),
}
```

The cache is physically owned by one accepted environment, so a string profile
ID is not repeated in the key. Profile pointer and accepted pointer checks
provide profile identity. Negotiated position encoding is absent because the
key contains the already-checked byte offset.

Only final `Help` and stable `NotApplicable` values are cacheable. Errors,
stale results, cancellations, deadlines, authority ambiguities, arithmetic
failures, and limit failures are never cached.

A poisoned cache mutex is cleared using the recovered inner value; the query
continues uncached and may repopulate after a successful final stamp check. A
cache fault never changes semantic resolution.

## 4. Cache capacity and eviction

The inclusive per-accepted-generation cache capacity is **512 entries**.

Insertion behavior is deterministic:

1. a matching key updates `last_access` after checked clock increment;
2. a new entry below capacity is inserted;
3. a new entry at capacity evicts the least `last_access` value; ties use the
   lexicographically smallest `SignatureCacheKey`;
4. access-clock overflow clears the cache, resets the clock to zero, and inserts
   the new value at access one;
5. an entry whose complete owned size cannot be represented is not cached, but
   the already-computed result may still be returned.

Eviction does not truncate a semantic result.

## 5. Invalidation matrix

| Event | Required behavior |
| --- | --- |
| successful accepted profile replacement | `replace_accepted` increments generation and creates a fresh empty cache; old cache remains reachable only by in-flight old `Arc`s, whose final stamp check fails |
| profile replacement with identical semantic facts | still a new generation and fresh cache |
| manifest change with unchanged character digest | successful replacement still creates a new generation; no old key is reused |
| character digest or revision change | new accepted environment and key; old result cannot publish |
| symbol world or revision change | new accepted environment and key; old result cannot publish |
| document text change | new `SourceDocumentIdentity` and usually new LSP version; old key misses and final check rejects old computation |
| LSP version change with identical bytes | identity may match but version differs; old key misses |
| failed profile rebuild | no `replace_accepted` call, no generation, world, revision, or digest change, and no candidate facts enter the key; prior accepted world/cache remain atomic |
| failed rebuild after a project source change | project table module identity differs from the new document; query returns stale rather than combining old world and new HIR |
| failed manifest rebuild with unchanged source document | prior accepted world and prior cache remain usable as one atomic older semantic world; rebuild diagnostics remain visible separately |
| document close | evict all entries for the document identity, remove URI profile mapping, then shut down the profile state when it is URI-owned |
| workspace removal | shut down all profile states owned only by the removed workspace and clear their accepted caches |
| session shutdown | stop admission, take every accepted environment, clear caches, then mark states closed |
| cancellation or deadline | no insertion even when a partial result was internally assembled |

A failed rebuild never creates a key from attempted inputs and never pairs an
old world with a new character digest or symbol revision.

## 6. Production query limits

All maxima are inclusive:

```rust
pub struct SignatureQueryLimits {
    candidate_calls: u64,
    overloads: u64,
    parameters_per_signature: u64,
    nested_calls: u64,
    recovery_nodes: u64,
    source_bytes: u64,
    diagnostics: u64,
    work_units: u64,
}

impl SignatureQueryLimits {
    pub const PRODUCTION: Self = Self {
        candidate_calls: 4_096,
        overloads: 64,
        parameters_per_signature: 128,
        nested_calls: 64,
        recovery_nodes: 512,
        source_bytes: 8_388_608,
        diagnostics: 32,
        work_units: 262_144,
    };
}
```

A checked test constructor accepts custom positive limits. Zero is rejected by
`SignatureLimitConfigurationError::Zero { kind }`.

## 7. Work accounting

`SignatureWork` records counters by `SignatureWorkKind` and a total. Every
counter and the total use checked `u64` addition. The query charges **before**
performing the corresponding operation:

| Operation | Charge |
| --- | ---: |
| visit one HIR/typed syntax node while locating call surfaces | 1 candidate-work unit |
| inspect one call or dialogue argument-list node | 1 candidate-call and 1 work unit |
| enter one additional nested call level | 1 nested-call and 1 work unit |
| inspect one `ArgumentSyntax` | 1 work unit |
| inspect one recovery node | 1 recovery-node and 1 work unit |
| probe one resolver family in the fixed precedence list | 1 work unit |
| materialize one candidate | 1 overload and 1 work unit |
| materialize one parameter | 1 parameter count and 1 work unit |
| attempt one argument-to-parameter binding | 1 work unit |
| compare one candidate's viability/specificity against one argument | 1 work unit |
| consider one diagnostic before bounding | 1 work unit |

Hash-table/B-tree implementation comparisons are not counted; only semantic
operations above are. This keeps tests independent of container internals.

## 8. Exact-limit and one-over policy

- Candidate calls, overloads, parameters, nesting, recovery nodes, source bytes,
  and work units succeed at the exact maximum.
- Observing maximum plus one returns `SignatureLimitExceeded`, discards all
  partial semantic output, and publishes no cache entry.
- Overloads and parameters are never truncated because truncation can alter
  active-signature selection.
- Diagnostics are the sole deterministic truncation case. The first 31 sorted
  diagnostics are retained, the final slot is `DiagnosticsTruncated`, and
  `omitted_diagnostics` records the number omitted. Exact 32 ordinary
  diagnostics succeed without the truncation marker. At 33, 31 ordinary
  diagnostics plus the marker are returned.
- Any checked-add or integer-conversion failure returns
  `ArithmeticOverflow { counter }`, publishes no result cache, and maps to LSP
  `RequestFailed`.

## 9. Limit error identity

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SignatureLimitKind {
    CandidateCalls,
    Overloads,
    ParametersPerSignature,
    NestedCalls,
    RecoveryNodes,
    SourceBytes,
    Diagnostics,
    WorkUnits,
}

#[derive(Clone, Debug, thiserror::Error, PartialEq)]
#[error("signature query limit exceeded")]
pub struct SignatureLimitExceeded {
    pub kind: SignatureLimitKind,
    pub observed: u64,
    pub maximum: u64,
}
```

Formatted text is presentation only. Tests assert fields and enum variants.

## 10. Position limits and ranges

The source-byte limit is checked before HIR traversal. Every `TextRange` and
`SourceRange` conversion uses checked bounds and exact `SourceDocumentIdentity`.
The LSP line/character conversion rejects out-of-range and split-scalar
positions instead of clamping.

Signature-label parameter offsets use checked UTF-16 code-unit counts and
checked `u32` conversion. A label overflow is an arithmetic failure and is not
cached.

## 11. Error/cache publication table

| Outcome | Return | Cache |
| --- | --- | --- |
| complete or recovered `Help` | success | yes after final stamp check |
| stable `NotApplicable` | success with `null` LSP result | yes after final stamp check |
| diagnostics truncated | success | yes; omitted count is part of value |
| stale at start or end | error | no |
| invalid position | error | no |
| no accepted environment | error | no |
| ambiguous same-rank authority | error | no |
| semantic facts unavailable | error | no |
| limit or arithmetic failure | error | no |
| cancelled or deadline elapsed | error | no |
