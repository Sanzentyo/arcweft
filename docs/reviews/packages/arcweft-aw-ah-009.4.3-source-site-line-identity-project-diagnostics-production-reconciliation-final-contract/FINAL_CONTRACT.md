# Final contract and precedence

## 1. Authority

This package is the final AW-AH-009.4.3 production-reconciliation contract. It
supersedes only unresolved or contradictory source-site line-identity clauses.
It does not redesign validated substrate.

Precedence is fixed as follows:

1. this package for package-aware line-site ownership, candidate construction,
   project collision acceptance, accepted line facts, text-key derivation,
   diagnostics, invalidation, rename behavior, and migration order;
2. AW-AH-009.4.2 for the one source-backed
   `HirExprKind::DialogueContentApplication`, its immediate `id`/`text_key`
   coordinates, typed `HirIdRef`, source component map, poison state, and lexical
   owner;
3. proof-concurrency v6.1.1 for `HirModuleKey`, `HirSnapshotId`, typed HIR IDs,
   immutable module snapshots, `ScopeId`, transactional lowering, and
   revision-bound source identity;
4. AW-AH-009.3.2 for one accepted `Arc<HirProject>`, source registry, generation,
   request leases, and atomic replacement;
5. AW-AH-009.3.3 for the shared callable catalog/resolver and existing
   `CallableDeclarationId`;
6. AW-AH-009.4 Cut 1 for the immutable CharacterDialogue domain and the exact
   256-byte line-identity production limit;
7. later AW-AH-009.4 cuts for sema value construction, runtime-plan, AWBC,
   display/save, and View projection.

## 2. Machine result

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_RESULT_CHANGING_DECISIONS=0
PRODUCTION_CHANGES_INCLUDED=0
BASELINE_GIT=27227bbc8e1d5c78d7b35c2865bad8fb6d00fca9
AW_AH_009_4_2_SHA256=05e825dde033f308f24fc1f6e504b4c26bba2d61fd33852ce880dc666ba8f2a8
```

## 3. Decision ledger

### D-001 — the AW-AH-009.4.2 package is the exact source-site prerequisite

The locally supplied AW-AH-009.4.2 ZIP was rehashed, its 16-member whitelist,
zero self-entry, non-self member hashes, CRCs, extraction bytes, and exact
`OPEN_QUESTIONS.md == "none\n"` were verified. Its archive SHA-256 is the value
above. No alternate source application owner is permitted.

### D-002 — one package-aware module key exists before line construction

Final lowering consumes exactly:

```rust
pub struct HirModuleKey {
    package: CallablePackageId,
    path: CanonicalModulePath,
    source: SourceDocumentIdentity,
}
```

The proof package's older `SourceDocumentId` example is tightened to the current
revision-bound `SourceDocumentIdentity`. This is not a second document identity;
it is the exact current production owner already required by
`HirProjectModule::try_new` and accepted project publication.

### D-003 — one checked lowering request owns the complete source tuple

`LoweringRequest` carries `HirModuleKey`, the existing `SourceSnapshotId`, and
the exact `SourceDocument`/typed syntax snapshot. Construction rejects any
package, module, snapshot, or document mismatch before HIR allocation. No later
project stage infers package or module from a display name.

### D-004 — line sites have one closed typed owner

`HirDialogueLineSourceOwner` is exactly `Flow`, `Callable`, or `Ownerless`.
Flow owns a checked complete `flow.*` `PublicId`. Callable owns the existing
`CallableDeclarationId`. Ownerless is explicit and never represented by an
empty string.

### D-005 — module/package facts are projected, not copied inconsistently

Package, module, and source identity come from `HirModuleKey`. A callable owner
must carry the same package/module as that key. A mismatch is a fatal lowering
invariant. Candidate records do not carry an independently constructible second
package or module identity.

### D-006 — named lexical scopes are typed

Each contributing named scope is a `HirDialogueNamedScope { scope: ScopeId,
segment: ModuleSegment, declaration: SourceSpan }`. Unnamed scopes affect
lexical ownership but contribute no line-ID segment. Named scopes are retained
outermost to innermost.

### D-007 — flow prefix is frozen

For a flow owner, generated/relative prefix construction is:

```text
say.<complete flow PublicId body>.<remaining named scopes>
```

The flow body is opaque typed `PublicId` data. It is not split to recover a
Character or callee. The frozen example therefore produces
`say.flow.game.intro.scene.greeting.002`.

### D-008 — callable prefix is frozen

For a callable owner, prefix construction is:

```text
say.fn.<package>.<canonical module segments>.<owner family>
       .<typed owner-path segments>.<callable name>.<remaining named scopes>
