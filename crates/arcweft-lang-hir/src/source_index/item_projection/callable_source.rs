//! Sole final-HIR source manifest for callable signatures.

use std::collections::BTreeMap;

use arcweft_lang_syntax::attachment::{
    AttachedCallableContractClause, AttachedCallableParameter, AttachedCallableReturn,
    AttachedFixedParameterGroup, AttachedImplMember, AttachedMethodParameter,
    AttachedMethodParameterGroup, AttachedRetainedName, AttachedTraitMember, TypedItemNode,
};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::patterns::PatternComponentRole;
use arcweft_source::{SourceRange, SourceSpan};

use crate::identity::{ItemId, SyntheticOwner};
use crate::item::{
    HirCapabilityMember, HirContractOperandList, HirFunctionParameterGroup, HirImplMember,
    HirItemKind, HirMethodParameter, HirParameter, HirTraitMember,
};
use crate::source_index::{
    HirCallableEffectSourcePart, HirCallableParameterSourcePart, HirCallableSourceOwner,
    HirCallableSourceRole, HirItemSourceRole, HirSourceCommitInvariantError, HirSourceIndex,
    HirSourceQuery, HirSourceQueryError, HirSourceRequirement, HirSourceSite, StagedHirSourceIndex,
};

#[derive(Default)]
struct CallableManifest {
    requirements: BTreeMap<HirSourceQuery, HirSourceRequirement>,
    components: BTreeMap<HirSourceQuery, HirSourceSite>,
}

impl CallableManifest {
    #[allow(
        clippy::result_large_err,
        reason = "required callable rows preserve complete typed query and source evidence"
    )]
    fn required(
        &mut self,
        parsed: &ParsedSource,
        query: HirSourceQuery,
        span: SourceSpan,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.insert(parsed, query, HirSourceRequirement::Required, Some(span))
    }

    #[allow(
        clippy::result_large_err,
        reason = "optional callable rows preserve complete typed query and source evidence"
    )]
    fn optional(
        &mut self,
        parsed: &ParsedSource,
        query: HirSourceQuery,
        span: Option<SourceSpan>,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.insert(parsed, query, HirSourceRequirement::Optional, span)
    }

    #[allow(
        clippy::result_large_err,
        reason = "callable manifest insertion preserves complete typed query and source evidence"
    )]
    fn insert(
        &mut self,
        parsed: &ParsedSource,
        query: HirSourceQuery,
        requirement: HirSourceRequirement,
        span: Option<SourceSpan>,
    ) -> Result<(), HirSourceCommitInvariantError> {
        if self
            .requirements
            .insert(query.clone(), requirement)
            .is_some()
        {
            return Err(HirSourceCommitInvariantError::ConflictingRequirement { query });
        }
        if let Some(span) = span {
            let site = HirSourceSite::from_attached_span(parsed.document(), &span)?;
            if self.components.insert(query.clone(), site).is_some() {
                return Err(HirSourceCommitInvariantError::ConflictingComponent { query });
            }
        } else if requirement == HirSourceRequirement::Required {
            return Err(HirSourceCommitInvariantError::MissingRequiredComponent { query });
        }
        Ok(())
    }
}

impl StagedHirSourceIndex {
    /// Stages every callable signature component from one attached item.
    #[allow(
        clippy::result_large_err,
        reason = "callable staging preserves complete typed owner and manifest evidence"
    )]
    pub(crate) fn stage_attached_callable(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        attached: &TypedItemNode,
        retained: &HirItemKind,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        let manifest = match callable_manifest(parsed, owner, attached, retained) {
            Ok(manifest) => manifest,
            Err(error) => return self.reject(error),
        };
        for (query, requirement) in manifest.requirements {
            self.require(&query, requirement)?;
        }
        for (query, site) in manifest.components {
            self.stage(&query, site)?;
        }
        Ok(())
    }
}

