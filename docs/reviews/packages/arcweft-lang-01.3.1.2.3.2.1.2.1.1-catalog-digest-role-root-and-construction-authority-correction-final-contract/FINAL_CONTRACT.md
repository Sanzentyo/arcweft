# Final contract: catalog-digest role root and construction authority

## 1. Status and scope

This is the decision-complete, design-only correction for `lang-01.3.1.2.3.2.1.2.1.1`. It is normative over conflicting predecessor wording only at the catalog-digest role-root and runtime construction-authority boundary. All non-conflicting decisions from the retained parent contracts remain binding.

The package changes no production file. Implementation must be a single cutover: migrate all owners and consumers, add exact acceptance evidence, then delete bypasses. Compatibility layers, source gates, dual readers, ad-hoc digest helpers, extension traits that duplicate an Arcweft-owned enum, and producer-selected authority are forbidden.

## 2. Single authority graph

The only valid authority graph is:

```text
raw declarations / plan / AWBC / save / replay / external producer input
    │  (untrusted, Serde-capable assertions only)
    ▼
structural parse + bounded validation
    ▼
canonical typed catalogs per closed digest role
    ▼
core-derived role digests
    ▼
complete + unique admitted role root
    ▼
plan/AWBC pair correlation + generation derivation
    ▼
AdmittedRuntimeGeneration (opaque, non-Serde)
    ├── executable admitted plan/AWBC handles
    ├── admitted nominal/layout/catalog handles
    └── role/producer/layout-scoped construction capabilities
            ▼
       validated runtime values
```

A digest appearing in input is only an assertion to compare against the derived digest. Equality of asserted bytes never upgrades an input into an authority object. No API may accept a naked 32-byte digest, root digest, nominal ID, or layout ID and return an admitted handle or executable runtime value.

## 3. Role-root closure

`RuntimeCatalogDigestRole` is a closed Arcweft-owned enum. Its original inherent `impl` is the sole owner of:

- stable ordinal;
- v1 digest domain tag;
- required/optional cardinality (this contract requires the roles below exactly once);
- canonical catalog encoder selection;
- whether a role may issue construction capabilities;
- error-display label.

Do not add an extension trait, side table, string switch, duplicated helper, or consumer-local match for behavior missing from the enum. Add the behavior to the enum's original implementation.

| Ordinal | Role | v1 domain | Root cardinality | Authority rule |
|---:|---|---|---|---|
| `1` | `AcceptedNominalCatalog` | `arcweft/runtime-catalog-role/1/v1` | exactly one | core-derived digest; capability eligibility fixed by the original enum impl |
| `2` | `ExternalProducerDeclarationCatalog` | `arcweft/runtime-catalog-role/2/v1` | exactly one | core-derived digest; capability eligibility fixed by the original enum impl |
| `3` | `CharacterDialogueLayoutCatalog` | `arcweft/runtime-catalog-role/3/v1` | exactly one | core-derived digest; capability eligibility fixed by the original enum impl |
| `4` | `CharacterCatalog` | `arcweft/runtime-catalog-role/4/v1` | exactly one | core-derived digest; capability eligibility fixed by the original enum impl |
| `5` | `ViewCatalog` | `arcweft/runtime-catalog-role/5/v1` | exactly one | core-derived digest; capability eligibility fixed by the original enum impl |
| `6` | `CustomFieldSchemaCatalog` | `arcweft/runtime-catalog-role/6/v1` | exactly one | core-derived digest; capability eligibility fixed by the original enum impl |
| `7` | `AwbcRuntimePlanBinding` | `arcweft/runtime-catalog-role/7/v1` | exactly one | core-derived digest; capability eligibility fixed by the original enum impl |

The admitted root contains exactly one entry for every listed role and no unknown role. Input order has no authority. Duplicate role, missing role, unknown ordinal, wrong grammar version, asserted/derived role digest mismatch, or asserted/derived root mismatch fails admission before any operational object is published.

## 4. Acyclic digest and generation derivation

The derivation graph is acyclic:

```text
canonical role catalog bytes
  -> role digest
ordered (role ordinal, role digest) entries
  -> role-root digest
producer declaration root + role-root digest + plan/AWBC binding digest
  -> RuntimeGenerationIdentity
```

