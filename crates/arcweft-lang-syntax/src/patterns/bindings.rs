//! Deterministic binding inventory and Or-pattern consistency validation.

use std::collections::BTreeSet;

use super::source::PatternSourceMapError;
use super::{
    PatternBindingSyntax, PatternNodePath, PatternNodeStep, PatternOrBindingIssue,
    PatternRecordFieldSyntax, PatternRecoveryIssue, PatternSyntaxKind, PatternSyntaxNode,
    PatternSyntaxState, PatternVariantPayloadSyntax,
};

/// Semantic kind of one binding in authored preorder.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternBindingSiteKind {
    Binding,
    MutableBinding,
    WholeBinding,
    TypedBinding,
    RecordShorthand { field: u32 },
    RecordRest { field: u32 },
    SequenceRest,
}

/// One typed binding site in deterministic authored preorder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternBindingSite {
    ordinal: u32,
    owner: PatternNodePath,
    kind: PatternBindingSiteKind,
    binding: PatternBindingSyntax,
}

impl PatternBindingSite {
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub const fn owner(&self) -> &PatternNodePath {
        &self.owner
    }

    pub const fn kind(&self) -> PatternBindingSiteKind {
        self.kind
    }

    pub const fn binding(&self) -> &PatternBindingSyntax {
        &self.binding
    }
}

pub(crate) fn mark_or_binding_mismatches(
    value: &mut PatternSyntaxNode,
) -> Result<(), PatternSourceMapError> {
    match &mut value.kind {
        PatternSyntaxKind::Variant(variant) => match &mut variant.payload {
            PatternVariantPayloadSyntax::Resolved(child)
            | PatternVariantPayloadSyntax::Recovered {
                value: Some(child), ..
            } => mark_or_binding_mismatches(child)?,
            PatternVariantPayloadSyntax::Recovered { value: None, .. }
            | PatternVariantPayloadSyntax::Absent => {}
        },
        PatternSyntaxKind::Tuple(items) | PatternSyntaxKind::Or(items) => {
            for item in items {
                mark_or_binding_mismatches(item)?;
            }
        }
        PatternSyntaxKind::Record(record) => {
            for field in &mut record.fields {
                if let PatternRecordFieldSyntax::Explicit { pattern, .. } = field {
                    mark_or_binding_mismatches(pattern)?;
                }
            }
        }
        PatternSyntaxKind::BracketSequence(sequence) => {
            for element in &mut sequence.elements {
                mark_or_binding_mismatches(element)?;
            }
        }
        PatternSyntaxKind::WholeBinding { pattern, .. } => {
            mark_or_binding_mismatches(pattern)?;
        }
        PatternSyntaxKind::Binding(_)
        | PatternSyntaxKind::MutableBinding(_)
        | PatternSyntaxKind::Literal(_)
        | PatternSyntaxKind::EntityReference(_)
        | PatternSyntaxKind::Discard
        | PatternSyntaxKind::TypedBinding(_)
        | PatternSyntaxKind::Error => {}
    }

    let issues = match &value.kind {
        PatternSyntaxKind::Or(alternatives) => or_binding_issues(alternatives)?,
        _ => Vec::new(),
    };
    if !issues.is_empty() {
        let additions = issues
            .into_iter()
            .map(PatternRecoveryIssue::OrBindings)
            .collect::<Vec<_>>();
        match &mut value.state {
            PatternSyntaxState::Valid => {
                value.state = PatternSyntaxState::Recovered(additions.into_boxed_slice());
            }
            PatternSyntaxState::Recovered(existing) => {
                let mut combined = existing.to_vec();
                combined.extend(additions);
                *existing = combined.into_boxed_slice();
            }
        }
    }
    Ok(())
}

