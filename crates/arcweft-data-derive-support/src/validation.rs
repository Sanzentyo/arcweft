//! Shared attribute validation, independent of a generated trait's payload ABI.
use crate::attrs::{ContainerAttrs, FieldAttrs, ReprAttr, VariantAttrs};
use std::collections::BTreeMap;
use syn::{Expr, Ident, Lit};
pub fn validate_repr_discriminants<'a>(
    variants: impl IntoIterator<Item = &'a syn::Variant>,
    repr: &ReprAttr,
) -> syn::Result<()> {
    let mut errors = None;
    let (min, max) = repr.inclusive_i128_bounds();
    let mut next_value = 0_i128;
    for variant in variants {
        let value = match &variant.discriminant {
            Some((_, expr)) => {
                if let Some(value) = integer_discriminant(expr) {
                    value
                } else {
                    combine_error(
                        &mut errors,
                        syn::Error::new_spanned(
                            expr,
                            "Arcweft repr enum discriminants must be integer literals",
                        ),
                    );
                    continue;
                }
            }
            None => next_value,
        };
        if !(min..=max).contains(&value) {
            combine_error(
                &mut errors,
                syn::Error::new_spanned(
                    variant,
                    format!(
                        "Arcweft repr enum discriminant {value} is outside the selected repr range {min}..={max}"
                    ),
                ),
            );
        }
        next_value = value.saturating_add(1);
    }
    errors.map_or(Ok(()), Err)
}

fn integer_discriminant(expr: &Expr) -> Option<i128> {
    match expr {
        Expr::Lit(expr) => match &expr.lit {
            Lit::Int(value) => value.base10_parse::<i128>().ok(),
            _ => None,
        },
        Expr::Unary(expr) if matches!(expr.op, syn::UnOp::Neg(_)) => {
            let Expr::Lit(lit) = &*expr.expr else {
                return None;
            };
            let Lit::Int(value) = &lit.lit else {
                return None;
            };
            match value.base10_parse::<u128>().ok()? {
                magnitude if magnitude == (1_u128 << 127) => Some(i128::MIN),
                magnitude => i128::try_from(magnitude).ok().and_then(i128::checked_neg),
            }
        }
        _ => None,
    }
}

pub fn validate_field_wire_names(
    fields: &syn::FieldsNamed,
    container: &ContainerAttrs,
) -> syn::Result<()> {
    let mut seen = BTreeMap::<String, &Ident>::new();
    let mut errors = None;
    for field in &fields.named {
        let Some(ident) = field.ident.as_ref() else {
            continue;
        };
        match FieldAttrs::from_attrs(&field.attrs, ident, container.rename_all) {
            Ok(attrs) if attrs.skip => {}
            Ok(attrs) => {
                if matches!(container.tag_style(), crate::attrs::TagStyleAttr::Internal { tag } if tag == attrs.wire_name)
                {
                    combine_error(
                        &mut errors,
                        syn::Error::new_spanned(
                            ident,
                            "enum payload field duplicates the internal tag key",
                        ),
                    );
                }
                if let Some(previous) = seen.insert(attrs.wire_name.clone(), ident) {
                    combine_error(
                        &mut errors,
                        syn::Error::new_spanned(
                            ident,
                            format!(
                                "duplicate Arcweft wire name `{}` also used by `{previous}`",
                                attrs.wire_name
                            ),
                        ),
                    );
                }
            }
            Err(error) => combine_error(&mut errors, error),
        }
    }
    errors.map_or(Ok(()), Err)
}

pub fn validate_variant_wire_names<'a>(
    variants: impl IntoIterator<Item = &'a syn::Variant>,
    container: &ContainerAttrs,
) -> syn::Result<()> {
    let mut seen = BTreeMap::<String, &Ident>::new();
    let mut errors = None;
    for variant in variants {
        let ident = &variant.ident;
        match VariantAttrs::from_attrs(&variant.attrs, ident, container.rename_all) {
            Ok(attrs) => {
                if let Some(previous) = seen.insert(attrs.wire_name.clone(), ident) {
                    combine_error(
                        &mut errors,
                        syn::Error::new_spanned(
                            ident,
                            format!(
                                "duplicate Arcweft variant wire name `{}` also used by `{previous}`",
                                attrs.wire_name
                            ),
                        ),
                    );
                }
            }
            Err(error) => combine_error(&mut errors, error),
        }
    }
    errors.map_or(Ok(()), Err)
}

fn combine_error(errors: &mut Option<syn::Error>, error: syn::Error) {
    match errors {
        Some(existing) => existing.combine(error),
        None => *errors = Some(error),
    }
}
