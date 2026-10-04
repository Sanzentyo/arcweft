//! Final local-rooted places shared by reads, writes and loans.

use crate::types::TypeKind;
use arcweft_lang_hir::identity::{ExprId, LocalId};

/// One local-rooted place, selected entirely from admitted field schemas.
/// Empty projections denote the whole declaration. Diagnostic field names
/// never participate in overlap or availability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedPlace {
    local: LocalId,
    fields: Box<[crate::final_analysis::CheckedFieldSelection]>,
}

impl CheckedPlace {
    pub(crate) fn from_field<'a>(
        owner: ExprId,
        expression: impl Fn(ExprId) -> Option<&'a crate::final_analysis::CheckedExpression>,
    ) -> Option<Self> {
        use crate::final_analysis::{
            CheckedExpressionResolution, CheckedFieldReceiver, CheckedSelectResolution,
        };
        let local = expression(owner)?.field_root(|child| {
            expression(child).and_then(crate::final_analysis::CheckedExpression::local_place_source)
        })?;
        let mut current = owner;
        let mut fields = Vec::new();
        while let Some(CheckedExpressionResolution::Select(CheckedSelectResolution::Field(field))) =
            expression(current).map(crate::final_analysis::CheckedExpression::resolution)
        {
            field.selection().runtime_field()?;
            match field.receiver() {
                CheckedFieldReceiver::Binding(_) => {
                    fields.extend(field.binding_place()?.fields().iter().rev().cloned());
                    break;
                }
                CheckedFieldReceiver::Expression(receiver) => {
                    fields.push(field.selection().clone());
                    current = receiver;
                }
            }
        }
        fields.reverse();
        Some(Self::new(local, fields.into_boxed_slice()))
    }

    pub(crate) fn new(
        local: LocalId,
        fields: Box<[crate::final_analysis::CheckedFieldSelection]>,
    ) -> Self {
        Self { local, fields }
    }

    pub(crate) fn from_local(local: LocalId) -> Self {
        Self::new(local, Box::new([]))
    }
    pub const fn local_id(&self) -> LocalId {
        self.local
    }
    pub(crate) fn matches_types(&self, root: &TypeKind, value: &TypeKind) -> bool {
        let Ok(root) = root.semantic_identity_digest() else {
            return false;
        };
        let Ok(value) = value.semantic_identity_digest() else {
            return false;
        };
        match (self.fields.first(), self.fields.last()) {
            (Some(first), Some(last)) => {
                first.owner_type() == root
                    && last.field_type() == value
                    && self
                        .fields
                        .iter()
                        .all(|field| field.runtime_field().is_some())
                    && self
                        .fields
                        .windows(2)
                        .all(|pair| pair[0].field_type() == pair[1].owner_type())
            }
            (None, None) => root == value,
            _ => false,
        }
    }
    pub const fn local(&self) -> LocalId {
        self.local
    }
    pub fn fields(&self) -> &[crate::final_analysis::CheckedFieldSelection] {
        &self.fields
    }
    pub(crate) fn into_fields(self) -> Box<[crate::final_analysis::CheckedFieldSelection]> {
        self.fields
    }

    pub(crate) fn overlaps(&self, other: &Self) -> bool {
        self.local == other.local
            && self
                .fields
                .iter()
                .zip(other.fields.iter())
                .all(|(left, right)| left.field() == right.field())
    }
}
