use arcweft_data_derive_support::validation::{
    validate_field_wire_names, validate_repr_discriminants, validate_variant_wire_names,
};
use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::Span;
use quote::quote;
use syn::{
    Data, DeriveInput, Fields, Generics, Ident, LitStr, Type, WherePredicate,
    visit::{self, Visit},
};

use arcweft_data_derive_support::attrs::{
    ContainerAttrs, FieldAttrs, ReprAttr, TagStyleAttr, VariantAttrs,
};

pub(crate) fn encode(input: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &input.ident;
    let container = match ContainerAttrs::from_attrs(&input.attrs) {
        Ok(attrs) => attrs,
        Err(error) => return error.to_compile_error(),
    };
    if let Err(error) = validate_input_attrs(input, &container) {
        return error.to_compile_error();
    }
    let generics = add_data_trait_bounds(
        input.generics.clone(),
        encode_bound_types(&input.data, &container),
        &quote!(::arcweft_data::Encode),
    );
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    match &input.data {
        Data::Struct(data) => encode_struct(
            name,
            &data.fields,
            &container,
            &impl_generics,
            &ty_generics,
            where_clause,
        ),
        Data::Enum(data) => encode_enum(
            name,
            data.variants.iter().collect(),
            &container,
            &impl_generics,
            &ty_generics,
            where_clause,
        ),
        Data::Union(_) => quote!(compile_error!("ArcweftEncode does not support unions");),
    }
}

pub(crate) fn decode(input: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &input.ident;
    let container = match ContainerAttrs::from_attrs(&input.attrs) {
        Ok(attrs) => attrs,
        Err(error) => return error.to_compile_error(),
    };
    if let Err(error) = validate_input_attrs(input, &container) {
        return error.to_compile_error();
    }
    let generics = add_data_trait_bounds(
        input.generics.clone(),
        decode_bound_types(&input.data, &container),
        &quote!(::arcweft_data::Decode),
    );
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    match &input.data {
        Data::Struct(data) => decode_struct(
            name,
            &data.fields,
            &container,
            &impl_generics,
            &ty_generics,
            where_clause,
        ),
        Data::Enum(data) => decode_enum(
            name,
            data.variants.iter(),
            &container,
            &impl_generics,
            &ty_generics,
            where_clause,
        ),
        Data::Union(_) => quote!(compile_error!("ArcweftDecode does not support unions");),
    }
}

