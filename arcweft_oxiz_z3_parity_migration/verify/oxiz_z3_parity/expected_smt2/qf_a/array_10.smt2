; Arcweft migration fixture for qf_a.array_10
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_a/array_10.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_A)
(declare-sort array_10_I 0)
(declare-sort array_10_E 0)
(declare-const array_10_a (Array array_10_I array_10_E))
(declare-const array_10_i array_10_I)
(declare-const array_10_e array_10_E)
(assert (= (select array_10_a array_10_i) array_10_e))
(check-sat)
