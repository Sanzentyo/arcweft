; Arcweft migration fixture for qf_dt.dt_03
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_dt/dt_03.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_DT)
(declare-datatypes ((dt_03_Box 0)) (((dt_03_box (dt_03_val Int)))))
(declare-const dt_03_b dt_03_Box)
(assert (= dt_03_b dt_03_b))
(assert (not (= dt_03_b dt_03_b)))
(check-sat)
