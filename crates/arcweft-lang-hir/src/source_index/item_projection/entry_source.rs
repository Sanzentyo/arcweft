//! Typed source projection for final Entry declaration owners.

use arcweft_lang_syntax::attachment::{
    AttachedEntryId, AttachedEntryMember, AttachedEntryValue, TypedItemNode,
};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_source::SourceSpan;

use crate::identity::{ItemId, SyntheticOwner};
use crate::item::HirItemKind;
use crate::source_index::{
    HirEntrySourcePart, HirItemSourceRole, HirSourceCommitInvariantError, HirSourceIndex,
    HirSourceQuery, HirSourceQueryError, HirSourceRequirement, HirSourceSite, StagedHirSourceIndex,
};

impl StagedHirSourceIndex {
    /// Stages the sole Entry component that is not already retained by the
    /// immutable item slot.
    #[allow(
        clippy::result_large_err,
        reason = "Entry staging preserves complete typed owner and source evidence"
    )]
    pub(crate) fn stage_attached_entry(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        attached: &TypedItemNode,
        retained: &HirItemKind,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        let applicable = matches!(
            (attached, retained),
            (TypedItemNode::Entry(_), HirItemKind::Entry(_))
        );
        if !applicable {
            if matches!(attached, TypedItemNode::Entry(_))
                || matches!(retained, HirItemKind::Entry(_))
            {
                return self.reject(
                    HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                        owner: SyntheticOwner::Item(owner),
                    },
                );
            }
            return Ok(());
        }
        if attached.snapshot_id() != parsed.snapshot_id() {
            return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                expected: parsed.snapshot_id().clone(),
                actual: attached.snapshot_id().clone(),
            });
        }
        let Some((id, member_values)) = entry_sources(owner, attached, retained)? else {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Item(owner),
                },
            );
        };
        self.stage_entry_component(parsed, owner, HirEntrySourcePart::Id, &id)?;
        for (member, span) in member_values {
            self.stage_entry_component(
                parsed,
                owner,
                HirEntrySourcePart::MemberValue { member },
                &span,
            )?;
        }
        Ok(())
    }

    #[allow(
        clippy::result_large_err,
        reason = "Entry component staging preserves complete typed role and source evidence"
    )]
    fn stage_entry_component(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        part: HirEntrySourcePart,
        span: &SourceSpan,
    ) -> Result<(), HirSourceCommitInvariantError> {
        let query = entry_query(owner, part);
        self.require(&query, HirSourceRequirement::Required)?;
        let site = HirSourceSite::from_attached_span(parsed.document(), span)?;
        self.stage(&query, site)
    }
}

impl HirItemKind {
    pub(crate) fn validate_entry_source_part(
        &self,
        owner: ItemId,
        part: HirEntrySourcePart,
    ) -> Result<(), HirSourceQueryError> {
        match (self, part) {
            (Self::Entry(_), HirEntrySourcePart::Whole | HirEntrySourcePart::Id) => Ok(()),
            (Self::Entry(entry), HirEntrySourcePart::MemberValue { member }) => {
                let Ok(index) = usize::try_from(member) else {
                    return Err(HirSourceQueryError::ItemOrdinalOutOfBounds {
                        owner,
                        role: HirItemSourceRole::Entry(part),
                        length: u32::try_from(entry.members().len()).unwrap_or(u32::MAX),
                    });
                };
                if entry.members().get(index).is_some_and(|member| {
                    matches!(
                        member,
                        crate::item::HirEntryMember::StateType(_)
                            | crate::item::HirEntryMember::Initializer(_)
                            | crate::item::HirEntryMember::EventType(_)
                            | crate::item::HirEntryMember::Reducer(_)
                            | crate::item::HirEntryMember::Controller(_)
                            | crate::item::HirEntryMember::Goto(_)
                    )
                }) {
                    Ok(())
                } else {
                    Err(HirSourceQueryError::ItemOrdinalOutOfBounds {
                        owner,
                        role: HirItemSourceRole::Entry(part),
                        length: u32::try_from(entry.members().len()).unwrap_or(u32::MAX),
                    })
                }
            }
            _ => Err(HirSourceQueryError::ItemRoleNotApplicable {
                owner,
                role: HirItemSourceRole::Entry(part),
            }),
        }
    }
}

