# Cargo workspace dependency centralization

## Status and scope

The manifest rewrite, cold compilation, workspace tests, Tier 2 validation,
and structural audit are complete for the settled integrated cut.

This cut makes the root `[workspace.dependencies]` table the single location
authority for dependencies between Arcweft workspace crates. It changes the
root `Cargo.toml` and 78 member manifests. It does not change crate ownership,
dependency direction, enabled capabilities, or the runtime contract.

The audited workspace contains:

- 92 workspace members;
- 425 internal dependency edges;
- 389 direct relative-path declarations converted to `workspace = true` in
  77 member manifests;
- zero remaining member-to-member relative-path declarations.

The remaining changed member manifest centralizes its external `num-traits`
declaration rather than replacing an internal path. Eight member declarations
now inherit the one root `num-traits` specification.

## Rationale

A workspace member previously repeated declarations such as
`path = "../arcweft-core"` even though the root workspace already owns member
discovery and dependency resolution. Those local paths duplicated location
policy across the graph and made it possible for two members to describe the
same Arcweft crate differently.

The final ownership rule is:

- the root manifest owns each workspace crate's `path`;
- a member consumes another workspace crate with `workspace = true`;
- the consuming member may retain only its own `optional`, `features`, and
  `default-features` policy;
- a direct relative path is allowed only for a documented non-member or
  standalone fixture that cannot inherit this workspace.

This is centralization, not feature unification. In particular, consumers that
previously enabled default features still do so, and consumers that previously
disabled them still do so. Root defaults and member-local overrides were chosen
so the effective dependency edge is unchanged.

External dependencies are not mechanically forced into the root table when
their existing local specification represents a distinct consumer contract.
The local pins for `bincode`, `reedline`, `taplo`, and `oxiz-core` are retained.
They are not workspace-member location declarations and are therefore outside
the internal-path replacement rule.

## Acceptance criteria

- Every Arcweft workspace member that is consumed by another member has one
  root `[workspace.dependencies]` path specification.
- Every member-to-member dependency inherits that specification through
  `workspace = true`.
- No `path = "../arcweft-*"` dependency remains in a workspace member.
- The effective `optional`, `features`, and `default-features` policy of all
  425 internal dependency edges is unchanged.
- The eight repeated `num-traits` declarations inherit one root version and
  feature specification.
- Deliberate consumer-local external pins remain local.
- A clean Cargo metadata load and cold workspace compilation succeed after
  removing all prior build artifacts.

## Validation evidence

The manifest inventory and before/after edge comparison reported:

```text
workspace members:                         92
internal dependency edges:                425
relative internal declarations converted: 389
member manifests containing conversions:   77
relative internal path declarations left:   0
optional-policy mismatches:                 0
feature-set mismatches:                     0
default-feature mismatches:                 0
num-traits declarations centralized:         8
```

The requested clean completed successfully:

```text
cargo clean
Removed 169036 files, 252.1GiB total
```

The repository-local `target/` directory was absent immediately after the
clean. A later validation command may recreate it; that does not weaken the
clean-boundary evidence above.

The cold-build and manifest gates completed successfully:

```text
cargo metadata --no-deps --format-version 1                   PASS (92 packages / 92 members)
cargo check --workspace --all-targets --all-features          PASS (3m 32s cold)
cargo clippy --workspace --all-targets --all-features -- -D warnings
                                                               PASS (2m 12s)
cargo fmt --all -- --check                                    PASS
git diff --check                                               PASS
member-to-member relative internal path scan                   PASS (0 matches)
```

The wider integrated gates also completed successfully:

```text
just test-workspace                                            PASS
just test-tier2                                                PASS (MCP stdio 22/22)
cargo +nightly -Zscript tools/structure-audit.rs --root .      PASS
  files scanned: 3492
  Rust files: 1818
  Rust physical LOC: 844870
  package manifests: 94
  violations: 0 error(s), 137 warning(s)
```

## Remaining maintenance rule

If a future member introduces another direct internal path, convert it to root
ownership immediately or document the concrete standalone-fixture reason that
prevents workspace inheritance.

## Design deviations

None. The rewrite preserves the resolved dependency graph and per-consumer
feature policy. It removes duplicated manifest location data without adding a
compatibility alias, dual dependency path, or alternate workspace model.
