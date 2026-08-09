//! Source projection for ordinary named declaration items.

use arcweft_lang_syntax::attachment::TypedItemNode;
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_source::SourceSpan;

use crate::identity::{ItemId, SyntheticOwner};
use crate::item::HirItemKind;
use crate::source_index::{
    HirDeclarationSourceRole, HirItemSourceRole, HirNominalMemberSourcePart,
    HirSourceCommitInvariantError, HirSourceIndex, HirSourceQuery, HirSourceQueryError,
    HirSourceRequirement, HirSourceSite, StagedHirSourceIndex,
};

impl StagedHirSourceIndex {
    /// Stages the exact required-name component for one admitted declaration.
    ///
    /// The declaration whole remains on the item slot. All six declaration
    /// families pass through this one dispatch, so no family-specific source
    /// table or later source-name reconstruction exists.
    #[allow(
        clippy::result_large_err,
        reason = "declaration staging failures preserve complete typed owner, role, and source evidence"
    )]
    pub(crate) fn stage_attached_declaration(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        attached: &TypedItemNode,
        retained: &HirItemKind,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        let applicable = matches!(
            (attached, retained),
            (TypedItemNode::Function(_), HirItemKind::Function(_))
                | (TypedItemNode::Predicate(_), HirItemKind::Predicate(_))
                | (TypedItemNode::Proof(_), HirItemKind::Proof(_))
                | (TypedItemNode::Struct(_), HirItemKind::Struct(_))
                | (TypedItemNode::Enum(_), HirItemKind::Enum(_))
                | (TypedItemNode::TypeAlias(_), HirItemKind::TypeAlias(_))
        );
        if !applicable {
            if is_declaration_syntax(attached) || is_declaration_kind(retained) {
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
        let Some(components) = declaration_component_sources(owner, attached, retained)? else {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Item(owner),
                },
            );
        };
        for (role, span) in components {
            let query = declaration_query(owner, role);
            self.require(&query, HirSourceRequirement::Required)?;
            let site = match HirSourceSite::from_attached_span(parsed.document(), &span) {
                Ok(site) => site,
                Err(error) => return self.reject(error.into()),
            };
            self.stage(&query, site)?;
        }
        Ok(())
    }
}

impl HirItemKind {
    pub(crate) fn validate_declaration_source_role(
        &self,
        owner: ItemId,
        role: HirDeclarationSourceRole,
    ) -> Result<(), HirSourceQueryError> {
        match (self, role) {
            (kind, HirDeclarationSourceRole::Whole) if admits_declaration_whole_source(kind) => {
                Ok(())
            }
            (kind, HirDeclarationSourceRole::Name) if is_declaration_kind(kind) => Ok(()),
            (
                HirItemKind::Proof(proof),
                HirDeclarationSourceRole::ProofTrustAttribute
                | HirDeclarationSourceRole::ProofTrustReason,
            ) if proof.trust().is_directly_trusted() => Ok(()),
            (HirItemKind::Struct(item), HirDeclarationSourceRole::StructField { field, .. }) => {
                validate_member_ordinal(owner, role, field, item.fields().len())
            }
            (HirItemKind::Enum(item), HirDeclarationSourceRole::EnumVariant { variant, .. }) => {
                validate_member_ordinal(owner, role, variant, item.variants().len())
            }
            _ => Err(HirSourceQueryError::ItemRoleNotApplicable {
                owner,
                role: HirItemSourceRole::Declaration(role),
            }),
        }
    }
}

fn validate_member_ordinal(
    owner: ItemId,
    role: HirDeclarationSourceRole,
    ordinal: u32,
    length: usize,
) -> Result<(), HirSourceQueryError> {
    let length = u32::try_from(length).expect("nominal member limit fits u32");
    if ordinal < length {
        Ok(())
    } else {
        Err(HirSourceQueryError::ItemOrdinalOutOfBounds {
            owner,
            role: HirItemSourceRole::Declaration(role),
            length,
        })
    }
}

pub(super) fn exact_manifest(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    owner: ItemId,
    attached: &TypedItemNode,
    retained: &HirItemKind,
) -> bool {
    let Ok(expected_components) = declaration_component_sources(owner, attached, retained) else {
        return false;
    };
    let is_declaration_query = |candidate: &&HirSourceQuery| {
        matches!(
            candidate,
            HirSourceQuery::Item {
                owner: actual,
                role: HirItemSourceRole::Declaration(_),
            } if *actual == owner
        )
    };

    let mut expected = Vec::new();
    for (role, span) in expected_components.into_iter().flatten() {
        let Ok(site) = HirSourceSite::from_attached_span(parsed.document(), &span) else {
            return false;
        };
        expected.push((declaration_query(owner, role), site));
    }
    expected.sort_by(|left, right| left.0.cmp(&right.0));

    index
        .requirements
        .iter()
        .filter(|(candidate, _)| is_declaration_query(candidate))
        .eq(expected
            .iter()
            .map(|(query, _)| (query, &HirSourceRequirement::Required)))
        && index
            .components
            .iter()
            .filter(|(candidate, _)| is_declaration_query(candidate))
            .eq(expected.iter().map(|(query, site)| (query, site)))
}

