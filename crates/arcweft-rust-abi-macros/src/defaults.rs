//! Default declaration provenance. These expansions inspect metadata and emit
//! callable code; none executes a user's default while collecting metadata.

use super::{ExportOptions, TokenStream2, expand_arcweft_export};
use quote::{format_ident, quote};
use syn::{ItemFn, ItemImpl, parse_quote, spanned::Spanned};

pub(super) struct DefaultExport {
    visibility: syn::Visibility,
    signature: syn::Signature,
}

impl syn::parse::Parse for DefaultExport {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let purity: syn::Ident = input.parse()?;
        if purity != "pure" {
            return Err(syn::Error::new_spanned(
                purity,
                "a Default wrapper requires an explicit `pure` contract",
            ));
        }
        input.parse::<syn::Token![,]>()?;
        let visibility = input.parse()?;
        let signature: syn::Signature = input.parse()?;
        if !signature.inputs.is_empty() {
            return Err(syn::Error::new_spanned(
                &signature.inputs,
                "a Default wrapper must have no parameters",
            ));
        }
        Ok(Self {
            visibility,
            signature,
        })
    }
}

pub(super) fn expand_default_export(declaration: DefaultExport) -> syn::Result<TokenStream2> {
    let DefaultExport {
        visibility,
        signature,
    } = declaration;
    let syn::ReturnType::Type(_, ty) = &signature.output else {
        return Err(syn::Error::new_spanned(
            &signature,
            "a Default wrapper requires an explicit result type",
        ));
    };
    let function: ItemFn = parse_quote! {
        #visibility #signature { <#ty as ::core::default::Default>::default() }
    };
    expand_arcweft_export(
        ExportOptions {
            pure: true,
            default_constructor: true,
            ..ExportOptions::default()
        },
        &function,
    )
}

pub(super) fn field_default(
    field: &syn::Field,
    attrs: &arcweft_data_derive_support::attrs::FieldAttrs,
) -> syn::Result<TokenStream2> {
    use arcweft_data_derive_support::attrs::DefaultAttr;
    Ok(match attrs.default() {
        DefaultAttr::None => quote! { None },
        DefaultAttr::Trait => quote! { Some(arcweft_rust_abi::ArcweftRustFieldDefault::Trait) },
        DefaultAttr::Path(path) => {
            let mut metadata = path.clone();
            let segment = metadata
                .segments
                .last_mut()
                .ok_or_else(|| syn::Error::new_spanned(path, "default function path is empty"))?;
            if !matches!(segment.arguments, syn::PathArguments::None) {
                return Err(syn::Error::new_spanned(
                    path,
                    "default functions must be registered concrete exports",
                ));
            }
            segment.ident = format_ident!("__arcweft_export_{}_metadata", segment.ident);
            let ty = &field.ty;
            quote! {{
                let _: fn() -> #ty = #path;
                Some(arcweft_rust_abi::ArcweftRustFieldDefault::Function {
                    rust_path: #metadata().rust_path,
                })
            }}
        }
    })
}
pub(super) fn expand_default_impl(
    mut options: ExportOptions,
    implementation: &ItemImpl,
) -> syn::Result<TokenStream2> {
    if !options.pure || options.task {
        return Err(syn::Error::new_spanned(
            implementation,
            "an exported Default implementation requires an explicit `pure` contract",
        ));
    }
    if !implementation.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &implementation.generics,
            "register a concrete Default implementation for each exported result type",
        ));
    }
    if !implementation
        .trait_
        .as_ref()
        .is_some_and(|(negative, path, _)| {
            negative.is_none()
                && path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "Default")
        })
    {
        return Err(syn::Error::new_spanned(
            implementation,
            "only Default implementations may publish a default constructor",
        ));
    }
    let name = options.name.as_ref().ok_or_else(|| {
        syn::Error::new(
            implementation.span(),
            "an exported Default implementation requires `name = \"callable_name\"`",
        )
    })?;
    let ident: syn::Ident = syn::parse_str(name).map_err(|_| {
        syn::Error::new(
            implementation.span(),
            "the default wrapper name must be a Rust identifier",
        )
    })?;
    let ty = &implementation.self_ty;
    let function: ItemFn = parse_quote! {
        pub fn #ident() -> #ty {
            <#ty as ::core::default::Default>::default()
        }
    };
    options.default_constructor = true;
    let exported = expand_arcweft_export(options, &function)?;
    Ok(quote! { #implementation #exported })
}
