# Ordered implementation plan

The plan is one ABI-2/codec-8/save-2 feature branch on `main`, delivered in small
compiling cuts. It must not ship a provisional flat or dual-format intermediate.
Production implementation begins only after this contract package is accepted.

## Cut 0 — intake and final baseline

1. Verify the package SHA-256 and `MANIFEST.sha256`.
2. Record the package path/hash and baseline commit in
   `docs/implementation/2026-07-22-lang-01-3-1-2-2-curried-external-stream-argument-projection.md`.
3. Re-resolve `main`, reread root/nested `AGENTS.md`, and compare intervening
   changes against the evidence paths in this package.
4. Keep the whole feature in the same integrated Lang-01.3.1.2.1 ABI-2 cut; do
   not commit a codec-8 reader with a flat argument shape.

## Cut 1 — core identities, signatures, products, and owners

Suggested responsibility layout:

```text
crates/arcweft-core/src/callable.rs
crates/arcweft-core/src/callable/coordinate.rs
crates/arcweft-core/src/callable/signature.rs
crates/arcweft-core/src/callable/arguments.rs
crates/arcweft-core/src/stream/external.rs
```

1. Add checked runtime coordinate/count newtypes and inherent constructors.
2. Add group-aware external Stream signature types and invariant validation.
3. Add argument dispositions, checked values, prefix/full product validation,
   canonical join, and typed errors.
4. Change the existing function-value owner to the closed `Closure` /
   `ExternalStreamPartial` enum; update behavior through inherent methods on that
   owner.
5. Integrate ownership classification with the parent affine value model.
6. Add direct unit tests for all signature/product invariants and value ownership.

No sema, syntax, host, filesystem, network, or adapter dependency enters core.

## Cut 2 — one accepted-sema-to-RuntimePlan projection

Owners:

```text
crates/arcweft-lang-sema/src/callable/...
crates/arcweft-compiler/src/... external Stream lowering context
crates/arcweft-core/src/... RuntimePlan data
```

1. Expose only any missing discoverable accessors/iterators as inherent behavior
   on the owning sema fact/schema types.
2. In the compiler lowering context, consume the accepted `CallTargetFacts`,
   selected schema, and resolver-produced slot coordinates.
3. Checked-convert each sema index to the core counterpart once.
4. Produce nested signature metadata, authored-evaluation order, and canonical
   slot plans in one traversal under existing query/work budgets.
5. Hash the signature and default plans from typed accepted data.
6. Reject any missing accepted coordinate/default/type evidence as a compiler
   invariant error; do not recover from source text or call the resolver again.
7. Keep `arcweft-runtime-plan` production dependencies free of sema.

Tests cover imports/re-exports, aliases, external binding paths, two/three groups,
all passing modes, empty groups, defaults, rest, and accepted-HIR revision/cache
identity.

## Cut 3 — structured runtime application and atomic open

1. Add structured group-application frame/cursor ownership.
2. Implement authored expression evaluation in source order and defaults in
   coordinate order without replay.
3. Implement non-final product join and partial commit with zero Stream requests.
4. Implement final full-product validation and a prepared open transaction.
5. Make instance-ID allocation, Opening state insertion, handle result, and request
   append one atomic non-fallible commit after all checks.
6. Extend `RuntimeStep` request batches with the parent typed Stream open/close
   request owner; remove final-group-only/Source request fields in the parent cut.
7. Test failure non-mutation through public runtime state/request observations,
   not source inspection.

## Cut 4 — one AWBC ABI 2 / codec 8 replacement

Owners:

```text
crates/arcweft-core/src/awbc/schema.rs and responsibility submodules
crates/arcweft-core/src/awbc/codec/...
crates/arcweft-core/src/awbc/verify/...
crates/arcweft-core/src/awbc/vm.rs and execution submodules
crates/arcweft-runtime-plan/src/awbc_lower/...
crates/arcweft-runtime-codegen/...
```

1. Change ABI/codec constants to 2/8 in the same commit that changes all tables.
2. Add callable signature/group/parameter tables and the sole Stream definition
   table in the exact order from `AWBC_AND_WIRE.md`.
3. Add runtime type tags 21/22, constant tag 18, and opcodes `0x27`/`0x28`.
4. Delete Source tables/opcodes/fiber state and old flat external Stream operands
   as required by the parent cut.
5. Update canonical string visitation/remapping and program/table fingerprints.
6. Add decode budgets before every new allocation.
7. Implement verifier structure/type/default/group/operand checks.
8. Implement VM and compiled-region parity through the shared `FiberState` and
   `RuntimeFunctionValue` owner.
9. Reject codec 7 and every removed shape; do not add a migration reader.

Focused acceptance includes encode/decode/re-encode parity, tampered bytes, budget
failures, verifier errors, VM/product/compiled parity, and exact opcode/tag tests.

## Cut 5 — native, Web, and Agent host boundary

1. Serialize `RuntimeStreamOpenRequest` directly through one shared typed serde
   shape with strict unknown/duplicate-field rejection.
2. Preserve the exact decimal-string wide-integer policy.
3. Ensure every adapter forwards the complete canonical product without grouping,
   name lookup, flattening, or endpoint DTOs.
4. Decode provider events through the parent typed Stream event owner and enforce
   generation/type/sequence validation before mutation.
5. Add equal-byte fixtures for native, Web, and Agent plus duplicate/unknown/tamper
   failures.

Core remains Sans I/O; adapters perform provider acquisition and transport.

## Cut 6 — bundle, save/restore, and hot reload

1. Include signature/group/product layout in bundle and host-ABI fingerprints.
2. Change save schema to 2 atomically with the new function-value snapshot shape.
3. Traverse partial captures in runtime-value snapshot validation.
4. Include partial generation pins in the parent required-generation set and add
   the three typed blockers specified in the contract.
5. Validate restore in the exact order specified, into temporary state.
6. Extend swap signature comparison with every group/parameter/default/type fact.
7. Keep active partials pinned to their original generation; never translate or
   rebind captures.
8. Add safe-point save/restore, stale generation, corrupt product, affine token,
   and code-compatible/code-generational/restart tests.

## Cut 7 — deletion, structural audit, and broad validation

1. Remove every flat final-group projection and stale direct test.
2. Replace useful old invariants with direct typed/codec/behavior tests.
3. Confirm dependency direction with Cargo metadata.
4. Measure changed files and responsibilities with the canonical structure audit;
   split responsibility modules when thresholds require it.
5. Run the exact final validation set:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
just test-tier2
cargo metadata --no-deps --format-version 1
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

`just test-tier2` is required because this public multi-crate contract affects
runtime and Agent paths. Record every command, commit, exact failure, retry reason,
and structural measurement in the implementation note.

## Compile-clean dependency order

```text
core callable/signature/product/value
-> compiler projection + RuntimePlan data
-> structured runtime
-> AWBC schema/codec/verifier/VM/codegen
-> RuntimeStep + native/Web/Agent adapters
-> bundle/save/restore/swap
-> deletion + broad validation
```

A later cut may depend on an earlier one. No earlier crate gains a dependency on a
later layer.
