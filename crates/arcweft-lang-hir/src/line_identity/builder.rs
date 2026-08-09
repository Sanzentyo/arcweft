use std::collections::BTreeMap;
use std::sync::Arc;

use arcweft_id::dialogue::{
    DialogueIdentityError, DialogueLineId, DialogueTextKey, MAX_DIALOGUE_ID_BYTES,
};

use crate::identity::HirLimit;
use crate::leaf::{HirIdRef, HirRelativeId};
use crate::lowering::HirModuleKey;

use super::{
    DialogueIdentityCoordinateKind, DialogueIdentityErrorKind, DialogueLineBuildFatal,
    DialogueLineBuildOperation, DialogueLineDiagnostic, DialogueLineIdOrigin,
    DialogueLineLimitKind, DialogueTextKeyOrigin, HirDialogueLineCandidate,
    HirDialogueLineCandidates, HirDialogueLineSourceOwner, HirDialogueLineSourceSite,
    OwnerlessLineRequestKind,
};

const GENERATED_ORDINAL_MAXIMUM: u32 = 262_144;
const CANDIDATE_WORK_UNITS: u32 = 5;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct DialogueLinePrefix(String);

impl DialogueLinePrefix {
    fn for_site(
        module: &HirModuleKey,
        site: &HirDialogueLineSourceSite,
        retained_scope_count: usize,
    ) -> Result<Self, DialogueLineBuildFatal> {
        let scopes = site
            .named_scopes()
            .get(..retained_scope_count)
            .ok_or(DialogueLineBuildFatal::InvalidInternalPrefix)?;
        let mut segments = Vec::new();
        match site.owner() {
            HirDialogueLineSourceOwner::Flow(owner) => {
                segments.push("say".to_owned());
                segments.extend(owner.id().as_str().split('.').map(str::to_owned));
            }
            HirDialogueLineSourceOwner::Callable(callable) => {
                if callable.package() != module.package() || callable.module() != module.path() {
                    return Err(DialogueLineBuildFatal::InvalidInternalPrefix);
                }
                segments.extend([
                    "say".to_owned(),
                    "fn".to_owned(),
                    callable.package().as_str().to_owned(),
                ]);
                segments.extend(
                    callable
                        .module()
                        .segments()
                        .iter()
                        .map(|segment| segment.as_str().to_owned()),
                );
                segments.push(callable.owner().as_str().to_owned());
                segments.extend(
                    callable
                        .owner_path()
                        .iter()
                        .map(|segment| segment.as_str().to_owned()),
                );
                segments.push(callable.name().to_owned());
            }
            HirDialogueLineSourceOwner::Ownerless => {
                return Err(DialogueLineBuildFatal::InvalidInternalPrefix);
            }
        }
        segments.extend(
            scopes
                .iter()
                .map(|scope| scope.segment().as_str().to_owned()),
        );
        if segments.iter().any(String::is_empty) {
            return Err(DialogueLineBuildFatal::InvalidInternalPrefix);
        }
        let value = segments.join(".");
        value
            .len()
            .checked_add(1)
            .ok_or(DialogueLineBuildFatal::ArithmeticOverflow {
                operation: DialogueLineBuildOperation::PrefixBytes,
            })?;
        Ok(Self(value))
    }

    fn append(&self, suffix: &str) -> Result<String, DialogueLineBuildFatal> {
        let capacity = self
            .0
            .len()
            .checked_add(1)
            .and_then(|length| length.checked_add(suffix.len()))
            .ok_or(DialogueLineBuildFatal::ArithmeticOverflow {
                operation: DialogueLineBuildOperation::PrefixBytes,
            })?;
        let mut value = String::with_capacity(capacity);
        value.push_str(&self.0);
        value.push('.');
        value.push_str(suffix);
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct DialogueGeneratedOrdinal(u32);

impl DialogueGeneratedOrdinal {
    fn peek_next(current: Option<Self>) -> Result<Self, DialogueLineBuildFatal> {
        let next = current.map_or(1, |ordinal| {
            ordinal
                .0
                .checked_add(1)
                .unwrap_or(GENERATED_ORDINAL_MAXIMUM.saturating_add(1))
        });
        if next > GENERATED_ORDINAL_MAXIMUM {
            return Err(DialogueLineBuildFatal::ArithmeticOverflow {
                operation: DialogueLineBuildOperation::GeneratedOrdinal,
            });
        }
        Ok(Self(next))
    }

    fn format(self) -> String {
        format!("{:03}", self.0)
    }
}

/// Sole module-local transaction for dialogue line candidate construction.
pub(crate) struct HirDialogueLineCandidateBuilder<'module> {
    module: &'module HirModuleKey,
    generated: BTreeMap<DialogueLinePrefix, DialogueGeneratedOrdinal>,
    candidates: Vec<HirDialogueLineCandidate>,
    diagnostics: Vec<DialogueLineDiagnostic>,
    source_order: u32,
    work: u32,
}

impl<'module> HirDialogueLineCandidateBuilder<'module> {
    pub(crate) fn new(module: &'module HirModuleKey) -> Self {
        Self {
            module,
            generated: BTreeMap::new(),
            candidates: Vec::new(),
            diagnostics: Vec::new(),
            source_order: 0,
            work: 0,
        }
    }

