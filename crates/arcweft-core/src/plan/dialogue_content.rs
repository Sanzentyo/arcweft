use std::collections::BTreeMap;
use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::entry::RuntimeDialogueContentTemplateDigest;
use crate::pattern::RuntimeSemanticTypeId;
use crate::runtime_id::RuntimePlanTypeId;
use crate::runtime_id::{
    RuntimeDialogueContentPlanId, RuntimeDialogueContentTemplateId, RuntimeDialogueEffectSiteCount,
    RuntimeDialogueMarkId, RuntimeDialogueValueSlotId, RuntimeFunctionSiteId,
    RuntimeLineTaskGroupId,
};
use crate::time::LogicalDuration;

use super::RuntimeLineId;
use crate::value::RuntimeDialogueOpaqueRole;

/// Semantic role of one evaluated dialogue template value.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeDialogueValueRole {
    Interpolation = 0,
    Content = 1,
}

impl RuntimeDialogueValueRole {
    /// Complete canonical role inventory in wire/tag order.
    pub const ALL: [Self; 2] = [Self::Interpolation, Self::Content];

    /// Canonical compact tag used by runtime-owned content envelopes.
    #[must_use]
    pub const fn encoded(self) -> u8 {
        self as u8
    }

    /// Decodes one canonical compact role tag without accepting aliases.
    #[must_use]
    pub const fn from_encoded(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Interpolation),
            1 => Some(Self::Content),
            _ => None,
        }
    }
}

/// Static scheduling trigger for one dialogue-content effect site.
///
/// This lower-level trigger intentionally carries no runtime-plan or text
/// model dependency.  The plan/runtime owners map their richer effect facts
/// into this closed ABI before publishing a core plan.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RuntimeDialogueContentEffectTrigger {
    /// Run when the containing content is revealed.
    Content,
    /// Run after the supplied logical delay has elapsed.
    Delay { duration: LogicalDuration },
}

/// Static ABI schema for one content-local effect callback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueContentEffectSlot {
    site: crate::runtime_id::RuntimeDialogueEffectSiteId,
    trigger: RuntimeDialogueContentEffectTrigger,
    capture_types: Box<[RuntimePlanTypeId]>,
}

impl RuntimeDialogueContentEffectSlot {
    pub(crate) fn new(
        site: crate::runtime_id::RuntimeDialogueEffectSiteId,
        trigger: RuntimeDialogueContentEffectTrigger,
        capture_types: Box<[RuntimePlanTypeId]>,
    ) -> Self {
        Self {
            site,
            trigger,
            capture_types,
        }
    }

    #[must_use]
    pub const fn site(&self) -> crate::runtime_id::RuntimeDialogueEffectSiteId {
        self.site
    }

    #[must_use]
    pub const fn trigger(&self) -> RuntimeDialogueContentEffectTrigger {
        self.trigger
    }

    #[must_use]
    pub const fn capture_types(&self) -> &[RuntimePlanTypeId] {
        &self.capture_types
    }
}

/// One plan-owned function site supplying a document-local dialogue slot.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueValueSite {
    slot: RuntimeDialogueValueSlotId,
    role: RuntimeDialogueValueRole,
    function: RuntimeFunctionSiteId,
    captures: Box<[crate::value::RuntimeExpr]>,
}

/// One plan-owned executable callback site for a dialogue content line.
///
/// The function site is the complete static callback authority.  Its
/// captures are evaluated when the dialogue activation is created and are
/// retained by that activation; reveal only selects this row by its rebased
/// effect site.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueEffectSite {
    site: crate::runtime_id::RuntimeDialogueEffectSiteId,
    function: RuntimeFunctionSiteId,
    captures: Box<[crate::value::RuntimeExpr]>,
}

