use arcweft_lang_sema::{
    checked_rich_text::{CheckedColor, CheckedContentEmission, LengthUnit},
    final_analysis::{
        CheckedCompileTimeScalar, CheckedCompileTimeScalarKind, CheckedTextProxyDefinition,
        FinalSemanticAnalysis,
    },
};
use arcweft_text_model::{
    Milli, RichTextColor, RichTextObjectProxy, RichTextObjectProxyDeclaration,
    RichTextTextProxyField, RichTextTextProxyFieldKind, RichTextTextProxyFieldSchema,
    RichTextTextProxyLength, RichTextTextProxyLengthUnit, RichTextTextProxyScalar,
    RichTextTextProxySchema,
};

use super::{ExprId, RuntimeSemanticProjectionError};

/// Lowers the closed `#object` emission carried by one checked ContentInsert.
/// The emission owns the application and its checked body; no synthetic HIR
/// object tag or source lookup is introduced at this boundary.
pub(super) fn lower_checked_object_emission(
    owner: ExprId,
    emission: &CheckedContentEmission,
    analysis: &FinalSemanticAnalysis,
) -> Result<RichTextObjectProxy, RuntimeSemanticProjectionError> {
    let CheckedContentEmission::ObjectSpan(application) = emission else {
        return Err(projection_error(
            owner,
            "non-object content emission reached object projection",
        ));
    };
    let view = analysis
        .checked_text_proxies()
        .application(application)
        .ok_or_else(|| {
            projection_error(
                owner,
                "checked text-proxy application does not join its final catalog",
            )
        })?;
    let definition = view.definition();
    let application = view.application();
    let fields = definition
        .fields()
        .iter()
        .zip(application.fields())
        .filter_map(|(definition, applied)| {
            applied.value().map(|value| RichTextTextProxyField {
                id: definition.declaration_ordinal(),
                name: definition.diagnostic_name().to_owned(),
                value: lower_scalar(value),
            })
        })
        .collect();
    let declaration = RichTextObjectProxyDeclaration {
        struct_name: definition.diagnostic_name().to_owned(),
        attribute: definition.attribute().source_name().to_owned(),
    };
    Ok(RichTextObjectProxy {
        id: application.id().value().as_str().to_owned(),
        schema: Some(lower_schema(definition)),
        declaration: Some(declaration),
        type_name: Some(definition.declaration().qualified_name()),
        role: application
            .metadata()
            .role()
            .map(|value| value.value().as_str().to_owned()),
        layer: application
            .metadata()
            .layer()
            .map(|value| value.value().as_str().to_owned()),
        depth: application
            .metadata()
            .depth()
            .map(|value| Milli(value.value().milli())),
        hit_test: *application.metadata().hit_test().value(),
        fields,
    })
}

fn lower_schema(definition: &CheckedTextProxyDefinition) -> RichTextTextProxySchema {
    RichTextTextProxySchema {
        id: definition.declaration().qualified_name(),
        declaration: RichTextObjectProxyDeclaration {
            struct_name: definition.diagnostic_name().to_owned(),
            attribute: definition.attribute().source_name().to_owned(),
        },
        fields: definition
            .fields()
            .iter()
            .map(|field| RichTextTextProxyFieldSchema {
                id: field.declaration_ordinal(),
                name: field.diagnostic_name().to_owned(),
                kind: lower_kind(field.kind()),
                optional: field.optional(),
                default: field.default().map(|value| lower_scalar(value.value())),
            })
            .collect(),
    }
}

fn lower_kind(kind: &CheckedCompileTimeScalarKind) -> RichTextTextProxyFieldKind {
    match kind {
        CheckedCompileTimeScalarKind::Bool => RichTextTextProxyFieldKind::Bool,
        CheckedCompileTimeScalarKind::Int => RichTextTextProxyFieldKind::Int,
        CheckedCompileTimeScalarKind::Milli => RichTextTextProxyFieldKind::Milli,
        CheckedCompileTimeScalarKind::Ratio => RichTextTextProxyFieldKind::Ratio,
        CheckedCompileTimeScalarKind::Length => RichTextTextProxyFieldKind::Length,
        CheckedCompileTimeScalarKind::Angle => RichTextTextProxyFieldKind::Angle,
        CheckedCompileTimeScalarKind::Duration => RichTextTextProxyFieldKind::Duration,
        CheckedCompileTimeScalarKind::ClosedEnum(schema) => {
            RichTextTextProxyFieldKind::ClosedEnum {
                enum_id: schema.declaration().qualified_name(),
                variants: schema
                    .cases()
                    .iter()
                    .map(|case| case.diagnostic_name().to_owned())
                    .collect(),
            }
        }
        CheckedCompileTimeScalarKind::PublicId => RichTextTextProxyFieldKind::PublicId,
        CheckedCompileTimeScalarKind::Text => RichTextTextProxyFieldKind::Text,
        CheckedCompileTimeScalarKind::Color => RichTextTextProxyFieldKind::Color,
    }
}

fn lower_scalar(value: &CheckedCompileTimeScalar) -> RichTextTextProxyScalar {
    match value {
        CheckedCompileTimeScalar::Bool(value) => RichTextTextProxyScalar::Bool { value: *value },
        CheckedCompileTimeScalar::Int(value) => RichTextTextProxyScalar::Int { value: *value },
        CheckedCompileTimeScalar::Milli(value) => RichTextTextProxyScalar::Milli {
            value: Milli(value.0),
        },
        CheckedCompileTimeScalar::Ratio(value) => RichTextTextProxyScalar::Ratio { milli: value.0 },
        CheckedCompileTimeScalar::Length(value) => RichTextTextProxyScalar::Length {
            value: RichTextTextProxyLength {
                milli: value.milli,
                unit: match value.unit {
                    LengthUnit::Px => RichTextTextProxyLengthUnit::Px,
                    LengthUnit::Pt => RichTextTextProxyLengthUnit::Pt,
                    LengthUnit::Ch => RichTextTextProxyLengthUnit::Ch,
                    LengthUnit::Em => RichTextTextProxyLengthUnit::Em,
                },
            },
        },
        CheckedCompileTimeScalar::Angle(value) => RichTextTextProxyScalar::Angle {
            milli_degrees: value.milli_degrees,
        },
        CheckedCompileTimeScalar::Duration(value) => RichTextTextProxyScalar::Duration {
            millis: value.millis,
        },
        CheckedCompileTimeScalar::Enum(value) => RichTextTextProxyScalar::ClosedEnum {
            enum_id: value.declaration().qualified_name(),
            variant: value.ordinal(),
        },
        CheckedCompileTimeScalar::PublicId(value) => RichTextTextProxyScalar::PublicId {
            value: value.as_str().to_owned(),
        },
        CheckedCompileTimeScalar::Text(value) => RichTextTextProxyScalar::Text {
            value: value.clone(),
        },
        CheckedCompileTimeScalar::Color(value) => RichTextTextProxyScalar::Color {
            value: match value {
                CheckedColor::Rgba8(value) => RichTextColor::Rgba8 { value: *value },
                CheckedColor::Resource(value) => RichTextColor::Resource {
                    id: value.as_str().to_owned(),
                },
            },
        },
    }
}

fn projection_error(owner: ExprId, reason: &'static str) -> RuntimeSemanticProjectionError {
    RuntimeSemanticProjectionError::Dialogue {
        owner: Some(owner),
        reason: reason.to_owned(),
    }
}
