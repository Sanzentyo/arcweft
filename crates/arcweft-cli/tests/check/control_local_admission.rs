mod control_local_admission {
    #[test]
    fn array_map_rejects_the_complete_local_inventory_before_expanding_rows() {
        use arcweft_lang_sema::final_analysis::{
            CheckedExecutionBodyOwner, CheckedExecutionSource,
        };
        let compiled = arcweft_compiler::source::compile_source(
            "pub fn root(values: Array<i64, 2147483648>) -> Array<i64, 2147483648> { values.map(|item| item) }\nflow main() -> String { return \"ok\" }\n",
        ).unwrap();
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
        let error = lease
            .compile_deterministic_program(
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                    declaration,
                    role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
                }),
                None,
                &arcweft_compiler::lower::ProjectInstantiationControl::default(),
            )
            .expect_err("the source Map requires more than the complete local namespace");
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains("runtime local-declaration identity space is exhausted"),
            "{diagnostic}"
        );
    }
}
