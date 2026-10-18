//! Iterative construction of the single admitted pattern algebra.

use super::{
    RuntimeLocalDeclarationId, RuntimeLocalSeedId, RuntimePattern, RuntimePatternRestSeed,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlanBodyConstruction, RuntimePlanBuildError,
    RuntimePlanTypeId, RuntimePlanTypeProjection, RuntimeRecordFieldId, invalid_projection,
    require_same, validate_sequence_length,
};
use crate::pattern::{
    RuntimePatternBindingCoordinate, RuntimePatternBindingPath, RuntimePatternBindingStep,
    RuntimePatternKind, RuntimePatternRest, RuntimeRecordPatternField,
};
use crate::plan::RuntimeRecordPatternFieldSeed;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests;

#[derive(Default)]
struct PatternAdmission {
    bindings: BTreeMap<RuntimeLocalDeclarationId, bool>,
    introduced: Vec<RuntimeLocalDeclarationId>,
}

impl PatternAdmission {
    fn insert(&mut self, local: RuntimeLocalDeclarationId, mutable: bool) -> bool {
        let std::collections::btree_map::Entry::Vacant(entry) = self.bindings.entry(local) else {
            return false;
        };
        entry.insert(mutable);
        self.introduced.push(local);
        true
    }

    fn checkpoint(&self) -> usize {
        self.introduced.len()
    }

    fn rewind(&mut self, checkpoint: usize) {
        assert!(
            checkpoint <= self.introduced.len(),
            "a pattern scope preserves its prefix"
        );
        while self.introduced.len() > checkpoint {
            let local = self
                .introduced
                .pop()
                .expect("the scope has an introduced binding");
            self.bindings.remove(&local);
        }
    }

    fn introduced_since(&self, checkpoint: usize) -> Box<[(RuntimeLocalDeclarationId, bool)]> {
        self.introduced[checkpoint..]
            .iter()
            .map(|local| {
                (
                    *local,
                    *self
                        .bindings
                        .get(local)
                        .expect("introduced bindings remain in their scope"),
                )
            })
            .collect()
    }

    fn validate_alternative(
        &self,
        checkpoint: usize,
        expected: &[(RuntimeLocalDeclarationId, bool)],
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        if self.introduced.len() - checkpoint != expected.len() {
            return invalid_projection("Or binding inventory", ty);
        }
        let mut mismatch = None;
        for &(local, expected) in expected {
            let Some(&actual) = self.bindings.get(&local) else {
                return invalid_projection("Or binding inventory", ty);
            };
            if expected != actual && mismatch.is_none_or(|(first, _, _)| local < first) {
                mismatch = Some((local, expected, actual));
            }
        }
        if let Some((local, expected, actual)) = mismatch {
            return Err(RuntimePlanBuildError::OrBindingMutabilityMismatch {
                local,
                expected,
                actual,
            });
        }
        Ok(())
    }
}

type Seeds = std::vec::IntoIter<RuntimePatternSeed>;

enum Prepared {
    Complete(RuntimePattern),
    Composite(Frame),
}

struct Frame {
    ty: RuntimePlanTypeId,
    path_depth: usize,
    pending: Pending,
}

enum Pending {
    Or {
        seeds: Seeds,
        checkpoint: usize,
        expected: Option<Box<[(RuntimeLocalDeclarationId, bool)]>>,
        values: Vec<RuntimePattern>,
    },
    Tuple {
        seeds: Seeds,
        values: Vec<RuntimePattern>,
    },
    Record(RecordFrame),
    Sequence {
        seeds: Seeds,
        rest: Option<RuntimePatternRestSeed>,
        item: RuntimePlanTypeId,
        values: Vec<RuntimePattern>,
    },
    Variant {
        seed: Option<Box<RuntimePatternSeed>>,
        ordinal: u32,
        expected: Option<RuntimePlanTypeId>,
        payload: Option<RuntimePattern>,
    },
    Whole(WholeFrame),
}

enum Progress {
    Child(RuntimePatternSeed, Option<RuntimePatternBindingStep>),
    Complete(RuntimePatternKind),
}

