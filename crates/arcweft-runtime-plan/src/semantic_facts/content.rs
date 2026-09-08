use std::collections::BTreeSet;

use arcweft_core::runtime_id::{
    RuntimeDialogueContentTemplateId, RuntimeDialogueEffectSiteId, RuntimeDialogueMarkId,
    RuntimeDialogueValueSlotId,
};
use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_hir::identity::LocalId;
use arcweft_lang_sema::effects::EffectSet;
use arcweft_lang_sema::semantic_coordinate::{
    StableCheckedBindingCoordinate, StableCheckedContentFragmentCoordinate,
    StableCheckedContentFragmentCoordinateError, StableCheckedValueCoordinate,
};
use arcweft_text_model::DialogueContentFragmentTemplate;
use thiserror::Error;

use super::{
    RuntimeDialogueEffectTrigger, RuntimeDialogueValueExpression, RuntimeEvaluatedEffectFact,
    RuntimeNormalizedType,
};

/// Opaque stable identity of one root or nested runtime content fragment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeContentFragmentId([u8; 32]);

impl RuntimeContentFragmentId {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Plan-unique key for one reveal-time effect program. Template identity is
/// the occurrence authority; a raw source expression is deliberately absent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeDialogueEffectProgramKey {
    template: RuntimeDialogueContentTemplateId,
    site: RuntimeDialogueEffectSiteId,
}

/// Plan-unique mark identity. A template-local mark ordinal is never a
/// sufficient trigger key once one source line has multiple closed templates.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeDialogueMarkKey {
    template: RuntimeDialogueContentTemplateId,
    mark: RuntimeDialogueMarkId,
}

/// Stable checked marker coordinate joined once to its plan-unique template
/// mark key. Trigger projection consumes this row instead of maintaining a
/// compiler-only coordinate side table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueMarkFact {
    coordinate: arcweft_lang_sema::semantic_coordinate::StableCheckedDialogueMarkCoordinate,
    key: RuntimeDialogueMarkKey,
}

impl RuntimeDialogueMarkFact {
    pub const fn new(
        coordinate: arcweft_lang_sema::semantic_coordinate::StableCheckedDialogueMarkCoordinate,
        key: RuntimeDialogueMarkKey,
    ) -> Self {
        Self { coordinate, key }
    }

    pub const fn coordinate(
        &self,
    ) -> &arcweft_lang_sema::semantic_coordinate::StableCheckedDialogueMarkCoordinate {
        &self.coordinate
    }

    pub const fn key(&self) -> RuntimeDialogueMarkKey {
        self.key
    }
}

impl RuntimeDialogueMarkKey {
    pub const fn new(
        template: RuntimeDialogueContentTemplateId,
        mark: RuntimeDialogueMarkId,
    ) -> Self {
        Self { template, mark }
    }

    pub const fn template(self) -> RuntimeDialogueContentTemplateId {
        self.template
    }

    pub const fn mark(self) -> RuntimeDialogueMarkId {
        self.mark
    }
}

impl RuntimeDialogueEffectProgramKey {
    pub const fn new(
        template: RuntimeDialogueContentTemplateId,
        site: RuntimeDialogueEffectSiteId,
    ) -> Self {
        Self { template, site }
    }

    pub const fn template(self) -> RuntimeDialogueContentTemplateId {
        self.template
    }

    pub const fn site(self) -> RuntimeDialogueEffectSiteId {
        self.site
    }
}

/// Exact synthetic input for one dialogue interpolation callback capture.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeDialogueValueCaptureKey {
    template: RuntimeDialogueContentTemplateId,
    slot: RuntimeDialogueValueSlotId,
    position: u32,
}

impl RuntimeDialogueValueCaptureKey {
    pub const fn new(
        template: RuntimeDialogueContentTemplateId,
        slot: RuntimeDialogueValueSlotId,
        position: u32,
    ) -> Self {
        Self {
            template,
            slot,
            position,
        }
    }

    pub const fn template(self) -> RuntimeDialogueContentTemplateId {
        self.template
    }

    pub const fn slot(self) -> RuntimeDialogueValueSlotId {
        self.slot
    }

