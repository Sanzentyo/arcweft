; Arcweft migration fixture for qf_dt.dt_10
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_dt/dt_10.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_DT)
(declare-datatypes ((dt_10_Box 0)) (((dt_10_box (dt_10_val Int)))))
(declare-const dt_10_b dt_10_Box)
(assert (= dt_10_b dt_10_b))
(check-sat)