fn or_binding_issues(
    alternatives: &[PatternSyntaxNode],
) -> Result<Vec<PatternOrBindingIssue>, PatternSourceMapError> {
    let Some((first, remaining)) = alternatives.split_first() else {
        return Ok(Vec::new());
    };
    let expected = canonical_binding_positions(first, PatternNodeStep::Element(0))?;
    let expected_count =
        u32::try_from(expected.len()).map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
    let mut issues = Vec::new();
    for (index, alternative) in remaining.iter().enumerate() {
        let alternative_ordinal =
            u32::try_from(index + 1).map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
        let actual = canonical_binding_positions(
            alternative,
            PatternNodeStep::Element(alternative_ordinal),
        )?;
        let actual_count = u32::try_from(actual.len())
            .map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
        if actual_count != expected_count {
            issues.push(PatternOrBindingIssue::CountMismatch {
                alternative: alternative_ordinal,
                expected: expected_count,
                actual: actual_count,
            });
        }
        for (position, (expected, actual)) in expected.iter().zip(&actual).enumerate() {
            if !same_binding_position(expected, actual) {
                issues.push(PatternOrBindingIssue::PositionMismatch {
                    alternative: alternative_ordinal,
                    ordinal: u32::try_from(position)
                        .map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?,
                });
            }
        }
    }
    Ok(issues)
}

fn canonical_binding_positions(
    value: &PatternSyntaxNode,
    step: PatternNodeStep,
) -> Result<Vec<PatternBindingSite>, PatternSourceMapError> {
    let mut sites = Vec::new();
    let mut next = 0_u32;
    collect_binding_sites(
        value,
        &PatternNodePath::root().child(step),
        &mut sites,
        &mut next,
    )?;
    let mut seen = BTreeSet::new();
    sites.retain(|site| seen.insert(site.ordinal));
    Ok(sites)
}

fn same_binding_position(left: &PatternBindingSite, right: &PatternBindingSite) -> bool {
    left.binding == right.binding
        && matches!(left.kind, PatternBindingSiteKind::MutableBinding)
            == matches!(right.kind, PatternBindingSiteKind::MutableBinding)
}

pub(crate) fn collect_binding_sites(
    value: &PatternSyntaxNode,
    path: &PatternNodePath,
    output: &mut Vec<PatternBindingSite>,
    next_ordinal: &mut u32,
) -> Result<(), PatternSourceMapError> {
    match value.kind() {
        PatternSyntaxKind::Binding(binding) => {
            push_binding(
                output,
                next_ordinal,
                path,
                PatternBindingSiteKind::Binding,
                binding,
            )?;
        }
        PatternSyntaxKind::MutableBinding(binding) => {
            push_binding(
                output,
                next_ordinal,
                path,
                PatternBindingSiteKind::MutableBinding,
                binding,
            )?;
        }
        PatternSyntaxKind::Variant(variant) => match variant.payload() {
            PatternVariantPayloadSyntax::Resolved(child)
            | PatternVariantPayloadSyntax::Recovered {
                value: Some(child), ..
            } => collect_binding_sites(
                child,
                &path.child(PatternNodeStep::VariantPayload),
                output,
                next_ordinal,
            )?,
            PatternVariantPayloadSyntax::Recovered { value: None, .. }
            | PatternVariantPayloadSyntax::Absent => {}
        },
        PatternSyntaxKind::Tuple(items) => {
            collect_element_bindings(items, path, output, next_ordinal)?;
        }
        PatternSyntaxKind::Or(items) => {
            collect_or_bindings(items, path, output, next_ordinal)?;
        }
        PatternSyntaxKind::Record(record) => {
            collect_record_bindings(record.fields(), path, output, next_ordinal)?;
        }
        PatternSyntaxKind::BracketSequence(sequence) => {
            collect_element_bindings(sequence.elements(), path, output, next_ordinal)?;
            if let Some(binding) = sequence.rest().binding() {
                push_binding(
                    output,
                    next_ordinal,
                    path,
                    PatternBindingSiteKind::SequenceRest,
                    binding,
                )?;
            }
        }
        PatternSyntaxKind::WholeBinding { binding, pattern } => {
            push_binding(
                output,
                next_ordinal,
                path,
                PatternBindingSiteKind::WholeBinding,
                binding,
            )?;
            collect_binding_sites(
                pattern,
                &path.child(PatternNodeStep::NestedPattern),
                output,
                next_ordinal,
            )?;
        }
        PatternSyntaxKind::TypedBinding(binding) => {
            push_binding(
                output,
                next_ordinal,
                path,
                PatternBindingSiteKind::TypedBinding,
                binding,
            )?;
        }
        PatternSyntaxKind::Literal(_)
        | PatternSyntaxKind::EntityReference(_)
        | PatternSyntaxKind::Discard
        | PatternSyntaxKind::Error => {}
    }
    Ok(())
}

