# OxiZ → Arcweft verify migration checklist

All entries below are materialized as both an `.awft` script and an expected SMT-LIB2 fixture.

- [x] Source repo pinned: `cool-japan/oxiz`
- [x] Source commit pinned: `9f6bb93df338fd8e965511e9e1abc97ed3ca395f`
- [x] Total manifest entries: `168`
- [x] One `.awft` file per manifest entry
- [x] One expected `.smt2` file per manifest entry
- [x] Cargo integration test added under `crates/arcweft-cli/tests/verify_oxiz_z3_parity.rs`
- [x] Optional exact-vendoring script included under `verify/oxiz_z3_parity/scripts/vendor_oxiz_sources.py`

## Per-case checklist

### AUFLIA (10)

- [x] `AUFLIA.array_extensionality` → `awft/AUFLIA/array_extensionality.awft` → `expected_smt2/AUFLIA/array_extensionality.smt2` expected `sat`
- [x] `AUFLIA.array_forall_init` → `awft/AUFLIA/array_forall_init.awft` → `expected_smt2/AUFLIA/array_forall_init.smt2` expected `sat`
- [x] `AUFLIA.array_max` → `awft/AUFLIA/array_max.awft` → `expected_smt2/AUFLIA/array_max.smt2` expected `sat`
- [x] `AUFLIA.array_partition` → `awft/AUFLIA/array_partition.awft` → `expected_smt2/AUFLIA/array_partition.smt2` expected `sat`
- [x] `AUFLIA.array_permutation` → `awft/AUFLIA/array_permutation.awft` → `expected_smt2/AUFLIA/array_permutation.smt2` expected `sat`
- [x] `AUFLIA.array_search` → `awft/AUFLIA/array_search.awft` → `expected_smt2/AUFLIA/array_search.smt2` expected `sat`
- [x] `AUFLIA.array_sorted` → `awft/AUFLIA/array_sorted.awft` → `expected_smt2/AUFLIA/array_sorted.smt2` expected `sat`
- [x] `AUFLIA.array_unique` → `awft/AUFLIA/array_unique.awft` → `expected_smt2/AUFLIA/array_unique.smt2` expected `unsat`
- [x] `AUFLIA.array_unsat` → `awft/AUFLIA/array_unsat.awft` → `expected_smt2/AUFLIA/array_unsat.smt2` expected `unsat`
- [x] `AUFLIA.array_update` → `awft/AUFLIA/array_update.awft` → `expected_smt2/AUFLIA/array_update.smt2` expected `sat`

### AUFLIRA (5)

- [x] `AUFLIRA.auflira_array_func` → `awft/AUFLIRA/auflira_array_func.awft` → `expected_smt2/AUFLIRA/auflira_array_func.smt2` expected `unsat`
- [x] `AUFLIRA.auflira_basic_sat` → `awft/AUFLIRA/auflira_basic_sat.awft` → `expected_smt2/AUFLIRA/auflira_basic_sat.smt2` expected `sat`
- [x] `AUFLIRA.auflira_mixed_arith` → `awft/AUFLIRA/auflira_mixed_arith.awft` → `expected_smt2/AUFLIRA/auflira_mixed_arith.smt2` expected `unsat`
- [x] `AUFLIRA.auflira_quantified` → `awft/AUFLIRA/auflira_quantified.awft` → `expected_smt2/AUFLIRA/auflira_quantified.smt2` expected `unsat`
- [x] `AUFLIRA.auflira_unsat` → `awft/AUFLIRA/auflira_unsat.awft` → `expected_smt2/AUFLIRA/auflira_unsat.smt2` expected `unsat`

### QF_ABV (5)

