//! Procedural macros for opt-in Rust API metadata exported to Arcweft.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Fields, FnArg, Ident, ItemFn, LitStr, Pat, PatIdent, ReturnType, Type,
    parse_macro_input, spanned::Spanned,
};

/// Derives `arcweft_rust_abi::ArcweftTypeMetadata` for a Rust ADT.
#[proc_macro_derive(ArcweftType)]
pub fn derive_arcweft_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_arcweft_type(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Exports one Rust function as Arcweft callable metadata.
#[proc_macro_attribute]
pub fn arcweft_export(attr: TokenStream, item: TokenStream) -> TokenStream {
    let options = parse_export_options(attr);
    let function = parse_macro_input!(item as ItemFn);
    expand_arcweft_export(options, &function)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Default)]
struct ExportOptions {
    name: Option<String>,
    pure: bool,
    task: bool,
}

fn parse_export_options(attr: TokenStream) -> ExportOptions {
    let mut options = ExportOptions::default();
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("name") {
            let value = meta.value()?;
            options.name = Some(value.parse::<LitStr>()?.value());
            return Ok(());
        }
        if meta.path.is_ident("pure") {
            options.pure = true;
            return Ok(());
        }
        if meta.path.is_ident("task") {
            options.task = true;
            return Ok(());
        }
        Err(meta.error("unsupported arcweft_export option"))
    });
    let _ = syn::parse::Parser::parse(parser, attr);
    options
}