    pub const fn position(self) -> u32 {
        self.position
    }
}

/// Exact synthetic input for one dialogue effect callback capture.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeDialogueEffectCaptureKey {
    program: RuntimeDialogueEffectProgramKey,
    position: u32,
}

impl RuntimeDialogueEffectCaptureKey {
    pub const fn new(program: RuntimeDialogueEffectProgramKey, position: u32) -> Self {
        Self { program, position }
    }

    pub const fn program(self) -> RuntimeDialogueEffectProgramKey {
        self.program
    }

    pub const fn position(self) -> u32 {
        self.position
    }
}

/// One exact typed free-local capture of a reveal-time content effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueEffectCaptureFact {
    local: LocalId,
    origin: StableCheckedBindingCoordinate,
    ty: RuntimeNormalizedType,
}

impl RuntimeDialogueEffectCaptureFact {
    pub const fn new(
        local: LocalId,
        origin: StableCheckedBindingCoordinate,
        ty: RuntimeNormalizedType,
    ) -> Self {
        Self { local, origin, ty }
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn origin(&self) -> &StableCheckedBindingCoordinate {
        &self.origin
    }

    pub const fn ty(&self) -> &RuntimeNormalizedType {
        &self.ty
    }
}

/// Complete executable program row for one template-local effect site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueEffectProgramFact {
    site: RuntimeDialogueEffectSiteId,
    trigger: RuntimeDialogueEffectTrigger,
    effects: EffectSet,
    operation: RuntimeEvaluatedEffectFact,
    captures: Box<[RuntimeDialogueEffectCaptureFact]>,
}

impl RuntimeDialogueEffectProgramFact {
    pub fn new(
        site: RuntimeDialogueEffectSiteId,
        trigger: RuntimeDialogueEffectTrigger,
        effects: EffectSet,
        operation: RuntimeEvaluatedEffectFact,
        captures: impl Into<Box<[RuntimeDialogueEffectCaptureFact]>>,
    ) -> Self {
        Self {
            site,
            trigger,
            effects,
            operation,
            captures: captures.into(),
        }
    }

    pub const fn site(&self) -> RuntimeDialogueEffectSiteId {
        self.site
    }

    pub const fn trigger(&self) -> &RuntimeDialogueEffectTrigger {
        &self.trigger
    }

    pub const fn effects(&self) -> &EffectSet {
        &self.effects
    }

    pub const fn operation(&self) -> &RuntimeEvaluatedEffectFact {
        &self.operation
    }

    pub const fn captures(&self) -> &[RuntimeDialogueEffectCaptureFact] {
        &self.captures
    }
}

/// Atomic runtime-plan projection of one checked content report.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeContentFragmentFact {
    id: RuntimeContentFragmentId,
    source: ExprId,
    owner: StableCheckedValueCoordinate,
    template: DialogueContentFragmentTemplate,
    values: Box<[RuntimeDialogueValueExpression]>,
    effects: Box<[RuntimeDialogueEffectProgramFact]>,
    marks: Box<[RuntimeDialogueMarkFact]>,
}

impl RuntimeContentFragmentFact {
    pub fn try_new(
        source: ExprId,
        coordinate: StableCheckedContentFragmentCoordinate,
        template: DialogueContentFragmentTemplate,
        values: impl Into<Box<[RuntimeDialogueValueExpression]>>,
        effects: impl Into<Box<[RuntimeDialogueEffectProgramFact]>>,
        marks: impl Into<Box<[RuntimeDialogueMarkFact]>>,
    ) -> Result<Self, RuntimeContentFragmentFactError> {
        let id = RuntimeContentFragmentId(
            *coordinate
                .semantic_digest()
                .map_err(RuntimeContentFragmentFactError::Identity)?
                .as_bytes(),
        );
        let fact = Self {
            id,
            source,
            owner: coordinate.owner().clone(),
            template,
            values: values.into(),
            effects: effects.into(),
            marks: marks.into(),
        };
        fact.validate()?;
        Ok(fact)
    }

    pub const fn id(&self) -> RuntimeContentFragmentId {
        self.id
    }