- [x] `QF_ABV.qf_abv_bv_arith` → `awft/QF_ABV/qf_abv_bv_arith.awft` → `expected_smt2/QF_ABV/qf_abv_bv_arith.smt2` expected `unsat`
- [x] `QF_ABV.qf_abv_contradiction` → `awft/QF_ABV/qf_abv_contradiction.awft` → `expected_smt2/QF_ABV/qf_abv_contradiction.smt2` expected `unsat`
- [x] `QF_ABV.qf_abv_overflow` → `awft/QF_ABV/qf_abv_overflow.awft` → `expected_smt2/QF_ABV/qf_abv_overflow.smt2` expected `unsat`
- [x] `QF_ABV.qf_abv_store_chain` → `awft/QF_ABV/qf_abv_store_chain.awft` → `expected_smt2/QF_ABV/qf_abv_store_chain.smt2` expected `sat`
- [x] `QF_ABV.qf_abv_store_select_sat` → `awft/QF_ABV/qf_abv_store_select_sat.awft` → `expected_smt2/QF_ABV/qf_abv_store_select_sat.smt2` expected `sat`

### QF_ALIA (5)

- [x] `QF_ALIA.array_int_basic` → `awft/QF_ALIA/array_int_basic.awft` → `expected_smt2/QF_ALIA/array_int_basic.smt2` expected `sat`
- [x] `QF_ALIA.array_int_const_array` → `awft/QF_ALIA/array_int_const_array.awft` → `expected_smt2/QF_ALIA/array_int_const_array.smt2` expected `unsat`
- [x] `QF_ALIA.array_int_sorted` → `awft/QF_ALIA/array_int_sorted.awft` → `expected_smt2/QF_ALIA/array_int_sorted.smt2` expected `sat`
- [x] `QF_ALIA.array_int_sum_pattern` → `awft/QF_ALIA/array_int_sum_pattern.awft` → `expected_smt2/QF_ALIA/array_int_sum_pattern.smt2` expected `sat`
- [x] `QF_ALIA.array_int_unsat` → `awft/QF_ALIA/array_int_unsat.awft` → `expected_smt2/QF_ALIA/array_int_unsat.smt2` expected `unsat`

### QF_AUFBV (5)

- [x] `QF_AUFBV.array_bv_equality` → `awft/QF_AUFBV/array_bv_equality.awft` → `expected_smt2/QF_AUFBV/array_bv_equality.smt2` expected `unsat`
- [x] `QF_AUFBV.array_bv_ite_select` → `awft/QF_AUFBV/array_bv_ite_select.awft` → `expected_smt2/QF_AUFBV/array_bv_ite_select.smt2` expected `sat`
- [x] `QF_AUFBV.array_bv_multi_store` → `awft/QF_AUFBV/array_bv_multi_store.awft` → `expected_smt2/QF_AUFBV/array_bv_multi_store.smt2` expected `sat`
- [x] `QF_AUFBV.array_bv_store_select` → `awft/QF_AUFBV/array_bv_store_select.awft` → `expected_smt2/QF_AUFBV/array_bv_store_select.smt2` expected `sat`
- [x] `QF_AUFBV.array_bv_unsat_conflict` → `awft/QF_AUFBV/array_bv_unsat_conflict.awft` → `expected_smt2/QF_AUFBV/array_bv_unsat_conflict.smt2` expected `unsat`

### QF_AUFLIA (5)

- [x] `QF_AUFLIA.uf_array_int` → `awft/QF_AUFLIA/uf_array_int.awft` → `expected_smt2/QF_AUFLIA/uf_array_int.smt2` expected `sat`
- [x] `QF_AUFLIA.uf_array_int_trans` → `awft/QF_AUFLIA/uf_array_int_trans.awft` → `expected_smt2/QF_AUFLIA/uf_array_int_trans.smt2` expected `unsat`
- [x] `QF_AUFLIA.uf_array_model` → `awft/QF_AUFLIA/uf_array_model.awft` → `expected_smt2/QF_AUFLIA/uf_array_model.smt2` expected `sat`
- [x] `QF_AUFLIA.uf_array_store_func` → `awft/QF_AUFLIA/uf_array_store_func.awft` → `expected_smt2/QF_AUFLIA/uf_array_store_func.smt2` expected `sat`
- [x] `QF_AUFLIA.uf_array_unsat` → `awft/QF_AUFLIA/uf_array_unsat.awft` → `expected_smt2/QF_AUFLIA/uf_array_unsat.smt2` expected `unsat`

### QF_NIRA (5)

