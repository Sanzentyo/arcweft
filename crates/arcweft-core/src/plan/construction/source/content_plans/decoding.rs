//! Only actual function/state/content/group handles enter construction.

use super::super::{RuntimePlanSourceConstruction, expression::RuntimeExpressionSourceHandles};
use super::cursor::{Captures, Cursor};
use super::{Counts, Error, PreparedRuntimeContentPlanSources, charge};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{
    RuntimeDialogueContentPlanSeed, RuntimeDialogueContentPlanSeedId,
    RuntimeDialogueEffectSiteSeed, RuntimeDialogueValueSiteSeed, RuntimeExprSeed,
    RuntimeFunctionSiteSeedId, RuntimeLineTaskGroupSeedId,
};
use crate::runtime_id::RuntimeDialogueEffectSiteCount;
use crate::value::RuntimeExpr;

impl PreparedRuntimeContentPlanSources<'_> {
    pub(in crate::plan) fn materialize(
        &self,
        source: &mut RuntimePlanSourceConstruction,
        functions: &[RuntimeFunctionSiteSeedId],
        handles: &RuntimeExpressionSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<Box<[RuntimeDialogueContentPlanSeedId]>, Error> {
        source.meter.status()?;
        let result = (|| {
            handles.validate(source, self.limits.expression)?;
            namespace(
                "function",
                self.limits.function_count,
                source.builder.function_sites.len(),
            )?;
            namespace(
                "function handles",
                self.limits.function_count,
                functions.len(),
            )?;
            namespace("existing content", 0, source.builder.dialogue_content.len())?;
            source.meter.charge_work(functions.len() as u64)?;
            for (index, function) in functions.iter().enumerate() {
                if function.id().get().get() as usize != index + 1
                    || function.resolve(&source.builder.issuer).is_none()
                {
                    return Err(Error::Handle { kind: "function" });
                }
            }
            charge(&mut source.meter, self.bytes.len())?;
            let mut cursor = Cursor::new(self.bytes, self.limits);
            let n = cursor.header()?;
            let mut counts = Counts::new(self.limits);
            counts.rows(n)?;
            let mut result = Vec::with_capacity(n);
            for _ in 0..n {
                let row = cursor.row()?;
                row.preflight(&mut counts, resources, &mut source.meter)?;
                source
                    .builder
                    .require_dialogue_content_template(row.template)?;
                let line = row.line.materialize(self.limits)?;
                let mut values = Cursor::new(row.values, self.limits);
                let mut value_seeds = Vec::with_capacity(row.value_count);
                for index in 0..row.value_count {
                    let value = values.value(index)?;
                    value_seeds.push(RuntimeDialogueValueSiteSeed {
                        slot: value.slot,
                        role: value.role,
                        function: functions[value.function as usize - 1].clone(),
                        captures: self.captures(value.captures, source, handles, resources)?,
                    });
                }
                values.finish()?;
                let mut effects = Cursor::new(row.effects, self.limits);
                let mut effect_seeds = Vec::with_capacity(row.effect_count);
                for index in 0..row.effect_count {
                    let effect = effects.effect(index)?;
                    effect_seeds.push(RuntimeDialogueEffectSiteSeed {
                        site: effect.site,
                        state: handles.states[effect.state as usize - 1].clone(),
                        captures: self.captures(effect.captures, source, handles, resources)?,
                    });
                }
                effects.finish()?;
                let mut marks = Cursor::new(row.marks, self.limits);
                let mut mark_seeds = Vec::with_capacity(row.mark_count);
                for index in 0..row.mark_count {
                    mark_seeds.push(marks.mark(index)?.to_owned());
                }
                marks.finish()?;
                result.push(
                    source
                        .builder
                        .push_dialogue_content_seed(RuntimeDialogueContentPlanSeed {
                            line,
                            template: row.template,
                            values: value_seeds.into_boxed_slice(),
                            effect_sites: effect_seeds.into_boxed_slice(),
                            marks: mark_seeds.into_boxed_slice(),
                            effect_site_count: RuntimeDialogueEffectSiteCount::try_from_len(
                                row.effect_count,
                            )
                            .expect("checked count"),
                        })?,
                );
            }
            cursor.finish()?;
            debug_assert_eq!(counts.footprint, self.footprint);
            Ok(result.into_boxed_slice())
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
    fn captures(
        &self,
        captures: Captures<'_>,
        source: &mut RuntimePlanSourceConstruction,
        handles: &RuntimeExpressionSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<Box<[RuntimeExprSeed]>, Error> {
        // The aggregate frame was fully preflighted before any seed retention.
        // Replay each already bounded child under the same live work/value bank.
        let mut cursor = Cursor::new(captures.bytes, self.limits);
        let mut result = Vec::with_capacity(captures.count);
        for _ in 0..captures.count {
            let bytes = cursor.frame()?;
            let child = RuntimeExpr::preflight_sources(
                bytes,
                Counts::new(self.limits).child_limits(bytes.len()),
                resources,
                &mut source.meter,
            )?;
            result.push(child.materialize(source, handles, resources)?);
        }
        cursor.finish()?;
        Ok(result.into_boxed_slice())
    }
    /// Called after the original Line groups have entered their actual factory.
    /// The frame retains the source links; no independent attachment catalog exists.
    pub(in crate::plan) fn bind_line_groups(
        &self,
        source: &mut RuntimePlanSourceConstruction,
        contents: &[RuntimeDialogueContentPlanSeedId],
        groups: &[RuntimeLineTaskGroupSeedId],
    ) -> Result<(), Error> {
        source.meter.status()?;
        let result = (|| {
            let mut cursor = Cursor::new(self.bytes, self.limits);
            let n = cursor.header()?;
            namespace("content", n, source.builder.dialogue_content.len())?;
            namespace("content handles", n, contents.len())?;
            namespace(
                "Line groups",
                self.limits.group_count,
                source.builder.line_task_groups.len(),
            )?;
            namespace("Line group handles", self.limits.group_count, groups.len())?;
            source.meter.charge_work(
                u64::try_from(
                    contents
                        .len()
                        .checked_add(groups.len())
                        .ok_or(Error::Arithmetic)?,
                )
                .map_err(|_| Error::Arithmetic)?,
            )?;
            for (index, handle) in contents.iter().enumerate() {
                if handle
                    .resolve(&source.builder.issuer)
                    .is_none_or(|id| id.get().get() as usize != index + 1)
                {
                    return Err(Error::Handle { kind: "content" });
                }
            }
            for (index, handle) in groups.iter().enumerate() {
                if handle
                    .resolve(&source.builder.issuer)
                    .is_none_or(|id| id.index() != index)
                {
                    return Err(Error::Handle { kind: "Line group" });
                }
            }
            charge(&mut source.meter, self.bytes.len())?;
            for handle in contents {
                let row = cursor.row()?;
                if let Some(group) = row.group {
                    source
                        .builder
                        .attach_line_task_group_seed(handle, &groups[group as usize - 1])?;
                }
            }
            cursor.finish()
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}
fn namespace(kind: &'static str, expected: usize, actual: usize) -> Result<(), Error> {
    if expected != actual {
        return Err(Error::Namespace {
            kind,
            expected,
            actual,
        });
    }
    Ok(())
}