    /// Consumes one already-classified immediate coordinate pair. Coordinate
    /// classification remains owned by the HIR expression adapter; this method
    /// owns all durable identity, limit, ordinal, and candidate mutation.
    pub(crate) fn push(
        &mut self,
        site: HirDialogueLineSourceSite,
        id: Option<&HirIdRef>,
        text_key: Option<&HirIdRef>,
    ) -> Result<(), DialogueLineBuildFatal> {
        self.accept_site(&site)?;

        let Some(line) = self.resolve_line(&site, id)? else {
            return Ok(());
        };
        let Some(resolved_text) = self.resolve_text_key(&site, &line.0, text_key)? else {
            return Ok(());
        };
        let observed = self.candidates.len().checked_add(1).ok_or(
            DialogueLineBuildFatal::ArithmeticOverflow {
                operation: DialogueLineBuildOperation::Work,
            },
        )?;
        let maximum = HirLimit::Expressions.maximum();
        if observed > maximum {
            return Err(DialogueLineBuildFatal::CandidateLimit { observed, maximum });
        }
        if let Some((prefix, ordinal)) = line.2 {
            self.generated.insert(prefix, ordinal);
        }
        self.candidates.push(HirDialogueLineCandidate {
            id: line.0,
            id_origin: line.1,
            text_key: resolved_text.0,
            text_key_origin: resolved_text.1,
            site,
        });
        Ok(())
    }

    pub(crate) fn reject(
        &mut self,
        site: &HirDialogueLineSourceSite,
        diagnostic: DialogueLineDiagnostic,
    ) -> Result<(), DialogueLineBuildFatal> {
        self.accept_site(site)?;
        self.push_diagnostic(diagnostic)
    }

    pub(crate) fn skip(
        &mut self,
        site: &HirDialogueLineSourceSite,
    ) -> Result<(), DialogueLineBuildFatal> {
        self.accept_site(site)
    }

    pub(crate) fn finish(mut self) -> (HirDialogueLineCandidates, Arc<[DialogueLineDiagnostic]>) {
        if !self.diagnostics.is_empty() {
            self.candidates.clear();
        }
        (
            HirDialogueLineCandidates {
                module: self.module.clone(),
                records: Arc::from(self.candidates),
            },
            Arc::from(self.diagnostics),
        )
    }

    fn accept_site(
        &mut self,
        site: &HirDialogueLineSourceSite,
    ) -> Result<(), DialogueLineBuildFatal> {
        self.charge_work(CANDIDATE_WORK_UNITS)?;
        let next_source_order =
            self.source_order
                .checked_add(1)
                .ok_or(DialogueLineBuildFatal::ArithmeticOverflow {
                    operation: DialogueLineBuildOperation::SourceOrder,
                })?;
        if site.source_order().get() != next_source_order
            || site.application_span().source() != self.module.source()
        {
            return Err(DialogueLineBuildFatal::SourceIdentityMismatch {
                expected: self.module.source().clone(),
                actual: site.application_span().source().clone(),
            });
        }
        self.source_order = next_source_order;
        Ok(())
    }