- [x] `QF_NIRA.qf_nira_basic_sat` → `awft/QF_NIRA/qf_nira_basic_sat.awft` → `expected_smt2/QF_NIRA/qf_nira_basic_sat.smt2` expected `sat`
- [x] `QF_NIRA.qf_nira_mixed` → `awft/QF_NIRA/qf_nira_mixed.awft` → `expected_smt2/QF_NIRA/qf_nira_mixed.smt2` expected `unsat`
- [x] `QF_NIRA.qf_nira_product_sat` → `awft/QF_NIRA/qf_nira_product_sat.awft` → `expected_smt2/QF_NIRA/qf_nira_product_sat.smt2` expected `sat`
- [x] `QF_NIRA.qf_nira_real_poly` → `awft/QF_NIRA/qf_nira_real_poly.awft` → `expected_smt2/QF_NIRA/qf_nira_real_poly.smt2` expected `unsat`
- [x] `QF_NIRA.qf_nira_unsat` → `awft/QF_NIRA/qf_nira_unsat.awft` → `expected_smt2/QF_NIRA/qf_nira_unsat.smt2` expected `unsat`

### QF_UFLIA (5)

- [x] `QF_UFLIA.uf_int_basic` → `awft/QF_UFLIA/uf_int_basic.awft` → `expected_smt2/QF_UFLIA/uf_int_basic.smt2` expected `sat`
- [x] `QF_UFLIA.uf_int_congruence` → `awft/QF_UFLIA/uf_int_congruence.awft` → `expected_smt2/QF_UFLIA/uf_int_congruence.smt2` expected `unsat`
- [x] `QF_UFLIA.uf_int_inject` → `awft/QF_UFLIA/uf_int_inject.awft` → `expected_smt2/QF_UFLIA/uf_int_inject.smt2` expected `sat`
- [x] `QF_UFLIA.uf_int_monotone` → `awft/QF_UFLIA/uf_int_monotone.awft` → `expected_smt2/QF_UFLIA/uf_int_monotone.smt2` expected `sat`
- [x] `QF_UFLIA.uf_int_unsat` → `awft/QF_UFLIA/uf_int_unsat.awft` → `expected_smt2/QF_UFLIA/uf_int_unsat.smt2` expected `unsat`

### QF_UFLRA (5)

- [x] `QF_UFLRA.uf_real_abs` → `awft/QF_UFLRA/uf_real_abs.awft` → `expected_smt2/QF_UFLRA/uf_real_abs.smt2` expected `sat`
- [x] `QF_UFLRA.uf_real_basic` → `awft/QF_UFLRA/uf_real_basic.awft` → `expected_smt2/QF_UFLRA/uf_real_basic.smt2` expected `sat`
- [x] `QF_UFLRA.uf_real_interp` → `awft/QF_UFLRA/uf_real_interp.awft` → `expected_smt2/QF_UFLRA/uf_real_interp.smt2` expected `unsat`
- [x] `QF_UFLRA.uf_real_linear` → `awft/QF_UFLRA/uf_real_linear.awft` → `expected_smt2/QF_UFLRA/uf_real_linear.smt2` expected `sat`
- [x] `QF_UFLRA.uf_real_unsat` → `awft/QF_UFLRA/uf_real_unsat.awft` → `expected_smt2/QF_UFLRA/uf_real_unsat.smt2` expected `unsat`

### UFLIA (20)

