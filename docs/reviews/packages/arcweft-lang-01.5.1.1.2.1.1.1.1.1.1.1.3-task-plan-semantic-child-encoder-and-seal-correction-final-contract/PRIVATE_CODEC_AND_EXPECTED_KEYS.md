# Private runtime-plan codec image and expected-key verification

## 1. Ownership and visibility

The wire rows in this file are private codec images. They are not public core
DTOs, do not implement a generic RuntimePlan `Deserialize`, and cannot be
constructed with public field literals. The codec module may use purpose-built
version-one readers/writers only.

The final public `RuntimeTaskPlan` does not contain an expected key. The wire
image does because the key is an integrity assertion that must be verified by
recomputation before publication.

## 2. Structured task-plan section grammar

```text
section_tag:u8 = assigned existing runtime-plan section tag
section_version:u8 = 1
row_count:u32-le
for each row in source order:
  coordinate_ordinal:u32-le
  producer_function: canonical RuntimeFunctionSiteId wire projection
  family_tag:u8
  task_class_tag:u8
  request_template: canonical RuntimeHostTaskRequestTemplate wire projection
  control_effect: canonical RuntimeControlEffectContractId wire projection
  binding:
    tag:u8
    tag 0 Ordinary:       no payload
    tag 1 View marker:    no payload
    tag 2 AwaitManyBase:  no payload
    tag 3 AwaitManyChild: no payload
    tag 4 Timeout:        NeedTimeoutContractDigest:digest32
    tag 5 Line:           LinePlanSemanticDigest:digest32
  expected_task_plan_key:digest32
```

`coordinate_ordinal` must equal the current row ordinal. The private decoder
mints one decoded-candidate owner token and resolves each ordinal to an owner-
bound `RuntimeTaskPlanBuildCoordinate`. It rejects an unknown family/class/
binding tag, mismatched family/binding, noncanonical coordinate, trailing byte,
unknown field, duplicate section, and count/limit violation.

The nested function/request/control identifiers use their existing purpose-
built canonical wire owners. Their wire projections are not semantic digests;
the common sealer resolves them against the complete decoded plan and computes
all four child digests again.

## 3. Validated View task-binding section grammar

This section is owned by the bundle/View codec, not core:

```text
section_tag:u8 = assigned existing validated-View task-binding section tag
section_version:u8 = 1
row_count:u32-le
for each row sorted by coordinate_ordinal:
  coordinate_ordinal:u32-le
  ViewProgramId: canonical validated public-ID string
  AcceptedViewProgramRevision:digest32
  ViewMatchSiteId:digest32
  CheckedViewMatchAdmissionDigest:digest32
```

The accepted revision is persisted because it validates the current resource.
It is omitted from the semantic task-plan transcript. The decoder resolves each
ordinal through the private decoded RuntimePlan image before constructing the
validated upper binding; it cannot create a free coordinate.

Rows must exactly cover the View-marker coordinates in the decoded core image.
Missing, extra, duplicate, unsorted, cross-program, stale-revision, site, or
admission rows reject before core sealing.

## 4. Decode/recompute/compare algorithm

For source-order row `i`:

1. strict decode creates a private static row and
   `ExpectedTaskPlanKey(expected_bytes)`;
2. structural validation resolves all nested IDs and family/binding rules;
3. child encoders compute `F_i`, `Q_i`, and `C_i`;
4. after all base rows are known, the executable encoder computes `E`;
5. core or the validated View authority computes `P_i` without reading
   `expected_bytes`;
6. compare `P_i.as_bytes()` with `expected_bytes`;
7. on mismatch, return `ExpectedKeyMismatch { coordinate, expected, actual }`;
8. only after every comparison passes, insert `P_i` in the global uniqueness
   map; and
9. only after final cross-reference checks, publish RuntimePlan/bundle.

Comparison is exact byte equality. It is not a zero check, fallback lookup,
rehash, or repair. All-zero is accepted only if it is the actual recomputed
BLAKE3 output.

## 5. Encode algorithm

The encoder accepts only a public RuntimePlan that already owns a complete
sealed table. It writes each source-order static row and the digest associated
with that table row. It does not ask a caller for an expected key. Reencoding a
decoded plan therefore writes recomputed trusted keys, not the untrusted input
bytes.

## 6. Atomic outer bundle rule

The outer loader retains private section images until the View product and core
plan both complete. It cannot publish a RuntimePlan first and attach a View
registry later. Conversely, it cannot publish a View task-binding resource that
refers to an unsealed plan coordinate. One final constructor receives both
complete products.

## 7. Tamper tests

Required fixtures alter independently:

- one expected key byte;
- row count and expected-key count;
- coordinate order/owner;
- family/binding tag;
- View binding program, revision, site, and admission;
- a nested request/control ID while preserving expected key;
- an expected key while also creating a duplicate; and
- trailing/unknown bytes.

Every fixture rejects at the first precedence row specified in
`ERROR_PRECEDENCE_AND_LIMITS.md` and publishes nothing.