    fn resolve_line(
        &mut self,
        site: &HirDialogueLineSourceSite,
        reference: Option<&HirIdRef>,
    ) -> Result<Option<ResolvedLine>, DialogueLineBuildFatal> {
        let span = site.id_coordinate_span().unwrap_or(site.application_span());
        match reference {
            Some(HirIdRef::Absolute(reference)) => {
                let value = reference.as_str();
                if reference.segments().next() != Some(DialogueLineId::family_prefix()) {
                    self.push_diagnostic(DialogueLineDiagnostic::InvalidLineIdFamily {
                        found: reference.segments().next().unwrap_or_default().to_owned(),
                        span: span.clone(),
                    })?;
                    return Ok(None);
                }
                self.construct_line(
                    value.to_owned(),
                    DialogueLineIdOrigin::ExplicitAbsolute,
                    span,
                    None,
                )
            }
            Some(HirIdRef::Relative(relative)) => self.resolve_relative(
                site,
                relative,
                DialogueLineIdOrigin::ExplicitRelative,
                OwnerlessLineRequestKind::Relative,
                span,
            ),
            Some(HirIdRef::FamilyRelative(relative)) => {
                if relative.family().as_str() != DialogueLineId::family_prefix() {
                    self.push_diagnostic(DialogueLineDiagnostic::InvalidLineIdFamily {
                        found: relative.family().as_str().to_owned(),
                        span: span.clone(),
                    })?;
                    return Ok(None);
                }
                self.resolve_relative(
                    site,
                    relative.relative(),
                    DialogueLineIdOrigin::ExplicitFamilyRelative,
                    OwnerlessLineRequestKind::FamilyRelative,
                    span,
                )
            }
            None => {
                if matches!(site.owner(), HirDialogueLineSourceOwner::Ownerless) {
                    self.push_diagnostic(DialogueLineDiagnostic::MissingLineSourceOwner {
                        application: site.application_span().clone(),
                        coordinate: None,
                        request: OwnerlessLineRequestKind::Generated,
                    })?;
                    return Ok(None);
                }
                let prefix =
                    DialogueLinePrefix::for_site(self.module, site, site.named_scopes().len())?;
                let ordinal =
                    DialogueGeneratedOrdinal::peek_next(self.generated.get(&prefix).copied())?;
                let value = prefix.append(&ordinal.format())?;
                self.construct_line(
                    value,
                    DialogueLineIdOrigin::Generated,
                    span,
                    Some((prefix, ordinal)),
                )
            }
        }
    }

    fn resolve_relative(
        &mut self,
        site: &HirDialogueLineSourceSite,
        relative: &HirRelativeId,
        origin: DialogueLineIdOrigin,
        request: OwnerlessLineRequestKind,
        span: &arcweft_source::SourceSpan,
    ) -> Result<Option<ResolvedLine>, DialogueLineBuildFatal> {
        if matches!(site.owner(), HirDialogueLineSourceOwner::Ownerless) {
            self.push_diagnostic(DialogueLineDiagnostic::MissingLineSourceOwner {
                application: site.application_span().clone(),
                coordinate: Some(span.clone()),
                request,
            })?;
            return Ok(None);
        }
        let available = site.named_scopes().len();
        let requested = relative.parent_depth();
        if requested > available {
            self.push_diagnostic(DialogueLineDiagnostic::RelativeLineIdEscapesOwner {
                requested: u16::try_from(requested).unwrap_or(u16::MAX),
                available: u16::try_from(available).unwrap_or(u16::MAX),
                span: span.clone(),
            })?;
            return Ok(None);
        }
        let prefix = DialogueLinePrefix::for_site(self.module, site, available - requested)?;
        let value = prefix.append(relative.suffix().as_str())?;
        self.construct_line(value, origin, span, None)
    }

    fn construct_line(
        &mut self,
        value: String,
        origin: DialogueLineIdOrigin,
        span: &arcweft_source::SourceSpan,
        generated: Option<(DialogueLinePrefix, DialogueGeneratedOrdinal)>,
    ) -> Result<Option<ResolvedLine>, DialogueLineBuildFatal> {
        match DialogueLineId::try_new(value) {
            Ok(id) => Ok(Some((id, origin, generated))),
            Err(error) => {
                self.push_identity_error(DialogueIdentityCoordinateKind::LineId, error, span)?;
                Ok(None)
            }
        }
    }