```

The `crate` display word is not a segment. The existing
`CallableDeclarationOwner::as_str`, owner path, and callable name are the sole
authorities. The frozen example produces
`say.fn.game.game.dialogue.function.phone_line.retry.001`.

### D-009 — no Character fact contributes to line identity

CharacterId, aliases, local variable names, callee labels, display names,
`.say`, source spelling, and narrator normalization are absent from every owner,
prefix, candidate, accepted record, digest, and rename key.

### D-010 — durable source IDs have a lower identity owner

`arcweft-id::dialogue` owns `DialogueLineId(PublicId)`,
`DialogueTextKey(TextKey)`, and `MAX_DIALOGUE_ID_BYTES = 256`. Their checked
constructors enforce exact `say.`/`text.` family, nonempty tail, generic ID
validity, and inclusive UTF-8 byte length. HIR uses these types; it does not
depend on `arcweft-dialogue`. Cut 1 retains its public
`max_line_id_bytes` field and initializes it from the same lower constant, so
its value and behavior are not redesigned.

### D-011 — source/public and runtime IDs stay distinct

`DialogueLineId` retains the source/public `say.*` family. The later
`RuntimeLineId` remains a pathless runtime lookup ID. Conversion occurs only in
runtime-plan lowering through a checked owned conversion; neither side parses
the other to recover Character identity.

### D-012 — module lowering creates unaccepted candidates only

Each executable HIR module owns an immutable source-ordered
`HirDialogueLineCandidates`. It contains validated candidates and no
project-wide acceptance claim. No module-local successful collision table is
public.

### D-013 — source order and generated order are different counters

`DialogueLineSourceOrder(u32)` advances for every source-backed application site
encountered by deterministic HIR traversal. A per-exact-prefix generated ordinal
advances only when a complete generated candidate, including its text key, has
validated successfully.

### D-014 — explicit sites never consume generated ordinals

Any site with an authored `id` coordinate leaves the generated ordinal state
unchanged, whether that explicit ID succeeds or fails.

### D-015 — failed and recovered sites never consume generated ordinals

Poisoned/recovered applications produce no candidate. Wrong family, dynamic
coordinate, duplicate coordinate, scope escape, oversized ID, invalid text key,
checked-arithmetic failure, or any other failed candidate does not commit a
counter. Candidate construction peeks the next ordinal and commits it only after
the entire candidate is valid.

### D-016 — generated ordinal formatting is exact

Ordinals start at 1 per exact prefix, use ASCII decimal, are left-padded to at
least three digits, and are never truncated. Values 999 and 1,000 format as
`999` and `1000`. The production per-prefix maximum is 262,144.

### D-017 — explicit line-ID resolution is typed

An absolute `HirIdRef` must be exactly `@say.<nonempty tail>` and is preserved.
An unqualified relative ID and `@say:` family-relative ID resolve identically
under the exact owner prefix. Any other family is AW-CD-013. No source text is
re-read.

### D-018 — parent traversal removes named scopes only

`RelativeId.parent_depth` removes that many trailing
`HirDialogueNamedScope` entries. It never removes the flow/callable owner,
package, module, owner family/path, or callable name. Traversal beyond the
available named scopes is AW-CD-022.

### D-019 — ownerless sites have one rule

An ownerless application succeeds only with one clean absolute `@say.*` ID.
Generated, unqualified relative, and family-relative IDs are AW-CD-021. An
ownerless absolute site still participates in the same project namespace.

### D-020 — line coordinates are constant-only

The candidate builder consumes the immediate coordinate products fixed by
AW-AH-009.4.2. `id` must resolve to typed `HirIdRef`; a runtime expression is
AW-CD-023. Duplicate immediate `id` coordinates are AW-CD-027. Nested names do
not participate.

### D-021 — explicit text keys are absolute

An authored `text_key` must be one typed absolute `@text.*` value. Relative and
family-relative text-key values are rejected as AW-CD-024 because no
owner-relative text-key spelling was frozen and the old implementation depended
on speaker identity. Duplicate or dynamic coordinates are AW-CD-027/AW-CD-023.

### D-022 — absent text keys derive mechanically

For accepted line ID `say.<complete body>`, absence of `text_key` produces
`text.<complete body>` by replacing only the leading family. The output is
validated as `DialogueTextKey`; no segment is interpreted as CharacterId.

### D-023 — text keys are not a uniqueness namespace in this cut

Multiple accepted lines may intentionally share one explicit text key. Derived
keys are unique when line IDs are unique. There is no second project collision
table for text keys. Localization can index one key to multiple line sources.

### D-024 — one package-qualified project module map is final

The final `HirProject` has one root package and one package-qualified module map.
A module lookup key is the existing package plus `CanonicalModulePath`; its
snapshot retains `HirModuleKey`. Root and dependency modules therefore
participate in one project without a second HIR project or flattened linked
module.

### D-025 — one project builder is the publication gate

`HirProjectBuilder` accepts package-aware module snapshots into a `BTreeMap` and
`finish()` performs module validation, callable publication, line collision
acceptance, and immutable project construction. No `HirProject` is returned on
any failure.

### D-026 — one line namespace covers every origin

One temporary `BTreeMap<DialogueLineId, PendingSite>` receives absolute,
relative, family-relative, and generated candidates from all root/dependency
modules. There is no origin-specific table and no generated-ID reservation API.

### D-027 — canonical traversal is independent of input order

Modules sort by package, canonical path, and exact source identity. Sites sort
by application SourceSpan, source-order coordinate, and ExprId. The same module
set in any caller order yields structurally and canonically identical accepted
inventories or diagnostics.

### D-028 — collision primary and secondary are fixed

The first site in canonical traversal is retained in scratch state. Every later
site producing the same ID yields AW-CD-020 with the later site as primary and
the first site as secondary. This rule covers explicit/explicit,
explicit/generated in either authored order, generated/generated, and
cross-document collisions.

### D-029 — collisions never skip or renumber

A generated candidate is complete before project insertion. An occupied value
produces AW-CD-020. The builder does not request another ordinal, mutate the
module candidate, or search for a free value.

### D-030 — independent collision diagnostics are accumulated deterministically

Project acceptance continues after a collision using the original first site as
the comparison owner, collecting independent collisions until the fixed
diagnostic or work limit. A diagnostic-limit/work-limit failure is fatal and
invalidates the whole transaction.

### D-031 — failure is fully atomic

All insertion maps, counters, diagnostics, indexes, and accepted records are
scratch values owned by `finish()`. Rejection returns no inventory, does not
increment an accepted generation, does not mutate the previous accepted
`Arc<HirProject>`, and cannot affect a later build.

### D-032 — recovered modules are tooling-only

A recovered/poisoned HIR module may exist in the HIR database for tooling but
must have no executable line candidates. `HirProjectBuilder::finish` rejects any
non-executable module in an executable project before collision publication.
Declaration-free executable modules are valid and contribute an empty inventory.

### D-033 — accepted output is immutable and indexed twice

`AcceptedDialogueLineInventory` stores records in canonical line-ID order and
owns checked indexes by `DialogueLineId` and source-backed `ExprId`. It also
retains a source-order index for tooling. Every accepted record contains the
typed line ID, typed text key, origins, exact owner/scopes, application ExprId,
and revision-bound source evidence.

### D-034 — line collisions publish no text-key fact

Candidate text keys may be validated module-locally, but no accepted line or
text-key record is visible unless the whole project transaction succeeds.

### D-035 — structured diagnostic identity lives in HIR/project ownership

`DialogueLineDiagnostic` is a closed typed HIR/project diagnostic. It retains
its subject ID, limit kind, owner kind, and every exact SourceSpan. Rendering is
an inherent `to_source_diagnostic()` method that reuses
`arcweft_source::Diagnostic`; no string-only compatibility error is introduced.

### D-036 — stable codes are reserved

This contract reserves:

```text
AW-CD-013 InvalidLineIdFamily
AW-CD-020 LineIdCollision
AW-CD-021 MissingLineSourceOwner
AW-CD-022 RelativeLineIdEscapesOwner
AW-CD-023 InvalidLineIdentityCoordinate
AW-CD-024 InvalidTextKeyFamily
AW-CD-025 DialogueLineIdentityLimit
AW-CD-026 DialogueLineSourceMismatch
AW-CD-027 DuplicateLineIdentityCoordinate
AW-CD-028 InvalidDialogueLineIdentity
```

### D-037 — collision labels are cross-document capable

AW-CD-020 projects one primary label for the later site and one secondary label
for the first site. CLI/Agent render both. LSP publishes on the primary document
and uses existing related-information projection for a secondary document.

### D-038 — ordinary lowering errors do not erase line identity

Authored identity errors become structured HIR diagnostics and make the module
non-executable. Source/snapshot mismatch, arithmetic overflow, and budget
exhaustion are fatal typed transaction errors. Project collisions are typed
project-construction rejection. None is flattened into current
`HirLowerError { message, range }`.

### D-039 — no independent line revision exists

Module candidate identity is owned by existing `HirModuleKey` plus
`HirSnapshotId`. Project line facts are owned by the exact `Arc<HirProject>` and
existing project/source revisions. No second generation counter or public cache
revision is added.

### D-040 — no-op and changed invalidation are exact

A no-op syntax/HIR rebuild reuses the same module snapshot and candidate `Arc`.
An unchanged accepted source set reuses the same `Arc<HirProject>`. A changed
source invalidates its module snapshot and the project collision result; other
module candidate Arcs remain reusable.

### D-041 — every consumer borrows one accepted generation

Sema, source index, rename, runtime-plan, compiler, LSP, Agent, MCP, and tooling
obtain line facts only from the same generation-owned `Arc<HirProject>` already
held by the accepted snapshot/lease. No consumer can publish or reconstruct a
parallel line table.

### D-042 — rename is line-owned

An explicit line rename edits the exact authored ID span and typed line
references. A generated line rename first materializes one immediate explicit
`id = @say.*` coordinate using AW-AH-009.4.2 component/insertion facts, then
renames typed references. Derived text keys track the new line ID; explicit text
keys remain unchanged. Character rename never touches either.

### D-043 — public and crate boundaries are narrow

Durable ID wrappers and immutable accepted inspection records are public in
responsibility modules. Candidate builders, ordinal state, scratch maps, and
project insertion machinery are crate-private. Session identities and
SourceSpans are non-Serde. Later data-format codecs encode only checked durable
IDs.

### D-044 — old identity recovery is deleted

`DialogueSpeakerSlug`, speaker-derived prefixes, `.say` stripping, callee/source
spelling identity, character-name-derived IDs, mutable `LowerContext.line_counters`,
post-lowering source scans, silent local skipping, and line-identity uses of
single-range/string-only errors are deleted in the direct replacement.

### D-045 — public replacement is one compiling series

Private independent substrate may land first. Package-aware lowering migration,
`HirProject` replacement, accepted line publication, downstream consumer
switch, and old-model deletion form one unmerged direct-replacement series and
are published only when the workspace again has one compiling model.

### D-046 — exclusions remain absolute

No compatibility shim, alias, dual reader, deprecated helper, source gate,
source text scan, `.say` recognizer, CSS/Takumi path, second HIR project, second
source index, second diagnostic transport, runtime wire choice, View projection,
voice/TTS decision, or text-layout change is part of this contract.

## 4. Frozen examples

```text
flow.game.intro + scene/greeting + generated #2
  -> say.flow.game.intro.scene.greeting.002

package game + module game.dialogue + Function phone_line + retry + #1
  -> say.fn.game.game.dialogue.function.phone_line.retry.001

@.greeting == @say:.greeting under the same owner/scopes
@super removes exactly one named scope
say.<body> -> text.<body>
```
