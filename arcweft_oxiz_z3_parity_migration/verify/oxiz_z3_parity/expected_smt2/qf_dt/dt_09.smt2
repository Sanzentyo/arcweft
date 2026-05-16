; Arcweft migration fixture for qf_dt.dt_09
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_dt/dt_09.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_DT)
(declare-datatypes ((dt_09_Box 0)) (((dt_09_box (dt_09_val Int)))))
(declare-const dt_09_b dt_09_Box)
(assert (= dt_09_b dt_09_b))
(check-sat)