    fn resolve_text_key(
        &mut self,
        site: &HirDialogueLineSourceSite,
        line: &DialogueLineId,
        reference: Option<&HirIdRef>,
    ) -> Result<Option<(DialogueTextKey, DialogueTextKeyOrigin)>, DialogueLineBuildFatal> {
        let span = site
            .text_key_coordinate_span()
            .unwrap_or(site.application_span());
        let value = match reference {
            None => match line.generated_text_key() {
                Ok(key) => return Ok(Some((key, DialogueTextKeyOrigin::Derived))),
                Err(error) => {
                    self.push_identity_error(DialogueIdentityCoordinateKind::TextKey, error, span)?;
                    return Ok(None);
                }
            },
            Some(HirIdRef::Absolute(reference))
                if reference.segments().next() == Some(DialogueTextKey::family_prefix()) =>
            {
                reference.as_str().to_owned()
            }
            Some(HirIdRef::Absolute(reference)) => {
                self.push_diagnostic(DialogueLineDiagnostic::InvalidTextKeyFamily {
                    found: reference.segments().next().map(str::to_owned),
                    span: span.clone(),
                })?;
                return Ok(None);
            }
            Some(HirIdRef::Relative(_) | HirIdRef::FamilyRelative(_)) => {
                self.push_diagnostic(DialogueLineDiagnostic::InvalidTextKeyFamily {
                    found: None,
                    span: span.clone(),
                })?;
                return Ok(None);
            }
        };
        match DialogueTextKey::try_new(value) {
            Ok(key) => Ok(Some((key, DialogueTextKeyOrigin::Explicit))),
            Err(error) => {
                self.push_identity_error(DialogueIdentityCoordinateKind::TextKey, error, span)?;
                Ok(None)
            }
        }
    }

    fn push_identity_error(
        &mut self,
        coordinate: DialogueIdentityCoordinateKind,
        error: DialogueIdentityError,
        span: &arcweft_source::SourceSpan,
    ) -> Result<(), DialogueLineBuildFatal> {
        let diagnostic = match error {
            DialogueIdentityError::TooManyBytes { bytes, maximum, .. } => {
                DialogueLineDiagnostic::DialogueLineIdentityLimit {
                    kind: DialogueLineLimitKind::IdentityBytes,
                    observed: u64::try_from(bytes).unwrap_or(u64::MAX),
                    maximum: u64::try_from(maximum).unwrap_or(u64::MAX),
                    span: Some(span.clone()),
                }
            }
            error => DialogueLineDiagnostic::InvalidDialogueLineIdentity {
                coordinate,
                reason: identity_error_kind(&error),
                span: span.clone(),
            },
        };
        self.push_diagnostic(diagnostic)
    }

    fn push_diagnostic(
        &mut self,
        diagnostic: DialogueLineDiagnostic,
    ) -> Result<(), DialogueLineBuildFatal> {
        let observed = self.diagnostics.len().checked_add(1).ok_or(
            DialogueLineBuildFatal::ArithmeticOverflow {
                operation: DialogueLineBuildOperation::Work,
            },
        )?;
        let maximum = HirLimit::Diagnostics.maximum();
        if observed > maximum {
            return Err(DialogueLineBuildFatal::DiagnosticLimit { observed, maximum });
        }
        self.diagnostics.push(diagnostic);
        Ok(())
    }

    fn charge_work(&mut self, units: u32) -> Result<(), DialogueLineBuildFatal> {
        let observed =
            self.work
                .checked_add(units)
                .ok_or(DialogueLineBuildFatal::ArithmeticOverflow {
                    operation: DialogueLineBuildOperation::Work,
                })?;
        let maximum = u32::try_from(HirLimit::Expressions.maximum())
            .ok()
            .and_then(|count| count.checked_mul(CANDIDATE_WORK_UNITS))
            .ok_or(DialogueLineBuildFatal::ArithmeticOverflow {
                operation: DialogueLineBuildOperation::Work,
            })?;
        if observed > maximum {
            return Err(DialogueLineBuildFatal::WorkLimit { observed, maximum });
        }
        self.work = observed;
        Ok(())
    }
}

type ResolvedLine = (
    DialogueLineId,
    DialogueLineIdOrigin,
    Option<(DialogueLinePrefix, DialogueGeneratedOrdinal)>,
);

const fn identity_error_kind(error: &DialogueIdentityError) -> DialogueIdentityErrorKind {
    match error {
        DialogueIdentityError::InvalidBase { .. } => DialogueIdentityErrorKind::InvalidBase,
        DialogueIdentityError::WrongFamily { .. } => DialogueIdentityErrorKind::WrongFamily,
        DialogueIdentityError::EmptyTail { .. } => DialogueIdentityErrorKind::EmptyTail,
        DialogueIdentityError::TooManyBytes { .. } => DialogueIdentityErrorKind::TooManyBytes,
    }
}

const _: () = assert!(MAX_DIALOGUE_ID_BYTES == 256);
