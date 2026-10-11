# 形式検証

## 入力

- Contract HIR
- Typed Graph
- Reducer/Flow/Activity contract
- Parser contract
- Shader parameter/resource contract
- Rust export manifest

## Proof IR

```arcw
pub enum ProofExpr {
    Bool(bool),
    Int(i64),
    Var(Symbol),
    And(Vec<ProofExpr>),
    Or(Vec<ProofExpr>),
    Not(Box<ProofExpr>),
    Eq(Box<ProofExpr>, Box<ProofExpr>),
    Le(Box<ProofExpr>, Box<ProofExpr>),
    Forall(Vec<ProofVar>, Box<ProofExpr>),
    Exists(Vec<ProofVar>, Box<ProofExpr>),
    App(Symbol, Vec<ProofExpr>),
}
```

## Backend

```arcw
pub trait SmtBackend {
    fn assert(&mut self, expr: ProofExpr) -> Result<()>;
    fn check(&mut self) -> Result<SmtResult>;
    fn model(&mut self) -> Option<Model>;
    fn unsat_core(&mut self) -> Option<UnsatCore>;
}
```

Backends:

- Z3
- OxiZ
- SMT-LIB external
- Kani harness for Rust code
- Creusot/Verus bridge for selected Rust code
- runtime contract checks
- property test generator

The verifier core remains Sans I/O. Concrete solver integrations live in
adapter crates:

```text
arcweft-verify       Proof IR, obligations, policies, diagnostics, SMT problem
arcweft-verify-z3    external Z3 process adapter
arcweft-verify-oxiz  pure Rust OxiZ adapter
```

CLI and build tooling choose an adapter. `arcweft-core`, parser, HIR, and the
verifier core must not spawn processes or depend on native solver libraries.

## CLI

```bash
arcw verify game/main.arcw --solver z3
arcw verify game/main.arcw --solver oxiz
arcw verify game/main.arcw --cross-check z3,oxiz
arcw verify game/main.arcw --emit-smt out/proofs
arcw verify activity mini_games/truck --kani
arcw verify game/main.arcw --emit-obligations out/proofs
```

Current implementation supports:

```bash
arcw verify game/main.arcw --backend emit --json
arcw verify game/main.arcw --backend oxiz
arcw verify game/main.arcw --backend z3 --solver-command z3
arcw verify game/main.arcw --emit-obligations obligations.json
arcw verify game/main.arcw --emit-smt out/proofs
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
The accepted verifier report collects obligations for lifetime
promotion, `unsafe lifetime`, upper-lifetime registry writes, thread capture,
proof-body validity, thread join result typing,
trusted assumptions, `Raw` syntax that reached HIR-facing analysis, sibling
thread and line child task write conflicts, and MustDrop registry values such
as `'line.focus` that are not explicitly dropped or transferred.

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

This Phase 1.9 pass is CFG-aware for blocks, branches, line plans,
cancellation rules, bounded loop fixed points, and scoped `defer` outcomes. It
checks proof references against typed `ensures` / `check` proof-body targets,
reports unjustified proof `assume` clauses and unknown proof dependencies
as `proof_body` obligations, and validates that unsafe audit blocks contain the
unchecked operation they justify. Ownership/region validation rejects borrowed
values escaping through block final values, returns, line-plan `out`, or
upper-lifetime registry writes, and the type checker tracks direct local borrow
drops across branch merges so maybe-dropped values cannot cross suspension
boundaries or be reused. It remains conservative for full solver-backed proof
term checking and open-ended effect inference; later compiler passes should
refine or discharge these obligations rather than bypassing the verifier
report.

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

