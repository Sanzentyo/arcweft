//! The existing owning factory still checks condition types and body scope.

use super::super::{RuntimePlanSourceConstruction, expression::RuntimeExpressionSourceHandles};
use super::cursor::{Cursor, Kind};
use super::{Counts, Error, PreparedRuntimeEvaluatedEffectSource, charge};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{
    RuntimeDropPolicySeed as Policy, RuntimeEffectFieldSeed, RuntimeEvaluatedEffectSeed as Effect,
    RuntimeExprSeed,
};
use crate::value::RuntimeExpr;

impl PreparedRuntimeEvaluatedEffectSource<'_> {
    pub(in crate::plan) fn materialize(
        &self,
        source: &mut RuntimePlanSourceConstruction,
        handles: &RuntimeExpressionSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<Effect, Error> {
        source.meter.status()?;
        let result = (|| {
            handles.validate(source, self.limits.expression)?;
            charge(&mut source.meter, self.bytes.len())?;
            let mut cursor = Cursor::new(self.bytes);
            let row = cursor.row()?;
            cursor.finish()?;
            let mut counts = Counts::new(self.limits)?;
            row.preflight(&mut counts, resources, &mut source.meter)?;
            debug_assert_eq!(counts.footprint, self.footprint);
            let first = self.expression(row.first, source, handles, resources)?;
            let second = if let Some(bytes) = row.second {
                Some(self.expression(bytes, source, handles, resources)?)
            } else {
                None
            };
            let mut fields = Vec::with_capacity(row.field_count);
            let mut cursor = Cursor::new(row.fields);
            for _ in 0..row.field_count {
                fields.push(RuntimeEffectFieldSeed {
                    name: cursor.text()?.to_owned(),
                    value: self.expression(cursor.frame()?, source, handles, resources)?,
                });
            }
            cursor.finish()?;
            Ok(match row.kind {
                Kind::Log(level) => Effect::Log {
                    level: level.to_owned(),
                    message: first,
                    fields,
                },
                Kind::Signal => Effect::SignalWrite {
                    target: first,
                    value: second.expect("checked second expression"),
                },
                Kind::Metric => Effect::MetricWrite {
                    target: first,
                    value: second.expect("checked second expression"),
                },
                Kind::Event => Effect::EmitEvent {
                    event: first,
                    fields,
                },
                Kind::Panic => Effect::Panic(first),
                Kind::Fail => Effect::Fail(first),
                Kind::Bail => Effect::Bail(first),
                Kind::Ensure => Effect::Ensure {
                    condition: first,
                    message: second.expect("checked second expression"),
                },
                Kind::Drop(policy) => Effect::Drop {
                    target: first,
                    policy: match policy {
                        0 => Policy::Default,
                        1 => Policy::Cancel,
                        2 => Policy::Stop {
                            fade: second.expect("checked fade"),
                        },
                        3 => Policy::Finish,
                        4 => Policy::Release,
                        5 => Policy::Detach,
                        _ => unreachable!("checked policy tag"),
                    },
                },
                Kind::Assert {
                    guard,
                    profile,
                    message,
                } => Effect::Assert {
                    guard,
                    condition: first,
                    message: message.to_owned(),
                    profile,
                },
            })
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
    fn expression(
        &self,
        bytes: &[u8],
        source: &mut RuntimePlanSourceConstruction,
        handles: &RuntimeExpressionSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<RuntimeExprSeed, Error> {
        // The entire effect and aggregate children were admitted before retention.
        let child = RuntimeExpr::preflight_sources(
            bytes,
            Counts::new(self.limits)?.expression(bytes.len()),
            resources,
            &mut source.meter,
        )?;
        Ok(child.materialize(source, handles, resources)?)
    }
}
