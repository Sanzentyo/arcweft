//! The source and runtime projections of the same sparse policy constructors.

use super::{
    CharacterDialogueRichTextColor, CharacterDialogueRichTextProperty,
    CharacterDialogueRichTextPropertyValue as Value, CharacterDialogueRolePayloadCodec,
    CharacterDialogueRuntimeSchema, CharacterDialogueValueError,
};
use arcweft_core::{
    pure::RuntimeExternalCallContext,
    value::{
        RuntimeBorrowedValue, RuntimeExternalCallable, RuntimeExternalCallableExport,
        RuntimeExternalCallableProvider, RuntimeQuantity, RuntimeValue, StandardEnvironmentId,
    },
};
use arcweft_presentation::rich_text::{
    PresentationReusableStylePropertyId as Property, PresentationReusableStylePropertySpec,
};
use arcweft_rich_text_schema::RichTextUnit;

/// A named, optional constructor input. A property-less input composes a complete
/// policy; property inputs reuse the presentation-owned value grammar.
#[derive(Clone, Copy, Debug)]
pub struct CharacterDialogueStyleFactoryParameter {
    name: &'static str,
    property: Option<Property>,
}

impl CharacterDialogueStyleFactoryParameter {
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    #[must_use]
    pub fn property(self) -> Option<PresentationReusableStylePropertySpec> {
        let id = self.property?;
        Some(
            CharacterDialogueRolePayloadCodec::RichTextProperties
                .payload_schema()
                .expect("the accepted presentation property inventory is coherent")
                .property_spec(id)
                .expect("a constructor property belongs to the complete schema"),
        )
    }
}

/// One standard constructor, shared by semantic registration and execution.
#[derive(Clone, Copy, Debug)]
pub struct CharacterDialogueStyleFactory {
    name: &'static str,
    parameters: &'static [CharacterDialogueStyleFactoryParameter],
}

