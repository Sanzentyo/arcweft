; Arcweft migration fixture for qf_a.array_08
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_a/array_08.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_A)
(declare-sort array_08_I 0)
(declare-sort array_08_E 0)
(declare-const array_08_a (Array array_08_I array_08_E))
(declare-const array_08_i array_08_I)
(declare-const array_08_e array_08_E)
(assert (= (select array_08_a array_08_i) array_08_e))
(assert (not (= (select array_08_a array_08_i) array_08_e)))
(check-sat)