pub(super) fn exact_manifest(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    owner: ItemId,
    attached: &TypedItemNode,
    retained: &HirItemKind,
) -> bool {
    let Ok(expected) = entry_sources(owner, attached, retained) else {
        return false;
    };
    let is_entry_query = |candidate: &&HirSourceQuery| {
        matches!(
            candidate,
            HirSourceQuery::Item {
                owner: actual,
                role: HirItemSourceRole::Entry(_),
            } if *actual == owner
        )
    };
    let Some((id, member_values)) = expected else {
        return index
            .requirements
            .keys()
            .find(|candidate| is_entry_query(candidate))
            .is_none()
            && index
                .components
                .keys()
                .find(|candidate| is_entry_query(candidate))
                .is_none();
    };
    let mut expected = Vec::with_capacity(member_values.len() + 1);
    expected.push((entry_query(owner, HirEntrySourcePart::Id), id));
    expected.extend(member_values.into_iter().map(|(member, span)| {
        (
            entry_query(owner, HirEntrySourcePart::MemberValue { member }),
            span,
        )
    }));
    let expected = expected
        .into_iter()
        .map(|(query, span)| {
            HirSourceSite::from_attached_span(parsed.document(), &span).map(|site| (query, site))
        })
        .collect::<Result<Vec<_>, _>>();
    let Ok(expected) = expected else {
        return false;
    };
    index
        .requirements
        .iter()
        .filter(|(candidate, _)| is_entry_query(candidate))
        .eq(expected
            .iter()
            .map(|(query, _)| (query, &HirSourceRequirement::Required)))
        && index
            .components
            .iter()
            .filter(|(candidate, _)| is_entry_query(candidate))
            .eq(expected.iter().map(|(query, site)| (query, site)))
}

#[allow(
    clippy::result_large_err,
    clippy::type_complexity,
    reason = "the closed Entry projection returns exact ID and ordered member-value source evidence"
)]
fn entry_sources(
    owner: ItemId,
    attached: &TypedItemNode,
    retained: &HirItemKind,
) -> Result<Option<(SourceSpan, Vec<(u32, SourceSpan)>)>, HirSourceCommitInvariantError> {
    match (attached, retained) {
        (TypedItemNode::Entry(node), HirItemKind::Entry(retained)) => {
            let entry = node.semantics().map_err(|error| {
                HirSourceCommitInvariantError::AttachedSyntaxAccess {
                    owner: SyntheticOwner::Item(owner),
                    error,
                }
            })?;
            if entry.body().members().len() != retained.members().len() {
                return Err(
                    HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                        owner: SyntheticOwner::Item(owner),
                    },
                );
            }
            let id = match entry.id() {
                AttachedEntryId::Authored { expression, .. } => expression.syntax().source_span(),
                AttachedEntryId::Missing(syntax) => syntax.syntax().source_span(),
            };
            let member_values = entry
                .body()
                .members()
                .iter()
                .filter_map(entry_member_value_source)
                .collect();
            Ok(Some((id, member_values)))
        }
        _ if matches!(attached, TypedItemNode::Entry(_))
            || matches!(retained, HirItemKind::Entry(_)) =>
        {
            Err(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Item(owner),
                },
            )
        }
        _ => Ok(None),
    }
}

fn entry_member_value_source(member: &AttachedEntryMember) -> Option<(u32, SourceSpan)> {
    let source = match member {
        AttachedEntryMember::StateType(binding) | AttachedEntryMember::EventType(binding) => {
            entry_type_value_source(binding.value())
        }
        AttachedEntryMember::Initializer(binding)
        | AttachedEntryMember::Reducer(binding)
        | AttachedEntryMember::Controller(binding) => entry_path_value_source(binding.value()),
        AttachedEntryMember::Goto { target, .. } => entry_expression_value_source(target),
        AttachedEntryMember::Route { .. }
        | AttachedEntryMember::Option { .. }
        | AttachedEntryMember::Error { .. } => return None,
    };
    Some((member.source_ordinal(), source))
}

fn entry_type_value_source(
    value: &AttachedEntryValue<arcweft_lang_syntax::attachment::AttachedTypeRefNode>,
) -> SourceSpan {
    match value {
        AttachedEntryValue::Authored(value) | AttachedEntryValue::Recovered(value) => {
            value.syntax().source_span()
        }
        AttachedEntryValue::Missing(syntax) | AttachedEntryValue::Invalid(syntax) => {
            syntax.source_span()
        }
    }
}

fn entry_path_value_source(
    value: &AttachedEntryValue<arcweft_lang_syntax::attachment::source_file::AttachedPath>,
) -> SourceSpan {
    match value {
        AttachedEntryValue::Authored(value) | AttachedEntryValue::Recovered(value) => {
            value.syntax().source_span()
        }
        AttachedEntryValue::Missing(syntax) | AttachedEntryValue::Invalid(syntax) => {
            syntax.source_span()
        }
    }
}

fn entry_expression_value_source(
    value: &AttachedEntryValue<arcweft_lang_syntax::attachment::AttachedExpressionNode>,
) -> SourceSpan {
    match value {
        AttachedEntryValue::Authored(value) | AttachedEntryValue::Recovered(value) => {
            value.whole_source_span()
        }
        AttachedEntryValue::Missing(syntax) | AttachedEntryValue::Invalid(syntax) => {
            syntax.source_span()
        }
    }
}

const fn entry_query(owner: ItemId, part: HirEntrySourcePart) -> HirSourceQuery {
    HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Entry(part),
    }
}