- [x] `UFLIA.ackermann` → `awft/UFLIA/ackermann.awft` → `expected_smt2/UFLIA/ackermann.smt2` expected `sat`
- [x] `UFLIA.arith_unsat` → `awft/UFLIA/arith_unsat.awft` → `expected_smt2/UFLIA/arith_unsat.smt2` expected `unsat`
- [x] `UFLIA.congruence_closure` → `awft/UFLIA/congruence_closure.awft` → `expected_smt2/UFLIA/congruence_closure.smt2` expected `sat`
- [x] `UFLIA.division_property` → `awft/UFLIA/division_property.awft` → `expected_smt2/UFLIA/division_property.smt2` expected `sat`
- [x] `UFLIA.forall_exists_simple` → `awft/UFLIA/forall_exists_simple.awft` → `expected_smt2/UFLIA/forall_exists_simple.smt2` expected `sat`
- [x] `UFLIA.forall_exists_unsat` → `awft/UFLIA/forall_exists_unsat.awft` → `expected_smt2/UFLIA/forall_exists_unsat.smt2` expected `unsat`
- [x] `UFLIA.idempotent` → `awft/UFLIA/idempotent.awft` → `expected_smt2/UFLIA/idempotent.smt2` expected `sat`
- [x] `UFLIA.injective` → `awft/UFLIA/injective.awft` → `expected_smt2/UFLIA/injective.smt2` expected `sat`
- [x] `UFLIA.injective_unsat` → `awft/UFLIA/injective_unsat.awft` → `expected_smt2/UFLIA/injective_unsat.smt2` expected `unsat`
- [x] `UFLIA.linear_order` → `awft/UFLIA/linear_order.awft` → `expected_smt2/UFLIA/linear_order.smt2` expected `sat`
- [x] `UFLIA.monotone_basic` → `awft/UFLIA/monotone_basic.awft` → `expected_smt2/UFLIA/monotone_basic.smt2` expected `sat`
- [x] `UFLIA.monotone_violation` → `awft/UFLIA/monotone_violation.awft` → `expected_smt2/UFLIA/monotone_violation.smt2` expected `unsat`
- [x] `UFLIA.nested_quantifiers` → `awft/UFLIA/nested_quantifiers.awft` → `expected_smt2/UFLIA/nested_quantifiers.smt2` expected `sat`
- [x] `UFLIA.pigeonhole_3` → `awft/UFLIA/pigeonhole_3.awft` → `expected_smt2/UFLIA/pigeonhole_3.smt2` expected `unsat`
- [x] `UFLIA.reflexivity` → `awft/UFLIA/reflexivity.awft` → `expected_smt2/UFLIA/reflexivity.smt2` expected `sat`
- [x] `UFLIA.skolem_test` → `awft/UFLIA/skolem_test.awft` → `expected_smt2/UFLIA/skolem_test.smt2` expected `sat`
- [x] `UFLIA.sum_bounds` → `awft/UFLIA/sum_bounds.awft` → `expected_smt2/UFLIA/sum_bounds.smt2` expected `sat`
- [x] `UFLIA.surjective` → `awft/UFLIA/surjective.awft` → `expected_smt2/UFLIA/surjective.smt2` expected `sat`
- [x] `UFLIA.trans_simple` → `awft/UFLIA/trans_simple.awft` → `expected_smt2/UFLIA/trans_simple.smt2` expected `sat`
- [x] `UFLIA.trans_unsat` → `awft/UFLIA/trans_unsat.awft` → `expected_smt2/UFLIA/trans_unsat.smt2` expected `unsat`

### UFLRA (10)

- [x] `UFLRA.real_archimedean` → `awft/UFLRA/real_archimedean.awft` → `expected_smt2/UFLRA/real_archimedean.smt2` expected `sat`
- [x] `UFLRA.real_bounds` → `awft/UFLRA/real_bounds.awft` → `expected_smt2/UFLRA/real_bounds.smt2` expected `sat`
- [x] `UFLRA.real_composition` → `awft/UFLRA/real_composition.awft` → `expected_smt2/UFLRA/real_composition.smt2` expected `sat`
- [x] `UFLRA.real_convex` → `awft/UFLRA/real_convex.awft` → `expected_smt2/UFLRA/real_convex.smt2` expected `sat`
- [x] `UFLRA.real_fixed_point` → `awft/UFLRA/real_fixed_point.awft` → `expected_smt2/UFLRA/real_fixed_point.smt2` expected `sat`
- [x] `UFLRA.real_identity` → `awft/UFLRA/real_identity.awft` → `expected_smt2/UFLRA/real_identity.smt2` expected `sat`
- [x] `UFLRA.real_interp` → `awft/UFLRA/real_interp.awft` → `expected_smt2/UFLRA/real_interp.smt2` expected `sat`
- [x] `UFLRA.real_lipschitz` → `awft/UFLRA/real_lipschitz.awft` → `expected_smt2/UFLRA/real_lipschitz.smt2` expected `sat`
- [x] `UFLRA.real_monotone` → `awft/UFLRA/real_monotone.awft` → `expected_smt2/UFLRA/real_monotone.smt2` expected `sat`
- [x] `UFLRA.real_unsat` → `awft/UFLRA/real_unsat.awft` → `expected_smt2/UFLRA/real_unsat.smt2` expected `unsat`