    pub const fn source(&self) -> ExprId {
        self.source
    }

    pub const fn owner(&self) -> &StableCheckedValueCoordinate {
        &self.owner
    }

    pub const fn template(&self) -> &DialogueContentFragmentTemplate {
        &self.template
    }

    pub const fn values(&self) -> &[RuntimeDialogueValueExpression] {
        &self.values
    }

    pub const fn effects(&self) -> &[RuntimeDialogueEffectProgramFact] {
        &self.effects
    }

    pub const fn marks(&self) -> &[RuntimeDialogueMarkFact] {
        &self.marks
    }

    fn validate(&self) -> Result<(), RuntimeContentFragmentFactError> {
        if self.template.slots().len() != self.values.len() {
            return Err(RuntimeContentFragmentFactError::ValueCountMismatch {
                declared: self.template.slots().len(),
                provided: self.values.len(),
            });
        }
        for (declared, value) in self.template.slots().iter().zip(&self.values) {
            if declared.slot() != value.slot()
                || declared.role() != value.role()
                || declared.semantic_type() != value.ty().identity()
            {
                return Err(RuntimeContentFragmentFactError::ValueMismatch {
                    declared: declared.slot(),
                    provided: value.slot(),
                });
            }
        }
        if self.template.effects().len() != self.effects.len() {
            return Err(RuntimeContentFragmentFactError::EffectCountMismatch {
                declared: self.template.effects().len(),
                provided: self.effects.len(),
            });
        }
        for (declared, effect) in self.template.effects().iter().zip(&self.effects) {
            if declared.id() != effect.site() {
                return Err(RuntimeContentFragmentFactError::EffectMismatch {
                    declared: declared.id(),
                    provided: effect.site(),
                });
            }
            let mut locals = BTreeSet::new();
            let mut origins = BTreeSet::new();
            for capture in effect.captures() {
                if !locals.insert(capture.local()) || !origins.insert(capture.origin().clone()) {
                    return Err(RuntimeContentFragmentFactError::DuplicateEffectCapture {
                        site: effect.site(),
                        local: capture.local(),
                    });
                }
            }
        }
        if self.template.marks().len() != self.marks.len() {
            return Err(RuntimeContentFragmentFactError::MarkCountMismatch {
                declared: self.template.marks().len(),
                provided: self.marks.len(),
            });
        }
        let mut coordinates = BTreeSet::new();
        for (declared, mark) in self.template.marks().iter().zip(&self.marks) {
            if mark.key().template() != self.template.id()
                || mark.key().mark() != declared.id()
                || !coordinates.insert(mark.coordinate().clone())
            {
                return Err(RuntimeContentFragmentFactError::MarkMismatch {
                    declared: declared.id(),
                    provided: mark.key().mark(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeContentFragmentFactError {
    #[error("content fragment identity cannot be issued: {0}")]
    Identity(StableCheckedContentFragmentCoordinateError),
    #[error("content fragment declares {declared} value slots but provides {provided} programs")]
    ValueCountMismatch { declared: usize, provided: usize },
    #[error("content fragment value row {provided} does not match declared slot {declared}")]
    ValueMismatch {
        declared: RuntimeDialogueValueSlotId,
        provided: RuntimeDialogueValueSlotId,
    },
    #[error("content fragment declares {declared} effect sites but provides {provided} programs")]
    EffectCountMismatch { declared: usize, provided: usize },
    #[error("content fragment effect row {provided} does not match declared site {declared}")]
    EffectMismatch {
        declared: RuntimeDialogueEffectSiteId,
        provided: RuntimeDialogueEffectSiteId,
    },
    #[error("content fragment declares {declared} marks but provides {provided} bindings")]
    MarkCountMismatch { declared: usize, provided: usize },
    #[error("content fragment mark row {provided} does not match declared mark {declared}")]
    MarkMismatch {
        declared: RuntimeDialogueMarkId,
        provided: RuntimeDialogueMarkId,
    },
    #[error("content fragment effect site {site} repeats capture local {local:?}")]
    DuplicateEffectCapture {
        site: RuntimeDialogueEffectSiteId,
        local: LocalId,
    },
}
