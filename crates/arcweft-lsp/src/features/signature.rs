//! Deterministic LSP projection of native semantic signature-query results.

use arcweft_lang_sema::{
    callable::{
        CallableDocumentation, CallableParameterCoordinate, SemanticSignature,
        SemanticSignatureHelp,
    },
    signature::SignatureQueryOutcome,
};
use lsp_types::{
    Documentation, MarkupContent, MarkupKind, ParameterInformation, ParameterLabel, SignatureHelp,
    SignatureInformation,
};
use thiserror::Error;

/// A typed semantic result could not be represented by the LSP wire model.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum SignatureProjectionError {
    #[error("signature label UTF-16 offsets exceed the LSP u32 range")]
    LabelOffsetOverflow,
    #[error("active signature index exceeds the LSP u32 range")]
    ActiveSignatureOverflow,
    #[error("active parameter index exceeds the LSP u32 range")]
    ActiveParameterOverflow,
    #[error("active parameter is absent from the selected semantic signature")]
    ActiveParameterMissing,
}

/// Converts one native semantic outcome without name lookup or source fallback.
pub(crate) fn signature_help(
    outcome: &SignatureQueryOutcome,
) -> Result<Option<SignatureHelp>, SignatureProjectionError> {
    match outcome {
        SignatureQueryOutcome::Help(help) => project_help(help).map(Some),
        SignatureQueryOutcome::NotApplicable(_) => Ok(None),
    }
}

fn project_help(help: &SemanticSignatureHelp) -> Result<SignatureHelp, SignatureProjectionError> {
    let active_signature = help.active_signature().get();
    let active_signature_lsp = u32::try_from(active_signature)
        .map_err(|_| SignatureProjectionError::ActiveSignatureOverflow)?;
    let mut active_parameter = None;
    let signatures = help
        .signatures()
        .iter()
        .enumerate()
        .map(|(index, signature)| {
            let active = (active_signature == index)
                .then_some(help.active_parameter())
                .flatten();
            let projected = project_signature(signature, active)?;
            if active_signature == index {
                active_parameter = help.active_parameter().and(projected.active_parameter);
            }
            Ok(projected.information)
        })
        .collect::<Result<Vec<_>, SignatureProjectionError>>()?;

    Ok(SignatureHelp {
        signatures,
        active_signature: Some(active_signature_lsp),
        active_parameter,
    })
}

struct ProjectedSignature {
    information: SignatureInformation,
    active_parameter: Option<u32>,
}

fn project_signature(
    signature: &SemanticSignature,
    active: Option<CallableParameterCoordinate>,
) -> Result<ProjectedSignature, SignatureProjectionError> {
    let mut label = SignatureLabelBuilder::new(signature.authored_callee())?;
    let mut parameters = Vec::new();
    let mut active_parameter = None;

    for group in signature.groups() {
        label.push("(")?;
        for (index, parameter) in group.parameters().iter().enumerate() {
            if index != 0 {
                label.push(", ")?;
            }
            let flat_index = u32::try_from(parameters.len())
                .map_err(|_| SignatureProjectionError::ActiveParameterOverflow)?;
            let range = label.push_parameter(parameter.label())?;
            if active == Some(parameter.coordinate()) {
                active_parameter = Some(flat_index);
            }
            parameters.push(ParameterInformation {
                label: ParameterLabel::LabelOffsets(range),
                documentation: text_documentation(parameter.documentation()),
            });
        }
        label.push(")")?;
    }
    label.push(" -> ")?;
    label.push(signature.result().source_label().as_str())?;

    if active.is_some() && active_parameter.is_none() {
        return Err(SignatureProjectionError::ActiveParameterMissing);
    }

    Ok(ProjectedSignature {
        information: SignatureInformation {
            label: label.finish(),
            documentation: callable_documentation(signature.documentation()),
            parameters: Some(parameters),
            active_parameter,
        },
        active_parameter,
    })
}

