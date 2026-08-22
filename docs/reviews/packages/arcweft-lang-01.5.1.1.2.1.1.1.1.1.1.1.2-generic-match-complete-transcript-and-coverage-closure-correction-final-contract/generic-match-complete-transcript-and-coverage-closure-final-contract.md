
## 14. Executable invariant appendix

The canonical transcript is the sole semantic input to coverage closure. Coverage closure is the sole exhaustiveness authority admitted by task-plan seal. Seal binds the canonical transcript digest, coverage digest, and runtime decision-program digest. Runtime admission resolves that sealed artifact; snapshot restore verifies the same canonical transcript and coverage closure before coordinator publication. No restore path reconstructs admission from an incomplete transcript.

```rust
impl CoverageClosedGenericMatch {
    pub fn into_sealed_child(
        self,
        encoder: &mut TaskPlanSemanticChildEncoder,
    ) -> Result<SealedGenericMatch, TaskPlanSealError> {
        self.verify_transcript_complete()?;
        self.verify_coverage_closed()?;
        self.verify_canonical_identity()?;
        encoder.seal_generic_match(self)
    }
}
```

```rust
impl MatchCoverageClosure {
    pub fn admits_exhaustive_context(&self) -> bool {
        matches!(self.status, CoverageClosureStatus::Exhaustive)
            && self.residual.is_empty()
    }
}
```

```rust
impl PersistedGenericMatchRefV1 {
    pub fn resolve(
        &self,
        plans: &RestoredTaskPlanCatalog,
    ) -> Result<Arc<SealedGenericMatch>, SnapshotRestoreError>;
}
```

These APIs make canonical transcript completion, coverage closure, admission, seal, digest identity, and restore verification explicit owner-local gates.

## 14. Executable invariant appendix

The canonical transcript is the sole semantic input to coverage closure. Coverage closure is the sole exhaustiveness authority admitted by task-plan seal. Seal binds the canonical transcript digest, coverage digest, and runtime decision-program digest. Runtime admission resolves that sealed artifact; snapshot restore verifies the same canonical transcript and coverage closure before coordinator publication. No restore path reconstructs admission from an incomplete transcript.

```rust
impl CoverageClosedGenericMatch {
    pub fn into_sealed_child(
        self,
        encoder: &mut TaskPlanSemanticChildEncoder,
    ) -> Result<SealedGenericMatch, TaskPlanSealError> {
        self.verify_transcript_complete()?;
        self.verify_coverage_closed()?;
        self.verify_canonical_identity()?;
        encoder.seal_generic_match(self)
    }
}
```

```rust
impl MatchCoverageClosure {
    pub fn admits_exhaustive_context(&self) -> bool {
        matches!(self.status, CoverageClosureStatus::Exhaustive)
            && self.residual.is_empty()
    }
}
```

```rust
impl PersistedGenericMatchRefV1 {
    pub fn resolve(
        &self,
        plans: &RestoredTaskPlanCatalog,
    ) -> Result<Arc<SealedGenericMatch>, SnapshotRestoreError>;
}
```

These APIs make canonical transcript completion, coverage closure, admission, seal, digest identity, and restore verification explicit owner-local gates.

## 14. Executable invariant appendix

The canonical transcript is the sole semantic input to coverage closure. Coverage closure is the sole exhaustiveness authority admitted by task-plan seal. Seal binds the canonical transcript digest, coverage digest, and runtime decision-program digest. Runtime admission resolves that sealed artifact; snapshot restore verifies the same canonical transcript and coverage closure before coordinator publication. No restore path reconstructs admission from an incomplete transcript.

```rust
impl CoverageClosedGenericMatch {
    pub fn into_sealed_child(
        self,
        encoder: &mut TaskPlanSemanticChildEncoder,
    ) -> Result<SealedGenericMatch, TaskPlanSealError> {
        self.verify_transcript_complete()?;
        self.verify_coverage_closed()?;
        self.verify_canonical_identity()?;
        encoder.seal_generic_match(self)
    }
}
```

```rust
impl MatchCoverageClosure {
    pub fn admits_exhaustive_context(&self) -> bool {
        matches!(self.status, CoverageClosureStatus::Exhaustive)
            && self.residual.is_empty()
    }
}
```

```rust
impl PersistedGenericMatchRefV1 {
    pub fn resolve(
        &self,
        plans: &RestoredTaskPlanCatalog,
    ) -> Result<Arc<SealedGenericMatch>, SnapshotRestoreError>;
}
```

These APIs make canonical transcript completion, coverage closure, admission, seal, digest identity, and restore verification explicit owner-local gates.
