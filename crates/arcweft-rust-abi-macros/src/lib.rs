//! Procedural macros for opt-in Rust API metadata exported to Arcweft.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::{
    Data, DeriveInput, Fields, FnArg, GenericArgument, GenericParam, Ident, ItemFn, LitStr, Pat,
    PatIdent, PathArguments, ReturnType, Type, parse_macro_input, parse_quote, spanned::Spanned,
};

/// Derives `arcweft_rust_abi::ArcweftTypeMetadata` for a Rust ADT.
#[proc_macro_derive(ArcweftType)]
pub fn derive_arcweft_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_arcweft_type(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Exports one non-generic Rust function as Arcweft callable metadata.
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

struct TypeParameters {
    ordered: Vec<(String, usize)>,
    by_name: BTreeMap<String, usize>,
}

impl TypeParameters {
    fn get(&self, name: &str) -> Option<&usize> {
        self.by_name.get(name)
    }
}

fn parse_export_options(attr: TokenStream) -> ExportOptions {
    let mut options = ExportOptions::default();
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("name") {
            options.name = Some(meta.value()?.parse::<LitStr>()?.value());
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
    let parameter_indices = type_parameter_indices(input)?;
    let mut bounded_generics = input.generics.clone();
    for parameter in &mut bounded_generics.params {
        if let GenericParam::Type(parameter) = parameter {
            parameter
                .bounds
                .push(parse_quote!(arcweft_rust_abi::ArcweftType));
        }
    }
    let (impl_generics, type_generics, where_clause) = bounded_generics.split_for_impl();
    let ident = &input.ident;
    let name = ident.to_string();
    let package = package_id_expression();
    let path = type_path_expression(&name);
    let rust_path = quote! { concat!(module_path!(), "::", stringify!(#ident)) };
    let arguments = parameter_indices.ordered.iter().map(|(parameter, _)| {
        let ident = Ident::new(parameter, input.ident.span());
        quote! { <#ident as arcweft_rust_abi::ArcweftType>::arcweft_type_ref() }
    });
    let parameters = parameter_indices.ordered.iter().map(|(parameter, index)| {
        quote! {
            arcweft_rust_abi::ArcweftRustTypeParameter {
                index: arcweft_rust_abi::ArcweftRustTypeParameterIndex::try_from_usize(#index)
                    .expect("macro-generated type parameter index is bounded"),
                name: arcweft_rust_abi::ArcweftRustTypeParameterName::try_new(#parameter)
                    .expect("Rust type parameter names are valid identifiers"),
            }
        }
    });
    let kind = match &input.data {
        Data::Struct(data) => expand_struct_kind(&data.fields, &parameter_indices)?,
        Data::Enum(data) => expand_enum_kind(data, &parameter_indices)?,
        Data::Union(data) => {
            return Err(syn::Error::new(
                data.union_token.span(),
                "ArcweftType metadata does not support unions",
            ));
        }
    };

    Ok(quote! {
        impl #impl_generics arcweft_rust_abi::ArcweftType for #ident #type_generics #where_clause {
            fn arcweft_type_ref() -> arcweft_rust_abi::ArcweftRustTypeRef {
                arcweft_rust_abi::ArcweftRustTypeRef::Nominal {
                    package: #package,
                    path: #path,
                    arguments: vec![#(#arguments),*],
                }
            }
        }

        impl #impl_generics arcweft_rust_abi::ArcweftTypeMetadata for #ident #type_generics #where_clause {
            fn arcweft_type_decl() -> arcweft_rust_abi::ArcweftRustTypeDecl {
                arcweft_rust_abi::ArcweftRustTypeDecl {
                    path: #path,
                    rust_path: #rust_path.to_owned(),
                    parameters: vec![#(#parameters),*],
                    kind: #kind,
                }
            }
        }
    })
}

fn type_parameter_indices(input: &DeriveInput) -> syn::Result<TypeParameters> {
    let mut ordered = Vec::new();
    let mut by_name = BTreeMap::new();
    let mut ordinal = 0_usize;
    for parameter in &input.generics.params {
        match parameter {
            GenericParam::Type(parameter) => {
                let name = parameter.ident.to_string();
                ordered.push((name.clone(), ordinal));
                by_name.insert(name, ordinal);
                ordinal = ordinal.saturating_add(1);
            }
            GenericParam::Lifetime(parameter) => {
                return Err(syn::Error::new(
                    parameter.span(),
                    "ArcweftType metadata does not support lifetime generic ADTs",
                ));
            }
            GenericParam::Const(parameter) => {
                return Err(syn::Error::new(
                    parameter.span(),
                    "ArcweftType metadata does not support const generic ADTs",
                ));
            }
        }
    }
    Ok(TypeParameters { ordered, by_name })
}

fn expand_struct_kind(fields: &Fields, parameters: &TypeParameters) -> syn::Result<TokenStream2> {
    match fields {
        Fields::Unit => Ok(quote! {
            arcweft_rust_abi::ArcweftRustTypeKind::Struct {
                shape: arcweft_rust_abi::ArcweftRustStructShape::Unit,
            }
        }),
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let inner = expand_metadata_type_ref(&fields.unnamed[0].ty, parameters)?;
            Ok(quote! {
                arcweft_rust_abi::ArcweftRustTypeKind::Newtype { inner: #inner }
            })
        }
        Fields::Unnamed(fields) => {
            let fields = fields
                .unnamed
                .iter()
                .map(|field| expand_metadata_type_ref(&field.ty, parameters))
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! {
                arcweft_rust_abi::ArcweftRustTypeKind::Struct {
                    shape: arcweft_rust_abi::ArcweftRustStructShape::Tuple {
                        fields: vec![#(#fields),*],
                    },
                }
            })
        }
        Fields::Named(fields) => {
            let fields = expand_record_fields(&fields.named, parameters)?;
            Ok(quote! {
                arcweft_rust_abi::ArcweftRustTypeKind::Struct {
                    shape: arcweft_rust_abi::ArcweftRustStructShape::Record {
                        fields: vec![#(#fields),*],
                    },
                }
            })
        }
    }
}

fn expand_enum_kind(
    data: &syn::DataEnum,
    parameters: &TypeParameters,
) -> syn::Result<TokenStream2> {
    let variants = data
        .variants
        .iter()
        .map(|variant| {
            let name = variant.ident.to_string();
            let payload = match &variant.fields {
                Fields::Unit => quote! { arcweft_rust_abi::ArcweftRustVariantPayload::Unit },
                Fields::Unnamed(fields) => {
                    let fields = fields
                        .unnamed
                        .iter()
                        .map(|field| expand_metadata_type_ref(&field.ty, parameters))
                        .collect::<syn::Result<Vec<_>>>()?;
                    quote! {
                        arcweft_rust_abi::ArcweftRustVariantPayload::Tuple {
                            fields: vec![#(#fields),*],
                        }
                    }
                }
                Fields::Named(fields) => {
                    let fields = expand_record_fields(&fields.named, parameters)?;
                    quote! {
                        arcweft_rust_abi::ArcweftRustVariantPayload::Record {
                            fields: vec![#(#fields),*],
                        }
                    }
                }
            };
            Ok(quote! {
                arcweft_rust_abi::ArcweftRustVariant {
                    name: #name.to_owned(),
                    payload: #payload,
                }
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(quote! {
        arcweft_rust_abi::ArcweftRustTypeKind::Enum {
            variants: vec![#(#variants),*],
        }
    })
}

fn expand_record_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    parameters: &TypeParameters,
) -> syn::Result<Vec<TokenStream2>> {
    fields
        .iter()
        .map(|field| {
            let name = field
                .ident
                .as_ref()
                .expect("named field collection contains identifiers")
                .to_string();
            let ty = expand_metadata_type_ref(&field.ty, parameters)?;
            Ok(quote! {
                arcweft_rust_abi::ArcweftRustField {
                    name: #name.to_owned(),
                    ty: #ty,
                }
            })
        })
        .collect()
}

fn expand_metadata_type_ref(ty: &Type, parameters: &TypeParameters) -> syn::Result<TokenStream2> {
    reject_unsupported_type(ty)?;
    match ty {
        Type::Tuple(tuple) => {
            let items = tuple
                .elems
                .iter()
                .map(|item| expand_metadata_type_ref(item, parameters))
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! {
                arcweft_rust_abi::ArcweftRustTypeRef::Tuple {
                    items: vec![#(#items),*],
                }
            })
        }
        Type::Path(path) if path.qself.is_none() => expand_path_type_ref(ty, path, parameters),
        _ => Ok(quote! {
            <#ty as arcweft_rust_abi::ArcweftType>::arcweft_type_ref()
        }),
    }
}

fn expand_path_type_ref(
    ty: &Type,
    path: &syn::TypePath,
    parameters: &TypeParameters,
) -> syn::Result<TokenStream2> {
    if path.path.segments.len() == 1 {
        let segment = path.path.segments.first().expect("one segment");
        if matches!(segment.arguments, PathArguments::None)
            && let Some(index) = parameters.get(&segment.ident.to_string())
        {
            return Ok(quote! {
                arcweft_rust_abi::ArcweftRustTypeRef::TypeParameter {
                    index: arcweft_rust_abi::ArcweftRustTypeParameterIndex::try_from_usize(#index)
                        .expect("macro-generated type parameter index is bounded"),
                }
            });
        }
    }

    let segment = path
        .path
        .segments
        .last()
        .expect("Rust type paths are non-empty");
    let arguments = type_arguments(segment)?;
    match (segment.ident.to_string().as_str(), arguments.as_slice()) {
        ("Vec", [item]) => wrap_one("Vec", item, parameters),
        ("Option", [item]) => wrap_one("Option", item, parameters),
        ("Result", [ok, error]) => {
            let ok = expand_metadata_type_ref(ok, parameters)?;
            let error = expand_metadata_type_ref(error, parameters)?;
            Ok(quote! {
                arcweft_rust_abi::ArcweftRustTypeRef::Result {
                    ok: Box::new(#ok),
                    error: Box::new(#error),
                }
            })
        }
        (_, []) => Ok(quote! {
            <#ty as arcweft_rust_abi::ArcweftType>::arcweft_type_ref()
        }),
        _ => {
            let arguments = arguments
                .iter()
                .map(|argument| expand_metadata_type_ref(argument, parameters))
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! {{
                let __arcweft_constructor =
                    <#ty as arcweft_rust_abi::ArcweftType>::arcweft_type_ref();
                match __arcweft_constructor {
                    arcweft_rust_abi::ArcweftRustTypeRef::Nominal {
                        package,
                        path,
                        ..
                    } => arcweft_rust_abi::ArcweftRustTypeRef::Nominal {
                        package,
                        path,
                        arguments: vec![#(#arguments),*],
                    },
                    _ => panic!("generic ArcweftType constructor must be nominal"),
                }
            }})
        }
    }
}

fn wrap_one(wrapper: &str, item: &Type, parameters: &TypeParameters) -> syn::Result<TokenStream2> {
    let item = expand_metadata_type_ref(item, parameters)?;
    match wrapper {
        "Vec" => Ok(quote! {
            arcweft_rust_abi::ArcweftRustTypeRef::Vec { item: Box::new(#item) }
        }),
        "Option" => Ok(quote! {
            arcweft_rust_abi::ArcweftRustTypeRef::Option { item: Box::new(#item) }
        }),
        _ => unreachable!("wrapper is selected by an exhaustive caller"),
    }
}

fn type_arguments(segment: &syn::PathSegment) -> syn::Result<Vec<Type>> {
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Ok(Vec::new());
    };
    arguments
        .args
        .iter()
        .map(|argument| match argument {
            GenericArgument::Type(ty) => Ok(ty.clone()),
            _ => Err(syn::Error::new(
                argument.span(),
                "ArcweftType metadata supports only type generic arguments",
            )),
        })
        .collect()
}

fn package_id_expression() -> TokenStream2 {
    quote! {
        arcweft_rust_abi::ArcweftRustPackageId::try_new(env!("CARGO_PKG_NAME"))
            .expect("Cargo package IDs are valid Arcweft ABI IDs")
    }
}

fn type_path_expression(name: &str) -> TokenStream2 {
    quote! {
        arcweft_rust_abi::ArcweftRustTypePath::try_new([
            arcweft_rust_abi::ArcweftRustTypePathSegment::try_new(#name)
                .expect("macro-generated Rust type path segment is valid"),
        ])
        .expect("macro-generated Rust type path is non-empty")
    }
}

fn expand_arcweft_export(options: ExportOptions, function: &ItemFn) -> syn::Result<TokenStream2> {
    if !function.sig.generics.params.is_empty() {
        return Err(syn::Error::new(
            function.sig.generics.span(),
            "arcweft_export does not support generic functions",
        ));
    }
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
