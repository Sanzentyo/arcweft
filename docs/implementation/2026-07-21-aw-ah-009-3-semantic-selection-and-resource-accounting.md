# AW-AH-009.3 ordinary-call semantic selection and resource accounting

Date: 2026-07-21

## Status and completion boundary

The implementation-ready semantic-selection and resource-accounting slice is
complete for parser-owned parenthesized `Expr::Call` surfaces on Jujutsu change
`nxyxxulm`, based on change `xrwkuxsl` / Git commit `888a0c09`. The change is
not pushed and owns no bookmark or branch.

This does **not** close the complete historical AW-AH-009.3 sequence. In
particular, the old S14-S16 dialogue speaker/content-call carriers cannot
provide the ordered authored arguments, per-argument ranges, recovery facts,
or focused expression identity required by the signature query. Those
dialogue-specific clauses were superseded by the CharacterDialogue direction
and are design-gated by
`docs/reviews/requests/2026-07-20-aw-ah-009.4.2-dialogue-content-application-syntax-hir-ownership-production-reconciliation.md`.
The production evidence and supersession boundary are also recorded in
`docs/implementation/2026-07-20-aw-ah-009-3-1-call-surface-production.md`.
No compatibility dialogue carrier, source reparse, forged range, or second
expression arena was added.

The completed slice provides:

- a pure, bounded typed-HIR surface scan before semantic checking;
- one focused checker entry after a surface has been selected;
- transactional overload probing, deterministic ranking, and atomic selected
  replay;
- separate callable-internal and outer-query work reports;
- public semantic help and typed not-applicable/error outcomes;
- native LSP projection of the committed active coordinates.

## Contract precedence and reconciliation

The later AW-AH-009.3.3 callable-catalog contract controls overload selection,
checker-owned facts, and deterministic UI focus where it conflicts with the
older AW-AH-009.3 sketch. Consequently:

- `SemanticSignatureHelp::active_signature` is a required, in-range index;
  ambiguous and no-viable results use deterministic candidate zero only as UI
  focus, while a selected overload retains its actual index;
- active-parameter state is one optional top-level coordinate derived from
  retained checker facts, not a field repeated on every signature;
- the AW-AH-009.3.1 surface carrier contributes the exact argument span,
  expression identity, current/next group, and recovery state;
- the outer query adds `SignatureQueryWorkReport` alongside the callable-owned
  `SignatureWorkReport`.

The result is an in-memory composite of the later contracts, not a
compatibility reader for the provisional AW-AH-009.3 shape.

One earlier AW-AH-009.3.3 acceptance item is still incomplete: the finalized
contract requires the immutable target-fact model and `TypeCheckReport` fact
read APIs to be public. Current production keeps `CallTargetFacts`,
`CallTargetFact`, checked argument/slot facts, `CallTargetFactError`, and the
focused report behind crate-owned boundaries. Publishing or reshaping that
read model is a separate callable-fact API cut; this note does not claim it.

`SignatureFamilySupport::NativeFacts` describes native resolver/checker fact
ownership for a `CallableFamily`. It does not assert that every historical
source spelling is a reachable signature-help surface. In particular,
dialogue tags and the superseded speaker/content-call carriers are not ordinary
call surfaces.

## Public query limits and reports

`SignatureQueryLimits` is independent from `CallableLimits`. Its production
bounds are inclusive:

| Resource | Production maximum |
| --- | ---: |
| visited call-expression candidate surfaces | 4,096 |
| projected overloads | 64 |
| parameters per projected signature | 128 |
| cursor-containing parenthesized call lists | 64 |
| parser recovery nodes | 512 |
| accepted source bytes | 8,388,608 |
| projected diagnostics | 32 |
| total outer-query work units | 262,144 |

The source-byte bound is checked against the accepted document before a work
meter or checker is constructed. Independently bounded operations return
`SignatureLimitExceeded` with the exact kind, observed value, and maximum.
Node visits, arguments, resolver operations, argument bindings, specificity
checks, parameters, and diagnostic considerations share the total work bound.
Arithmetic overflow remains a separate typed error carrying the exact
`SignatureWorkKind`. Checked charges validate both the component and total
before mutating either counter.

`SignatureQueryWorkReport` exposes exact outer-query counts:

- search: node visits, visited call expressions, cursor-containing call
  lists, authored arguments, and recovery nodes;
- resolution: focused resolver operations, argument bindings, and specificity
  checks;
- projection: overloads, parameters, and diagnostic considerations;
- the checked total of all outer-query operations.

`SignatureWorkReport` separately exposes callable-internal resolver,
argument-mapping, and type-check units plus the retained recovery-node and
diagnostic counts. Its `total_work` is the checked sum of the three work-unit
components.

