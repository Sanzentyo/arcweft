; Arcweft migration fixture for qf_a.array_06
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_a/array_06.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_A)
(declare-sort array_06_I 0)
(declare-sort array_06_E 0)
(declare-const array_06_a (Array array_06_I array_06_E))
(declare-const array_06_i array_06_I)
(declare-const array_06_e array_06_E)
(assert (= (select array_06_a array_06_i) array_06_e))
(assert (not (= (select array_06_a array_06_i) array_06_e)))
(check-sat)
