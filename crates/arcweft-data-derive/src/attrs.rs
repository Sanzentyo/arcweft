use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Ident, Lit};

use crate::rename::RenameRuleAttr;

#[derive(Clone, Copy)]
enum AttrName {
    Arcweft,
    Bytes,
    Content,
    Default,
    DenyUnknownFields,
    Rename,
    RenameAll,
    Repr,
    Skip,
    Tag,
}

impl AttrName {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Arcweft => "arcweft",
            Self::Bytes => "bytes",
            Self::Content => "content",
            Self::Default => "default",
            Self::DenyUnknownFields => "deny_unknown_fields",
            Self::Rename => "rename",
            Self::RenameAll => "rename_all",
            Self::Repr => "repr",
            Self::Skip => "skip",
            Self::Tag => "tag",
        }
    }
}

#[derive(Default)]
pub(crate) struct ContainerAttrs {
    pub(crate) rename_all: RenameRuleAttr,
    pub(crate) deny_unknown_fields: bool,
    tag: Option<String>,
    content: Option<String>,
    pub(crate) repr: Option<ReprAttr>,
}

impl ContainerAttrs {
    pub(crate) fn from_attrs(attrs: &[Attribute]) -> Self {
        let mut out = Self {
            rename_all: RenameRuleAttr::None,
            deny_unknown_fields: true,
            tag: None,
            content: None,
            repr: None,
        };
        attrs
            .iter()
            .filter(|attr| attr.path().is_ident(AttrName::Arcweft.as_str()))
            .for_each(|attr| {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident(AttrName::RenameAll.as_str()) {
                        let value = meta.value()?.parse::<Lit>()?;
                        if let Lit::Str(value) = value {
                            out.rename_all = RenameRuleAttr::parse(&value.value());
                        }
                        return Ok(());
                    }
                    if meta.path.is_ident(AttrName::DenyUnknownFields.as_str()) {
                        out.deny_unknown_fields = true;
                        return Ok(());
                    }
                    if meta.path.is_ident(AttrName::Tag.as_str()) {
                        let value = meta.value()?.parse::<Lit>()?;
                        if let Lit::Str(value) = value {
                            out.tag = Some(value.value());
                        }
                        return Ok(());
                    }
                    if meta.path.is_ident(AttrName::Content.as_str()) {
                        let value = meta.value()?.parse::<Lit>()?;
                        if let Lit::Str(value) = value {
                            out.content = Some(value.value());
                        }
                        return Ok(());
                    }
                    if meta.path.is_ident(AttrName::Repr.as_str()) {
                        let value = meta.value()?.parse::<Lit>()?;
                        if let Lit::Str(value) = value {
                            out.repr = ReprAttr::parse(&value.value());
                        }
                        return Ok(());
                    }
                    Ok(())
                });
            });
        out
    }

    pub(crate) fn tag_style(&self) -> TagStyleAttr {
        match (&self.tag, &self.content) {
            (Some(tag), Some(content)) => TagStyleAttr::Adjacent {
                tag: tag.clone(),
                content: content.clone(),
            },
            (Some(tag), None) => TagStyleAttr::Internal { tag: tag.clone() },
            (None, _) => TagStyleAttr::External,
        }
    }
}

#[derive(Clone)]
pub(crate) enum TagStyleAttr {
    External,
    Internal { tag: String },
    Adjacent { tag: String, content: String },
}

