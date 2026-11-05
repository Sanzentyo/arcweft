//! Borrowed property admission and construction under the exact executable.

use super::{
    CharacterDialogueRichTextColor, CharacterDialogueRichTextProperty,
    CharacterDialogueRichTextPropertyValue, CharacterDialogueRolePayloadCodec,
    CharacterDialogueRolePayloadSchema, CharacterDialogueValueError,
    PRODUCTION_CHARACTER_DIALOGUE_LIMITS, PresentationReusableStylePropertyId, RuntimeScalarView,
    RuntimeUInt, RuntimeValue, decode_property_value, schema_error,
};
use crate::character_dialogue::schema::CharacterDialogueRuntimeRoleBody;
use crate::character_dialogue::{
    CharacterDialogueRuntimeRole as Role, CharacterDialogueRuntimeSchema,
};
use arcweft_core::{
    entry::RuntimeSchemaLimits,
    pure::RuntimeExternalCallContext,
    task::RuntimeProgramOwner,
    value::{RuntimeInt, RuntimePlainValueRecipe as Recipe, RuntimeRetainedValueAdmission},
};
use std::collections::BTreeMap;

struct PropertyRecipe<'a> {
    atoms: Vec<Recipe<'a>>,
    tuple: bool,
}

impl<'a> PropertyRecipe<'a> {
    fn new(value: &CharacterDialogueRichTextPropertyValue<&'a str, &'a str>) -> Self {
        use CharacterDialogueRichTextColor as Color;
        use CharacterDialogueRichTextPropertyValue as Value;
        let (atoms, tuple) = match value {
            Value::Bool(value) => (
                vec![
                    Recipe::try_from(RuntimeScalarView::Bool(*value))
                        .expect("a boolean scalar has no fallible numeric constraint"),
                ],
                false,
            ),
            Value::Int(value) => (vec![Recipe::from(RuntimeInt::I64(*value))], false),
            Value::Milli(value) | Value::Angle(value) => {
                (vec![Recipe::from(RuntimeInt::I32(*value))], false)
            }
            Value::Ratio(value) | Value::ClosedEnum { variant: value, .. } => {
                (vec![Recipe::from(RuntimeUInt::U16(*value))], false)
            }
            Value::Length { milli, unit } => (
                vec![
                    Recipe::from(RuntimeInt::I32(*milli)),
                    Recipe::from(RuntimeUInt::U8(unit.semantic_tag())),
                ],
                true,
            ),
            Value::Duration(value) => (vec![Recipe::from(RuntimeUInt::U64(*value))], false),
            Value::PublicId(value) | Value::Text(value) | Value::Color(Color::Resource(value)) => {
                (vec![Recipe::from(*value)], false)
            }
            Value::Color(Color::Rgba8(values)) => (
                values
                    .iter()
                    .map(|value| Recipe::from(RuntimeUInt::U8(*value)))
                    .collect(),
                true,
            ),
            Value::Vec2 { x_milli, y_milli } => (
                vec![
                    Recipe::from(RuntimeInt::I32(*x_milli)),
                    Recipe::from(RuntimeInt::I32(*y_milli)),
                ],
                true,
            ),
            Value::Seed32(value) => (vec![Recipe::from(RuntimeUInt::U32(*value))], false),
        };
        Self { atoms, tuple }
    }

    fn root(&self) -> Recipe<'_> {
        if self.tuple {
            Recipe::tuple(&self.atoms)
        } else {
            self.atoms[0]
        }
    }
}

impl<'a> CharacterDialogueRichTextProperty<&'a str, &'a str> {
    /// Admits a borrowed semantic setting through the same codec as live and
    /// inert stored payloads, including the closed enum's exact owning domain.
    pub fn try_new(
        id: PresentationReusableStylePropertyId,
        value: CharacterDialogueRichTextPropertyValue<&'a str, &'a str>,
    ) -> Result<Self, Box<CharacterDialogueValueError>> {
        let schema = CharacterDialogueRolePayloadCodec::RichTextProperties.payload_schema()?;
        let field = schema
            .fields
            .iter()
            .find(|field| field.spec.id == id)
            .ok_or_else(|| schema_error("unknown reusable property"))?;
        let storage = PropertyRecipe::new(&value);
        if decode_property_value(field.spec, storage.root().borrowed())? != value {
            return Err(Box::new(schema_error(
                "property value differs from its owning domain",
            )));
        }
        Ok(Self { id, value })
    }
}

