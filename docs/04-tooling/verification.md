# 形式検証

## 入力

- Contract HIR
- Typed Graph
- Reducer/Flow/Activity contract
- Parser contract
- Shader parameter/resource contract
- Rust export manifest

## Proof IR and backend

`arcweft-verify` owns the solver-neutral proof-expression grammar, typed SMT
problem construction, deterministic SMT-LIB emission, and `SmtBackend` boundary.
A backend receives an `SmtProblem` and returns a typed check result. The exact
expression constructors and result types are owned by
[`arcweft-verify::smt`](../../crates/arcweft-verify/src/smt.rs).

The verifier core remains Sans I/O. Current solver integrations live in adapter
crates:

- `arcweft-verify-oxiz` provides the pure Rust OxiZ adapter.
- `arcweft-verify-z3` provides the external Z3 process adapter.
- `emit` writes the report and SMT-LIB without solving.

CLI and build tooling select an adapter. `arcweft-core`, parser, HIR, and the
verifier core do not spawn solver processes or depend on native solver libraries.

## CLI

```bash
arcw verify game/main.arcw --backend emit --json
arcw verify game/main.arcw --backend oxiz --json
arcw verify game/main.arcw --backend z3 --z3-command z3 --json
arcw verify game/main.arcw --backend emit --emit-obligations obligations.json
arcw verify game/main.arcw --backend emit --emit-smt out/proofs
arcw unsafe game/main.arcw --json
```

## Source proof items and unsafe audits

Arcweft source can contain formal proof items and audited unsafe lifetime
regions. See
[Proofs and Unsafe Lifetime Audits](../01-language/proofs-and-unsafe-audits.md)
for the surface syntax.

Verification must collect:

```text
- generated proof obligations
- proof item bodies
- proof references such as proof = @proof.id
- `#[verify.trusted(reason = "...")]` proof metadata
- transitive trusted proof dependencies
- unsafe lifetime audit blocks
- assume clauses and their proof dependencies
```

Release verification should reject undisclosed audited unsafe, policy-forbidden
trusted proof evidence, and unproven non-trivial lifetime promotion, thread
capture, global mutation, and MustDrop override obligations.

The final project semantic pass owns checked facts and admits one exact HIR
generation. `arcweft-verify` consumes that generation, its symbol authority and
sealed `FinalSemanticAnalysis`. CLI, LSP and Agent tooling borrow those facts;
they do not rerun analysis or manufacture an executable plan after rejection.
The accepted verifier report can carry `proof_body`, `assertion_proof`, and
`unsafe_lifetime_audit` obligations for an admitted project.

Effects are checked as semantic facts, not as ordinary value expressions.
Flow/function effects clauses bound the inferred effects of their bodies:

```arcw
flow effects
effects { signal.write, metric.write, state.write('flow) }
{
    signal.set(@signal.current_flow, @flow.effects)
    metric.set(@metric.choice_count, 1)
    'flow.flags.seen <- true
}
```

`signal.set(...)` requires `signal.write` and `metric.set(...)` requires
`metric.write`; upper-lifetime registry writes such as `'flow.flags.seen <- true`
require `state.write(flow)`, written in source as `state.write('flow)`. Omitted
`effects` infer the body row. Explicit `effects {}` imposes an empty upper
bound; explicit members impose the corresponding checked upper bound. Target
capability availability is checked separately and cannot widen that bound.

When project compilation rejects analysis, `arcw verify --json` publishes the
compiler-owned rejection and returns a non-success exit code. This version1
report has `status: "denied"`, its original compilation `stage`, and canonical
diagnostic rows. Typed effect rejections carry
`rejected_obligation_kind: "effect_capability"`; other rejections omit that
optional classification. Codes, severity, messages, source revision-bound
spans, primary/secondary labels, notes, suggestions and commands come from the
original diagnostic authority. A rejected seal has no accepted obligations,
solver checks, executable runtime plan or generated proof artifacts. Solver
execution and `--emit-obligations`/`--emit-smt` begin only after compilation
succeeds. The accepted verifier JSON grammar remains the shared verifier report.

The report draws proof references and unsafe audit metadata from checked inputs.
The language contract defines proof admission and the checker's ownership and
region rules. [Proofs and Unsafe Lifetime Audits](../01-language/proofs-and-unsafe-audits.md).

Verifier JSON serializes proof expressions with adjacent tags so string-carrying
variants remain stable across serde implementations:

```json
{ "kind": "var", "value": "signal.write" }
```

## Counterexample

```json
{
  "property": "inv.affection_bounds",
  "failed_in": "reducer.update",
  "trace": [
    { "event": "ChoiceSelected", "id": "choice.opening.listen" }
  ],
  "state_before": { "affection.character.alice": 100 },
  "result_state": { "affection.character.alice": 101 },
  "suggestion": "Clamp affection after addition"
}
```
