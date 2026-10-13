//! Initialized place storage, separate from complete runtime value carriers.
//!
//! A partial record retains its immutable header and the remaining child
//! owners. Vacant children are never encoded as Unit or malformed values.

use serde::{Deserialize, Serialize};

use super::{RuntimeNominalRecordValue, RuntimeRecordFieldId, RuntimeValue};
use crate::{
    entry::{RuntimeNominalTypeId, TypeLayoutHash},
    pattern::RuntimeSemanticTypeId,
};

/// Storage of one declaration or frame register. The same tree is mapped to
/// inert value snapshots at persistence boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePlaceStorage<T> {
    state: PlaceState<T>,
}

impl Serialize for RuntimePlaceStorage<super::AwbcRuntimeValueSnapshot> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.state.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuntimePlaceStorage<super::AwbcRuntimeValueSnapshot> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(PlaceState::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl Serialize for RuntimePlaceStorage<RuntimeValue> {
    fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom(
            "live place storage requires a program-bound snapshot",
        ))
    }
}

impl<'de> Deserialize<'de> for RuntimePlaceStorage<RuntimeValue> {
    fn deserialize<D: serde::Deserializer<'de>>(_deserializer: D) -> Result<Self, D::Error> {
        Err(serde::de::Error::custom(
            "live place storage requires program-bound restore",
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(
    serialize = "T: Serialize, RuntimePlaceStorage<T>: Serialize",
    deserialize = "T: Deserialize<'de>, RuntimePlaceStorage<T>: Deserialize<'de>"
))]
enum PlaceState<T> {
    Vacant,
    Initialized(T),
    Record {
        header: RecordHeader,
        fields: Vec<RuntimePlaceStorage<T>>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) enum RecordHeader {
    Nominal {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
        type_instantiation:
            Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
    },
    Structural {
        names: Vec<String>,
    },
}

impl<T> Default for RuntimePlaceStorage<T> {
    fn default() -> Self {
        Self {
            state: PlaceState::Vacant,
        }
    }
}

impl<T> From<T> for RuntimePlaceStorage<T> {
    fn from(value: T) -> Self {
        Self {
            state: PlaceState::Initialized(value),
        }
    }
}

impl<T> RuntimePlaceStorage<T> {
    /// Resolves every retained nominal header before activating any child.
    /// The same preflight applies to live capture and inert restore storage.
    pub(crate) fn validate_record_headers(
        &self,
        owner: Option<&crate::task::RuntimeProgramOwner>,
    ) -> Result<(), String> {
        let limits = crate::entry::RuntimeSchemaLimits::engine_default();
        let mut pending = vec![(self, 0usize)];
        let mut nodes = 0usize;
        while let Some((storage, depth)) = pending.pop() {
            nodes += 1;
            if !limits.permits_depth(depth) || !limits.permits_nodes(nodes) {
                return Err("partial record header traversal exceeds its work limits".to_owned());
            }
            if let Some((header, fields)) = storage.record_parts() {
                let scheduled_nodes = nodes
                    .checked_add(pending.len())
                    .and_then(|count| count.checked_add(fields.len()));
                if !scheduled_nodes.is_some_and(|count| limits.permits_nodes(count)) {
                    return Err(
                        "partial record header traversal exceeds its work limits".to_owned()
                    );
                }
                if let RecordHeader::Nominal {
                    nominal,
                    semantic_identity,
                    layout,
                    type_instantiation,
                } = header
                {
                    owner
                        .ok_or_else(|| {
                            "nominal place storage requires a program-bound snapshot".to_owned()
                        })?
                        .types()
                        .require_nominal_record_snapshot_header(
                            *semantic_identity,
                            nominal,
                            *layout,
                            fields.len(),
                            type_instantiation.as_deref(),
                        )
                        .map_err(|error| error.to_string())?;
                }
                pending.extend(fields.iter().rev().map(|field| (field, depth + 1)));
            }
        }
        Ok(())
    }

