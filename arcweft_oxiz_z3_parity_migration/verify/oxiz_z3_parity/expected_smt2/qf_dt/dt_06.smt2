; Arcweft migration fixture for qf_dt.dt_06
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_dt/dt_06.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_DT)
(declare-datatypes ((dt_06_Box 0)) (((dt_06_box (dt_06_val Int)))))
(declare-const dt_06_b dt_06_Box)
(assert (= dt_06_b dt_06_b))
(assert (not (= dt_06_b dt_06_b)))
(check-sat)