impl RuntimePlanBodyConstruction<'_> {
    pub(in crate::plan::construction) fn lower_pattern_seed(
        &self,
        seed: RuntimePatternSeed,
    ) -> Result<RuntimePattern, RuntimePlanBuildError> {
        self.lower_pattern(seed, &mut PatternAdmission::default(), &mut Vec::new())
    }

    fn lower_pattern_rest(
        &self,
        rest: RuntimePatternRestSeed,
        whole_ty: RuntimePlanTypeId,
        step: RuntimePatternBindingStep,
        admission: &mut PatternAdmission,
        path: &mut Vec<RuntimePatternBindingStep>,
    ) -> Result<RuntimePatternRest, RuntimePlanBuildError> {
        match rest {
            RuntimePatternRestSeed::Exact => Ok(RuntimePatternRest::Exact),
            RuntimePatternRestSeed::Ignore => Ok(RuntimePatternRest::Ignore),
            RuntimePatternRestSeed::Bind(local) => {
                path.push(step);
                let binding = self.lower_pattern_binding(&local, whole_ty, admission, path, false);
                path.pop();
                Ok(RuntimePatternRest::Bind(binding?))
            }
        }
    }

    fn lower_pattern_binding(
        &self,
        seed: &RuntimeLocalSeedId,
        expected_ty: RuntimePlanTypeId,
        admission: &mut PatternAdmission,
        path: &[RuntimePatternBindingStep],
        mutable: bool,
    ) -> Result<RuntimePatternBindingCoordinate, RuntimePlanBuildError> {
        let (local, actual_ty) = self.resolve_local(seed)?;
        require_same("pattern binding local", expected_ty, actual_ty)?;
        if !admission.insert(local, mutable) {
            return Err(RuntimePlanBuildError::DuplicatePatternBinding { local });
        }
        let path = if path.is_empty() {
            RuntimePatternBindingPath::whole()
        } else {
            RuntimePatternBindingPath::try_from_steps(path.iter().copied())?
        };
        Ok(RuntimePatternBindingCoordinate::from_admitted_parts(
            local, path,
        ))
    }

    fn lower_pattern(
        &self,
        seed: RuntimePatternSeed,
        admission: &mut PatternAdmission,
        path: &mut Vec<RuntimePatternBindingStep>,
    ) -> Result<RuntimePattern, RuntimePlanBuildError> {
        let original_depth = path.len();
        let result = self.lower_pattern_iterative(seed, admission, path);
        path.truncate(original_depth);
        result
    }

    fn lower_pattern_iterative(
        &self,
        seed: RuntimePatternSeed,
        admission: &mut PatternAdmission,
        path: &mut Vec<RuntimePatternBindingStep>,
    ) -> Result<RuntimePattern, RuntimePlanBuildError> {
        let mut frames = Vec::new();
        let mut next = Some(seed);
        let mut completed = None;
        loop {
            if let Some(seed) = next.take() {
                match self.prepare_pattern(seed, admission, path)? {
                    Prepared::Complete(pattern) => completed = Some(pattern),
                    Prepared::Composite(frame) => frames.push(frame),
                }
            }
            if let Some(pattern) = completed.take() {
                let Some(mut frame) = frames.pop() else {
                    return Ok(pattern);
                };
                path.truncate(frame.path_depth);
                frame.accept(pattern, admission, self)?;
                frames.push(frame);
            }
            let mut frame = frames
                .pop()
                .expect("an unfinished composite owns its frame");
            path.truncate(frame.path_depth);
            match frame.advance(self, admission, path)? {
                Progress::Child(seed, step) => {
                    if let Some(step) = step {
                        path.push(step);
                    }
                    frames.push(frame);
                    next = Some(seed);
                }
                Progress::Complete(kind) => {
                    completed = Some(RuntimePattern::from_admitted_parts(frame.ty, kind));
                }
            }
        }
    }

    fn prepare_pattern(
        &self,
        seed: RuntimePatternSeed,
        admission: &mut PatternAdmission,
        path: &[RuntimePatternBindingStep],
    ) -> Result<Prepared, RuntimePlanBuildError> {
        let (semantic_ty, kind) = seed.into_parts();
        let ty = self.resolve_seed_type("pattern", semantic_ty)?;
        let complete = |kind| Prepared::Complete(RuntimePattern::from_admitted_parts(ty, kind));
        let pending = match kind {
            RuntimePatternSeedKind::Bind { mutable, local } => {
                return Ok(complete(RuntimePatternKind::Bind {
                    mutable,
                    binding: self.lower_pattern_binding(&local, ty, admission, path, mutable)?,
                }));
            }
            RuntimePatternSeedKind::Discard => return Ok(complete(RuntimePatternKind::Discard)),
            RuntimePatternSeedKind::Literal(value) => {
                self.validate_plan_value("pattern literal", ty, &value)?;
                return Ok(complete(RuntimePatternKind::Literal(value)));
            }
            RuntimePatternSeedKind::Entity(entity) => {
                self.require_projection("entity-reference pattern", ty, |projection| {
                    matches!(projection, RuntimePlanTypeProjection::EntityReference)
                })?;
                return Ok(complete(RuntimePatternKind::Entity(entity)));
            }
            RuntimePatternSeedKind::Typed { local } => {
                return Ok(complete(RuntimePatternKind::Typed {
                    binding: self.lower_pattern_binding(&local, ty, admission, path, false)?,
                }));
            }
            RuntimePatternSeedKind::Or(seeds) => {
                if seeds.len() < 2 {
                    return invalid_projection("Or alternative count", ty);
                }
                Pending::Or {
                    values: Vec::with_capacity(seeds.len()),
                    seeds: seeds.into_vec().into_iter(),
                    checkpoint: admission.checkpoint(),
                    expected: None,
                }
            }
            RuntimePatternSeedKind::Tuple(seeds) => {
                let expected = match self.projection(ty)? {
                    RuntimePlanTypeProjection::Tuple(items) => items,
                    _ => return invalid_projection("tuple pattern", ty),
                };
                if expected.len() != seeds.len() {
                    return invalid_projection("tuple pattern arity", ty);
                }
                Pending::Tuple {
                    values: Vec::with_capacity(seeds.len()),
                    seeds: seeds.into_vec().into_iter(),
                }
            }
            RuntimePatternSeedKind::Record { fields, rest } => {
                let field_count = match self.projection(ty)? {
                    RuntimePlanTypeProjection::Record(fields) => fields.len(),
                    RuntimePlanTypeProjection::Nominal { .. } => self
                        .nominal_record_domains
                        .get(ty)
                        .map(|domain| domain.fields().len())
                        .ok_or(RuntimePlanBuildError::UnknownNominalRecordDomain { owner: ty })?,
                    _ => return invalid_projection("record pattern owner", ty),
                };
                Pending::Record(RecordFrame {
                    values: Vec::with_capacity(fields.len()),
                    seeds: fields.into_vec().into_iter(),
                    rest: Some(rest),
                    field_count,
                    admitted: BTreeSet::new(),
                    current: None,
                })
            }
            RuntimePatternSeedKind::Sequence { items, rest } => {
                self.prepare_sequence_pattern(ty, items, rest)?
            }
            RuntimePatternSeedKind::Variant { ordinal, payload } => Pending::Variant {
                expected: self.variant_case(ty, ordinal)?.payload(),
                ordinal,
                seed: payload,
                payload: None,
            },
            RuntimePatternSeedKind::Whole { local, pattern } => Pending::Whole(WholeFrame {
                binding: Some(self.lower_pattern_binding(&local, ty, admission, path, false)?),
                seed: Some(pattern),
                value: None,
            }),
        };
        Ok(Prepared::Composite(Frame {
            ty,
            path_depth: path.len(),
            pending,
        }))
    }
    fn prepare_sequence_pattern(
        &self,
        ty: RuntimePlanTypeId,
        items: Box<[RuntimePatternSeed]>,
        rest: RuntimePatternRestSeed,
    ) -> Result<Pending, RuntimePlanBuildError> {
        let (item, fixed_len) = self.sequence_projection(ty, "sequence pattern")?;
        if fixed_len.is_some() && matches!(&rest, RuntimePatternRestSeed::Bind(_)) {
            return invalid_projection("array rest-binding pattern", ty);
        }
        if let Some(expected) = fixed_len {
            let actual = items.len();
            if matches!(&rest, RuntimePatternRestSeed::Exact) {
                validate_sequence_length(expected, actual)?;
            } else if u64::try_from(actual).map_or(true, |actual| actual > expected) {
                return Err(RuntimePlanBuildError::SequenceLengthMismatch { expected, actual });
            }
        }
        Ok(Pending::Sequence {
            values: Vec::with_capacity(items.len()),
            seeds: items.into_vec().into_iter(),
            rest: Some(rest),
            item,
        })
    }
}

