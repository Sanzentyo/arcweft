use crate::lower_context::LowerContext;
use crate::lower_ids::{
    normalize_choice_action, normalize_choice_id, normalize_option_id, normalize_text_key_id,
};
use crate::model::{HirChoice, HirChoiceOption, HirLowerError};
use arcweft_lang_syntax::ChoiceBlock;

pub(crate) fn lower_choice(
    choice: &ChoiceBlock,
    context: &mut LowerContext,
) -> Result<HirChoice, HirLowerError> {
    let id = choice
        .id()
        .map(|id| normalize_choice_id(id, context))
        .transpose()?;
    if let Some(id) = &id {
        context.choice_stack.push(id.body().to_owned());
    }
    let options = choice
        .options()
        .iter()
        .map(|option| {
            Ok(HirChoiceOption {
                id: option
                    .id()
                    .map(|id| normalize_option_id(id, context))
                    .transpose()?,
                label: option.label().to_owned(),
                condition: option.condition().cloned(),
                action: normalize_choice_action(option.action(), context)?,
                value: option.value().cloned(),
                label_text_key: option
                    .label_text_key()
                    .map(|id| normalize_text_key_id(id, context))
                    .transpose()?,
            })
        })
        .collect::<Result<Vec<_>, HirLowerError>>()?;
    if choice.id().is_some() {
        context.choice_stack.pop();
    }
    Ok(HirChoice {
        id,
        items: choice.items().to_vec(),
        plan: choice.plan().cloned(),
        options,
    })
}
