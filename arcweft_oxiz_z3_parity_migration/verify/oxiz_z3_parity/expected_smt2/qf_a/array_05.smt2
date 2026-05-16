; Arcweft migration fixture for qf_a.array_05
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_a/array_05.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_A)
(declare-sort array_05_I 0)
(declare-sort array_05_E 0)
(declare-const array_05_a (Array array_05_I array_05_E))
(declare-const array_05_i array_05_I)
(declare-const array_05_e array_05_E)
(assert (= (select array_05_a array_05_i) array_05_e))
(check-sat)
