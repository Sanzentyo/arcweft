use std::collections::BTreeSet;

use crate::{
    Coordinate, DeclarationDigest, DefaultFingerprint, DefinitionId, GenerationId, Parameter,
    Passing, Presence, Signature, SignatureError, SignatureFingerprint, TypeLayoutHash, ValueDigest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipClass {
    Unrestricted,
    Affine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ownership {
    Unrestricted,
    Affine(u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedValue {
    pub ty: TypeLayoutHash,
    pub digest: ValueDigest,
    pub ownership: Ownership,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedRestEntry {
    pub name: String,
    pub value: CheckedValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArgumentValue {
    Explicit(CheckedValue),
    Defaulted {
        default: DefaultFingerprint,
        value: CheckedValue,
    },
    OmittedOptional,
    RestPositional {
        item_ty: TypeLayoutHash,
        items: Vec<CheckedValue>,
    },
    RestNamed {
        value_ty: TypeLayoutHash,
        entries: Vec<NamedRestEntry>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentProduct {
    pub definition: DefinitionId,
    pub declaration: DeclarationDigest,
    pub generation: GenerationId,
    pub signature: SignatureFingerprint,
    pub completed_groups: u16,
    pub coordinates: Vec<Coordinate>,
    pub values: Vec<ArgumentValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArgumentError {
    InvalidSignature(SignatureError),
    WrongDefinition,
    ForeignDeclaration,
    StaleGeneration,
    SignatureMismatch,
    CompletedGroupsOutOfRange,
    CoordinateValueLengthMismatch,
    DuplicateCoordinate(Coordinate),
    OutOfOrderCoordinate {
        previous: Coordinate,
        actual: Coordinate,
    },
    MissingCoordinate(Coordinate),
    UnknownCoordinate(Coordinate),
    IllegalDisposition(Coordinate),
    DefaultFingerprintMismatch(Coordinate),
    TypeMismatch(Coordinate),
    MalformedPositionalRest(Coordinate),
    DuplicateNamedRestEntry {
        coordinate: Coordinate,
        name: String,
    },
    OutOfOrderNamedRestEntry(Coordinate),
    DuplicateAffineToken(u64),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveGenerations {
    generations: BTreeSet<GenerationId>,
}

impl LiveGenerations {
    #[must_use]
    pub fn with(generations: impl IntoIterator<Item = GenerationId>) -> Self {
        Self {
            generations: generations.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn contains(&self, generation: GenerationId) -> bool {
        self.generations.contains(&generation)
    }
}

impl ArgumentProduct {
    #[must_use]
    pub fn empty(signature: &Signature, generation: GenerationId) -> Self {
        Self {
            definition: signature.definition,
            declaration: signature.declaration,
            generation,
            signature: signature.fingerprint,
            completed_groups: 0,
            coordinates: Vec::new(),
            values: Vec::new(),
        }
    }

    pub fn validate_prefix(
        &self,
        signature: &Signature,
        live: &LiveGenerations,
    ) -> Result<OwnershipClass, ArgumentError> {
        signature
            .validate()
            .map_err(ArgumentError::InvalidSignature)?;
        if self.definition != signature.definition {
            return Err(ArgumentError::WrongDefinition);
        }
        if self.declaration != signature.declaration {
            return Err(ArgumentError::ForeignDeclaration);
        }
        if self.signature != signature.fingerprint {
            return Err(ArgumentError::SignatureMismatch);
        }
        if !live.contains(self.generation) {
            return Err(ArgumentError::StaleGeneration);
        }
        if self.completed_groups > signature.group_count() {
            return Err(ArgumentError::CompletedGroupsOutOfRange);
        }
        if self.coordinates.len() != self.values.len() {
            return Err(ArgumentError::CoordinateValueLengthMismatch);
        }
        validate_coordinate_order(&self.coordinates)?;

        let expected = signature.expected_coordinates(self.completed_groups);
        for (index, expected_coordinate) in expected.iter().copied().enumerate() {
            let Some(actual) = self.coordinates.get(index).copied() else {
                return Err(ArgumentError::MissingCoordinate(expected_coordinate));
            };
            if actual != expected_coordinate {
                if expected.contains(&actual) {
                    return Err(ArgumentError::MissingCoordinate(expected_coordinate));
                }
                return Err(ArgumentError::UnknownCoordinate(actual));
            }
        }
        if let Some(extra) = self.coordinates.get(expected.len()).copied() {
            return Err(ArgumentError::UnknownCoordinate(extra));
        }

        let mut affine_tokens = BTreeSet::new();
        let mut ownership = OwnershipClass::Unrestricted;
        for (coordinate, value) in self
            .coordinates
            .iter()
            .copied()
            .zip(self.values.iter())
        {
            let parameter = signature
                .parameter(coordinate)
                .ok_or(ArgumentError::UnknownCoordinate(coordinate))?;
            validate_argument_value(parameter, value, &mut affine_tokens)?;
            if argument_is_affine(value) {
                ownership = OwnershipClass::Affine;
            }
        }
        Ok(ownership)
    }

    pub fn validate_complete(
        &self,
        signature: &Signature,
        live: &LiveGenerations,
    ) -> Result<OwnershipClass, ArgumentError> {
        if self.completed_groups != signature.group_count() {
            let expected = signature
                .expected_coordinates(signature.group_count())
                .into_iter()
                .find(|coordinate| !self.coordinates.contains(coordinate))
                .unwrap_or(Coordinate::new(self.completed_groups, 0));
            return Err(ArgumentError::MissingCoordinate(expected));
        }
        self.validate_prefix(signature, live)
    }
}

fn validate_coordinate_order(coordinates: &[Coordinate]) -> Result<(), ArgumentError> {
    for window in coordinates.windows(2) {
        let previous = window[0];
        let actual = window[1];
        if previous == actual {
            return Err(ArgumentError::DuplicateCoordinate(actual));
        }
        if previous > actual {
            return Err(ArgumentError::OutOfOrderCoordinate { previous, actual });
        }
    }
    Ok(())
}

fn validate_argument_value(
    parameter: &Parameter,
    value: &ArgumentValue,
    affine_tokens: &mut BTreeSet<u64>,
) -> Result<(), ArgumentError> {
    let coordinate = parameter.coordinate;
    match (parameter.passing, parameter.presence, value) {
        (
            Passing::PositionalOnly | Passing::PositionalOrNamed | Passing::NamedOnly,
            Presence::Required,
            ArgumentValue::Explicit(value),
        )
        | (
            Passing::PositionalOnly | Passing::PositionalOrNamed | Passing::NamedOnly,
            Presence::Optional,
            ArgumentValue::Explicit(value),
        ) => validate_checked_value(coordinate, parameter.ty, value, affine_tokens),
        (
            Passing::PositionalOnly | Passing::PositionalOrNamed | Passing::NamedOnly,
            Presence::Optional,
            ArgumentValue::OmittedOptional,
        ) => Ok(()),
        (
            Passing::PositionalOnly | Passing::PositionalOrNamed | Passing::NamedOnly,
            Presence::Defaulted(_expected_default),
            ArgumentValue::Explicit(value),
        ) => validate_checked_value(coordinate, parameter.ty, value, affine_tokens),
        (
            Passing::PositionalOnly | Passing::PositionalOrNamed | Passing::NamedOnly,
            Presence::Defaulted(expected_default),
            ArgumentValue::Defaulted { default, value },
        ) => {
            if *default != expected_default {
                return Err(ArgumentError::DefaultFingerprintMismatch(coordinate));
            }
            validate_checked_value(coordinate, parameter.ty, value, affine_tokens)
        }
        (
            Passing::RestPositional,
            Presence::Required,
            ArgumentValue::RestPositional { item_ty, items },
        ) => {
            if *item_ty != parameter.ty {
                return Err(ArgumentError::MalformedPositionalRest(coordinate));
            }
            for item in items {
                validate_checked_value(coordinate, parameter.ty, item, affine_tokens)?;
            }
            Ok(())
        }
        (
            Passing::RestNamed,
            Presence::Required,
            ArgumentValue::RestNamed { value_ty, entries },
        ) => {
            if *value_ty != parameter.ty {
                return Err(ArgumentError::TypeMismatch(coordinate));
            }
            for window in entries.windows(2) {
                let previous = window[0].name.as_bytes();
                let actual = window[1].name.as_bytes();
                if previous == actual {
                    return Err(ArgumentError::DuplicateNamedRestEntry {
                        coordinate,
                        name: window[1].name.clone(),
                    });
                }
                if previous > actual {
                    return Err(ArgumentError::OutOfOrderNamedRestEntry(coordinate));
                }
            }
            for entry in entries {
                validate_checked_value(coordinate, parameter.ty, &entry.value, affine_tokens)?;
            }
            Ok(())
        }
        _ => Err(ArgumentError::IllegalDisposition(coordinate)),
    }
}

fn validate_checked_value(
    coordinate: Coordinate,
    expected: TypeLayoutHash,
    value: &CheckedValue,
    affine_tokens: &mut BTreeSet<u64>,
) -> Result<(), ArgumentError> {
    if value.ty != expected {
        return Err(ArgumentError::TypeMismatch(coordinate));
    }
    if let Ownership::Affine(token) = value.ownership
        && !affine_tokens.insert(token)
    {
        return Err(ArgumentError::DuplicateAffineToken(token));
    }
    Ok(())
}

fn argument_is_affine(value: &ArgumentValue) -> bool {
    match value {
        ArgumentValue::Explicit(value) | ArgumentValue::Defaulted { value, .. } => {
            matches!(value.ownership, Ownership::Affine(_))
        }
        ArgumentValue::OmittedOptional => false,
        ArgumentValue::RestPositional { items, .. } => items
            .iter()
            .any(|value| matches!(value.ownership, Ownership::Affine(_))),
        ArgumentValue::RestNamed { entries, .. } => entries
            .iter()
            .any(|entry| matches!(entry.value.ownership, Ownership::Affine(_))),
    }
}
