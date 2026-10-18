use super::*;

fn registry_fixture(
    label: &str,
) -> (
    HirProject,
    RuntimePlanSemanticFactInput,
    super::super::RuntimeRegistryWriteFact,
) {
    let project = project_fixture(
        label,
        "flow registry effects { state.write('flow) } { 'flow.flags.seen <- true }\n",
    );
    let analysis = analyze_identity_fixture(&project);
    let checked = analysis
        .statements()
        .find_map(|(_, statement)| match statement.payload() {
            arcweft_lang_sema::final_analysis::CheckedStatementPayload::RegistryWrite(write) => {
                Some(write.as_ref().clone())
            }
            _ => None,
        })
        .expect("original checked registry write");
    let boolean = super::super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes(
            *checked
                .value_type()
                .semantic_identity_digest()
                .expect("original Bool type")
                .as_bytes(),
        ),
        RuntimeTypeShape::Bool,
    );
    let mut input = complete_type_input(&project);
    let value = input
        .expression_facts
        .iter_mut()
        .find(|(owner, _)| *owner == checked.value())
        .expect("original checked value");
    value.1.ty = Some(boolean.clone());
    let write = super::super::RuntimeRegistryWriteFact::new(checked, boolean);
    (project, input, write)
}

#[test]
fn registry_write_fact_requires_complete_unique_non_value_address_admission() {
    let (project, mut input, write) = registry_fixture("registry-write-accepted");
    let statement = write.address().statement();
    let target = write.address().target();
    input.push_registry_write(write.clone());
    let facts = runtime_facts(&project, input).expect("complete original write");
    assert_eq!(facts.registry_write(statement), Some(&write));
    assert!(
        facts.expression_type(target).is_none(),
        "address metadata has no value domain"
    );
    assert_eq!(
        facts.expression_type(write.value()),
        Some(write.value_type())
    );
    assert_eq!(
        write.address().effect().as_str(),
        "state.write('flow.flags.seen)"
    );

    let (project, input, write) = registry_fixture("registry-write-missing");
    assert_eq!(
        runtime_facts(&project, input).unwrap_err(),
        RuntimeSemanticFactsError::MissingRegistryWriteFact {
            statement: write.address().statement()
        }
    );

    let (project, mut input, write) = registry_fixture("registry-write-duplicate");
    input.push_registry_write(write.clone());
    input.push_registry_write(write);
    assert_eq!(
        runtime_facts(&project, input).unwrap_err(),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::RegistryWrite
        }
    );
}

#[test]
fn registry_write_fact_rejects_another_value_domain_and_source_generation() {
    let (project, mut input, write) = registry_fixture("registry-write-wrong-type");
    let statement = write.address().statement();
    let checked = analyze_identity_fixture(&project)
        .statement(statement)
        .expect("checked source")
        .payload()
        .clone();
    let arcweft_lang_sema::final_analysis::CheckedStatementPayload::RegistryWrite(checked) =
        checked
    else {
        panic!("checked write")
    };
    input.push_registry_write(super::super::RuntimeRegistryWriteFact::new(
        *checked,
        unit_type(),
    ));
    assert_eq!(
        runtime_facts(&project, input).unwrap_err(),
        RuntimeSemanticFactsError::InvalidRegistryWriteFact { statement }
    );

    let (project, mut input, write) = registry_fixture("registry-write-own-generation");
    let (_, _, foreign) = registry_fixture("registry-write-foreign-generation");
    let statement = write.address().statement();
    input.registry_writes.push((statement, foreign));
    assert_eq!(
        runtime_facts(&project, input).unwrap_err(),
        RuntimeSemanticFactsError::InvalidRegistryWriteFact { statement }
    );
}
