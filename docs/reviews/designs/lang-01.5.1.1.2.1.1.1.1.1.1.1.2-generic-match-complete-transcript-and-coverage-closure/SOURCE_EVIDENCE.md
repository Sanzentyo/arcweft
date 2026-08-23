# Source evidence

## Evidence cut

This design is derived from Git `main` at
`9a5d30d25620541c3f2975d31e04e04e3bc9514c`. The returned review archives
are intake evidence only: neither archive contained repository source, and
their external return types, tags, wire shapes, persistence claims, status,
and self-asserted acceptance are not authority for this contract.

The maintained request is
`docs/reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure-correction.md`
(9,780 bytes; SHA-256
`7250743e386ce404664c4c211d109094c9f40031211edc70847688754922aa9c`).
`REQUEST.md` is its byte-identical mirror.

The first-return and redispatch intakes were read at
`docs/implementation/2026-08-23-lang-01-5-1-1-2-1-1-1-1-1-1-1-2-generic-match-complete-return-intake.md`
and
`docs/implementation/2026-08-23-lang-01-5-1-1-2-1-1-1-1-1-1-1-2-generic-match-redispatch-return-intake.md`.
They establish failure/source-unavailability only.

Accepted predecessor evidence was read from
`docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-launch-receipt-keyed-ordinal-and-current-owner/`,
including its final design, Match child edges, ownership matrix, cuts/tests,
and source evidence. The implemented substrate was cross-checked against:

- `docs/implementation/2026-08-22-runtime-convergence-cut-1-match-child-edges.md`;
- `docs/implementation/2026-08-22-runtime-convergence-cut-1a-child-edge-substrate-reclassification.md`;
- `docs/implementation/2026-08-22-runtime-convergence-cut-1b-match-transcript-substrate.md`; and
- `docs/implementation/2026-08-22-runtime-convergence-cut-3-match-producer-admission-safe-subset.md`.

The blocked downstream predecessor at
`docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.3.1-current-runtime-plan-semantic-owner-and-view-predecessor-reconciliation/`
was read only to preserve the direction of its future declaration/body-path
and semantic-identity consumption. Its task-plan seal is deliberately not
imported into this cut.

The repository-aware validator pins these source blobs rather than accepting
source spelling as proof:

| Git path | Blob |
|---|---|
| `crates/arcweft-lang-sema/src/final_analysis/model.rs` | `836295b1c58ce1a08d06a302643b2a265e8b9cd3` |
| `crates/arcweft-lang-sema/src/final_analysis/semantic_transcript.rs` | `91b764e625f9582acc3ea0dcf646951cb42a7cd1` |
| `crates/arcweft-lang-sema/src/final_analysis/match_edges.rs` | `d7426bce4818bba25d3f64aa5e1c5f628a027283` |
| `crates/arcweft-lang-sema/src/final_analysis/match_edges/model.rs` | `055ad0f01e4c29d97bb3f1734c4c9574ffe800e2` |
| `crates/arcweft-lang-sema/src/final_analysis/analyzer/patterns.rs` | `736d2be8bf6521042fc93bca9b592ce0ef81e255` |
| `crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs` | `4d70d5dde6f7920c2af83d268b8dc9d38cbf7282` |
| `crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs` | `a7df1895471661b168db0eddaf900661c42f2625` |
| `crates/arcweft-lang-hir/src/final_project/semantic_paths.rs` | `78a68bc9d8dd8679a6f7d0514111f9bbb046ca98` |
| `crates/arcweft-lang-hir/src/final_project.rs` | `5069669226a22f65b4b4d89654715166e53d7227` |
| `crates/arcweft-lang-hir/src/body_edges.rs` | `e08f25b5ed1c74160b542837151eb33e86e5d6ad` |
| `crates/arcweft-lang-hir/src/expr.rs` | `b9e2c3e9117ba61e5c428064a5b7f0af973adcd5` |
| `crates/arcweft-lang-hir/src/pattern.rs` | `1a7adf00f7caeee6aa517dfe2c7873dff86145bc` |
| `crates/arcweft-lang-hir/src/pattern/child_edges.rs` | `7e3972887daa16b7f2a2b914d0d51a7f637de2c9` |
| `crates/arcweft-lang-hir/src/stmt.rs` | `061c97afb6367413d60ca8bcdb271f578dd7d379` |
| `crates/arcweft-lang-hir/src/stmt/child_edges.rs` | `9b6bf0d0a30aca2296f63a514245cbf237f7401d` |
| `crates/arcweft-lang-hir/src/item.rs` | `67c9ed2d12b00c4d8e42009c3e19f51772d8bfb5` |
| `crates/arcweft-lang-hir/src/item/retained.rs` | `eb7e2a04c3ca8d54b58c153c1b43464de0873b8a` |
| `crates/arcweft-lang-hir/src/symbol/identity.rs` | `7f06f1093c74942b06923ebc4663ce56609c54d1` |
| `crates/arcweft-lang-sema/src/nominal/model.rs` | `b8f6c605d5f471de4fd0deb52592572ca44b91bd` |
| `crates/arcweft-lang-sema/src/callable/checked_catalog.rs` | `593028b11a4197ce085be3c58be3be820c44471f` |
| `crates/arcweft-lang-sema/src/dialogue_view.rs` | `eb55dfa79639b85256fd4b1440bfbbc70ab9467a` |

