; Arcweft migration fixture for qf_a.array_01
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_a/array_01.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_A)
(declare-sort array_01_I 0)
(declare-sort array_01_E 0)
(declare-const array_01_a (Array array_01_I array_01_E))
(declare-const array_01_i array_01_I)
(declare-const array_01_e array_01_E)
(assert (= (select array_01_a array_01_i) array_01_e))
(check-sat)
