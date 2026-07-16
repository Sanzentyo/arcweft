//! Session-owned presentation-environment precedence and update transaction.

use arcweft_presentation::appearance::{
    PresentationEnvironment, PresentationEnvironmentField, PresentationEnvironmentFieldRevisions,
    PresentationEnvironmentFieldSet, PresentationEnvironmentOverrides,
    PresentationEnvironmentValue, PresentationEnvironmentValues,
};
use thiserror::Error;

/// Provider, theme, and session sources plus their revisioned effective value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionEnvironmentState {
    provider: Option<PresentationEnvironmentValues>,
    theme: PresentationEnvironmentOverrides,
    session: PresentationEnvironmentOverrides,
    effective: PresentationEnvironment,
}

/// One committed environment source transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationEnvironmentUpdate {
    previous: PresentationEnvironment,
    current: PresentationEnvironment,
    source_changed_fields: PresentationEnvironmentFieldSet,
    effective_changed_fields: PresentationEnvironmentFieldSet,
}

/// Failure to preflight a monotonic environment revision transaction.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PresentationEnvironmentUpdateError {
    #[error("presentation environment revision overflow")]
    RevisionOverflow,
    #[error("presentation environment field revision overflow for {field:?}")]
    FieldRevisionOverflow { field: PresentationEnvironmentField },
}

impl SessionEnvironmentState {
    pub fn new(
        provider: Option<PresentationEnvironmentValues>,
        theme: PresentationEnvironmentOverrides,
    ) -> Self {
        let values = effective_values(provider, theme, PresentationEnvironmentOverrides::empty());
        Self {
            provider,
            theme,
            session: PresentationEnvironmentOverrides::empty(),
            effective: PresentationEnvironment::initial(values),
        }
    }

    pub const fn effective(&self) -> PresentationEnvironment {
        self.effective
    }

    pub const fn provider(&self) -> Option<PresentationEnvironmentValues> {
        self.provider
    }

    pub const fn theme(&self) -> PresentationEnvironmentOverrides {
        self.theme
    }

    pub const fn session(&self) -> PresentationEnvironmentOverrides {
        self.session
    }

    pub fn replace_provider(
        &mut self,
        values: PresentationEnvironmentValues,
    ) -> Result<PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError> {
        let mut candidate = self.clone();
        let source_changed_fields = provider_changed_fields(candidate.provider, Some(values));
        candidate.provider = Some(values);
        self.commit_candidate(candidate, source_changed_fields)
    }

    pub fn clear_provider(
        &mut self,
    ) -> Result<PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError> {
        let mut candidate = self.clone();
        let source_changed_fields = provider_changed_fields(candidate.provider, None);
        candidate.provider = None;
        self.commit_candidate(candidate, source_changed_fields)
    }

    pub fn set_session_override(
        &mut self,
        value: PresentationEnvironmentValue,
    ) -> Result<PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError> {
        let mut candidate = self.clone();
        let previous = candidate.session.get(value.field());
        candidate.session.insert(value);
        let source_changed_fields = if previous == Some(value) {
            PresentationEnvironmentFieldSet::NONE
        } else {
            PresentationEnvironmentFieldSet::from_field(value.field())
        };
        self.commit_candidate(candidate, source_changed_fields)
    }

    pub fn remove_session_override(
        &mut self,
        field: PresentationEnvironmentField,
    ) -> Result<PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError> {
        let mut candidate = self.clone();
        let source_changed_fields = if candidate.session.remove(field).is_some() {
            PresentationEnvironmentFieldSet::from_field(field)
        } else {
            PresentationEnvironmentFieldSet::NONE
        };
        self.commit_candidate(candidate, source_changed_fields)
    }

    pub(crate) fn replace_theme(
        &mut self,
        theme: PresentationEnvironmentOverrides,
    ) -> Result<PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError> {
        let mut candidate = self.clone();
        let source_changed_fields = override_changed_fields(candidate.theme, theme);
        candidate.theme = theme;
        self.commit_candidate(candidate, source_changed_fields)
    }