    pub(crate) fn record_parts(&self) -> Option<(&RecordHeader, &[Self])> {
        match &self.state {
            PlaceState::Record { header, fields } => Some((header, fields)),
            _ => None,
        }
    }
    pub fn as_ref(&self) -> Option<&T> {
        match &self.state {
            PlaceState::Initialized(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_mut(&mut self) -> Option<&mut T> {
        match &mut self.state {
            PlaceState::Initialized(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_vacant(&self) -> bool {
        matches!(self.state, PlaceState::Vacant)
    }

    pub fn take(&mut self) -> Option<T> {
        if self.as_ref().is_none() {
            return None;
        }
        match std::mem::take(&mut self.state) {
            PlaceState::Initialized(value) => Some(value),
            _ => unreachable!("preflight requires an initialized complete value"),
        }
    }

    /// Visits every remaining complete child, including owners in partial
    /// records. Ownership/cleanup must use this inventory, not `as_ref`.
    pub fn values(&self) -> impl Iterator<Item = &T> {
        let mut current = Some(self);
        let mut stack = Vec::new();
        std::iter::from_fn(move || {
            while let Some(storage) = current.take().or_else(|| stack.pop()) {
                match &storage.state {
                    PlaceState::Initialized(value) => return Some(value),
                    PlaceState::Record { fields, .. } => stack.extend(fields.iter().rev()),
                    PlaceState::Vacant => {}
                }
            }
            None
        })
    }

    pub fn into_values(self) -> Vec<T> {
        let mut values = Vec::new();
        let mut current = Some(self);
        let mut stack = Vec::new();
        while let Some(storage) = current.take().or_else(|| stack.pop()) {
            match storage.state {
                PlaceState::Initialized(value) => values.push(value),
                PlaceState::Record { fields, .. } => stack.extend(fields.into_iter().rev()),
                PlaceState::Vacant => {}
            }
        }
        values
    }

    pub(crate) fn try_map<U, E>(
        self,
        mapper: &mut impl FnMut(T) -> Result<U, E>,
    ) -> Result<RuntimePlaceStorage<U>, E> {
        Ok(RuntimePlaceStorage {
            state: match self.state {
                PlaceState::Vacant => PlaceState::Vacant,
                PlaceState::Initialized(value) => PlaceState::Initialized(mapper(value)?),
                PlaceState::Record { header, fields } => PlaceState::Record {
                    header,
                    fields: fields
                        .into_iter()
                        .map(|field| field.try_map(mapper))
                        .collect::<Result<_, _>>()?,
                },
            },
        })
    }

    pub(crate) fn try_map_ref<U, E>(
        &self,
        mapper: &mut impl FnMut(&T) -> Result<U, E>,
    ) -> Result<RuntimePlaceStorage<U>, E> {
        Ok(RuntimePlaceStorage {
            state: match &self.state {
                PlaceState::Vacant => PlaceState::Vacant,
                PlaceState::Initialized(value) => PlaceState::Initialized(mapper(value)?),
                PlaceState::Record { header, fields } => PlaceState::Record {
                    header: header.clone(),
                    fields: fields
                        .iter()
                        .map(|field| field.try_map_ref(mapper))
                        .collect::<Result<_, _>>()?,
                },
            },
        })
    }
}

impl<T> TryFrom<PlaceState<T>> for RuntimePlaceStorage<T> {
    type Error = &'static str;
    fn try_from(state: PlaceState<T>) -> Result<Self, Self::Error> {
        if let PlaceState::Record { header, fields } = &state {
            if fields.is_empty() || fields.iter().all(|field| field.as_ref().is_some()) {
                return Err(
                    "partial record must retain a nonempty header and an uninitialized child",
                );
            }
            if let RecordHeader::Structural { names } = header {
                let unique = names.iter().collect::<std::collections::BTreeSet<_>>();
                if names.len() != fields.len()
                    || unique.len() != names.len()
                    || names.iter().any(String::is_empty)
                {
                    return Err("partial structural record has an invalid field inventory");
                }
            }
        }
        Ok(Self { state })
    }
}

impl<T> Default for PlaceState<T> {
    fn default() -> Self {
        Self::Vacant
    }
}

impl RuntimePlaceStorage<RuntimeValue> {
    pub(crate) fn snapshot_for_owner(
        &self,
        owner: Option<&crate::task::RuntimeProgramOwner>,
    ) -> Result<
        RuntimePlaceStorage<super::AwbcRuntimeValueSnapshot>,
        super::AwbcRuntimeValueSnapshotError,
    > {
        self.validate_record_headers(owner)
            .map_err(|message| super::AwbcRuntimeValueSnapshotError::Message { message })?;
        self.try_map_ref(&mut |value| match owner {
            Some(owner) => {
                super::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value, owner)
            }
            None => super::AwbcRuntimeValueSnapshot::from_standalone_value(value),
        })
    }

    /// Checks the sealed cleanup contour against drop flags. A record header
    /// counts as an initialized container even when a child has been moved.
    /// This is artifact/state validation; source access legality is static.
    pub(crate) fn matches_displacement(
        &self,
        target: &[RuntimeRecordFieldId],
        displacement: &super::RuntimePlaceDisplacement<RuntimeRecordFieldId>,
    ) -> bool {
        use super::{RuntimePlaceDisplacement, RuntimePlaceInitialization};
        let RuntimePlaceDisplacement::Reachable {
            initialization,
            fields,
        } = displacement
        else {
            return false;
        };
        let accepts = |expected, actual| match (expected, actual) {
            (RuntimePlaceInitialization::Conditional, Some(_)) => true,
            (RuntimePlaceInitialization::Initialized, Some(true))
            | (RuntimePlaceInitialization::Uninitialized, Some(false)) => true,
            _ => false,
        };
        if !accepts(*initialization, self.container_initialized_at(target)) {
            return false;
        }
        for field in fields {
            if field.fields.is_empty() {
                return false;
            }
            let path: Vec<_> = target.iter().chain(field.fields.iter()).copied().collect();
            if !accepts(field.initialization, self.container_initialized_at(&path)) {
                return false;
            }
        }
        // Conditional containers use their persisted drop flags. A definite
        // initialized container must account for every absent child owner.
        *initialization != RuntimePlaceInitialization::Initialized
            || self.displacement_holes_covered(target, fields)
    }

    fn container_initialized_at(&self, path: &[RuntimeRecordFieldId]) -> Option<bool> {
        let mut storage = self;
        let mut path = path;
        loop {
            match &storage.state {
                PlaceState::Vacant => return Some(false),
                PlaceState::Record { fields, .. } => match path.split_first() {
                    None => return Some(true),
                    Some((field, rest)) => {
                        storage = fields.get(field.zero_based() as usize)?;
                        path = rest;
                    }
                },
                PlaceState::Initialized(value) => {
                    let mut value = value;
                    for field in path {
                        value = value.record_field(*field)?;
                    }
                    return Some(true);
                }
            }
        }
    }

    fn displacement_holes_covered(
        &self,
        target: &[RuntimeRecordFieldId],
        contour: &[super::RuntimeDisplacedField<RuntimeRecordFieldId>],
    ) -> bool {
        if self.record_parts().is_none() {
            return true;
        }
        enum Visit<'a> {
            Storage(
                &'a RuntimePlaceStorage<RuntimeValue>,
                Option<RuntimeRecordFieldId>,
            ),
            Leave,
        }
        let mut path = Vec::new();
        let mut pending = vec![Visit::Storage(self, None)];
        while let Some(visit) = pending.pop() {
            let Visit::Storage(storage, ordinal) = visit else {
                path.pop();
                continue;
            };
            if let Some(ordinal) = ordinal {
                path.push(ordinal);
                pending.push(Visit::Leave);
            }
            match &storage.state {
                PlaceState::Vacant => {
                    if path.starts_with(target)
                        && !contour.iter().any(|field| {
                            field.initialization != super::RuntimePlaceInitialization::Initialized
                                && path[target.len()..].starts_with(&field.fields)
                        })
                    {
                        return false;
                    }
                }
                PlaceState::Initialized(_) => {}
                PlaceState::Record { fields, .. } => {
                    for (ordinal, child) in fields.iter().enumerate().rev() {
                        pending.push(Visit::Storage(
                            child,
                            Some(
                                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                                    .expect("admitted record field ordinal"),
                            ),
                        ));
                    }
                }
            }
        }
        true
    }

    pub(crate) fn field(&self, path: &[RuntimeRecordFieldId]) -> Option<&RuntimeValue> {
        let Some((first, rest)) = path.split_first() else {
            return self.as_ref();
        };
        match &self.state {
            PlaceState::Initialized(value) => {
                let mut value = value;
                for field in path {
                    value = value.record_field(*field)?;
                }
                Some(value)
            }
            PlaceState::Record { fields, .. } => {
                fields.get(first.zero_based() as usize)?.field(rest)
            }
            PlaceState::Vacant => None,
        }
    }

    /// Moves one initialized child. Preflight leaves storage intact on failure.
    pub(crate) fn take_field(&mut self, path: &[RuntimeRecordFieldId]) -> Option<RuntimeValue> {
        self.field(path)?;
        if path.is_empty() {
            return self.take();
        }
        self.split_record()?;
        let PlaceState::Record { fields, .. } = &mut self.state else {
            return None;
        };
        let value = fields
            .get_mut(path[0].zero_based() as usize)?
            .take_field(&path[1..]);
        self.collapse_record();
        value
    }

    /// Assigns a child of an existing aggregate. Its previous value may have
    /// been moved; an absent parent cannot fabricate an aggregate header.
    pub(crate) fn assign_field(
        &mut self,
        path: &[RuntimeRecordFieldId],
        value: RuntimeValue,
    ) -> Result<Vec<RuntimeValue>, RuntimeValue> {
        let Some((first, rest)) = path.split_first() else {
            return Err(value);
        };
        if self.values_at(path).is_none() {
            return Err(value);
        }
        if let Some(target) = self.field_mut(path) {
            return Ok(vec![std::mem::replace(target, value)]);
        }
        if self.split_record().is_none() {
            return Err(value);
        }
        let PlaceState::Record { fields, .. } = &mut self.state else {
            return Err(value);
        };
        let Some(child) = fields.get_mut(first.zero_based() as usize) else {
            return Err(value);
        };
        let result = if rest.is_empty() {
            let previous = std::mem::take(child).into_values();
            *child = value.into();
            Ok(previous)
        } else {
            child.assign_field(rest, value)
        };
        self.collapse_record();
        result
    }

    pub(crate) fn replace(&mut self, value: RuntimeValue) -> Vec<RuntimeValue> {
        std::mem::replace(self, value.into()).into_values()
    }

    pub(crate) fn values_at(&self, path: &[RuntimeRecordFieldId]) -> Option<Vec<&RuntimeValue>> {
        if path.is_empty() {
            return Some(self.values().collect());
        }
        match &self.state {
            PlaceState::Record { fields, .. } => fields
                .get(path[0].zero_based() as usize)?
                .values_at(&path[1..]),
            PlaceState::Initialized(_) => self.field(path).map(|value| vec![value]),
            PlaceState::Vacant => None,
        }
    }

    pub(crate) fn field_mut(&mut self, path: &[RuntimeRecordFieldId]) -> Option<&mut RuntimeValue> {
        if path.is_empty() {
            return self.as_mut();
        }
        match &mut self.state {
            PlaceState::Initialized(value) => {
                let mut value = value;
                for field in path {
                    value = match value {
                        RuntimeValue::NominalRecord(record) => record.field_mut(*field)?,
                        RuntimeValue::Record(record) => record.field_value_mut(*field)?,
                        _ => return None,
                    };
                }
                Some(value)
            }
            PlaceState::Record { fields, .. } => fields
                .get_mut(path[0].zero_based() as usize)?
                .field_mut(&path[1..]),
            PlaceState::Vacant => None,
        }
    }

    fn split_record(&mut self) -> Option<()> {
        if matches!(self.state, PlaceState::Record { .. }) {
            return Some(());
        }
        match self.as_ref()? {
            RuntimeValue::NominalRecord(_) | RuntimeValue::Record(_) => {}
            _ => return None,
        }
        let value = self.take()?;
        let (header, fields) = match value {
            RuntimeValue::NominalRecord(record) => {
                let header = RecordHeader::Nominal {
                    nominal: record.type_id().clone(),
                    semantic_identity: record.semantic_identity(),
                    layout: record.layout(),
                    type_instantiation: record
                        .type_instantiation()
                        .cloned()
                        .map(std::sync::Arc::new),
                };
                (header, record.into_fields())
            }
            RuntimeValue::Record(record) => {
                let names = record
                    .fields()
                    .iter()
                    .map(|field| field.name().to_owned())
                    .collect();
                (
                    RecordHeader::Structural { names },
                    record.into_iter().map(|field| field.into_value()).collect(),
                )
            }
            _ => unreachable!("preflight admits record representations only"),
        };
        self.state = PlaceState::Record {
            header,
            fields: fields.into_iter().map(Self::from).collect(),
        };
        Some(())
    }

    fn collapse_record(&mut self) {
        let PlaceState::Record { fields, .. } = &self.state else {
            return;
        };
        if !fields.iter().all(|field| field.as_ref().is_some()) {
            return;
        }
        let PlaceState::Record { header, fields } = std::mem::take(&mut self.state) else {
            unreachable!()
        };
        let fields = fields
            .into_iter()
            .map(|mut field| field.take().expect("complete child"))
            .collect();
        let value = match header {
            RecordHeader::Nominal {
                nominal,
                semantic_identity,
                layout,
                type_instantiation,
            } => RuntimeValue::NominalRecord(
                RuntimeNominalRecordValue::new(nominal, semantic_identity, layout, fields)
                    .with_type_instantiation(type_instantiation),
            ),
            RecordHeader::Structural { names } => {
                RuntimeValue::try_record(names.into_iter().zip(fields).collect())
                    .expect("header retained from an admitted record")
            }
        };
        self.state = PlaceState::Initialized(value);
    }
}

impl RuntimePlaceStorage<super::AwbcRuntimeValueSnapshot> {
    pub(crate) fn into_live_for_program(
        self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<RuntimePlaceStorage<RuntimeValue>, super::AwbcRuntimeValueSnapshotError> {
        self.validate_record_headers(Some(owner))
            .map_err(|message| super::AwbcRuntimeValueSnapshotError::Message { message })?;
        self.try_map(&mut |value| value.into_runtime_value_for_program(owner))
    }
}

#[cfg(test)]
mod tests {
    use super::super::AwbcRuntimeValueSnapshot;
    use super::*;

    fn standalone_snapshot(
        storage: &RuntimePlaceStorage<RuntimeValue>,
    ) -> RuntimePlaceStorage<AwbcRuntimeValueSnapshot> {
        storage
            .try_map_ref(&mut AwbcRuntimeValueSnapshot::from_standalone_value)
            .unwrap()
    }

    fn empty_owner() -> crate::task::RuntimeProgramOwner {
        crate::task::RuntimeProgramOwner::Awbc(std::sync::Arc::new(
            crate::awbc::schema::AwbcProgram::default(),
        ))
    }

    fn field(ordinal: usize) -> RuntimeRecordFieldId {
        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap()
    }

    #[test]
    fn borrowed_place_keeps_original_local_type_id_and_rejects_ambiguous_roles() {
        use crate::awbc::schema::{AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcTypeId};
        use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
        use crate::program_types::{
            RuntimeProgramTypeError, RuntimeProgramTypes, RuntimeProgramValueType,
        };
        let semantic = RuntimeSemanticTypeId::from_bytes([0x91; 32]);
        // A malformed inventory cannot redirect a known local ID through its
        // first semantic match. Semantic-role lookup must report ambiguity.
        let program = AwbcProgram {
            runtime_types: vec![
                AwbcRuntimeType::new(semantic, AwbcRuntimeTypeShape::Bool),
                AwbcRuntimeType::new(semantic, AwbcRuntimeTypeShape::Unit),
            ],
            ..AwbcProgram::default()
        };
        assert!(matches!(
            RuntimeProgramTypes::Awbc(&program).value_type(semantic),
            Err(RuntimeProgramTypeError::Ambiguous { semantic_type }) if semantic_type == semantic
        ));
        let saved = RuntimePlaceStorage::from(AwbcRuntimeValueSnapshot::Bool(true));
        let admit = |ty| {
            saved.validate_program_type(
                RuntimeProgramValueType::Awbc {
                    program: &program,
                    ty,
                },
                None,
                &mut ValueResourceAdmission::snapshot(
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                    ValueAdmissionOrigin::InertSnapshot,
                ),
                AwbcRuntimeValueSnapshot::view,
            )
        };
        admit(AwbcTypeId(0)).unwrap();
        assert!(matches!(
            admit(AwbcTypeId(1)),
            Err(RuntimeProgramTypeError::AwbcValue(source))
                if matches!(&*source, crate::awbc::value_admission::AwbcValueAdmissionError::Value {
                    ty: AwbcTypeId(1), source: crate::entry::RuntimeSchemaError::Type { .. }
                })
        ));
        assert!(admit(AwbcTypeId(999)).is_err());
        let live = RuntimePlaceStorage::from(RuntimeValue::Bool(true));
        assert!(live.matches_program_type(
            RuntimeProgramValueType::Awbc {
                program: &program,
                ty: AwbcTypeId(0)
            },
            None
        ));
        assert!(!live.matches_program_type(
            RuntimeProgramValueType::Awbc {
                program: &program,
                ty: AwbcTypeId(1)
            },
            None
        ));
        assert_eq!(saved.as_ref(), Some(&AwbcRuntimeValueSnapshot::Bool(true)));
    }

    #[test]
    fn borrowed_place_admission_accumulates_complete_value_resources() {
        use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
        use crate::entry::{RuntimeSchemaError, RuntimeSchemaLimits};
        use crate::program_types::RuntimeProgramTypeError;
        let saved = RuntimePlaceStorage::from(AwbcRuntimeValueSnapshot::Bool(true));
        let exact = RuntimeSchemaLimits {
            max_nodes: 2,
            max_encoded_bytes: 4,
            max_validation_work: 6,
            ..RuntimeSchemaLimits::engine_default()
        };
        for owner in super::super::awbc_save::tests::owners() {
            let mut admission =
                ValueResourceAdmission::snapshot(exact, ValueAdmissionOrigin::InertSnapshot);
            for _ in 0..2 {
                saved
                    .validate_program_type(
                        owner
                            .types()
                            .value_type(RuntimeSemanticTypeId::from_bytes([2; 32]))
                            .unwrap(),
                        None,
                        &mut admission,
                        AwbcRuntimeValueSnapshot::view,
                    )
                    .unwrap();
            }
            for (limits, expected) in [
                (
                    RuntimeSchemaLimits {
                        max_nodes: 1,
                        ..exact
                    },
                    RuntimeSchemaError::BudgetExceeded { budget: "nodes" },
                ),
                (
                    RuntimeSchemaLimits {
                        max_encoded_bytes: 3,
                        ..exact
                    },
                    RuntimeSchemaError::BudgetExceeded {
                        budget: "encoded_bytes",
                    },
                ),
                (
                    RuntimeSchemaLimits {
                        max_validation_work: 5,
                        ..exact
                    },
                    RuntimeSchemaError::ValidationWork {
                        path: "$".to_owned(),
                        limit: 5,
                        consumed: 6,
                    },
                ),
            ] {
                let mut admission =
                    ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::InertSnapshot);
                saved
                    .validate_program_type(
                        owner
                            .types()
                            .value_type(RuntimeSemanticTypeId::from_bytes([2; 32]))
                            .unwrap(),
                        None,
                        &mut admission,
                        AwbcRuntimeValueSnapshot::view,
                    )
                    .unwrap();
                let error = saved
                    .validate_program_type(
                        owner
                            .types()
                            .value_type(RuntimeSemanticTypeId::from_bytes([2; 32]))
                            .unwrap(),
                        None,
                        &mut admission,
                        AwbcRuntimeValueSnapshot::view,
                    )
                    .unwrap_err();
                assert_eq!(admission.work().failure(), Some(expected.clone()));
                let source = match error {
                    RuntimeProgramTypeError::Resource(source) => *source,
                    RuntimeProgramTypeError::PlanValue(source) => match *source {
                        crate::plan::RuntimePlanValueAdmissionError::Value { source, .. } => source,
                        error => panic!("unexpected plan error: {error}"),
                    },
                    RuntimeProgramTypeError::AwbcValue(source) => match *source {
                        crate::awbc::value_admission::AwbcValueAdmissionError::Value {
                            source,
                            ..
                        } => source,
                        error @ crate::awbc::value_admission::AwbcValueAdmissionError::UnknownType { .. } => panic!("unexpected AWBC error: {error}"),
                    },
                    error => panic!("unexpected admission error: {error}"),
                };
                assert_eq!(source, expected);
            }
        }
    }

    #[test]
    fn partial_place_admission_checks_header_provenance_with_no_complete_value() {
        use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
        let semantic = RuntimeSemanticTypeId::from_bytes([1; 32]);
        let limits = crate::entry::RuntimeSchemaLimits::engine_default();
        for owner in super::super::awbc_save::tests::owners() {
            let mut live = RuntimePlaceStorage::from(
                owner
                    .types()
                    .try_record_value(semantic, vec![RuntimeValue::Bool(true)], limits)
                    .unwrap(),
            );
            live.take_field(&[field(0)]).unwrap();
            let saved = live.snapshot_for_owner(Some(&owner)).unwrap();
            saved
                .validate_program_type(
                    owner.types().value_type(semantic).unwrap(),
                    None,
                    &mut ValueResourceAdmission::snapshot(
                        limits,
                        ValueAdmissionOrigin::InertSnapshot,
                    ),
                    AwbcRuntimeValueSnapshot::view,
                )
                .unwrap();
            let exact_work = crate::entry::RuntimeSchemaLimits {
                max_validation_work: 5,
                ..limits
            };
            saved
                .validate_program_type(
                    owner.types().value_type(semantic).unwrap(),
                    None,
                    &mut ValueResourceAdmission::snapshot(
                        exact_work,
                        ValueAdmissionOrigin::InertSnapshot,
                    ),
                    AwbcRuntimeValueSnapshot::view,
                )
                .unwrap();
            let error = saved
                .validate_program_type(
                    owner.types().value_type(semantic).unwrap(),
                    None,
                    &mut ValueResourceAdmission::snapshot(
                        crate::entry::RuntimeSchemaLimits {
                            max_validation_work: 4,
                            ..exact_work
                        },
                        ValueAdmissionOrigin::InertSnapshot,
                    ),
                    AwbcRuntimeValueSnapshot::view,
                )
                .unwrap_err();
            assert!(
                matches!(error, crate::program_types::RuntimeProgramTypeError::Resource(source)
                if matches!(&*source, crate::entry::RuntimeSchemaError::ValidationWork { path, limit: 4, consumed: 5 } if path == "$[0]"))
            );
            let mut forged = saved.clone();
            let PlaceState::Record {
                header:
                    RecordHeader::Nominal {
                        type_instantiation, ..
                    },
                ..
            } = &mut forged.state
            else {
                panic!("original partial nominal")
            };
            *type_instantiation = Some(std::sync::Arc::new(
                serde_json::from_value(serde_json::json!({
                    "context": semantic, "effects": []
                }))
                .unwrap(),
            ));
            assert!(
                forged
                    .validate_program_type(
                        owner.types().value_type(semantic).unwrap(),
                        None,
                        &mut ValueResourceAdmission::snapshot(
                            limits,
                            ValueAdmissionOrigin::InertSnapshot
                        ),
                        AwbcRuntimeValueSnapshot::view
                    )
                    .is_err()
            );
            assert_eq!(live.snapshot_for_owner(Some(&owner)).unwrap(), saved);
        }
    }

    #[test]
    fn partial_move_restore_and_snapshot_mapping_preserve_siblings() {
        let original = RuntimeValue::try_record(vec![
            ("a".into(), RuntimeValue::String("first".into())),
            ("b".into(), RuntimeValue::Bool(true)),
        ])
        .unwrap();
        let mut storage = RuntimePlaceStorage::from(original.clone());
        assert_eq!(
            storage.take_field(&[field(0)]),
            Some(RuntimeValue::String("first".into()))
        );
        assert!(storage.as_ref().is_none());
        assert!(storage.take().is_none());
        assert_eq!(storage.field(&[field(1)]), Some(&RuntimeValue::Bool(true)));
        assert_eq!(storage.values().count(), 1);
        assert!(serde_json::to_value(&storage).is_err());
        let encoded = serde_json::to_value(standalone_snapshot(&storage)).unwrap();
        assert!(
            serde_json::from_value::<RuntimePlaceStorage<RuntimeValue>>(encoded.clone()).is_err()
        );
        let restored: RuntimePlaceStorage<AwbcRuntimeValueSnapshot> =
            serde_json::from_value(encoded).unwrap();
        let restored = restored.into_live_for_program(&empty_owner()).unwrap();
        assert_eq!(restored, storage);
        storage = restored.try_map(&mut Ok::<_, ()>).unwrap();
        assert_eq!(
            storage.assign_field(&[field(0)], RuntimeValue::String("first".into())),
            Ok(vec![])
        );
        assert_eq!(storage.take(), Some(original));
        assert!(
            storage
                .assign_field(&[field(0)], RuntimeValue::Bool(false))
                .is_err()
        );
    }

    #[test]
    fn nested_move_keeps_header_and_remaining_cleanup_inventory() {
        let nested = RuntimeValue::try_record(vec![
            ("a".into(), RuntimeValue::Bool(true)),
            ("b".into(), RuntimeValue::Bool(false)),
        ])
        .unwrap();
        let mut storage = RuntimePlaceStorage::from(
            RuntimeValue::try_record(vec![
                ("nested".into(), nested),
                ("sibling".into(), RuntimeValue::String("owner".into())),
            ])
            .unwrap(),
        );
        assert_eq!(
            storage.take_field(&[field(0), field(0)]),
            Some(RuntimeValue::Bool(true))
        );
        let before = storage.clone();
        assert!(storage.take_field(&[field(0)]).is_none());
        assert!(storage.take_field(&[field(9)]).is_none());
        assert_eq!(storage, before);
        assert_eq!(
            storage.values().collect::<Vec<_>>(),
            vec![
                &RuntimeValue::Bool(false),
                &RuntimeValue::String("owner".into())
            ]
        );
        assert_eq!(
            storage.into_values(),
            vec![
                RuntimeValue::Bool(false),
                RuntimeValue::String("owner".into())
            ]
        );
    }

    #[test]
    fn partial_nominal_move_and_repair_preserve_the_construction_binding() {
        let binding = std::sync::Arc::new(
            serde_json::from_value::<crate::program_types::RuntimeFunctionEffectInstantiation>(
                serde_json::json!({"context": vec![7; 32], "effects": [["io.read"]]}),
            )
            .unwrap(),
        );
        let record = RuntimeNominalRecordValue::new(
            RuntimeNominalTypeId::try_new("game.Holder").unwrap(),
            RuntimeSemanticTypeId::from_bytes([3; 32]),
            TypeLayoutHash::from_bytes([5; 32]),
            vec![RuntimeValue::Bool(true)],
        )
        .with_type_instantiation(Some(binding.clone()));
        let mut storage = RuntimePlaceStorage::from(RuntimeValue::NominalRecord(record));
        let moved = storage.take_field(&[field(0)]).unwrap();
        assert!(serde_json::to_vec(&storage).is_err());
        let encoded = serde_json::to_vec(&standalone_snapshot(&storage)).unwrap();
        let restored: RuntimePlaceStorage<AwbcRuntimeValueSnapshot> =
            serde_json::from_slice(&encoded).unwrap();
        assert!(
            restored
                .clone()
                .into_live_for_program(&empty_owner())
                .is_err()
        );
        let mut restored = restored
            .try_map(&mut |value| value.into_runtime_value_for_program(&empty_owner()))
            .unwrap();
        let Some((
            RecordHeader::Nominal {
                type_instantiation, ..
            },
            _,
        )) = restored.record_parts()
        else {
            panic!("partial nominal header")
        };
        assert_eq!(type_instantiation.as_deref(), Some(binding.as_ref()));
        assert!(
            restored
                .assign_field(&[field(0)], moved)
                .unwrap()
                .is_empty()
        );
        let RuntimeValue::NominalRecord(record) = restored.take().unwrap() else {
            panic!("repaired nominal value")
        };
        assert_eq!(record.type_instantiation(), Some(binding.as_ref()));
        assert_eq!(record.fields(), [RuntimeValue::Bool(true)]);
    }

    #[test]
    fn malformed_partial_record_inventory_is_rejected_during_decode() {
        let mut storage = RuntimePlaceStorage::from(
            RuntimeValue::try_record(vec![
                ("first".into(), RuntimeValue::Bool(true)),
                ("second".into(), RuntimeValue::Bool(false)),
            ])
            .unwrap(),
        );
        storage.take_field(&[field(0)]).unwrap();
        let valid = serde_json::to_value(standalone_snapshot(&storage)).unwrap();
        for malformed in [
            {
                let mut value = valid.clone();
                value["Record"]["header"]["Structural"]["names"][1] = serde_json::json!("first");
                value
            },
            {
                let mut value = valid.clone();
                value["Record"]["fields"] = serde_json::json!([]);
                value
            },
            {
                let mut value = valid.clone();
                value["Record"]["fields"][0] = value["Record"]["fields"][1].clone();
                value
            },
        ] {
            assert!(
                serde_json::from_value::<RuntimePlaceStorage<AwbcRuntimeValueSnapshot>>(malformed)
                    .is_err()
            );
        }
    }

    #[test]
    fn partial_nominal_snapshot_preflights_the_program_header_before_children() {
        for owner in super::super::awbc_save::tests::owners() {
            let semantic = RuntimeSemanticTypeId::from_bytes([1; 32]);
            let original = owner
                .types()
                .try_record_value(
                    semantic,
                    vec![RuntimeValue::Bool(true)],
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                )
                .unwrap();
            let mut live = RuntimePlaceStorage::from(original.clone());
            let moved = live.take_field(&[field(0)]).unwrap();
            assert!(live.snapshot_for_owner(None).is_err());
            let saved = live.snapshot_for_owner(Some(&owner)).unwrap();
            let bytes = serde_json::to_vec(&saved).unwrap();
            let decoded: RuntimePlaceStorage<AwbcRuntimeValueSnapshot> =
                serde_json::from_slice(&bytes).unwrap();
            let mut restored = decoded.into_live_for_program(&owner).unwrap();
            assert!(
                restored
                    .assign_field(&[field(0)], moved)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(restored.take(), Some(original));

            let mut forged = saved.clone();
            let PlaceState::Record {
                header: RecordHeader::Nominal { layout, .. },
                fields,
            } = &mut forged.state
            else {
                panic!("nominal partial record")
            };
            *layout = TypeLayoutHash::from_bytes([255; 32]);
            fields.push(RuntimePlaceStorage::from(
                AwbcRuntimeValueSnapshot::Callable(super::super::AwbcRuntimeCallableSnapshot {
                    state: crate::runtime_id::RuntimeCallableStateId::from_zero_based(999).unwrap(),
                    retained: vec![],
                }),
            ));
            let before = live.clone();
            let error = forged
                .into_live_for_program(&owner)
                .unwrap_err()
                .to_string();
            assert!(error.contains("layout"), "{error}");
            assert!(!error.contains("callable"), "{error}");
            assert_eq!(live, before);
            assert_eq!(serde_json::to_vec(&saved).unwrap(), bytes);
        }
    }

    #[test]
    fn partial_record_header_work_limits_reject_exactly_one_over_without_activation() {
        let owner = empty_owner();
        let limits = crate::entry::RuntimeSchemaLimits::engine_default();
        let mut deepest = RuntimePlaceStorage::<AwbcRuntimeValueSnapshot>::default();
        for _ in 0..limits.max_depth {
            deepest = RuntimePlaceStorage {
                state: PlaceState::Record {
                    header: RecordHeader::Structural {
                        names: vec!["next".into()],
                    },
                    fields: vec![deepest],
                },
            };
        }
        assert!(deepest.validate_record_headers(Some(&owner)).is_ok());
        let deeper = RuntimePlaceStorage {
            state: PlaceState::Record {
                header: RecordHeader::Structural {
                    names: vec!["next".into()],
                },
                fields: vec![deepest],
            },
        };
        assert!(deeper.validate_record_headers(Some(&owner)).is_err());

        let children = (limits.max_nodes - 1) as usize;
        let mut widest = RuntimePlaceStorage::<AwbcRuntimeValueSnapshot> {
            state: PlaceState::Record {
                header: RecordHeader::Structural {
                    names: (0..children).map(|ordinal| format!("f{ordinal}")).collect(),
                },
                fields: vec![RuntimePlaceStorage::default(); children],
            },
        };
        assert!(widest.validate_record_headers(Some(&owner)).is_ok());
        let PlaceState::Record {
            header: RecordHeader::Structural { names },
            fields,
        } = &mut widest.state
        else {
            unreachable!()
        };
        names.push(format!("f{children}"));
        fields.push(RuntimePlaceStorage::default());
        assert!(widest.validate_record_headers(Some(&owner)).is_err());
        assert!(widest.values().next().is_none());
    }
}