Parameter counts reset for every projected signature. Exactly 32 diagnostics
need no marker. At 33, deterministic projection retains the first 31 plus one
typed truncation marker and reports `omitted_diagnostics == 2`.

## Surface scan and focused semantic ownership

The query first walks the accepted typed HIR belonging to the exact accepted
source identity. It does not parse source text or infer a call from a rendered
label. The walk is deliberately complete rather than cursor-local:

- every visited node consumes node work;
- every visited `Expr::Call`, including a callback-shaped call that later maps
  to `UnsupportedSurface`, consumes candidate-call and authored-argument work;
- parser-owned call recovery nodes consume recovery work;
- only a parenthesized argument list containing the cursor consumes the
  `nested_calls` counter and participates in focus selection;
- the most deeply containing call is selected deterministically; overlapping
  equal ranges fail with a typed ambiguity error.

Sibling calls therefore consume search work, but they do not consume the
caller-owned focused resolver/probe work. After the pure scan, the query makes
one call to the focused checker entry point. Ordinary whole-module checking
retains its separate resettable work authority and has no deadline authority.

The scanner traverses ordinary expressions inside dialogue interpolation,
dialogue option values, and line plans. It does not reinterpret the dialogue
container itself as an ordinary call. Every dialogue tag range and `goto`
target range is explicitly `UnsupportedSurface`; unknown and non-callable
ordinary callees become the unit public outcomes
`SignatureNotApplicable::UnknownCallee` and
`SignatureNotApplicable::NonCallableCallee`. Internally, missing facts retain
their `UnknownCallKind`, and non-callable facts retain typed source and type
evidence.

## Resolver and selection accounting

The established `CallableLimits` continue to own catalog and resolver-fact
construction. Relevant production bounds remain 32 overloads per catalog key,
256 candidates per resolved call, 128 parameters per callable, 32 nested
calls, 256 recovery nodes, 128 retained callable diagnostics, 4,096 resolver
work units, and 8 MiB of source. The outer query neither widens these bounds
nor converts an internal failure into a truncated success.

All non-catalog `ResolvedCallable::try_new` paths charge callable-internal
resolver work immediately before candidate construction. For `N` overloads,
the `N - 1` pair comparisons also charge callable-internal resolver work.
Those comparisons do not consume outer `SpecificityChecks`. The outer counter
is charged once for each candidate/authored-argument probe; selected replay
does not charge it a second time.

Every candidate, including a singleton, is transactionally probed. Ranking is
applied in this exact order:

1. clean over recovered; rejected is non-viable;
2. fewer hard type-check errors;
3. more exact expected/inferred matches;
4. fewer unchecked or recursively open slots;
5. fewer omitted parameters;
6. Standard authority over Adapter authority only when every earlier
   dimension is equal.

Compatible-match and rest-binding counts are retained specificity metrics but
are deliberately not comparator stages. Same-authority equality remains
ambiguous. An ambiguity publishes only tied viable candidates; a rejected
candidate cannot survive beside a better viable tie. With no viable overload,
candidate zero owns the deterministic checked argument mapping/UI focus. A
rejected singleton is replayed only to retain its specific diagnostic instead
of replacing it with a duplicate generic error.

`TypeKind::has_open_components` is an inherent recursive rule owned by the
semantic type. A typed entity reference is open only when it has a payload and
that payload is open; an unparameterized `Ref(None)` remains closed and can be
an exact overload match.

## Transaction and active-coordinate invariants

Speculative checking uses a nested mutation journal and compact checkpoints,
not cloned `TypeChecker` instances. Rollback covers diagnostics, statistics,
judgments, lowering evidence, local scopes, presentation defaults, lifetime
and borrow state, closure captures, higher-order invocation inventories,
focused facts, and effect-collector mutations. Terminal cancellation,
deadline, resolver-work, and signature-accounting errors survive rollback.

A unique viable winner is replayed through the ordinary registered-candidate
checker. Replay commits only after the terminal-error check; a terminal error
rolls the complete replay back. Direct tests cover rejected closure/effect
state, block-local/presentation/borrow state, nested transactions, and the
positive `EffectCollector` commit path.

`SemanticSignature` contains no active coordinate. `SemanticSignatureHelp`
owns one required active signature and one optional active parameter. Its
constructor verifies that the active signature exists and that an active
parameter belongs to the current group of that signature. Fixed expression
spreads retain per-element source ranges, allowing exact cursor-to-slot focus.
The LSP bridge always emits `Some(active_signature)` and attaches the single
top-level parameter coordinate only to that active signature.

## Known gaps and non-goals

The following prevent a claim that all historical AW-AH-009.3 acceptance
criteria are closed:

1. **CharacterDialogue source surface.** The superseded speaker/content-call
   syntax/HIR lacks the final argument-list and expression-identity carrier.
   AW-AH-009.4.2 owns the replacement design and production order.