impl Frame {
    fn accept(
        &mut self,
        pattern: RuntimePattern,
        admission: &PatternAdmission,
        construction: &RuntimePlanBodyConstruction<'_>,
    ) -> Result<(), RuntimePlanBuildError> {
        match &mut self.pending {
            Pending::Or {
                checkpoint,
                expected,
                values,
                ..
            } => {
                require_same("Or alternative", self.ty, pattern.ty())?;
                if let Some(expected) = expected {
                    admission.validate_alternative(*checkpoint, expected, self.ty)?;
                } else {
                    *expected = Some(admission.introduced_since(*checkpoint));
                }
                values.push(pattern);
            }
            Pending::Tuple { values, .. } => {
                let RuntimePlanTypeProjection::Tuple(expected) =
                    construction.projection(self.ty)?
                else {
                    return invalid_projection("tuple pattern", self.ty);
                };
                require_same(
                    "tuple pattern element",
                    expected[values.len()],
                    pattern.ty(),
                )?;
                values.push(pattern);
            }
            Pending::Record(record) => record.accept(pattern)?,
            Pending::Sequence { item, values, .. } => {
                require_same("sequence pattern element", *item, pattern.ty())?;
                values.push(pattern);
            }
            Pending::Variant {
                ordinal,
                expected,
                payload,
                ..
            } => {
                if *expected != Some(pattern.ty()) {
                    return Err(RuntimePlanBuildError::VariantPayloadMismatch {
                        owner: self.ty,
                        ordinal: *ordinal,
                        expected: *expected,
                        actual: Some(pattern.ty()),
                    });
                }
                *payload = Some(pattern);
            }
            Pending::Whole(whole) => whole.accept(self.ty, pattern)?,
        }
        Ok(())
    }