    fn commit_candidate(
        &mut self,
        mut candidate: Self,
        source_changed_fields: PresentationEnvironmentFieldSet,
    ) -> Result<PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError> {
        let previous = self.effective;
        let candidate_values =
            effective_values(candidate.provider, candidate.theme, candidate.session);
        let effective_changed_fields = values_changed_fields(previous.values(), candidate_values);
        if effective_changed_fields.is_empty() {
            candidate.effective = previous;
        } else {
            let revision = previous
                .revision()
                .checked_next()
                .ok_or(PresentationEnvironmentUpdateError::RevisionOverflow)?;
            let field_revisions = increment_field_revisions(previous, effective_changed_fields)?;
            candidate.effective = PresentationEnvironment::try_from_parts(
                candidate_values,
                revision,
                field_revisions,
            )
            .expect("preflighted field revisions cannot exceed the next global revision");
        }
        let current = candidate.effective;
        *self = candidate;
        Ok(PresentationEnvironmentUpdate {
            previous,
            current,
            source_changed_fields,
            effective_changed_fields,
        })
    }
}

impl PresentationEnvironmentUpdate {
    pub const fn previous(self) -> PresentationEnvironment {
        self.previous
    }

    pub const fn current(self) -> PresentationEnvironment {
        self.current
    }

    pub const fn source_changed_fields(self) -> PresentationEnvironmentFieldSet {
        self.source_changed_fields
    }

    pub const fn effective_changed_fields(self) -> PresentationEnvironmentFieldSet {
        self.effective_changed_fields
    }

    pub const fn effective_changed(self) -> bool {
        !self.effective_changed_fields.is_empty()
    }
}

fn effective_values(
    provider: Option<PresentationEnvironmentValues>,
    theme: PresentationEnvironmentOverrides,
    session: PresentationEnvironmentOverrides,
) -> PresentationEnvironmentValues {
    let provider = provider.unwrap_or(PresentationEnvironmentValues::ENGINE_DEFAULT);
    session.apply_to(theme.apply_to(provider))
}

fn provider_changed_fields(
    previous: Option<PresentationEnvironmentValues>,
    current: Option<PresentationEnvironmentValues>,
) -> PresentationEnvironmentFieldSet {
    match (previous, current) {
        (Some(previous), Some(current)) => values_changed_fields(previous, current),
        (None, None) => PresentationEnvironmentFieldSet::NONE,
        (Some(_), None) | (None, Some(_)) => PresentationEnvironmentFieldSet::ALL,
    }
}

fn override_changed_fields(
    previous: PresentationEnvironmentOverrides,
    current: PresentationEnvironmentOverrides,
) -> PresentationEnvironmentFieldSet {
    PresentationEnvironmentFieldSet::ALL
        .iter()
        .filter(|field| previous.get(*field) != current.get(*field))
        .fold(PresentationEnvironmentFieldSet::NONE, |fields, field| {
            fields.union(PresentationEnvironmentFieldSet::from_field(field))
        })
}

fn values_changed_fields(
    previous: PresentationEnvironmentValues,
    current: PresentationEnvironmentValues,
) -> PresentationEnvironmentFieldSet {
    PresentationEnvironmentFieldSet::ALL
        .iter()
        .filter(|field| previous.value(*field) != current.value(*field))
        .fold(PresentationEnvironmentFieldSet::NONE, |fields, field| {
            fields.union(PresentationEnvironmentFieldSet::from_field(field))
        })
}

