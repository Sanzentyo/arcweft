# Dependency graph

```text
ProjectSymbolTable ─┐
RegisteredSemanticWorld ─┼─> FinalSemanticCatalogs / CheckedOwnershipContext
ResourceTypeRegistry ────┘                  │
                                            ├─> MatchCoverageAnalyzer
accepted HIR + checked maps ────────────────┤
                                            └─> CheckedMatch + semantic digest
                                                         │
                                                         v
arcweft-compiler one-way projection ─> RuntimePlanSemanticFactInput
                                                         │
                                                         v
RuntimePlan typed selector/task seeds ─> AWBC inventory/lowering/verifier
                                                         │
        AwbcOpcode/kind/flags + canonical Wire ──────────┤
                                                         v
                  bundle static View/AWBC join + content root
                                                         │
                                                         v
runtime-driver decode/install ─> typed Need/task scheduler/journal/snapshots
                                                         │
                                                         v
                 awbc::vm::step / structured product-step / AOT parity
```

Dependency-order invariant: no consumer lands before its final owner. Pending
opcode/kind/flag variants are absent until their complete verifier and execution
cut. No layer reconstructs a fact owned by an earlier layer.
