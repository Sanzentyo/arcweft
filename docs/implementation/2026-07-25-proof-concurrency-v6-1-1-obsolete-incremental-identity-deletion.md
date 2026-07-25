# Proof-concurrency v6.1.1 obsolete incremental identity deletion

## Status

`IMPLEMENTED_VALIDATED_WITH_INHERITED_PROOF_GATE`

This is a deletion-only preparatory cut for the Proof-concurrency v6.1.1
public syntax authority switch. It does not publish the attached syntax API or
claim completion of Proof Stage 3.

## Package intake

The repository-retained inputs were reverified before implementation:

| Package | SHA-256 | Manifest result | Status |
|---|---|---|---|
| `arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip` | `1B7DE5F2C10A5B29D67C72011E4272DF9A76AF8907FD21FE162DE54809FC69EF` | all 19 non-self payload hashes match; the manifest self row is the specified 64-zero placeholder | `READY_FOR_IMPLEMENTATION`, no open questions |
| `arcweft-proof-concurrency-v6.1.1.2-retained-global-identity-declaration-grammar-reconciliation-final-contract.zip` | `0E30A91FA2F7A288E9A12D8AFC7356525604CBDC907D659CD97311207D26A68E` | all 17 non-self payload hashes match; the manifest self row is the specified 64-zero placeholder | `READY_FOR_IMPLEMENTATION`, no open questions |

The retained-global package narrowly overrides the base package's retained
declaration inventory. Current `AGENTS.md` and current production facts remain
higher-precedence implementation constraints. No additional Proof design
request is required.

## Deleted authority

The incremental parser previously maintained two independent identity systems
for every accepted transaction:

1. a coarse flat-Rowan `incremental::SyntaxNodeId` plus
   `SyntaxIdentityMap`; and
2. the qualified database/lineage/snapshot attachment identity required by the
   final contract.

Workspace production had no consumer of the coarse public ID or map. This cut
therefore deletes:

- coarse `incremental::SyntaxNodeId` and `SyntaxIdentityMap`;
- `ParsedSource::identities()` and its duplicate map field;
- the duplicate `NodeAllocator` and flat-Rowan allocation/reconciliation pass;
- the old `ShapeNode` hierarchy and its line-parent punctuation helper;
- compile-fail fixtures that permanently named the removed provisional ID;
- tests whose only evidence duplicated the qualified grammar reconciliation
  matrix.

`SyntaxDatabase` now commits and rolls back only the existing qualified
attachment identity state. Exact/one-over node allocation, stale/foreign
snapshot rejection, reordering, copying, parent movement, recovery roles, and
attachment-failure rollback remain tested against that authority.

## Deliberately retained boundary

The disconnected `expr/dialogue_application` modules are not an obsolete
Dialogue carrier. They are the accepted AW-AH-009.4.2 private Cut 2 substrate
for checked bracket/colon source surfaces and will be connected to the final
typed expression arena. This cut keeps them and replaces only their coarse-ID
test dependency with qualified attached IDs.

The actual obsolete production Dialogue path remains frozen until
AW-AH-009.4.2/.3, sema, and runtime-plan can switch together. At that switch,
`SpeakerLineSurface`, string `ContentCall`, `HirDialogue`, and their consumers
must be deleted in the same workspace-compiling series.

## Public-switch dependency order

The full attached `ParsedSource` switch cannot delete the only live Source,
resource, or Dialogue reader before its final owner exists. Accordingly,
deletion-driven migration means replacement and deletion occur in the same
authority cut; it does not authorize a temporary production capability gap.

The remaining order is therefore:

1. finish safe deletion-only cleanup and private Proof HIR substrate;
2. provide the returned-contract final owners needed by the public syntax/HIR
   cohort, including Source-to-Stream elimination, public `res`, attached
   Dialogue/RichText syntax, and HIR-owned content payloads;
3. atomically publish attached syntax and arena HIR/project authority while
   deleting `TypedSyntaxTree`, detached fragment readers, linked/cloned HIR,
   old Source/Dialogue owners, and all parallel readers;
4. complete runtime assertion identity, AWBC/bundle/cache/save/replay codecs,
   and diagnostic projection without a dual reader.

This is an implementation-order reconciliation of already returned contracts,
not a new API decision.

## Validation

Completed:

```text
cargo test -p arcweft-lang-syntax incremental::database::tests -- --nocapture
  PASS: 34 passed, 0 failed
cargo test -p arcweft-lang-syntax dialogue_application -- --nocapture
  PASS: 6 passed, 0 failed
cargo test -p arcweft-lang-syntax --test public_api
  PASS: 1 passed, 0 failed; 6 trybuild cases passed
cargo test -p arcweft-lang-syntax --all-features
  PASS: 470 unit tests and all integration/doc tests
cargo check -p arcweft-lang-syntax --all-targets --all-features
  PASS
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
  PASS
cargo check --workspace --all-targets --all-features
  PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings
  PASS
```

`just test-workspace` ran to completion and retained the two pre-existing
capability fixtures already recorded by the AW-AH-009.3 completion boundary:

```text
FAIL (inherited Proof gate only):
  spec_should_pass_check_fixtures_pass_after_refactor
    tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw
  spec_should_pass_run_fixtures_pass_after_refactor
    tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw
```

These fixtures require the capability-owned `FsError` facts that are connected
by the attached syntax/HIR Proof switch. This cut does not touch their parser,
HIR, sema, CLI, or runtime paths. Restoring the deleted coarse identity reader
would not provide those facts and is not an acceptable workaround. All earlier
workspace tests and trybuild suites in the same run passed.

Tier 2 is not applicable to this cut: no runtime, render, Agent, MCP, capture,
or corresponding public contract changes.

## Structural audit

The audit used parent revision `b305c698b22a01b30f1d7e68be6d925e6e3a2875`
and working change `zwvzyumtpmsxqupuztkvzxwmpnvvwozx`.

```text
cargo +nightly -Zscript tools/structure-audit.rs --root .
files scanned: 3660
Rust files: 1935
Rust physical LOC: 907859
package manifests: 94
violations: 0 error(s), 146 warning(s)
```

Changed production files are all below the 1,200-LOC warning threshold:

| Path | Bytes | Physical LOC | Responsibility |
|---|---:|---:|---|
| `src/cst/punctuation.rs` | 23,094 | 731 | shared CST punctuation scanning after obsolete parent-transition deletion |
| `src/expr/dialogue_application/indentation.rs` | 20,430 | 620 | accepted AW-AH-009.4.2 indentation substrate using qualified IDs |
| `src/expr/dialogue_application/surface.rs` | 27,307 | 774 | accepted checked source surfaces using qualified IDs |
| `src/incremental/database.rs` | 17,811 | 555 | one incremental transaction owner |
| `src/incremental/reconcile.rs` | 11,422 | 330 | qualified grammar reconciliation only |
| `src/incremental/shape.rs` | 9,234 | 298 | qualified grammar shapes only |
| `src/incremental/transaction.rs` | 9,165 | 287 | qualified identity allocation and atomic staging |
| `src/incremental.rs` | 295 | 13 | incremental facade |

The changed `database_tests.rs` is a 58,172-byte, 1,611-LOC test module and is
below the 2,500-LOC integration-test warning threshold. No dependency, feature,
crate boundary, or root re-export was added. The public re-export surface only
shrinks by deleting the provisional ID/map authority.