pub(crate) fn reflect(input: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &input.ident;
    let type_name = name.to_string();
    let container = match ContainerAttrs::from_attrs(&input.attrs) {
        Ok(attrs) => attrs,
        Err(error) => return error.to_compile_error(),
    };
    if let Err(error) = validate_input_attrs(input, &container) {
        return error.to_compile_error();
    }
    let generics = add_reflect_generic_bounds(input.generics.clone(), &input.data);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    match &input.data {
        Data::Struct(data) => {
            let Fields::Named(fields) = &data.fields else {
                return quote!(compile_error!("ArcweftReflect currently supports named-field structs"););
            };
            let field_shapes = reflected_named_fields(fields, &container);
            let graph_field_shapes = registered_named_fields(fields, &container);
            let deny_unknown_fields = container.deny_unknown_fields;
            quote! {
                impl #impl_generics ::arcweft_data::Reflect for #name #ty_generics #where_clause {
                    fn shape() -> ::arcweft_data::TypeShape {
                        ::arcweft_data::TypeShape::Record {
                            name: #type_name.to_owned(),
                            fields: vec![#(#field_shapes),*],
                            policy: ::arcweft_data::RecordPolicy { deny_unknown_fields: #deny_unknown_fields },
                        }
                    }

                    fn register_shape(
                        builder: &mut ::arcweft_data::ShapeGraphBuilder,
                    ) -> ::arcweft_data::Result<::arcweft_data::ShapeId>
                    where
                        Self: 'static + Sized,
                    {
                        let (id, is_new) = builder.reserve_type::<Self>();
                        if !is_new {
                            return Ok(id);
                        }
                        let shape = ::arcweft_data::TypeShape::Record {
                            name: #type_name.to_owned(),
                            fields: vec![#(#graph_field_shapes),*],
                            policy: ::arcweft_data::RecordPolicy { deny_unknown_fields: #deny_unknown_fields },
                        };
                        builder.define(id, shape)?;
                        Ok(id)
                    }
                }
            }
        }
        Data::Enum(data) => {
            if container.repr.is_some()
                && data
                    .variants
                    .iter()
                    .any(|variant| !matches!(variant.fields, Fields::Unit))
            {
                return quote!(compile_error!("Arcweft repr enums must be C-like unit variants"););
            }
            let variants = data
                .variants
                .iter()
                .map(|variant| reflected_variant(variant, &type_name, &container));
            let graph_variants = data
                .variants
                .iter()
                .map(|variant| registered_variant(variant, &type_name, &container));
            let tag = container.tag_style().shape_tokens();
            let repr = container
                .repr
                .as_ref()
                .map_or_else(|| quote!(None), ReprAttr::shape_option_tokens);
            quote! {
                impl #impl_generics ::arcweft_data::Reflect for #name #ty_generics #where_clause {
                    fn shape() -> ::arcweft_data::TypeShape {
                        ::arcweft_data::TypeShape::Enum {
                            name: #type_name.to_owned(),
                            variants: vec![#(#variants),*],
                            tag: #tag,
                            repr: #repr,
                        }
                    }

                    fn register_shape(
                        builder: &mut ::arcweft_data::ShapeGraphBuilder,
                    ) -> ::arcweft_data::Result<::arcweft_data::ShapeId>
                    where
                        Self: 'static + Sized,
                    {
                        let (id, is_new) = builder.reserve_type::<Self>();
                        if !is_new {
                            return Ok(id);
                        }
                        let shape = ::arcweft_data::TypeShape::Enum {
                            name: #type_name.to_owned(),
                            variants: vec![#(#graph_variants),*],
                            tag: #tag,
                            repr: #repr,
                        };
                        builder.define(id, shape)?;
                        Ok(id)
                    }
                }
            }
        }
        Data::Union(_) => quote!(compile_error!("ArcweftReflect does not support unions");),
    }
}

fn encode_struct(
    name: &Ident,
    fields: &Fields,
    container: &ContainerAttrs,
    impl_generics: &syn::ImplGenerics<'_>,
    ty_generics: &syn::TypeGenerics<'_>,
    where_clause: Option<&syn::WhereClause>,
) -> proc_macro2::TokenStream {
    let Fields::Named(fields) = fields else {
        return quote!(compile_error!("ArcweftEncode currently supports named-field structs"););
    };
    let inserts = fields.named.iter().filter_map(|field| {
        let ident = field.ident.as_ref()?;
        let attrs = match FieldAttrs::from_attrs(&field.attrs, ident, container.rename_all) {
            Ok(attrs) => attrs,
            Err(error) => return Some(error.to_compile_error()),
        };
        if attrs.skip {
            return None;
        }
        let wire = attrs.wire_name;
        Some(quote! {
            fields.insert(#wire.to_owned(), ::arcweft_data::Encode::encode(&self.#ident).map_err(|err| err.at_field(#wire))?);
        })
    });
    quote! {
        impl #impl_generics ::arcweft_data::Encode for #name #ty_generics #where_clause {
            fn encode(&self) -> ::arcweft_data::Result<::arcweft_data::Value> {
                let mut fields = ::std::collections::BTreeMap::new();
                #(#inserts)*
                Ok(::arcweft_data::Value::Record(fields))
            }
        }
    }
}

// Encode and Decode preserve the typed enum carrier. Wire tags and numeric
// representations are applied exactly once by codecs using Reflect metadata.
fn encode_enum(
    name: &Ident,
    variants: Vec<&syn::Variant>,
    container: &ContainerAttrs,
    impl_generics: &syn::ImplGenerics<'_>,
    ty_generics: &syn::TypeGenerics<'_>,
    where_clause: Option<&syn::WhereClause>,
) -> proc_macro2::TokenStream {
    let arms = variants.into_iter().map(|variant| {
        let ident = &variant.ident;
        let wire = match VariantAttrs::from_attrs(&variant.attrs, ident, container.rename_all) {
            Ok(attrs) => attrs.wire_name,
            Err(error) => return error.to_compile_error(),
        };
        encode_enum_variant_arm(ident, &wire, &variant.fields, container)
    });
    quote! {
        impl #impl_generics ::arcweft_data::Encode for #name #ty_generics #where_clause {
            fn encode(&self) -> ::arcweft_data::Result<::arcweft_data::Value> {
                match self { #(#arms),* }
            }
        }
    }
}

fn encode_enum_variant_arm(
    ident: &Ident,
    wire: &str,
    fields: &Fields,
    container: &ContainerAttrs,
) -> proc_macro2::TokenStream {
    match fields {
        Fields::Unit => quote! {
            Self::#ident => Ok(::arcweft_data::Value::Enum { variant: #wire.to_owned(), payload: None })
        },
        Fields::Unnamed(fields) if fields.unnamed.is_empty() => quote! {
            Self::#ident() => Ok(::arcweft_data::Value::Enum {
                variant: #wire.to_owned(),
                payload: Some(Box::new(::arcweft_data::Value::Tuple(::std::vec::Vec::new()))),
            })
        },
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => quote! {
            Self::#ident(payload) => Ok(::arcweft_data::Value::Enum {
                variant: #wire.to_owned(),
                payload: Some(Box::new(::arcweft_data::Encode::encode(payload).map_err(|err| err.at_variant(#wire))?)),
            })
        },
        Fields::Named(fields) => {
            let bindings: Vec<&Ident> = fields
                .named
                .iter()
                .filter_map(|field| field.ident.as_ref())
                .collect();
            let insertions = fields.named.iter().filter_map(|field| {
                let field_ident = field.ident.as_ref()?;
                let attrs = match FieldAttrs::from_attrs(&field.attrs, field_ident, container.rename_all) {
                    Ok(attrs) => attrs,
                    Err(error) => return Some(error.to_compile_error()),
                };
                if attrs.skip { return None; }
                let wire_name = attrs.wire_name;
                Some(quote! {
                    record.insert(#wire_name.to_owned(), ::arcweft_data::Encode::encode(#field_ident).map_err(|err| err.at_field(#wire_name))?);
                })
            });
            quote! {
                Self::#ident { #(#bindings),* } => {
                    let mut record = ::std::collections::BTreeMap::new();
                    #(#insertions)*
                    Ok(::arcweft_data::Value::Enum {
                        variant: #wire.to_owned(),
                        payload: Some(Box::new(::arcweft_data::Value::Record(record))),
                    })
                }
            }
        }
        Fields::Unnamed(_) => quote! {
            Self::#ident(..) => Err(::arcweft_data::DataError::unsupported("multi-field tuple enum variants are not supported by Arcweft derives"))
        },
    }
}
fn decode_struct(
    name: &Ident,
    fields: &Fields,
    container: &ContainerAttrs,
    impl_generics: &syn::ImplGenerics<'_>,
    ty_generics: &syn::TypeGenerics<'_>,
    where_clause: Option<&syn::WhereClause>,
) -> proc_macro2::TokenStream {
    let Fields::Named(fields) = fields else {
        return quote!(compile_error!("ArcweftDecode currently supports named-field structs"););
    };
    let known_fields = known_named_fields(fields, container);
    let unknown_check = unknown_field_check(container.deny_unknown_fields, &known_fields);
    let initializers = fields.named.iter().filter_map(|field| {
        let ident = field.ident.as_ref()?;
        let attrs = match FieldAttrs::from_attrs(&field.attrs, ident, container.rename_all) {
            Ok(attrs) => attrs,
            Err(error) => return Some(error.to_compile_error()),
        };
        let has_default = attrs.has_default();
        let default = attrs.default_value_tokens();
        let wire = attrs.wire_name;
        if attrs.skip {
            Some(quote! {
                #ident: #default
            })
        } else if has_default {
            Some(quote! {
                #ident: match record.get(#wire) {
                    Some(value) => ::arcweft_data::Decode::decode(value).map_err(|err| err.at_field(#wire))?,
                    None => #default,
                }
            })
        } else {
            Some(quote! {
                #ident: match record.get(#wire) {
                    Some(value) => ::arcweft_data::Decode::decode(value).map_err(|err| err.at_field(#wire))?,
                    None => return Err(::arcweft_data::DataError::new(
                        ::arcweft_data::DataErrorKind::MissingField,
                        concat!("missing field ", #wire),
                    ).at_field(#wire)),
                }
            })
        }
    });
    quote! {
        impl #impl_generics ::arcweft_data::Decode for #name #ty_generics #where_clause {
            fn decode(value: &::arcweft_data::Value) -> ::arcweft_data::Result<Self> {
                let record = value.as_record()?;
                #unknown_check
                Ok(Self { #(#initializers),* })
            }
        }
    }
}

fn decode_enum<'a>(
    name: &Ident,
    variants: impl IntoIterator<Item = &'a syn::Variant>,
    container: &ContainerAttrs,
    impl_generics: &syn::ImplGenerics<'_>,
    ty_generics: &syn::TypeGenerics<'_>,
    where_clause: Option<&syn::WhereClause>,
) -> proc_macro2::TokenStream {
    let arms = variants.into_iter().map(|variant| {
        let ident = &variant.ident;
        let wire = match VariantAttrs::from_attrs(&variant.attrs, ident, container.rename_all) {
            Ok(attrs) => attrs.wire_name,
            Err(error) => return error.to_compile_error(),
        };
        decode_enum_variant_arm(ident, &wire, &variant.fields, container)
    });
    quote! {
        impl #impl_generics ::arcweft_data::Decode for #name #ty_generics #where_clause {
            fn decode(value: &::arcweft_data::Value) -> ::arcweft_data::Result<Self> {
                match value {
                    ::arcweft_data::Value::Enum { variant, payload } => match variant.as_str() {
                        #(#arms,)*
                        other => Err(::arcweft_data::DataError::new(
                            ::arcweft_data::DataErrorKind::InvalidEnumTag,
                            format!("unknown variant {other}"),
                        )),
                    },
                    other => Err(::arcweft_data::DataError::invalid_type("enum", other.type_name())),
                }
            }
        }
    }
}

fn decode_enum_variant_arm(
    ident: &Ident,
    wire: &str,
    fields: &Fields,
    container: &ContainerAttrs,
) -> proc_macro2::TokenStream {
    match fields {
        Fields::Unit => quote! {
            #wire => {
                if payload.is_some() {
                    return Err(::arcweft_data::DataError::new(
                        ::arcweft_data::DataErrorKind::UnknownField,
                        concat!("unexpected payload for unit variant ", #wire),
                    ).at_variant(#wire));
                }
                Ok(Self::#ident)
            }
        },
        Fields::Unnamed(fields) if fields.unnamed.is_empty() => quote! {
            #wire => {
                match payload.as_deref() {
                    Some(::arcweft_data::Value::Tuple(items)) if items.is_empty() => Ok(Self::#ident()),
                    _ => Err(::arcweft_data::DataError::new(
                        ::arcweft_data::DataErrorKind::InvalidType,
                        concat!("expected empty tuple payload for variant ", #wire),
                    ).at_variant(#wire)),
                }
            }
        },
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => quote! {
            #wire => {
                let payload = payload.as_deref().ok_or_else(|| ::arcweft_data::DataError::new(
                    ::arcweft_data::DataErrorKind::MissingField,
                    concat!("missing payload for variant ", #wire),
                ).at_variant(#wire))?;
                ::arcweft_data::Decode::decode(payload)
                    .map(Self::#ident)
                    .map_err(|err| err.at_variant(#wire))
            }
        },
        Fields::Named(fields) => decode_named_enum_variant_arm(ident, wire, fields, container),
        Fields::Unnamed(_) => quote! {
            #wire => Err(::arcweft_data::DataError::unsupported("multi-field tuple enum variants are not supported by Arcweft derives"))
        },
    }
}

fn decode_named_enum_variant_arm(
    ident: &Ident,
    wire: &str,
    fields: &syn::FieldsNamed,
    container: &ContainerAttrs,
) -> proc_macro2::TokenStream {
    let known_fields = known_named_fields(fields, container);
    let field_initializers = fields.named.iter().filter_map(|field| {
        let field_ident = field.ident.as_ref()?;
        let attrs = match FieldAttrs::from_attrs(&field.attrs, field_ident, container.rename_all) {
            Ok(attrs) => attrs,
            Err(error) => return Some(error.to_compile_error()),
        };
        let has_default = attrs.has_default();
        let default = attrs.default_value_tokens();
        let wire_name = attrs.wire_name;
        if attrs.skip {
            Some(quote! { #field_ident: #default })
        } else if has_default {
            Some(quote! {
                #field_ident: match record.get(#wire_name) {
                    Some(value) => ::arcweft_data::Decode::decode(value).map_err(|err| err.at_field(#wire_name))?,
                    None => #default,
                }
            })
        } else {
            Some(quote! {
                #field_ident: match record.get(#wire_name) {
                    Some(value) => ::arcweft_data::Decode::decode(value).map_err(|err| err.at_field(#wire_name))?,
                    None => return Err(::arcweft_data::DataError::new(
                        ::arcweft_data::DataErrorKind::MissingField,
                        concat!("missing field ", #wire_name),
                    ).at_variant(#wire).at_field(#wire_name)),
                }
            })
        }
    });
    let unknown_check = unknown_field_check(container.deny_unknown_fields, &known_fields);
    quote! {
        #wire => {
            let payload = payload.as_deref().ok_or_else(|| ::arcweft_data::DataError::new(
                ::arcweft_data::DataErrorKind::MissingField,
                concat!("missing payload for variant ", #wire),
            ).at_variant(#wire))?;
            let record = payload.as_record()?;
            #unknown_check
            Ok(Self::#ident { #(#field_initializers),* })
        }
    }
}
fn reflected_named_fields(
    fields: &syn::FieldsNamed,
    container: &ContainerAttrs,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .named
        .iter()
        .filter_map(|field| {
            let ident = field.ident.as_ref()?;
            let ty = &field.ty;
            let attrs = match FieldAttrs::from_attrs(&field.attrs, ident, container.rename_all) {
                Ok(attrs) => attrs,
                Err(error) => return Some(error.to_compile_error()),
            };
            let rust_name = ident.to_string();
            let default_call = attrs.has_default().then(|| quote!(.with_default()));
            let wire_name = attrs.wire_name;
            let skip_call = attrs.skip.then(|| quote!(.skipped()));
            let bytes_call = attrs
                .bytes_format
                .map(|format| quote!(.with_bytes_format(#format)));
            Some(quote! {
                ::arcweft_data::FieldShape::new(#rust_name, #wire_name, <#ty as ::arcweft_data::Reflect>::shape())
                    #default_call
                    #skip_call
                    #bytes_call
            })
        })
        .collect()
}

fn registered_named_fields(
    fields: &syn::FieldsNamed,
    container: &ContainerAttrs,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .named
        .iter()
        .filter_map(|field| {
            let ident = field.ident.as_ref()?;
            let ty = &field.ty;
            let attrs = match FieldAttrs::from_attrs(&field.attrs, ident, container.rename_all) {
                Ok(attrs) => attrs,
                Err(error) => return Some(error.to_compile_error()),
            };
            let rust_name = ident.to_string();
            let default_call = attrs.has_default().then(|| quote!(.with_default()));
            let wire_name = attrs.wire_name;
            let skip_call = attrs.skip.then(|| quote!(.skipped()));
            let bytes_call = attrs
                .bytes_format
                .map(|format| quote!(.with_bytes_format(#format)));
            Some(quote! {
                ::arcweft_data::FieldShape::new(
                    #rust_name,
                    #wire_name,
                    ::arcweft_data::TypeShape::Ref(
                        <#ty as ::arcweft_data::Reflect>::register_shape(builder)?,
                    ),
                )
                #default_call
                #skip_call
                #bytes_call
            })
        })
        .collect()
}

fn reflected_variant(
    variant: &syn::Variant,
    type_name: &str,
    container: &ContainerAttrs,
) -> proc_macro2::TokenStream {
    let ident = &variant.ident;
    let wire = match VariantAttrs::from_attrs(&variant.attrs, ident, container.rename_all) {
        Ok(attrs) => attrs.wire_name,
        Err(error) => return error.to_compile_error(),
    };
    let rust = ident.to_string();
    let discriminant = container
        .repr
        .as_ref()
        .map(|_| quote!(.with_discriminant(Self::#ident as i128)));
    match &variant.fields {
        Fields::Unnamed(fields) if fields.unnamed.is_empty() => quote!(
            ::arcweft_data::VariantShape::unit(#rust, #wire)
                .with_payload(::arcweft_data::TypeShape::Tuple(::std::vec::Vec::new()))
                #discriminant
        ),
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let ty = &fields.unnamed.first().expect("one field").ty;
            quote!(::arcweft_data::VariantShape::unit(#rust, #wire).with_payload(<#ty as ::arcweft_data::Reflect>::shape()) #discriminant)
        }
        Fields::Named(fields) => {
            let field_shapes = reflected_named_fields(fields, container);
            let record_name = format!("{type_name}::{rust}");
            let deny_unknown_fields = container.deny_unknown_fields;
            quote! {
                ::arcweft_data::VariantShape::unit(#rust, #wire)
                    .with_payload(::arcweft_data::TypeShape::Record {
                        name: #record_name.to_owned(),
                        fields: vec![#(#field_shapes),*],
                        policy: ::arcweft_data::RecordPolicy { deny_unknown_fields: #deny_unknown_fields },
                    })
                    #discriminant
            }
        }
        Fields::Unit | Fields::Unnamed(_) => {
            quote!(::arcweft_data::VariantShape::unit(#rust, #wire) #discriminant)
        }
    }
}

fn registered_variant(
    variant: &syn::Variant,
    type_name: &str,
    container: &ContainerAttrs,
) -> proc_macro2::TokenStream {
    let ident = &variant.ident;
    let wire = match VariantAttrs::from_attrs(&variant.attrs, ident, container.rename_all) {
        Ok(attrs) => attrs.wire_name,
        Err(error) => return error.to_compile_error(),
    };
    let rust = ident.to_string();
    let discriminant = container
        .repr
        .as_ref()
        .map(|_| quote!(.with_discriminant(Self::#ident as i128)));
    match &variant.fields {
        Fields::Unnamed(fields) if fields.unnamed.is_empty() => quote!(
            ::arcweft_data::VariantShape::unit(#rust, #wire)
                .with_payload(::arcweft_data::TypeShape::Tuple(::std::vec::Vec::new()))
                #discriminant
        ),
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let ty = &fields.unnamed.first().expect("one field").ty;
            quote!(
                ::arcweft_data::VariantShape::unit(#rust, #wire)
                    .with_payload(::arcweft_data::TypeShape::Ref(
                        <#ty as ::arcweft_data::Reflect>::register_shape(builder)?,
                    ))
                    #discriminant
            )
        }
        Fields::Named(fields) => {
            let field_shapes = registered_named_fields(fields, container);
            let record_name = format!("{type_name}::{rust}");
            let deny_unknown_fields = container.deny_unknown_fields;
            quote! {
                ::arcweft_data::VariantShape::unit(#rust, #wire)
                    .with_payload(::arcweft_data::TypeShape::Record {
                        name: #record_name.to_owned(),
                        fields: vec![#(#field_shapes),*],
                        policy: ::arcweft_data::RecordPolicy { deny_unknown_fields: #deny_unknown_fields },
                    })
                    #discriminant
            }
        }
        Fields::Unit | Fields::Unnamed(_) => {
            quote!(::arcweft_data::VariantShape::unit(#rust, #wire) #discriminant)
        }
    }
}

fn add_data_trait_bounds<'a>(
    mut generics: Generics,
    types: impl IntoIterator<Item = &'a Type>,
    bound: &proc_macro2::TokenStream,
) -> Generics {
    let where_clause = generics.make_where_clause();
    where_clause.predicates.extend(types.into_iter().map(|ty| {
        syn::parse2::<WherePredicate>(quote!(#ty: #bound))
            .expect("valid Arcweft derive where predicate")
    }));
    generics
}

fn validate_input_attrs(input: &DeriveInput, container: &ContainerAttrs) -> syn::Result<()> {
    let mut errors = None;
    if let Err(error) = container.validate_for_input(input) {
        combine_error(&mut errors, error);
    }
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => {
                if let Err(error) = validate_field_wire_names(fields, container) {
                    combine_error(&mut errors, error);
                }
            }
            Fields::Unnamed(fields) => combine_error(
                &mut errors,
                syn::Error::new_spanned(
                    fields,
                    "Arcweft derives support named-field structs only; tuple structs require a manual implementation",
                ),
            ),
            Fields::Unit => combine_error(
                &mut errors,
                syn::Error::new_spanned(
                    &input.ident,
                    "Arcweft derives support named-field structs only; unit structs require a manual implementation",
                ),
            ),
        },
        Data::Enum(data) => {
            if let Err(error) = validate_variant_wire_names(data.variants.iter(), container) {
                combine_error(&mut errors, error);
            }
            if let Err(error) = validate_enum_variant_policy(data.variants.iter(), container) {
                combine_error(&mut errors, error);
            }
            if let Some(repr) = &container.repr
                && let Err(error) = validate_repr_discriminants(data.variants.iter(), repr)
            {
                combine_error(&mut errors, error);
            }
            for variant in &data.variants {
                if let Fields::Named(fields) = &variant.fields
                    && let Err(error) = validate_field_wire_names(fields, container)
                {
                    combine_error(&mut errors, error);
                }
            }
        }
        Data::Union(_) => {}
    }
    errors.map_or(Ok(()), Err)
}

fn encode_bound_types<'a>(data: &'a Data, container: &ContainerAttrs) -> Vec<&'a Type> {
    match data {
        Data::Struct(data) => encode_field_bound_types(&data.fields, container).collect(),
        Data::Enum(data) => data
            .variants
            .iter()
            .flat_map(|variant| encode_field_bound_types(&variant.fields, container))
            .collect(),
        Data::Union(_) => Vec::new(),
    }
}

fn decode_bound_types<'a>(data: &'a Data, container: &ContainerAttrs) -> Vec<&'a Type> {
    match data {
        Data::Struct(data) => decode_field_bound_types(&data.fields, container).collect(),
        Data::Enum(data) => data
            .variants
            .iter()
            .flat_map(|variant| decode_field_bound_types(&variant.fields, container))
            .collect(),
        Data::Union(_) => Vec::new(),
    }
}

fn reflect_bound_types(data: &Data) -> Vec<&Type> {
    match data {
        Data::Struct(data) => field_types(&data.fields).collect(),
        Data::Enum(data) => data
            .variants
            .iter()
            .flat_map(|variant| field_types(&variant.fields))
            .collect(),
        Data::Union(_) => Vec::new(),
    }
}

fn add_reflect_generic_bounds(mut generics: Generics, data: &Data) -> Generics {
    let generic_params: BTreeMap<String, Ident> = generics
        .type_params()
        .map(|parameter| (parameter.ident.to_string(), parameter.ident.clone()))
        .collect();
    let generic_names = generic_params.keys().cloned().collect::<BTreeSet<_>>();
    let mut collector = ReflectGenericUseCollector {
        generic_names: &generic_names,
        used: BTreeSet::new(),
    };
    for ty in reflect_bound_types(data) {
        collector.visit_type(ty);
    }
    if !collector.used.is_empty() {
        let where_clause = generics.make_where_clause();
        where_clause.predicates.extend(
            collector
                .used
                .into_iter()
                .filter_map(|name| generic_params.get(&name))
                .map(|ident| {
                    syn::parse2::<WherePredicate>(quote!(#ident: ::arcweft_data::Reflect))
                        .expect("valid Arcweft derive generic bound")
                }),
        );
    }
    generics
}

struct ReflectGenericUseCollector<'a> {
    generic_names: &'a BTreeSet<String>,
    used: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for ReflectGenericUseCollector<'_> {
    fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
        if ty.qself.is_none() && ty.path.segments.len() == 1 {
            let name = ty.path.segments[0].ident.to_string();
            if self.generic_names.contains(&name) {
                self.used.insert(name);
                return;
            }
        }
        visit::visit_type_path(self, ty);
    }
}

fn encode_field_bound_types<'a>(
    fields: &'a Fields,
    container: &ContainerAttrs,
) -> impl Iterator<Item = &'a Type> {
    field_types_with_skip(fields, container).filter_map(|(ty, skip)| (!skip).then_some(ty))
}

fn decode_field_bound_types<'a>(
    fields: &'a Fields,
    container: &ContainerAttrs,
) -> impl Iterator<Item = &'a Type> {
    field_types_with_skip(fields, container).filter_map(|(ty, skip)| (!skip).then_some(ty))
}

fn field_types_with_skip<'a>(
    fields: &'a Fields,
    container: &ContainerAttrs,
) -> impl Iterator<Item = (&'a Type, bool)> {
    fields.iter().map(|field| {
        let skip = field
            .ident
            .as_ref()
            .and_then(|ident| {
                FieldAttrs::from_attrs(&field.attrs, ident, container.rename_all).ok()
            })
            .is_some_and(|attrs| attrs.skip);
        (&field.ty, skip)
    })
}

fn field_types(fields: &Fields) -> impl Iterator<Item = &Type> {
    fields.iter().map(|field| &field.ty)
}

fn validate_enum_variant_policy<'a>(
    variants: impl IntoIterator<Item = &'a syn::Variant>,
    container: &ContainerAttrs,
) -> syn::Result<()> {
    let mut errors = None;
    let tag_style = container.tag_style();
    for variant in variants {
        match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() > 1 => combine_error(
                &mut errors,
                syn::Error::new_spanned(
                    fields,
                    "multi-field tuple enum variants are not supported by Arcweft derives; use a named-field variant or manual implementation",
                ),
            ),
            Fields::Unnamed(fields) if matches!(tag_style, TagStyleAttr::Internal { .. }) => {
                combine_error(
                    &mut errors,
                    syn::Error::new_spanned(
                        fields,
                        "internally tagged Arcweft enum variants must be unit or named-field variants",
                    ),
                );
            }
            Fields::Unnamed(_) | Fields::Named(_) | Fields::Unit => {}
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

fn known_named_fields(fields: &syn::FieldsNamed, container: &ContainerAttrs) -> Vec<String> {
    fields
        .named
        .iter()
        .filter_map(|field| {
            let ident = field.ident.as_ref()?;
            let Ok(attrs) = FieldAttrs::from_attrs(&field.attrs, ident, container.rename_all)
            else {
                return None;
            };
            (!attrs.skip).then_some(attrs.wire_name)
        })
        .collect()
}

fn unknown_field_check(
    deny_unknown_fields: bool,
    known_fields: &[String],
) -> proc_macro2::TokenStream {
    if !deny_unknown_fields {
        return quote!();
    }
    let known = known_fields
        .iter()
        .map(|field| LitStr::new(field, Span::call_site()));
    quote! {
        {
            let known_fields: &[&str] = &[#(#known),*];
            for field in record.keys() {
                if !known_fields.contains(&field.as_str()) {
                    return Err(::arcweft_data::DataError::new(
                        ::arcweft_data::DataErrorKind::UnknownField,
                        format!("unknown field {field}"),
                    ).at_field(field.clone()));
                }
            }
        }
    }
}
