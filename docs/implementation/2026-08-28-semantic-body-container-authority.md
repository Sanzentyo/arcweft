# Semantic body-container authority

Date: 2026-08-28

Status: implemented

## Established implementation

- `HirBodyProjection` is the single HIR-owned carrier for body kind and exact
  source-ordered `HirBodyChildEdge` rows. Declaration roots, item roots,
  direct expression bodies, expression-owned bodies, and statement bodies all
  use that carrier; the former `Body | Expression` root split was deleted.
- Direct expression projections cover Thread, Block, ComputationBlock,
  NamedBlock, and Loop. Expression-owned projections retain every Await body
  and only actual Choice Thread bodies (option Select, timeout, cancel, and
  on-select), including authored empty bodies.
- Statement body projection is exhaustive over all 35 statement families and
  all 13 body roles. Match expression arms use the same typed Expression body
  projection as predicate/proof and declaration/item expression roots.
- Conceptual Choice bodies, option bodies, line plans, Init slices, and
  Start/Together groups are not fabricated as generic body rows. Their owners
  retain their existing nested typed path algebras.
- Recovery Await branches, missing unsafe bodies, recovered callable bodies,
  and Thread error items are terminal projection errors. They do not publish a
  successful semantic body row.
- `HirSemanticBodyOwner` has a private representation and checked
  expression-owned constructor. Its exhaustive raw-ID-free
  `HirSemanticBodyOwnerRole` projection prevents future owner families from
  defaulting to an existing checked identity.
- Body rows form a separate typed keyspace. Declaration/item wrappers and
  direct expression bodies may intentionally share one structural path, while
  duplicate typed body owners remain terminal.
- Path-index sealing validates exact root/owner/kind pairing and joins every
  body child to its retained expression or statement path. It does not infer
  membership from a prefix or serialize raw HIR owners.
- `HirSemanticBodyLocator` performs borrowed project-wide lookup against the
  retained root index without a copied project side table.
- `StableCheckedBodyCoordinate` starts with the accepted-rooted checked path,
  then appends closed numeric body-coordinate, owner-family/role, and body-kind
  atoms. Raw expression/statement IDs and a second domain/version string are
  absent.

## Validation

- focused empty-body, Match wrapper, owner/kind, duplicate-owner, wrong-child
  join, equal-path typed-owner, locator, and coordinate issuance tests: passed;
- `cargo test -p arcweft-lang-hir --all-features`: 890 unit tests passed,
  8 ignored, and all 6 integration/API tests passed;
- `cargo test -p arcweft-lang-sema --all-targets --all-features`: 536 unit,
  12 compile-API, and 4 integration tests passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo check -p arcweft-lang-hir --all-targets --all-features`: passed;
- `cargo check -p arcweft-lang-sema --all-targets --all-features`: passed;
- strict HIR Clippy reached only three pre-existing findings (two runtime
  reachability `match_same_arms` findings and one 101-line pattern test); no
  finding named a file changed by this cut;
- strict sema Clippy remains blocked by 95 pre-existing `arcweft-core`
  findings and the same two HIR findings before sema itself is linted.
- `cargo fmt --all --check` and `git diff --check`: passed.

## Structural audit disposition

Audit base: `89f5930ef0659b1cefcb0e4a8881354dd8ede558`. The checkout was dirty
with protected unrelated WIP; measurements below include only this cut's
paths. `just structure-audit` and `just structure-audit-gate` scanned 2,261
files and 95 workspace packages, reported 273 review triggers, and found zero
blocking violations.

| Path | Class | Base -> current | Bytes | Disposition |
| --- | --- | ---: | ---: | --- |
| `crates/arcweft-lang-hir/src/body_edges.rs` | production | 259 -> 565 LOC (+306) | 19,773 | Cohesive owner of the shared body kind/projection/child grammar; expression-owned and statement-specific traversal is already decomposed into their owning modules. |
| `crates/arcweft-lang-hir/src/final_project/semantic_paths.rs` | production | 4,519 -> 5,250 LOC (+731) | 193,737 | Existing project-topology authority. The cut deleted copied Await/Choice/statement AST traversal and moved it to HIR owners. Remaining additions are the single path join, validation, locator, and root-index integration and therefore stay with the topology seal rather than forming a second body index. |
| `crates/arcweft-lang-hir/src/final_project/tests.rs` | test | 3,795 -> 4,043 LOC (+248) | 144,388 | Existing project-topology integration suite; added cases exercise the same topology fixture and do not add production responsibility. |
| `crates/arcweft-lang-sema/src/semantic_coordinate.rs` | production | 1,503 -> 1,619 LOC (+116) | 57,376 | Existing sole checked-coordinate grammar owner; body-coordinate atoms remain centralized with the other canonical path encoders. |
| `crates/arcweft-lang-sema/src/final_analysis/tests.rs` | test | 8,017 -> 8,054 LOC (+37) | 278,031 | Existing end-to-end final-analysis fixture; the added issuer test joins an actual accepted-root catalog and does not embed production logic. |

The other changed Rust files remain below their applicable size triggers. The
dependency direction is unchanged: sema consumes HIR, never the reverse.
Current workspace direct dependency fan-in/fan-out is 11/3 for
`arcweft-lang-hir` and 10/14 for `arcweft-lang-sema`.