impl RuntimeDialogueEffectSite {
    pub(crate) const fn new(
        site: crate::runtime_id::RuntimeDialogueEffectSiteId,
        function: RuntimeFunctionSiteId,
        captures: Box<[crate::value::RuntimeExpr]>,
    ) -> Self {
        Self {
            site,
            function,
            captures,
        }
    }

    #[must_use]
    pub const fn site(&self) -> crate::runtime_id::RuntimeDialogueEffectSiteId {
        self.site
    }

    #[must_use]
    pub const fn function(&self) -> RuntimeFunctionSiteId {
        self.function
    }

    #[must_use]
    pub const fn captures(&self) -> &[crate::value::RuntimeExpr] {
        &self.captures
    }
}

/// Exact immutable text-template slot schema joined into one runtime plan.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct RuntimeDialogueContentSlot {
    slot: RuntimeDialogueValueSlotId,
    role: RuntimeDialogueValueRole,
    semantic_type: RuntimeSemanticTypeId,
}

/// One immutable runtime-plan manifest row for a text-model content template.
///
/// The row is the sole plan-owned copy of the template digest and canonical
/// role/type slot schema. Individual dialogue plans retain only this row's
/// dedicated template identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueContentTemplateManifest {
    id: RuntimeDialogueContentTemplateId,
    digest: RuntimeDialogueContentTemplateDigest,
    slots: Box<[RuntimeDialogueContentSlot]>,
    effects: Box<[RuntimeDialogueContentEffectSlot]>,
}

impl RuntimeDialogueContentTemplateManifest {
    pub(crate) fn new_with_effects(
        id: RuntimeDialogueContentTemplateId,
        digest: RuntimeDialogueContentTemplateDigest,
        slots: Box<[RuntimeDialogueContentSlot]>,
        effects: Box<[RuntimeDialogueContentEffectSlot]>,
    ) -> Self {
        Self {
            id,
            digest,
            slots,
            effects,
        }
    }

    #[must_use]
    pub const fn id(&self) -> RuntimeDialogueContentTemplateId {
        self.id
    }

    #[must_use]
    pub const fn digest(&self) -> RuntimeDialogueContentTemplateDigest {
        self.digest
    }

    #[must_use]
    pub const fn slots(&self) -> &[RuntimeDialogueContentSlot] {
        &self.slots
    }

    #[must_use]
    pub const fn effects(&self) -> &[RuntimeDialogueContentEffectSlot] {
        &self.effects
    }

    pub(crate) fn validate_slot_schema(
        &self,
    ) -> Result<(), RuntimeDialogueContentTemplateManifestError> {
        for (index, slot) in self.slots.iter().enumerate() {
            let expected = RuntimeDialogueValueSlotId::from_zero_based(index)
                .ok_or(RuntimeDialogueContentTemplateManifestError::TooManySlots)?;
            if slot.slot() != expected {
                return Err(
                    RuntimeDialogueContentTemplateManifestError::NonCanonicalSlot {
                        expected,
                        actual: slot.slot(),
                    },
                );
            }
            if slot.role() == RuntimeDialogueValueRole::Content
                && slot.semantic_type() != RuntimeDialogueOpaqueRole::Content.semantic_identity()
            {
                return Err(
                    RuntimeDialogueContentTemplateManifestError::InvalidContentSlot {
                        slot: expected,
                    },
                );
            }
        }
        for (index, effect) in self.effects.iter().enumerate() {
            let expected = crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                .ok_or(RuntimeDialogueContentTemplateManifestError::TooManyEffects)?;
            if effect.site() != expected {
                return Err(
                    RuntimeDialogueContentTemplateManifestError::NonCanonicalEffect {
                        expected,
                        actual: effect.site(),
                    },
                );
            }
        }
        Ok(())
    }
}

/// Immutable, identity-keyed runtime-plan template manifest table.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeDialogueContentTemplateManifestTable {
    rows: Box<[RuntimeDialogueContentTemplateManifest]>,
    by_id: BTreeMap<RuntimeDialogueContentTemplateId, usize>,
}

