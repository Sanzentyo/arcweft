//! Borrowed source preparation and reserved-capacity emission.

use super::{
    Counts, DOMAIN, Footprint, RuntimePatternSourceError as Error, RuntimePatternSourceLimits, Tag,
    charge, limit, reference,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::{
    RuntimePattern, RuntimePatternBindingCoordinate, RuntimePatternBindingStep, RuntimePatternKind,
    RuntimePatternRest,
};
use crate::task::semantic::TaskSemanticMeter;
use crate::value::PreparedRuntimeLiteralEncoding;

struct Walk<'a> {
    pattern: &'a RuntimePattern,
    next: usize,
    path_len: usize,
    entered: bool,
}

pub(super) fn scratch_bytes(nodes: usize, literals: usize) -> Option<usize> {
    let walk = nodes.checked_mul(
        size_of::<Walk<'_>>()
            + size_of::<&RuntimePattern>()
            + size_of::<RuntimePatternBindingStep>(),
    )?;
    let prepared = nodes
        .checked_mul(size_of::<&RuntimePattern>())?
        .checked_add(literals.checked_mul(size_of::<PreparedRuntimeLiteralEncoding<'_>>())?)?;
    Some(walk.max(prepared))
}

pub(in crate::plan) struct PreparedRuntimePatternEncoding<'a> {
    rows: Vec<&'a RuntimePattern>,
    literals: Vec<PreparedRuntimeLiteralEncoding<'a>>,
    encoded_bytes: usize,
    storage_bytes: usize,
    footprint: Footprint,
}

impl PreparedRuntimePatternEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.encoded_bytes {
            return Err(Error::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let start = output.len();
        output.extend_from_slice(DOMAIN);
        output.push(1);
        put(
            output,
            u32::try_from(self.rows.len()).expect("prepared source node count"),
        );
        let mut literals = self.literals.into_iter();
        for pattern in self.rows {
            let (tag, _) = layout(pattern)?;
            output.push(tag as u8);
            put(output, pattern.ty().get().get());
            match pattern.kind() {
                RuntimePatternKind::Bind { mutable, binding } => {
                    output.push(u8::from(*mutable));
                    put(output, binding.local().get().get());
                }
                RuntimePatternKind::Discard => {}
                RuntimePatternKind::Literal(_) | RuntimePatternKind::Entity(_) => {
                    let literal = literals.next().expect("prepared literal order");
                    put(
                        output,
                        u32::try_from(literal.encoded_bytes())
                            .expect("prepared source literal length"),
                    );
                    literal.append_to(output)?;
                }
                RuntimePatternKind::Tuple(items) | RuntimePatternKind::Or(items) => put(
                    output,
                    u32::try_from(items.len()).expect("prepared source arity"),
                ),
                RuntimePatternKind::Record { fields, rest } => {
                    put(
                        output,
                        u32::try_from(fields.len()).expect("prepared source fields"),
                    );
                    for field in fields {
                        put(output, field.field().get().get());
                    }
                    emit_rest(rest, output);
                }
                RuntimePatternKind::Sequence { items, rest } => {
                    put(
                        output,
                        u32::try_from(items.len()).expect("prepared source items"),
                    );
                    emit_rest(rest, output);
                }
                RuntimePatternKind::Variant { ordinal, payload } => {
                    put(output, *ordinal);
                    output.push(u8::from(payload.is_some()));
                }
                RuntimePatternKind::Whole { binding, .. }
                | RuntimePatternKind::Typed { binding } => put(output, binding.local().get().get()),
            }
        }
        debug_assert!(literals.next().is_none());
        debug_assert_eq!(output.len() - start, self.encoded_bytes);
        Ok(())
    }
}

impl RuntimePattern {
    pub(in crate::plan) fn prepare_sources_encoding<'a>(
        &'a self,
        limits: RuntimePatternSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimePatternEncoding<'a>, Error> {
        meter.status()?;
        let result = (|| {
            let rows = gather(self, limits, meter)?;
            let mut counts = Counts::new(limits, rows.len())?;
            let mut length = DOMAIN.len() + 1 + 4;
            let mut literals = Vec::new();
            for pattern in &rows {
                reference("type", pattern.ty().get().get(), limits.type_count)?;
                let (_, fields) = layout(pattern)?;
                length = length
                    .checked_add(5)
                    .and_then(|n| n.checked_add(fields))
                    .ok_or(Error::Arithmetic)?;
                match pattern.kind() {
                    RuntimePatternKind::Bind { binding, .. }
                    | RuntimePatternKind::Typed { binding } => {
                        check_binding(binding, limits)?;
                        counts.binding(binding.path().steps().len())?;
                    }
                    RuntimePatternKind::Discard => {}
                    RuntimePatternKind::Literal(value) => {
                        let literal = value.prepare_literal_source_encoding(
                            limits.literal(),
                            resources,
                            meter,
                        )?;
                        u32::try_from(literal.encoded_bytes()).map_err(|_| Error::Arithmetic)?;
                        length = length
                            .checked_add(literal.encoded_bytes())
                            .ok_or(Error::Arithmetic)?;
                        counts.literal(literal.storage_bytes(), literal.elements())?;
                        limit(
                            "encoder working bytes",
                            scratch_bytes(rows.len(), literals.len() + 1)
                                .ok_or(Error::Arithmetic)?,
                            limits.decoded_bytes,
                        )?;
                        literals.reserve_exact(1);
                        literals.push(literal);
                    }
                    RuntimePatternKind::Entity(entity) => {
                        let literal = entity.prepare_literal_source_encoding(
                            limits.literal(),
                            resources,
                            meter,
                        )?;
                        u32::try_from(literal.encoded_bytes()).map_err(|_| Error::Arithmetic)?;
                        length = length
                            .checked_add(literal.encoded_bytes())
                            .ok_or(Error::Arithmetic)?;
                        counts.literal(literal.storage_bytes(), literal.elements())?;
                        limit(
                            "encoder working bytes",
                            scratch_bytes(rows.len(), literals.len() + 1)
                                .ok_or(Error::Arithmetic)?,
                            limits.decoded_bytes,
                        )?;
                        literals.reserve_exact(1);
                        literals.push(literal);
                    }
                    RuntimePatternKind::Tuple(items) | RuntimePatternKind::Or(items) => {
                        counts.children(items.len(), false)?
                    }
                    RuntimePatternKind::Record { fields, rest } => {
                        counts.children(fields.len(), true)?;
                        count_rest(rest, &mut counts, limits)?;
                    }
                    RuntimePatternKind::Sequence { items, rest } => {
                        counts.children(items.len(), false)?;
                        count_rest(rest, &mut counts, limits)?;
                    }
                    RuntimePatternKind::Variant { payload, .. } => {
                        counts.children(usize::from(payload.is_some()), false)?
                    }
                    RuntimePatternKind::Whole { binding, .. } => {
                        check_binding(binding, limits)?;
                        counts.binding(binding.path().steps().len())?;
                        counts.children(1, false)?;
                    }
                }
                limit("encoded bytes", length, limits.encoded_bytes)?;
            }
            charge(meter, length)?;
            Ok(PreparedRuntimePatternEncoding {
                rows,
                literals,
                encoded_bytes: length,
                storage_bytes: counts.storage_bytes()?,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject_in_bank(meter))
    }
}

fn gather<'a>(
    root: &'a RuntimePattern,
    limits: RuntimePatternSourceLimits,
    meter: &mut TaskSemanticMeter,
) -> Result<Vec<&'a RuntimePattern>, Error> {
    let mut rows = Vec::new();
    let mut frames = Vec::new();
    let mut path = Vec::new();
    limit(
        "encoder working bytes",
        scratch_bytes(1, 0).ok_or(Error::Arithmetic)?,
        limits.decoded_bytes,
    )?;
    frames.push(Walk {
        pattern: root,
        next: 0,
        path_len: 0,
        entered: false,
    });
    while let Some(frame) = frames.last_mut() {
        if !frame.entered {
            meter.charge_work(1)?;
            let nodes = rows.len().checked_add(1).ok_or(Error::Arithmetic)?;
            limit("pattern nodes", nodes, limits.nodes as usize)?;
            limit(
                "encoder working bytes",
                scratch_bytes(nodes, 0).ok_or(Error::Arithmetic)?,
                limits.decoded_bytes,
            )?;
            validate_coordinates(frame.pattern, &path)?;
            rows.reserve_exact(1);
            rows.push(frame.pattern);
            frame.entered = true;
        }
        path.truncate(frame.path_len);
        if let Some((pattern, step)) = child(frame.pattern, frame.next) {
            frame.next += 1;
            let pending = rows
                .len()
                .checked_add(frames.len())
                .and_then(|n| n.checked_add(1))
                .ok_or(Error::Arithmetic)?;
            limit(
                "encoder working bytes",
                scratch_bytes(pending, 0).ok_or(Error::Arithmetic)?,
                limits.decoded_bytes,
            )?;
            if let Some(step) = step {
                path.reserve_exact(1);
                path.push(step);
            }
            frames.reserve_exact(1);
            frames.push(Walk {
                pattern,
                next: 0,
                path_len: path.len(),
                entered: false,
            });
        } else {
            frames.pop();
        }
    }
    Ok(rows)
}

fn child(
    pattern: &RuntimePattern,
    index: usize,
) -> Option<(&RuntimePattern, Option<RuntimePatternBindingStep>)> {
    let ordinal = u32::try_from(index).ok()?;
    Some(match pattern.kind() {
        RuntimePatternKind::Tuple(items) => (
            items.get(index)?,
            Some(RuntimePatternBindingStep::TupleElement(ordinal)),
        ),
        RuntimePatternKind::Or(items) => (items.get(index)?, None),
        RuntimePatternKind::Record { fields, .. } => (
            fields.get(index)?.pattern(),
            Some(RuntimePatternBindingStep::RecordField(ordinal)),
        ),
        RuntimePatternKind::Sequence { items, .. } => (
            items.get(index)?,
            Some(RuntimePatternBindingStep::SequenceElement(ordinal)),
        ),
        RuntimePatternKind::Variant { payload, .. } if index == 0 => (
            payload.as_deref()?,
            Some(RuntimePatternBindingStep::VariantPayload),
        ),
        RuntimePatternKind::Whole { pattern, .. } if index == 0 => (pattern, None),
        RuntimePatternKind::Bind { .. }
        | RuntimePatternKind::Discard
        | RuntimePatternKind::Literal(_)
        | RuntimePatternKind::Entity(_)
        | RuntimePatternKind::Typed { .. }
        | RuntimePatternKind::Variant { .. }
        | RuntimePatternKind::Whole { .. } => return None,
    })
}

fn validate_coordinates(
    pattern: &RuntimePattern,
    path: &[RuntimePatternBindingStep],
) -> Result<(), Error> {
    let check = |binding: &RuntimePatternBindingCoordinate,
                 suffix: Option<RuntimePatternBindingStep>| {
        let actual = binding.path().steps();
        let valid = match suffix {
            Some(suffix) => {
                actual.len() == path.len() + 1
                    && actual.starts_with(path)
                    && actual.last() == Some(&suffix)
            }
            None if path.is_empty() => actual == [RuntimePatternBindingStep::Whole],
            None => actual == path,
        };
        if valid {
            Ok(())
        } else {
            Err(Error::Coordinate)
        }
    };
    match pattern.kind() {
        RuntimePatternKind::Bind { binding, .. }
        | RuntimePatternKind::Typed { binding }
        | RuntimePatternKind::Whole { binding, .. } => check(binding, None),
        RuntimePatternKind::Record {
            rest: RuntimePatternRest::Bind(binding),
            ..
        } => check(binding, Some(RuntimePatternBindingStep::RecordRest)),
        RuntimePatternKind::Sequence {
            rest: RuntimePatternRest::Bind(binding),
            ..
        } => check(binding, Some(RuntimePatternBindingStep::SequenceRest)),
        _ => Ok(()),
    }
}

fn check_binding(
    binding: &RuntimePatternBindingCoordinate,
    limits: RuntimePatternSourceLimits,
) -> Result<(), Error> {
    reference("local", binding.local().get().get(), limits.local_count)
}

fn count_rest(
    rest: &RuntimePatternRest,
    counts: &mut Counts,
    limits: RuntimePatternSourceLimits,
) -> Result<(), Error> {
    if let RuntimePatternRest::Bind(binding) = rest {
        check_binding(binding, limits)?;
        counts.binding(binding.path().steps().len())?;
    }
    Ok(())
}

fn layout(pattern: &RuntimePattern) -> Result<(Tag, usize), Error> {
    Ok(match pattern.kind() {
        RuntimePatternKind::Bind { .. } => (Tag::Bind, 5),
        RuntimePatternKind::Discard => (Tag::Discard, 0),
        RuntimePatternKind::Literal(_) => (Tag::Literal, 4),
        RuntimePatternKind::Entity(_) => (Tag::Entity, 4),
        RuntimePatternKind::Tuple(items) => {
            u32::try_from(items.len()).map_err(|_| Error::Arithmetic)?;
            (Tag::Tuple, 4)
        }
        RuntimePatternKind::Or(items) => {
            u32::try_from(items.len()).map_err(|_| Error::Arithmetic)?;
            if items.len() < 2 {
                return Err(Error::OrArity {
                    actual: items.len(),
                });
            }
            (Tag::Or, 4)
        }
        RuntimePatternKind::Record { fields, rest } => (
            Tag::Record,
            4usize
                .checked_add(fields.len().checked_mul(4).ok_or(Error::Arithmetic)?)
                .and_then(|n| n.checked_add(rest_bytes(rest)))
                .ok_or(Error::Arithmetic)?,
        ),
        RuntimePatternKind::Sequence { rest, .. } => (Tag::Sequence, 4 + rest_bytes(rest)),
        RuntimePatternKind::Variant { .. } => (Tag::Variant, 5),
        RuntimePatternKind::Whole { .. } => (Tag::Whole, 4),
        RuntimePatternKind::Typed { .. } => (Tag::Typed, 4),
    })
}

fn rest_bytes(rest: &RuntimePatternRest) -> usize {
    if matches!(rest, RuntimePatternRest::Bind(_)) {
        5
    } else {
        1
    }
}
fn emit_rest(rest: &RuntimePatternRest, output: &mut Vec<u8>) {
    match rest {
        RuntimePatternRest::Exact => output.push(0),
        RuntimePatternRest::Ignore => output.push(1),
        RuntimePatternRest::Bind(binding) => {
            output.push(2);
            put(output, binding.local().get().get());
        }
    }
}
fn put(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}