impl TagStyleAttr {
    pub(crate) fn shape_tokens(&self) -> TokenStream {
        match self {
            Self::External => quote!(::arcweft_data::EnumTagStyle::External),
            Self::Internal { tag } => {
                quote!(::arcweft_data::EnumTagStyle::Internal { tag: #tag.to_owned() })
            }
            Self::Adjacent { tag, content } => quote!(
                ::arcweft_data::EnumTagStyle::Adjacent {
                    tag: #tag.to_owned(),
                    content: #content.to_owned(),
                }
            ),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ReprAttr {
    kind: IntegerRepr,
}

impl ReprAttr {
    fn parse(value: &str) -> Option<Self> {
        IntegerRepr::parse(value).map(|kind| Self { kind })
    }

    pub(crate) fn ty_tokens(&self) -> TokenStream {
        self.kind.ty_tokens()
    }

    pub(crate) fn number_value_tokens(&self, value: &TokenStream) -> TokenStream {
        if self.kind.is_signed() {
            quote!(::arcweft_data::Number::I((#value) as i128))
        } else {
            quote!(::arcweft_data::Number::U((#value) as u128))
        }
    }

    pub(crate) fn numeric_decode_tokens(&self) -> TokenStream {
        let ty = self.ty_tokens();
        quote! {
            |value: &::arcweft_data::Value| -> ::arcweft_data::Result<#ty> {
                match value {
                    ::arcweft_data::Value::Number(::arcweft_data::Number::I(value)) => {
                        <#ty as ::core::convert::TryFrom<i128>>::try_from(*value).map_err(|_| {
                            ::arcweft_data::DataError::new(
                                ::arcweft_data::DataErrorKind::NumberOutOfRange,
                                format!("cannot fit {value} into {}", stringify!(#ty)),
                            )
                        })
                    }
                    ::arcweft_data::Value::Number(::arcweft_data::Number::U(value)) => {
                        <#ty as ::core::convert::TryFrom<u128>>::try_from(*value).map_err(|_| {
                            ::arcweft_data::DataError::new(
                                ::arcweft_data::DataErrorKind::NumberOutOfRange,
                                format!("cannot fit {value} into {}", stringify!(#ty)),
                            )
                        })
                    }
                    other => Err(::arcweft_data::DataError::invalid_type("numeric enum discriminant", other.type_name())),
                }
            }
        }
    }

    pub(crate) fn shape_option_tokens(&self) -> TokenStream {
        let shape = self.kind.shape_tokens();
        quote!(Some(#shape))
    }
}

#[derive(Clone, Copy)]
enum IntegerRepr {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

impl IntegerRepr {
    const ALL: [Self; 12] = [
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::I128,
        Self::Isize,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::U128,
        Self::Usize,
    ];

    fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|repr| repr.as_str() == value)
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::Isize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::Usize => "usize",
        }
    }

    const fn is_signed(self) -> bool {
        match self {
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::I128 | Self::Isize => true,
            Self::U8 | Self::U16 | Self::U32 | Self::U64 | Self::U128 | Self::Usize => false,
        }
    }

    fn ty_tokens(self) -> TokenStream {
        match self {
            Self::I8 => quote!(::core::primitive::i8),
            Self::I16 => quote!(::core::primitive::i16),
            Self::I32 => quote!(::core::primitive::i32),
            Self::I64 => quote!(::core::primitive::i64),
            Self::I128 => quote!(::core::primitive::i128),
            Self::Isize => quote!(::core::primitive::isize),
            Self::U8 => quote!(::core::primitive::u8),
            Self::U16 => quote!(::core::primitive::u16),
            Self::U32 => quote!(::core::primitive::u32),
            Self::U64 => quote!(::core::primitive::u64),
            Self::U128 => quote!(::core::primitive::u128),
            Self::Usize => quote!(::core::primitive::usize),
        }
    }

    fn shape_tokens(self) -> TokenStream {
        match self {
            Self::I8 => quote!(::arcweft_data::EnumRepr::I8),
            Self::I16 => quote!(::arcweft_data::EnumRepr::I16),
            Self::I32 => quote!(::arcweft_data::EnumRepr::I32),
            Self::I64 => quote!(::arcweft_data::EnumRepr::I64),
            Self::I128 => quote!(::arcweft_data::EnumRepr::I128),
            Self::Isize => quote!(::arcweft_data::EnumRepr::Isize),
            Self::U8 => quote!(::arcweft_data::EnumRepr::U8),
            Self::U16 => quote!(::arcweft_data::EnumRepr::U16),
            Self::U32 => quote!(::arcweft_data::EnumRepr::U32),
            Self::U64 => quote!(::arcweft_data::EnumRepr::U64),
            Self::U128 => quote!(::arcweft_data::EnumRepr::U128),
            Self::Usize => quote!(::arcweft_data::EnumRepr::Usize),
        }
    }
}

#[derive(Default)]
pub(crate) struct FieldAttrs {
    pub(crate) wire_name: String,
    pub(crate) default: bool,
    pub(crate) skip: bool,
    pub(crate) bytes_format: Option<TokenStream>,
}

impl FieldAttrs {
    pub(crate) fn from_attrs(
        attrs: &[Attribute],
        ident: &Ident,
        rename_all: RenameRuleAttr,
    ) -> Self {
        let mut out = Self {
            wire_name: rename_all.apply(&ident.to_string()),
            default: false,
            skip: false,
            bytes_format: None,
        };
        attrs
            .iter()
            .filter(|attr| attr.path().is_ident(AttrName::Arcweft.as_str()))
            .for_each(|attr| {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident(AttrName::Default.as_str()) {
                        out.default = true;
                        return Ok(());
                    }
                    if meta.path.is_ident(AttrName::Skip.as_str()) {
                        out.skip = true;
                        return Ok(());
                    }
                    if meta.path.is_ident(AttrName::Rename.as_str()) {
                        let value = meta.value()?.parse::<Lit>()?;
                        if let Lit::Str(value) = value {
                            out.wire_name = value.value();
                        }
                        return Ok(());
                    }
                    if meta.path.is_ident(AttrName::Bytes.as_str()) {
                        out.bytes_format = Some(if let Ok(value) = meta.value() {
                            if let Lit::Str(value) = value.parse::<Lit>()? {
                                BytesFormatAttr::parse_or_default(&value.value()).tokens()
                            } else {
                                BytesFormatAttr::Binary.tokens()
                            }
                        } else {
                            BytesFormatAttr::Binary.tokens()
                        });
                        return Ok(());
                    }
                    Ok(())
                });
            });
        out
    }
}

pub(crate) struct VariantAttrs {
    pub(crate) wire_name: String,
}

impl VariantAttrs {
    pub(crate) fn from_attrs(
        attrs: &[Attribute],
        ident: &Ident,
        rename_all: RenameRuleAttr,
    ) -> Self {
        let mut out = Self {
            wire_name: rename_all.apply(&ident.to_string()),
        };
        attrs
            .iter()
            .filter(|attr| attr.path().is_ident(AttrName::Arcweft.as_str()))
            .for_each(|attr| {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident(AttrName::Rename.as_str()) {
                        let value = meta.value()?.parse::<Lit>()?;
                        if let Lit::Str(value) = value {
                            out.wire_name = value.value();
                        }
                    }
                    Ok(())
                });
            });
        out
    }
}

#[derive(Clone, Copy)]
enum BytesFormatAttr {
    Binary,
    Base64,
    Hex,
    Array,
}

impl BytesFormatAttr {
    const ALL: [Self; 4] = [Self::Binary, Self::Base64, Self::Hex, Self::Array];

    fn parse_or_default(value: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|format| format.as_str() == value)
            .unwrap_or(Self::Base64)
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::Base64 => "base64",
            Self::Hex => "hex",
            Self::Array => "array",
        }
    }

    fn tokens(self) -> TokenStream {
        match self {
            Self::Binary => quote!(::arcweft_data::BytesFormat::Binary),
            Self::Base64 => quote!(::arcweft_data::BytesFormat::Base64),
            Self::Hex => quote!(::arcweft_data::BytesFormat::Hex),
            Self::Array => quote!(::arcweft_data::BytesFormat::Array),
        }
    }
}
