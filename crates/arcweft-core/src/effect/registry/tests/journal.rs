use super::*;

fn key() -> LifetimeRegistryKey {
    LifetimeRegistryKey::new(
        LifetimeScopeKind::Flow,
        [DeclarationName::try_new("seen").unwrap()].into(),
    )
}

#[test]
fn registry_update_journal_keeps_one_aggregate_bank_and_publishes_no_failed_prefix() {
    for owner in owners() {
        let limits = RuntimeSchemaLimits {
            max_encoded_bytes: 92,
            ..RuntimeSchemaLimits::engine_default()
        };
        let mut journal = RuntimeRegistryUpdateJournal::new(owner, limits).unwrap();
        journal
            .try_push(&key(), semantic(1), None, &RuntimeValue::Bool(true))
            .unwrap();
        journal
            .try_push(&key(), semantic(1), None, &RuntimeValue::Bool(false))
            .unwrap();
        let error = journal
            .try_push(&key(), semantic(1), None, &RuntimeValue::Bool(true))
            .unwrap_err();
        assert_eq!(
            error,
            RuntimeRegistryUpdateJournalError::Value(AwbcRuntimeValueSnapshotError::Resource(
                RuntimeSchemaError::BudgetExceeded {
                    budget: "encoded_bytes",
                }
            ))
        );
        assert_eq!(
            journal.try_push(&key(), semantic(1), None, &RuntimeValue::Bool(false)),
            Err(error.clone())
        );
        assert_eq!(journal.finish().unwrap_err(), error);
    }
}

#[test]
fn registry_update_journal_retains_the_first_type_failure_across_later_appends() {
    for owner in owners() {
        let mut journal =
            RuntimeRegistryUpdateJournal::new(owner, RuntimeSchemaLimits::engine_default())
                .unwrap();
        journal
            .try_push(&key(), semantic(1), None, &RuntimeValue::Bool(true))
            .unwrap();
        let error = journal
            .try_push(
                &key(),
                semantic(1),
                None,
                &RuntimeValue::String("wrong".to_owned()),
            )
            .unwrap_err();
        assert_eq!(
            journal.try_push(&key(), semantic(1), None, &RuntimeValue::Bool(false)),
            Err(error.clone())
        );
        assert_eq!(journal.finish().unwrap_err(), error);
    }
}

#[test]
fn registry_update_journal_finishes_immutable_packets_under_the_exact_program() {
    for owner in owners() {
        let mut journal =
            RuntimeRegistryUpdateJournal::new(owner.clone(), RuntimeSchemaLimits::engine_default())
                .unwrap();
        journal
            .try_push(&key(), semantic(1), None, &RuntimeValue::Bool(true))
            .unwrap();
        journal
            .try_push(&key(), semantic(1), None, &RuntimeValue::Bool(false))
            .unwrap();
        fn require_send<T: Send>(_: &T) {}
        require_send(&journal);
        let updates = journal.finish().unwrap();
        assert_eq!(updates.len(), 2);
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner,
            RuntimeSchemaLimits::engine_default(),
        );
        assert_eq!(
            updates[0].materialize(&mut admission).unwrap(),
            RuntimeValue::Bool(true)
        );
        assert_eq!(
            updates[1].materialize(&mut admission).unwrap(),
            RuntimeValue::Bool(false)
        );
    }
}
