# Bundle cross-section and product join

## Sole section

`ViewReactiveBindingSectionV1` is the only static join between the core-independent View program and AWBC program. It is encoded in the product bundle, covered by the content root, merged atomically, and validated before runtime publication.

It contains exactly:

- schema version 1;
- exact `ResourceTypeRegistryDigest` used by sema/View compilation;
- site-sorted Match selector bindings;
- producer-contract-sorted Need producer bindings; and
- canonically sorted typed source-map role rows.

It contains no runtime value, observer/journal/frame/register state, source text, duplicate semantic type map, or mutable endpoint registry.

## Match join

For each ViewMatchSiteId validation requires:

- site exists exactly once on View and bundle sides;
- View arms and bundle cases are dense/equal in count/order;
- `ViewMatchArmOrdinal == case_ordinal`;
- output ordinal is dense/equal to array index;
- local/body coordinates exist and body ranges are valid;
- checked-match digest equals same `FinalSemanticAnalysis` reference;
- selector signature is exactly `[input_state_type] -> result_type`;
- input/result types and canonical digests match verified AWBC;
- payload tuple/item types match outputs; and
- every disposition is SnapshotClone.

View carries no function/type ID. The bundle section is the sole join; only runtime-driver consumes both sides together.

## Need producer join

For each NeedProducerContractDigest validation requires exactly one binding and verifies producer flags/body/result, `NeedHandle<T>`, payload type/digest, one task plan, exact argument types, task signature/host metadata, JoinSameKey, `many == None`, absence of static need String, and contract recomputation.

The binding is immutable product metadata. The existing producer journal is the sole runtime endpoint/state table—no second endpoint table exists.

## Source-map rows

`ViewReactiveSourceMapEntryV1` has one closed role:

- MatchSite(site);
- MatchArm(site, arm);
- MatchBinding(site, arm, output); or
- NeedProducer(producer_contract).

Each points to an in-range `AwbcSourceMapId`. Match roles must resolve to an existing selector/case/output; producer roles must resolve to an existing producer binding. Required generated rows are complete and unique. Source maps support diagnostics only and never participate in semantic/runtime identity.

## Canonical order and merge

Canonical sort is selector site; case arm; output ordinal; producer-contract bytes; then source-map role discriminant and coordinates. Section construction rejects noncanonical order and duplicates. Merge is set union by canonical key with byte-for-byte exact-equality duplicate admission; conflicting duplicates are errors. Last-writer-wins is forbidden.

## Digest/generation coverage

The bundle content root covers complete section bytes. ProgramGeneration therefore binds View program, AWBC program, resource registry digest, reactive joins, and source-map role rows under one active generation. Tampering with selector state/result type, case/output, producer/task plan/contract, resource digest, source-map role, or ordering changes the root or fails validation.