impl RuntimeDialogueContentTemplateManifestTable {
    #[must_use]
    pub fn get(
        &self,
        id: RuntimeDialogueContentTemplateId,
    ) -> Option<&RuntimeDialogueContentTemplateManifest> {
        self.by_id.get(&id).and_then(|index| self.rows.get(*index))
    }

    #[must_use]
    pub const fn rows(&self) -> &[RuntimeDialogueContentTemplateManifest] {
        &self.rows
    }
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeDialogueContentTemplateManifestTableBuilder {
    rows: Vec<RuntimeDialogueContentTemplateManifest>,
    by_id: BTreeMap<RuntimeDialogueContentTemplateId, usize>,
}

impl RuntimeDialogueContentTemplateManifestTableBuilder {
    pub(crate) fn intern(
        &mut self,
        manifest: RuntimeDialogueContentTemplateManifest,
    ) -> Result<RuntimeDialogueContentTemplateId, RuntimeDialogueContentTemplateManifestError> {
        if let Some(index) = self.by_id.get(&manifest.id).copied() {
            let existing = self
                .rows
                .get(index)
                .expect("template manifest index remains valid");
            if existing != &manifest {
                return Err(
                    RuntimeDialogueContentTemplateManifestError::IdentityConflict {
                        id: manifest.id,
                    },
                );
            }
            return Ok(manifest.id);
        }
        let expected = RuntimeDialogueContentTemplateId::from_zero_based(self.rows.len())
            .ok_or(RuntimeDialogueContentTemplateManifestError::TooManyRows)?;
        if manifest.id != expected {
            return Err(
                RuntimeDialogueContentTemplateManifestError::NonCanonicalId {
                    expected,
                    actual: manifest.id,
                },
            );
        }
        manifest.validate_slot_schema()?;
        let index = self.rows.len();
        self.by_id.insert(manifest.id, index);
        let id = manifest.id;
        self.rows.push(manifest);
        Ok(id)
    }

    pub(crate) fn finish(self) -> RuntimeDialogueContentTemplateManifestTable {
        RuntimeDialogueContentTemplateManifestTable {
            rows: self.rows.into_boxed_slice(),
            by_id: self.by_id,
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeDialogueContentTemplateManifestError {
    #[error("runtime dialogue template manifest table exceeds its u32 row limit")]
    TooManyRows,
    #[error("runtime dialogue template manifest exceeds its u32 slot limit")]
    TooManySlots,
    #[error("runtime dialogue template manifest exceeds its u32 effect-site limit")]
    TooManyEffects,
    #[error("runtime dialogue template manifest id {actual} is not canonical id {expected}")]
    NonCanonicalId {
        expected: RuntimeDialogueContentTemplateId,
        actual: RuntimeDialogueContentTemplateId,
    },
    #[error("runtime dialogue template manifest slot {actual} is not canonical slot {expected}")]
    NonCanonicalSlot {
        expected: RuntimeDialogueValueSlotId,
        actual: RuntimeDialogueValueSlotId,
    },
    #[error(
        "runtime dialogue template manifest effect site {actual} is not canonical site {expected}"
    )]
    NonCanonicalEffect {
        expected: crate::runtime_id::RuntimeDialogueEffectSiteId,
        actual: crate::runtime_id::RuntimeDialogueEffectSiteId,
    },
    #[error("runtime dialogue template manifest Content slot {slot} is not exact Content")]
    InvalidContentSlot { slot: RuntimeDialogueValueSlotId },
    #[error("runtime dialogue template manifest id {id} has conflicting authority")]
    IdentityConflict {
        id: RuntimeDialogueContentTemplateId,
    },
}

impl RuntimeDialogueContentSlot {
    pub const fn new(
        slot: RuntimeDialogueValueSlotId,
        role: RuntimeDialogueValueRole,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Self {
        Self {
            slot,
            role,
            semantic_type,
        }
    }

    #[must_use]
    pub const fn slot(self) -> RuntimeDialogueValueSlotId {
        self.slot
    }

    #[must_use]
    pub const fn role(self) -> RuntimeDialogueValueRole {
        self.role
    }

    #[must_use]
    pub const fn semantic_type(self) -> RuntimeSemanticTypeId {
        self.semantic_type
    }
}

impl RuntimeDialogueValueSite {
    pub(crate) const fn new(
        slot: RuntimeDialogueValueSlotId,
        role: RuntimeDialogueValueRole,
        function: RuntimeFunctionSiteId,
        captures: Box<[crate::value::RuntimeExpr]>,
    ) -> Self {
        Self {
            slot,
            role,
            function,
            captures,
        }
    }