impl HirItemKind {
    pub(crate) fn validate_callable_source_role(
        &self,
        item: ItemId,
        role: HirCallableSourceRole,
    ) -> Result<(), HirSourceQueryError> {
        if let HirCallableSourceRole::EffectClause { owner, clause, .. } = role {
            return match (self, owner) {
                (Self::Function(function), HirCallableSourceOwner::Item)
                    if usize::from(clause) < function.effect_clauses().len() =>
                {
                    Ok(())
                }
                _ => Err(callable_role_not_applicable(item, role)),
            };
        }
        let parameter_counts = match (self, role.owner()) {
            (Self::Function(function), HirCallableSourceOwner::Item) => function
                .parameter_groups()
                .iter()
                .map(|group| group.parameters().len())
                .collect::<Vec<_>>(),
            (Self::Predicate(predicate), HirCallableSourceOwner::Item) => {
                vec![predicate.parameters().len()]
            }
            (Self::Proof(proof), HirCallableSourceOwner::Item) => vec![proof.parameters().len()],
            (Self::View(view), HirCallableSourceOwner::ViewItem) => {
                vec![view.parameters().len()]
            }
            (
                Self::ExternCapability(capability),
                HirCallableSourceOwner::ExternCapabilityFunction { member },
            ) => {
                let Some(HirCapabilityMember::Function(function)) =
                    capability.members().get(usize::from(member))
                else {
                    return Err(callable_role_not_applicable(item, role));
                };
                function
                    .parameter_groups()
                    .iter()
                    .map(|group| group.parameters().len())
                    .collect::<Vec<_>>()
            }
            (Self::Trait(trait_item), HirCallableSourceOwner::TraitFunction { member }) => {
                let Some(HirTraitMember::Function(function)) =
                    trait_item.members().get(usize::from(member))
                else {
                    return Err(callable_role_not_applicable(item, role));
                };
                function
                    .parameter_groups()
                    .iter()
                    .map(|group| group.parameters().len())
                    .collect::<Vec<_>>()
            }
            (Self::Impl(impl_item), HirCallableSourceOwner::ImplFunction { member }) => {
                let Some(HirImplMember::Function(function)) =
                    impl_item.members().get(usize::from(member))
                else {
                    return Err(callable_role_not_applicable(item, role));
                };
                function
                    .parameter_groups()
                    .iter()
                    .map(|group| group.parameters().len())
                    .collect::<Vec<_>>()
            }
            _ => return Err(callable_role_not_applicable(item, role)),
        };

        match role {
            HirCallableSourceRole::Name { .. }
            | HirCallableSourceRole::Signature { .. }
            | HirCallableSourceRole::Result { .. } => Ok(()),
            HirCallableSourceRole::Parameter {
                group, parameter, ..
            } => parameter_counts
                .get(usize::from(group))
                .filter(|count| usize::from(parameter) < **count)
                .map(|_| ())
                .ok_or_else(|| callable_role_not_applicable(item, role)),
            HirCallableSourceRole::EffectClause { .. } => {
                unreachable!("effect-clause roles return before parameter validation")
            }
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
    let Ok(expected) = callable_manifest(parsed, owner, attached, retained) else {
        return false;
    };
    let is_callable_query = |candidate: &&HirSourceQuery| {
        matches!(
            candidate,
            HirSourceQuery::Item {
                owner: actual,
                role: HirItemSourceRole::Callable(_),
            } if *actual == owner
        )
    };
    index
        .requirements
        .iter()
        .filter(|(candidate, _)| is_callable_query(candidate))
        .eq(expected.requirements.iter())
        && index
            .components
            .iter()
            .filter(|(candidate, _)| is_callable_query(candidate))
            .eq(expected.components.iter())
}

#[allow(
    clippy::result_large_err,
    clippy::too_many_lines,
    reason = "one callable manifest owns the complete signature, contract, return, body, and source-role matrix"
)]
fn callable_manifest(
    parsed: &ParsedSource,
    item: ItemId,
    attached: &TypedItemNode,
    retained: &HirItemKind,
) -> Result<CallableManifest, HirSourceCommitInvariantError> {
    let mut manifest = CallableManifest::default();
    match (attached, retained) {
        (TypedItemNode::Function(attached), HirItemKind::Function(retained)) => {
            let attached = attached.semantics().map_err(|error| {
                HirSourceCommitInvariantError::AttachedSyntaxAccess {
                    owner: SyntheticOwner::Item(item),
                    error,
                }
            })?;
            stage_callable(
                &mut manifest,
                parsed,
                item,
                HirCallableSourceOwner::Item,
                attached.name().syntax().source_span(),
                attached.parameter_groups(),
                retained
                    .parameter_groups()
                    .iter()
                    .map(HirFunctionParameterGroup::parameters)
                    .collect(),
                callable_return_span(parsed, item, attached.authored_return())?,
            )?;
            stage_effect_clauses(
                &mut manifest,
                parsed,
                item,
                attached.contracts(),
                retained.effect_clauses(),
            )?;
        }
        (TypedItemNode::Predicate(attached), HirItemKind::Predicate(retained)) => {
            let attached = attached.semantics().map_err(|error| {
                HirSourceCommitInvariantError::AttachedSyntaxAccess {
                    owner: SyntheticOwner::Item(item),
                    error,
                }
            })?;
            stage_callable(
                &mut manifest,
                parsed,
                item,
                HirCallableSourceOwner::Item,
                attached.name().syntax().source_span(),
                std::slice::from_ref(attached.parameter_group()),
                vec![retained.parameters()],
                None,
            )?;
        }
        (TypedItemNode::Proof(attached), HirItemKind::Proof(retained)) => {
            let attached = attached.semantics().map_err(|error| {
                HirSourceCommitInvariantError::AttachedSyntaxAccess {
                    owner: SyntheticOwner::Item(item),
                    error,
                }
            })?;
            stage_callable(
                &mut manifest,
                parsed,
                item,
                HirCallableSourceOwner::Item,
                attached.name().syntax().source_span(),
                std::slice::from_ref(attached.parameter_group()),
                vec![retained.parameters()],
                callable_return_span(parsed, item, attached.authored_return())?,
            )?;
        }
        (TypedItemNode::View(attached), HirItemKind::View(retained)) => {
            let attached = attached.semantics().map_err(|error| {
                HirSourceCommitInvariantError::AttachedSyntaxAccess {
                    owner: SyntheticOwner::Item(item),
                    error,
                }
            })?;
            stage_callable(
                &mut manifest,
                parsed,
                item,
                HirCallableSourceOwner::ViewItem,
                retained_name_source(attached.header().name()),
                std::slice::from_ref(attached.parameter_group()),
                vec![retained.parameters()],
                None,
            )?;
        }
        (TypedItemNode::ExternCapability(attached), HirItemKind::ExternCapability(retained)) => {
            let attached = attached.semantics().map_err(|error| {
                HirSourceCommitInvariantError::AttachedSyntaxAccess {
                    owner: SyntheticOwner::Item(item),
                    error,
                }
            })?;
            if attached.body().members().len() != retained.members().len() {
                return Err(
                    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                        owner: SyntheticOwner::Item(item),
                    },
                );
            }
            for (position, (attached, retained)) in attached
                .body()
                .members()
                .iter()
                .zip(retained.members())
                .enumerate()
            {
                let member = u16::try_from(position).map_err(|_| {
                    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                        owner: SyntheticOwner::Item(item),
                    }
                })?;
                if attached.source_ordinal() != member {
                    return Err(
                        HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                            owner: SyntheticOwner::Item(item),
                        },
                    );
                }
                let (
                    arcweft_lang_syntax::attachment::AttachedCapabilityMember::Function(attached),
                    HirCapabilityMember::Function(retained),
                ) = (attached, retained)
                else {
                    continue;
                };
                stage_callable(
                    &mut manifest,
                    parsed,
                    item,
                    HirCallableSourceOwner::ExternCapabilityFunction { member },
                    attached.name().syntax().source_span(),
                    attached.parameter_groups(),
                    retained
                        .parameter_groups()
                        .iter()
                        .map(HirFunctionParameterGroup::parameters)
                        .collect(),
                    callable_return_span(parsed, item, attached.authored_return())?,
                )?;
            }
        }
        (TypedItemNode::Trait(attached), HirItemKind::Trait(retained)) => {
            let attached = attached.semantics().map_err(|error| {
                HirSourceCommitInvariantError::AttachedSyntaxAccess {
                    owner: SyntheticOwner::Item(item),
                    error,
                }
            })?;
            if attached.body().members().len() != retained.members().len() {
                return Err(
                    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                        owner: SyntheticOwner::Item(item),
                    },
                );
            }
            for (position, (attached, retained)) in attached
                .body()
                .members()
                .iter()
                .zip(retained.members())
                .enumerate()
            {
                let member = u16::try_from(position).map_err(|_| {
                    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                        owner: SyntheticOwner::Item(item),
                    }
                })?;
                let (AttachedTraitMember::Function(attached), HirTraitMember::Function(retained)) =
                    (attached, retained)
                else {
                    continue;
                };
                if attached.source_ordinal() != member {
                    return Err(
                        HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                            owner: SyntheticOwner::Item(item),
                        },
                    );
                }
                stage_method_callable(
                    &mut manifest,
                    parsed,
                    item,
                    HirCallableSourceOwner::TraitFunction { member },
                    attached.name().syntax().source_span(),
                    attached.parameter_groups(),
                    retained.parameter_groups(),
                    callable_return_span(parsed, item, attached.authored_return())?,
                )?;
            }
        }
        (TypedItemNode::Impl(attached), HirItemKind::Impl(retained)) => {
            let attached = attached.semantics().map_err(|error| {
                HirSourceCommitInvariantError::AttachedSyntaxAccess {
                    owner: SyntheticOwner::Item(item),
                    error,
                }
            })?;
            if attached.body().members().len() != retained.members().len() {
                return Err(
                    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                        owner: SyntheticOwner::Item(item),
                    },
                );
            }
            for (position, (attached, retained)) in attached
                .body()
                .members()
                .iter()
                .zip(retained.members())
                .enumerate()
            {
                let member = u16::try_from(position).map_err(|_| {
                    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                        owner: SyntheticOwner::Item(item),
                    }
                })?;
                let (AttachedImplMember::Function(attached), HirImplMember::Function(retained)) =
                    (attached, retained)
                else {
                    continue;
                };
                if attached.source_ordinal() != member {
                    return Err(
                        HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                            owner: SyntheticOwner::Item(item),
                        },
                    );
                }
                stage_method_callable(
                    &mut manifest,
                    parsed,
                    item,
                    HirCallableSourceOwner::ImplFunction { member },
                    attached.name().syntax().source_span(),
                    attached.parameter_groups(),
                    retained.parameter_groups(),
                    callable_return_span(parsed, item, attached.authored_return())?,
                )?;
            }
        }
        _ => {}
    }
    Ok(manifest)
}