    fn advance(
        &mut self,
        construction: &RuntimePlanBodyConstruction<'_>,
        admission: &mut PatternAdmission,
        path: &mut Vec<RuntimePatternBindingStep>,
    ) -> Result<Progress, RuntimePlanBuildError> {
        match &mut self.pending {
            Pending::Or {
                seeds,
                checkpoint,
                values,
                ..
            } => {
                if let Some(seed) = seeds.next() {
                    admission.rewind(*checkpoint);
                    return Ok(Progress::Child(seed, None));
                }
                Ok(Progress::Complete(RuntimePatternKind::Or(
                    std::mem::take(values).into_boxed_slice(),
                )))
            }
            Pending::Tuple { seeds, values, .. } => {
                if let Some(seed) = seeds.next() {
                    let ordinal = u32::try_from(values.len()).map_err(|_| {
                        RuntimePlanBuildError::InvalidTypeProjection {
                            context: "tuple pattern ordinal",
                            ty: self.ty,
                        }
                    })?;
                    return Ok(Progress::Child(
                        seed,
                        Some(RuntimePatternBindingStep::TupleElement(ordinal)),
                    ));
                }
                Ok(Progress::Complete(RuntimePatternKind::Tuple(
                    std::mem::take(values).into_boxed_slice(),
                )))
            }
            Pending::Record(record) => record.advance(self.ty, construction, admission, path),
            Pending::Sequence {
                seeds,
                rest,
                values,
                ..
            } => {
                if let Some(seed) = seeds.next() {
                    let ordinal = u32::try_from(values.len()).map_err(|_| {
                        RuntimePlanBuildError::InvalidTypeProjection {
                            context: "sequence pattern ordinal",
                            ty: self.ty,
                        }
                    })?;
                    return Ok(Progress::Child(
                        seed,
                        Some(RuntimePatternBindingStep::SequenceElement(ordinal)),
                    ));
                }
                let rest = construction.lower_pattern_rest(
                    rest.take().expect("sequence remainder is consumed once"),
                    self.ty,
                    RuntimePatternBindingStep::SequenceRest,
                    admission,
                    path,
                )?;
                Ok(Progress::Complete(RuntimePatternKind::Sequence {
                    items: std::mem::take(values).into_boxed_slice(),
                    rest,
                }))
            }
            Pending::Variant {
                seed,
                ordinal,
                expected,
                payload,
            } => {
                if let Some(seed) = seed.take() {
                    return Ok(Progress::Child(
                        *seed,
                        Some(RuntimePatternBindingStep::VariantPayload),
                    ));
                }
                let actual = payload.as_ref().map(RuntimePattern::ty);
                if *expected != actual {
                    return Err(RuntimePlanBuildError::VariantPayloadMismatch {
                        owner: self.ty,
                        ordinal: *ordinal,
                        expected: *expected,
                        actual,
                    });
                }
                Ok(Progress::Complete(RuntimePatternKind::Variant {
                    ordinal: *ordinal,
                    payload: payload.take().map(Box::new),
                }))
            }
            Pending::Whole(whole) => Ok(whole.advance()),
        }
    }
}

