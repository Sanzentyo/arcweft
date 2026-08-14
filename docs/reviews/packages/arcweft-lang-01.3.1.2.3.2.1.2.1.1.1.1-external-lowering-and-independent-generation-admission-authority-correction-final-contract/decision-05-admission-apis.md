# Decision 05 — admission consumes an issued generation

The standalone methods `RuntimePlan::try_admit(self)` and
`AwbcProgram::try_admit(self)` do not exist. The only raw admission entry points
are methods on an independently issued generation:

```rust
impl AdmittedRuntimeGeneration {
    pub fn try_admit_plan(
        &self,
        plan: RuntimePlan,
    ) -> Result<AdmittedRuntimePlan, RuntimePlanAdmissionError>;

    pub fn try_admit_awbc(
        &self,
        program: AwbcProgram,
    ) -> Result<AdmittedAwbcProduct, AwbcAdmissionError>;
}

impl AdmittedRuntimePlan {
    pub fn try_admit_awbc(
        self,
        program: AwbcProgram,
    ) -> Result<AdmittedRuntimeProduct, RuntimeProductAdmissionError>;
}
```

Each raw artifact's mandatory generation declaration is compared to the
existing generation; it is never converted into facts. Plan admission resolves
all declarations/sites against generation facts. Pair admission uses the plan's
exact Arc parent and directly compares coordinate origins. The compiler's only
convenience is a full accepted-project operation that first builds/validates the
accepted world and issues the generation; there is no raw-only convenience.
