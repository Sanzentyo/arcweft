//! Logical payload type terms. Stable case/field identities belong to the seal.

use std::hash::{Hash, Hasher};
use std::slice;

use super::{
    AcceptedVariantCaseSemanticId, AcceptedVariantPayloadFieldSemanticId, CheckedVariantPayload,
    TypeKind, VariantPayloadOwnerFamily, VariantPayloadSealError, VariantPayloadShape,
};

/// One record field in a logical payload type, without a checked field identity.
#[derive(Clone, Debug)]
pub struct VariantPayloadRecordTypeField {
    ordinal: u32,
    diagnostic_name: String,
    ty: TypeKind,
}

impl VariantPayloadRecordTypeField {
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub fn diagnostic_name(&self) -> &str {
        &self.diagnostic_name
    }
    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
}

impl PartialEq for VariantPayloadRecordTypeField {
    fn eq(&self, other: &Self) -> bool {
        self.ordinal == other.ordinal && self.ty == other.ty
    }
}
impl Eq for VariantPayloadRecordTypeField {}
impl Hash for VariantPayloadRecordTypeField {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ordinal.hash(state);
        self.ty.hash(state);
    }
}

/// Non-unit payload structure. Empty tuple and empty record remain distinct.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum VariantPayloadTypeShape {
    Tuple(Box<[TypeKind]>),
    Record(Box<[VariantPayloadRecordTypeField]>),
}

pub(crate) struct VariantPayloadTypeChildren<'a> {
    owner: Option<&'a TypeKind>,
    fields: PayloadFieldChildren<'a>,
}