fn increment_field_revisions(
    previous: PresentationEnvironment,
    changed: PresentationEnvironmentFieldSet,
) -> Result<PresentationEnvironmentFieldRevisions, PresentationEnvironmentUpdateError> {
    let next = |field| {
        let revision = previous.field_revision(field);
        if changed.contains(field) {
            revision
                .checked_next()
                .ok_or(PresentationEnvironmentUpdateError::FieldRevisionOverflow { field })
        } else {
            Ok(revision)
        }
    };
    Ok(PresentationEnvironmentFieldRevisions::new(
        next(PresentationEnvironmentField::ColorScheme)?,
        next(PresentationEnvironmentField::Contrast)?,
        next(PresentationEnvironmentField::ReducedMotion)?,
        next(PresentationEnvironmentField::TextScale)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_presentation::appearance::{ColorScheme, ContrastPreference, TextScaleMilli};

    fn values(
        color_scheme: ColorScheme,
        contrast: ContrastPreference,
        reduced_motion: bool,
        text_scale: u16,
    ) -> PresentationEnvironmentValues {
        PresentationEnvironmentValues::new(
            color_scheme,
            contrast,
            reduced_motion,
            TextScaleMilli::try_new(text_scale).unwrap(),
        )
    }

    #[test]
    fn precedence_hidden_updates_and_override_removal_are_exact() {
        let provider = values(
            ColorScheme::Light,
            ContrastPreference::Standard,
            false,
            1_000,
        );
        let mut theme = PresentationEnvironmentOverrides::empty();
        theme.insert(PresentationEnvironmentValue::ColorScheme(ColorScheme::Dark));
        let mut state = SessionEnvironmentState::new(Some(provider), theme);
        assert_eq!(state.effective().color_scheme(), ColorScheme::Dark);

        state
            .set_session_override(PresentationEnvironmentValue::ColorScheme(
                ColorScheme::Light,
            ))
            .unwrap();
        let hidden = state
            .replace_provider(values(
                ColorScheme::Dark,
                ContrastPreference::Standard,
                false,
                1_000,
            ))
            .unwrap();
        assert!(
            hidden
                .source_changed_fields()
                .contains(PresentationEnvironmentField::ColorScheme)
        );
        assert!(hidden.effective_changed_fields().is_empty());

        let revealed = state
            .remove_session_override(PresentationEnvironmentField::ColorScheme)
            .unwrap();
        assert_eq!(revealed.current().color_scheme(), ColorScheme::Dark);
        assert_eq!(revealed.current().revision().value(), 2);
        assert_eq!(
            revealed
                .current()
                .field_revision(PresentationEnvironmentField::ColorScheme)
                .value(),
            2
        );
    }

    #[test]
    fn multi_field_update_advances_global_once_and_changed_fields_once() {
        let mut state =
            SessionEnvironmentState::new(None, PresentationEnvironmentOverrides::empty());
        let update = state
            .replace_provider(values(
                ColorScheme::Light,
                ContrastPreference::More,
                true,
                1_250,
            ))
            .unwrap();
        assert_eq!(update.current().revision().value(), 1);
        assert_eq!(
            update.effective_changed_fields(),
            PresentationEnvironmentFieldSet::ALL
        );
        for field in PresentationEnvironmentFieldSet::ALL.iter() {
            assert_eq!(update.current().field_revision(field).value(), 1);
        }
    }

    #[test]
    fn same_value_update_preserves_all_revisions() {
        let mut state =
            SessionEnvironmentState::new(None, PresentationEnvironmentOverrides::empty());
        let update = state.clear_provider().unwrap();
        assert_eq!(update.previous(), update.current());
        assert!(update.source_changed_fields().is_empty());
        assert!(update.effective_changed_fields().is_empty());
    }

    #[test]
    fn preflight_overflow_does_not_mutate_sources_or_effective_state() {
        let mut state =
            SessionEnvironmentState::new(None, PresentationEnvironmentOverrides::empty());
        state.effective = PresentationEnvironment::try_from_parts(
            state.effective.values(),
            arcweft_presentation::appearance::EnvironmentRevision::from_value(u64::MAX),
            PresentationEnvironmentFieldRevisions::new(
                arcweft_presentation::appearance::EnvironmentRevision::from_value(u64::MAX),
                arcweft_presentation::appearance::EnvironmentRevision::ZERO,
                arcweft_presentation::appearance::EnvironmentRevision::ZERO,
                arcweft_presentation::appearance::EnvironmentRevision::ZERO,
            ),
        )
        .unwrap();
        let previous = state.clone();
        assert_eq!(
            state.set_session_override(PresentationEnvironmentValue::Contrast(
                ContrastPreference::More,
            )),
            Err(PresentationEnvironmentUpdateError::RevisionOverflow)
        );
        assert_eq!(state, previous);
    }
}
