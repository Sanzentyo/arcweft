use std::collections::BTreeSet;

use crate::{
    Coordinate, DeclarationDigest, DefaultFingerprint, DefinitionId, GroupIndex,
    SignatureFingerprint, TypeLayoutHash,
};

pub const MAX_GROUPS: usize = 16;
pub const MAX_PARAMETERS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupKind {
    Initial,
    Curried,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Passing {
    PositionalOnly,
    PositionalOrNamed,
    NamedOnly,
    RestPositional,
    RestNamed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Presence {
    Required,
    Optional,
    Defaulted(DefaultFingerprint),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub coordinate: Coordinate,
    pub name: Option<String>,
    pub passing: Passing,
    pub presence: Presence,
    pub ty: TypeLayoutHash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Group {
    pub index: GroupIndex,
    pub kind: GroupKind,
    pub parameters: Vec<Parameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature {
    pub definition: DefinitionId,
    pub declaration: DeclarationDigest,
    pub fingerprint: SignatureFingerprint,
    pub groups: Vec<Group>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureError {
    MissingGroups,
    TooManyGroups,
    TooManyParameters,
    WrongGroupIndex {
        expected: u16,
        actual: u16,
    },
    WrongGroupKind {
        group: u16,
    },
    WrongParameterIndex {
        group: u16,
        expected: u16,
        actual: u16,
    },
    DuplicateName {
        group: u16,
        name: String,
    },
    DuplicateRest {
        group: u16,
        passing: Passing,
    },
    IllegalRestPresence {
        coordinate: Coordinate,
    },
}

impl Signature {
    pub fn validate(&self) -> Result<(), SignatureError> {
        if self.groups.is_empty() {
            return Err(SignatureError::MissingGroups);
        }
        if self.groups.len() > MAX_GROUPS {
            return Err(SignatureError::TooManyGroups);
        }

        let mut total_parameters = 0usize;
        for (group_position, group) in self.groups.iter().enumerate() {
            let expected_group = u16::try_from(group_position).expect("group limit fits u16");
            if group.index.0 != expected_group {
                return Err(SignatureError::WrongGroupIndex {
                    expected: expected_group,
                    actual: group.index.0,
                });
            }
            let expected_kind = if group_position == 0 {
                GroupKind::Initial
            } else {
                GroupKind::Curried
            };
            if group.kind != expected_kind {
                return Err(SignatureError::WrongGroupKind {
                    group: expected_group,
                });
            }

            total_parameters = total_parameters
                .checked_add(group.parameters.len())
                .ok_or(SignatureError::TooManyParameters)?;
            if total_parameters > MAX_PARAMETERS {
                return Err(SignatureError::TooManyParameters);
            }

            let mut names = BTreeSet::new();
            let mut positional_rest = false;
            let mut named_rest = false;
            for (parameter_position, parameter) in group.parameters.iter().enumerate() {
                let expected_parameter =
                    u16::try_from(parameter_position).expect("parameter limit fits u16");
                if parameter.coordinate.group.0 != expected_group
                    || parameter.coordinate.parameter.0 != expected_parameter
                {
                    return Err(SignatureError::WrongParameterIndex {
                        group: expected_group,
                        expected: expected_parameter,
                        actual: parameter.coordinate.parameter.0,
                    });
                }
                if let Some(name) = &parameter.name
                    && !names.insert(name.clone())
                {
                    return Err(SignatureError::DuplicateName {
                        group: expected_group,
                        name: name.clone(),
                    });
                }
                match parameter.passing {
                    Passing::RestPositional => {
                        if positional_rest {
                            return Err(SignatureError::DuplicateRest {
                                group: expected_group,
                                passing: Passing::RestPositional,
                            });
                        }
                        positional_rest = true;
                        if parameter.presence != Presence::Required {
                            return Err(SignatureError::IllegalRestPresence {
                                coordinate: parameter.coordinate,
                            });
                        }
                    }
                    Passing::RestNamed => {
                        if named_rest {
                            return Err(SignatureError::DuplicateRest {
                                group: expected_group,
                                passing: Passing::RestNamed,
                            });
                        }
                        named_rest = true;
                        if parameter.presence != Presence::Required {
                            return Err(SignatureError::IllegalRestPresence {
                                coordinate: parameter.coordinate,
                            });
                        }
                    }
                    Passing::PositionalOnly
                    | Passing::PositionalOrNamed
                    | Passing::NamedOnly => {}
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn group_count(&self) -> u16 {
        u16::try_from(self.groups.len()).expect("validated group count fits u16")
    }

    #[must_use]
    pub fn expected_coordinates(&self, completed_groups: u16) -> Vec<Coordinate> {
        self.groups
            .iter()
            .take(usize::from(completed_groups))
            .flat_map(|group| group.parameters.iter().map(|parameter| parameter.coordinate))
            .collect()
    }

    #[must_use]
    pub fn parameter(&self, coordinate: Coordinate) -> Option<&Parameter> {
        self.groups
            .get(usize::from(coordinate.group.0))?
            .parameters
            .get(usize::from(coordinate.parameter.0))
            .filter(|parameter| parameter.coordinate == coordinate)
    }
}