#[allow(
    clippy::result_large_err,
    reason = "effect-clause staging preserves complete typed ordinal and source evidence"
)]
fn stage_effect_clauses(
    manifest: &mut CallableManifest,
    parsed: &ParsedSource,
    item: ItemId,
    attached_contracts: &[AttachedCallableContractClause],
    retained_clauses: &[HirContractOperandList],
) -> Result<(), HirSourceCommitInvariantError> {
    let attached_clauses = attached_contracts
        .iter()
        .filter(|clause| clause.is_effects())
        .collect::<Vec<_>>();
    if attached_clauses.len() != retained_clauses.len() {
        return Err(
            HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(item),
            },
        );
    }
    for (position, (attached, retained)) in attached_clauses
        .into_iter()
        .zip(retained_clauses)
        .enumerate()
    {
        let clause = u16::try_from(position).map_err(|_| {
            HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(item),
            }
        })?;
        let Some(attached_operands) = attached.effects() else {
            unreachable!("effect filter admits only effect clauses")
        };
        if attached.family_ordinal() != clause
            || attached_operands.len() != retained.operands().len()
        {
            return Err(
                HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                    owner: SyntheticOwner::Item(item),
                },
            );
        }
        let query = |part| {
            callable_query(
                item,
                HirCallableSourceRole::EffectClause {
                    owner: HirCallableSourceOwner::Item,
                    clause,
                    part,
                },
            )
        };
        manifest.required(
            parsed,
            query(HirCallableEffectSourcePart::Whole),
            attached.syntax_source_span(),
        )?;
        manifest.required(
            parsed,
            query(HirCallableEffectSourcePart::Keyword),
            attached.keyword_source_span(),
        )?;
    }
    Ok(())
}

