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
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(try_from = "PlaceState<T>")]
pub struct RuntimePlaceStorage<T> {
    state: PlaceState<T>,
}

impl<T: Serialize> Serialize for RuntimePlaceStorage<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.state.serialize(serializer)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

    pub fn try_map<U, E>(
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

    pub fn try_map_ref<U, E>(
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

    pub(crate) fn validate_record_headers(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<(), String> {
        if let Some((header, fields)) = self.record_parts() {
            if let RecordHeader::Nominal {
                nominal,
                semantic_identity,
                layout,
            } = header
            {
                owner
                    .types()
                    .require_nominal(*semantic_identity, nominal, *layout)
                    .map_err(|error| error.to_string())?;
            }
            for field in fields {
                field.validate_record_headers(owner)?;
            }
        }
        Ok(())
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
            } => RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                nominal,
                semantic_identity,
                layout,
                fields,
            )),
            RecordHeader::Structural { names } => {
                RuntimeValue::try_record(names.into_iter().zip(fields).collect())
                    .expect("header retained from an admitted record")
            }
        };
        self.state = PlaceState::Initialized(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(ordinal: usize) -> RuntimeRecordFieldId {
        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap()
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
        let encoded = serde_json::to_value(&storage).unwrap();
        let restored: RuntimePlaceStorage<RuntimeValue> = serde_json::from_value(encoded).unwrap();
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
    fn malformed_partial_record_inventory_is_rejected_during_decode() {
        let mut storage = RuntimePlaceStorage::from(
            RuntimeValue::try_record(vec![
                ("first".into(), RuntimeValue::Bool(true)),
                ("second".into(), RuntimeValue::Bool(false)),
            ])
            .unwrap(),
        );
        storage.take_field(&[field(0)]).unwrap();
        let valid = serde_json::to_value(storage).unwrap();
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
                serde_json::from_value::<RuntimePlaceStorage<RuntimeValue>>(malformed).is_err()
            );
        }
    }
}
