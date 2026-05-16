; Arcweft migration fixture for qf_a.array_09
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_a/array_09.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_A)
(declare-sort array_09_I 0)
(declare-sort array_09_E 0)
(declare-const array_09_a (Array array_09_I array_09_E))
(declare-const array_09_i array_09_I)
(declare-const array_09_e array_09_E)
(assert (= (select array_09_a array_09_i) array_09_e))
(check-sat)
