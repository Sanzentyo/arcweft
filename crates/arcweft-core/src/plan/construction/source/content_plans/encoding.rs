//! Original application rows and their registered declarations are borrowed.
//! Two counted passes use the existing expression owner and original work bank.

use super::{
    Counts, DOMAIN, Error, Footprint, RuntimeContentPlanSourceLimits, charge, limit, reference,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::RuntimePlan;
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath};
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeExpr;

pub(in crate::plan) struct EncodedRuntimeContentPlanSources {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeContentPlanSources {
    pub(in crate::plan) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(in crate::plan) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
    }
    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.bytes.len() {
            return Err(Error::OutputCapacity {
                required: self.bytes.len(),
                remaining,
            });
        }
        output.extend_from_slice(&self.bytes);
        Ok(())
    }
}
impl RuntimePlan {
    pub(in crate::plan) fn encode_content_plan_sources(
        &self,
        limits: RuntimeContentPlanSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeContentPlanSources, Error> {
        meter.status()?;
        let result = (|| {
            for (kind, expected, actual) in [
                (
                    "function",
                    limits.function_count,
                    self.function_sites().len(),
                ),
                (
                    "Line group",
                    limits.group_count,
                    self.line_task_groups().len(),
                ),
                (
                    "type",
                    limits.expression.type_count,
                    self.type_table().len(),
                ),
                (
                    "local",
                    limits.expression.local_count,
                    self.local_declarations().len(),
                ),
                (
                    "state",
                    limits.expression.state_count,
                    self.callable_states().len(),
                ),
                (
                    "specialization",
                    limits.expression.specialization_count,
                    self.callable_specializations().len(),
                ),
                (
                    "helper",
                    limits.expression.helper_count,
                    self.pure_helpers().len(),
                ),
                (
                    "method",
                    limits.expression.method_count,
                    self.trait_methods().len(),
                ),
                (
                    "template",
                    limits.expression.template_count,
                    self.dialogue_content_templates().rows().len(),
                ),
                (
                    "formatter",
                    limits.expression.attempt_count,
                    self.format_attempts().len(),
                ),
            ] {
                if expected != actual {
                    return Err(Error::Namespace {
                        kind,
                        expected,
                        actual,
                    });
                }
            }
            let mut counts = Counts::new(limits);
            let mut writer = Writer {
                output: None,
                length: 0,
                maximum: limits.encoded_bytes,
            };
            emit(self, &mut writer, &mut counts, resources, meter)?;
            let n = writer.length;
            charge(meter, n)?;
            let mut bytes = Vec::with_capacity(n);
            let mut emitted = Counts::new(limits);
            let mut writer = Writer {
                output: Some(&mut bytes),
                length: 0,
                maximum: n,
            };
            emit(self, &mut writer, &mut emitted, resources, meter)?;
            debug_assert_eq!(writer.length, n);
            debug_assert_eq!(emitted.footprint, counts.footprint);
            Ok(EncodedRuntimeContentPlanSources {
                bytes,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    length: usize,
    maximum: usize,
}
impl Writer<'_> {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", self.length, self.maximum)?;
        if let Some(output) = self.output.as_mut() {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn byte(&mut self, n: u8) -> Result<(), Error> {
        self.raw(&[n])
    }
    fn count(&mut self, n: usize) -> Result<(), Error> {
        self.raw(
            &u32::try_from(n)
                .map_err(|_| Error::Arithmetic)?
                .to_le_bytes(),
        )
    }
    fn frame(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.count(bytes.len())?;
        self.raw(bytes)
    }
    fn ordinal(&mut self, actual: u32, index: usize) -> Result<(), Error> {
        if actual as usize != index.checked_add(1).ok_or(Error::Arithmetic)? {
            return Err(Error::Invalid {
                kind: "ordinal",
                offset: self.length,
            });
        }
        self.count(actual as usize)
    }
    fn captures(
        &mut self,
        captures: &[RuntimeExpr],
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        counts.captures(captures.len())?;
        self.count(captures.len())?;
        for capture in captures {
            let remaining = self.maximum - self.length;
            limit("encoded bytes", 4, remaining)?;
            let child =
                capture.encode_sources(counts.child_limits(remaining - 4), resources, meter)?;
            counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
            self.frame(child.bytes())?;
        }
        Ok(())
    }
}
fn emit(
    plan: &RuntimePlan,
    writer: &mut Writer<'_>,
    counts: &mut Counts,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<(), Error> {
    writer.raw(DOMAIN)?;
    writer.byte(1)?;
    let rows = plan.dialogue_content().rows();
    counts.rows(rows.len())?;
    writer.count(rows.len())?;
    for row in rows {
        let path = row.line().path();
        let segments = path.segments();
        if segments.is_empty() {
            return Err(Error::Invalid {
                kind: "empty Line path",
                offset: writer.length,
            });
        }
        counts.line(segments.len())?;
        writer.count(segments.len())?;
        for (index, segment) in segments.iter().enumerate() {
            RuntimeIdPath::preflight_stored_source_segment(
                RuntimeIdFamily::Line,
                index,
                segments.len(),
                segment.as_str(),
            )?;
            counts.text(segment.as_str().len(), 3)?;
            writer.frame(segment.as_str().as_bytes())?;
        }
        let template = plan
            .dialogue_content_templates()
            .get(row.template())
            .ok_or(
                crate::plan::RuntimePlanBuildError::MissingDialogueTemplateManifest {
                    template: row.template(),
                },
            )?;
        reference(
            "template",
            template.id().get().get(),
            counts.limits.expression.template_count,
        )?;
        writer.count(template.id().get().get() as usize)?;
        writer.raw(template.digest().as_bytes())?;
        counts.values(row.values().len())?;
        writer.count(row.values().len())?;
        for (index, value) in row.values().iter().enumerate() {
            writer.ordinal(value.slot().get().get(), index)?;
            writer.byte(value.role().encoded())?;
            reference(
                "function",
                value.function().get().get(),
                counts.limits.function_count,
            )?;
            writer.count(value.function().get().get() as usize)?;
            writer.captures(value.captures(), counts, resources, meter)?;
        }
        counts.effects(row.effect_sites().len())?;
        writer.count(row.effect_sites().len())?;
        for (index, effect) in row.effect_sites().iter().enumerate() {
            writer.ordinal(effect.site().get().get(), index)?;
            reference(
                "state",
                effect.state().get().get(),
                counts.limits.expression.state_count,
            )?;
            writer.count(effect.state().get().get() as usize)?;
            writer.captures(effect.captures(), counts, resources, meter)?;
        }
        counts.marks(row.marks().len())?;
        writer.count(row.marks().len())?;
        for (index, mark) in row.marks().iter().enumerate() {
            writer.ordinal(mark.id().get().get(), index)?;
            counts.text(mark.label().len(), 1)?;
            writer.frame(mark.label().as_bytes())?;
        }
        if row.effect_site_count().get() as usize != row.effect_sites().len() {
            return Err(Error::Invalid {
                kind: "effect site count",
                offset: writer.length,
            });
        }
        writer.count(row.effect_site_count().get() as usize)?;
        writer.byte(u8::from(row.line_task_group().is_some()))?;
        if let Some(group) = row.line_task_group() {
            reference("Line group", group.get().get(), counts.limits.group_count)?;
            writer.count(group.get().get() as usize)?;
        }
    }
    Ok(())
}
