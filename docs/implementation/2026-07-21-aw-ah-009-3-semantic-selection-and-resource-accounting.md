# AW-AH-009.3 ordinary-call semantic selection and resource accounting

Date: 2026-07-21

## Status and completion boundary

The implementation-ready semantic-selection and resource-accounting slice is
implemented and integrally validated for parser-owned parenthesized
`Expr::Call` surfaces. The implementation began in Jujutsu change `nxyxxulm`;
the final integrated change and push are recorded at the repository cut that
contains this note.

This does **not** close the complete historical AW-AH-009.3 sequence. In
particular, the old S14-S16 dialogue speaker/content-call carriers cannot
provide the ordered authored arguments, per-argument ranges, recovery facts,
or focused expression identity required by the signature query. Those
dialogue-specific clauses were superseded by the CharacterDialogue direction
and are owned by the separate AW-AH-009.4.2 production slice described by
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
- a public, immutable checker-owned call-target fact model and report read API;
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

The finalized AW-AH-009.3.3 immutable fact boundary is now public without
publishing mutation authority. `CallTargetFacts`, `CallTargetFact`,
`CheckedCallArgumentFact`, `CheckedCallArgumentSlotFact`, and
`CallTargetFactError` expose read-only accessors, while their constructors and
fields remain owned by sema. `TypeCheckReport::call_target_facts` reads a fact
by `TypeExpressionId`, and the focused report exposes
`focused_call_target_facts`. Registered whole-module analysis records all
accepted call facts; standalone checking keeps collection disabled unless a
caller explicitly enters the focused query path.

The public focused read is backed by the production
`analyze_registered_project_types_for_focused_call` entry. It accepts an
exact registered `SourceSpan`, constructs production-bounded non-cancelled
resolver control internally, and returns the ordinary public
`TypeCheckReport`. Mutable recorders and caller-injected work/cancellation
remain crate-private for the interactive signature-query path and tests.

`CallTargetFact::Rejected` is the truthful no-viable-overload state. It retains
the checked candidate facts needed for deterministic candidate-zero UI focus
without misreporting a rejected singleton or rejected candidate set as
`Ambiguous`. `Ambiguous` is reserved for multiple equally viable candidates.
This is a correction to the provisional closed variant list, not a
compatibility extension or dual reader.

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

Parser-owned argument syntax also owns the R07 comma boundary: from the start
offset of a between-argument or trailing comma, focus belongs to the following
slot. The semantic query consumes that typed slot result; it does not recover
the boundary by scanning source text.

The scanner traverses ordinary expressions inside dialogue interpolation,
dialogue option values, and line plans. It does not reinterpret the dialogue
container itself as an ordinary call. Every dialogue tag range and `goto`
target range is explicitly `UnsupportedSurface`; unknown and non-callable
ordinary callees become the unit public outcomes
`SignatureNotApplicable::UnknownCallee` and
`SignatureNotApplicable::NonCallableCallee`. Internally, missing facts retain
their `UnknownCallKind`, and non-callable facts retain typed source and type
evidence.

## Argument diagnostics and projection hardening

The checked argument facts now retain an exact parser-owned name span in
addition to the complete argument span. Deterministic projection therefore
binds these stable codes to the authored token or insertion point that owns the
failure:

- A05 `DuplicateArgument` uses the duplicate name and related first-name span;
- A08 `UnknownNamedArgument` uses the exact unknown name span;
- A10 `UnsupportedSpread` stops subsequent positional mapping for the rejected
  spread shape while subsequent expressions are still semantically checked;
- A11 `TooManyPositionalArguments` owns the extra argument span;
- A12 `MissingArgument` uses the zero-width argument-list insertion span;
- A14 `ParameterAlreadyBound` relates the later positional argument to the
  earlier named binding.

When one of these specific argument failures explains a rejected candidate,
projection does not add a duplicate generic `NoViableSignature` diagnostic.
The retained `Rejected` target fact still exposes the deterministic candidate
set and candidate-zero UI focus.

L16 projection now performs checked UTF-16 accumulation all the way from a
wide intermediate count to the LSP `u32` label offsets and returns
`LabelOffsetOverflow` without publishing a partial result. L18 cancellation is
polled inside resolver iteration and produces a terminal typed cancellation;
the surrounding fact transaction publishes no partial candidate set.

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
`CallTargetFact::Rejected` retains the rejected candidates and candidate zero
owns the deterministic checked argument mapping/UI focus. A rejected singleton
is replayed only to retain its specific diagnostic instead of replacing it
with a duplicate generic error.

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
Compact `NumericBracketSeq` literals now do the same: parser construction binds
each literal to its exact absolute range, including under a non-zero parse
base; HIR lowering preserves that typed syntax object; and synthetic
construction has no authored range rather than fabricating one. The LSP bridge
always emits `Some(active_signature)` and attaches the single top-level
parameter coordinate only to that active signature.

## Character and callable-family parity

Direct accepted-HIR tests cover canonical, compact, qualified, and alias
Character references and prove that they resolve to the same nominal
`CharacterId`, type, and canonical label. The same evidence covers structural
Look, Variant, and Part paths, overload selection, public fact/query parity,
label-only edits that preserve nominal identity, exact source spans in the
presence of same-name comments, and the absence of Rust-symbol-suffix fallback.

