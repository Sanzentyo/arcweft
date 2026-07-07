use super::{CallArg, Expr, TypeCheckError, TypeChecker, TypeKind};
use crate::checker::EnumVariantPayloadType;
use crate::checker::helpers::expr_path_label;

impl TypeChecker<'_> {
    pub(super) fn check_enum_variant_call_expr(
        &mut self,
        callee: &Expr,
        args: &[CallArg],
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let path = expr_path_label(callee)?;
        self.check_enum_variant_call_path(&path, args, expected)
    }

    pub(super) fn check_enum_variant_call_path(
        &mut self,
        path: &str,
        args: &[CallArg],
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let expected = expected?;
        let payload = self.enum_variant_payload_for_path(expected, path)?;
        self.check_enum_tuple_constructor_payload(path, args, &payload);
        Some(expected.clone())
    }

    fn check_enum_tuple_constructor_payload(
        &mut self,
        path: &str,
        args: &[CallArg],
        payload: &EnumVariantPayloadType,
    ) {
        match payload {
            EnumVariantPayloadType::Unit => {
                if !args.is_empty() {
                    self.errors.push(TypeCheckError::new(format!(
                        "enum variant `{path}` does not accept a payload"
                    )));
                }
                for arg in args {
                    self.check_expr(arg.value());
                }
            }
            EnumVariantPayloadType::Tuple(items) => {
                if args.len() != items.len() {
                    self.errors.push(TypeCheckError::new(format!(
                        "enum variant `{path}` expects {} positional payload item(s), got {}",
                        items.len(),
                        args.len()
                    )));
                }
                for (index, arg) in args.iter().enumerate() {
                    match arg {
                        CallArg::Positional(value) => {
                            if let Some(expected) = items.get(index) {
                                self.expect_expr_type(
                                    value,
                                    expected,
                                    &format!("enum variant `{path}` payload {index}"),
                                );
                            } else {
                                self.check_expr(value);
                            }
                        }
                        CallArg::Named { name, value } => {
                            self.errors.push(TypeCheckError::new(format!(
                                "enum variant `{path}` tuple payload cannot use named argument `{name}`"
                            )));
                            self.check_expr(value);
                        }
                        CallArg::Spread { value } => {
                            self.errors.push(TypeCheckError::new(format!(
                                "enum variant `{path}` payload cannot be spread"
                            )));
                            self.check_expr(value);
                        }
                    }
                }
            }
            EnumVariantPayloadType::Record(_) => {
                self.errors.push(TypeCheckError::new(format!(
                    "enum variant `{path}` record payload must use record constructor syntax"
                )));
                for arg in args {
                    self.check_expr(arg.value());
                }
            }
        }
    }

    pub(super) fn check_enum_record_constructor_payload(
        &mut self,
        path: &str,
        fields: &[(String, Expr)],
        payload: &EnumVariantPayloadType,
    ) {
        let EnumVariantPayloadType::Record(expected_fields) = payload else {
            self.errors.push(TypeCheckError::new(format!(
                "enum variant `{path}` tuple payload must use call constructor syntax"
            )));
            self.check_record_fields(fields);
            return;
        };
        for (name, value) in fields {
            if let Some(expected) = expected_fields.get(name) {
                self.expect_expr_type(
                    value,
                    expected,
                    &format!("enum variant `{path}` field `{name}`"),
                );
            } else {
                self.errors.push(TypeCheckError::new(format!(
                    "enum variant `{path}` has no field `{name}`"
                )));
                self.check_expr(value);
            }
        }
        for required in expected_fields.keys() {
            if !fields.iter().any(|(name, _)| name == required) {
                self.errors.push(TypeCheckError::new(format!(
                    "enum variant `{path}` literal is missing field `{required}`"
                )));
            }
        }
    }

    pub(super) fn enum_has_variant(&self, ty: &TypeKind, variant: &str) -> bool {
        self.enum_variant_payload_for_name(ty, variant).is_some()
            || self.env.enum_has_variant(ty, variant)
    }

    pub(super) fn enum_variant_payload_for_path(
        &self,
        ty: &TypeKind,
        path: &str,
    ) -> Option<EnumVariantPayloadType> {
        let enum_name = nominal_type_name(ty)?;
        let (prefix, variant) = enum_constructor_path_parts(path);
        if prefix.is_some_and(|prefix| prefix != enum_name) {
            return None;
        }
        self.nominal_variant_payloads
            .get(enum_name)?
            .get(variant)
            .cloned()
    }

    fn enum_variant_payload_for_name(
        &self,
        ty: &TypeKind,
        variant: &str,
    ) -> Option<EnumVariantPayloadType> {
        let enum_name = nominal_type_name(ty)?;
        let variant = variant.strip_prefix('.').unwrap_or(variant);
        let variant = variant.rsplit_once('.').map_or(variant, |(_, name)| name);
        self.nominal_variant_payloads
            .get(enum_name)?
            .get(variant)
            .cloned()
    }
}

fn nominal_type_name(ty: &TypeKind) -> Option<&str> {
    match ty {
        TypeKind::Named(name) => Some(name),
        TypeKind::BorrowRef { inner, .. } | TypeKind::Shared(inner) => nominal_type_name(inner),
        _ => None,
    }
}

fn enum_constructor_path_parts(path: &str) -> (Option<&str>, &str) {
    let path = path.strip_prefix('.').unwrap_or(path);
    path.rsplit_once('.')
        .map_or((None, path), |(prefix, variant)| (Some(prefix), variant))
}
