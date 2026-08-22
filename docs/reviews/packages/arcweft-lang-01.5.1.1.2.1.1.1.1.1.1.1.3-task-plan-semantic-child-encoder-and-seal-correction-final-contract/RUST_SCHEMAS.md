# Rust-shaped schemas

`schemas/final_contract.rs` is the normative API-shaped schema. It is not a
standalone crate and deliberately leaves already accepted Arcweft fields as
owner placeholders. The following rules are part of the schema, not optional
implementation notes.

## 1. Digest construction and visibility

The five digest newtypes (`TaskPlanSemanticDigest` plus its four child digests)
have private byte fields. Public code can compare and borrow bytes through
`as_bytes`; it cannot construct a value from bytes. The only ordinary creation
path is private `from_hasher_output` inside the owner module.

The upper View authority has one necessary cross-crate completion seam because
Rust has no friend-crate visibility. That seam is capability-gated:

```rust
ViewTaskPlanDigestRequest::finish_authority_transcript(self, blake3::Hasher)
```

It consumes an unforgeable, one-use request minted by the core encoder. It is
not a constructor on `TaskPlanSemanticDigest`, accepts no View identity fields,
exposes no sink, and cannot be invoked without an active core seal pass. The
only production caller is the validated bundle/View owner. A caller cannot
reuse the request, clone its base, deserialize it, or mint a request from raw
fields.

This closes the cross-crate construction problem without any of the forbidden
models:

- no public `TaskPlanSemanticDigest::from_bytes`;
- no `pub [u8; 32]` projection;
- no raw View identity in core;
- no callback that receives a core hasher/sink;
- no extension trait; and
- no caller field added to `RuntimeTaskPlan` or `NeedProducerSpec`.

## 2. Construction-only coordinate

`RuntimeTaskPlanBuildCoordinate` contains a private construction-owner token and
zero-based ordinal. A caller receives it only from
`RuntimePlanBuilder::push_runtime_task_plan` or from a decoded image's checked
ordinal resolver. The token prevents coordinates from another plan candidate
from joining a View binding.

Only the ordinal is encoded in the private bundle image. Decode resolves the
ordinal against the current private plan image and returns a token carrying the
same private owner. The coordinate is not a semantic ID and is not a plan
digest input except where explicitly used as a source-order role/reference in
the structured executable transcript.

## 3. Final table shape

The table keeps source-order sealed rows and one digest-to-index map:

```rust
struct SealedRuntimeTaskPlanRow {
    plan: RuntimeTaskPlan,
    digest: TaskPlanSemanticDigest,
}

pub struct RuntimeTaskPlanTable {
    rows: Box<[SealedRuntimeTaskPlanRow]>,
    by_digest: BTreeMap<TaskPlanSemanticDigest, RuntimeTaskPlanIndex>,
}
```

The digest belongs to the table association, not to `RuntimeTaskPlan` itself.
There is no second catalog. Multiple producer sites may reference the same
index/digest. A second separately declared row with the same digest is rejected
rather than silently deduplicated, because silent deduplication would erase its
source-order build coordinate and obscure malformed decoded images.

## 4. Common seal implementation

Builder and decoder both create `UnsealedRuntimePlanImage` and invoke exactly:

```rust
RuntimePlanSemanticEncoder::new(...)
    .seal_task_plans(authority)
```

The decoder differs only by carrying private expected keys and a private
construction owner minted for the decoded candidate. Expected bytes are checked
after recomputation and before uniqueness. They are never converted with a raw
digest constructor.

## 5. Ordinary plan independence

`RuntimePlanBuilder::finish()` calls the common method with `None`. The encoder
does not query an authority while sealing Ordinary, AwaitManyBase,
AwaitManyChild, Timeout, or Line. A View authority is required only when the
first `RuntimeTaskSemanticBinding::View` row is reached. Therefore a completely
ordinary plan has no runtime or compile-time dependency on a View registry.

## 6. Ownership of enum behavior

`NeedProducerFamily::validate_runtime_task_binding` owns the exhaustive
family/binding match. `RuntimeTaskSemanticBinding::semantic_tag` owns the closed
binding tags. `TaskClass::semantic_tag` remains inherent on the existing enum.
No free `family_to_binding`, feature-local helper, macro-generated side table,
or extension trait is authorized.

## 7. Serialization policy

Public `RuntimePlan`, validated bundle resources, and snapshots use their
purpose-built codecs. The semantic base/request and all child encoder contexts
are nonserialized. Private decoded expected keys are codec data, not semantic
owners. Snapshot restore resolves a stored plan digest byte string against the
already sealed table and returns that existing typed key; it cannot fabricate a
new digest.

## 8. Error ownership

- core structural and seal errors: `RuntimeTaskPlanSealError`;
- upper current-program/site/admission errors:
  `ViewTaskPlanValidationError` through the core protocol;
- complete View product errors: existing `ViewProductValidationError` evolved
  in place;
- outer atomic bundle errors: existing bundle validation error evolved in
  place.

No compatibility error, old-reader success branch, or generic string error is
added.
