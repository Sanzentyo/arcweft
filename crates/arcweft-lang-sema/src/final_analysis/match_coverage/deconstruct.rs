//! HIR pattern deconstruction and OR normalization for Match coverage.

use arcweft_lang_hir::{
    identity::PatternId,
    leaf::HirLiteral,
    pattern::{HirPatternKind, HirPatternSequenceRest, HirVariantPatternPayload},
};

use super::super::{CheckedPatternResolution, CheckedRecordPatternSourceRef};
use super::{
    CheckedMatchBuildError, CheckedMatchLimitKind, CoverageConstructor, CoverageConstructorId,
    CoverageTypeDomain, DeconstructedPattern, DeconstructedPatternKind, ExpandedPattern,
    MatchCoverageAnalyzer, SequencePartition, StableMatchArmCoordinate, append_coordinate,
    append_record_coordinate, checked_depth_successor, checked_len,
};
use crate::{
    semantic_coordinate::{
        StablePatternCoordinate, StablePatternCoordinateStep, StableSemanticCoordinate,
    },
    types::{ArrayLength, SemanticTypeDigest, TypeKind, VariantPayloadShape},
};

#[derive(Clone)]
struct PatternSite {
    arm: StableMatchArmCoordinate,
    coordinate: StablePatternCoordinate,
    depth: u64,
}

impl PatternSite {
    fn new(
        arm: &StableMatchArmCoordinate,
        coordinate: StablePatternCoordinate,
        depth: u64,
    ) -> Self {
        Self {
            arm: arm.clone(),
            coordinate,
            depth,
        }
    }

    fn semantic_coordinate(&self) -> StableSemanticCoordinate {
        self.arm.pattern_coordinate(self.coordinate.clone())
    }

    fn child(&self, step: StablePatternCoordinateStep, depth: u64) -> Self {
        Self {
            arm: self.arm.clone(),
            coordinate: append_coordinate(&self.coordinate, step),
            depth,
        }
    }

    fn at(&self, coordinate: StablePatternCoordinate, depth: u64) -> Self {
        Self {
            arm: self.arm.clone(),
            coordinate,
            depth,
        }
    }
}

struct SequencePatternForm<'a> {
    item: &'a TypeKind,
    is_array: bool,
    minimum: u64,
    has_rest: bool,
}

impl<'a> SequencePatternForm<'a> {
    fn from_expected(
        expected: &'a TypeKind,
        coordinate: StableSemanticCoordinate,
    ) -> Result<Self, CheckedMatchBuildError> {
        let (item, is_array) = match expected {
            TypeKind::Array {
                item,
                len: ArrayLength::Const(_),
            } => (item.as_ref(), true),
            TypeKind::Vec(item) | TypeKind::Seq(item) | TypeKind::Slice(item) => {
                (item.as_ref(), false)
            }
            _ => {
                return Err(CheckedMatchBuildError::InvalidCheckedRow { coordinate });
            }
        };
        Ok(Self {
            item,
            is_array,
            minimum: 0,
            has_rest: false,
        })
    }
}

impl MatchCoverageAnalyzer<'_, '_> {
    pub(super) fn deconstruct(
        &mut self,
        owner: PatternId,
        expected: &TypeKind,
        arm: &StableMatchArmCoordinate,
        coordinate: StablePatternCoordinate,
        depth: u64,
    ) -> Result<DeconstructedPattern, CheckedMatchBuildError> {
        let site = PatternSite::new(arm, coordinate, depth);
        self.deconstruct_site(owner, expected, &site)
    }

