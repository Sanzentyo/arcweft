# AW-AH-009.2.1.3 shared query budget — atomic sema/LSP cut

## Status and basis

The sema inventory/core query and the LSP request/cache/adaptation integration
are implemented together in Jujutsu change `pmnrkxrv`. The source package is
`arcweft-aw-ah-009.2.1.3-shared-query-budget-verification-reconciliation-final-contract.zip`
with SHA-256
`2dc2c2ee3c4425029639349273ace0b9eb9b323432be2426d67d133c2fdee52b`.

The public sema signatures require `&mut CharacterDefinitionRequestBudget` and
both production LSP call sites now pass the request's sole mutable budget. The
atomic sema/LSP cut is therefore independently compiling: no old-signature
overload, per-call replacement budget, dual cache, or compatibility path is
present. Cut 9's external compile/dependency cases and exact copied contract
matrix remain follow-up evidence; this note does not claim those package rows.

## Implemented contract

### Cut 2 — charged inventory

- `CharacterDefinitionIndex::member_candidates` streams the existing indexed
  look, part, and variant slices in that fixed order. It creates no staging
  vector, alternate index, display parse, or compatibility path.
- `collect_character_references` requires the shared mutable request budget.
  Environment/index world and revision integrity, typed-tree/document text,
  every checker judgment, every eligible expression reparse, and every parser
  diagnostic overlap are charged at their normative I/P boundaries.
- The existing project-symbol resolver remains the only owner/alias authority.
  Successful and wrong-kind targets are charged once. Unknown targets have no
  project-candidate charge. Deterministic ambiguous targets are charged and
  checked against the existing 256-candidate limit before any issue-payload
  clone.
- Expected-nominal selection charges every examined judgment. Exact typed
  member lookup, including a miss, is charged once, and fallback candidates are
  streamed and charged individually.
- Every nonresource failure charges its envelope. Vector payload admission is
  bounded, and each retained candidate is charged immediately before its
  clone. A charge failure therefore has precedence over the semantic issue.
- The former local `work` counter, `charge_inventory_work`, and collect-all
  member fallback are removed.

### Cut 3 — charged core query

- `query_character_definition` requires the same shared mutable budget and
  preserves the existing resolved, not-applicable, unresolved, stale,
  exhausted, and integrity result categories.
- World, revision, and document comparisons; checked cursor/source-length
  admission; selected and unselected fact scans; declaration copy attempts;
  and owned-document lookups are charged in query order.
- Declaration sources are streamed in their canonical index order. Each source
  is charged, checked-counted, and rejected at one-over before clone; each
  admitted clone is followed by its charged exact owned-document lookup. No
  partial declaration result escapes a failure.
- Span widths and all newly introduced counts/conversions use checked
  arithmetic. Malformed ranges map to a typed integrity result after error
  admission rather than saturating.

### Cut 4 — one production-backed fixture

- The reusable manifest, source, registration-fact, and registrar builders were
  moved from `registration/tests.rs` into the crate's `#[cfg(test)]`
  `test_support::character_project` boundary. They were not copied and there is
  no second or hand-written registrar.
- `CharacterProjectFixture` owns durable source/project/registration values and
  one `RegisteredSemanticWorld` produced by `CharacterRegistrar`. Its `collect`
  method keeps parse, document-bound HIR lowering, linked-project type checking,
  inventory collection, and their borrows in one stack frame.
- Focused pipeline tests exercise canonical, compact, qualified, and imported
  alias owner spellings; same-target aliases; unknown and wrong-kind owners;
  look/part/variant members; expected and untyped member resolution; wrong
  nominal family and owning part; deterministic member permutations; the
  256/257 member boundary; cursor classification and ambiguity; stale
  identities; missing declarations/documents; the 64/65 declaration boundary;
  error-admission precedence; and core budget exhaustion.

The real pipeline exposed one general checker defect: an entity reference whose
surface spelling does not encode an entity kind discarded an available expected
`Ref<_>` type. The checker now forwards the expected type to entity-reference
checking and uses it only when normal symbol and syntax-kind lookup cannot type
the reference. This is a generic expected-type rule and does not special-case a
character name or alias.

### Cuts 5–6 — receipt caches and one LSP budget owner

- The ordinary LSP entry creates exactly one production budget and delegates
  to a budget-taking runner. Preparation, inventory, core query, target
  adaptation, link construction, and final stamping share that same mutable
  owner.
- Accepted-generation semantic caches pair each immutable `Arc` inventory or
  query result with its ordered work receipt. Cache-key equality is charged
  before lookup; a hit clones only the private entry `Arc`, replays the receipt,
  and exposes the cached value only after replay succeeds.
- Inventory misses checkpoint immediately before parser admission and record
  parse, HIR lowering, checker, and sema collection. Query misses checkpoint
  immediately before the core query and cache only resolved, not-applicable,
  and unresolved results. Query results move into `Arc` without clone-on-insert.
- Target adaptation remains live and uncached. Origin conversion is hoisted
  once, every target/range/link operation is charged, and links remain local
  until final accepted-generation validation succeeds.
- LSP-local stale and integrity failures admit their envelope through inherent
  request-error constructors. Sema failures already admitted by the shared
  pipeline are converted without a second charge.

### Cuts 7–8 — current adapters and focused accepted-generation evidence

- The existing open-overlay and exact-file target adapters are charged from the
  same request budget. Every current exact URI/document/file identity check is
  preceded by `IdentityCheck`; missing or unreadable current targets admit their
  failure before the all-or-empty response is discarded.
