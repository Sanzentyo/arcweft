//! Deterministic binding-position preflight for one attached Pattern tree.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::patterns::{
    PatternBindingSyntax, PatternNodePath, PatternNodeStep, PatternOrBindingIssue,
    PatternRecordFieldIssue, PatternRecordFieldSyntax, PatternSyntaxKind, PatternSyntaxNode,
    PatternVariantPayloadSyntax,
};

use crate::identity::HirLimit;
use crate::leaf::HirName;
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::pattern::{HirPatternBindingIssue, HirPatternFieldIssue};

use super::super::name_projection::{name, name_issue};
use super::super::require_limit;
use super::{PatternInput, attached_children};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum BindingSiteRole {
    Node,
    RecordShorthand(u32),
    RecordRest(u32),
    SequenceRest,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct BindingSite {
    path: PatternNodePath,
    role: BindingSiteRole,
}

impl BindingSite {
    pub(super) fn new(path: &PatternNodePath, role: BindingSiteRole) -> Self {
        Self {
            path: path.clone(),
            role,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum BindingAllocation {
    Ordinary {
        ordinal: u32,
        poisoned: bool,
    },
    Rest {
        canonical: BindingSite,
        poisoned: bool,
    },
}

#[derive(Clone, Debug)]
pub(super) struct BindingPlan {
    allocations: BTreeMap<BindingSite, BindingAllocation>,
    ordered: Box<[BindingAllocation]>,
}

impl BindingPlan {
    pub(super) fn build(
        root: &PatternInput<'_>,
        reverse_child_insertion: bool,
    ) -> Result<Self, HirLowerFailure> {
        let layout = layout(root, reverse_child_insertion)?;
        let ordinary_count = layout
            .positions
            .iter()
            .filter(|position| !position.shape.rest && position.shape.binding.name().is_some())
            .count();
        require_limit(HirLimit::SyntheticDescendantsPerOwner, ordinary_count)?;

        let mut next_ordinary = 0_u32;
        let mut names = BTreeSet::new();
        let allocations = layout
            .positions
            .iter()
            .map(|position| {
                let Some(source_name) = position.shape.binding.name() else {
                    return Ok(None);
                };
                let poisoned = !names.insert(name(source_name)?);
                let allocation = if position.shape.rest {
                    BindingAllocation::Rest {
                        canonical: position.canonical.clone(),
                        poisoned,
                    }
                } else {
                    let ordinal = next_ordinary;
                    next_ordinary = next_ordinary
                        .checked_add(1)
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                    BindingAllocation::Ordinary { ordinal, poisoned }
                };
                Ok(Some(allocation))
            })
            .collect::<Result<Vec<_>, HirLowerFailure>>()?;
        let mut by_site = BTreeMap::new();
        for (site, position) in layout.occurrences {
            let Some(allocation) = allocations.get(position).cloned().flatten() else {
                continue;
            };
            if by_site.insert(site, allocation).is_some() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
        }
        Ok(Self {
            allocations: by_site,
            ordered: allocations.into_iter().flatten().collect(),
        })
    }

    pub(super) fn allocation(
        &self,
        site: BindingSite,
    ) -> Result<BindingAllocation, HirLowerFailure> {
        self.allocations
            .get(&site)
            .cloned()
            .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
    }

    pub(super) fn ordered_allocations(&self) -> &[BindingAllocation] {
        &self.ordered
    }
}

/// Re-derives syntactic irrefutability from the parser-owned semantic Pattern.
///
/// This intentionally does not consult the projected HIR graph. Source-index
/// freezing owns a separate derivation so a corrupt semantic projection cannot
/// make the two authorities agree accidentally.
pub(super) fn pattern_is_irrefutable(root: &PatternInput<'_>) -> bool {
    syntax_pattern_is_irrefutable(root.value())
}

fn syntax_pattern_is_irrefutable(pattern: &PatternSyntaxNode) -> bool {
    match pattern.kind() {
        PatternSyntaxKind::Binding(_)
        | PatternSyntaxKind::MutableBinding(_)
        | PatternSyntaxKind::Discard
        | PatternSyntaxKind::TypedBinding(_) => true,
        PatternSyntaxKind::Tuple(elements) => elements.iter().all(syntax_pattern_is_irrefutable),
        PatternSyntaxKind::Record(record) => record.fields().iter().all(|field| match field {
            PatternRecordFieldSyntax::Explicit { pattern, .. } => {
                syntax_pattern_is_irrefutable(pattern)
            }
            PatternRecordFieldSyntax::Shorthand(_) | PatternRecordFieldSyntax::Rest(_) => true,
            PatternRecordFieldSyntax::Invalid(_) => false,
        }),
        PatternSyntaxKind::WholeBinding { pattern, .. } => syntax_pattern_is_irrefutable(pattern),
        PatternSyntaxKind::Literal(_)
        | PatternSyntaxKind::EntityReference(_)
        | PatternSyntaxKind::Variant(_)
        | PatternSyntaxKind::BracketSequence(_)
        | PatternSyntaxKind::Or(_)
        | PatternSyntaxKind::Error => false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BindingShape {
    binding: PatternBindingSyntax,
    mutable: bool,
    rest: bool,
}

#[derive(Clone, Debug)]
struct LayoutPosition {
    shape: BindingShape,
    canonical: BindingSite,
}

#[derive(Clone, Debug, Default)]
struct BindingLayout {
    positions: Vec<LayoutPosition>,
    occurrences: Vec<(BindingSite, usize)>,
}

impl BindingLayout {
    fn one(site: BindingSite, binding: &PatternBindingSyntax, mutable: bool, rest: bool) -> Self {
        Self {
            positions: vec![LayoutPosition {
                shape: BindingShape {
                    binding: binding.clone(),
                    mutable,
                    rest,
                },
                canonical: site.clone(),
            }],
            occurrences: vec![(site, 0)],
        }
    }

    fn append(&mut self, mut other: Self) {
        let offset = self.positions.len();
        self.positions.append(&mut other.positions);
        self.occurrences.extend(
            other
                .occurrences
                .into_iter()
                .map(|(site, position)| (site, position + offset)),
        );
    }
}

fn layout(
    attached: &PatternInput<'_>,
    reverse_child_insertion: bool,
) -> Result<BindingLayout, HirLowerFailure> {
    let children = pattern_children(attached, reverse_child_insertion)?;
    let result = match attached.value().kind() {
        PatternSyntaxKind::Binding(binding) | PatternSyntaxKind::TypedBinding(binding) => {
            BindingLayout::one(
                BindingSite::new(attached.path(), BindingSiteRole::Node),
                binding,
                false,
                false,
            )
        }
        PatternSyntaxKind::MutableBinding(binding) => BindingLayout::one(
            BindingSite::new(attached.path(), BindingSiteRole::Node),
            binding,
            true,
            false,
        ),
        PatternSyntaxKind::Variant(variant) => match variant.payload() {
            PatternVariantPayloadSyntax::Resolved(_)
            | PatternVariantPayloadSyntax::Recovered { value: Some(_), .. } => layout(
                required_child(&children, PatternNodeStep::VariantPayload)?,
                reverse_child_insertion,
            )?,
            PatternVariantPayloadSyntax::Recovered { value: None, .. }
            | PatternVariantPayloadSyntax::Absent => BindingLayout::default(),
        },
        PatternSyntaxKind::Tuple(elements) => {
            indexed_layout(&children, elements.len(), reverse_child_insertion)?
        }
        PatternSyntaxKind::BracketSequence(sequence) => {
            let mut result = indexed_layout(
                &children,
                sequence.elements().len(),
                reverse_child_insertion,
            )?;
            if let Some(binding) = sequence.rest().binding() {
                result.append(BindingLayout::one(
                    BindingSite::new(attached.path(), BindingSiteRole::SequenceRest),
                    binding,
                    false,
                    true,
                ));
            }
            result
        }
        PatternSyntaxKind::Record(record) => record_layout(
            attached,
            record.fields(),
            &children,
            reverse_child_insertion,
        )?,
        PatternSyntaxKind::WholeBinding { binding, .. } => {
            let mut result = BindingLayout::one(
                BindingSite::new(attached.path(), BindingSiteRole::Node),
                binding,
                false,
                false,
            );
            result.append(layout(
                required_child(&children, PatternNodeStep::NestedPattern)?,
                reverse_child_insertion,
            )?);
            result
        }
        PatternSyntaxKind::Or(alternatives) => or_layout(
            attached,
            &children,
            alternatives.len(),
            reverse_child_insertion,
        )?,
        PatternSyntaxKind::Literal(_)
        | PatternSyntaxKind::EntityReference(_)
        | PatternSyntaxKind::Discard
        | PatternSyntaxKind::Error => BindingLayout::default(),
    };
    Ok(result)
}

fn indexed_layout(
    children: &BTreeMap<PatternNodeStep, PatternInput<'_>>,
    count: usize,
    reverse_child_insertion: bool,
) -> Result<BindingLayout, HirLowerFailure> {
    let mut result = BindingLayout::default();
    for index in 0..count {
        let ordinal = u32::try_from(index).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        result.append(layout(
            required_child(children, PatternNodeStep::Element(ordinal))?,
            reverse_child_insertion,
        )?);
    }
    Ok(result)
}

fn or_layout(
    attached: &PatternInput<'_>,
    children: &BTreeMap<PatternNodeStep, PatternInput<'_>>,
    count: usize,
    reverse_child_insertion: bool,
) -> Result<BindingLayout, HirLowerFailure> {
    if count < 2 {
        return Err(HirInvariantFailure::InvalidArenaCommit.into());
    }
    let first = required_child(children, PatternNodeStep::Element(0))?;
    let mut canonical = layout(first, reverse_child_insertion)?;
    for index in 1..count {
        let ordinal = u32::try_from(index).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let alternative = layout(
            required_child(children, PatternNodeStep::Element(ordinal))?,
            reverse_child_insertion,
        )?;
        if alternative.positions.len() != canonical.positions.len() {
            return Err(HirLowerFailure::OrAlternativeBindingsMismatch {
                owner: attached.source_owner_id(),
                issue: PatternOrBindingIssue::CountMismatch {
                    alternative: ordinal,
                    expected: u32::try_from(canonical.positions.len())
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                    actual: u32::try_from(alternative.positions.len())
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                },
            });
        }
        if let Some(position) = alternative
            .positions
            .iter()
            .zip(&canonical.positions)
            .position(|(actual, expected)| actual.shape != expected.shape)
        {
            return Err(HirLowerFailure::OrAlternativeBindingsMismatch {
                owner: attached.source_owner_id(),
                issue: PatternOrBindingIssue::PositionMismatch {
                    alternative: ordinal,
                    ordinal: u32::try_from(position)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                },
            });
        }
        canonical.occurrences.extend(alternative.occurrences);
    }
    Ok(canonical)
}

fn record_layout(
    attached: &PatternInput<'_>,
    fields: &[PatternRecordFieldSyntax],
    children: &BTreeMap<PatternNodeStep, PatternInput<'_>>,
    reverse_child_insertion: bool,
) -> Result<BindingLayout, HirLowerFailure> {
    let dispositions = classify_record_fields(fields)?;
    let mut result = BindingLayout::default();
    for (index, (field, disposition)) in fields.iter().zip(dispositions).enumerate() {
        let ordinal = u32::try_from(index).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        match (field, disposition) {
            (
                PatternRecordFieldSyntax::Explicit { .. },
                RecordFieldDisposition::Explicit { .. },
            ) => {
                result.append(layout(
                    required_child(children, PatternNodeStep::RecordField(ordinal))?,
                    reverse_child_insertion,
                )?);
            }
            (
                PatternRecordFieldSyntax::Shorthand(binding),
                RecordFieldDisposition::Shorthand { .. },
            ) => {
                result.append(BindingLayout::one(
                    BindingSite::new(attached.path(), BindingSiteRole::RecordShorthand(ordinal)),
                    binding,
                    false,
                    false,
                ));
            }
            (PatternRecordFieldSyntax::Rest(Some(binding)), RecordFieldDisposition::Rest) => {
                result.append(BindingLayout::one(
                    BindingSite::new(attached.path(), BindingSiteRole::RecordRest(ordinal)),
                    binding,
                    false,
                    true,
                ));
            }
            (PatternRecordFieldSyntax::Shorthand(binding), RecordFieldDisposition::Invalid(_)) => {
                // A malformed shorthand still occupies an Or binding position,
                // but it never admits a Local.
                if matches!(binding, PatternBindingSyntax::Recovered(_)) {
                    result.append(BindingLayout::one(
                        BindingSite::new(
                            attached.path(),
                            BindingSiteRole::RecordShorthand(ordinal),
                        ),
                        binding,
                        false,
                        false,
                    ));
                }
            }
            (
                PatternRecordFieldSyntax::Rest(Some(binding)),
                RecordFieldDisposition::Invalid(issue),
            ) if issue != HirPatternFieldIssue::MultipleRest => {
                result.append(BindingLayout::one(
                    BindingSite::new(attached.path(), BindingSiteRole::RecordRest(ordinal)),
                    binding,
                    false,
                    true,
                ));
            }
            _ => {}
        }
    }
    Ok(result)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RecordFieldDisposition {
    Explicit { name: HirName },
    Shorthand { name: HirName },
    Rest,
    Invalid(HirPatternFieldIssue),
}

pub(crate) fn classify_record_fields(
    fields: &[PatternRecordFieldSyntax],
) -> Result<Vec<RecordFieldDisposition>, HirLowerFailure> {
    let mut names = BTreeSet::new();
    let mut found_rest = false;
    fields
        .iter()
        .map(|field| match field {
            PatternRecordFieldSyntax::Explicit { name: source, .. } => {
                classify_named_field(source, &mut names, |name| {
                    RecordFieldDisposition::Explicit { name }
                })
            }
            PatternRecordFieldSyntax::Shorthand(binding) => match binding {
                PatternBindingSyntax::Resolved(source) => {
                    let name = name(source)?;
                    if names.insert(name.clone()) {
                        Ok(RecordFieldDisposition::Shorthand { name })
                    } else {
                        Ok(RecordFieldDisposition::Invalid(
                            HirPatternFieldIssue::DuplicateName,
                        ))
                    }
                }
                PatternBindingSyntax::Recovered(issue) => Ok(RecordFieldDisposition::Invalid(
                    HirPatternFieldIssue::InvalidBinding(binding_issue(issue)),
                )),
            },
            PatternRecordFieldSyntax::Rest(binding) => {
                if found_rest {
                    return Ok(RecordFieldDisposition::Invalid(
                        HirPatternFieldIssue::MultipleRest,
                    ));
                }
                found_rest = true;
                match binding {
                    Some(PatternBindingSyntax::Recovered(issue)) => {
                        Ok(RecordFieldDisposition::Invalid(
                            HirPatternFieldIssue::InvalidRestBinding(binding_issue(issue)),
                        ))
                    }
                    Some(PatternBindingSyntax::Resolved(_)) | None => {
                        Ok(RecordFieldDisposition::Rest)
                    }
                }
            }
            PatternRecordFieldSyntax::Invalid(invalid) => Ok(RecordFieldDisposition::Invalid(
                field_issue(invalid.issue()),
            )),
        })
        .collect()
}

fn classify_named_field(
    source: &arcweft_lang_syntax::patterns::PatternNameSyntax,
    names: &mut BTreeSet<HirName>,
    valid: impl FnOnce(HirName) -> RecordFieldDisposition,
) -> Result<RecordFieldDisposition, HirLowerFailure> {
    match source {
        arcweft_lang_syntax::patterns::PatternNameSyntax::Resolved(source) => {
            let name = name(source)?;
            if names.insert(name.clone()) {
                Ok(valid(name))
            } else {
                Ok(RecordFieldDisposition::Invalid(
                    HirPatternFieldIssue::DuplicateName,
                ))
            }
        }
        arcweft_lang_syntax::patterns::PatternNameSyntax::Recovered(issue) => Ok(
            RecordFieldDisposition::Invalid(HirPatternFieldIssue::InvalidName(name_issue(issue))),
        ),
        arcweft_lang_syntax::patterns::PatternNameSyntax::Absent => Ok(
            RecordFieldDisposition::Invalid(HirPatternFieldIssue::MissingName),
        ),
    }
}

pub(crate) const fn binding_issue(
    issue: &arcweft_lang_syntax::patterns::PatternBindingIssue,
) -> HirPatternBindingIssue {
    match issue {
        arcweft_lang_syntax::patterns::PatternBindingIssue::MissingName => {
            HirPatternBindingIssue::MissingName
        }
        arcweft_lang_syntax::patterns::PatternBindingIssue::InvalidName(issue) => {
            HirPatternBindingIssue::InvalidName(name_issue(issue))
        }
        arcweft_lang_syntax::patterns::PatternBindingIssue::ReservedBindingKeyword { .. } => {
            HirPatternBindingIssue::InvalidName(
                crate::leaf::HirNameInvariantError::InvalidIdentifier,
            )
        }
        arcweft_lang_syntax::patterns::PatternBindingIssue::UnexpectedTrailingInput {
            token_count,
        } => HirPatternBindingIssue::UnexpectedTrailingInput {
            token_count: *token_count,
        },
    }
}

fn field_issue(issue: &PatternRecordFieldIssue) -> HirPatternFieldIssue {
    match issue {
        PatternRecordFieldIssue::MissingName => HirPatternFieldIssue::MissingName,
        PatternRecordFieldIssue::InvalidName(issue) => {
            HirPatternFieldIssue::InvalidName(name_issue(issue))
        }
        PatternRecordFieldIssue::InvalidBinding(issue) => {
            HirPatternFieldIssue::InvalidBinding(binding_issue(issue))
        }
        PatternRecordFieldIssue::MissingPattern => HirPatternFieldIssue::MissingPattern,
        PatternRecordFieldIssue::InvalidRestBinding(issue) => {
            HirPatternFieldIssue::InvalidRestBinding(binding_issue(issue))
        }
    }
}

fn pattern_children<'candidate>(
    attached: &PatternInput<'candidate>,
    reverse_child_insertion: bool,
) -> Result<BTreeMap<PatternNodeStep, PatternInput<'candidate>>, HirLowerFailure> {
    Ok(attached_children(attached, reverse_child_insertion)?.patterns)
}

fn required_child<'map, 'candidate>(
    children: &'map BTreeMap<PatternNodeStep, PatternInput<'candidate>>,
    step: PatternNodeStep,
) -> Result<&'map PatternInput<'candidate>, HirLowerFailure> {
    children
        .get(&step)
        .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
}