2. **Public callable-fact read API.** The finalized AW-AH-009.3.3 public fact
   visibility/read boundary remains crate-private, as described above.
3. **Nested focused resolution count.** The outer query invokes the focused
   checker once, and transactions preserve one published focused result, but a
   target nested under an enclosing overload can still be evaluated again for
   different outer speculative expected types. Exact single resolver
   invocation for that nested target is not yet directly proved.
4. **Family-specific validator parity.** The focused registered route shares
   generic schema checking across many families. This cut does not claim
   complete direct parity evidence for every legacy family-specific
   value-shape validator.
5. **Compact numeric spread coordinates.** Ordinary fixed `Expr::BracketSeq`
   spreads retain exact per-element ranges. The compact
   `NumericBracketSeq` HIR stores integer values without per-element source
   coordinates, so active parameter can be absent when expanded elements map
   to different parameters.

This cut intentionally does not redesign CharacterDialogue, restore removed
syntax, add source gates, or introduce compatibility aliases, deprecated
fields, duplicate resolvers, or migration shims.

## Structural audit

The canonical audit was run from Jujutsu change `nxyxxulm`:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write \
  docs/implementation/structure-audits/aw-ah-009-3-semantic-selection-2026-07-21
```

Result:

```text
files scanned: 3469
Rust files: 1810
Rust physical LOC: 834930
package manifests: 94
violations: 0 error(s), 131 warning(s)
```

Reports are under
`docs/implementation/structure-audits/aw-ah-009-3-semantic-selection-2026-07-21/`.
This cut changes no Cargo dependency or feature edge. `arcweft-lang-sema` has
fan-in 11 and fan-out 12; `arcweft-lsp` has fan-in 1 and fan-out 29.

Changed Rust measurements from the final audit:

| Path | Owner / class | Bytes | LOC | Embedded test LOC | Cut responsibility |
| --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-lang-sema/src/callable/error.rs` | sema production | 20,270 | 575 | 0 | typed query/limit errors |
| `crates/arcweft-lang-sema/src/callable/facts.rs` | sema production | 35,750 | 1,093 | 0 | fact and help invariants |
| `crates/arcweft-lang-sema/src/callable/limits.rs` | sema production | 23,912 | 774 | 0 | two work-report layers |
| `crates/arcweft-lang-sema/src/callable/resolver.rs` | sema production | 65,540 | 1,959 | 0 | candidate construction charges/control |
| `crates/arcweft-lang-sema/src/callable/tests.rs` | sema unit test | 72,797 | 2,167 | 0 | public-model/limit invariants |
| `crates/arcweft-lang-sema/src/callable.rs` | sema facade | 5,206 | 98 | 0 | intentional callable exports |
| `crates/arcweft-lang-sema/src/checker/assertion.rs` | sema production | 3,400 | 90 | 0 | journal-aware assertion mutation |
| `crates/arcweft-lang-sema/src/checker/call_target_facts.rs` | sema production | 17,429 | 530 | 57 | focused work/fact recorder |
| `crates/arcweft-lang-sema/src/checker/expr/registered_call/facts.rs` | sema production | 6,131 | 163 | 0 | checked fact construction |
| `crates/arcweft-lang-sema/src/checker/expr/registered_call/selection.rs` | sema production | 30,942 | 845 | 126 | probe/rank/replay |
| `crates/arcweft-lang-sema/src/checker/expr/registered_call.rs` | sema production | 81,554 | 2,119 | 0 | registered family validation |
| `crates/arcweft-lang-sema/src/checker/lifetime_access.rs` | sema production | 4,038 | 109 | 0 | journal-aware lifetime mutation |
| `crates/arcweft-lang-sema/src/checker/line_plan.rs` | sema production | 20,734 | 550 | 0 | journal-aware lifetime release |
| `crates/arcweft-lang-sema/src/checker/module.rs` | sema production | 93,106 | 2,477 | 0 | focused checker entry |
| `crates/arcweft-lang-sema/src/checker/presentation.rs` | sema production | 23,401 | 591 | 0 | journal-aware defaults |
| `crates/arcweft-lang-sema/src/checker/registered_candidate_transaction.rs` | sema production | 18,522 | 484 | 0 | nested checkpoint/rollback/commit |
| `crates/arcweft-lang-sema/src/checker/registered_candidate_transaction_tests.rs` | sema unit test | 6,304 | 170 | 0 | transaction evidence |
| `crates/arcweft-lang-sema/src/checker/stmt.rs` | sema production | 31,347 | 813 | 0 | journal-aware scoped locals |
| `crates/arcweft-lang-sema/src/checker.rs` | sema production | 71,608 | 2,015 | 0 | checker/focused state |
| `crates/arcweft-lang-sema/src/effect_collector.rs` | sema production | 11,046 | 321 | 0 | nested effect journal |
| `crates/arcweft-lang-sema/src/effect_model.rs` | sema production | 11,563 | 454 | 50 | reversible effect inventories |
| `crates/arcweft-lang-sema/src/signature/project/tests.rs` | sema unit test | 2,899 | 84 | 0 | diagnostic projection bound |
| `crates/arcweft-lang-sema/src/signature/project.rs` | sema production | 13,371 | 378 | 0 | bounded public projection |
| `crates/arcweft-lang-sema/src/signature/surface.rs` | sema production | 36,206 | 988 | 0 | exhaustive typed-HIR scan |
| `crates/arcweft-lang-sema/src/signature/tests.rs` | sema unit test | 64,000 | 2,011 | 0 | end-to-end query matrix |
| `crates/arcweft-lang-sema/src/signature.rs` | sema production | 15,821 | 448 | 0 | query orchestration/outcomes |
| `crates/arcweft-lang-sema/src/types/openness.rs` | sema production | 3,574 | 98 | 0 | recursive openness rule |
| `crates/arcweft-lang-sema/src/types.rs` | sema production | 38,218 | 1,113 | 0 | semantic type model |
| `crates/arcweft-lsp/src/features/signature.rs` | LSP production | 8,788 | 253 | 73 | active-coordinate projection |
| `crates/arcweft-lsp/src/requests/signature.rs` | LSP production | 33,260 | 918 | 116 | typed protocol error mapping |