struct SignatureLabelBuilder {
    text: String,
    utf16_units: u32,
}

impl SignatureLabelBuilder {
    fn new(authored_callee: &str) -> Result<Self, SignatureProjectionError> {
        let mut builder = Self {
            text: String::new(),
            utf16_units: 0,
        };
        builder.push(authored_callee)?;
        Ok(builder)
    }

    fn push(&mut self, text: &str) -> Result<(), SignatureProjectionError> {
        let added = text.encode_utf16().try_fold(0u32, |units, _| {
            units
                .checked_add(1)
                .ok_or(SignatureProjectionError::LabelOffsetOverflow)
        })?;
        self.utf16_units = self
            .utf16_units
            .checked_add(added)
            .ok_or(SignatureProjectionError::LabelOffsetOverflow)?;
        self.text.push_str(text);
        Ok(())
    }

    fn push_parameter(&mut self, parameter: &str) -> Result<[u32; 2], SignatureProjectionError> {
        let start = self.utf16_units;
        self.push(parameter)?;
        Ok([start, self.utf16_units])
    }

    fn finish(self) -> String {
        self.text
    }
}

fn callable_documentation(documentation: &CallableDocumentation) -> Option<Documentation> {
    let value = documentation
        .summary()
        .into_iter()
        .chain(documentation.details())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    (!value.is_empty()).then(|| markdown_documentation(value))
}

fn text_documentation(value: Option<&str>) -> Option<Documentation> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| markdown_documentation(value.to_owned()))
}

fn markdown_documentation(value: String) -> Documentation {
    Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arcweft_lang_sema::{
        callable::{
            BuiltinCallableId, CallPoison, CallableCandidateId, CallableDocumentation,
            CallableGroupIndex, CallableGroupKind, CallableName, CallableParameterCoordinate,
            CallableParameterIndex, CallableParameterPassing, CallableParameterPresence,
            CallableParameterType, LanguageCallableFamily, PRODUCTION_CALLABLE_LIMITS,
            SemanticParameter, SemanticParameterGroup, SemanticSignature, SignatureOrigin,
        },
        effect_row::EffectRow,
        types::TypeKind,
    };

    use super::*;

    #[test]
    fn typed_non_ascii_fields_produce_utf16_parameter_offsets() {
        let group = CallableGroupIndex::try_from_usize(0).expect("group");
        let parameter_index = CallableParameterIndex::try_from_usize(0).expect("parameter");
        let coordinate = CallableParameterCoordinate::new(group, parameter_index);
        let parameter = SemanticParameter::try_new(
            coordinate,
            "値: String",
            Some(CallableName::try_new("値").expect("name")),
            CallableParameterType::Exact(TypeKind::String),
            CallableParameterPassing::PositionalOrNamed,
            CallableParameterPresence::Required,
            None,
            None,
        )
        .expect("semantic parameter");
        let group = SemanticParameterGroup::try_new(
            group,
            CallableGroupKind::Initial,
            vec![parameter],
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("semantic group");
        let signature = SemanticSignature::try_new(
            CallableCandidateId::Builtin(BuiltinCallableId::Panic),
            Vec::new(),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Builtin,
            },
            Arc::from("計算"),
            Arc::from("canonical.calculate"),
            vec![group],
            TypeKind::String,
            EffectRow::default(),
            CallableDocumentation::missing(),
            None,
            CallableGroupIndex::try_from_usize(0).expect("group"),
            CallPoison::Clean,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("semantic signature");

        let projected = project_signature(&signature, Some(coordinate)).expect("LSP projection");

        assert_eq!(projected.information.label, "計算(値: String) -> String");
        assert_eq!(projected.active_parameter, Some(0));
        assert_eq!(
            projected.information.parameters,
            Some(vec![ParameterInformation {
                label: ParameterLabel::LabelOffsets([3, 12]),
                documentation: None,
            }])
        );
    }
}