### qf_a (10)

- [x] `qf_a.array_01` → `awft/qf_a/array_01.awft` → `expected_smt2/qf_a/array_01.smt2` expected `sat`
- [x] `qf_a.array_02` → `awft/qf_a/array_02.awft` → `expected_smt2/qf_a/array_02.smt2` expected `sat`
- [x] `qf_a.array_03` → `awft/qf_a/array_03.awft` → `expected_smt2/qf_a/array_03.smt2` expected `unsat`
- [x] `qf_a.array_04` → `awft/qf_a/array_04.awft` → `expected_smt2/qf_a/array_04.smt2` expected `sat`
- [x] `qf_a.array_05` → `awft/qf_a/array_05.awft` → `expected_smt2/qf_a/array_05.smt2` expected `sat`
- [x] `qf_a.array_06` → `awft/qf_a/array_06.awft` → `expected_smt2/qf_a/array_06.smt2` expected `unsat`
- [x] `qf_a.array_07` → `awft/qf_a/array_07.awft` → `expected_smt2/qf_a/array_07.smt2` expected `sat`
- [x] `qf_a.array_08` → `awft/qf_a/array_08.awft` → `expected_smt2/qf_a/array_08.smt2` expected `unsat`
- [x] `qf_a.array_09` → `awft/qf_a/array_09.awft` → `expected_smt2/qf_a/array_09.smt2` expected `sat`
- [x] `qf_a.array_10` → `awft/qf_a/array_10.awft` → `expected_smt2/qf_a/array_10.smt2` expected `sat`

### qf_bv (15)

- [x] `qf_bv.bv_01` → `awft/qf_bv/bv_01.awft` → `expected_smt2/qf_bv/bv_01.smt2` expected `sat`
- [x] `qf_bv.bv_02` → `awft/qf_bv/bv_02.awft` → `expected_smt2/qf_bv/bv_02.smt2` expected `unsat`
- [x] `qf_bv.bv_03` → `awft/qf_bv/bv_03.awft` → `expected_smt2/qf_bv/bv_03.smt2` expected `sat`
- [x] `qf_bv.bv_04` → `awft/qf_bv/bv_04.awft` → `expected_smt2/qf_bv/bv_04.smt2` expected `sat`
- [x] `qf_bv.bv_05` → `awft/qf_bv/bv_05.awft` → `expected_smt2/qf_bv/bv_05.smt2` expected `sat`
- [x] `qf_bv.bv_06` → `awft/qf_bv/bv_06.awft` → `expected_smt2/qf_bv/bv_06.smt2` expected `unsat`
- [x] `qf_bv.bv_07` → `awft/qf_bv/bv_07.awft` → `expected_smt2/qf_bv/bv_07.smt2` expected `sat`
- [x] `qf_bv.bv_08` → `awft/qf_bv/bv_08.awft` → `expected_smt2/qf_bv/bv_08.smt2` expected `sat`
- [x] `qf_bv.bv_09` → `awft/qf_bv/bv_09.awft` → `expected_smt2/qf_bv/bv_09.smt2` expected `sat`
- [x] `qf_bv.bv_10` → `awft/qf_bv/bv_10.awft` → `expected_smt2/qf_bv/bv_10.smt2` expected `sat`
- [x] `qf_bv.bv_11` → `awft/qf_bv/bv_11.awft` → `expected_smt2/qf_bv/bv_11.smt2` expected `unsat`
- [x] `qf_bv.bv_12` → `awft/qf_bv/bv_12.awft` → `expected_smt2/qf_bv/bv_12.smt2` expected `unsat`
- [x] `qf_bv.bv_13` → `awft/qf_bv/bv_13.awft` → `expected_smt2/qf_bv/bv_13.smt2` expected `sat`
- [x] `qf_bv.bv_14` → `awft/qf_bv/bv_14.awft` → `expected_smt2/qf_bv/bv_14.smt2` expected `sat`
- [x] `qf_bv.bv_15` → `awft/qf_bv/bv_15.awft` → `expected_smt2/qf_bv/bv_15.smt2` expected `unsat`

