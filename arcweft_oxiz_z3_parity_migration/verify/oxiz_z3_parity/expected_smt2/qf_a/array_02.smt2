; Arcweft migration fixture for qf_a.array_02
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_a/array_02.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_A)
(declare-sort array_02_I 0)
(declare-sort array_02_E 0)
(declare-const array_02_a (Array array_02_I array_02_E))
(declare-const array_02_i array_02_I)
(declare-const array_02_e array_02_E)
(assert (= (select array_02_a array_02_i) array_02_e))
(check-sat)
