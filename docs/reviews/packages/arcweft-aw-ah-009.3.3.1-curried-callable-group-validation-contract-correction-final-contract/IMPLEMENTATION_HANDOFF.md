\
# IMPLEMENTATION HANDOFF

## Implementation status

No production implementation is included in this archive. The following order is decision-complete and may be executed without design invention.

## Ordered cuts

### Cut 0 — contract and test ownership

1. Adopt `FINAL_CONTRACT.md` and `TEST_MATRIX.md` as the corrected curried-group clauses.
2. Remove the old matrix expectation that a schema-less constructor proves group existence.
3. Record that all unrelated AW-AH-009.3.3 clauses remain unchanged.

This archive completes Cut 0.

### Cut 1A — minimal identity error correction

Files:

- `crates/arcweft-lang-sema/src/callable/error.rs`
- `crates/arcweft-lang-sema/src/callable/identity.rs`
- `crates/arcweft-lang-sema/src/callable/tests.rs`

Actions:

1. Replace `CallableIdentityError::MissingGroup` with the exact `InvalidCurriedGroup { base, group }` variant.
2. Keep `CurriedCallableId::try_new` signature, fields, accessors, and check ordering.
3. Return `InvalidCurriedGroup` only for group zero.
4. Update direct constructor tests, including both prohibited wrapper kinds.
5. Fix all compiler-reported references directly; add no alias or deprecated variant.

Compiling checkpoint:

```bash
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema --all-targets \
  curried_id_
CARGO_INCREMENTAL=0 cargo check -p arcweft-lang-sema --all-targets
```

### Cut 1B — one canonical resolved representation

Files:

- `crates/arcweft-lang-sema/src/callable/resolver.rs`
- `crates/arcweft-lang-sema/src/callable/tests.rs`

Actions in `ResolvedCallable::try_new`:

1. Change the private instantiation match so a curried success is canonical only when the ID is `CallableCandidateId::Curried` and embedded base/group exactly match `CallableInstantiation::Curried`.
2. Delete the existing base-ID-plus-Curried success arm.
3. Preserve the existing candidate-count, origin/authority, canonical-instantiation, and duplicate equivalent-source checks and their `InvalidResolvedCallable` precedence.
4. After those existing structural checks pass, call `schema.group(group)` for the canonical curried pair.
5. Return `InvalidCallGroup { candidate: base.clone(), group }` when absent.
6. Preserve the full input `Arc<CallableSignatureSchema>` in the result.
7. Leave all non-curried origin, authority, equivalent-source, and limit behavior unchanged.

Reference implementation shape, placed around the existing checks rather than ahead of them:

```rust
if equivalent_sources.len().saturating_add(1) > limits.max_candidates_per_call()
    || !origin_matches(&id, &origin, authority)
    || !instantiation_matches(&id, &instantiation)
{
    return Err(ResolveCallError::InvalidResolvedCallable);
}

let mut ids = std::collections::HashSet::new();
ids.insert(id.clone());
if equivalent_sources
    .iter()
    .any(|source| !ids.insert(source.id().clone()))
{
    return Err(ResolveCallError::InvalidResolvedCallable);
}

if let (
    CallableCandidateId::Curried(curried),
    CallableInstantiation::Curried { base, group },
) = (&id, &instantiation)
{
    debug_assert_eq!(curried.base(), base);
    debug_assert_eq!(curried.next_group(), *group);
    if schema.group(*group).is_none() {
        return Err(ResolveCallError::InvalidCallGroup {
            candidate: base.clone(),
            group: *group,
        });
    }
}
```

The exact local organization may follow existing style, but behavior and error precedence are fixed. Do not extract a public helper, context object, extension trait, or second product.

Compiling checkpoint:

```bash
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema --all-targets \
  resolved_curried_
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
```

### Cut 2+ — shared resolver connection by existing migration order

When project and environment catalog publication and the shared resolver are connected:

1. perform AW-AH-009.3.2 accepted request/world/source validation first;
2. obtain the base record and full schema through the one shared resolver;
3. construct the structural curried ID;
4. map constructor group-zero failure inline to `InvalidCallGroup`;
5. construct the canonical curried ID/instantiation pair;
6. call `ResolvedCallable::try_new`;
7. return `ResolveCallOutcome::Rejected(error)` unchanged on failure;
8. never retry the legacy route or another provider after this typed failure.

Add project, standard, adapter, positive multi-group, one-over, and corrupt-world integration tests as each provider family becomes reachable through the shared route.

### Final migration deletion cut

Delete old checker-local curried group-existence logic only after all deletion conditions in `FINAL_CONTRACT.md` are met. Delete the old route and duplicate check together; do not leave a compatibility fallback.

## Exact error mapping table

| Input at shared resolver | Result |
|---|---|
| requested group zero | `ResolveCallError::InvalidCallGroup { candidate: base, group: 0 }` |
| requested nonzero group absent from schema | same error with exact nonzero group |
| one-over group | same error with `group == schema.groups().len()` |
| base is already `Curried` or `DataLast` | `ResolveCallError::InvalidResolvedCallable` |
| canonical pair has mismatched embedded base | `InvalidResolvedCallable` |
| canonical pair has mismatched embedded group | `InvalidResolvedCallable` |
| base ID paired directly with Curried instantiation | `InvalidResolvedCallable` |
| canonical pair and group exists | successful `ResolvedCallable` |

## Validation order

After each compiling cut, use focused tests. At the reviewable migration cut, run:

```bash
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo check -p arcweft-lang-sema --all-targets
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema --all-targets callable
just test-workspace
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Use the repository's current test-execution policy if command names or scope have changed at implementation time. The public error contract change triggers a structural/API audit, but no source gate.

## Required implementation report

The implementation report SHALL state:

- exact base revision and final revision;
- files changed;
- tests and commands actually run;
- whether each provider integration row passed;
- confirmation that the old base-ID success arm is gone by behavioral tests, not source scanning;
- confirmation that no compatibility path, second resolver, source gate, CSS path, or Takumi path was added;
- remaining TODOs, which must be zero for this correction before claiming completion.