## Complete current inventories

`CheckedExpressionResolution` has exactly 27 families:
`Structural`, `Literal`, `Value`, `Select`, `Nominal`, `Variant`,
`StageLook`, `Effect`, `Call`, `Await`, `Choice`, `Try`,
`ImplicitCallable`, `ImplicitParameter`, `Pipe`, `PipeLeft`, `ViewCall`,
`ViewCallee`, `StyleValue`, `StyleCallee`, `DialogueLineReference`,
`DialogueLineCoordinate`, `DialogueTextKeyCoordinate`,
`CharacterDialogueFactory`, `CharacterDialogueReconfigure`,
`DialogueApplication`, and `PostfixBracket`.

Its two nested families are also closed here. `CheckedValueResolution` has
exactly 8 families: `Local`, `LineContext`, `CharacterField`,
`ProjectCallable`, `ProjectItem`, `Entry`, `Registered`, and `Constant`.
`CheckedSelectResolution` has exactly 7 families: `Method`, `DialogueView`,
`AgentField`, `ProgressField`, `Field`, `TupleElement`, and `RecordElement`.
`RecordElement` has no source producer at this cut and is therefore a deletion,
not a success arm to preserve.

`CheckedPatternResolution` has exactly 5 families: `Structural`, `Literal`,
`Entity`, `Nominal`, and `Variant`. The HIR pattern inventory has exactly 13
families: `Binding`, `MutableBinding`, `Literal`, `EntityReference`, `Variant`,
`Discard`, `Tuple`, `Record`, `BracketSequence`, `WholeBinding`, `Or`,
`TypedBinding`, and `Error`.

The HIR statement inventory has exactly 35 families: `Assertion`, `Let`,
`Assign`, `LetElse`, `LetChoice`, `LetScope`, `LetActionReceive`, `Return`,
`Out`, `Goto`, `DeferBlock`, `Defer`, `Yield`, `Signal`, `LifetimeSet`, `Wait`,
`On`, `UnsafeLifetime`, `Choice`, `If`, `IfLet`, `Match`, `While`, `WhileLet`,
`For`, `Close`, `Select`, `SourceLocale`, `Scope`, `Include`, `Break`,
`Continue`, `Expression`, `ProofCall`, and `Error`.

The current executable body-edge authority has exactly 5
`HirBodyChildRole` families: `Expression`, `Statement`, `Tail`,
`RecoveryExpression`, and `ThreadItem`. Its nested statement-body authority has
exactly 13 `HirStatementBodyRole` families: `LetElse`, `Defer`, `On`,
`UnsafeLifetime`, `Then`, `Else`, `MatchArm`, `While`, `WhileLet`, `For`,
`SelectBranch`, `SourceLocale`, and `Scope`. Payload-bearing roles retain their
checked arm/branch/ordinal. Recovery roots reject transcript publication.