### qf_dt (10)

- [x] `qf_dt.dt_01` → `awft/qf_dt/dt_01.awft` → `expected_smt2/qf_dt/dt_01.smt2` expected `sat`
- [x] `qf_dt.dt_02` → `awft/qf_dt/dt_02.awft` → `expected_smt2/qf_dt/dt_02.smt2` expected `sat`
- [x] `qf_dt.dt_03` → `awft/qf_dt/dt_03.awft` → `expected_smt2/qf_dt/dt_03.smt2` expected `unsat`
- [x] `qf_dt.dt_04` → `awft/qf_dt/dt_04.awft` → `expected_smt2/qf_dt/dt_04.smt2` expected `sat`
- [x] `qf_dt.dt_05` → `awft/qf_dt/dt_05.awft` → `expected_smt2/qf_dt/dt_05.smt2` expected `sat`
- [x] `qf_dt.dt_06` → `awft/qf_dt/dt_06.awft` → `expected_smt2/qf_dt/dt_06.smt2` expected `unsat`
- [x] `qf_dt.dt_07` → `awft/qf_dt/dt_07.awft` → `expected_smt2/qf_dt/dt_07.smt2` expected `sat`
- [x] `qf_dt.dt_08` → `awft/qf_dt/dt_08.awft` → `expected_smt2/qf_dt/dt_08.smt2` expected `unsat`
- [x] `qf_dt.dt_09` → `awft/qf_dt/dt_09.awft` → `expected_smt2/qf_dt/dt_09.smt2` expected `sat`
- [x] `qf_dt.dt_10` → `awft/qf_dt/dt_10.awft` → `expected_smt2/qf_dt/dt_10.smt2` expected `sat`

### qf_fp (10)

- [x] `qf_fp.fp_01` → `awft/qf_fp/fp_01.awft` → `expected_smt2/qf_fp/fp_01.smt2` expected `sat`
- [x] `qf_fp.fp_02` → `awft/qf_fp/fp_02.awft` → `expected_smt2/qf_fp/fp_02.smt2` expected `sat`
- [x] `qf_fp.fp_03` → `awft/qf_fp/fp_03.awft` → `expected_smt2/qf_fp/fp_03.smt2` expected `unsat`
- [x] `qf_fp.fp_04` → `awft/qf_fp/fp_04.awft` → `expected_smt2/qf_fp/fp_04.smt2` expected `sat`
- [x] `qf_fp.fp_05` → `awft/qf_fp/fp_05.awft` → `expected_smt2/qf_fp/fp_05.smt2` expected `sat`
- [x] `qf_fp.fp_06` → `awft/qf_fp/fp_06.awft` → `expected_smt2/qf_fp/fp_06.smt2` expected `unsat`
- [x] `qf_fp.fp_07` → `awft/qf_fp/fp_07.awft` → `expected_smt2/qf_fp/fp_07.smt2` expected `sat`
- [x] `qf_fp.fp_08` → `awft/qf_fp/fp_08.awft` → `expected_smt2/qf_fp/fp_08.smt2` expected `unsat`
- [x] `qf_fp.fp_09` → `awft/qf_fp/fp_09.awft` → `expected_smt2/qf_fp/fp_09.smt2` expected `sat`
- [x] `qf_fp.fp_10` → `awft/qf_fp/fp_10.awft` → `expected_smt2/qf_fp/fp_10.smt2` expected `unsat`

### qf_lia (16)