- Final validation charges accepted `Arc`/generation, document version,
  profile, overlay set, and origin URI remapping comparisons before response
  publication.
- The production temporary-project test compares the complete opaque work
  receipt for cache miss and hit, not only a scalar total. It also precharges a
  production budget to one-over, proves resource precedence with no response,
  and directly verifies semantic-entry cache clearing. The seven pre-existing
  target/profile/overlay integration tests remain green.

## Intentional package wording deviation

`REQUEST_BUDGET_API.md` sketches a private `from_resource` helper for mapping a
resource error into `CharacterReferenceInventoryError`. The implementation
uses the equivalent standard
`From<CharacterDefinitionResourceError> for CharacterReferenceInventoryError`
conversion instead. The conversion is infallible, context-free, owned by the
target Arcweft type, and matches the repository's current conversion discipline.
It preserves the exact limit/arithmetic payload and introduces no wrapper,
extension trait, compatibility shim, or second error enum.

`CACHE_POLICY.md` separately says that cache entries are private to the cache
module while sketching a getter that returns an entry to its caller. The
implementation keeps the entries genuinely private: the cache owner performs
the charged key comparison, clones the entry `Arc`, replays its receipt, and
only then returns the contained value `Arc`. Insert methods accept the complete
immutable value/receipt pair and construct the private entry in the owning
module. This enforces the required replay-before-observation ordering by API
rather than exposing an entry type to sibling modules.

## Remaining integration boundary

AW-AH-009.2.1.2's dedicated source-diagnostic adapter is not present on the
current base. This cut charges the existing open/disk adapter directly and does
not recreate 009.2.1.2. When that independent production reconciliation lands,
its inherent target-adaptation method must accept this same mutable budget (or
leave its exact comparisons visible here); it must not introduce an
adapter-local counter or compatibility overload. Cut 9's compile/dependency
tests and exact matrix fixture also remain separate follow-up evidence.

## Structural result

The canonical integrated report is
[`structure-audits/aw-ah-009-2-1-3-shared-query-budget/`](structure-audits/aw-ah-009-2-1-3-shared-query-budget/).
It scanned 3,103 files, including 1,553 Rust files and 711,191 Rust physical
LOC, and reported zero error-level violations and 128 repository-wide warnings.
The sema crate's normal dependency fan-in/fan-out remains `8/10`; no dependency
or Cargo feature changed.

| Path | Bytes | Physical LOC | Class | Major responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-lang-sema/src/character_definition.rs` | 36,102 | 1,034 | production | inventory model, charged collection, owner/member resolution |
| `crates/arcweft-lang-sema/src/character_definition/query.rs` | 15,497 | 380 | production | charged cursor classification and declaration query |
| `crates/arcweft-lang-sema/src/character_definition/request_budget.rs` | 6,134 | 198 | production | request budget, checkpoints, receipts, replay |
| `crates/arcweft-lang-sema/src/registration/source_index.rs` | 35,661 | 1,046 | production | immutable character definition/source indexes plus same-crate tamper fixtures |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 94,899 | 2,486 | production | expression type checking and expected-type propagation |
| `crates/arcweft-lang-sema/src/test_support/character_project.rs` | 14,244 | 407 | unit-test support | production-backed character project fixture |
| `crates/arcweft-lang-sema/src/character_definition/tests/pipeline.rs` | 32,544 | 979 | unit test | sema charge/order and parser-to-query evidence |
| `crates/arcweft-lang-sema/src/registration/tests.rs` | 79,901 | 2,176 | unit test | registration invariants after fixture extraction |
| `crates/arcweft-lsp/src/features/character_definition.rs` | 23,721 | 603 | production | one-budget request orchestration, live target adaptation, final stamp |
| `crates/arcweft-lsp/src/profiles/caches.rs` | 7,563 | 243 | production | generation-local semantic entries, charged lookup, ordered replay |
| `crates/arcweft-lsp/src/profiles/state.rs` | 27,924 | 817 | production | accepted environment and typed cache delegation |
| `crates/arcweft-lsp/src/session/character_definition_tests.rs` | 20,043 | 537 | unit test | production profile/adapter/cache receipt integration |

`checker/expr.rs` remains an existing warning-level hotspot and is 14 physical
lines below the 2,500-LOC error threshold. This cut changes nine physical lines
there and adds no new responsibility: the change belongs to its existing entity
reference expression typing path. The character-definition root remains below
the 1,200-LOC production warning threshold because the core query is now a
separate responsibility module. The extracted registration test file remains
below the 2,500-LOC integration/unit-test warning threshold.

## Validation

All Rust commands use `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-sema --lib --all-features character_definition::request_budget::tests:: -- --nocapture
cargo test -p arcweft-lang-sema character_definition::tests::pipeline --all-features -- --nocapture
cargo test -p arcweft-lang-sema --lib --all-features registration::tests:: -- --nocapture
cargo test -p arcweft-lsp character_definition --lib -- --nocapture
cargo check -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features
cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-lsp --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-2-1-3-shared-query-budget
jj diff --git | git apply --numstat --whitespace=error-all
```

The request-budget suite passes 26 of 26 tests, the real sema pipeline suite
passes 39 of 39 tests, the registration suite passes 55 of 55 tests, and the
focused LSP suite passes 8 of 8 tests. Both affected crates check; sema and LSP
Clippy pass with warnings denied; formatting and diff-whitespace validation
pass; and the structural audit reports no error-level violation. The exact
package matrix and workspace-wide validation remain outside this atomic cut and
are not reported as completed.
