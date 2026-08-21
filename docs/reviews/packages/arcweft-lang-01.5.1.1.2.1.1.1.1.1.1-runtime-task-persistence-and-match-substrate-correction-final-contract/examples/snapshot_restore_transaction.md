# Example: strict restore and one atomic publication

```rust
pub fn restore(
    &mut self,
    bytes: &[u8],
) -> Result<(), RuntimeTaskRestoreError> {
    // Direct borrowed reader; each length is checked before allocation.
    let decoded = RuntimeTaskSnapshotCodecV1::decode(bytes, self.config.snapshot_limits())?;

    // Private temporary owner. No field in `self` is changed yet.
    let mut candidate = RuntimeTaskSchedulerCandidate::from_snapshot(decoded)?;

    candidate.validate_versions_exactly_one()?;
    candidate.validate_sorted_unique_keys()?;
    candidate.rederive_semantic_digests()?;
    candidate.rederive_all_correlations()?;
    candidate.validate_groups_launches_needs_observers()?;
    candidate.validate_runtime_task_state()?;
    candidate.validate_event_cursors_and_terminals()?;
    candidate.validate_embedded_handles()?;
    candidate.validate_replacement_rows()?;
    candidate.validate_no_prepared_adapter_tokens()?;

    // Adapter restore preparation is allowed only when it returns an owned
    // token and all remaining work is infallible.
    let prepared = self.adapter.prepare_restore(candidate.host_projection())
        .map_err(RuntimeTaskRestoreError::AdapterPrepare)?;

    let published = candidate.into_published_state(); // infallible conversion
    let old = std::mem::replace(&mut self.state, published);
    self.adapter.commit_restore(prepared);             // -> ()
    drop(old);
    Ok(())
}
```

The concrete implementation may swap the complete scheduler rather than one
`state` field, but the transaction cut is the same. Decode, all rederivation,
all joins, and adapter reservation happen before the first irreversible
publication. A failure retains the old scheduler byte-for-byte.