- [x] `qf_lia.lia_01_range` → `awft/qf_lia/lia_01_range.awft` → `expected_smt2/qf_lia/lia_01_range.smt2` expected `sat`
- [x] `qf_lia.lia_02_linear_eq` → `awft/qf_lia/lia_02_linear_eq.awft` → `expected_smt2/qf_lia/lia_02_linear_eq.smt2` expected `sat`
- [x] `qf_lia.lia_03_linear_ineq` → `awft/qf_lia/lia_03_linear_ineq.awft` → `expected_smt2/qf_lia/lia_03_linear_ineq.smt2` expected `sat`
- [x] `qf_lia.lia_04_unsat_simple` → `awft/qf_lia/lia_04_unsat_simple.awft` → `expected_smt2/qf_lia/lia_04_unsat_simple.smt2` expected `unsat`
- [x] `qf_lia.lia_05_branch_bound` → `awft/qf_lia/lia_05_branch_bound.awft` → `expected_smt2/qf_lia/lia_05_branch_bound.smt2` expected `sat`
- [x] `qf_lia.lia_06_cutting_planes` → `awft/qf_lia/lia_06_cutting_planes.awft` → `expected_smt2/qf_lia/lia_06_cutting_planes.smt2` expected `unsat`
- [x] `qf_lia.lia_07_mixed_eq_ineq` → `awft/qf_lia/lia_07_mixed_eq_ineq.awft` → `expected_smt2/qf_lia/lia_07_mixed_eq_ineq.smt2` expected `sat`
- [x] `qf_lia.lia_08_negative_coeffs` → `awft/qf_lia/lia_08_negative_coeffs.awft` → `expected_smt2/qf_lia/lia_08_negative_coeffs.smt2` expected `sat`
- [x] `qf_lia.lia_09_large_coeffs` → `awft/qf_lia/lia_09_large_coeffs.awft` → `expected_smt2/qf_lia/lia_09_large_coeffs.smt2` expected `sat`
- [x] `qf_lia.lia_10_strict_ineq` → `awft/qf_lia/lia_10_strict_ineq.awft` → `expected_smt2/qf_lia/lia_10_strict_ineq.smt2` expected `sat`
- [x] `qf_lia.lia_11_unsat_bounds` → `awft/qf_lia/lia_11_unsat_bounds.awft` → `expected_smt2/qf_lia/lia_11_unsat_bounds.smt2` expected `unsat`
- [x] `qf_lia.lia_12_many_vars` → `awft/qf_lia/lia_12_many_vars.awft` → `expected_smt2/qf_lia/lia_12_many_vars.smt2` expected `sat`
- [x] `qf_lia.lia_13_divisibility` → `awft/qf_lia/lia_13_divisibility.awft` → `expected_smt2/qf_lia/lia_13_divisibility.smt2` expected `sat`
- [x] `qf_lia.lia_14_sparse` → `awft/qf_lia/lia_14_sparse.awft` → `expected_smt2/qf_lia/lia_14_sparse.smt2` expected `sat`
- [x] `qf_lia.lia_15_dense` → `awft/qf_lia/lia_15_dense.awft` → `expected_smt2/qf_lia/lia_15_dense.smt2` expected `sat`
- [x] `qf_lia.lia_16_edge_zero` → `awft/qf_lia/lia_16_edge_zero.awft` → `expected_smt2/qf_lia/lia_16_edge_zero.smt2` expected `sat`

### qf_lra (16)