enum PayloadFieldChildren<'a> {
    Tuple(slice::Iter<'a, TypeKind>),
    Record(slice::Iter<'a, VariantPayloadRecordTypeField>),
}

impl<'a> Iterator for VariantPayloadTypeChildren<'a> {
    type Item = &'a TypeKind;

    fn next(&mut self) -> Option<Self::Item> {
        self.owner.take().or_else(|| match &mut self.fields {
            PayloadFieldChildren::Tuple(fields) => fields.next(),
            PayloadFieldChildren::Record(fields) => {
                fields.next().map(VariantPayloadRecordTypeField::ty)
            }
        })
    }
}

impl VariantPayloadTypeShape {
    pub(crate) fn try_record(
        fields: impl IntoIterator<Item = (String, TypeKind)>,
    ) -> Result<Self, VariantPayloadSealError> {
        let mut names = std::collections::HashSet::new();
        let fields = fields
            .into_iter()
            .enumerate()
            .map(|(ordinal, (diagnostic_name, ty))| {
                let ordinal = u32::try_from(ordinal)
                    .map_err(|_| VariantPayloadSealError::FieldOrdinalOverflow)?;
                if !names.insert(diagnostic_name.clone()) {
                    return Err(VariantPayloadSealError::DuplicateRecordFieldName);
                }
                if ty.contains_nominal_poison() {
                    return Err(VariantPayloadSealError::PoisonedFieldType { ordinal });
                }
                Ok(VariantPayloadRecordTypeField {
                    ordinal,
                    diagnostic_name,
                    ty,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self::Record(fields))
    }

    pub(crate) fn try_seal(
        &self,
        family: VariantPayloadOwnerFamily,
        owner: super::SemanticTypeDigest,
        ordinal: u32,
    ) -> Result<VariantPayloadShape, VariantPayloadSealError> {
        match self {
            Self::Tuple(fields) => {
                VariantPayloadShape::try_tuple(family, owner, ordinal, fields.iter().cloned())
            }
            Self::Record(fields) => VariantPayloadShape::try_record(
                family,
                owner,
                ordinal,
                fields
                    .iter()
                    .map(|field| (field.diagnostic_name.clone(), field.ty.clone())),
            ),
        }
    }

    pub const fn tuple_fields(&self) -> Option<&[TypeKind]> {
        match self {
            Self::Tuple(fields) => Some(fields),
            Self::Record(_) => None,
        }
    }

    pub const fn record_fields(&self) -> Option<&[VariantPayloadRecordTypeField]> {
        match self {
            Self::Record(fields) => Some(fields),
            Self::Tuple(_) => None,
        }
    }

    pub const fn field_count(&self) -> usize {
        match self {
            Self::Tuple(fields) => fields.len(),
            Self::Record(fields) => fields.len(),
        }
    }

    pub const fn semantic_shape_tag(&self) -> u8 {
        match self {
            Self::Tuple(_) => 1,
            Self::Record(_) => 2,
        }
    }

    pub fn single_tuple_field(&self) -> Option<&TypeKind> {
        match self {
            Self::Tuple(fields) if fields.len() == 1 => fields.first(),
            _ => None,
        }
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Tuple(fields) => fields.iter().try_for_each(visitor),
            Self::Record(fields) => fields.iter().try_for_each(|field| visitor(&field.ty)),
        }
    }

    pub(crate) fn try_map<E>(
        &self,
        map: &mut impl FnMut(&TypeKind) -> Result<TypeKind, E>,
    ) -> Result<Self, E> {
        Ok(match self {
            Self::Tuple(fields) => Self::Tuple(fields.iter().map(map).collect::<Result<_, _>>()?),
            Self::Record(fields) => Self::Record(
                fields
                    .iter()
                    .map(|field| {
                        Ok(VariantPayloadRecordTypeField {
                            ordinal: field.ordinal,
                            diagnostic_name: field.diagnostic_name.clone(),
                            ty: map(&field.ty)?,
                        })
                    })
                    .collect::<Result<_, E>>()?,
            ),
        })
    }
}

/// An exact case's logical type projection with a transformable typed owner.
///
/// Creation originates in an accepted case. A uniform type transformation must
/// map both the owner and the fields; field/case hashes are never carried across
/// that transition. This type may contain active inference terms and is not a
/// certificate that stable semantic identities have already been issued.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VariantPayloadType {
    owner_family: VariantPayloadOwnerFamily,
    owner_type: Box<TypeKind>,
    case_ordinal: u32,
    shape: VariantPayloadTypeShape,
}

impl VariantPayloadType {
    /// Projects an admitted case schema before its owner type has completed
    /// inference. Stable case/field identities are issued only by `try_seal`.
    pub(crate) fn from_prepared_case(
        owner_family: VariantPayloadOwnerFamily,
        owner_type: TypeKind,
        case_ordinal: u32,
        shape: VariantPayloadTypeShape,
    ) -> Self {
        Self {
            owner_family,
            owner_type: Box::new(owner_type),
            case_ordinal,
            shape,
        }
    }
    pub(crate) fn try_new(
        owner_family: VariantPayloadOwnerFamily,
        owner_type: TypeKind,
        case_ordinal: u32,
        case: AcceptedVariantCaseSemanticId,
        shape: VariantPayloadShape,
    ) -> Result<Self, VariantPayloadSealError> {
        CheckedVariantPayload::try_new(owner_family, owner_type, case_ordinal, case, shape)
            .map(CheckedVariantPayload::into_type)
    }

    pub(super) fn from_checked(checked: CheckedVariantPayload) -> Self {
        let shape = match checked.shape {
            VariantPayloadShape::Tuple(fields) => VariantPayloadTypeShape::Tuple(
                fields
                    .into_vec()
                    .into_iter()
                    .map(|field| field.ty)
                    .collect(),
            ),
            VariantPayloadShape::Record(fields) => VariantPayloadTypeShape::Record(
                fields
                    .into_vec()
                    .into_iter()
                    .map(|field| VariantPayloadRecordTypeField {
                        ordinal: field.ordinal,
                        diagnostic_name: field.diagnostic_name,
                        ty: field.ty,
                    })
                    .collect(),
            ),
            VariantPayloadShape::Unit => unreachable!("checked payloads exclude unit cases"),
        };
        Self {
            owner_family: checked.owner_family,
            owner_type: checked.owner_type,
            case_ordinal: checked.case_ordinal,
            shape,
        }
    }

    pub const fn owner_family(&self) -> VariantPayloadOwnerFamily {
        self.owner_family
    }
    pub fn owner_type(&self) -> &TypeKind {
        &self.owner_type
    }
    pub const fn case_ordinal(&self) -> u32 {
        self.case_ordinal
    }
    pub const fn shape(&self) -> &VariantPayloadTypeShape {
        &self.shape
    }

    /// Joins independently encoded owner/field identities in the same order
    /// supplied by `children`; it does not seal semantic rows or hash types.
    pub(in crate::types) fn semantic_case_from_type_digests(
        &self,
        digests: Vec<crate::types::SemanticTypeDigest>,
    ) -> AcceptedVariantCaseSemanticId {
        let mut digests = digests.into_iter();
        let owner = digests.next().expect("payload identity retains its owner");
        let tag = self.shape.semantic_shape_tag();
        let mut field = |ordinal| {
            (
                ordinal,
                AcceptedVariantPayloadFieldSemanticId::issue_from_type_digest(
                    self.owner_family,
                    owner,
                    self.case_ordinal,
                    tag,
                    ordinal,
                    digests
                        .next()
                        .expect("payload identity retains every field"),
                ),
            )
        };
        let fields = match &self.shape {
            VariantPayloadTypeShape::Tuple(fields) => fields
                .iter()
                .enumerate()
                .map(|(ordinal, _)| {
                    field(
                        u32::try_from(ordinal)
                            .expect("payload field inventories retain u32 ordinals"),
                    )
                })
                .collect::<Vec<_>>(),
            VariantPayloadTypeShape::Record(fields) => fields
                .iter()
                .map(|item| field(item.ordinal))
                .collect::<Vec<_>>(),
        };
        assert!(
            digests.next().is_none(),
            "payload identity has no extra fields"
        );
        AcceptedVariantCaseSemanticId::issue_from_fields(
            self.owner_family,
            owner,
            self.case_ordinal,
            tag,
            fields.len(),
            fields,
        )
    }

    pub(in crate::types) fn children(&self) -> VariantPayloadTypeChildren<'_> {
        VariantPayloadTypeChildren {
            owner: Some(&self.owner_type),
            fields: match &self.shape {
                VariantPayloadTypeShape::Tuple(fields) => {
                    PayloadFieldChildren::Tuple(fields.iter())
                }
                VariantPayloadTypeShape::Record(fields) => {
                    PayloadFieldChildren::Record(fields.iter())
                }
            },
        }
    }

    pub(in crate::types) fn has_same_header(&self, other: &Self) -> bool {
        self.owner_family == other.owner_family
            && self.case_ordinal == other.case_ordinal
            && self.shape.semantic_shape_tag() == other.shape.semantic_shape_tag()
            && self.shape.field_count() == other.shape.field_count()
    }

    pub(in crate::types) fn map(&self, mut map: impl FnMut(&TypeKind) -> TypeKind) -> Self {
        self.try_map(|ty| Ok::<_, std::convert::Infallible>(map(ty)))
            .unwrap_or_else(|never| match never {})
    }

    /// Maps the entire dependent type relation without issuing stable identities.
    pub(in crate::types) fn try_map<E>(
        &self,
        mut map: impl FnMut(&TypeKind) -> Result<TypeKind, E>,
    ) -> Result<Self, E> {
        let owner_type = map(&self.owner_type)?;
        let shape = self.shape.try_map(&mut map)?;
        Ok(Self {
            owner_family: self.owner_family,
            owner_type: Box::new(owner_type),
            case_ordinal: self.case_ordinal,
            shape,
        })
    }

    pub(in crate::types) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.children().try_for_each(visitor)
    }

    /// Issues stable identities for the transformed owner and its payload rows.
    pub(crate) fn try_seal(&self) -> Result<CheckedVariantPayload, VariantPayloadSealError> {
        if self.owner_type.contains_nominal_poison() {
            return Err(VariantPayloadSealError::PoisonedOwnerType);
        }
        let owner_semantic_type = self.owner_type.semantic_identity_digest()?;
        let shape =
            self.shape
                .try_seal(self.owner_family, owner_semantic_type, self.case_ordinal)?;
        let case = AcceptedVariantCaseSemanticId::issue(
            self.owner_family,
            owner_semantic_type,
            self.case_ordinal,
            &shape,
        );
        Ok(CheckedVariantPayload {
            owner_family: self.owner_family,
            owner_type: self.owner_type.clone(),
            owner_semantic_type,
            case_ordinal: self.case_ordinal,
            case,
            shape,
        })
    }
}
