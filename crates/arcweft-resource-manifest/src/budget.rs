use crate::{JsonPath, ResourceManifestDecodeLimits, ResourceManifestDiagnosticCode};
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BudgetError {
    pub(crate) code: ResourceManifestDiagnosticCode,
    pub(crate) message: String,
    pub(crate) path: JsonPath,
}

pub(crate) struct DecodeBudget {
    limits: ResourceManifestDecodeLimits,
    semantic_records: usize,
    work_units: u64,
}

impl DecodeBudget {
    pub(crate) const fn new(limits: ResourceManifestDecodeLimits) -> Self {
        Self {
            limits,
            semantic_records: 0,
            work_units: 0,
        }
    }

    pub(crate) fn charge_lexical_revisit(
        &mut self,
        value: &Value,
        path: &JsonPath,
    ) -> Result<(), BudgetError> {
        self.charge_work(1, path, "lexical value revisit")?;
        match value {
            Value::Array(values) => values.iter().enumerate().try_for_each(|(index, value)| {
                self.charge_lexical_revisit(value, &path.index(index))
            }),
            Value::Object(values) => values.iter().try_for_each(|(name, value)| {
                let field = path.field(name);
                self.charge_work(1, &field, "lexical object-key revisit")?;
                self.charge_lexical_revisit(value, &field)
            }),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Ok(()),
        }
    }

    pub(crate) fn charge_typed(&mut self, count: u64, path: &JsonPath) -> Result<(), BudgetError> {
        self.charge_work(count, path, "typed value construction")
    }

    pub(crate) fn charge_edge(&mut self, path: &JsonPath) -> Result<(), BudgetError> {
        self.charge_work(1, path, "semantic validation edge")
    }

    pub(crate) fn charge_record(&mut self, path: &JsonPath) -> Result<(), BudgetError> {
        self.semantic_records = self.semantic_records.checked_add(1).ok_or_else(|| {
            Self::error(
                ResourceManifestDiagnosticCode::RecordLimit,
                "semantic record count overflowed",
                path,
            )
        })?;
        if self.semantic_records > self.limits.semantic_records() {
            return Err(Self::error(
                ResourceManifestDiagnosticCode::RecordLimit,
                format!(
                    "manifest semantic record count exceeds {}",
                    self.limits.semantic_records()
                ),
                path,
            ));
        }
        self.charge_work(1, path, "semantic record")
    }

    pub(crate) fn charge_collection(
        &mut self,
        len: usize,
        path: &JsonPath,
    ) -> Result<(), BudgetError> {
        self.charge_work(
            u64::try_from(len).unwrap_or(u64::MAX),
            path,
            "collection elements",
        )
    }

    pub(crate) fn charge_sort(&mut self, len: usize, path: &JsonPath) -> Result<(), BudgetError> {
        let n = u64::try_from(len).unwrap_or(u64::MAX);
        let levels = u64::from(n.max(2).ilog2() + u32::from(!n.max(2).is_power_of_two()));
        let work = n.saturating_mul(levels);
        self.charge_work(work, path, "semantic collection sort")
    }

    pub(crate) fn charge_bytes(
        &mut self,
        bytes: usize,
        path: &JsonPath,
        operation: &'static str,
    ) -> Result<(), BudgetError> {
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        let units = bytes.checked_add(63).map_or(u64::MAX, |value| value / 64);
        self.charge_work(units, path, operation)
    }

    fn charge_work(
        &mut self,
        units: u64,
        path: &JsonPath,
        operation: &'static str,
    ) -> Result<(), BudgetError> {
        let Some(next) = self.work_units.checked_add(units) else {
            return Err(Self::error(
                ResourceManifestDiagnosticCode::WorkLimit,
                format!("deterministic work overflow before {operation}"),
                path,
            ));
        };
        if next > self.limits.work_units() {
            return Err(Self::error(
                ResourceManifestDiagnosticCode::WorkLimit,
                format!(
                    "deterministic work before {operation} exceeds {} units",
                    self.limits.work_units()
                ),
                path,
            ));
        }
        self.work_units = next;
        Ok(())
    }

    fn error(
        code: ResourceManifestDiagnosticCode,
        message: impl Into<String>,
        path: &JsonPath,
    ) -> BudgetError {
        BudgetError {
            code,
            message: message.into(),
            path: path.clone(),
        }
    }
}
