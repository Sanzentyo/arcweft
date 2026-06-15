use crate::lower_context::LowerContext;
use crate::lower_ids::{
    content_callee_slug, normalize_entity_ref_syntax, normalize_line_id, normalize_line_text_key,
    speaker_slug,
};
use crate::model::{HirDialogue, HirLowerError};
use arcweft_lang_syntax::ast::dialogue::{ContentCall, SpeakerLine};

pub(crate) fn lower_speaker_line(
    line: &SpeakerLine,
    context: &mut LowerContext,
) -> Result<HirDialogue, HirLowerError> {
    let speaker = speaker_slug(line.speaker());
    let id = normalize_line_id(line.options().id(), speaker.clone(), context, *line.range())?;
    let text_key =
        normalize_line_text_key(line.options().text_key(), id.as_ref(), speaker, context)?;
    let window = line
        .options()
        .window()
        .map(|window| normalize_entity_ref_syntax(window, context))
        .transpose()?;
    Ok(HirDialogue {
        callee: line.speaker().to_owned(),
        text_key,
        id,
        voice: line.options().voice().cloned(),
        look: line.options().look().cloned(),
        stage: line.options().stage().cloned(),
        portrait: line.options().portrait().cloned(),
        focus: line.options().focus().cloned(),
        cleanup: line.options().cleanup().cloned(),
        window,
        source_locale: line.options().source_locale().map(str::to_owned),
        hooks: line.options().hooks().to_vec(),
        style: line.options().style().cloned(),
        style_raw: line.options().style_raw().map(str::to_owned),
        style_range: line.options().style_range(),
        rich_text: line.options().rich_text().cloned(),
        rich_text_raw: line.options().rich_text_raw().map(str::to_owned),
        rich_text_range: line.options().rich_text_range(),
        args: line.options().args().to_vec(),
        content: line.content().clone(),
        plan: line.plan().cloned(),
    })
}

pub(crate) fn lower_content_call(
    call: &ContentCall,
    context: &mut LowerContext,
) -> Result<HirDialogue, HirLowerError> {
    let speaker = content_callee_slug(call.callee());
    let id = normalize_line_id(call.options().id(), speaker.clone(), context, *call.range())?;
    let text_key =
        normalize_line_text_key(call.options().text_key(), id.as_ref(), speaker, context)?;
    let window = call
        .options()
        .window()
        .map(|window| normalize_entity_ref_syntax(window, context))
        .transpose()?;
    Ok(HirDialogue {
        callee: call.callee().to_owned(),
        text_key,
        id,
        voice: call.options().voice().cloned(),
        look: call.options().look().cloned(),
        stage: call.options().stage().cloned(),
        portrait: call.options().portrait().cloned(),
        focus: call.options().focus().cloned(),
        cleanup: call.options().cleanup().cloned(),
        window,
        source_locale: call.options().source_locale().map(str::to_owned),
        hooks: call.options().hooks().to_vec(),
        style: call.options().style().cloned(),
        style_raw: call.options().style_raw().map(str::to_owned),
        style_range: call.options().style_range(),
        rich_text: call.options().rich_text().cloned(),
        rich_text_raw: call.options().rich_text_raw().map(str::to_owned),
        rich_text_range: call.options().rich_text_range(),
        args: call.options().args().to_vec(),
        content: call.content().clone(),
        plan: call.plan().cloned(),
    })
}
