# Seq-04.5 Deferred Runtime-Plan / Bytecode / Link Reuse Design

## Runtime-Plan Unit Reuse

Runtime-plan reuse remains design-only in this cut. A sound persistent runtime-plan unit needs all of the following identities before it can be implementation-ready:

1. A stable unit descriptor independent of transient `CompiledProjectModule` layout.
2. A deterministic runtime IR schema whose digest is defined before lowering side effects or target packaging.
3. Explicit lowering-option, target-profile, adapter-environment, runtime ABI, and dependency-body digests in the query key.
4. A validation contract proving rebuilt runtime-plan bytes and reused plan bytes produce the same bundle-visible output.

The current repository stores runtime-plan bytes as a project build artifact. That remains useful for whole-build caching, but it is not yet a safe persistent compiler query unit.

## Bytecode Unit Reuse

Bytecode-unit reuse is deferred. AWBC reuse needs a stable bytecode schema version, codegen backend identity, target feature digest, runtime ABI digest, and deterministic validation that decoded bytecode is semantically equivalent to a rebuilt unit. Current `.awbo` `BytecodeUnitObject` shape only records `module`, `awbc_digest`, and payload bytes, which is insufficient for broad read-through claims.

## Link-Plan Reuse

Link-plan reuse is safe only when all input unit identities are stable. The current build path stores snapshot and bundle artifacts and preserves content roots, but link-plan persistent query reuse would need:

- stable entrypoint descriptors;
- stable ordered unit digest roots;
- link-option and bundle-format digest roots;
- validation that reused link plans produce the same AWFB content root as rebuilt link plans.

Until runtime and bytecode units are stable, link-plan persistent query reuse remains a conservative miss.

## Typecheck And Linked-HIR Reuse

Typecheck and linked-HIR reuse remain conservative. Interface summary hits may be recorded as evidence, but module-aware sema is required before typecheck can be safely skipped or reconstructed from summaries. Direct serialization of `HirModule`, linked HIR, `CompiledProjectModule`, or transient compiler internals remains outside the design.