fn retained_name_source(name: &AttachedRetainedName) -> SourceSpan {
    match name {
        AttachedRetainedName::Resolved { syntax, .. }
        | AttachedRetainedName::Missing { syntax }
        | AttachedRetainedName::Invalid { syntax } => syntax.source_span(),
    }
}

#[allow(
    clippy::result_large_err,
    reason = "return projection preserves complete typed callable owner and syntax evidence"
)]
fn callable_return_span(
    parsed: &ParsedSource,
    item: ItemId,
    result: Option<&AttachedCallableReturn>,
) -> Result<Option<SourceSpan>, HirSourceCommitInvariantError> {
    result
        .map(|result| {
            parsed
                .document()
                .span(SourceRange::new(
                    result.arrow().source_span().range().start(),
                    result.ty().syntax().range().end(),
                ))
                .map_err(
                    |_| HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                        owner: SyntheticOwner::Item(item),
                    },
                )
        })
        .transpose()
}

#[allow(
    clippy::too_many_arguments,
    reason = "one source transaction validates every retained callable component"
)]
#[allow(
    clippy::result_large_err,
    reason = "callable projection preserves complete typed owner and source evidence"
)]
fn stage_callable(
    manifest: &mut CallableManifest,
    parsed: &ParsedSource,
    item: ItemId,
    owner: HirCallableSourceOwner,
    name: SourceSpan,
    attached_groups: &[AttachedFixedParameterGroup],
    retained_groups: Vec<&[HirParameter]>,
    result: Option<SourceSpan>,
) -> Result<(), HirSourceCommitInvariantError> {
    if attached_groups.len() != retained_groups.len() {
        return Err(
            HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(item),
            },
        );
    }
    let signature_end = result
        .as_ref()
        .map_or_else(
            || {
                attached_groups
                    .last()
                    .map(|group| group.syntax().range().end())
            },
            |result| Some(result.range().end()),
        )
        .ok_or(
            HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(item),
            },
        )?;
    let signature = parsed
        .document()
        .span(SourceRange::new(name.range().start(), signature_end))
        .map_err(
            |_| HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(item),
            },
        )?;
    manifest.required(
        parsed,
        callable_query(item, HirCallableSourceRole::Name { owner }),
        name,
    )?;
    manifest.required(
        parsed,
        callable_query(item, HirCallableSourceRole::Signature { owner }),
        signature,
    )?;
    manifest.optional(
        parsed,
        callable_query(item, HirCallableSourceRole::Result { owner }),
        result,
    )?;

    for (group_index, (attached_group, retained_group)) in
        attached_groups.iter().zip(retained_groups).enumerate()
    {
        let group = u16::try_from(group_index).map_err(|_| {
            HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(item),
            }
        })?;
        if attached_group.source_ordinal() != group
            || attached_group.parameters().len() != retained_group.len()
        {
            return Err(
                HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                    owner: SyntheticOwner::Item(item),
                },
            );
        }
        for (parameter_index, (attached, _retained)) in attached_group
            .parameters()
            .iter()
            .zip(retained_group)
            .enumerate()
        {
            let parameter = u16::try_from(parameter_index).map_err(|_| {
                HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                    owner: SyntheticOwner::Item(item),
                }
            })?;
            if attached.group_ordinal() != group || attached.parameter_ordinal() != parameter {
                return Err(
                    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                        owner: SyntheticOwner::Item(item),
                    },
                );
            }
            stage_parameter(manifest, parsed, item, owner, group, parameter, attached)?;
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "one source transaction validates every retained method component"
)]
#[allow(
    clippy::result_large_err,
    clippy::too_many_lines,
    reason = "one method-callable projection owns the complete receiver, parameters, return, body, and source-role matrix"
)]
fn stage_method_callable(
    manifest: &mut CallableManifest,
    parsed: &ParsedSource,
    item: ItemId,
    owner: HirCallableSourceOwner,
    name: SourceSpan,
    attached_groups: &[AttachedMethodParameterGroup],
    retained_groups: &[crate::item::HirMethodParameterGroup],
    result: Option<SourceSpan>,
) -> Result<(), HirSourceCommitInvariantError> {
    if attached_groups.len() != retained_groups.len() {
        return Err(
            HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(item),
            },
        );
    }
    let signature_end = result
        .as_ref()
        .map_or_else(
            || {
                attached_groups
                    .last()
                    .map(|group| group.syntax().range().end())
            },
            |result| Some(result.range().end()),
        )
        .ok_or(
            HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(item),
            },
        )?;
    let signature = parsed
        .document()
        .span(SourceRange::new(name.range().start(), signature_end))
        .map_err(
            |_| HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(item),
            },
        )?;
    manifest.required(
        parsed,
        callable_query(item, HirCallableSourceRole::Name { owner }),
        name,
    )?;
    manifest.required(
        parsed,
        callable_query(item, HirCallableSourceRole::Signature { owner }),
        signature,
    )?;
    manifest.optional(
        parsed,
        callable_query(item, HirCallableSourceRole::Result { owner }),
        result,
    )?;

    for (group_index, (attached_group, retained_group)) in
        attached_groups.iter().zip(retained_groups).enumerate()
    {
        let group = u16::try_from(group_index).map_err(|_| {
            HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(item),
            }
        })?;
        if attached_group.source_ordinal() != group
            || attached_group.parameters().len() != retained_group.parameters().len()
        {
            return Err(
                HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                    owner: SyntheticOwner::Item(item),
                },
            );
        }
        for (parameter_index, (attached, retained)) in attached_group
            .parameters()
            .iter()
            .zip(retained_group.parameters())
            .enumerate()
        {
            let parameter = u16::try_from(parameter_index).map_err(|_| {
                HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                    owner: SyntheticOwner::Item(item),
                }
            })?;
            if attached.group_ordinal() != group || attached.parameter_ordinal() != parameter {
                return Err(
                    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                        owner: SyntheticOwner::Item(item),
                    },
                );
            }
            match (attached, retained) {
                (AttachedMethodParameter::Typed(attached), HirMethodParameter::Typed(_)) => {
                    stage_parameter(manifest, parsed, item, owner, group, parameter, attached)?;
                }
                (AttachedMethodParameter::Receiver(attached), HirMethodParameter::Receiver(_)) => {
                    let query = |part| {
                        callable_query(
                            item,
                            HirCallableSourceRole::Parameter {
                                owner,
                                group,
                                parameter,
                                part,
                            },
                        )
                    };
                    manifest.required(
                        parsed,
                        query(HirCallableParameterSourcePart::Whole),
                        attached.whole_source().clone(),
                    )?;
                    manifest.required(
                        parsed,
                        query(HirCallableParameterSourcePart::Name),
                        attached.self_keyword_source().clone(),
                    )?;
                    manifest.optional(parsed, query(HirCallableParameterSourcePart::Type), None)?;
                    manifest.optional(
                        parsed,
                        query(HirCallableParameterSourcePart::Default),
                        None,
                    )?;
                }
                _ => {
                    return Err(
                        HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                            owner: SyntheticOwner::Item(item),
                        },
                    );
                }
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::result_large_err,
    reason = "parameter staging preserves complete typed ordinal and source evidence"
)]
fn stage_parameter(
    manifest: &mut CallableManifest,
    parsed: &ParsedSource,
    item: ItemId,
    owner: HirCallableSourceOwner,
    group: u16,
    parameter: u16,
    attached: &AttachedCallableParameter,
) -> Result<(), HirSourceCommitInvariantError> {
    let query = |part| {
        callable_query(
            item,
            HirCallableSourceRole::Parameter {
                owner,
                group,
                parameter,
                part,
            },
        )
    };
    manifest.required(
        parsed,
        query(HirCallableParameterSourcePart::Whole),
        attached.syntax().source_span(),
    )?;
    manifest.optional(
        parsed,
        query(HirCallableParameterSourcePart::Name),
        attached.pattern().component(PatternComponentRole::Name),
    )?;
    manifest.required(
        parsed,
        query(HirCallableParameterSourcePart::Type),
        attached.ty().syntax().source_span(),
    )?;
    manifest.optional(
        parsed,
        query(HirCallableParameterSourcePart::Default),
        attached
            .default()
            .map(|default| default.value().syntax().source_span()),
    )
}

const fn callable_query(item: ItemId, role: HirCallableSourceRole) -> HirSourceQuery {
    HirSourceQuery::Item {
        owner: item,
        role: HirItemSourceRole::Callable(role),
    }
}

const fn callable_role_not_applicable(
    owner: ItemId,
    role: HirCallableSourceRole,
) -> HirSourceQueryError {
    HirSourceQueryError::ItemRoleNotApplicable {
        owner,
        role: HirItemSourceRole::Callable(role),
    }
}
