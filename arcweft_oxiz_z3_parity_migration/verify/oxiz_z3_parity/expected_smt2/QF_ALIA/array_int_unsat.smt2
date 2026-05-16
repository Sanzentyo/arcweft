; Arcweft migration fixture for QF_ALIA.array_int_unsat
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_ALIA/array_int_unsat.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_ALIA)
(declare-const array_int_unsat_a (Array Int Int))
(declare-const array_int_unsat_i Int)
(assert (= (select (store array_int_unsat_a array_int_unsat_i 7) array_int_unsat_i) 7))
(assert (= (select (store array_int_unsat_a array_int_unsat_i 7) array_int_unsat_i) 8))
(check-sat)