    #[must_use]
    pub const fn slot(&self) -> RuntimeDialogueValueSlotId {
        self.slot
    }

    #[must_use]
    pub const fn role(&self) -> RuntimeDialogueValueRole {
        self.role
    }

    #[must_use]
    pub const fn function(&self) -> RuntimeFunctionSiteId {
        self.function
    }

    #[must_use]
    pub const fn captures(&self) -> &[crate::value::RuntimeExpr] {
        &self.captures
    }
}

/// Exact execution mapping for one source-owned dialogue document.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueContentPlan {
    line: RuntimeLineId,
    template: RuntimeDialogueContentTemplateId,
    values: Box<[RuntimeDialogueValueSite]>,
    effect_sites: Box<[RuntimeDialogueEffectSite]>,
    marks: Box<[RuntimeDialogueMark]>,
    effect_site_count: RuntimeDialogueEffectSiteCount,
    line_task_group: Option<RuntimeLineTaskGroupId>,
}

/// Exact identity of one dialogue-content application.
///
/// A runtime line may have more than one closed template when the same source
/// application is instantiated under different project-function solutions.
/// The pair, rather than the line alone, is therefore the lookup authority.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeDialogueContentApplicationKey {
    line: RuntimeLineId,
    template: RuntimeDialogueContentTemplateId,
}

impl RuntimeDialogueContentApplicationKey {
    #[must_use]
    pub const fn new(line: RuntimeLineId, template: RuntimeDialogueContentTemplateId) -> Self {
        Self { line, template }
    }

    #[must_use]
    pub const fn line(&self) -> &RuntimeLineId {
        &self.line
    }

    #[must_use]
    pub const fn template(&self) -> RuntimeDialogueContentTemplateId {
        self.template
    }
}

/// One source-owned dialogue mark with its content-local typed identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueMark {
    id: RuntimeDialogueMarkId,
    label: String,
}

impl RuntimeDialogueMark {
    pub(crate) const fn new(id: RuntimeDialogueMarkId, label: String) -> Self {
        Self { id, label }
    }

