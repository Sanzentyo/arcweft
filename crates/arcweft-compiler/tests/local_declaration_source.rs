use arcweft_compiler::source::compile_source;
use arcweft_core::plan::{
    RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind as Kind,
    RuntimeLocalBindingStorage as Storage,
};

fn binding_sources(source: &str) -> Vec<RuntimeLocalBindingDeclaration> {
    compile_source(source)
        .expect("authored declarations compile")
        .plan
        .local_declarations()
        .declarations()
        .filter_map(|row| row.source().binding())
        .collect()
}

#[test]
fn authored_shadowing_keeps_distinct_declared_mutability_in_core_rows() {
    let declarations = binding_sources(
        "flow main() -> i64 { let mut value = 1i64; let value = 2i64; return value }",
    );
    let lets: Vec<_> = declarations
        .iter()
        .filter(|d| d.kind() == Kind::LetBinding)
        .collect();
    assert_eq!(lets.len(), 2);
    assert_eq!(lets.iter().filter(|d| d.is_mutable()).count(), 1);
    assert!(lets.iter().all(|d| d.storage() == Storage::Derived));
}

#[test]
fn authored_retained_storage_survives_view_lowering_to_core_rows() {
    let declarations = binding_sources(
        r#"view Main(initial: String) { local state caption: String = initial; let suffix = "!"; Text(caption) }"#,
    );
    let retained: Vec<_> = declarations
        .iter()
        .filter(|d| d.storage() == Storage::RetainedState)
        .collect();
    assert!(
        !retained.is_empty(),
        "retained binding survives source admission"
    );
    assert!(retained.iter().all(|d| d.kind() == Kind::LetBinding));
    assert!(
        declarations
            .iter()
            .any(|d| d.kind() == Kind::LetBinding && d.storage() == Storage::Derived)
    );
}

#[test]
fn authored_closure_capture_keeps_the_captured_declaration_source() {
    let compiled = compile_source(
        "flow main() -> i64 { let mut value = 7i64; let compute = || value; return compute() }",
    )
    .expect("capturing closure compiles");
    let declarations: Vec<_> = compiled
        .plan
        .function_sites()
        .iter()
        .filter(|site| site.role() == arcweft_core::plan::RuntimeFunctionSemanticRole::Closure)
        .flat_map(arcweft_core::plan::RuntimeFunctionSite::inputs)
        .filter(|input| {
            matches!(
                input.source(),
                arcweft_core::plan::RuntimeFunctionInputSource::Capture { .. }
            )
        })
        .flat_map(|input| input.pattern().binding_declarations())
        .map(|binding| {
            compiled
                .plan
                .local_declarations()
                .get(binding.local())
                .unwrap()
                .source()
                .binding()
                .unwrap()
        })
        .collect();
    assert_eq!(declarations.len(), 1, "one captured declaration input");
    assert!(
        declarations
            .iter()
            .any(|d| d.kind() == Kind::LetBinding && d.is_mutable())
    );
    assert!(declarations.iter().all(|d| d.storage() == Storage::Derived));
}

fn referenced_slots(
    plan: &arcweft_core::plan::RuntimePlan,
) -> std::collections::BTreeSet<arcweft_core::runtime_id::RuntimeLocalDeclarationId> {
    use arcweft_core::plan::RuntimeFunctionSiteBody;
    use arcweft_core::value::{RuntimeExprKind, RuntimeExpressionNode as Node};
    let mut used = std::collections::BTreeSet::new();
    let mut visit = |root: Node<'_>| {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            match node {
                Node::Pattern(pattern) => {
                    used.extend(
                        pattern
                            .binding_declarations()
                            .map(arcweft_core::pattern::RuntimePatternBindingDeclaration::local),
                    );
                }
                Node::Expression(expression) => match expression.kind() {
                    RuntimeExprKind::Local(read) => {
                        used.insert(read.local());
                    }
                    RuntimeExprKind::Let { binding, .. } => {
                        used.insert(*binding);
                    }
                    _ => {}
                },
            }
            pending.extend(node.owned_children().map(|(_, child)| child));
        }
    };
    for site in plan.function_sites().iter() {
        for input in site.inputs() {
            visit(Node::Pattern(input.pattern()));
        }
        if let RuntimeFunctionSiteBody::Expression(body) = site.body() {
            visit(Node::Expression(body));
        }
    }
    plan.visit_flow_ops(&mut |operation| {
        operation
            .try_visit_value_roots(&mut |_, root| {
                visit(root);
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
    });
    for site in plan.function_sites().iter() {
        used.extend(
            site.inputs()
                .iter()
                .map(arcweft_core::plan::RuntimeFunctionInputBinding::input_local),
        );
    }
    assert!(
        used.iter()
            .all(|local| plan.local_declarations().contains(*local))
    );
    used
}

#[test]
fn closed_program_and_function_frames_do_not_leave_global_template_slots() {
    let mut mismatches = Vec::new();
    for source in [
        r#"view Main(initial: String) { local state caption: String = initial; let suffix = "!"; Text(caption) }"#,
        "fn identity<T>(input: T) -> T { let value = input; value }\nflow main() -> i64 { return identity(7i64) }",
        "flow main() -> i64 { let mut value = 7i64; let compute = || value; return compute() }",
    ] {
        let compiled = compile_source(source).expect("owned frame fixture compiles");
        assert!(compiled.plan.pure_helpers().is_empty());
        assert!(compiled.plan.trait_methods().is_empty());
        assert!(compiled.plan.line_task_groups().is_empty());
        assert!(compiled.plan.stream_plans().is_empty());
        let used = referenced_slots(&compiled.plan);
        if compiled.plan.local_declarations().len() != used.len() {
            let unused: Vec<_> = compiled
                .plan
                .local_declarations()
                .declarations()
                .enumerate()
                .filter(|(ordinal, _)| {
                    !used
                        .iter()
                        .any(|local| local.get().get() as usize == ordinal + 1)
                })
                .map(|(ordinal, row)| (ordinal, row.ty(), row.source()))
                .collect();
            mismatches.push((
                source,
                compiled.plan.local_declarations().len(),
                used.len(),
                unused,
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "unreferenced actual slot rows: {mismatches:#?}"
    );
}
