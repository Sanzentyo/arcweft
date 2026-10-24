mod control_local_admission {
    #[test]
    fn array_map_admits_extreme_length_with_constant_local_inventory() {
        use arcweft_core::plan::RuntimePlanTypeProjection;
        use arcweft_lang_sema::final_analysis::{
            CheckedExecutionBodyOwner, CheckedExecutionSource,
        };

        let mut inventory = None;
        for length in [0_u64, 1, 4_096, 2_147_483_648] {
            let source = format!(
                "pub fn root(values: Array<i64, {length}>) -> Array<i64, {length}> {{ values.map(|item| item) }}\nflow main() -> String {{ return \"ok\" }}\n"
            );
            let compiled = arcweft_compiler::source::compile_source(&source).unwrap();
            let lease = &compiled.analysis;
            let declaration = lease
                .final_analysis()
                .hir_topology()
                .modules()
                .iter()
                .flat_map(arcweft_lang_hir::project::HirModuleEvaluationTopology::entries)
                .filter_map(|entry| entry.body())
                .find(|body| body.declaration().name() == "root")
                .unwrap()
                .declaration()
                .clone();
            let selected = lease
                .compile_deterministic_program(
                    CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                        declaration,
                        role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
                    }),
                    None,
                    &arcweft_compiler::lower::ProjectInstantiationControl::default(),
                )
                .expect("an Array Map loop admits its fixed inventory without allocating rows");
            let plan = selected.plan();
            let root = plan
                .function_sites()
                .get(selected.function_site().unwrap())
                .unwrap();
            let RuntimePlanTypeProjection::Array {
                length: result_length,
                ..
            } = plan.type_table().get(root.result()).unwrap().projection()
            else {
                panic!("Map must preserve its exact Array result type");
            };
            assert_eq!(result_length.constant(), Some(length));
            let actual = (plan.local_declarations().len(), plan.function_sites().len());
            assert!(actual.0 > 0);
            if let Some(expected) = inventory.replace(actual) {
                assert_eq!(
                    actual, expected,
                    "Array length {length} must not expand local or callback declarations"
                );
            }
        }
    }
}
