//! Dependent type checking for core `Reduction` constructors.

use arcweft_lang_syntax::{expr::CallArg, reference::BorrowKind};

use super::super::helpers::type_kind_label;
use super::{TypeCheckError, TypeChecker, TypeKind};
use crate::callable::ReductionConstructorKind;

impl TypeChecker<'_> {
    pub(super) fn check_reduction_constructor_call(
        &mut self,
        kind: ReductionConstructorKind,
        args: &[CallArg],
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        match kind {
            ReductionConstructorKind::Unchanged => {
                let expected_state = expected.and_then(reduction_state_type);
                if args.len() != 1 {
                    self.errors.push(TypeCheckError::new(format!(
                        "`Reduction.unchanged` requires exactly one positional state borrow, got {}",
                        args.len()
                    )));
                }
                let mut inferred_state = None;
                for arg in args {
                    match arg {
                        CallArg::Positional(value) => {
                            let actual = self.check_expr(value);
                            let Some(TypeKind::BorrowRef { kind, inner, .. }) = actual else {
                                self.errors.push(TypeCheckError::new(
                                    "`Reduction.unchanged` state must be a shared borrow"
                                        .to_owned(),
                                ));
                                continue;
                            };
                            if kind != BorrowKind::Shared {
                                self.errors.push(TypeCheckError::new(
                                    "`Reduction.unchanged` state must be a shared borrow"
                                        .to_owned(),
                                ));
                                continue;
                            }
                            if let Some(expected_state) = expected_state.as_ref()
                                && !self.types_compatible(expected_state, &inner)
                            {
                                self.errors.push(TypeCheckError::new(format!(
                                    "`Reduction.unchanged` state must borrow {}, found {}",
                                    type_kind_label(expected_state),
                                    type_kind_label(&inner)
                                )));
                            }
                            inferred_state.get_or_insert(*inner);
                        }
                        CallArg::Named { name, value } => {
                            self.errors.push(TypeCheckError::new(format!(
                                "`Reduction.unchanged` state must be positional, got named `{name}`"
                            )));
                            self.check_expr(value);
                        }
                        CallArg::Spread { value } => {
                            self.errors.push(TypeCheckError::new(
                                "`Reduction.unchanged` state cannot be spread".to_owned(),
                            ));
                            self.check_expr(value);
                        }
                    }
                }
                if expected_state.is_some() {
                    expected
                        .cloned()
                        .expect("a resolved Reduction state came from an expected type")
                } else {
                    inferred_state.map_or_else(
                        || TypeKind::Named("Reduction<_>".to_owned()),
                        |state| TypeKind::Named(format!("Reduction<{}>", state.source_label())),
                    )
                }
            }
        }
    }
}

fn reduction_state_type(ty: &TypeKind) -> Option<TypeKind> {
    let TypeKind::AcceptedNominal(nominal) = ty else {
        return None;
    };
    if crate::types::direct_type_name(nominal.declaration().canonical_path()) != Some("Reduction") {
        return None;
    }
    let [state] = nominal.arguments() else {
        return None;
    };
    Some(state.clone())
}