The role root does not include `RuntimeGenerationIdentity` and no role digest includes either the root digest or generation identity. Wire assertions may contain all three values, but core derives them in this order and compares assertions only after derivation. This prevents self-authenticating or cyclic roots.

## 5. Construction authority

`RuntimeConstructionAuthority` is an opaque, non-Serde capability backed by the admitted generation aggregate. It is not constructible from raw parts and has no `Default`, public unchecked constructor, `From<[u8; 32]>`, public `Clone` that changes scope, or deserialization path.

Every issued capability is bound to:

1. the exact admitted generation object;
2. its role-root digest;
3. one `RuntimeCatalogDigestRole` role;
4. an optional exact external producer identity where the role is producer-owned;
5. the admitted nominal/layout closure allowed for that scope.

Capability use rechecks active-generation identity at the mutation boundary. A capability retained across hot swap is stale and fails before allocation/publish. Same-generation opaque handle clones preserve identity; they do not reconstruct authority from bytes.

External producers receive only a narrower producer façade. It can construct values from admitted declaration/layout handles already closed under that producer. It cannot choose a nominal catalog, role digest, root digest, generation identity, or arbitrary checked type.

## 6. Runtime nominal and CharacterDialogue construction

The original `RuntimeNominalRecordAdmissionDomain` implementation remains the final nominal value invariant owner. Its admitted-layout constructor is crate-private (or narrower) and requires an admitted layout handle plus the scoped authority. All fields, nested nominals, owner identity, exact checked types, and generation correlation are validated before the value is returned.

CharacterDialogue does not use descriptorless nominal wrappers or a generic raw nominal constructor. Each closed CharacterDialogue runtime role is created through a dedicated typed façade derived from the corresponding admitted role capability. Custom entries resolve field ID to the admitted exact checked type and accepted View closure; callers cannot pass `Dynamic` or an arbitrary checked type. Normalize, clear, and structured patch reconstruct through the same façade, run the nominal postcondition, and publish atomically.

## 7. Plan, AWBC, activation, restore, and replay

`AdmittedRuntimePlan` and `AwbcProgram` raw forms are not executable. Pair admission derives/validates the same role root and generation, then produces one `AdmittedRuntimeGeneration` aggregate. No consumer may independently admit a second operational catalog for the same generation.

VM, fiber, executor, product-step, session, player, root/replay, bundle, restore, and hot-swap entry points accept only handles issued by that aggregate. Restore and replay deserialize raw data, correlate declared generation/root assertions, re-admit all catalogs and nested values, then activate. They never deserialize admitted roots or capabilities.

Hot swap constructs and validates a complete replacement aggregate off to the side. Publication is one atomic step. Failure leaves the previous active aggregate and all runtime state untouched.

## 8. Failure atomicity and work accounting

All duplicate checks, canonical sort bounds, catalog cardinality limits, declaration closure checks, digest derivations, assertion comparisons, pair correlation, and exact checked-type validation complete before publication. Errors carry a deterministic role/path and typed expected/actual fields. No failure path partially inserts a catalog, activates AWBC, mutates dialogue state, or registers a producer capability.

Canonicalization must use existing bounded work-accounting facilities. Never hash directly from unbounded caller iteration. Enforce item-count, encoded-byte, nesting, and total-work limits before or during canonical encoding, with deterministic limit errors.

## 9. Version policy

Arcweft-owned schema, ABI, codec, digest grammar, and protocol versions remain `1`. This correction does not introduce a v2 reader. Unknown versions fail with typed errors. There is exactly one v1 canonical grammar and no legacy fallback.

## 10. Deletion rule

After all exact callers migrate, delete every public/raw constructor, digest/root-to-admitted conversion, admitted-type Serde implementation, direct raw plan/AWBC execution route, producer-selected operational digest, descriptorless CharacterDialogue wrapper, extension trait/side table duplicating enum behavior, and fallback/dual reader. A deprecation-only state is not final acceptance.

## 11. Closure statement

`OPEN_QUESTIONS=0`. Implementation choices left to ordinary Rust mechanics may not alter the authority graph, canonical bytes, role closure, capability scope, failure atomicity, or deletion requirements fixed here.
