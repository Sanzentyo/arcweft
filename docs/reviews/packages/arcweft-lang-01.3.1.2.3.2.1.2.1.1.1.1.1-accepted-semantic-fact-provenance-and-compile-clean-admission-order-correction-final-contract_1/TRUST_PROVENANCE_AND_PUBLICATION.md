# Trust, provenance, and publication

## Guarantee matrix

| Layer/token | Who may obtain it | What it guarantees | What it does not guarantee |
|---|---|---|---|
| Raw `RuntimePlan` / `AwbcProgram` | checked builder or v1 decoder | structural object passed builder/decode checks | independent semantic acceptance, origin, operational publication |
| `AdmittedRuntimeGeneration` | any public safe Rust caller to `try_issue` | canonical fact structure, internally derived roots/digests, immutable parent identity | that facts came from Arcweft compiler or a trusted signer |
| `CompilerRuntimeEvidence` | official `arcweft-compiler` path only | exact final-HIR snapshots, owner transcript, sema projection and produced artifact digests | persisted-byte authenticity or trusted-key signature |
| `VerifiedRuntimeBundleProduct` under `TrustedIntegrator` | bundle verifier | canonical bytes/sections/facts/admissions/same-parent product; host elected to trust integrator | external identity/authentication |
| `VerifiedRuntimeBundleProduct` under `RequireTrustedEd25519` | full bundle verifier with trusted keys | all above plus signature authentication by configured trusted key | semantic intent beyond the signed compiler/integrator claim |
| `PublishedRuntimeGeneration` | runtime-driver publication only | verified product, policy, host capability binding, operational checks completed | permission to replace an unrelated generation without swap policy |

## Layer-correct issuance

`arcweft-core` knows no compiler/bundle/driver types. `arcweft-bundle` owns
only the version-1 byte/container verifier and never depends on compiler.
`arcweft-compiler`, which already depends on bundle, returns its own evidence
plus the core admitted product; its bridge encodes the same in-memory v1 bundle
transcript and invokes `arcweft-bundle::verify_runtime_bundle_product`.
`arcweft-runtime-driver` depends only on bundle/core/save/host layers and accepts
only the bundle-owned verified product. The driver never names a compiler token.

The fact-section decoder returns `DecodedRuntimeGenerationFactSection`, which
is quarantine data. Only the full bundle verifier may combine decoded facts,
raw plan, raw AWBC, external catalogs/resources, signature policy, and limits to
produce `VerifiedRuntimeBundleProduct`.

## Operational publication checks

Publication consumes the verified product and applies this precedence:

1. evidence policy result and trusted-key requirement;
2. exact generation/plan/AWBC parent and plan-key correlation;
3. required catalog/resource section availability and canonical digest equality;
4. host capability/effect/opaque-producer bindings (bindings are adapters only);
5. target VM/JIT/runtime-codegen support and operational-type support;
6. publication limits/budgets;
7. atomic registration/visibility.

No root map or catalog digest is accepted from `RuntimeHostBindings`.

## Direct compiler execution

There is one route: compiler-owned `verify_compiler_runtime_product` encodes
`CompilerRuntimeProduct` to the same in-memory version-1 bundle section
transcript, then invokes the public bundle section/canonical/admission verifier. Unsigned direct execution
requires explicit `TrustedIntegrator`; signed-only policy rejects it unless the
in-memory product carries a verifiable trusted signature produced by the
configured build/signing path.

## Hot swap

`PublishedRuntimeGeneration::verify_hot_swap_candidate` invokes the bundle
verifier with `self.product().generation().clone()` through
`verify_runtime_bundle_product_for_parent`. That verifier still decodes and
canonically validates the candidate fact section and requires exact equality
with the supplied parent before admitting its plan/AWBC against that same Arc;
it cannot amend the parent. `prepare_hot_swap` then validates the next verified
product into a private candidate without mutating the current generation and
requires `Arc::ptr_eq` before state migration. A different parent requires full
republish plus an explicit migration transaction; it is never accepted by this
same-parent hot-swap API. `commit` rechecks the current parent and publication
epoch, then atomically replaces the published product and migrated state.

## Restore/replay

The fixed order is:

1. verify bundle and trust policy;
2. issue/admit generation facts;
3. decode/build/admit plan;
4. decode/build/admit AWBC;
5. pair same-parent product;
6. publish host bindings/policy;
7. decode snapshot header and require exact generation identity;
8. ask the lower `arcweft-save` codec for raw envelope/value/event payloads;
9. runtime-driver admits each decoded value through the product-issued checked
   context/domain;
10. replay events only after all referenced typed sites/domains resolve.

`arcweft-save` never imports runtime-driver and never issues semantic authority.
Tampered raw declarations, decoded sidecar facts, or a runtime value's physical
shape cannot fill missing semantic evidence. JIT, AOT, and accelerator setup
occurs after publication and receives only `&PublishedRuntimeGeneration`.
