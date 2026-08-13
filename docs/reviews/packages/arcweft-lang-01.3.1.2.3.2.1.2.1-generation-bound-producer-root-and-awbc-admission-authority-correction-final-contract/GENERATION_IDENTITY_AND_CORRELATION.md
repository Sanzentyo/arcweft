# Generation identity and correlation

## 1. One semantic identity, one host slot

`RuntimeGenerationIdentity` is the semantic identity of all authority-bearing
facts for one executable generation. It is a 32-byte BLAKE3 digest of the
canonical generation-contract body.

The existing runtime-driver `GenerationId(u64)` remains a host-local slot used
for scheduling and hot-swap bookkeeping. It is never serialized as semantic
authority and is never compared instead of `RuntimeGenerationIdentity`.

A host may assign a new `GenerationId` to an artifact with the same runtime
identity, or reuse a numeric slot after retirement. Neither operation changes
or aliases the semantic contract.

## 2. Owning aggregate

One `AdmittedRuntimeGeneration` owns:

- the exact canonical generation-contract bytes;
- `RuntimeGenerationIdentity`;
- canonical nominal catalog;
- independently derived project nominal closure;
- each producer's independently derived nominal closure;
- each validated raw producer payload contract;
- CharacterDialogue role and custom facts;
- Character/View catalog correlation digests.

`AdmittedRuntimePlan` and `AdmittedAwbcProduct` own a clone of the same
`AdmittedRuntimeGeneration`, whose private inner value is an `Arc`. All
operational handles borrow from that inner value.

No public API exposes the inner Arc, catalog ownership, a generation-free
handle, `Deref`, or `into_inner`.

## 3. Allocation source

The identity is not a counter, random token, timestamp, source hash, plan hash,
AWBC hash, bundle hash, or runtime session ID. The bridge computes it only from
canonical authority facts.

`RuntimeGenerationContractDeclaration::try_from_checked_projection` computes
the identity when producing a raw declaration. Deserialization accepts a
claimed identity only as untrusted data. Plan/AWBC admission recomputes it.

## 4. Exact equality and collision defense

Within one process and across persisted artifacts:

1. compare all 32 identity bytes;
2. when artifacts are being combined, compare canonical contract body bytes;
3. reject equal identity with different body as
   `RuntimeGenerationContractError::IdentityCollision`.

Operational handles from different aggregate allocations but byte-identical
contracts may be independently valid standalone generations. They cannot be
mixed without the same identity/body comparison.

## 5. RuntimePlan and AWBC

A paired plan/AWBC path always admits the plan first. The AWBC product is then
admitted through `AdmittedRuntimePlan::try_admit_awbc`, which reuses the plan
aggregate.

Standalone AWBC admission is allowed because AWBC embeds the complete
generation contract. A standalone admitted product cannot later be attached to
a plan; the raw program must be admitted through the plan wrapper so only one
aggregate is used.

## 6. ProgramGeneration and generation images

`ProgramGeneration` gains a required
`RuntimeGenerationIdentity` and retains the admitted generation aggregate.
`GenerationRuntimeImage<R>` owns both the host slot and the admitted aggregate.

The existing `into_runtime` escape is deleted when it would drop generation
authority. The selected replacement returns an admitted image object or a
tuple containing the runtime plus `AdmittedRuntimeGeneration`; it never returns
bare runtime state that can be activated under another generation.

## 7. Character and View catalogs

Raw `CharacterCatalog` and raw `ViewRegistry` remain their domain owners.
Operational wrappers are issued only after:

- exact generation identity comparison;
- recomputation of the raw catalog's canonical digest;
- exact comparison with the digest inside the admitted CharacterDialogue
  producer payload.

The wrapper stores the identity and digest. Schema construction checks identity
before producer or catalog details, as required by the selected precedence.

## 8. Save, restore, replay, and snapshots

Every persisted runtime value/snapshot carrying generation-dependent typed
data records `RuntimeGenerationIdentity`. Restore performs:

1. decode/version checks;
2. load and admit the target plan or AWBC product;
3. compare saved identity with target identity;
4. obtain catalog/producer/schema views from the target aggregate;
5. validate values;
6. only then reconstruct ownership, fibers, roots, View state, or session
   activation.

A snapshot may never choose a generation by host `GenerationId`, display name,
producer ID, plan filename, or hash subset.

## 9. Hot swap

Hot swap admits the complete candidate first. It then checks migration policy
and generation identity/canonical contract relation before touching the active
image.

Same-identity replacement is allowed only when canonical contract bytes are
identical. Different-identity replacement is a real generation transition and
must use the existing typed migration policy; no handle or typed value from the
old aggregate is reused.

Any late failure leaves the prior image fully active and publishes no candidate
catalog or producer view.

## 10. Lifetime role

Lifetimes prevent a borrowed view from outliving its aggregate. They do not
prove two separately owned values came from the same generation. Every
cross-object operation therefore checks `RuntimeGenerationIdentity`; artifact
join additionally checks canonical bytes.