The historical C09 corrupt-world premise is not constructible through the
accepted registration API: invalid alias punctuation is rejected, and an
owner collision rejects the registration transaction atomically. Production
does not add an impossible fallback branch or a compatibility world solely to
manufacture that state.

The final surface matrix is covered directly for S01-S13, S17-S18,
S20-S27, and S30. Existing semantic-query tests directly retain S19's inline
tag rejection, S28's second curried group, and S29's non-call `goto` surface.
S14-S16 are superseded by AW-AH-009.4.2 rather than restored as old
speaker/content-call carriers.

The old C12 premise is likewise not constructible in the accepted final API.
Dynamic presentation specialization owns `CharacterId -> CharacterLook`; it
does not expose a part-bearing dynamic owner. The removed ContentCall route was
the only proposed carrier for that premise. An unknown-part resolver or
synthetic accepted callable is therefore not reintroduced merely to preserve
an obsolete test shape; part-bearing CharacterDialogue semantics remain owned
by AW-AH-009.4.2. The unused provisional `CharacterOwnerResolution` and its
unproduced owner-unavailable diagnostic variants were removed with that
decision instead of being retained as a compatibility-shaped public API.

The shared callable catalog also closes the previously split Agent prelude by
including `observe` as the typed `AgentIntrinsicSignatureId::Observe` member
with its `Result<Observation, AgentError>` result and `agent.observe` effect.
Committed call facts retain their lexical `CallableDeclarationId` privately.
Entry binding, which owns callable roles, compares a selected Agent-family call
with the exact ordinary function selected by an Agent entry. A call from a
selected controller (including its lexically owned closures) is accepted; a
call from an unselected helper, flow, or other owner is rejected at the exact
call span with `sema.entry.unbound_agent_intrinsic`. This policy does not infer
roles from callee spellings, effect strings, attributes, or function bodies,
and it leaves signature lookup role-neutral.

## Known gaps and non-goals

The ordinary-call fact, diagnostic, public read, Character-family parity, and
compact numeric-coordinate gaps described by the earlier draft are closed in
the current implementation. Nested speculative evaluation remains governed by
the transaction/work-accounting contract: only one focused result is
published, and no acceptance requirement is inferred from the number of
internal expected-type probes.

The remaining surface boundary is **CharacterDialogue application syntax**.
The superseded speaker/content-call carriers are not repaired or accepted as
ordinary calls; AW-AH-009.4.2 owns their replacement typed argument-list and
expression-identity path. That work is outside this ordinary-call cut.

Final integrated workspace, Tier 2, and current-checkout structural validation
all passed. The workspace run exposed shared Stage/Fx/expected-enum-shorthand
resolver defects; those were corrected at the registered callable schema and
typed-environment owners rather than hidden in fixtures or compatibility paths.

This cut intentionally does not redesign CharacterDialogue, restore removed
syntax, add source gates, or introduce compatibility aliases, deprecated
fields, duplicate resolvers, or migration shims.

## Structural audit

The final audit was regenerated from the settled integrated checkout:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write \
  docs/implementation/structure-audits/aw-ah-009-3-semantic-selection-2026-07-21
```

Result:

```text
files scanned: 3491
Rust files: 1818
Rust physical LOC: 844870
package manifests: 94
violations: 0 error(s), 137 warning(s)
```

The generated reports under
`docs/implementation/structure-audits/aw-ah-009-3-semantic-selection-2026-07-21/`
are therefore current-checkout evidence. The detailed measurements below remain
the decomposition baseline for the semantic-selection files; the generated
CSV reports are the authority for exact whole-checkout values.

Changed Rust measurements from the prior baseline audit:

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

Focused syntax, HIR, sema public-fact, Character parity, resolver-cancellation,
and LSP overflow tests passed while developing the individual changes. The
settled semantic-surface follow-up passed the seven-test surface matrix, the
four-test public-facts suite, the four-test Character parity suite, 44
signature unit tests, 52 sema entry tests, 98 compiler tests, the focused Agent
role regression, and Agent identity/schema parity tests.

The settled integration then passed every required gate:

```text
cargo fmt --all -- --check
git diff --check
cargo test -p arcweft-lang-syntax --lib
cargo test -p arcweft-lang-hir --lib
cargo test -p arcweft-lang-sema --lib --no-fail-fast
cargo test -p arcweft-lang-sema --test call_target_facts_public_api
cargo test -p arcweft-lang-sema --test character_signature_fact_parity
cargo test -p arcweft-lang-sema --test call_surface_signature_matrix
cargo test -p arcweft-lsp --lib features::signature
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
just test-tier2
cargo +nightly -Zscript tools/structure-audit.rs --root . --write \
  docs/implementation/structure-audits/aw-ah-009-3-semantic-selection-2026-07-21
```

The final results were:

```text
cargo fmt --all -- --check                                    PASS
git diff --check                                               PASS
cargo check --workspace --all-targets --all-features          PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings
                                                               PASS
just test-workspace                                            PASS
just test-tier2                                                PASS (180.6s)
  MCP stdio                                                    PASS (22/22)
  Agent observe/native capture/visual golden groups           PASS
structure audit                                               PASS (0 errors, 137 warnings)
```

The 137 structural warnings are ownership-review warnings rather than waived
errors. No changed file crossed the configured 2,500-LOC production error
threshold, and no source gate or compatibility surface was introduced to make
the gates pass.