struct RecordFrame {
    seeds: std::vec::IntoIter<RuntimeRecordPatternFieldSeed>,
    rest: Option<RuntimePatternRestSeed>,
    field_count: usize,
    admitted: BTreeSet<RuntimeRecordFieldId>,
    current: Option<(RuntimeRecordFieldId, RuntimePlanTypeId)>,
    values: Vec<RuntimeRecordPatternField>,
}

impl RecordFrame {
    fn accept(&mut self, pattern: RuntimePattern) -> Result<(), RuntimePlanBuildError> {
        let (field, expected) = self
            .current
            .take()
            .expect("record child has its checked field");
        require_same("record pattern field", expected, pattern.ty())?;
        self.values
            .push(RuntimeRecordPatternField::from_admitted_parts(
                field, pattern,
            ));
        Ok(())
    }

    fn advance(
        &mut self,
        owner: RuntimePlanTypeId,
        construction: &RuntimePlanBodyConstruction<'_>,
        admission: &mut PatternAdmission,
        path: &mut Vec<RuntimePatternBindingStep>,
    ) -> Result<Progress, RuntimePlanBuildError> {
        if let Some(seed) = self.seeds.next() {
            let (field, seed) = seed.into_parts();
            let (field, ty) = construction.resolve_pattern_record_field(owner, field)?;
            if !self.admitted.insert(field) {
                return Err(RuntimePlanBuildError::DuplicateRecordField { owner, field });
            }
            let ordinal = u32::try_from(self.values.len()).map_err(|_| {
                RuntimePlanBuildError::InvalidTypeProjection {
                    context: "record pattern ordinal",
                    ty: owner,
                }
            })?;
            self.current = Some((field, ty));
            return Ok(Progress::Child(
                seed,
                Some(RuntimePatternBindingStep::RecordField(ordinal)),
            ));
        }
        let rest = self.rest.take().expect("record remainder is consumed once");
        if matches!(&rest, RuntimePatternRestSeed::Exact) && self.admitted.len() != self.field_count
        {
            for ordinal in 0..self.field_count {
                let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)?;
                if !self.admitted.contains(&field) {
                    return Err(RuntimePlanBuildError::MissingRecordField { owner, field });
                }
            }
        }
        let rest = construction.lower_pattern_rest(
            rest,
            owner,
            RuntimePatternBindingStep::RecordRest,
            admission,
            path,
        )?;
        Ok(Progress::Complete(RuntimePatternKind::Record {
            fields: std::mem::take(&mut self.values).into_boxed_slice(),
            rest,
        }))
    }
}

struct WholeFrame {
    seed: Option<Box<RuntimePatternSeed>>,
    binding: Option<RuntimePatternBindingCoordinate>,
    value: Option<RuntimePattern>,
}
impl WholeFrame {
    fn accept(
        &mut self,
        expected: RuntimePlanTypeId,
        pattern: RuntimePattern,
    ) -> Result<(), RuntimePlanBuildError> {
        require_same("whole pattern child", expected, pattern.ty())?;
        self.value = Some(pattern);
        Ok(())
    }

    fn advance(&mut self) -> Progress {
        if let Some(seed) = self.seed.take() {
            return Progress::Child(*seed, None);
        }
        Progress::Complete(RuntimePatternKind::Whole {
            binding: self
                .binding
                .take()
                .expect("whole binding is transferred once"),
            pattern: Box::new(self.value.take().expect("whole pattern received its child")),
        })
    }
}