#[allow(
    clippy::result_large_err,
    clippy::too_many_lines,
    reason = "one declaration-family projection owns the complete typed component-source and recovery matrix"
)]
fn declaration_component_sources(
    owner: ItemId,
    attached: &TypedItemNode,
    retained: &HirItemKind,
) -> Result<Option<Vec<(HirDeclarationSourceRole, SourceSpan)>>, HirSourceCommitInvariantError> {
    let result = match (attached, retained) {
        (TypedItemNode::Function(node), HirItemKind::Function(_)) => {
            node.semantics().map(|declaration| {
                vec![(
                    HirDeclarationSourceRole::Name,
                    declaration.name().syntax().source_span(),
                )]
            })
        }
        (TypedItemNode::Predicate(node), HirItemKind::Predicate(_)) => {
            node.semantics().map(|declaration| {
                vec![(
                    HirDeclarationSourceRole::Name,
                    declaration.name().syntax().source_span(),
                )]
            })
        }
        (TypedItemNode::Proof(node), HirItemKind::Proof(proof)) => {
            node.semantics().map(|declaration| {
                let mut components = vec![(
                    HirDeclarationSourceRole::Name,
                    declaration.name().syntax().source_span(),
                )];
                if proof.trust().is_directly_trusted() {
                    if let Some(source) = declaration.trust_attribute_source_span() {
                        components.push((
                            HirDeclarationSourceRole::ProofTrustAttribute,
                            source.clone(),
                        ));
                    }
                    if let Some(source) = declaration.trust_reason_source_span() {
                        components
                            .push((HirDeclarationSourceRole::ProofTrustReason, source.clone()));
                    }
                }
                components
            })
        }
        (TypedItemNode::Struct(node), HirItemKind::Struct(_)) => {
            node.semantics().map(|declaration| {
                let mut components = vec![(
                    HirDeclarationSourceRole::Name,
                    declaration.name().syntax().source_span(),
                )];
                for (field, member) in declaration.body().fields().iter().enumerate() {
                    let field = u32::try_from(field).expect("nominal member limit fits u32");
                    components.push((
                        HirDeclarationSourceRole::StructField {
                            field,
                            part: HirNominalMemberSourcePart::Whole,
                        },
                        member.syntax().source_span(),
                    ));
                    components.push((
                        HirDeclarationSourceRole::StructField {
                            field,
                            part: HirNominalMemberSourcePart::Name,
                        },
                        member.name().syntax().source_span(),
                    ));
                }
                components
            })
        }
        (TypedItemNode::Enum(node), HirItemKind::Enum(_)) => node.semantics().map(|declaration| {
            let mut components = vec![(
                HirDeclarationSourceRole::Name,
                declaration.name().syntax().source_span(),
            )];
            for (variant, member) in declaration.body().variants().iter().enumerate() {
                let variant = u32::try_from(variant).expect("nominal member limit fits u32");
                components.push((
                    HirDeclarationSourceRole::EnumVariant {
                        variant,
                        part: HirNominalMemberSourcePart::Whole,
                    },
                    member.syntax().source_span(),
                ));
                components.push((
                    HirDeclarationSourceRole::EnumVariant {
                        variant,
                        part: HirNominalMemberSourcePart::Name,
                    },
                    member.name().syntax().source_span(),
                ));
            }
            components
        }),
        (TypedItemNode::TypeAlias(node), HirItemKind::TypeAlias(_)) => {
            node.semantics().map(|declaration| {
                vec![(
                    HirDeclarationSourceRole::Name,
                    declaration.name().syntax().source_span(),
                )]
            })
        }
        _ if is_declaration_syntax(attached) || is_declaration_kind(retained) => {
            return Err(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Item(owner),
                },
            );
        }
        _ => return Ok(None),
    };
    result.map(Some).map_err(
        |error| HirSourceCommitInvariantError::AttachedSyntaxAccess {
            owner: SyntheticOwner::Item(owner),
            error,
        },
    )
}

const fn declaration_query(owner: ItemId, role: HirDeclarationSourceRole) -> HirSourceQuery {
    HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Declaration(role),
    }
}

const fn is_declaration_syntax(attached: &TypedItemNode) -> bool {
    matches!(
        attached,
        TypedItemNode::Function(_)
            | TypedItemNode::Predicate(_)
            | TypedItemNode::Proof(_)
            | TypedItemNode::Struct(_)
            | TypedItemNode::Enum(_)
            | TypedItemNode::TypeAlias(_)
    )
}

const fn is_declaration_kind(retained: &HirItemKind) -> bool {
    matches!(
        retained,
        HirItemKind::Function(_)
            | HirItemKind::Predicate(_)
            | HirItemKind::Proof(_)
            | HirItemKind::Struct(_)
            | HirItemKind::Enum(_)
            | HirItemKind::TypeAlias(_)
    )
}

const fn admits_declaration_whole_source(retained: &HirItemKind) -> bool {
    is_declaration_kind(retained)
        || matches!(
            retained,
            HirItemKind::Character(_)
                | HirItemKind::View(_)
                | HirItemKind::Action(_)
                | HirItemKind::Activity(_)
                | HirItemKind::Signal(_)
                | HirItemKind::Metric(_)
                | HirItemKind::Layer(_)
        )
}
