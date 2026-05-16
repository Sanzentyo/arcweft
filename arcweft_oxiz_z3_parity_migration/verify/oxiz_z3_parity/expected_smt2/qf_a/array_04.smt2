; Arcweft migration fixture for qf_a.array_04
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_a/array_04.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_A)
(declare-sort array_04_I 0)
(declare-sort array_04_E 0)
(declare-const array_04_a (Array array_04_I array_04_E))
(declare-const array_04_i array_04_I)
(declare-const array_04_e array_04_E)
(assert (= (select array_04_a array_04_i) array_04_e))
(check-sat)
