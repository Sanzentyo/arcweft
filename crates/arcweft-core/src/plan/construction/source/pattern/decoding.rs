//! Complete framing precedes the source offset index and actual seed arena.

use super::{
    Cursor, Kind, PreparedRuntimePatternSource, RuntimePatternSourceError as Error, WireRest,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::construction::source::RuntimePlanSourceConstruction;
use crate::plan::value_admission::PlanValueAuthority;
use crate::plan::{
    RuntimeLocalSeedId, RuntimePatternRestSeed, RuntimePatternSeed, RuntimePatternSeedKind,
    RuntimeRecordFieldSeedId, RuntimeRecordPatternFieldSeed,
};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::value::{AwbcRuntimeValueSnapshot, RuntimeLiteralSourceError, RuntimeValue};

impl PreparedRuntimePatternSource<'_> {
    pub(in crate::plan) fn materialize(
        self,
        source: &mut RuntimePlanSourceConstruction,
        resources: &mut ValueResourceAdmission,
    ) -> Result<RuntimePatternSeed, Error> {
        source.meter.status()?;
        let result = (|| {
            for (kind, expected, actual) in [
                ("type", self.limits.type_count, source.builder.types.len()),
                ("local", self.limits.local_count, source.locals.len()),
            ] {
                if expected != actual {
                    return Err(Error::Namespace {
                        kind,
                        expected,
                        actual,
                    });
                }
            }
            let mut cursor = Cursor::new(self.bytes, self.limits);
            let count = cursor.header()?;
            debug_assert_eq!(count, self.footprint.nodes);
            let mut offsets = Vec::with_capacity(count);
            for _ in 0..count {
                offsets.push(cursor.offset);
                let _ = cursor.row()?;
            }
            cursor.finish()?;
            let mut seeds = Vec::with_capacity(count);
            for offset in offsets.into_iter().rev() {
                cursor.offset = offset;
                let row = cursor.row()?;
                let available = seeds.len();
                let children = row.kind.children();
                if children > available {
                    return Err(Error::Operands {
                        children,
                        available,
                    });
                }
                let semantic = source
                    .builder
                    .types
                    .get(row.ty)
                    .expect("prepared source type belongs to the actual builder table")
                    .semantic_identity();
                let kind = match row.kind {
                    Kind::Bind { mutable, local } => RuntimePatternSeedKind::Bind {
                        mutable,
                        local: local_handle(source, local)?,
                    },
                    Kind::Discard => RuntimePatternSeedKind::Discard,
                    Kind::Literal(bytes) => RuntimePatternSeedKind::Literal(
                        self.literal(source, row.ty, bytes, resources)?,
                    ),
                    Kind::Entity(bytes) => {
                        let RuntimeValue::EntityRef(entity) =
                            self.literal(source, row.ty, bytes, resources)?
                        else {
                            return Err(Error::Entity);
                        };
                        RuntimePatternSeedKind::Entity(entity)
                    }
                    Kind::Tuple(count) => {
                        RuntimePatternSeedKind::Tuple(take_children(&mut seeds, count))
                    }
                    Kind::Or(count) => RuntimePatternSeedKind::Or(take_children(&mut seeds, count)),
                    Kind::Record { fields, rest } => {
                        let fields = fields
                            .chunks_exact(4)
                            .map(|field| {
                                let ordinal = u32::from_le_bytes(
                                    field.try_into().expect("source field width"),
                                );
                                RuntimeRecordPatternFieldSeed::new(
                                    RuntimeRecordFieldSeedId::from_zero_based(ordinal - 1),
                                    seeds.pop().expect("prepared record child"),
                                )
                            })
                            .collect();
                        RuntimePatternSeedKind::Record {
                            fields,
                            rest: rest_seed(source, rest)?,
                        }
                    }
                    Kind::Sequence { items, rest } => RuntimePatternSeedKind::Sequence {
                        items: take_children(&mut seeds, items),
                        rest: rest_seed(source, rest)?,
                    },
                    Kind::Variant { ordinal, payload } => RuntimePatternSeedKind::Variant {
                        ordinal,
                        payload: payload
                            .then(|| Box::new(seeds.pop().expect("prepared variant child"))),
                    },
                    Kind::Whole(local) => RuntimePatternSeedKind::Whole {
                        local: local_handle(source, local)?,
                        pattern: Box::new(seeds.pop().expect("prepared whole child")),
                    },
                    Kind::Typed(local) => RuntimePatternSeedKind::Typed {
                        local: local_handle(source, local)?,
                    },
                };
                seeds.push(RuntimePatternSeed::new(semantic, kind));
            }
            if seeds.len() != 1 {
                return Err(Error::Roots {
                    actual: seeds.len(),
                });
            }
            Ok(seeds.pop().expect("one checked source root"))
        })();
        result.map_err(|error: Error| error.reject_in_bank(&mut source.meter))
    }

    fn literal(
        &self,
        source: &mut RuntimePlanSourceConstruction,
        ty: RuntimePlanTypeId,
        bytes: &[u8],
        resources: &mut ValueResourceAdmission,
    ) -> Result<RuntimeValue, Error> {
        let inert = AwbcRuntimeValueSnapshot::preflight_literal_source(
            bytes,
            self.limits.literal(),
            resources,
            &mut source.meter,
        )?
        .materialize(&mut source.meter)?;
        PlanValueAuthority::Building {
            types: &source.builder.types,
            records: &source.builder.nominal_record_domains,
            variants: &source.builder.variant_domains,
        }
        .materialize_literal_source_value(ty, inert, resources)
        .map_err(|error| Error::Literal(RuntimeLiteralSourceError::Value(error)))
    }
}

fn take_children(seeds: &mut Vec<RuntimePatternSeed>, count: usize) -> Box<[RuntimePatternSeed]> {
    (0..count)
        .map(|_| seeds.pop().expect("prepared child arity"))
        .collect()
}

fn local_handle(
    source: &RuntimePlanSourceConstruction,
    id: RuntimeLocalDeclarationId,
) -> Result<RuntimeLocalSeedId, Error> {
    source.local(id).cloned().ok_or(Error::Reference {
        kind: "local",
        actual: id.get().get(),
        maximum: source.locals.len(),
    })
}

fn rest_seed(
    source: &RuntimePlanSourceConstruction,
    rest: WireRest,
) -> Result<RuntimePatternRestSeed, Error> {
    Ok(match rest {
        WireRest::Exact => RuntimePatternRestSeed::Exact,
        WireRest::Ignore => RuntimePatternRestSeed::Ignore,
        WireRest::Bind(local) => RuntimePatternRestSeed::Bind(local_handle(source, local)?),
    })
}
