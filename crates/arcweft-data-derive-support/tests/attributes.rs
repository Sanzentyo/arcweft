use arcweft_data_derive_support::{
    attrs::{BytesFormatAttr, ContainerAttrs, DefaultAttr, FieldAttrs, TagStyleAttr},
    rename::RenameRuleAttr,
    validation::{validate_field_wire_names, validate_repr_discriminants},
};
use syn::{Data, DeriveInput, Fields, parse_quote};

#[test]
fn source_attributes_preserve_flags_paths_and_resolved_wire_names() {
    let input: DeriveInput = parse_quote! {
        #[arcweft(rename_all = "camelCase", deny_unknown_fields)]
        struct Record {
            #[arcweft(default = "factory::make", bytes = "hex", rename = "wire", skip)]
            logical_name: Bytes,
        }
    };
    let attrs = ContainerAttrs::from_attrs(&input.attrs).unwrap();
    attrs.validate_for_input(&input).unwrap();
    let Data::Struct(data) = &input.data else {
        panic!()
    };
    let field = data.fields.iter().next().unwrap();
    let field = FieldAttrs::from_attrs(
        &field.attrs,
        field.ident.as_ref().unwrap(),
        attrs.rename_all,
    )
    .unwrap();
    assert_eq!(field.wire_name, "wire");
    assert!(field.skip && field.has_default());
    assert!(matches!(field.bytes_format, Some(BytesFormatAttr::Hex)));
    assert!(matches!(field.default(), DefaultAttr::Path(path) if path.segments.len() == 2));
    let input: DeriveInput =
        parse_quote! { struct Defaults { #[arcweft(default, bytes)] payload_value: Bytes } };
    let Data::Struct(data) = &input.data else {
        panic!()
    };
    let field = data.fields.iter().next().unwrap();
    let field = FieldAttrs::from_attrs(
        &field.attrs,
        field.ident.as_ref().unwrap(),
        RenameRuleAttr::CamelCase,
    )
    .unwrap();
    assert_eq!(field.wire_name, "payloadValue");
    assert!(matches!(field.default(), DefaultAttr::Trait));
    assert!(matches!(field.bytes_format, Some(BytesFormatAttr::Binary)));
}

#[test]
fn malformed_and_ambiguous_attributes_fail_at_the_shared_parser() {
    let bad: DeriveInput =
        parse_quote! { #[arcweft(rename_all = "invalid")] struct Record { value: bool } };
    assert!(ContainerAttrs::from_attrs(&bad.attrs).is_err());
    let bad: DeriveInput =
        parse_quote! { #[arcweft(tag = "same", content = "same")] enum Event { One } };
    assert!(
        ContainerAttrs::from_attrs(&bad.attrs)
            .unwrap()
            .validate_for_input(&bad)
            .is_err()
    );
    let bad: DeriveInput = parse_quote! { #[arcweft(content = "data")] enum Event { One } };
    assert!(
        ContainerAttrs::from_attrs(&bad.attrs)
            .unwrap()
            .validate_for_input(&bad)
            .is_err()
    );
    let bad: DeriveInput =
        parse_quote! { #[arcweft(tag = "kind")] enum Event { One { kind: bool } } };
    let attrs = ContainerAttrs::from_attrs(&bad.attrs).unwrap();
    assert!(matches!(attrs.tag_style(), TagStyleAttr::Internal { .. }));
    let Data::Enum(data) = bad.data else { panic!() };
    let Fields::Named(fields) = &data.variants[0].fields else {
        panic!()
    };
    assert!(validate_field_wire_names(fields, &attrs).is_err());
}

#[test]
fn resolved_field_collisions_and_repr_ranges_are_checked() {
    let bad: DeriveInput = parse_quote! { struct Record { #[arcweft(rename = "wire")] left: bool, #[arcweft(rename = "wire")] right: bool } };
    let attrs = ContainerAttrs::from_attrs(&bad.attrs).unwrap();
    let Data::Struct(data) = bad.data else {
        panic!()
    };
    let Fields::Named(fields) = data.fields else {
        panic!()
    };
    assert!(validate_field_wire_names(&fields, &attrs).is_err());
    let bad: DeriveInput =
        parse_quote! { #[arcweft(repr = "i8")] enum Number { Low = -129, High = 128 } };
    let attrs = ContainerAttrs::from_attrs(&bad.attrs).unwrap();
    let Data::Enum(data) = bad.data else { panic!() };
    assert!(
        validate_repr_discriminants(data.variants.iter(), attrs.repr.as_ref().unwrap()).is_err()
    );
}

#[test]
fn signed_minimum_discriminant_is_parsed_without_overflow() {
    let input: DeriveInput = parse_quote! {
        #[arcweft(repr = "i128")]
        enum Edge { Min = -170141183460469231731687303715884105728 }
    };
    let attrs = ContainerAttrs::from_attrs(&input.attrs).unwrap();
    let Data::Enum(data) = input.data else {
        panic!()
    };
    validate_repr_discriminants(data.variants.iter(), attrs.repr.as_ref().unwrap()).unwrap();
}
