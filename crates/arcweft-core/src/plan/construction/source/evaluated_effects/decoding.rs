//! The existing owning factory still checks condition types and body scope.

use super::super::{RuntimePlanSourceConstruction, expression::RuntimeExpressionSourceHandles};
use super::cursor::{Cursor, Kind};
use super::{Counts, Error, PreparedRuntimeEvaluatedEffectSource, charge};
use crate::effect::{RuntimeDropPolicyKind, RuntimeRegistryDropTarget};
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
            let first = row
                .first
                .map(|bytes| self.expression(bytes, source, handles, resources))
                .transpose()?;
            let operand = |value: Option<RuntimeExprSeed>| {
                value.expect("the checked effect has one first operand")
            };
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
                Kind::Wait => Effect::Wait(operand(first)),
                Kind::Log(level) => Effect::Log {
                    level: level.to_owned(),
                    message: operand(first),
                    fields,
                },
                Kind::Signal => Effect::SignalWrite {
                    target: operand(first),
                    value: second.expect("checked second expression"),
                },
                Kind::Metric => Effect::MetricWrite {
                    target: operand(first),
                    value: second.expect("checked second expression"),
                },
                Kind::Registry(key) => Effect::RegistryWrite {
                    key: key.materialize()?,
                    value: operand(first),
                },
                Kind::Event => Effect::EmitEvent {
                    event: operand(first),
                    fields,
                },
                Kind::Action => Effect::EmitAction {
                    payload: operand(first),
                },
                Kind::Panic => Effect::Panic(operand(first)),
                Kind::Fail => Effect::Fail(operand(first)),
                Kind::Bail => Effect::Bail(operand(first)),
                Kind::Ensure => Effect::Ensure {
                    condition: operand(first),
                    message: second.expect("checked second expression"),
                },
                Kind::Drop(policy) => Effect::Drop {
                    target: operand(first),
                    policy: drop_policy(policy, second),
                },
                Kind::RegistryDrop {
                    key,
                    presence,
                    item_type,
                    policy,
                } => Effect::RegistryDrop {
                    target: RuntimeRegistryDropTarget::new(key.materialize()?, presence, item_type),
                    policy: drop_policy(policy, first),
                },

                Kind::Assert {
                    guard,
                    profile,
                    message,
                } => Effect::Assert {
                    guard,
                    condition: operand(first),
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

fn drop_policy(kind: RuntimeDropPolicyKind, fade: Option<RuntimeExprSeed>) -> Policy {
    match kind {
        RuntimeDropPolicyKind::Default => Policy::Default,
        RuntimeDropPolicyKind::Cancel => Policy::Cancel,
        RuntimeDropPolicyKind::Stop => Policy::Stop {
            fade: fade.expect("the checked policy owns a fade"),
        },
        RuntimeDropPolicyKind::Finish => Policy::Finish,
        RuntimeDropPolicyKind::Release => Policy::Release,
        RuntimeDropPolicyKind::Detach => Policy::Detach,
    }
}