impl CharacterDialogueStyleFactory {
    pub const ALL: &'static [Self] = &[
        Self {
            name: "text_style",
            parameters: &[
                CharacterDialogueStyleFactoryParameter {
                    name: "font",
                    property: Some(Property::Font),
                },
                CharacterDialogueStyleFactoryParameter {
                    name: "color",
                    property: Some(Property::Color),
                },
                CharacterDialogueStyleFactoryParameter {
                    name: "size",
                    property: Some(Property::Size),
                },
            ],
        },
        Self {
            name: "ruby_style",
            parameters: &[
                CharacterDialogueStyleFactoryParameter {
                    name: "size",
                    property: Some(Property::RubySize),
                },
                CharacterDialogueStyleFactoryParameter {
                    name: "gap",
                    property: Some(Property::RubyGap),
                },
                CharacterDialogueStyleFactoryParameter {
                    name: "overhang",
                    property: Some(Property::RubyOverhang),
                },
                CharacterDialogueStyleFactoryParameter {
                    name: "collision_gap",
                    property: Some(Property::RubyCollisionGap),
                },
            ],
        },
        Self {
            name: "rich_text_style",
            parameters: &[
                CharacterDialogueStyleFactoryParameter {
                    name: "text",
                    property: None,
                },
                CharacterDialogueStyleFactoryParameter {
                    name: "ruby",
                    property: None,
                },
            ],
        },
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn parameters(self) -> &'static [CharacterDialogueStyleFactoryParameter] {
        self.parameters
    }

    /// Selects only an original Core standard declaration, never a source alias
    /// or a matching display label from another provider or overload.
    #[must_use]
    pub fn for_callable(callable: &RuntimeExternalCallable) -> Option<Self> {
        let declaration = callable.declaration();
        if declaration.provider()
            != &RuntimeExternalCallableProvider::Standard(StandardEnvironmentId::Core)
            || declaration.overload() != 0
        {
            return None;
        }
        let RuntimeExternalCallableExport::Function(path) = declaration.export() else {
            return None;
        };
        let [name] = path.as_ref() else {
            return None;
        };
        Self::ALL.iter().copied().find(|row| row.name == name)
    }

    pub(super) fn construct(
        self,
        schema: &CharacterDialogueRuntimeSchema,
        context: &RuntimeExternalCallContext,
        callable: &RuntimeExternalCallable,
        arguments: &[RuntimeValue],
    ) -> Result<RuntimeValue, Box<CharacterDialogueValueError>> {
        let invalid = || {
            Box::new(CharacterDialogueValueError::Field {
                field: "rich_text",
                reason: "constructor inputs differ from the registered signature".to_owned(),
            })
        };
        let execution = context
            .execution_context()
            .ok_or_else(|| Box::new(CharacterDialogueValueError::ExternalCallContextRequired))?;
        let types = context.argument_types().ok_or_else(invalid)?;
        if arguments.len() != types.len() || callable.operands().len() != arguments.len() {
            return Err(invalid());
        }
        let mut seen = std::collections::BTreeSet::new();
        for row in callable.operands() {
            let coordinate = row.parameter.ok_or_else(invalid)?;
            if coordinate.group != 0
                || row.expansion_width != 1
                || row.projection
                    != arcweft_core::value::RuntimeExternalCallOperandProjection::Value
                || coordinate.parameter as usize >= self.parameters.len()
                || !seen.insert(coordinate.parameter)
            {
                return Err(invalid());
            }
            let index = row.abi_position as usize;
            let value = arguments.get(index).ok_or_else(invalid)?;
            let ty = *types.get(index).ok_or_else(invalid)?;
            execution
                .types()
                .validate_live_value(ty, value, context.limits().ok_or_else(invalid)?)
                .map_err(|error| Box::new(CharacterDialogueValueError::from(error)))?;
        }
        let mut properties = Vec::new();
        for row in callable.operands() {
            let parameter = self.parameters
                [row.parameter.expect("admitted formal coordinate").parameter as usize];
            let value = &arguments[row.abi_position as usize];
            if let Some(property) = parameter.property {
                let value = match property {
                    Property::Font => {
                        use arcweft_presentation::rich_text::RichTextFontFamily;
                        let RuntimeValue::Variant(value) = value else {
                            return Err(invalid());
                        };
                        let family = if value.ordinal() == RichTextFontFamily::<&str>::NAMED_ORDINAL
                        {
                            let Some(RuntimeValue::Tuple(fields)) = value.payload() else {
                                return Err(invalid());
                            };
                            let [RuntimeValue::String(name)] = fields.as_slice() else {
                                return Err(invalid());
                            };
                            RichTextFontFamily::Named {
                                name: name.as_str(),
                            }
                        } else {
                            if value.payload().is_some() {
                                return Err(invalid());
                            }
                            RichTextFontFamily::from_generic_ordinal(value.ordinal())
                                .ok_or_else(invalid)?
                        };
                        Value::FontFamily(family)
                    }
                    Property::Color => {
                        let RuntimeValue::Color(value) = value else {
                            return Err(invalid());
                        };
                        Value::Color(CharacterDialogueRichTextColor::Rgba8(value.rgba8()))
                    }
                    _ => {
                        let Some(RuntimeQuantity::Length(value)) =
                            RuntimeQuantity::from_value(value)
                        else {
                            return Err(invalid());
                        };
                        let unit = RichTextUnit::try_from(value.unit).map_err(|_| invalid())?;
                        Value::Length {
                            milli: value.milli,
                            unit,
                        }
                    }
                };
                properties.push(CharacterDialogueRichTextProperty::try_new(property, value)?);
            } else {
                let opaque = RuntimeBorrowedValue::from(value)
                    .opaque()
                    .ok_or_else(invalid)?;
                if Some(opaque.semantic_identity()) != context.result_type() {
                    return Err(invalid());
                }
                let borrowed = CharacterDialogueRolePayloadCodec::RichTextProperties
                    .borrow_properties(opaque.payload())?;
                properties.extend(borrowed.iter().cloned());
            }
        }
        schema.construct_rich_text_properties_in_call(context, &properties)
    }
}