- [x] `qf_lra.lra_01_simple` → `awft/qf_lra/lra_01_simple.awft` → `expected_smt2/qf_lra/lra_01_simple.smt2` expected `sat`
- [x] `qf_lra.lra_02_simplex` → `awft/qf_lra/lra_02_simplex.awft` → `expected_smt2/qf_lra/lra_02_simplex.smt2` expected `sat`
- [x] `qf_lra.lra_03_rational` → `awft/qf_lra/lra_03_rational.awft` → `expected_smt2/qf_lra/lra_03_rational.smt2` expected `sat`
- [x] `qf_lra.lra_04_strict_ineq` → `awft/qf_lra/lra_04_strict_ineq.awft` → `expected_smt2/qf_lra/lra_04_strict_ineq.smt2` expected `sat`
- [x] `qf_lra.lra_05_unsat_simple` → `awft/qf_lra/lra_05_unsat_simple.awft` → `expected_smt2/qf_lra/lra_05_unsat_simple.smt2` expected `unsat`
- [x] `qf_lra.lra_06_dense` → `awft/qf_lra/lra_06_dense.awft` → `expected_smt2/qf_lra/lra_06_dense.smt2` expected `sat`
- [x] `qf_lra.lra_07_sparse` → `awft/qf_lra/lra_07_sparse.awft` → `expected_smt2/qf_lra/lra_07_sparse.smt2` expected `sat`
- [x] `qf_lra.lra_08_negative` → `awft/qf_lra/lra_08_negative.awft` → `expected_smt2/qf_lra/lra_08_negative.smt2` expected `sat`
- [x] `qf_lra.lra_09_tight_bounds` → `awft/qf_lra/lra_09_tight_bounds.awft` → `expected_smt2/qf_lra/lra_09_tight_bounds.smt2` expected `sat`
- [x] `qf_lra.lra_10_infeasible` → `awft/qf_lra/lra_10_infeasible.awft` → `expected_smt2/qf_lra/lra_10_infeasible.smt2` expected `unsat`
- [x] `qf_lra.lra_11_many_vars` → `awft/qf_lra/lra_11_many_vars.awft` → `expected_smt2/qf_lra/lra_11_many_vars.smt2` expected `sat`
- [x] `qf_lra.lra_12_small_coeffs` → `awft/qf_lra/lra_12_small_coeffs.awft` → `expected_smt2/qf_lra/lra_12_small_coeffs.smt2` expected `sat`
- [x] `qf_lra.lra_13_large_coeffs` → `awft/qf_lra/lra_13_large_coeffs.awft` → `expected_smt2/qf_lra/lra_13_large_coeffs.smt2` expected `sat`
- [x] `qf_lra.lra_14_edge_zero` → `awft/qf_lra/lra_14_edge_zero.awft` → `expected_smt2/qf_lra/lra_14_edge_zero.smt2` expected `sat`
- [x] `qf_lra.lra_15_strict_boundary` → `awft/qf_lra/lra_15_strict_boundary.awft` → `expected_smt2/qf_lra/lra_15_strict_boundary.smt2` expected `sat`
- [x] `qf_lra.lra_16_mixed_strict` → `awft/qf_lra/lra_16_mixed_strict.awft` → `expected_smt2/qf_lra/lra_16_mixed_strict.smt2` expected `sat`

### qf_nia (1)

- [x] `qf_nia.nia_01_simple_mult` → `awft/qf_nia/nia_01_simple_mult.awft` → `expected_smt2/qf_nia/nia_01_simple_mult.smt2` expected `sat`

### qf_s (10)

- [x] `qf_s.string_01` → `awft/qf_s/string_01.awft` → `expected_smt2/qf_s/string_01.smt2` expected `sat`
- [x] `qf_s.string_02` → `awft/qf_s/string_02.awft` → `expected_smt2/qf_s/string_02.smt2` expected `unsat`
- [x] `qf_s.string_03` → `awft/qf_s/string_03.awft` → `expected_smt2/qf_s/string_03.smt2` expected `sat`
- [x] `qf_s.string_04` → `awft/qf_s/string_04.awft` → `expected_smt2/qf_s/string_04.smt2` expected `unsat`
- [x] `qf_s.string_05` → `awft/qf_s/string_05.awft` → `expected_smt2/qf_s/string_05.smt2` expected `sat`
- [x] `qf_s.string_06` → `awft/qf_s/string_06.awft` → `expected_smt2/qf_s/string_06.smt2` expected `sat`
- [x] `qf_s.string_07` → `awft/qf_s/string_07.awft` → `expected_smt2/qf_s/string_07.smt2` expected `sat`
- [x] `qf_s.string_08` → `awft/qf_s/string_08.awft` → `expected_smt2/qf_s/string_08.smt2` expected `unsat`
- [x] `qf_s.string_09` → `awft/qf_s/string_09.awft` → `expected_smt2/qf_s/string_09.smt2` expected `sat`
- [x] `qf_s.string_10` → `awft/qf_s/string_10.awft` → `expected_smt2/qf_s/string_10.smt2` expected `sat`
