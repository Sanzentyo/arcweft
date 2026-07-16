//! Typed callable model owned by adapter manifests before sema publication.

use std::collections::HashSet;

use thiserror::Error;

use crate::manifest::AdapterTypeKind;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterCallableName(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterCallablePath(Vec<AdapterCallableName>);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterCallableOverloadIndex(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterCallableGroupIndex(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterCallableParameterIndex(u16);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFunctionSignature {
    groups: Vec<AdapterParameterGroup>,
    return_type: AdapterTypeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterParameterGroup {
    index: AdapterCallableGroupIndex,
    parameters: Vec<AdapterFunctionParam>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFunctionParam {
    index: AdapterCallableParameterIndex,
    name: Option<AdapterCallableName>,
    ty: AdapterTypeKind,
    passing: AdapterParameterPassing,
    presence: AdapterParameterPresence,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AdapterParameterPassing {
    PositionalOrNamed,
    PositionalOnly,
    NamedOnly,
    RestPositional,
    RestNamed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AdapterParameterPresence {
    Required,
    Defaulted,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AdapterFreeCallableKind {
    Function,
    RustFunction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterToolingSubject {
    Free {
        kind: AdapterFreeCallableKind,
        path: AdapterCallablePath,
        overload: AdapterCallableOverloadIndex,
    },
    Method {
        receiver: AdapterTypeKind,
        name: AdapterCallableName,
        overload: AdapterCallableOverloadIndex,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterToolingParameterDoc {
    group: AdapterCallableGroupIndex,
    parameter: AdapterCallableParameterIndex,
    text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterToolingDoc {
    subject: AdapterToolingSubject,
    summary: Option<String>,
    details: Option<String>,
    parameters: Vec<AdapterToolingParameterDoc>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdapterCallableModelError {
    #[error("adapter callable name must not be empty")]
    EmptyName,
    #[error("adapter callable name contains a control character at byte {byte}")]
    Control { byte: usize },
    #[error("adapter callable name contains separator `{separator}` at byte {byte}")]
    Separator { byte: usize, separator: char },
    #[error("adapter callable path must contain at least one typed segment")]
    EmptyPath,
    #[error("adapter callable index {value} exceeds u16")]
    IndexOverflow { value: usize },
    #[error("adapter callable signature must contain an initial group")]
    EmptyGroups,
    #[error("non-contiguous adapter callable group: expected {expected}, got {actual}")]
    NonContiguousGroup { expected: usize, actual: usize },
    #[error(
        "non-contiguous adapter callable parameter in group {group}: expected {expected}, got {actual}"
    )]
    NonContiguousParameter {
        group: usize,
        expected: usize,
        actual: usize,
    },
    #[error("duplicate adapter callable parameter `{name}` in group {group}")]
    DuplicateParameterName { group: usize, name: String },
    #[error("adapter callable parameter {parameter} in group {group} requires a name")]
    MissingParameterName { group: usize, parameter: usize },
    #[error("rest parameter {parameter} in group {group} cannot be defaulted")]
    DefaultedRest { group: usize, parameter: usize },
    #[error("group {group} contains more than one {passing:?} parameter")]
    DuplicateRest {
        group: usize,
        passing: AdapterParameterPassing,
    },
    #[error("rest parameter {parameter} in group {group} is not final in its passing class")]
    RestNotFinal { group: usize, parameter: usize },
    #[error("adapter tooling documentation must contain non-empty content")]
    EmptyDocumentation,
    #[error("adapter tooling subject occurs more than once")]
    DuplicateToolingSubject { subject: AdapterToolingSubject },
    #[error(
        "adapter tooling parameter coordinate ({group}, {parameter}) does not exist for its subject"
    )]
    ToolingParameterOutOfBounds {
        subject: AdapterToolingSubject,
        group: usize,
        parameter: usize,
    },
}

impl AdapterCallableName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, AdapterCallableModelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(AdapterCallableModelError::EmptyName);
        }
        for (byte, character) in value.char_indices() {
            if character.is_control() {
                return Err(AdapterCallableModelError::Control { byte });
            }
            if matches!(
                character,
                '.' | ':' | '/' | '\\' | '(' | ')' | '[' | ']' | '{' | '}'
            ) {
                return Err(AdapterCallableModelError::Separator {
                    byte,
                    separator: character,
                });
            }
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AdapterCallablePath {
    pub fn try_new(
        segments: impl IntoIterator<Item = AdapterCallableName>,
    ) -> Result<Self, AdapterCallableModelError> {
        let segments = segments.into_iter().collect::<Vec<_>>();
        if segments.is_empty() {
            return Err(AdapterCallableModelError::EmptyPath);
        }
        Ok(Self(segments))
    }

    pub fn single(segment: AdapterCallableName) -> Self {
        Self(vec![segment])
    }

    pub fn segments(&self) -> &[AdapterCallableName] {
        &self.0
    }
}

macro_rules! checked_index {
    ($name:ident) => {
        impl $name {
            pub fn try_from_usize(value: usize) -> Result<Self, AdapterCallableModelError> {
                u16::try_from(value)
                    .map(Self)
                    .map_err(|_| AdapterCallableModelError::IndexOverflow { value })
            }

            pub const fn get(self) -> usize {
                self.0 as usize
            }
        }
    };
}

checked_index!(AdapterCallableOverloadIndex);
checked_index!(AdapterCallableGroupIndex);
checked_index!(AdapterCallableParameterIndex);

impl AdapterFunctionParam {
    pub fn try_new(
        index: AdapterCallableParameterIndex,
        name: Option<AdapterCallableName>,
        ty: AdapterTypeKind,
        passing: AdapterParameterPassing,
        presence: AdapterParameterPresence,
    ) -> Result<Self, AdapterCallableModelError> {
        if matches!(
            passing,
            AdapterParameterPassing::NamedOnly | AdapterParameterPassing::RestNamed
        ) && name.is_none()
        {
            return Err(AdapterCallableModelError::MissingParameterName {
                group: 0,
                parameter: index.get(),
            });
        }
        if matches!(
            passing,
            AdapterParameterPassing::RestPositional | AdapterParameterPassing::RestNamed
        ) && presence == AdapterParameterPresence::Defaulted
        {
            return Err(AdapterCallableModelError::DefaultedRest {
                group: 0,
                parameter: index.get(),
            });
        }
        Ok(Self {
            index,
            name,
            ty,
            passing,
            presence,
        })
    }

    pub const fn index(&self) -> AdapterCallableParameterIndex {
        self.index
    }

    pub const fn name(&self) -> Option<&AdapterCallableName> {
        self.name.as_ref()
    }

    pub const fn ty(&self) -> &AdapterTypeKind {
        &self.ty
    }

    pub const fn passing(&self) -> AdapterParameterPassing {
        self.passing
    }

    pub const fn presence(&self) -> AdapterParameterPresence {
        self.presence
    }
}

impl AdapterParameterGroup {
    pub fn try_new(
        index: AdapterCallableGroupIndex,
        parameters: Vec<AdapterFunctionParam>,
    ) -> Result<Self, AdapterCallableModelError> {
        let group = index.get();
        let mut names = HashSet::new();
        let mut rest_positional = None;
        let mut rest_named = None;
        for (expected, parameter) in parameters.iter().enumerate() {
            if parameter.index.get() != expected {
                return Err(AdapterCallableModelError::NonContiguousParameter {
                    group,
                    expected,
                    actual: parameter.index.get(),
                });
            }
            if let Some(name) = parameter.name()
                && !names.insert(name.as_str())
            {
                return Err(AdapterCallableModelError::DuplicateParameterName {
                    group,
                    name: name.as_str().to_owned(),
                });
            }
            match parameter.passing {
                AdapterParameterPassing::RestPositional => {
                    if rest_positional.replace(expected).is_some() {
                        return Err(AdapterCallableModelError::DuplicateRest {
                            group,
                            passing: parameter.passing,
                        });
                    }
                }
                AdapterParameterPassing::RestNamed => {
                    if rest_named.replace(expected).is_some() {
                        return Err(AdapterCallableModelError::DuplicateRest {
                            group,
                            passing: parameter.passing,
                        });
                    }
                }
                AdapterParameterPassing::PositionalOrNamed
                | AdapterParameterPassing::PositionalOnly
                | AdapterParameterPassing::NamedOnly => {}
            }
        }
        if let Some(rest) = rest_positional
            && parameters[rest + 1..].iter().any(|parameter| {
                matches!(
                    parameter.passing,
                    AdapterParameterPassing::PositionalOrNamed
                        | AdapterParameterPassing::PositionalOnly
                        | AdapterParameterPassing::RestPositional
                )
            })
        {
            return Err(AdapterCallableModelError::RestNotFinal {
                group,
                parameter: rest,
            });
        }
        if let Some(rest) = rest_named
            && parameters[rest + 1..].iter().any(|parameter| {
                matches!(
                    parameter.passing,
                    AdapterParameterPassing::PositionalOrNamed
                        | AdapterParameterPassing::NamedOnly
                        | AdapterParameterPassing::RestNamed
                )
            })
        {
            return Err(AdapterCallableModelError::RestNotFinal {
                group,
                parameter: rest,
            });
        }
        Ok(Self { index, parameters })
    }

    pub const fn index(&self) -> AdapterCallableGroupIndex {
        self.index
    }

    pub fn parameters(&self) -> &[AdapterFunctionParam] {
        &self.parameters
    }
}

impl AdapterFunctionSignature {
    pub fn try_new(
        groups: Vec<AdapterParameterGroup>,
        return_type: AdapterTypeKind,
    ) -> Result<Self, AdapterCallableModelError> {
        if groups.is_empty() {
            return Err(AdapterCallableModelError::EmptyGroups);
        }
        for (expected, group) in groups.iter().enumerate() {
            if group.index.get() != expected {
                return Err(AdapterCallableModelError::NonContiguousGroup {
                    expected,
                    actual: group.index.get(),
                });
            }
        }
        Ok(Self {
            groups,
            return_type,
        })
    }

    pub fn groups(&self) -> &[AdapterParameterGroup] {
        &self.groups
    }

    pub const fn return_type(&self) -> &AdapterTypeKind {
        &self.return_type
    }
}

impl AdapterToolingParameterDoc {
    pub fn try_new(
        group: AdapterCallableGroupIndex,
        parameter: AdapterCallableParameterIndex,
        text: impl Into<String>,
    ) -> Result<Self, AdapterCallableModelError> {
        let text = text.into();
        if text.is_empty() {
            return Err(AdapterCallableModelError::EmptyDocumentation);
        }
        Ok(Self {
            group,
            parameter,
            text,
        })
    }

    pub const fn group(&self) -> AdapterCallableGroupIndex {
        self.group
    }

    pub const fn parameter(&self) -> AdapterCallableParameterIndex {
        self.parameter
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

impl AdapterToolingDoc {
    pub fn try_new(
        subject: AdapterToolingSubject,
        summary: Option<String>,
        details: Option<String>,
        parameters: Vec<AdapterToolingParameterDoc>,
    ) -> Result<Self, AdapterCallableModelError> {
        if summary.as_deref().is_none_or(str::is_empty)
            && details.as_deref().is_none_or(str::is_empty)
            && parameters.is_empty()
        {
            return Err(AdapterCallableModelError::EmptyDocumentation);
        }
        Ok(Self {
            subject,
            summary,
            details,
            parameters,
        })
    }

    pub const fn subject(&self) -> &AdapterToolingSubject {
        &self.subject
    }

    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub fn details(&self) -> Option<&str> {
        self.details.as_deref()
    }

    pub fn parameters(&self) -> &[AdapterToolingParameterDoc] {
        &self.parameters
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(value: usize) -> AdapterCallableParameterIndex {
        AdapterCallableParameterIndex::try_from_usize(value).unwrap()
    }

    fn group(value: usize) -> AdapterCallableGroupIndex {
        AdapterCallableGroupIndex::try_from_usize(value).unwrap()
    }

    fn parameter(value: usize, name: &str) -> AdapterFunctionParam {
        AdapterFunctionParam::try_new(
            index(value),
            Some(AdapterCallableName::try_new(name).unwrap()),
            AdapterTypeKind::String,
            AdapterParameterPassing::PositionalOrNamed,
            AdapterParameterPresence::Required,
        )
        .unwrap()
    }

    #[test]
    fn signature_rejects_group_and_parameter_gaps() {
        assert_eq!(
            AdapterParameterGroup::try_new(group(0), vec![parameter(1, "value")]),
            Err(AdapterCallableModelError::NonContiguousParameter {
                group: 0,
                expected: 0,
                actual: 1,
            })
        );
        let later = AdapterParameterGroup::try_new(group(1), vec![]).unwrap();
        assert_eq!(
            AdapterFunctionSignature::try_new(vec![later], AdapterTypeKind::Unit),
            Err(AdapterCallableModelError::NonContiguousGroup {
                expected: 0,
                actual: 1,
            })
        );
    }

    #[test]
    fn group_rejects_duplicate_names_and_nonfinal_rest() {
        assert_eq!(
            AdapterParameterGroup::try_new(
                group(0),
                vec![parameter(0, "value"), parameter(1, "value")],
            ),
            Err(AdapterCallableModelError::DuplicateParameterName {
                group: 0,
                name: "value".to_owned(),
            })
        );
        let rest = AdapterFunctionParam::try_new(
            index(0),
            Some(AdapterCallableName::try_new("values").unwrap()),
            AdapterTypeKind::String,
            AdapterParameterPassing::RestPositional,
            AdapterParameterPresence::Required,
        )
        .unwrap();
        assert_eq!(
            AdapterParameterGroup::try_new(group(0), vec![rest, parameter(1, "tail")]),
            Err(AdapterCallableModelError::RestNotFinal {
                group: 0,
                parameter: 0,
            })
        );
    }

    #[test]
    fn callable_name_rejects_display_path_separators() {
        assert!(matches!(
            AdapterCallableName::try_new("network.fetch"),
            Err(AdapterCallableModelError::Separator { separator: '.', .. })
        ));
        assert_eq!(
            AdapterCallablePath::try_new(Vec::<AdapterCallableName>::new()),
            Err(AdapterCallableModelError::EmptyPath)
        );
    }
}