The exact growth/decomposition review found:

- `callable/limits.rs` grew from 281 to 774 LOC but remains the cohesive
  inclusive-limit/meter/report owner in the preferred 300-800 range;
- new `registered_call/selection.rs` is 845 LOC including 126 embedded test
  lines, leaving a 719-line production responsibility;
- new `signature/surface.rs` is a 988-line exhaustive typed-HIR visitor. It is
  above the preferred ordinary-module range but below the warning threshold;
  splitting syntax families would obscure the single traversal/accounting
  boundary, so it remains cohesive;
- new `registered_candidate_transaction.rs` is a 484-line transaction owner;
- `signature/tests.rs` grew from 882 to 2,011 LOC but remains below the 2,500
  integration-test warning threshold;
- the new openness rule was moved to `types/openness.rs`, leaving `types.rs` at
  1,113 LOC rather than crossing the 1,200-LOC production warning threshold.

The changed files still reported at warning size are existing cohesive
hotspots: `callable/resolver.rs` (1,959 LOC), `checker.rs` (2,015),
`checker/expr/registered_call.rs` (2,119), and `checker/module.rs` (2,477).
None crossed the 2,500-LOC error threshold. The largest non-generated
production files in the checkout are core `engine/eval/calls.rs` (89,488
bytes/2,481 LOC), sema `checker/module.rs` (93,106/2,477), core `value.rs`
(83,366/2,465), CLI `toolchain_profile.rs` (75,712/2,463), bundle
`container.rs` (78,366/2,393), and runtime-plan `expr.rs` (84,530/2,382).

## Validation

Passing gates at the time of this note:

```text
cargo fmt --all -- --check
  passed

cargo clippy -p arcweft-lang-sema -p arcweft-lsp \
  --all-targets --all-features -- -D warnings
  passed

cargo test -p arcweft-lang-sema --lib --no-fail-fast
  832 passed; 0 failed

cargo test -p arcweft-lsp signature --lib --no-fail-fast
  20 passed; 0 failed; 159 filtered out

cargo +nightly -Zscript tools/structure-audit.rs --root . --write \
  docs/implementation/structure-audits/aw-ah-009-3-semantic-selection-2026-07-21
  0 errors; 131 repository-wide warnings
```

Workspace-wide validation is blocked by a checkout prerequisite rather than a
Rust diagnostic from this cut:

```text
cargo check --workspace --all-targets --all-features
  failed: web/assets/noto-sans-jp-vf.ttf is absent for arcweft-glyphon and
  arcweft-render-wgpu compile-time test assets

cargo check --workspace --lib --bins --all-features
  failed: the same absent asset is embedded by arcweft-player-scene

cargo clippy --workspace --all-targets --all-features -- -D warnings
  failed: the same absent asset blocks arcweft-glyphon and
  arcweft-render-wgpu test compilation
```

The first all-target check attempt timed out after 124 seconds without a
compiler result; the rerun reached the concrete missing-asset failure above.
No Rust compiler or Clippy diagnostic preceded the asset error. `just
test-workspace` was not run after this deterministic prerequisite failure
because its workspace test compilation requires the same asset. Focused
all-target/all-feature Clippy and both changed-crate test routes passed.

Tier 2 is not required: although the cut spans sema and LSP and changes public
semantic results, it does not affect a runtime, render, Agent, MCP, or capture
path. Native visual suites and ignored Tier 2 suites were not run.