    #[must_use]
    pub const fn id(&self) -> RuntimeDialogueMarkId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl RuntimeDialogueContentPlan {
    pub(crate) fn new(
        line: RuntimeLineId,
        template: RuntimeDialogueContentTemplateId,
        values: Box<[RuntimeDialogueValueSite]>,
        effect_sites: Box<[RuntimeDialogueEffectSite]>,
        marks: Box<[RuntimeDialogueMark]>,
        effect_site_count: RuntimeDialogueEffectSiteCount,
    ) -> Self {
        Self {
            line,
            template,
            values,
            effect_sites,
            marks,
            effect_site_count,
            line_task_group: None,
        }
    }

    #[must_use]
    pub const fn line(&self) -> &RuntimeLineId {
        &self.line
    }

    #[must_use]
    pub const fn template(&self) -> RuntimeDialogueContentTemplateId {
        self.template
    }

    #[must_use]
    pub fn key(&self) -> RuntimeDialogueContentApplicationKey {
        RuntimeDialogueContentApplicationKey::new(self.line.clone(), self.template)
    }

    #[must_use]
    pub const fn values(&self) -> &[RuntimeDialogueValueSite] {
        &self.values
    }

    #[must_use]
    pub const fn effect_sites(&self) -> &[RuntimeDialogueEffectSite] {
        &self.effect_sites
    }

    #[must_use]
    pub const fn marks(&self) -> &[RuntimeDialogueMark] {
        &self.marks
    }

    #[must_use]
    pub const fn effect_site_count(&self) -> RuntimeDialogueEffectSiteCount {
        self.effect_site_count
    }

    #[must_use]
    pub const fn line_task_group(&self) -> Option<RuntimeLineTaskGroupId> {
        self.line_task_group
    }

    pub(crate) fn attach_line_task_group(&mut self, group: RuntimeLineTaskGroupId) -> bool {
        if self.line_task_group.is_some() {
            return false;
        }
        self.line_task_group = Some(group);
        true
    }
}

/// Immutable plan-owned dialogue execution table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeDialogueContentPlanTable {
    rows: Box<[RuntimeDialogueContentPlan]>,
    by_key: BTreeMap<RuntimeDialogueContentApplicationKey, RuntimeDialogueContentPlanId>,
    templates: RuntimeDialogueContentTemplateManifestTable,
}

impl RuntimeDialogueContentPlanTable {
    #[must_use]
    pub fn get(&self, id: RuntimeDialogueContentPlanId) -> Option<&RuntimeDialogueContentPlan> {
        self.rows.get(id.get().get().checked_sub(1)? as usize)
    }

    #[must_use]
    pub fn find(
        &self,
        key: &RuntimeDialogueContentApplicationKey,
    ) -> Option<RuntimeDialogueContentPlanId> {
        self.by_key.get(key).copied()
    }

    #[must_use]
    pub const fn rows(&self) -> &[RuntimeDialogueContentPlan] {
        &self.rows
    }

    #[must_use]
    pub fn template(
        &self,
        id: RuntimeDialogueContentTemplateId,
    ) -> Option<&RuntimeDialogueContentTemplateManifest> {
        self.templates.get(id)
    }

    #[must_use]
    pub const fn templates(&self) -> &RuntimeDialogueContentTemplateManifestTable {
        &self.templates
    }
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeDialogueContentPlanTableBuilder {
    rows: Vec<RuntimeDialogueContentPlan>,
    by_key: BTreeMap<RuntimeDialogueContentApplicationKey, RuntimeDialogueContentPlanId>,
    templates: RuntimeDialogueContentTemplateManifestTableBuilder,
}

impl RuntimeDialogueContentPlanTableBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn intern_template(
        &mut self,
        manifest: RuntimeDialogueContentTemplateManifest,
    ) -> Result<RuntimeDialogueContentTemplateId, RuntimeDialogueContentTemplateManifestError> {
        self.templates.intern(manifest)
    }

    pub(crate) fn template(
        &self,
        id: RuntimeDialogueContentTemplateId,
    ) -> Option<&RuntimeDialogueContentTemplateManifest> {
        self.templates
            .by_id
            .get(&id)
            .and_then(|index| self.templates.rows.get(*index))
    }

