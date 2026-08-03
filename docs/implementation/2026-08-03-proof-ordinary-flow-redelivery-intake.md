# Proof 01.1.1.2.1.1.1 ordinary Flow corrected-redelivery intake

Date: 2026-08-03

Status: `RETURNED_READY_CLAIM_REJECTED_REPOSITORY_ADJUDICATED_IMPLEMENTATION_RELEASED`

## Archive identity and integrity

The corrected standalone archive was inspected at:

```text
D:/sanze/Downloads/arcweft-proof-concurrency-v6.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction-final-contract.zip
```

It is retained in the repository at:

```text
docs/reviews/packages/arcweft-proof-concurrency-v6.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction-final-contract.zip
```

- byte length: `87,004`;
- SHA-256:
  `BDC55671E7D4F8CDB3D07D8EC004672C90E14DEA88A47E63D8189E585BB3E4DF`;
- inspected Git commit:
  `aa983fda6b0de36d2f6867085ecdc95e630c5d99`, exactly the current `main`
  and `origin/main` baseline at intake;
- `28` unique flat file members, with no absolute path, traversal path,
  duplicate name, case-folded collision, or directory member;
- `MANIFEST.sha256`: `27` intentional non-self rows, all with matching exact
  byte lengths and SHA-256 values and no missing or extra member;
- `FINAL_STATUS.md`: `READY_FOR_IMPLEMENTATION`;
- `OPEN_QUESTIONS.md`: exactly the four bytes `none`; and
- the parent, primary, correction, and rejected-return intake copies are all
  byte-identical to their repository files.

All required sidecars are inside the ZIP. There is no adjacent status, hash,
summary, or manifest file and no production patch or implementation overlay.

The local checkout had the same Git `HEAD` and a protected dirty Proof working
set. This intake did not use that mutable state as package evidence and did not
stage or rewrite any of its production paths.

## Return verdict

The archive's standalone `READY_FOR_IMPLEMENTATION` claim is rejected. The
archive is mechanically sound and contains substantial usable Flow/Thread
evidence, but four result-changing schema decisions conflict with accepted
predecessors, maintained grammar, or existing final owners. Two additional
rows require evidence normalization, and its consumer list is not exhaustive.

No further redelivery is requested. These conflicts have one answer under the
accepted qualified-arena architecture, so this note records the repository
adjudication and releases implementation against that composite authority. The
ZIP remains retained evidence; it is not the sole implementation authority.

The applicable precedence is:

1. accepted Proof predecessor contracts and repository-wide rules;
2. the repository-local final-HIR owner decisions already implemented or
   recorded for the same sequence;
3. the adjudications below; and
4. this ZIP where it does not conflict with the first three sources.

## Repository adjudication

### Shared signature, scope, local, and prefix owners

The ZIP's Flow-specific `HirFlowGenericParameter`, `HirFlowParameter`, and
`HirFlowWherePredicate` records are not admitted. Flow uses the existing shared
`HirGenericParameter`, `HirParameter`, and `HirWherePredicate` owners. Its
attached header likewise uses the existing shared `AttachedItemPrefix` and
existing attached callable parameter, generic, and where owners. Flow
validation restricts that shared representation to one optional fixed
parameter group and forbids defaults; it does not create a second signature
schema.

The ZIP's `FlowCallable`, `FlowBody`, and `ThreadBody` scope kinds are also not
admitted. The exact mapping is:

- `HirScopeKind::Callable` for the Flow callable scope;
- `HirScopeKind::ContractRequires` and
  `HirScopeKind::ContractEnsures` for contract phases;
- `HirScopeKind::Flow` for the top-level Flow body; and
- `HirScopeKind::Block` for Thread-expression and nested statement bodies.