impl CharacterDialogueRolePayloadSchema {
    fn with_properties_recipe<'a, T>(
        &self,
        properties: &[CharacterDialogueRichTextProperty<&'a str, &'a str>],
        construct: impl FnOnce(&Recipe<'_>) -> Result<T, Box<CharacterDialogueValueError>>,
    ) -> Result<T, Box<CharacterDialogueValueError>> {
        let maximum = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_structured_leaves);
        if properties.len() > maximum {
            return Err(Box::new(CharacterDialogueValueError::Limit {
                limit: "rich_text_property_inputs",
                maximum,
            }));
        }
        let overrides = properties
            .iter()
            .map(|property| (property.id, property))
            .collect::<BTreeMap<_, _>>();
        let storage = self
            .fields
            .iter()
            .map(|field| {
                overrides
                    .get(&field.spec.id)
                    .map(|property| PropertyRecipe::new(property.value()))
            })
            .collect::<Vec<_>>();
        let values = storage
            .iter()
            .map(|storage| storage.as_ref().map(PropertyRecipe::root))
            .collect::<Vec<_>>();
        let slots = values
            .iter()
            .map(|value| value.as_ref().map_or_else(Recipe::none, Recipe::some))
            .collect::<Vec<_>>();
        let root = Recipe::tuple(&slots);
        self.visit_properties(root.borrowed(), |_, _| Ok(()))?;
        construct(&root)
    }
}

impl CharacterDialogueRuntimeSchema {
    /// Constructs a plain `RichText` role for this exact executable generation.
    /// Source-ordered duplicate settings retain the last value; no scalar text
    /// is cloned until the complete opaque root and its source types pass.
    pub fn construct_rich_text_properties<'a>(
        &self,
        owner: &RuntimeProgramOwner,
        properties: &[CharacterDialogueRichTextProperty<&'a str, &'a str>],
    ) -> Result<RuntimeValue, Box<CharacterDialogueValueError>> {
        self.construct_properties(owner, Self::limits(), properties)
    }

    /// Constructs the selected call's exact result using its original program
    /// and resource allowance. An unbound call cannot substitute defaults.
    pub fn construct_rich_text_properties_in_call<'a>(
        &self,
        context: &RuntimeExternalCallContext,
        properties: &[CharacterDialogueRichTextProperty<&'a str, &'a str>],
    ) -> Result<RuntimeValue, Box<CharacterDialogueValueError>> {
        let (Some(owner), Some(expected), Some(limits)) = (
            context.program_owner(),
            context.result_type(),
            context.limits(),
        ) else {
            return Err(Box::new(
                CharacterDialogueValueError::ExternalCallContextRequired,
            ));
        };
        self.ensure_program_owner(&owner)?;
        let actual = self.policies.rich_text_owner().semantic_identity();
        if expected != actual {
            return Err(Box::new(
                CharacterDialogueValueError::ExternalCallResultType { expected, actual },
            ));
        }
        self.construct_properties(&owner, limits, properties)
    }

    fn construct_properties<'a>(
        &self,
        owner: &RuntimeProgramOwner,
        mut limits: RuntimeSchemaLimits,
        properties: &[CharacterDialogueRichTextProperty<&'a str, &'a str>],
    ) -> Result<RuntimeValue, Box<CharacterDialogueValueError>> {
        self.ensure_program_owner(owner)?;
        let binding = self.roles.authored(Role::RichText).ok_or_else(|| {
            CharacterDialogueValueError::RoleType {
                role: Role::RichText,
                reason: "executable has no authored rich-text role",
            }
        })?;
        let CharacterDialogueRuntimeRoleBody::Bound { payload, codec } = binding.body() else {
            return Err(Box::new(CharacterDialogueValueError::RoleType {
                role: Role::RichText,
                reason: "role has no accepted payload codec",
            }));
        };
        limits.max_encoded_bytes = limits.max_encoded_bytes.min(u64::from(
            PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes,
        ));
        let mut admission = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        codec
            .payload_schema()?
            .with_properties_recipe(properties, |recipe| {
                admission
                    .construct_plain_opaque(self.policies.rich_text_owner(), payload, recipe)
                    .map_err(|error| Box::new(CharacterDialogueValueError::from(*error)))
            })
    }
}
