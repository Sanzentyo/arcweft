# Plan record domain shapes and explicit field identities — 2026-09-12

Inspected Git `main == origin/main` at
`238b310bef74c560ee6fd52f98eeb852c64084cf` in the existing checkout.
The working tree contained 174 preserved dirty/deleted files from the ongoing
callable/effect, nominal-schema and Rust ADT work. Only the record-domain
changes described here were selected for this cut, using explicit paths and
hunks. The other changes remain in place.

## Result and authority

The accepted core record layout already distinguishes Unit, Tuple, Record and
Newtype and assigns explicit one-based field IDs. The executable plan domain
previously discarded that shape and retained only a mandatory name/type pair.
It consequently could not express unnamed tuple/newtype fields, and its
consumer regenerated IDs from positions.

`RuntimeNominalRecordDomainSeed` and `RuntimeNominalRecordDomain` now retain
the existing core `RuntimeNominalRecordShape`. Their field rows retain
`RuntimeRecordFieldId`, an optional name and the existing semantic/plan-local
type reference. No second shape enum, nominal catalog or compatibility
constructor was added.

The domain owns admission of its shape and IDs. It delegates name/cardinality
rules to `RuntimeNominalRecordShape::validate_field_names` and requires each
supplied ID to equal its defining-order coordinate. Unit, empty Tuple, empty
Record and Newtype remain distinct. Duplicate, reordered and gapped field IDs
cannot be silently normalized.

The existing aggregate builder still prepares types, locals and all domains
before committing any table. Repeated identical domains coalesce; a different
shape for an existing owner is a conflict. Record construction and pattern
resolution now consume the admitted IDs instead of manufacturing another ID
inventory.

The runtime-plan producer forwards shape, IDs and optional names from the
already correlated `RuntimeNominalRecordLayout`, together with normalized
field types. The zipped inventories are immutable and their equal cardinality
and type/name correspondence are established by the existing
`RuntimeResolvedNominalRecord::try_new` boundary. Current producers and core
construction/flow/pattern fixtures were migrated together.

## Validation performed

All commands below ran against the preserved working tree. Results do not
claim an entirely clean workspace or completion of the structural nominal
convergence series.

Passed:

- `cargo check -p arcweft-core --all-targets --all-features` after migrating
  the exposed fixture calls.
- `cargo test -p arcweft-core --all-features --lib nominal_domains_tests`:
  4 tests passed. They cover every record shape, empty/one/multiple fields,
  source order, duplicate names, wrong shape, swapped/gapped/repeated IDs,
  unchanged type/local/domain counts on failure and conflicting-shape rollback.
- `cargo test -p arcweft-core --all-features`: 498 tests passed, comprising
  465 unit and 33 integration tests; core had 0 doctests. This includes the
  focused four tests, not four additional tests.
- `cargo clippy -p arcweft-core --all-targets --all-features`: passed with
  122 library and 142 test-target warnings (121 duplicates in the latter).
  This is not a warning-free result.
- `cargo fmt -p arcweft-core -p arcweft-runtime-plan`, subsequent core
  formatting, `git diff --check` and `git diff --cached --check`.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --fail-on-blocking`:
  95 workspace packages, 2,317 Rust files, 1,279,828 physical Rust LOC,
  310 review triggers and 0 blocking violations.

Failed / blocked before the relevant downstream tests:

- The initial core all-target check found 14 stale constructor/error references
  in existing tests. Those consumers were migrated; the repeated check and
  subsequent tests passed.
- `cargo check --workspace --all-targets --all-features` and
  `cargo clippy --workspace --all-targets --all-features` failed at
  `arcweft-host-adapter/src/lib.rs:503`: the preserved Rust ADT migration
  removed `AdapterRustType::opaque_producer`, but the host result projector
  still uses that obsolete opaque publication.
- `just test-workspace` failed in its first Cargo command at that same host
  consumer; subsequent CLI recipe commands did not run.
- `just test-doc` failed during dependency compilation at that same consumer.
- `just test-tier2` failed in `test-slow-mcp` during compilation. It exposed
  the host consumer above and the existing missing nominal Variant layouts in
  Dialogue `character_dialogue/schema.rs:83` and runtime-accelerator
  `external.rs:543`. The ignored MCP test and later Tier 2 prerequisites did
  not run.

The policy-selected Just recipes intentionally use their documented default
or native-capture feature sets. Direct core checks/tests/Clippy used the stable
all-feature slice. No explicit Cargo job count was supplied, and Cargo
validation commands ran sequentially.

Logs are retained locally under
`.arcweft-local/validation/2026-09-11-effect-row-formulas/` with the
`nominal-domain-shape-` prefix. The current runtime-plan dependent path and
the preserved C2 sema work have not passed downstream compilation.

## Structural review

The measured working tree includes the preserved changes outside this cut.
Physical LOC includes blank lines; byte sizes are working-tree file sizes.

| Owner | Base LOC | Current LOC | Bytes | Responsibility |
|---|---:|---:|---:|---|
| core `plan/nominal_record_domains.rs` | 262 | 307 | 9,053 | inert rows and atomic domain admission |
| core `plan/construction.rs` | 2,753 | 2,854 | 113,222 | aggregate table transaction and field-reference rewrite |
| core `plan/construction/lower.rs` | 5,185 | 5,196 | 215,372 | expression/pattern lowering in the same builder |
| core `pattern.rs` | 2,933 | 2,922 | 106,897 | existing checked predicates; only a fixture changed in this cut |
| core `tests/flow.rs` | 1,003 | 1,008 | 38,298 | native flow fixture |
| runtime-plan `semantic_facts.rs` | 10,439 | 10,461 | 397,126 | normalized semantic facts and core seed projection |

The new 218-line `plan/construction/nominal_domains_tests.rs` follows the
aggregate admission boundary. Existing construction/lowering/pattern owners
retain their cohesive responsibilities; the small field projection does not
introduce unrelated state or a separate issuer. The runtime-plan normalized
fact owner retains its established layout correlation and calls into the
lower core domain API. Core imports no sema/runtime-plan type, and no I/O,
dependency, feature, facade or persisted schema was introduced.

The large owner files remain review triggers. Their disposition for this cut
is cohesion: the changes use existing transaction, layout and field ownership.
The new behavior tests are separated by that admission responsibility. LOC
reduction is not claimed as an architecture result.

## Remaining work and design decisions

This completes the record-domain metadata propagation and validation boundary.
It does not prove that layout scalars agree with a complete canonical schema
graph, publish new structural ownership success, or complete C1-C6. General
schema-graph admission, recursive live acceptance, Rust compiler projection,
AWBC and program-bound restore remain in the active convergence goal.

The Dialogue/data producer and host-result issues belong to the existing
[nominal producer/schema closure request](../reviews/requests/2026-09-11-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1.1-nominal-variant-layout-producer-and-schema-closure.md).
The host audit found a semantic migration requirement, not just an obsolete
getter: `project_adapter_runtime_type` maps every manifest nominal to
`RuntimeCheckedType::Opaque`, and its Rust-package branch invents the opaque
owner directly from manifest metadata. That path must be replaced by the
selected structural/runtime authority; reintroducing the Rust producer field
or substituting an arbitrary producer would preserve the wrong model.

The Dialogue inspection confirmed that its current schema receives only a
layout scalar. Its selected complete producer/role/generation contract is
still required. This cut does not replace its four policy Variants with tuple
markers or make any new Dialogue codec/admission claim.

No design deviation or contract-version change was selected in this cut.
The full goal remains active; the failures above are repository work and are
not an external blocker.