Postcondition `result` uses the existing `HirLocal` arena and
`HirLocalKind::PostconditionResult`. An omitted return has
`annotation = None`; an authored return has `annotation = Some(TypeId)`.
`HirLocalType` and `HirLocalOrigin` are not added. `HirFlowReturn` may retain
the storage distinction `OmittedUnit | Authored(TypeId)`, and a result-local
view may retain only the corresponding `LocalId`; neither becomes a parallel
type or local authority.

### Shared Thread-body item limit

`ACCOUNTING_LIMIT_TRANSACTION_MATRIX.tsv` row `A26` and test `T-LIMIT-26`
incorrectly remove the accepted direct-body item limit. The existing
`HirLimit::ThreadFlowItems` remains authoritative with inclusive maximum
`65,536`.

Every direct child inserted into a `HirThreadBody`, whether the owner is a Flow
or Thread expression, consumes one shared `ThreadFlowItems` unit in addition
to applicable statement, expression, synthetic-descendant, and global limits.
Exactly `65,536` commits. Attempt `65,537` fails and rolls back the whole
lowering transaction without publishing slots, source rows, diagnostics,
module state, or invalidation facts. No smaller Flow-only or Thread-only limit
is introduced.

### Exact `ensures no_effect` source ownership

The maintained authored grammar is exactly:

```arcw
ensures no_effect network.request
```

`NoEffect` remains one dedicated semantic contract variant, but its attached
source owner retains both authored keywords and the operand. The source-role
family distinguishes the outer clause keyword from the inner `no_effect`
keyword, for example `ClauseKeyword` and `NoEffectKeyword`; neither token is
overloaded as a mode. The operand lowers under requires-phase visibility and
cannot resolve the postcondition `result` local.

Bare `no_effect expression` is not current authored grammar. It follows normal
current-grammar recovery, is not accepted as a contract, and receives no
permanent removed-spelling diagnostic. Missing-operand and recovered-operand
tests must preserve both keyword components when present.

### Flow/Thread matrix symmetry and recovery

Identity, signature, and declaration-contract rows are Flow-only and explicitly
`NOT_APPLICABLE` to Thread expressions. Tests must assert that a Thread
expression cannot publish a Flow declaration identity, signature, contract
projection, project item, or callable candidate.

The attached Thread expression body must represent `Present` and `Missing` in
the existing accepted attached owner or an equivalent required-body wrapper.
A missing body commits a stable empty `HirThreadBody` plus
`HirThreadIssue::MissingBody`; an unclosed body retains its ordered items and
close-delimiter recovery. Thread bodies never have a value tail.

All sixteen `HirThreadFlowItem` variants require concrete per-variant fixtures
in both Flow and Thread-expression contexts. The `Error` variant is
recovery-only: it must be produced by an actual recovered input and must never
be described or tested as a valid authored item.

### Missing required Flow body

`IDENTITY_SIGNATURE_MATRIX.tsv` row `B03`, the non-optional
`HirFlowItem::body` schema, the four-scope graph, and test `T-BODY-03` take
precedence over the final sentence of `SOURCE_RECOVERY_DIAGNOSTIC_MATRIX.tsv`
row `D08`.

A recognized Flow with an absent required body commits:

- the recovered Flow `ItemId` and its four exact scopes;
- an empty `HirThreadBody` owned by the stable Flow body scope;
- the checked body-insertion source requirement; and
- `HirFlowIssueClass::MissingBody` poison.

It publishes no project, callable, or executable candidate. Fatal invariant,
stale/foreign identity, limit, cancellation, panic, and publication failures
publish nothing; ordinary missing-body recovery is not one of those failures.
No optional final-HIR body, sentinel, or generic Error item is introduced.

### Evidence cardinality and migration inventory

`REQUIREMENTS_TRACEABILITY.tsv` row `TR023` says `288` test rows. The archive's
actual strict TSV cardinality and its own `VALIDATION_REPORT.md` are `306`;
`306` is the evidence count.

