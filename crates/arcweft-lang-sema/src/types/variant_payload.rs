//! Exact anonymous payload types owned by accepted variant cases.
//!
//! Authored type resolution cannot construct this family. Final semantic
//! analysis issues it only from one accepted case schema so tuple/record
//! payload children and record-rest bindings retain one exact type authority.

use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

use super::{SemanticTypeDigest, TypeKind};

const VARIANT_CASE_SEMANTIC_DOMAIN: &[u8] = b"arcweft.lang.accepted-variant-case.v1\0";
const VARIANT_PAYLOAD_FIELD_SEMANTIC_DOMAIN: &[u8] =
    b"arcweft.lang.accepted-variant-payload-field.v1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedVariantCaseSemanticId([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VariantPayloadOwnerFamily {
    Project,
    CharacterNominal,
    BuiltinClosed,
    Option,
    Result,
    RuntimeBuiltin,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedVariantPayloadFieldSemanticId([u8; 32]);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VariantPayloadTupleField {
    ordinal: u32,
    semantic_id: AcceptedVariantPayloadFieldSemanticId,
    ty: TypeKind,
}

#[derive(Clone, Debug)]
pub struct VariantPayloadRecordField {
    ordinal: u32,
    semantic_id: AcceptedVariantPayloadFieldSemanticId,
    diagnostic_name: String,
    ty: TypeKind,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum VariantPayloadShape {
    Unit,
    Tuple(Box<[VariantPayloadTupleField]>),
    Record(Box<[VariantPayloadRecordField]>),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VariantPayloadType {
    owner_family: VariantPayloadOwnerFamily,
    owner_type: SemanticTypeDigest,
    case_ordinal: u32,
    case: AcceptedVariantCaseSemanticId,
    shape: VariantPayloadShape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VariantPayloadSealError {
    FieldOrdinalOverflow,
    DuplicateRecordFieldName,
    PoisonedFieldType { ordinal: u32 },
    UnitPayloadType,
    InvalidFieldRows,
    CaseIdentityMismatch,
}

impl AcceptedVariantCaseSemanticId {
    pub(crate) fn issue(
        owner_family: VariantPayloadOwnerFamily,
        owner_type: SemanticTypeDigest,
        case_ordinal: u32,
        shape: &VariantPayloadShape,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(VARIANT_CASE_SEMANTIC_DOMAIN);
        hasher.update(&[owner_family.canonical_tag()]);
        hasher.update(owner_type.as_bytes());
        hasher.update(&case_ordinal.to_le_bytes());
        match shape {
            VariantPayloadShape::Unit => {
                hasher.update(&[0]);
            }
            VariantPayloadShape::Tuple(fields) => {
                hasher.update(&[1]);
                hasher.update(&field_count(
                    fields.last().map(VariantPayloadTupleField::ordinal),
                ));
                for field in fields {
                    hasher.update(&field.ordinal.to_le_bytes());
                    hasher.update(field.semantic_id.as_bytes());
                    hasher.update(field.ty.semantic_identity_digest().as_bytes());
                }
            }
            VariantPayloadShape::Record(fields) => {
                hasher.update(&[2]);
                hasher.update(&field_count(
                    fields.last().map(VariantPayloadRecordField::ordinal),
                ));
                for field in fields {
                    hasher.update(field.semantic_id.as_bytes());
                    hasher.update(field.ty.semantic_identity_digest().as_bytes());
                }
            }
        }
        Self(hasher.finalize().into())
    }

    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl AcceptedVariantPayloadFieldSemanticId {
    fn issue(
        owner_family: VariantPayloadOwnerFamily,
        owner_type: SemanticTypeDigest,
        case_ordinal: u32,
        shape_tag: u8,
        field_ordinal: u32,
        ty: &TypeKind,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(VARIANT_PAYLOAD_FIELD_SEMANTIC_DOMAIN);
        hasher.update(&[owner_family.canonical_tag()]);
        hasher.update(owner_type.as_bytes());
        hasher.update(&case_ordinal.to_le_bytes());
        hasher.update(&[shape_tag]);
        hasher.update(&field_ordinal.to_le_bytes());
        hasher.update(ty.semantic_identity_digest().as_bytes());
        Self(hasher.finalize().into())
    }

    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl VariantPayloadTupleField {
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    pub(crate) const fn semantic_id(&self) -> AcceptedVariantPayloadFieldSemanticId {
        self.semantic_id
    }
}

impl VariantPayloadRecordField {
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn semantic_id(&self) -> AcceptedVariantPayloadFieldSemanticId {
        self.semantic_id
    }

    pub fn diagnostic_name(&self) -> &str {
        &self.diagnostic_name
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
}

impl PartialEq for VariantPayloadRecordField {
    fn eq(&self, other: &Self) -> bool {
        self.ordinal == other.ordinal
            && self.semantic_id == other.semantic_id
            && self.ty == other.ty
    }
}

impl Eq for VariantPayloadRecordField {}

impl Hash for VariantPayloadRecordField {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ordinal.hash(state);
        self.semantic_id.hash(state);
        self.ty.hash(state);
    }
}

impl VariantPayloadOwnerFamily {
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::Project => 0,
            Self::CharacterNominal => 1,
            Self::BuiltinClosed => 2,
            Self::Option => 3,
            Self::Result => 4,
            Self::RuntimeBuiltin => 5,
        }
    }
}

impl VariantPayloadShape {
    pub(crate) fn try_tuple(
        owner_family: VariantPayloadOwnerFamily,
        owner_type: SemanticTypeDigest,
        case_ordinal: u32,
        fields: impl IntoIterator<Item = TypeKind>,
    ) -> Result<Self, VariantPayloadSealError> {
        fields
            .into_iter()
            .enumerate()
            .map(|(ordinal, ty)| {
                let ordinal = u32::try_from(ordinal)
                    .map_err(|_| VariantPayloadSealError::FieldOrdinalOverflow)?;
                if ty.contains_nominal_poison() {
                    return Err(VariantPayloadSealError::PoisonedFieldType { ordinal });
                }
                Ok(VariantPayloadTupleField {
                    ordinal,
                    semantic_id: AcceptedVariantPayloadFieldSemanticId::issue(
                        owner_family,
                        owner_type,
                        case_ordinal,
                        1,
                        ordinal,
                        &ty,
                    ),
                    ty,
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|fields| Self::Tuple(fields.into_boxed_slice()))
    }

    pub(crate) fn try_record(
        owner_family: VariantPayloadOwnerFamily,
        owner_type: SemanticTypeDigest,
        case_ordinal: u32,
        fields: impl IntoIterator<Item = (String, TypeKind)>,
    ) -> Result<Self, VariantPayloadSealError> {
        let mut names = HashSet::new();
        let mut checked = Vec::new();
        for (ordinal, (diagnostic_name, ty)) in fields.into_iter().enumerate() {
            if !names.insert(diagnostic_name.clone()) {
                return Err(VariantPayloadSealError::DuplicateRecordFieldName);
            }
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| VariantPayloadSealError::FieldOrdinalOverflow)?;
            if ty.contains_nominal_poison() {
                return Err(VariantPayloadSealError::PoisonedFieldType { ordinal });
            }
            checked.push(VariantPayloadRecordField {
                ordinal,
                semantic_id: AcceptedVariantPayloadFieldSemanticId::issue(
                    owner_family,
                    owner_type,
                    case_ordinal,
                    2,
                    ordinal,
                    &ty,
                ),
                diagnostic_name,
                ty,
            });
        }
        Ok(Self::Record(checked.into_boxed_slice()))
    }

    pub const fn tuple_fields(&self) -> Option<&[VariantPayloadTupleField]> {
        match self {
            Self::Tuple(fields) => Some(fields),
            Self::Unit | Self::Record(_) => None,
        }
    }

    pub const fn record_fields(&self) -> Option<&[VariantPayloadRecordField]> {
        match self {
            Self::Record(fields) => Some(fields),
            Self::Unit | Self::Tuple(_) => None,
        }
    }

    pub const fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    pub const fn semantic_shape_tag(&self) -> u8 {
        match self {
            Self::Unit => 0,
            Self::Tuple(_) => 1,
            Self::Record(_) => 2,
        }
    }

    pub const fn field_count(&self) -> usize {
        match self {
            Self::Unit => 0,
            Self::Tuple(fields) => fields.len(),
            Self::Record(fields) => fields.len(),
        }
    }

    pub const fn single_tuple_field(&self) -> Option<&TypeKind> {
        match self {
            Self::Tuple(fields) if fields.len() == 1 && fields[0].ordinal == 0 => {
                Some(&fields[0].ty)
            }
            Self::Unit | Self::Tuple(_) | Self::Record(_) => None,
        }
    }

    pub(crate) fn has_same_diagnostic_schema(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unit, Self::Unit) => true,
            (Self::Tuple(left), Self::Tuple(right)) => left == right,
            (Self::Record(left), Self::Record(right)) => {
                left.len() == right.len()
                    && left.iter().zip(right).all(|(left, right)| {
                        left == right && left.diagnostic_name == right.diagnostic_name
                    })
            }
            (Self::Unit, Self::Tuple(_) | Self::Record(_))
            | (Self::Tuple(_), Self::Unit | Self::Record(_))
            | (Self::Record(_), Self::Unit | Self::Tuple(_)) => false,
        }
    }

    pub(crate) fn has_valid_rows(
        &self,
        owner_family: VariantPayloadOwnerFamily,
        owner_type: SemanticTypeDigest,
        case_ordinal: u32,
    ) -> bool {
        match self {
            Self::Unit => true,
            Self::Tuple(fields) => fields.iter().enumerate().all(|(ordinal, field)| {
                u32::try_from(ordinal).is_ok_and(|ordinal| {
                    field.ordinal == ordinal
                        && field.semantic_id
                            == AcceptedVariantPayloadFieldSemanticId::issue(
                                owner_family,
                                owner_type,
                                case_ordinal,
                                1,
                                ordinal,
                                &field.ty,
                            )
                }) && !field.ty.contains_nominal_poison()
            }),
            Self::Record(fields) => {
                let mut names = HashSet::new();
                fields.iter().enumerate().all(|(ordinal, field)| {
                    u32::try_from(ordinal).is_ok_and(|ordinal| {
                        field.ordinal == ordinal
                            && names.insert(field.diagnostic_name.as_str())
                            && !field.ty.contains_nominal_poison()
                            && field.semantic_id
                                == AcceptedVariantPayloadFieldSemanticId::issue(
                                    owner_family,
                                    owner_type,
                                    case_ordinal,
                                    2,
                                    ordinal,
                                    &field.ty,
                                )
                    })
                })
            }
        }
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Unit => Ok(()),
            Self::Tuple(fields) => fields.iter().try_for_each(|field| visitor(&field.ty)),
            Self::Record(fields) => fields.iter().try_for_each(|field| visitor(&field.ty)),
        }
    }
}

impl VariantPayloadType {
    pub(crate) fn try_new(
        owner_family: VariantPayloadOwnerFamily,
        owner_type: SemanticTypeDigest,
        case_ordinal: u32,
        case: AcceptedVariantCaseSemanticId,
        shape: VariantPayloadShape,
    ) -> Result<Self, VariantPayloadSealError> {
        if shape.is_unit() {
            return Err(VariantPayloadSealError::UnitPayloadType);
        }
        if !shape.has_valid_rows(owner_family, owner_type, case_ordinal) {
            return Err(VariantPayloadSealError::InvalidFieldRows);
        }
        if AcceptedVariantCaseSemanticId::issue(owner_family, owner_type, case_ordinal, &shape)
            != case
        {
            return Err(VariantPayloadSealError::CaseIdentityMismatch);
        }
        Ok(Self {
            owner_family,
            owner_type,
            case_ordinal,
            case,
            shape,
        })
    }

    pub const fn owner_family(&self) -> VariantPayloadOwnerFamily {
        self.owner_family
    }

    pub const fn owner_type(&self) -> SemanticTypeDigest {
        self.owner_type
    }

    pub const fn case_ordinal(&self) -> u32 {
        self.case_ordinal
    }

    pub(crate) const fn case(&self) -> AcceptedVariantCaseSemanticId {
        self.case
    }

    pub const fn shape(&self) -> &VariantPayloadShape {
        &self.shape
    }
}

fn field_count(last_ordinal: Option<u32>) -> [u8; 8] {
    last_ordinal
        .map_or(0, |ordinal| u64::from(ordinal) + 1)
        .to_le_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(name: &str) -> SemanticTypeDigest {
        TypeKind::Named(name.to_owned()).semantic_identity_digest()
    }

    fn payload_type(
        owner_family: VariantPayloadOwnerFamily,
        owner_type: SemanticTypeDigest,
        case_ordinal: u32,
        shape: VariantPayloadShape,
    ) -> VariantPayloadType {
        let case =
            AcceptedVariantCaseSemanticId::issue(owner_family, owner_type, case_ordinal, &shape);
        VariantPayloadType::try_new(owner_family, owner_type, case_ordinal, case, shape)
            .expect("fixture payload schema is internally consistent")
    }

    #[test]
    fn unit_empty_tuple_and_empty_record_are_distinct_payload_shapes() {
        let owner = owner("EmptyShapes");
        let tuple =
            VariantPayloadShape::try_tuple(VariantPayloadOwnerFamily::Project, owner, 0, [])
                .expect("empty tuple remains an explicit tuple payload");
        let record = VariantPayloadShape::try_record(
            VariantPayloadOwnerFamily::Project,
            owner,
            0,
            std::iter::empty::<(String, TypeKind)>(),
        )
        .expect("empty record remains an explicit record payload");
        assert_ne!(VariantPayloadShape::Unit, tuple);
        assert_ne!(VariantPayloadShape::Unit, record);
        assert_ne!(tuple, record);
        assert_ne!(
            AcceptedVariantCaseSemanticId::issue(
                VariantPayloadOwnerFamily::Project,
                owner,
                0,
                &VariantPayloadShape::Unit,
            ),
            AcceptedVariantCaseSemanticId::issue(
                VariantPayloadOwnerFamily::Project,
                owner,
                0,
                &tuple,
            )
        );
        assert_ne!(
            AcceptedVariantCaseSemanticId::issue(
                VariantPayloadOwnerFamily::Project,
                owner,
                0,
                &tuple,
            ),
            AcceptedVariantCaseSemanticId::issue(
                VariantPayloadOwnerFamily::Project,
                owner,
                0,
                &record,
            )
        );
    }

    #[test]
    fn project_newtype_tuple_and_environment_tuple_variant_keep_exact_arity() {
        let project_owner = owner("ProjectNewtype");
        let nested_tuple = TypeKind::Tuple(vec![TypeKind::I64, TypeKind::Bool]);
        let project = VariantPayloadShape::try_tuple(
            VariantPayloadOwnerFamily::Project,
            project_owner,
            0,
            [nested_tuple.clone()],
        )
        .expect("project payload is one authored field");
        let environment = VariantPayloadShape::try_tuple(
            VariantPayloadOwnerFamily::RuntimeBuiltin,
            owner("EnvironmentTuple"),
            0,
            [TypeKind::I64, TypeKind::Bool],
        )
        .expect("environment tuple payload retains its explicit Rust fields");

        assert!(matches!(
            project.tuple_fields(),
            Some([field]) if field.ordinal() == 0 && field.ty() == &nested_tuple
        ));
        assert_eq!(environment.tuple_fields().map(<[_]>::len), Some(2));
    }

    #[test]
    fn record_diagnostic_renames_do_not_change_payload_semantic_identity() {
        let owner = owner("RenameInvariant");
        let original = VariantPayloadShape::try_record(
            VariantPayloadOwnerFamily::BuiltinClosed,
            owner,
            3,
            [
                ("z".to_owned(), TypeKind::I64),
                ("a".to_owned(), TypeKind::Bool),
            ],
        )
        .expect("original record schema");
        let renamed = VariantPayloadShape::try_record(
            VariantPayloadOwnerFamily::BuiltinClosed,
            owner,
            3,
            [
                ("left".to_owned(), TypeKind::I64),
                ("right".to_owned(), TypeKind::Bool),
            ],
        )
        .expect("renamed record schema");
        let original_type = payload_type(
            VariantPayloadOwnerFamily::BuiltinClosed,
            owner,
            3,
            original.clone(),
        );
        let renamed_type = payload_type(
            VariantPayloadOwnerFamily::BuiltinClosed,
            owner,
            3,
            renamed.clone(),
        );

        assert_eq!(original, renamed);
        assert!(!original.has_same_diagnostic_schema(&renamed));
        assert_eq!(original_type, renamed_type);
        assert_eq!(
            TypeKind::VariantPayload(Box::new(original_type)).semantic_identity_digest(),
            TypeKind::VariantPayload(Box::new(renamed_type)).semantic_identity_digest(),
        );
        let original_fields = original.record_fields().expect("record fields");
        let renamed_fields = renamed.record_fields().expect("record fields");
        assert!(
            original_fields
                .iter()
                .zip(renamed_fields)
                .all(|(left, right)| {
                    left.semantic_id() == right.semantic_id() && left.ty() == right.ty()
                })
        );
    }

    #[test]
    fn owner_case_shape_ordinal_and_field_type_all_change_payload_identity() {
        let owner_type = owner("IdentityInputs");
        let base = VariantPayloadShape::try_record(
            VariantPayloadOwnerFamily::Project,
            owner_type,
            0,
            [
                ("first".to_owned(), TypeKind::I64),
                ("second".to_owned(), TypeKind::Bool),
            ],
        )
        .expect("base schema");
        let base_case = AcceptedVariantCaseSemanticId::issue(
            VariantPayloadOwnerFamily::Project,
            owner_type,
            0,
            &base,
        );
        let changed_type = VariantPayloadShape::try_record(
            VariantPayloadOwnerFamily::Project,
            owner_type,
            0,
            [
                ("first".to_owned(), TypeKind::U64),
                ("second".to_owned(), TypeKind::Bool),
            ],
        )
        .expect("changed field type");
        let reversed = VariantPayloadShape::try_record(
            VariantPayloadOwnerFamily::Project,
            owner_type,
            0,
            [
                ("second".to_owned(), TypeKind::Bool),
                ("first".to_owned(), TypeKind::I64),
            ],
        )
        .expect("changed declaration ordinals");
        let tuple = VariantPayloadShape::try_tuple(
            VariantPayloadOwnerFamily::Project,
            owner_type,
            0,
            [TypeKind::I64, TypeKind::Bool],
        )
        .expect("changed payload family");

        for changed in [
            AcceptedVariantCaseSemanticId::issue(
                VariantPayloadOwnerFamily::Result,
                owner_type,
                0,
                &base,
            ),
            AcceptedVariantCaseSemanticId::issue(
                VariantPayloadOwnerFamily::Project,
                owner("OtherOwner"),
                0,
                &base,
            ),
            AcceptedVariantCaseSemanticId::issue(
                VariantPayloadOwnerFamily::Project,
                owner_type,
                1,
                &base,
            ),
            AcceptedVariantCaseSemanticId::issue(
                VariantPayloadOwnerFamily::Project,
                owner_type,
                0,
                &changed_type,
            ),
            AcceptedVariantCaseSemanticId::issue(
                VariantPayloadOwnerFamily::Project,
                owner_type,
                0,
                &reversed,
            ),
            AcceptedVariantCaseSemanticId::issue(
                VariantPayloadOwnerFamily::Project,
                owner_type,
                0,
                &tuple,
            ),
        ] {
            assert_ne!(base_case, changed);
        }
    }

    #[test]
    fn record_payload_duplicate_names_fail_typed_for_same_or_different_types() {
        let owner = owner("Duplicates");
        for second in [TypeKind::I64, TypeKind::Bool] {
            assert_eq!(
                VariantPayloadShape::try_record(
                    VariantPayloadOwnerFamily::RuntimeBuiltin,
                    owner,
                    0,
                    [
                        ("field".to_owned(), TypeKind::I64),
                        ("field".to_owned(), second)
                    ],
                ),
                Err(VariantPayloadSealError::DuplicateRecordFieldName)
            );
        }
    }
}
