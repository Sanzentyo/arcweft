//! Matrix usefulness, specialization, and recursive witness construction.

use super::{
    CheckedCoverageWitness, CheckedMatchBuildError, CheckedMatchLimitKind,
    CheckedSequencePartitionWitness, CheckedVariantCoverageWitness,
    CheckedVariantRecordCoverageWitnessField, CoverageConstructor, CoverageConstructorId,
    CoverageTypeDomain, DeconstructedPattern, DeconstructedPatternKind, MatchCoverageAnalyzer,
    Matrix, PatternVector, SequencePartition, checked_depth_successor, checked_len,
};
use crate::{
    semantic_coordinate::StableSemanticCoordinate,
    types::{SemanticTypeDigest, TypeKind, VariantPayloadShape},
};

struct UsefulConstructorContext<'a> {
    matrix: &'a Matrix,
    query: &'a [DeconstructedPattern],
    head_type: &'a TypeKind,
    tail_types: &'a [TypeKind],
    head: &'a DeconstructedPattern,
    constructor: &'a CoverageConstructorId,
    fields: &'a [DeconstructedPattern],
    child_depth: u64,
    active_witness_types: &'a mut Vec<SemanticTypeDigest>,
}

struct UsefulWildcardContext<'a> {
    matrix: &'a Matrix,
    query: &'a [DeconstructedPattern],
    head_type: &'a TypeKind,
    tail_types: &'a [TypeKind],
    head: &'a DeconstructedPattern,
    child_depth: u64,
    active_witness_types: &'a mut Vec<SemanticTypeDigest>,
}