    fn deconstruct_site(
        &mut self,
        owner: PatternId,
        expected: &TypeKind,
        site: &PatternSite,
    ) -> Result<DeconstructedPattern, CheckedMatchBuildError> {
        self.poll()?;
        self.budget.observe_depth(site.depth)?;
        let semantic_coordinate = site.semantic_coordinate();
        let hir = self.module.resolve_pattern(owner).map_err(|_| {
            CheckedMatchBuildError::MissingExactOwner {
                coordinate: semantic_coordinate.clone(),
            }
        })?;
        let checked = self.analysis.pattern(owner).ok_or_else(|| {
            CheckedMatchBuildError::MissingExactOwner {
                coordinate: semantic_coordinate.clone(),
            }
        })?;
        if hir.is_poisoned()
            || checked.ty().semantic_identity_digest() != expected.semantic_identity_digest()
        {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: semantic_coordinate,
            });
        }
        if matches!(expected, TypeKind::StatementIngress(_))
            && !matches!(
                hir.kind(),
                HirPatternKind::Binding(_)
                    | HirPatternKind::MutableBinding(_)
                    | HirPatternKind::Discard
                    | HirPatternKind::WholeBinding { .. }
                    | HirPatternKind::TypedBinding { .. }
            )
        {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: semantic_coordinate,
            });
        }
        let child_depth = checked_depth_successor(site.depth)?;
        let kind = match hir.kind() {
            HirPatternKind::Binding(_)
            | HirPatternKind::MutableBinding(_)
            | HirPatternKind::Discard => DeconstructedPatternKind::Wildcard,
            HirPatternKind::WholeBinding { pattern, .. } => {
                return self.deconstruct_site(
                    *pattern,
                    expected,
                    &site.child(StablePatternCoordinateStep::WholeBindingInner, child_depth),
                );
            }
            HirPatternKind::TypedBinding { .. } => {
                Self::deconstruct_typed_binding(expected, checked.resolution(), site)?
            }
            HirPatternKind::Literal(HirLiteral::Boolean(value))
                if matches!(expected, TypeKind::Bool) =>
            {
                DeconstructedPatternKind::Constructor {
                    constructor: CoverageConstructorId::Bool(*value),
                    fields: Box::new([]),
                }
            }
            HirPatternKind::Literal(literal) => {
                self.deconstruct_literal(owner, literal, checked.resolution(), expected, site)?
            }
            HirPatternKind::EntityReference(_) => {
                Self::deconstruct_entity(checked.resolution(), expected, site)?
            }
            HirPatternKind::Tuple { elements } => {
                self.deconstruct_tuple(elements, expected, site, child_depth)?
            }
            HirPatternKind::Variant(variant) => self.deconstruct_variant(
                variant,
                checked.resolution(),
                expected,
                site,
                child_depth,
            )?,
            HirPatternKind::Record { .. } => {
                self.deconstruct_record(checked.resolution(), expected, site, child_depth)?
            }
            HirPatternKind::BracketSequence { elements, rest } => {
                return self.deconstruct_sequence(elements, rest, expected, site, child_depth);
            }
            HirPatternKind::Or { alternatives } => {
                self.deconstruct_or(alternatives, expected, site, child_depth)?
            }
            HirPatternKind::Error(_) => {
                return Err(CheckedMatchBuildError::PoisonedSemanticNode {
                    coordinate: site.semantic_coordinate(),
                });
            }
        };
        Ok(DeconstructedPattern {
            kind,
            coordinate: site.coordinate.clone(),
            semantic_coordinate: site.semantic_coordinate(),
        })
    }

    fn deconstruct_literal(
        &mut self,
        owner: PatternId,
        literal: &HirLiteral,
        resolution: &CheckedPatternResolution,
        expected: &TypeKind,
        site: &PatternSite,
    ) -> Result<DeconstructedPatternKind, CheckedMatchBuildError> {
        let CheckedPatternResolution::Literal(checked_literal) = resolution else {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: site.semantic_coordinate(),
            });
        };
        if checked_literal != literal {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: site.semantic_coordinate(),
            });
        }
        Ok(DeconstructedPatternKind::Constructor {
            constructor: CoverageConstructorId::Literal(self.canonical_literal(
                owner,
                literal,
                expected,
                &site.semantic_coordinate(),
            )?),
            fields: Box::new([]),
        })
    }

    fn deconstruct_entity(
        resolution: &CheckedPatternResolution,
        expected: &TypeKind,
        site: &PatternSite,
    ) -> Result<DeconstructedPatternKind, CheckedMatchBuildError> {
        let CheckedPatternResolution::Entity(item) = resolution else {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: site.semantic_coordinate(),
            });
        };
        Ok(DeconstructedPatternKind::Constructor {
            constructor: CoverageConstructorId::Entity {
                owner: expected.semantic_identity_digest(),
                item: item.semantic_id(),
            },
            fields: Box::new([]),
        })
    }

    fn deconstruct_typed_binding(
        expected: &TypeKind,
        resolution: &CheckedPatternResolution,
        site: &PatternSite,
    ) -> Result<DeconstructedPatternKind, CheckedMatchBuildError> {
        let CheckedPatternResolution::TypedBinding(binding) = resolution else {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: site.semantic_coordinate(),
            });
        };
        if let TypeKind::Choice(alternatives) = expected {
            let mut patterns = Vec::new();
            for ordinal in binding.choice_alternatives() {
                let index = usize::try_from(*ordinal).map_err(|_| {
                    CheckedMatchBuildError::InvalidCheckedRow {
                        coordinate: site.semantic_coordinate(),
                    }
                })?;
                let alternative = alternatives.get(index).ok_or_else(|| {
                    CheckedMatchBuildError::InvalidCheckedRow {
                        coordinate: site.semantic_coordinate(),
                    }
                })?;
                patterns.push(DeconstructedPattern {
                    coordinate: site.coordinate.clone(),
                    semantic_coordinate: site.semantic_coordinate(),
                    kind: DeconstructedPatternKind::Constructor {
                        constructor: CoverageConstructorId::Choice {
                            owner: expected.semantic_identity_digest(),
                            ordinal: *ordinal,
                            alternative: alternative.semantic_identity_digest(),
                        },
                        fields: vec![DeconstructedPattern::wildcard(
                            site.coordinate.clone(),
                            site.semantic_coordinate(),
                        )]
                        .into_boxed_slice(),
                    },
                });
            }
            if patterns.len() == alternatives.len() {
                Ok(DeconstructedPatternKind::Wildcard)
            } else {
                Ok(DeconstructedPatternKind::Or(patterns.into_boxed_slice()))
            }
        } else {
            Ok(DeconstructedPatternKind::Wildcard)
        }
    }

    fn deconstruct_tuple(
        &mut self,
        elements: &[PatternId],
        expected: &TypeKind,
        site: &PatternSite,
        child_depth: u64,
    ) -> Result<DeconstructedPatternKind, CheckedMatchBuildError> {
        let semantic_coordinate = site.semantic_coordinate();
        let field_count = match expected {
            TypeKind::Tuple(field_types) => field_types.len(),
            TypeKind::VariantPayload(payload) => payload
                .shape()
                .tuple_fields()
                .map_or(usize::MAX, <[_]>::len),
            _ => usize::MAX,
        };
        if elements.len() != field_count {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: semantic_coordinate,
            });
        }
        let mut fields = Vec::new();
        for (ordinal, child) in elements.iter().enumerate() {
            self.poll()?;
            let ordinal =
                u32::try_from(ordinal).map_err(|_| CheckedMatchBuildError::InvalidCheckedRow {
                    coordinate: site.semantic_coordinate(),
                })?;
            let ty = match expected {
                TypeKind::Tuple(field_types) => &field_types[ordinal as usize],
                TypeKind::VariantPayload(payload) => payload
                    .shape()
                    .tuple_fields()
                    .and_then(|field_types| field_types.get(ordinal as usize))
                    .map(crate::types::VariantPayloadTupleField::ty)
                    .ok_or_else(|| CheckedMatchBuildError::InvalidCheckedRow {
                        coordinate: site.semantic_coordinate(),
                    })?,
                _ => unreachable!("tuple family validated above"),
            };
            let child_site = site.child(
                StablePatternCoordinateStep::TupleElement(ordinal),
                child_depth,
            );
            fields.push(self.deconstruct_site(*child, ty, &child_site)?);
        }
        Ok(DeconstructedPatternKind::Constructor {
            constructor: CoverageConstructorId::Tuple {
                owner: expected.semantic_identity_digest(),
            },
            fields: fields.into_boxed_slice(),
        })
    }

    fn deconstruct_variant(
        &mut self,
        variant: &arcweft_lang_hir::pattern::HirVariantPattern,
        resolution: &CheckedPatternResolution,
        expected: &TypeKind,
        site: &PatternSite,
        child_depth: u64,
    ) -> Result<DeconstructedPatternKind, CheckedMatchBuildError> {
        let CheckedPatternResolution::Variant(resolution) = resolution else {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: site.semantic_coordinate(),
            });
        };
        if resolution.owner().semantic_type() != expected.semantic_identity_digest() {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: site.semantic_coordinate(),
            });
        }
        let constructor = CoverageConstructorId::Variant {
            owner: resolution.owner().semantic_type(),
            case: resolution.selected().semantic_id(),
            ordinal: resolution.ordinal(),
        };
        match (variant.payload(), resolution.selected().payload()) {
            (HirVariantPatternPayload::Absent, VariantPayloadShape::Unit) => {
                Ok(DeconstructedPatternKind::Constructor {
                    constructor,
                    fields: Box::new([]),
                })
            }
            (
                HirVariantPatternPayload::Pattern(child),
                payload_shape @ (VariantPayloadShape::Tuple(_) | VariantPayloadShape::Record(_)),
            ) => {
                let child_site =
                    site.child(StablePatternCoordinateStep::VariantPayload, child_depth);
                let child_ty = self
                    .analysis
                    .pattern(*child)
                    .ok_or_else(|| CheckedMatchBuildError::MissingExactOwner {
                        coordinate: child_site.semantic_coordinate(),
                    })?
                    .ty()
                    .clone();
                let TypeKind::VariantPayload(payload_type) = &child_ty else {
                    return Err(CheckedMatchBuildError::InvalidCheckedRow {
                        coordinate: site.semantic_coordinate(),
                    });
                };
                if payload_type.owner_family() != resolution.owner().payload_owner_family()
                    || payload_type.owner_type() != resolution.owner().semantic_type()
                    || payload_type.case_ordinal() != resolution.ordinal()
                    || payload_type.case() != resolution.selected().semantic_id()
                    || payload_type.shape() != payload_shape
                {
                    return Err(CheckedMatchBuildError::InvalidCheckedRow {
                        coordinate: site.semantic_coordinate(),
                    });
                }
                let child = self.deconstruct_site(*child, &child_ty, &child_site)?;
                self.lift_variant_payload_pattern(
                    constructor,
                    payload_shape,
                    child_ty.semantic_identity_digest(),
                    child,
                )
            }
            (HirVariantPatternPayload::Recovered { .. }, _) => {
                Err(CheckedMatchBuildError::PoisonedSemanticNode {
                    coordinate: site.semantic_coordinate(),
                })
            }
            (HirVariantPatternPayload::Absent, _)
            | (HirVariantPatternPayload::Pattern(_), VariantPayloadShape::Unit) => {
                Err(CheckedMatchBuildError::InvalidCheckedRow {
                    coordinate: site.semantic_coordinate(),
                })
            }
        }
    }

    fn deconstruct_record(
        &mut self,
        resolution: &CheckedPatternResolution,
        expected: &TypeKind,
        site: &PatternSite,
        child_depth: u64,
    ) -> Result<DeconstructedPatternKind, CheckedMatchBuildError> {
        let CheckedPatternResolution::Record(record) = resolution else {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: site.semantic_coordinate(),
            });
        };
        let domain = self.domain(expected, &site.semantic_coordinate())?;
        let CoverageTypeDomain::Constructors(constructors) = domain else {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: site.semantic_coordinate(),
            });
        };
        let constructor = constructors
            .iter()
            .find(|constructor| {
                constructor.identity
                    == (CoverageConstructorId::Record {
                        owner: expected.semantic_identity_digest(),
                    })
            })
            .ok_or_else(|| CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: site.semantic_coordinate(),
            })?;
        let mut fields = constructor
            .field_types
            .iter()
            .map(|_| {
                DeconstructedPattern::wildcard(site.coordinate.clone(), site.semantic_coordinate())
            })
            .collect::<Vec<_>>();
        for field in record.fields() {
            let index = usize::try_from(field.declaration_ordinal()).map_err(|_| {
                CheckedMatchBuildError::InvalidCheckedRow {
                    coordinate: site.semantic_coordinate(),
                }
            })?;
            let slot =
                fields
                    .get_mut(index)
                    .ok_or_else(|| CheckedMatchBuildError::InvalidCheckedRow {
                        coordinate: site.semantic_coordinate(),
                    })?;
            let relative = append_record_coordinate(&site.coordinate, field);
            let field_site = site.at(relative, child_depth);
            *slot = match field.source().value() {
                CheckedRecordPatternSourceRef::Pattern(child) => {
                    self.deconstruct_site(child, field.field_type(), &field_site)?
                }
                CheckedRecordPatternSourceRef::Binding(_) => DeconstructedPattern::wildcard(
                    field_site.coordinate.clone(),
                    field_site.semantic_coordinate(),
                ),
            };
        }
        Ok(DeconstructedPatternKind::Constructor {
            constructor: CoverageConstructorId::Record {
                owner: expected.semantic_identity_digest(),
            },
            fields: fields.into_boxed_slice(),
        })
    }

    fn deconstruct_or(
        &mut self,
        alternatives: &[PatternId],
        expected: &TypeKind,
        site: &PatternSite,
        child_depth: u64,
    ) -> Result<DeconstructedPatternKind, CheckedMatchBuildError> {
        let alternatives = alternatives
            .iter()
            .enumerate()
            .map(|(ordinal, alternative)| {
                self.budget
                    .charge(CheckedMatchLimitKind::OrAlternatives, 1)?;
                let ordinal = u32::try_from(ordinal).map_err(|_| {
                    CheckedMatchBuildError::InvalidCheckedRow {
                        coordinate: site.semantic_coordinate(),
                    }
                })?;
                self.deconstruct_site(
                    *alternative,
                    expected,
                    &site.child(
                        StablePatternCoordinateStep::OrAlternative(ordinal),
                        child_depth,
                    ),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DeconstructedPatternKind::Or(
            alternatives.into_boxed_slice(),
        ))
    }

    fn lift_variant_payload_pattern(
        &mut self,
        constructor: CoverageConstructorId,
        payload_shape: &VariantPayloadShape,
        payload_owner: SemanticTypeDigest,
        child: DeconstructedPattern,
    ) -> Result<DeconstructedPatternKind, CheckedMatchBuildError> {
        self.poll()?;
        let payload_constructor = match payload_shape {
            VariantPayloadShape::Tuple(_) => CoverageConstructorId::Tuple {
                owner: payload_owner,
            },
            VariantPayloadShape::Record(_) => CoverageConstructorId::Record {
                owner: payload_owner,
            },
            VariantPayloadShape::Unit => {
                return Err(CheckedMatchBuildError::InvalidCheckedRow {
                    coordinate: child.semantic_coordinate,
                });
            }
        };
        match child.kind {
            DeconstructedPatternKind::Wildcard => Ok(DeconstructedPatternKind::Constructor {
                constructor,
                fields: (0..payload_shape.field_count())
                    .map(|_| {
                        DeconstructedPattern::wildcard(
                            child.coordinate.clone(),
                            child.semantic_coordinate.clone(),
                        )
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            }),
            DeconstructedPatternKind::Constructor {
                constructor: observed,
                fields,
            } if observed == payload_constructor && fields.len() == payload_shape.field_count() => {
                Ok(DeconstructedPatternKind::Constructor {
                    constructor,
                    fields,
                })
            }
            DeconstructedPatternKind::Or(alternatives) => {
                let mut lifted = Vec::new();
                for alternative in alternatives.into_vec() {
                    self.poll()?;
                    let coordinate = alternative.coordinate.clone();
                    let semantic_coordinate = alternative.semantic_coordinate.clone();
                    let kind = self.lift_variant_payload_pattern(
                        constructor.clone(),
                        payload_shape,
                        payload_owner,
                        alternative,
                    )?;
                    lifted.push(DeconstructedPattern {
                        kind,
                        coordinate,
                        semantic_coordinate,
                    });
                }
                Ok(DeconstructedPatternKind::Or(lifted.into_boxed_slice()))
            }
            DeconstructedPatternKind::Constructor { .. } => {
                Err(CheckedMatchBuildError::InvalidCheckedRow {
                    coordinate: child.semantic_coordinate,
                })
            }
        }
    }

    fn deconstruct_sequence(
        &mut self,
        elements: &[PatternId],
        rest: &HirPatternSequenceRest,
        expected: &TypeKind,
        site: &PatternSite,
        child_depth: u64,
    ) -> Result<DeconstructedPattern, CheckedMatchBuildError> {
        self.poll()?;
        self.budget.observe_depth(site.depth)?;
        let semantic_coordinate = site.semantic_coordinate();
        let mut form = SequencePatternForm::from_expected(expected, semantic_coordinate.clone())?;
        let mut authored = Vec::new();
        for (ordinal, child) in elements.iter().enumerate() {
            self.poll()?;
            let ordinal =
                u32::try_from(ordinal).map_err(|_| CheckedMatchBuildError::InvalidCheckedRow {
                    coordinate: semantic_coordinate.clone(),
                })?;
            let child_site = site.child(
                StablePatternCoordinateStep::SequenceElement(ordinal),
                child_depth,
            );
            authored.push(self.deconstruct_site(*child, form.item, &child_site)?);
        }
        let domain = self.domain(expected, &semantic_coordinate)?;
        let CoverageTypeDomain::Constructors(constructors) = domain else {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: semantic_coordinate.clone(),
            });
        };
        form.minimum = checked_len(elements.len(), CheckedMatchLimitKind::PatternNodes)?;
        form.has_rest = match rest {
            HirPatternSequenceRest::Absent => false,
            HirPatternSequenceRest::Unbound | HirPatternSequenceRest::Bound(_) => true,
            HirPatternSequenceRest::Recovered(_) => {
                return Err(CheckedMatchBuildError::PoisonedSemanticNode {
                    coordinate: semantic_coordinate.clone(),
                });
            }
        };
        self.expand_sequence_constructors(&form, &constructors, &authored, site)
    }

    fn expand_sequence_constructors(
        &mut self,
        form: &SequencePatternForm<'_>,
        constructors: &[CoverageConstructor],
        authored: &[DeconstructedPattern],
        site: &PatternSite,
    ) -> Result<DeconstructedPattern, CheckedMatchBuildError> {
        let mut accepted = Vec::new();
        for constructor in constructors {
            self.poll()?;
            let accepts = match &constructor.identity {
                CoverageConstructorId::Array { length, .. } if form.is_array => {
                    if form.has_rest {
                        *length >= form.minimum
                    } else {
                        *length == form.minimum
                    }
                }
                _ if form.is_array => false,
                CoverageConstructorId::Sequence { partition, .. } => {
                    if form.has_rest {
                        partition.lower() >= form.minimum
                    } else {
                        matches!(partition, SequencePartition::Exact(length) if *length == form.minimum)
                    }
                }
                _ => false,
            };
            if !accepts {
                continue;
            }
            if authored.len() > constructor.field_types.len() {
                return Err(CheckedMatchBuildError::InvalidCheckedRow {
                    coordinate: site.semantic_coordinate(),
                });
            }
            let mut fields = authored.to_vec();
            fields.extend((fields.len()..constructor.field_types.len()).map(|_| {
                DeconstructedPattern::wildcard(site.coordinate.clone(), site.semantic_coordinate())
            }));
            accepted.push(DeconstructedPattern {
                coordinate: site.coordinate.clone(),
                semantic_coordinate: site.semantic_coordinate(),
                kind: DeconstructedPatternKind::Constructor {
                    constructor: constructor.identity.clone(),
                    fields: fields.into_boxed_slice(),
                },
            });
        }
        let semantic_coordinate = site.semantic_coordinate();
        match accepted.len() {
            0 => Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: semantic_coordinate,
            }),
            1 => Ok(accepted.remove(0)),
            _ => Ok(DeconstructedPattern {
                coordinate: site.coordinate.clone(),
                semantic_coordinate,
                kind: DeconstructedPatternKind::Or(accepted.into_boxed_slice()),
            }),
        }
    }

    pub(super) fn expand_or(
        &mut self,
        pattern: DeconstructedPattern,
        depth: u64,
    ) -> Result<Vec<ExpandedPattern>, CheckedMatchBuildError> {
        self.poll()?;
        self.budget.observe_depth(depth)?;
        let child_depth = checked_depth_successor(depth)?;
        match pattern.kind {
            DeconstructedPatternKind::Wildcard => Ok(vec![ExpandedPattern {
                pattern,
                alternative: None,
            }]),
            DeconstructedPatternKind::Or(alternatives) => {
                let mut expanded = Vec::new();
                for alternative in alternatives.into_vec() {
                    self.poll()?;
                    let coordinate = alternative.coordinate.clone();
                    for mut row in self.expand_or(alternative, child_depth)? {
                        if row.alternative.is_none() {
                            row.alternative = Some(coordinate.clone());
                        }
                        expanded.push(row);
                    }
                }
                Ok(expanded)
            }
            DeconstructedPatternKind::Constructor {
                constructor,
                fields,
            } => {
                let mut products: Vec<(
                    Vec<DeconstructedPattern>,
                    Option<StablePatternCoordinate>,
                )> = vec![(Vec::new(), None)];
                for field in fields.into_vec() {
                    self.poll()?;
                    let alternatives = self.expand_or(field, child_depth)?;
                    let product_count =
                        checked_len(products.len(), CheckedMatchLimitKind::OrAlternatives)?
                            .checked_mul(checked_len(
                                alternatives.len(),
                                CheckedMatchLimitKind::OrAlternatives,
                            )?)
                            .ok_or(CheckedMatchBuildError::ArithmeticOverflow {
                                kind: CheckedMatchLimitKind::OrAlternatives,
                            })?;
                    self.budget
                        .charge(CheckedMatchLimitKind::OrAlternatives, product_count)?;
                    let mut next = Vec::new();
                    for (prefix, prior_coordinate) in products {
                        for alternative in &alternatives {
                            self.poll()?;
                            let mut fields = prefix.clone();
                            fields.push(alternative.pattern.clone());
                            next.push((
                                fields,
                                alternative
                                    .alternative
                                    .clone()
                                    .or_else(|| prior_coordinate.clone()),
                            ));
                        }
                    }
                    products = next;
                }
                Ok(products
                    .into_iter()
                    .map(|(fields, alternative)| ExpandedPattern {
                        pattern: DeconstructedPattern {
                            coordinate: pattern.coordinate.clone(),
                            semantic_coordinate: pattern.semantic_coordinate.clone(),
                            kind: DeconstructedPatternKind::Constructor {
                                constructor: constructor.clone(),
                                fields: fields.into_boxed_slice(),
                            },
                        },
                        alternative,
                    })
                    .collect())
            }
        }
    }
}