The HIR expression inventory has exactly 38 families: `Unit`, `Literal`,
`EntityReference`, `LifetimePath`, `Path`, `ShortVariant`, `Placeholder`,
`Tuple`, `BracketSequence`, `NumericBracketSequence`, `ArrayRepeat`, `Call`,
`Select`, `Index`, `Pipe`, `Try`, `Await`, `Thread`, `Choice`, `Range`,
`Record`, `RecordLiteral`, `Binary`, `Borrow`, `Dereference`, `Closure`,
`Unary`, `Block`, `ComputationBlock`, `NamedBlock`, `Loop`, `If`, `IfLet`,
`Match`, `DialogueContentApplication`, `PostfixBracket`, `Error`, and
`ForSynthetic`.

## Executable roots and declaration bridge

Current declaration semantic paths cover function, predicate, proof, flow,
and impl-function bodies plus parameter patterns/defaults. A `View` is already
an accepted `CallableDeclarationKey`, is already present in the checked
callable catalog, and owns source-ordered `HirViewDeclaration::values()`.
Nevertheless `declaration_semantic_paths` explicitly returns `MissingBody` for
`ViewItem`. The correction is one new HIR-owned root role
`ViewValue { ordinal }`; it does not create a Match-site ID or a parallel View
catalog.

The complete Match-bearing declaration owner set is therefore `Function`,
`Predicate`, `Proof`, `Flow`, `TraitImplementation`, `InherentMethod`, and
`View`. `ExternCapability` and `TraitRequirement` have no executable body and
remain `MissingBody`/ineligible.

Expression child edges already cover expression children, but they do not
cover all non-expression roots nested under expressions. Same-cut path closure
must add the following typed HIR body-root roles, preserving source order:

- Await branch pattern and contextual statement body;
- Choice `Let` statement, `For` pattern, Match-arm pattern, OptionFor pattern,
  option `Select` body, option `Let` statement, and lifecycle-plan
  Timeout/Cancel/OnSelect pattern and bodies;
- dialogue line-plan Init/Thread/On/Statement/CancelRule/Error statements and
  Let pattern, recursively through StartGroup and TogetherGroup.

All ordinary statement-owned nested statement, expression, pattern, type, and
local roots continue to come from the exhaustive `HirStatementChildRole`
projection. `Error` expression, pattern, statement, Choice item/field/plan, and
line-plan recovery rows reject admission rather than contributing a digest.

## Result-changing gaps established from source

1. The current transcript writer matches all 27 resolution families but maps
   many of them to `UnsupportedIdentity`; syntactic exhaustiveness is not
   semantic completeness.
2. Character and builtin variant owners retain case names, Entity/ProjectItem
   lacks an accepted semantic ID, and record-pattern fields retain only a child
   ordinal. Exact checked case, entity, and field owner atoms must be produced
   in the same cut.
3. The expression transcript does not encode the exhaustive expression-family
   shape/non-child atoms, nested Match pattern/coverage meaning, or the
   statement bodies owned by Await, Choice, and dialogue line plans.
4. Current coverage is a finite Bool/Variant/Unit atom approximation. It cannot
   decide products, sequences, Or usefulness, non-Boolean literals plus
   `Other`, open entity domains, `Never`, or `Choice`.
5. Current limits mix `u32` and `u64`, use saturating transcript accounting,
   and contain infallible conversion assumptions. The selected contract uses
   checked `u64` counters at every admission edge.
6. `CheckedMatchRef { HirSnapshotId, ExprId }` is already a private,
   non-Serde compiler-local handle. It remains so; its raw fields are neither
   transcript bytes nor persistence identity.
7. Current direct consumers are sema reports/exports/tests. No compiler or
   runtime consumer justifies a persisted generic-Match DTO, external return
   wire, compatibility reader, whole-catalog seal, or version other than `1`.

## Existing accepted authorities reused

- HIR child-edge ordering and semantic-path ownership from the accepted
  keyed-ordinal/current-owner parent;
- the current checked callable join and `CallableDeclarationKey` digest;
- canonical `RuntimeProjectNominalProjection`, `RuntimeSemanticTypeId`,
  `TypeLayoutHash`, and `RuntimeTypeSchema` from the project nominal boundary;
- current registered/project entry and dialogue-coordinate identities.

This design adds only purpose-built semantic atoms where those authorities do
not yet expose exact checked meaning. It does not copy runtime layouts or
catalogs and does not treat any raw arena ID, span, or source spelling as
semantic authority.