impl MatchCoverageAnalyzer<'_, '_> {
    pub(super) fn useful(
        &mut self,
        matrix: &Matrix,
        query: &[DeconstructedPattern],
        types: &[TypeKind],
        depth: u64,
        active_witness_types: &mut Vec<SemanticTypeDigest>,
    ) -> Result<Option<Vec<CheckedCoverageWitness>>, CheckedMatchBuildError> {
        self.poll()?;
        self.budget.observe_depth(depth)?;
        self.budget
            .charge(CheckedMatchLimitKind::Specializations, 1)?;
        if query.is_empty() {
            return Ok((!matrix.iter().any(Vec::is_empty)).then(Vec::new));
        }
        let failure_coordinate = query.first().map_or_else(
            || self.match_coordinate.clone(),
            |pattern| pattern.semantic_coordinate.clone(),
        );
        let child_depth = checked_depth_successor(depth)?;
        let (head_type, tail_types) =
            types
                .split_first()
                .ok_or_else(|| CheckedMatchBuildError::InvalidCheckedRow {
                    coordinate: failure_coordinate.clone(),
                })?;
        let head = &query[0];
        match &head.kind {
            DeconstructedPatternKind::Or(_) => Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: head.semantic_coordinate.clone(),
            }),
            DeconstructedPatternKind::Constructor {
                constructor,
                fields,
            } => self.useful_constructor_head(UsefulConstructorContext {
                matrix,
                query,
                head_type,
                tail_types,
                head,
                constructor,
                fields,
                child_depth,
                active_witness_types,
            }),
            DeconstructedPatternKind::Wildcard => {
                self.useful_wildcard_head(UsefulWildcardContext {
                    matrix,
                    query,
                    head_type,
                    tail_types,
                    head,
                    child_depth,
                    active_witness_types,
                })
            }
        }
    }

    fn useful_constructor_head(
        &mut self,
        context: UsefulConstructorContext<'_>,
    ) -> Result<Option<Vec<CheckedCoverageWitness>>, CheckedMatchBuildError> {
        let UsefulConstructorContext {
            matrix,
            query,
            head_type,
            tail_types,
            head,
            constructor,
            fields,
            child_depth,
            active_witness_types,
        } = context;
        let domain = self.domain(head_type, &head.semantic_coordinate)?;
        let CoverageTypeDomain::Constructors(constructors) = domain else {
            return Ok(None);
        };
        let selected = constructors
            .iter()
            .find(|candidate| &candidate.identity == constructor)
            .ok_or_else(|| CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: head.semantic_coordinate.clone(),
            })?;
        if selected.field_types.len() != fields.len() {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: head.semantic_coordinate.clone(),
            });
        }
        self.budget.charge(CheckedMatchLimitKind::WitnessNodes, 1)?;
        let specialized = self.specialize(matrix, selected)?;
        let mut specialized_query = fields.to_vec();
        specialized_query.extend_from_slice(&query[1..]);
        let mut specialized_types = selected.field_types.to_vec();
        specialized_types.extend_from_slice(tail_types);
        let Some(mut witnesses) = self.useful(
            &specialized,
            &specialized_query,
            &specialized_types,
            child_depth,
            active_witness_types,
        )?
        else {
            return Ok(None);
        };
        let field_count = selected.field_types.len();
        let tail = witnesses.split_off(field_count);
        let witness = Self::constructor_witness(selected, witnesses, &head.semantic_coordinate)?;
        let mut result = vec![witness];
        result.extend(tail);
        Ok(Some(result))
    }

    fn useful_wildcard_head(
        &mut self,
        context: UsefulWildcardContext<'_>,
    ) -> Result<Option<Vec<CheckedCoverageWitness>>, CheckedMatchBuildError> {
        let UsefulWildcardContext {
            matrix,
            query,
            head_type,
            tail_types,
            head,
            child_depth,
            active_witness_types,
        } = context;
        let domain = self.domain(head_type, &head.semantic_coordinate)?;
        let CoverageTypeDomain::Constructors(constructors) = domain else {
            return Ok(None);
        };
        let default = self.default_matrix(matrix)?;
        if !default.is_empty()
            && self
                .useful(
                    &default,
                    &query[1..],
                    tail_types,
                    child_depth,
                    active_witness_types,
                )?
                .is_none()
        {
            return Ok(None);
        }
        let type_digest = head_type.semantic_identity_digest();
        let already_active = active_witness_types.contains(&type_digest);
        if !already_active {
            active_witness_types.push(type_digest);
        }
        let mut recursive_witness_skipped = false;
        for constructor in &constructors {
            self.poll()?;
            if already_active
                && constructor
                    .field_types
                    .iter()
                    .any(|field| active_witness_types.contains(&field.semantic_identity_digest()))
            {
                recursive_witness_skipped = true;
                continue;
            }
            self.budget.charge(CheckedMatchLimitKind::WitnessNodes, 1)?;
            let specialized = self.specialize(matrix, constructor)?;
            let mut specialized_query = constructor
                .field_types
                .iter()
                .map(|_| {
                    DeconstructedPattern::wildcard(
                        head.coordinate.clone(),
                        head.semantic_coordinate.clone(),
                    )
                })
                .collect::<Vec<_>>();
            specialized_query.extend_from_slice(&query[1..]);
            let mut specialized_types = constructor.field_types.to_vec();
            specialized_types.extend_from_slice(tail_types);
            if let Some(mut witnesses) = self.useful(
                &specialized,
                &specialized_query,
                &specialized_types,
                child_depth,
                active_witness_types,
            )? {
                if !already_active {
                    active_witness_types.pop();
                }
                let field_count = constructor.field_types.len();
                let tail = witnesses.split_off(field_count);
                let witness =
                    Self::constructor_witness(constructor, witnesses, &head.semantic_coordinate)?;
                let mut result = vec![witness];
                result.extend(tail);
                return Ok(Some(result));
            }
        }
        if !already_active {
            active_witness_types.pop();
        }
        if recursive_witness_skipped {
            return Err(CheckedMatchBuildError::UnsupportedDomain { type_digest });
        }
        Ok(None)
    }

    fn default_matrix(&mut self, matrix: &Matrix) -> Result<Matrix, CheckedMatchBuildError> {
        self.poll()?;
        self.budget
            .charge(CheckedMatchLimitKind::Specializations, 1)?;
        let mut default = Matrix::new();
        for row in matrix {
            self.poll()?;
            let Some((head, tail)) = row.split_first() else {
                continue;
            };
            if matches!(head.kind, DeconstructedPatternKind::Wildcard) {
                self.push_matrix_row(&mut default, tail.to_vec())?;
            }
        }
        Ok(default)
    }

    fn specialize(
        &mut self,
        matrix: &Matrix,
        constructor: &CoverageConstructor,
    ) -> Result<Matrix, CheckedMatchBuildError> {
        self.poll()?;
        self.budget
            .charge(CheckedMatchLimitKind::Specializations, 1)?;
        let mut specialized = Matrix::new();
        for row in matrix {
            self.poll()?;
            let Some((head, tail)) = row.split_first() else {
                continue;
            };
            let mut emitted = match &head.kind {
                DeconstructedPatternKind::Wildcard => constructor
                    .field_types
                    .iter()
                    .map(|_| {
                        DeconstructedPattern::wildcard(
                            head.coordinate.clone(),
                            head.semantic_coordinate.clone(),
                        )
                    })
                    .collect::<Vec<_>>(),
                DeconstructedPatternKind::Constructor {
                    constructor: observed,
                    fields,
                } if observed == &constructor.identity => fields.to_vec(),
                DeconstructedPatternKind::Constructor { .. } => continue,
                DeconstructedPatternKind::Or(_) => {
                    return Err(CheckedMatchBuildError::InvalidCheckedRow {
                        coordinate: head.semantic_coordinate.clone(),
                    });
                }
            };
            emitted.extend_from_slice(tail);
            self.push_matrix_row(&mut specialized, emitted)?;
        }
        Ok(specialized)
    }

    pub(super) fn push_matrix_row(
        &mut self,
        matrix: &mut Matrix,
        row: PatternVector,
    ) -> Result<(), CheckedMatchBuildError> {
        self.budget.charge(CheckedMatchLimitKind::MatrixRows, 1)?;
        matrix.push(row);
        Ok(())
    }

    pub(super) fn clone_matrix(
        &mut self,
        matrix: &Matrix,
    ) -> Result<Matrix, CheckedMatchBuildError> {
        self.budget.charge(
            CheckedMatchLimitKind::MatrixRows,
            checked_len(matrix.len(), CheckedMatchLimitKind::MatrixRows)?,
        )?;
        Ok(matrix.clone())
    }

    pub(super) fn constructor_witness(
        constructor: &CoverageConstructor,
        fields: Vec<CheckedCoverageWitness>,
        coordinate: &StableSemanticCoordinate,
    ) -> Result<CheckedCoverageWitness, CheckedMatchBuildError> {
        Ok(match &constructor.identity {
            CoverageConstructorId::Unit => CheckedCoverageWitness::Unit,
            CoverageConstructorId::Bool(value) => CheckedCoverageWitness::Bool(*value),
            CoverageConstructorId::Literal(literal) => {
                CheckedCoverageWitness::Literal(literal.clone())
            }
            CoverageConstructorId::Entity { item, .. } => CheckedCoverageWitness::Entity(*item),
            CoverageConstructorId::Other(type_digest) => CheckedCoverageWitness::Other {
                type_digest: *type_digest,
            },
            CoverageConstructorId::Variant { case, .. } => {
                let payload = match constructor.variant_payload.as_ref() {
                    Some(VariantPayloadShape::Unit) if fields.is_empty() => {
                        CheckedVariantCoverageWitness::Unit
                    }
                    Some(VariantPayloadShape::Tuple(schema)) if schema.len() == fields.len() => {
                        CheckedVariantCoverageWitness::Tuple(fields.into_boxed_slice())
                    }
                    Some(VariantPayloadShape::Record(schema)) if schema.len() == fields.len() => {
                        let rows = schema
                            .iter()
                            .zip(fields)
                            .map(|(field, value)| CheckedVariantRecordCoverageWitnessField {
                                semantic_id: field.semantic_id(),
                                value,
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice();
                        CheckedVariantCoverageWitness::Record(rows)
                    }
                    Some(
                        VariantPayloadShape::Unit
                        | VariantPayloadShape::Tuple(_)
                        | VariantPayloadShape::Record(_),
                    )
                    | None => {
                        return Err(CheckedMatchBuildError::InvalidCheckedRow {
                            coordinate: coordinate.clone(),
                        });
                    }
                };
                CheckedCoverageWitness::Variant {
                    case: *case,
                    payload,
                }
            }
            CoverageConstructorId::Tuple { .. } => {
                CheckedCoverageWitness::Tuple(fields.into_boxed_slice())
            }
            CoverageConstructorId::Record { owner } => CheckedCoverageWitness::Record {
                owner: *owner,
                fields: fields.into_boxed_slice(),
            },
            CoverageConstructorId::Array { .. } => {
                CheckedCoverageWitness::Array(fields.into_boxed_slice())
            }
            CoverageConstructorId::Sequence { partition, .. } => {
                let partition = match partition {
                    SequencePartition::Exact(value) => {
                        CheckedSequencePartitionWitness::Exact(*value)
                    }
                    SequencePartition::Interval {
                        lower,
                        upper_exclusive,
                    } => CheckedSequencePartitionWitness::Interval {
                        lower: *lower,
                        upper_exclusive: *upper_exclusive,
                    },
                };
                CheckedCoverageWitness::Sequence {
                    partition,
                    visible_prefix: fields.into_boxed_slice(),
                }
            }
            CoverageConstructorId::Choice {
                ordinal,
                alternative,
                ..
            } => {
                let [value] = fields.as_slice() else {
                    return Err(CheckedMatchBuildError::InvalidCheckedRow {
                        coordinate: coordinate.clone(),
                    });
                };
                CheckedCoverageWitness::Choice {
                    ordinal: *ordinal,
                    alternative: *alternative,
                    value: Box::new(value.clone()),
                }
            }
        })
    }
}