fn collect_or_bindings(
    items: &[PatternSyntaxNode],
    path: &PatternNodePath,
    output: &mut Vec<PatternBindingSite>,
    next_ordinal: &mut u32,
) -> Result<(), PatternSourceMapError> {
    let Some((first, remaining)) = items.split_first() else {
        return Ok(());
    };
    let first_start = *next_ordinal;
    collect_binding_sites(
        first,
        &path.child(PatternNodeStep::Element(0)),
        output,
        next_ordinal,
    )?;
    let first_end = *next_ordinal;
    for (index, alternative) in remaining.iter().enumerate() {
        let ordinal =
            u32::try_from(index + 1).map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
        let mut reused = first_start;
        let mut alternative_sites = Vec::new();
        collect_binding_sites(
            alternative,
            &path.child(PatternNodeStep::Element(ordinal)),
            &mut alternative_sites,
            &mut reused,
        )?;
        // A later alternative never advances the containing binding cursor.
        // Its positions are the first alternative's map.
        output.extend(
            alternative_sites
                .into_iter()
                .filter(|site| site.ordinal < first_end),
        );
    }
    Ok(())
}

fn collect_record_bindings(
    fields: &[PatternRecordFieldSyntax],
    path: &PatternNodePath,
    output: &mut Vec<PatternBindingSite>,
    next_ordinal: &mut u32,
) -> Result<(), PatternSourceMapError> {
    for (index, field) in fields.iter().enumerate() {
        let ordinal =
            u32::try_from(index).map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
        match field {
            PatternRecordFieldSyntax::Explicit { pattern, .. } => collect_binding_sites(
                pattern,
                &path.child(PatternNodeStep::RecordField(ordinal)),
                output,
                next_ordinal,
            )?,
            PatternRecordFieldSyntax::Shorthand(binding) => push_binding(
                output,
                next_ordinal,
                path,
                PatternBindingSiteKind::RecordShorthand { field: ordinal },
                binding,
            )?,
            PatternRecordFieldSyntax::Rest(Some(binding)) => push_binding(
                output,
                next_ordinal,
                path,
                PatternBindingSiteKind::RecordRest { field: ordinal },
                binding,
            )?,
            PatternRecordFieldSyntax::Rest(None) | PatternRecordFieldSyntax::Invalid(_) => {}
        }
    }
    Ok(())
}

fn collect_element_bindings(
    items: &[PatternSyntaxNode],
    path: &PatternNodePath,
    output: &mut Vec<PatternBindingSite>,
    next_ordinal: &mut u32,
) -> Result<(), PatternSourceMapError> {
    for (index, item) in items.iter().enumerate() {
        let ordinal =
            u32::try_from(index).map_err(|_| PatternSourceMapError::BindingOrdinalOverflow)?;
        collect_binding_sites(
            item,
            &path.child(PatternNodeStep::Element(ordinal)),
            output,
            next_ordinal,
        )?;
    }
    Ok(())
}

fn push_binding(
    output: &mut Vec<PatternBindingSite>,
    next_ordinal: &mut u32,
    owner: &PatternNodePath,
    kind: PatternBindingSiteKind,
    binding: &PatternBindingSyntax,
) -> Result<(), PatternSourceMapError> {
    let ordinal = *next_ordinal;
    *next_ordinal = next_ordinal
        .checked_add(1)
        .ok_or(PatternSourceMapError::BindingOrdinalOverflow)?;
    output.push(PatternBindingSite {
        ordinal,
        owner: owner.clone(),
        kind,
        binding: binding.clone(),
    });
    Ok(())
}
