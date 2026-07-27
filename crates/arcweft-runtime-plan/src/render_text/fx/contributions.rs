//! Cascade provenance emitted for typed `RichText` Fx applications.

use arcweft_lang_hir::model::HirDialogue;
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_presentation::fx::{
    FxApplication, FxCapabilitySet, FxDefinition, FxEvaluationBinding, FxEvaluationBudget,
    FxGraphChildPath, FxGraphEvaluator, FxInstanceSnapshot, FxLogicalTime, FxRendererInterface,
    FxResolvedValue, FxRuntimeValue, ResolvedFxOperation, derive_deterministic_seed,
};
use arcweft_render_text::{
    RichTextAssignOp, RichTextCascadeLayer, RichTextSettingSource, RichTextSourceRange,
    RichTextStyleContribution, TextColor,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FxInlineAssignment {
    definition: String,
    source_range: TextRange,
    values: Vec<FxInlineValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FxInlineValue {
    path: String,
    value: String,
}

impl FxInlineAssignment {
    pub(super) fn new(
        definition: &FxDefinition,
        application: &FxApplication,
        source_range: TextRange,
    ) -> Self {
        Self {
            definition: application.definition().to_string(),
            source_range,
            values: resolved_inline_values(definition, application),
        }
    }
}

pub(crate) fn append_fx_inline_contributions(
    target: &mut Vec<RichTextStyleContribution>,
    dialogue: &HirDialogue,
    assignments: &[FxInlineAssignment],
) {
    let content = dialogue.content();
    for assignment in assignments {
        let Some(source_range) = content.source_range(assignment.source_range) else {
            continue;
        };
        let source = RichTextSettingSource::SourceFile {
            item_id: dialogue.id().map(|id| id.body().to_owned()),
            public_id: dialogue.text_key().map(|id| id.body().to_owned()),
            range: Some(RichTextSourceRange {
                start: source_range.start(),
                end: source_range.end(),
            }),
        };
        target.push(RichTextStyleContribution {
            path: format!("rich_text.fx.{}", assignment.definition),
            layer: RichTextCascadeLayer::InlineSpan,
            source: source.clone(),
            op: RichTextAssignOp::Replace,
            value: assignment.definition.clone(),
            style_index: None,
            active: true,
            shadowed_by: None,
        });
        target.extend(
            assignment
                .values
                .iter()
                .map(|value| RichTextStyleContribution {
                    path: value.path.clone(),
                    layer: RichTextCascadeLayer::InlineSpan,
                    source: source.clone(),
                    op: RichTextAssignOp::Replace,
                    value: value.value.clone(),
                    style_index: None,
                    active: true,
                    shadowed_by: None,
                }),
        );
    }
}

fn resolved_inline_values(
    definition: &FxDefinition,
    application: &FxApplication,
) -> Vec<FxInlineValue> {
    let instance = application.derive_instance_id(["rich_text.provenance"]);
    let child_path = FxGraphChildPath::default();
    let snapshot = FxInstanceSnapshot {
        instance,
        definition: definition.id().clone(),
        abi_hash: definition.abi_hash(),
        activation_logical_time: FxLogicalTime::zero(),
        deterministic_seed: derive_deterministic_seed(
            instance,
            definition.semantic_hash(),
            None,
            &child_path,
        ),
        parameters: application.parameters().to_vec(),
        child_path,
        provider_state: Vec::new(),
    };
    let mut budget = FxEvaluationBudget::default();
    let plan = FxGraphEvaluator::evaluate(
        application,
        FxEvaluationBinding {
            definition,
            instance: &snapshot,
            runtime_time: FxLogicalTime::zero(),
        },
        application.authored_ordinal(),
        true,
        false,
        &FxCapabilitySet::canonical(),
        &mut budget,
    );
    if !plan.is_conformant() {
        return Vec::new();
    }
    plan.layout()
        .iter()
        .chain(plan.glyph())
        .filter_map(|operation| match operation {
            ResolvedFxOperation::Values(operation) => Some(operation),
            ResolvedFxOperation::Transform(_) => None,
        })
        .flat_map(|operation| {
            operation.values.iter().filter_map(move |value| {
                let path = inline_value_path(operation.interface, &value.name)?;
                inline_value_label(&value.value).map(|value| FxInlineValue {
                    path: path.to_owned(),
                    value,
                })
            })
        })
        .collect()
}

fn inline_value_path(interface: FxRendererInterface, name: &str) -> Option<&'static str> {
    match (interface, name) {
        (FxRendererInterface::TextStyle, "color") => Some("rich_text.text.color"),
        (FxRendererInterface::TextStyle, "font_family") => Some("rich_text.text.font"),
        (FxRendererInterface::TextStyle, "size") => Some("rich_text.text.size"),
        (FxRendererInterface::TextStyle, "weight") => Some("rich_text.text.weight"),
        (FxRendererInterface::TextStyle, "slant") => Some("rich_text.text.slant"),
        (FxRendererInterface::TextStyle, "spacing") => Some("rich_text.text.spacing"),
        (FxRendererInterface::TextStyle, "opacity") => Some("rich_text.presentation.opacity"),
        (FxRendererInterface::Color, "tint") => Some("rich_text.color.tint"),
        (FxRendererInterface::Color, "multiply") => Some("rich_text.color.multiply"),
        (FxRendererInterface::Color, "opacity") => Some("rich_text.color.opacity"),
        _ => None,
    }
}

fn inline_value_label(value: &FxResolvedValue) -> Option<String> {
    match value {
        FxResolvedValue::Runtime(FxRuntimeValue::Bool(value)) => Some(value.to_string()),
        FxResolvedValue::Runtime(FxRuntimeValue::I32(value)) => Some(value.to_string()),
        FxResolvedValue::Runtime(FxRuntimeValue::F32(value)) => Some(value.to_string()),
        FxResolvedValue::Runtime(FxRuntimeValue::Length(value)) => {
            Some(format!("{}px", value.value()))
        }
        FxResolvedValue::Runtime(FxRuntimeValue::Angle(value)) => {
            Some(format!("{}rad", value.value()))
        }
        FxResolvedValue::Runtime(FxRuntimeValue::Seconds(value)) => {
            Some(format!("{}s", value.value()))
        }
        FxResolvedValue::Runtime(FxRuntimeValue::Color(value)) => {
            let [red, green, blue, alpha] = TextColor::from(*value).channels();
            Some(if alpha == u8::MAX {
                format!("#{red:02x}{green:02x}{blue:02x}")
            } else {
                format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
            })
        }
        FxResolvedValue::String(value) | FxResolvedValue::Selector(value) => Some(value.clone()),
        FxResolvedValue::Runtime(FxRuntimeValue::Vec2(_) | FxRuntimeValue::Transform2D(_))
        | FxResolvedValue::Resource(_)
        | FxResolvedValue::List(_)
        | FxResolvedValue::Record(_) => None,
    }
}