fn expand_arcweft_type(input: &DeriveInput) -> syn::Result<TokenStream2> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "ArcweftType metadata does not support generic ADTs yet",
        ));
    }
    let ident = &input.ident;
    let name = ident.to_string();
    let rust_path = quote! { concat!(module_path!(), "::", stringify!(#ident)) };
    let kind = match &input.data {
        Data::Struct(data) => expand_struct_kind(&data.fields)?,
        Data::Enum(data) => {
            let variants = data
                .variants
                .iter()
                .map(|variant| {
                    let variant_name = variant.ident.to_string();
                    let fields = expand_fields(&variant.fields)?;
                    Ok(quote! {
                        arcweft_rust_abi::ArcweftRustVariant {
                            name: #variant_name.to_owned(),
                            fields: #fields,
                        }
                    })
                })
                .collect::<syn::Result<Vec<_>>>()?;
            quote! {
                arcweft_rust_abi::ArcweftRustTypeKind::Enum {
                    variants: vec![#(#variants),*],
                }
            }
        }
        Data::Union(data) => {
            return Err(syn::Error::new(
                data.union_token.span(),
                "ArcweftType metadata does not support unions",
            ));
        }
    };

    Ok(quote! {
        impl arcweft_rust_abi::ArcweftType for #ident {
            fn arcweft_type_ref() -> arcweft_rust_abi::ArcweftRustTypeRef {
                arcweft_rust_abi::ArcweftRustTypeRef::Named {
                    name: #name.to_owned(),
                }
            }
        }

        impl arcweft_rust_abi::ArcweftTypeMetadata for #ident {
            fn arcweft_type_decl() -> arcweft_rust_abi::ArcweftRustTypeDecl {
                arcweft_rust_abi::ArcweftRustTypeDecl {
                    name: #name.to_owned(),
                    rust_path: #rust_path.to_owned(),
                    kind: #kind,
                }
            }
        }
    })
}

fn expand_struct_kind(fields: &Fields) -> syn::Result<TokenStream2> {
    if let Fields::Unnamed(fields) = fields
        && fields.unnamed.len() == 1
    {
        let ty = &fields.unnamed[0].ty;
        return Ok(quote! {
            arcweft_rust_abi::ArcweftRustTypeKind::Newtype {
                inner: <#ty as arcweft_rust_abi::ArcweftType>::arcweft_type_ref(),
            }
        });
    }
    let fields = expand_fields(fields)?;
    Ok(quote! {
        arcweft_rust_abi::ArcweftRustTypeKind::Struct {
            fields: #fields,
        }
    })
}

fn expand_fields(fields: &Fields) -> syn::Result<TokenStream2> {
    let expanded = fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let name = field
                .ident
                .as_ref()
                .map_or_else(|| index.to_string(), Ident::to_string);
            let ty = &field.ty;
            reject_unsupported_type(ty)?;
            Ok(quote! {
                arcweft_rust_abi::ArcweftRustField {
                    name: #name.to_owned(),
                    ty: <#ty as arcweft_rust_abi::ArcweftType>::arcweft_type_ref(),
                }
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(quote! { vec![#(#expanded),*] })
}

fn expand_arcweft_export(options: ExportOptions, function: &ItemFn) -> syn::Result<TokenStream2> {
    let ident = &function.sig.ident;
    let metadata_fn = format_ident!("__arcweft_export_{}_metadata", ident);
    let export_name = options.name.unwrap_or_else(|| ident.to_string());
    let rust_path = quote! { concat!(module_path!(), "::", stringify!(#ident)) };
    let params = function
        .sig
        .inputs
        .iter()
        .map(expand_fn_param)
        .collect::<syn::Result<Vec<_>>>()?;
    let return_type = match &function.sig.output {
        ReturnType::Default => quote! { arcweft_rust_abi::ArcweftRustTypeRef::Unit },
        ReturnType::Type(_, ty) => {
            reject_unsupported_type(ty)?;
            quote! { <#ty as arcweft_rust_abi::ArcweftType>::arcweft_type_ref() }
        }
    };
    let purity = if options.task {
        quote! { arcweft_rust_abi::ArcweftRustPurity::Task }
    } else if options.pure {
        quote! { arcweft_rust_abi::ArcweftRustPurity::Pure }
    } else {
        quote! { arcweft_rust_abi::ArcweftRustPurity::External }
    };
    let visibility = &function.vis;

    Ok(quote! {
        #function

        #visibility fn #metadata_fn() -> arcweft_rust_abi::ArcweftRustFunction {
            arcweft_rust_abi::ArcweftRustFunction {
                name: #export_name.to_owned(),
                rust_path: #rust_path.to_owned(),
                params: vec![#(#params),*],
                return_type: #return_type,
                purity: #purity,
                effects: Vec::new(),
            }
        }
    })
}

fn expand_fn_param(arg: &FnArg) -> syn::Result<TokenStream2> {
    match arg {
        FnArg::Receiver(receiver) => Err(syn::Error::new(
            receiver.span(),
            "arcweft_export functions cannot have a self receiver",
        )),
        FnArg::Typed(arg) => {
            let Pat::Ident(PatIdent { ident, .. }) = arg.pat.as_ref() else {
                return Err(syn::Error::new(
                    arg.pat.span(),
                    "arcweft_export parameters must be simple identifiers",
                ));
            };
            let name = ident.to_string();
            let ty = &arg.ty;
            reject_unsupported_type(ty)?;
            Ok(quote! {
                arcweft_rust_abi::ArcweftRustParam {
                    name: #name.to_owned(),
                    ty: <#ty as arcweft_rust_abi::ArcweftType>::arcweft_type_ref(),
                }
            })
        }
    }
}

fn reject_unsupported_type(ty: &Type) -> syn::Result<()> {
    match ty {
        Type::Reference(reference) => Err(syn::Error::new(
            reference.span(),
            "Arcweft Rust ABI metadata does not support borrowed references yet",
        )),
        Type::Ptr(pointer) => Err(syn::Error::new(
            pointer.span(),
            "Arcweft Rust ABI metadata does not support raw pointers",
        )),
        Type::Never(never) => Err(syn::Error::new(
            never.span(),
            "Arcweft Rust ABI metadata does not support never-returning exports",
        )),
        _ => Ok(()),
    }
}
