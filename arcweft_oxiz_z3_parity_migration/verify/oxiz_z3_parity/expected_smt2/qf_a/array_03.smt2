; Arcweft migration fixture for qf_a.array_03
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_a/array_03.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_A)
(declare-sort array_03_I 0)
(declare-sort array_03_E 0)
(declare-const array_03_a (Array array_03_I array_03_E))
(declare-const array_03_i array_03_I)
(declare-const array_03_e array_03_E)
(assert (= (select array_03_a array_03_i) array_03_e))
(assert (not (= (select array_03_a array_03_i) array_03_e)))
(check-sat)