The ZIP's `60` consumer rows are a seed inventory, not an exhaustive migration
boundary. In particular, the current compiler project/linking choke point,
project loader, LSP accepted-project and local-lowering readers, sema and
runtime-plan child modules/tests, compiler source/Agent clones, formatter, CLI,
cache, and downstream persistence/tooling readers must be included when they
consume the replaced authority. Current typed references plus
deletion-induced compile failures are authoritative. Package path and symbol
locators remain descriptive evidence and never become source gates.

Three locators are normalized during migration: row `U008` refers to the
current `classify_flow_item` owner rather than a non-existent
`flow_item_kind`; row `U026` finds `HirSourceSite` in the central source index,
not in `dialogue_application.rs`; and the package's
`DialogueCallExpression` spelling has no current consumer. The actual obsolete
Dialogue leaves are `FlowItem::SpeakerLine`, `FlowItem::ContentCall`, and
`HirDialogue`. These corrections do not authorize a compatibility spelling or
path check.

## Accepted package content

Subject to the adjudications above, the ZIP supplies implementation-ready
evidence for:

- the four exact attached and final-HIR Flow identity states;
- one ordered heterogeneous contract sequence covering all nine maintained
  clause families;
- callable, requires, ensures, and statement-only body visibility;
- one shared no-tail `HirThreadBody` with sixteen exact item variants;
- typed source-query, recovery, poison, accounting, and transaction matrices;
  and
- deletion-driven migration without compatibility readers, source
  reconstruction, source gates, removed-syntax-only final diagnostics, CSS, or
  Takumi.

All `148` normative matrix identifiers occur in the package's `306` test rows.
That coverage does not cure the conflicts above; tests are implemented against
the repository-adjudicated meaning.

## Released implementation boundary

Ordinary Flow and the shared Thread/Flow-item projection are no longer design
blocked. No additional request is needed for these decisions.

Implementation proceeds deletion-first through coherent compiling cuts:

1. replace the split Requires/Ensures parser roles and the auxiliary-clause
   discard helper with one attached heterogeneous contract-clause authority;
2. install the central attached `FlowItemNode` and required Flow/Thread body
   projection using existing shared prefix, delimiter, token, and callable
   owners;
3. directly replace provisional private `HirFlowItem` and `HirThreadBody`
   shapes with the shared owner decisions above, then complete scopes, locals,
   source rows, poison, limits, freeze, and transactional lowering;
4. publish the module-preserving accepted project and callable authority; and
5. in the same workspace-compiling public authority switch, make the old
   `lower_document_to_hir`/linked-module readers unavailable, migrate every
   compiler, project, sema, verifier, runtime-plan, formatter, LSP, CLI, Agent,
   cache, and persistence consumer exposed by compilation, and delete detached
   Flow/Thread AST, value-tail, legacy SpeakerLine/ContentCall/HirDialogue, and
   old capability paths.

The production legacy route remains frozen only until its replacement can carry
the full execution path. It receives no feature repair or semantic extension.
At the public switch boundary, obsolete entry points are removed before their
call sites are repaired, and compile failures are the migration inventory.
Deleted helpers, variants, or readers must not be restored to obtain an
intermediate green build.

## Intake validation

- archive path safety, decompression, member uniqueness, and case-folded
  uniqueness checked;
- every manifest length and SHA-256 recomputed;
- all four repository request/intake copies compared byte-for-byte;
- exact status and four-byte open-question payload checked;
- every TSV parsed, row IDs and cardinalities checked, and all `148` normative
  matrix rows independently cross-referenced to the test matrix;
- all `60` listed consumer paths checked against the current checkout;
- three independent schema, owner, and public-consumer audits performed; and
- the archive's READY claim challenged against accepted predecessor and current
  typed-owner evidence rather than inferred from its sidecars.

This is a docs-and-package intake cut. It changes no Rust, Cargo, runtime,
render, Agent, MCP, persistence, or codec behavior, so Rust tests and Tier 2
are not applicable to this cut.