    pub(crate) fn push(
        &mut self,
        plan: RuntimeDialogueContentPlan,
    ) -> Result<RuntimeDialogueContentPlanId, RuntimeDialogueContentPlanTableError> {
        let key = plan.key();
        if self.by_key.contains_key(&key) {
            return Err(RuntimeDialogueContentPlanTableError::DuplicateApplication { key });
        }
        let ordinal = u32::try_from(self.rows.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .and_then(NonZeroU32::new)
            .ok_or(RuntimeDialogueContentPlanTableError::TooManyRows)?;
        let id = RuntimeDialogueContentPlanId::from_accepted_ordinal(ordinal);
        self.by_key.insert(key, id);
        self.rows.push(plan);
        Ok(id)
    }

    pub(crate) fn get(
        &self,
        id: RuntimeDialogueContentPlanId,
    ) -> Option<&RuntimeDialogueContentPlan> {
        self.rows.get(id.get().get().checked_sub(1)? as usize)
    }

    pub(crate) fn get_mut(
        &mut self,
        id: RuntimeDialogueContentPlanId,
    ) -> Option<&mut RuntimeDialogueContentPlan> {
        self.rows.get_mut(id.get().get().checked_sub(1)? as usize)
    }

    pub(crate) fn ensure_pushable(
        &self,
        key: &RuntimeDialogueContentApplicationKey,
    ) -> Result<(), RuntimeDialogueContentPlanTableError> {
        if self.by_key.contains_key(key) {
            return Err(RuntimeDialogueContentPlanTableError::DuplicateApplication {
                key: key.clone(),
            });
        }
        u32::try_from(self.rows.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .and_then(NonZeroU32::new)
            .ok_or(RuntimeDialogueContentPlanTableError::TooManyRows)
            .map(|_| ())
    }

    pub(crate) fn finish(self) -> RuntimeDialogueContentPlanTable {
        RuntimeDialogueContentPlanTable {
            rows: self.rows.into_boxed_slice(),
            by_key: self.by_key,
            templates: self.templates.finish(),
        }
    }
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum RuntimeDialogueContentPlanTableError {
    #[error("runtime dialogue content plan repeats application ({key:?})")]
    DuplicateApplication {
        key: RuntimeDialogueContentApplicationKey,
    },
    #[error("runtime dialogue content plan table exceeds its u32 row limit")]
    TooManyRows,
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeDialogueContentApplicationKey, RuntimeDialogueContentPlan,
        RuntimeDialogueContentPlanTableBuilder, RuntimeDialogueContentPlanTableError,
    };
    use crate::plan::RuntimeLineId;
    use crate::runtime_id::RuntimeDialogueContentTemplateId;

    fn plan(
        line: RuntimeLineId,
        template: RuntimeDialogueContentTemplateId,
    ) -> RuntimeDialogueContentPlan {
        RuntimeDialogueContentPlan::new(
            line,
            template,
            Vec::new().into_boxed_slice(),
            Vec::new().into_boxed_slice(),
            Vec::new().into_boxed_slice(),
            Default::default(),
        )
    }

    #[test]
    fn content_plan_lookup_is_keyed_by_line_and_template() {
        let line = RuntimeLineId::from_runtime_line_value("line.same").expect("line identity");
        let first_template =
            RuntimeDialogueContentTemplateId::from_zero_based(0).expect("first template identity");
        let second_template =
            RuntimeDialogueContentTemplateId::from_zero_based(1).expect("second template identity");
        let mut builder = RuntimeDialogueContentPlanTableBuilder::new();
        let first = builder
            .push(plan(line.clone(), first_template))
            .expect("first application");
        let second = builder
            .push(plan(line.clone(), second_template))
            .expect("second application with the same line");

        let table = builder.finish();
        assert_eq!(
            table.find(&RuntimeDialogueContentApplicationKey::new(
                line.clone(),
                first_template,
            )),
            Some(first)
        );
        assert_eq!(
            table.find(&RuntimeDialogueContentApplicationKey::new(
                line.clone(),
                second_template
            )),
            Some(second)
        );
    }

    #[test]
    fn content_plan_lookup_rejects_duplicate_application_key() {
        let line = RuntimeLineId::from_runtime_line_value("line.same").expect("line identity");
        let template =
            RuntimeDialogueContentTemplateId::from_zero_based(0).expect("template identity");
        let mut builder = RuntimeDialogueContentPlanTableBuilder::new();
        builder
            .push(plan(line.clone(), template))
            .expect("first application");
        assert!(matches!(
            builder.push(plan(line.clone(), template)),
            Err(RuntimeDialogueContentPlanTableError::DuplicateApplication { .. })
        ));
    }
}
