//! Complete canonical operation-field grammar. Value and body cursors follow
//! the original operation owner's ordered projections, never source spelling.

use super::super::super::audio_commands::PreparedRuntimeAudioCommandSource;
use super::super::super::evaluated_effects::PreparedRuntimeEvaluatedEffectSource;
use super::super::super::host_calls::PreparedRuntimeHostCallSource;
use super::super::super::project_calls::PreparedRuntimeProjectCallSource;
use super::super::super::selected_producers::PreparedRuntimeSelectedProducerSource;
use super::super::super::static_effects::PreparedRuntimeStaticEffectSource;
use super::super::{Counts, Error};
use super::{Cursor, Row};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimePattern;
use crate::plan::{
    RuntimeAwaitPendingObserverSeed, RuntimeChoiceOptionSeed, RuntimeFlowMatchArmSeed,
    RuntimeFlowMatchGuardSeed, RuntimeHostArgumentSeed, RuntimeScheduledCaptureSeed,
};
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeExpr;

pub(super) fn preflight(
    row: &Row<'_>,
    counts: &mut Counts,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<(), Error> {
    let mut reader = Reader {
        metadata: Cursor::new(row.metadata),
        values: Cursor::new(row.values),
        bodies: Cursor::new(row.bodies),
        counts,
        resources,
        meter,
    };
    match row.tag {
        0 | 3 | 21 | 23 | 25 | 27 | 30 | 44 | 47 | 48 => return Err(Error::Continuation),
        1 | 45 => {
            reader.pattern()?;
            reader.expression()?;
        }
        2 => {
            reader.metadata.zero_based(
                "format attempt",
                reader.counts.limits.expression.attempt_count,
            )?;
            reader.metadata.u32()?;
            reader.expression()?;
            reader.body()?;
        }
        4 => {
            reader.pattern()?;
            reader.expression()?;
            reader.body()?;
        }
        5 => {
            reader.assignment()?;
            reader.expression()?;
        }
        6 => reader.line_operation()?,
        7 | 8 | 34 | 36 => reader.expression()?,
        9 => {
            reader
                .metadata
                .positive("content plan", reader.counts.limits.content_count)?;
            reader.metadata.positive(
                "dialogue result type",
                reader.counts.limits.expression.type_count,
            )?;
            reader.expression()?;
            reader.pattern()?;
        }
        10 => reader.choice()?,
        11 => {
            reader.optional_pattern()?;
            reader.expression()?;
            let count = reader.metadata.u32()? as usize;
            reader
                .counts
                .array::<RuntimeAwaitPendingObserverSeed>(count)?;
            for _ in 0..count {
                reader.pattern()?;
                reader.body()?;
            }
        }
        12 => {
            let bytes = reader.metadata.frame()?;
            let child = PreparedRuntimeSelectedProducerSource::preflight(
                bytes,
                reader.counts.selected(bytes.len()),
                resources,
                reader.meter,
            )?;
            reader
                .counts
                .child(child.node_count(), child.elements(), child.storage_bytes())?;
        }
        13 => {
            reader.optional_pattern()?;
            reader.expression()?;
            reader.local()?;
            reader.metadata.u32()?;
            reader.template()?;
            reader.template()?;
            let count = reader.metadata.u32()? as usize;
            reader
                .counts
                .array::<crate::plan::RuntimeLineEffectSeed>(count)?;
            for _ in 0..count {
                reader.effect()?;
            }
        }
        14 => {
            let bytes = reader.metadata.frame()?;
            let child = PreparedRuntimeHostCallSource::preflight(
                bytes,
                reader.counts.host(bytes.len()),
                resources,
                reader.meter,
            )?;
            reader
                .counts
                .child(child.node_count(), child.elements(), child.storage_bytes())?;
        }
        15 => {
            let bytes = reader.metadata.frame()?;
            let child = PreparedRuntimeProjectCallSource::preflight(
                bytes,
                reader.counts.project(bytes.len()),
                resources,
                reader.meter,
            )?;
            reader
                .counts
                .child(child.node_count(), child.elements(), child.storage_bytes())?;
        }
        16 => {
            reader.expression()?;
            reader.call_arguments()?;
            reader.pattern()?;
        }
        17 => {
            reader.expression()?;
            reader.body()?;
            reader.body()?;
        }
        18 => {
            reader.pattern()?;
            reader.expression()?;
            reader.optional_expression()?;
            reader.body()?;
            reader.body()?;
        }
        19 => reader.match_arms()?,
        20 => {
            reader.optional_pattern()?;
            reader.body()?;
        }
        22 => {
            reader.expression()?;
            reader.body()?;
        }
        24 => {
            reader.pattern()?;
            reader.expression()?;
            reader.optional_expression()?;
            reader.body()?;
        }
        26 => {
            reader.pattern()?;
            reader.expression()?;
            reader.iterator()?;
            reader.body()?;
        }
        28 => {
            reader.optional_text()?;
            reader.template()?;
            reader.locals()?;
            reader.body()?;
        }
        29 => {
            reader.scope()?;
            reader.body()?;
        }
        31 => reader.optional_expression()?,
        32 | 43 | 46 => (),
        33 => reader.path()?,
        35 | 41 => {
            reader.text()?;
        }
        37 => reader.effect()?,
        38 => {
            let bytes = reader.metadata.frame()?;
            let child = PreparedRuntimeEvaluatedEffectSource::preflight(
                bytes,
                reader.counts.evaluated(bytes.len()),
                resources,
                reader.meter,
            )?;
            reader
                .counts
                .child(child.node_count(), child.elements(), child.storage_bytes())?;
        }
        39 => {
            reader
                .metadata
                .zero_based("defer site", reader.counts.limits.defer_count)?;
            reader.tag("defer outcome", 4)?;
            reader.tag("defer owner", 2)?;
            let count = reader.metadata.u32()? as usize;
            reader.counts.elements(count)?;
            for _ in 0..count {
                reader.expression()?;
            }
        }
        40 => {
            reader.text()?;
            reader.effect()?;
        }
        42 => reader.scope()?,
        49 => {
            let tag = reader.metadata.byte()?;
            crate::value::RuntimeStandardMapFamily::from_semantic_tag(tag).ok_or(Error::Tag {
                kind: "Map family",
                tag,
            })?;
            reader.local()?;
            reader.local()?;
            reader.local()?;
            let completion = reader.tag("Map completion", 2)?;
            let boundary = if completion == 1 {
                Some(reader.metadata.u32()? as usize)
            } else {
                None
            };
            let count = reader.body()?;
            if boundary.is_some_and(|boundary| boundary > count) {
                return Err(Error::Prefix);
            }
        }
        tag => {
            return Err(Error::Tag {
                kind: "operation",
                tag,
            });
        }
    }
    reader.metadata.finish()?;
    reader.values.finish()?;
    reader.bodies.finish()?;
    Ok(())
}

struct Reader<'a, 'c> {
    metadata: Cursor<'a>,
    values: Cursor<'a>,
    bodies: Cursor<'a>,
    counts: &'c mut Counts,
    resources: &'c ValueResourceAdmission,
    meter: &'c mut TaskSemanticMeter,
}
impl Reader<'_, '_> {
    fn tag(&mut self, kind: &'static str, variants: u8) -> Result<u8, Error> {
        let tag = self.metadata.byte()?;
        if tag < variants {
            Ok(tag)
        } else {
            Err(Error::Tag { kind, tag })
        }
    }
    fn text(&mut self) -> Result<&str, Error> {
        let text = self.metadata.text()?;
        self.counts.text(text)?;
        Ok(text)
    }
    fn optional_text(&mut self) -> Result<(), Error> {
        if self.metadata.flag("optional text")? {
            self.text()?;
        }
        Ok(())
    }
    fn local(&mut self) -> Result<(), Error> {
        self.metadata
            .positive("local", self.counts.limits.expression.local_count)?;
        Ok(())
    }
    fn ty(&mut self) -> Result<(), Error> {
        self.metadata
            .positive("type", self.counts.limits.expression.type_count)?;
        Ok(())
    }
    fn locals(&mut self) -> Result<(), Error> {
        let count = self.metadata.u32()? as usize;
        self.counts
            .array::<crate::plan::RuntimeLocalSeedId>(count)?;
        for _ in 0..count {
            self.local()?;
        }
        Ok(())
    }
    fn body(&mut self) -> Result<usize, Error> {
        Ok(self.bodies.u32()? as usize)
    }
    fn expression(&mut self) -> Result<(), Error> {
        let (kind, bytes) = self.values.value()?;
        if kind != 0 {
            return Err(Error::Tag {
                kind: "expression root",
                tag: kind,
            });
        }
        let child = RuntimeExpr::preflight_sources(
            bytes,
            self.counts.expression(bytes.len()),
            self.resources,
            self.meter,
        )?;
        self.counts
            .child(child.node_count(), child.elements(), child.storage_bytes())
    }
    fn pattern(&mut self) -> Result<(), Error> {
        let (kind, bytes) = self.values.value()?;
        if kind != 1 {
            return Err(Error::Tag {
                kind: "pattern root",
                tag: kind,
            });
        }
        let child = RuntimePattern::preflight_sources(
            bytes,
            self.counts.pattern(bytes.len()),
            self.resources,
            self.meter,
        )?;
        self.counts
            .child(child.node_count(), child.elements(), child.storage_bytes())
    }
    fn optional_pattern(&mut self) -> Result<(), Error> {
        if self.metadata.flag("optional pattern")? {
            self.pattern()?;
        }
        Ok(())
    }
    fn optional_expression(&mut self) -> Result<(), Error> {
        if self.metadata.flag("optional expression")? {
            self.expression()?;
        }
        Ok(())
    }
    fn effect(&mut self) -> Result<(), Error> {
        let tag = self.tag("effect Source", 2)?;
        let bytes = self.metadata.frame()?;
        if tag == 0 {
            let child = PreparedRuntimeStaticEffectSource::preflight(
                bytes,
                self.counts.static_effect(bytes.len()),
                self.resources,
                self.meter,
            )?;
            self.counts
                .child(child.node_count(), child.elements(), child.storage_bytes())
        } else {
            let child = PreparedRuntimeAudioCommandSource::preflight(
                bytes,
                self.counts.audio(bytes.len()),
                self.resources,
                self.meter,
            )?;
            self.counts
                .child(child.node_count(), child.elements(), child.storage_bytes())
        }
    }
    fn path(&mut self) -> Result<(), Error> {
        use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath, RuntimeIdSegment};
        let count = self.metadata.u32()? as usize;
        if count == 0 {
            return Err(crate::runtime_id::RuntimeIdError::Empty {
                family: RuntimeIdFamily::Flow,
            }
            .into());
        }
        self.counts.array::<RuntimeIdSegment>(count)?;
        for index in 0..count {
            let text = self.metadata.text()?;
            self.counts.text(text)?;
            RuntimeIdPath::preflight_stored_source_segment(
                RuntimeIdFamily::Flow,
                index,
                count,
                text,
            )?;
        }
        self.text()?;
        Ok(())
    }
    fn scope(&mut self) -> Result<(), Error> {
        if self.metadata.flag("named scope")? {
            let name = self.metadata.text()?;
            self.counts.text(name)?;
            arcweft_id::DeclarationName::validate_source(name)?;
        }
        Ok(())
    }
    fn argument_roles(&mut self) -> Result<usize, Error> {
        let count = self.metadata.u32()? as usize;
        self.counts.array::<RuntimeHostArgumentSeed>(count)?;
        for _ in 0..count {
            let tag = self.tag("argument", 3)?;
            self.metadata.digest()?;
            if tag == 1 {
                self.text()?;
            }
        }
        Ok(count)
    }
    fn arguments(&mut self) -> Result<(), Error> {
        let count = self.argument_roles()?;
        for _ in 0..count {
            self.expression()?;
        }
        Ok(())
    }
    fn call_arguments(&mut self) -> Result<(), Error> {
        let count = self.metadata.u32()? as usize;
        self.counts
            .array::<crate::plan::RuntimeCallArgumentSeed>(count)?;
        for _ in 0..count {
            self.tag("call argument mode", 2)?;
            self.metadata.u32()?;
            self.expression()?;
        }
        Ok(())
    }
    fn template(&mut self) -> Result<(), Error> {
        self.counts.storage(
            size_of::<crate::plan::RuntimeNeedProducerTemplateSeed>()
                + size_of::<crate::task::NeedProducerTemplate>(),
        )?;
        self.metadata.digest()?;
        let tag = self.metadata.byte()?;
        crate::task::NeedProducerFamily::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "producer family",
            tag,
        })?;
        self.metadata.digest()?;
        self.metadata
            .zero_based("task", self.counts.limits.task_count)?;
        self.metadata.digest()?;
        self.ty()?;
        let tag = self.metadata.byte()?;
        crate::task::TaskClass::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "task class",
            tag,
        })?;
        self.metadata.i32()?;
        self.text()?;
        self.tag("task policy", 2)?;
        self.text()?;
        self.text()?;
        self.text()?;
        let before = self.counts.footprint;
        self.arguments()?;
        // The original request owner derives captures from all argument roots.
        // Reserve a conservative scratch/retained slot per syntactic node and
        // edge, including wide captures, before any argument value is built.
        let units = (self.counts.footprint.nodes - before.nodes)
            .checked_add(self.counts.footprint.elements - before.elements)
            .ok_or(Error::Arithmetic)?;
        self.counts.storage(
            units
                .checked_mul(
                    2 * size_of::<crate::plan::RuntimeLocalSeedId>()
                        + 2 * size_of::<crate::runtime_id::RuntimeLocalDeclarationId>(),
                )
                .ok_or(Error::Arithmetic)?,
        )
    }
    fn match_arms(&mut self) -> Result<(), Error> {
        self.expression()?;
        let count = self.metadata.u32()? as usize;
        self.counts.array::<RuntimeFlowMatchArmSeed>(count)?;
        for _ in 0..count {
            self.pattern()?;
            if self.metadata.flag("Match guard")? {
                self.counts
                    .storage(size_of::<RuntimeFlowMatchGuardSeed>())?;
                self.local()?;
                self.ty()?;
                self.optional_expression()?;
                self.locals()?;
                self.body()?;
            }
            self.body()?;
        }
        Ok(())
    }
    fn fields(&mut self) -> Result<(), Error> {
        let count = self.metadata.u32()? as usize;
        self.counts
            .array::<crate::plan::RuntimeRecordFieldSeedId>(count)?;
        for _ in 0..count {
            self.metadata.u32()?;
        }
        Ok(())
    }
    fn initialization(&mut self) -> Result<(), Error> {
        self.tag("place initialization", 3)?;
        Ok(())
    }
    fn assignment(&mut self) -> Result<(), Error> {
        match self.tag("mutable place", 2)? {
            0 => self.local()?,
            1 => {
                self.local()?;
                self.fields()?;
            }
            _ => unreachable!(),
        }
        if self.tag("place displacement", 2)? == 1 {
            self.initialization()?;
            let count = self.metadata.u32()? as usize;
            self.counts.array::<crate::value::RuntimeDisplacedField<
                crate::plan::RuntimeRecordFieldSeedId,
            >>(count)?;
            for _ in 0..count {
                self.fields()?;
                self.initialization()?;
            }
        }
        Ok(())
    }
    fn iterator(&mut self) -> Result<(), Error> {
        if self.tag("iterator evidence", 2)? == 0 {
            self.tag("builtin iterator", 7)?;
            for _ in 0..4 {
                self.ty()?;
            }
        } else {
            self.ty()?;
            self.ty()?;
            let tag = self.tag("iterator witness", 2)?;
            if tag == 0 {
                self.metadata.positive(
                    "into_iter method",
                    self.counts.limits.expression.method_count,
                )?;
            }
            self.metadata
                .positive("next method", self.counts.limits.expression.method_count)?;
        }
        Ok(())
    }
    fn character(&mut self) -> Result<(), Error> {
        let character = self.metadata.text()?;
        self.counts.text(character)?;
        arcweft_character::id::CharacterId::validate_source(character)?;
        Ok(())
    }
    fn line_operation(&mut self) -> Result<(), Error> {
        self.optional_pattern()?;
        let tag = self.tag("Line operation", 4)?;
        self.metadata.u32()?;
        match tag {
            0 => {
                self.character()?;
                self.tag("Line handle scope", 1)?;
            }
            1 => {
                self.metadata.u32()?;
                self.expression()?;
                let count = self.metadata.u32()? as usize;
                self.counts.array::<RuntimeScheduledCaptureSeed>(count)?;
                for _ in 0..count {
                    self.local()?;
                    self.expression()?;
                }
            }
            2 => {
                self.character()?;
                self.local()?;
                self.expression()?;
                self.expression()?;
            }
            3 => (),
            _ => unreachable!(),
        }
        Ok(())
    }
    fn choice(&mut self) -> Result<(), Error> {
        self.optional_text()?;
        let count = self.metadata.u32()? as usize;
        self.counts.array::<RuntimeChoiceOptionSeed>(count)?;
        for _ in 0..count {
            self.optional_text()?;
            self.text()?;
            if self.metadata.flag("choice target")? {
                self.path()?;
            }
            if self.metadata.flag("choice output")? {
                self.optional_text()?;
                self.text()?;
            }
            let count = self.metadata.u32()? as usize;
            self.counts
                .array::<crate::plan::RuntimeLineEffectSeed>(count)?;
            for _ in 0..count {
                self.effect()?;
            }
        }
        Ok(())
    }
}
