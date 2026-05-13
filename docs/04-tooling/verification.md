# 形式検証

## 入力

- Contract HIR
- Typed Graph
- Reducer/Flow/Activity contract
- Parser contract
- Shader parameter/resource contract
- Rust export manifest

## Proof IR

```awft
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

```awft
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

## CLI

```bash
arcw verify game/main.awft --solver z3
arcw verify game/main.awft --solver oxiz
arcw verify game/main.awft --cross-check z3,oxiz
arcw verify game/main.awft --emit-smt out/proofs
arcw verify activity mini_games/truck --kani
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

