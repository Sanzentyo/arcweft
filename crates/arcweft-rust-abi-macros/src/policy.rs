//! Typed projection from the shared Rust attribute grammar to ABI metadata.

use arcweft_data_derive_support::{
    attrs::{BytesFormatAttr, ContainerAttrs, IntegerRepr, TagStyleAttr},
    validation::{
        validate_field_wire_names, validate_repr_discriminants, validate_variant_wire_names,
    },
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

pub(super) fn parse(input: &DeriveInput) -> syn::Result<ContainerAttrs> {
    let attrs = ContainerAttrs::from_attrs(&input.attrs)?;
    attrs.validate_for_input(input)?;
    match &input.data {
        Data::Struct(data) => {
            if let Fields::Named(fields) = &data.fields {
                validate_field_wire_names(fields, &attrs)?;
            }
        }
        Data::Enum(data) => {
            validate_variant_wire_names(data.variants.iter(), &attrs)?;
            if let Some(repr) = &attrs.repr {
                if data
                    .variants
                    .iter()
                    .any(|variant| !matches!(variant.fields, Fields::Unit))
                {
                    return Err(syn::Error::new_spanned(
                        input,
                        "Arcweft repr enums must be C-like unit variants",
                    ));
                }
                validate_repr_discriminants(data.variants.iter(), repr)?;
            }
            for variant in &data.variants {
                match &variant.fields {
                    Fields::Named(fields) => validate_field_wire_names(fields, &attrs)?,
                    Fields::Unnamed(fields)
                        if matches!(attrs.tag_style(), TagStyleAttr::Internal { .. }) =>
                    {
                        return Err(syn::Error::new_spanned(
                            fields,
                            "internally tagged Arcweft enum variants must be unit or named-field variants",
                        ));
                    }
                    _ => {}
                }
            }
        }
        Data::Union(_) => {}
    }
    Ok(attrs)
}

pub(super) fn type_policy(attrs: &ContainerAttrs, name: &str) -> TokenStream {
    let deny = attrs.deny_unknown_fields;
    let tag = match attrs.tag_style() {
        TagStyleAttr::External => quote! { arcweft_rust_abi::ArcweftRustEnumTagStyle::External },
        TagStyleAttr::Internal { tag } => {
            quote! { arcweft_rust_abi::ArcweftRustEnumTagStyle::Internal { tag: #tag.to_owned() } }
        }
        TagStyleAttr::Adjacent { tag, content } => {
            quote! { arcweft_rust_abi::ArcweftRustEnumTagStyle::Adjacent { tag: #tag.to_owned(), content: #content.to_owned() } }
        }
    };
    let repr = attrs.repr.as_ref().map_or_else(
        || quote! { None },
        |repr| {
            let name = match repr.kind() {
                IntegerRepr::I8 => "I8",
                IntegerRepr::I16 => "I16",
                IntegerRepr::I32 => "I32",
                IntegerRepr::I64 => "I64",
                IntegerRepr::I128 => "I128",
                IntegerRepr::Isize => "Isize",
                IntegerRepr::U8 => "U8",
                IntegerRepr::U16 => "U16",
                IntegerRepr::U32 => "U32",
                IntegerRepr::U64 => "U64",
                IntegerRepr::U128 => "U128",
                IntegerRepr::Usize => "Usize",
            };
            let name = quote::format_ident!("{name}");
            quote! { Some(arcweft_rust_abi::ArcweftRustEnumRepr::#name) }
        },
    );
    quote! { arcweft_rust_abi::ArcweftRustDataTypePolicy { name: #name.to_owned(), deny_unknown_fields: #deny, tag: #tag, repr: #repr } }
}

pub(super) fn bytes_format(format: Option<BytesFormatAttr>) -> TokenStream {
    match format {
        None => quote! { None },
        Some(format) => {
            let name = match format {
                BytesFormatAttr::Binary => "Binary",
                BytesFormatAttr::Base64 => "Base64",
                BytesFormatAttr::Hex => "Hex",
                BytesFormatAttr::Array => "Array",
            };
            let name = quote::format_ident!("{name}");
            quote! { Some(arcweft_rust_abi::ArcweftRustBytesFormat::#name) }
        }
    }
}
