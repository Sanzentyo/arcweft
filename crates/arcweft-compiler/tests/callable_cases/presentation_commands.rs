use super::presentation_factories::{program_from_source, run_selected};
use arcweft_core::value::RuntimeValue;

fn assert_true(value: &RuntimeValue) {
    assert_eq!(value, &RuntimeValue::Bool(true));
}

fn assert_unit(value: &RuntimeValue) {
    assert_eq!(value, &RuntimeValue::Unit);
}

fn empty_slot(character: bool, native: bool) {
    let call = if character {
        "hide(@character.alice)"
    } else {
        "clear.bg()"
    };
    let source = format!(
        r#"
pub character alice {{ display = "Alice" }}
pub fn root() -> bool {{
    match {call} {{ .None => true, .Some(_) => false }}
}}
flow main() -> i64 {{ return 0i64 }}
"#
    );
    run_selected(program_from_source(&source), native, assert_true);
}

fn occupied_slot(character: bool, native: bool) {
    let (first_factory, second_factory, remove, result) = if character {
        (
            "show(@character.alice)",
            "show(@character.alice, target = @target.scene)",
            "hide(@character.alice)",
            "CharacterSurface",
        )
    } else {
        (
            "bg(@asset.bg.first)",
            "bg(@asset.bg.second)",
            "clear.bg()",
            "BackgroundSurface",
        )
    };
    let source = format!(
        r#"
pub character alice {{ display = "Alice" }}
pub fn root() -> (Option<{result}>, Option<{result}>, bool) {{
    let original = {first_factory};
    let first = {remove};
    let replacement = {second_factory};
    original.release();
    let second = {remove};
    replacement.release();
    let empty = match {remove} {{ .None => true, .Some(_) => false }};
    (first, second, empty)
}}
flow main() -> i64 {{ return 0i64 }}
"#
    );
    run_selected(program_from_source(&source), native, |value| {
        use arcweft_core::pattern::RuntimeBuiltinVariantCaseIdentity;
        use arcweft_interaction_model::presentation::PresentationArgumentRole;
        let RuntimeValue::Tuple(fields) = value else {
            panic!("owned removal results")
        };
        assert_eq!(fields.len(), 3);
        for (ordinal, value) in fields[..2].iter().enumerate() {
            let Some((
                RuntimeBuiltinVariantCaseIdentity::OptionSome,
                Some(RuntimeValue::PresentationResource(resource)),
            )) = value.builtin_variant_case()
            else {
                panic!("occupied slot must return its actual resource")
            };
            let RuntimeValue::Tuple(captures) = resource.body() else {
                panic!("original capture tuple")
            };
            let role = match resource.resource() {
                arcweft_interaction_model::presentation::PresentationResourceKind::Background => {
                    PresentationArgumentRole::Asset
                }
                arcweft_interaction_model::presentation::PresentationResourceKind::Character => {
                    PresentationArgumentRole::Character
                }
                _ => panic!("selected slot resource family"),
            };
            let identity_index = resource
                .layout()
                .inputs()
                .iter()
                .position(|key| key.role() == role)
                .unwrap();
            let RuntimeValue::EntityRef(identity) = &captures[identity_index] else {
                panic!("original typed entity capture")
            };
            let identity = match identity {
                arcweft_core::value::RuntimeEntityReference::Project { public_id, .. } => {
                    public_id.as_str()
                }
                arcweft_core::value::RuntimeEntityReference::ImportedProject(reference) => {
                    reference.public_id().as_str()
                }
                _ => panic!("the actual source-issued entity reference"),
            };
            if role == PresentationArgumentRole::Asset {
                assert_eq!(identity, ["asset.bg.first", "asset.bg.second"][ordinal]);
            } else {
                assert_eq!(identity, "character.alice");
                assert_eq!(
                    resource
                        .layout()
                        .inputs()
                        .iter()
                        .any(|key| key.role() == PresentationArgumentRole::TargetEntity),
                    ordinal == 1
                );
            }
        }
        assert_eq!(fields[2], RuntimeValue::Bool(true));
    });
}

#[test]
fn source_presentation_clear_moves_the_occupied_background_in_native() {
    occupied_slot(false, true);
}

#[test]
fn source_presentation_clear_moves_the_occupied_background_in_decoded_awbc() {
    occupied_slot(false, false);
}

#[test]
fn source_presentation_hide_moves_the_occupied_character_in_native() {
    occupied_slot(true, true);
}

#[test]
fn source_presentation_hide_moves_the_occupied_character_in_decoded_awbc() {
    occupied_slot(true, false);
}

fn selected_slot(native: bool) {
    let selected = program_from_source(
        r#"
pub fn root() -> (bool, bool, bool, i32) {
    let mut order = 0i32;
    let handle = bg(@asset.bg.room, target = @target.menu, slot = @slot.background.main);
    let other = match clear.bg() { .None => true, .Some(_) => false };
    let removed = match clear.bg(
        slot = scope { order = order * 10i32 + 1i32; @slot.background.main },
        target = scope { order = order * 10i32 + 2i32; @target.menu }
    ) { .None => false, .Some(_) => true };
    let empty = match clear.bg(target = @target.menu, slot = @slot.background.main) { .None => true, .Some(_) => false };
    handle.release();
    (other, removed, empty, order)
}
flow main() -> i64 { return 0i64 }
"#,
    );
    run_selected(selected, native, |value| {
        assert_eq!(
            value,
            &RuntimeValue::Tuple(vec![
                RuntimeValue::Bool(true),
                RuntimeValue::Bool(true),
                RuntimeValue::Bool(true),
                RuntimeValue::i32(12),
            ])
        )
    });
}

#[test]
fn source_presentation_removal_selects_target_slot_in_authored_order_in_native() {
    selected_slot(true);
}

#[test]
fn source_presentation_removal_selects_target_slot_in_authored_order_in_decoded_awbc() {
    selected_slot(false);
}

fn handle_method(method: &str, native: bool) {
    let factory = if method == "pop" {
        "overlay(@view.Panel)"
    } else {
        "image(param.channel = 7i32)"
    };
    let source = format!(
        r#"
view Panel() {{ Text("ready") }}
pub fn root() {{
    let handle = {factory};
    handle.{method}()
}}
flow main() -> i64 {{ return 0i64 }}
"#
    );
    run_selected(program_from_source(&source), native, assert_unit);
}

#[test]
fn source_presentation_clear_empty_background_in_native() {
    empty_slot(false, true);
}

#[test]
fn source_presentation_clear_empty_background_in_decoded_awbc() {
    empty_slot(false, false);
}

#[test]
fn source_presentation_hide_empty_character_in_native() {
    empty_slot(true, true);
}

#[test]
fn source_presentation_hide_empty_character_in_decoded_awbc() {
    empty_slot(true, false);
}

#[test]
fn source_presentation_handle_show_in_native() {
    handle_method("show", true);
}

#[test]
fn source_presentation_handle_show_in_decoded_awbc() {
    handle_method("show", false);
}

#[test]
fn source_presentation_handle_hide_in_native() {
    handle_method("hide", true);
}

#[test]
fn source_presentation_handle_hide_in_decoded_awbc() {
    handle_method("hide", false);
}

#[test]
fn source_presentation_handle_unmount_in_native() {
    handle_method("unmount", true);
}

#[test]
fn source_presentation_handle_unmount_in_decoded_awbc() {
    handle_method("unmount", false);
}

#[test]
fn source_presentation_handle_release_in_native() {
    handle_method("release", true);
}

#[test]
fn source_presentation_handle_release_in_decoded_awbc() {
    handle_method("release", false);
}

#[test]
fn source_presentation_handle_destroy_in_native() {
    handle_method("destroy", true);
}

#[test]
fn source_presentation_handle_destroy_in_decoded_awbc() {
    handle_method("destroy", false);
}

#[test]
fn source_presentation_overlay_pop_in_native() {
    handle_method("pop", true);
}

#[test]
fn source_presentation_overlay_pop_in_decoded_awbc() {
    handle_method("pop", false);
}

fn visibility_then_retire(native: bool) {
    run_selected(
        program_from_source(
            r#"
pub fn root() {
    let handle = image(param.channel = 7i32);
    handle.hide();
    handle.show();
    handle.hide();
    handle.release()
}
flow main() -> i64 { return 0i64 }
"#,
        ),
        native,
        assert_unit,
    );
}

#[test]
fn source_presentation_visibility_keeps_handle_for_retirement_in_native() {
    visibility_then_retire(true);
}

#[test]
fn source_presentation_visibility_keeps_handle_for_retirement_in_decoded_awbc() {
    visibility_then_retire(false);
}

fn stored_fields(native: bool) {
    run_selected(
        program_from_source(
            r#"
struct Pair { first: Handle<ImageSurface>, second: Handle<ImageSurface>, marker: i32 }
pub fn root() -> i32 {
    let pair = Pair { first: image(param.channel = 1i32), second: image(param.channel = 2i32), marker: 42i32 };
    pair.first.hide();
    pair.first.show();
    pair.first.release();
    pair.second.hide();
    pair.second.destroy();
    pair.marker
}
flow main() -> i64 { return 0i64 }
"#,
        ),
        native,
        |value| assert_eq!(value, &RuntimeValue::i32(42)),
    );
}

#[test]
fn source_presentation_handle_fields_keep_siblings_in_native() {
    stored_fields(true);
}

#[test]
fn source_presentation_handle_fields_keep_siblings_in_decoded_awbc() {
    stored_fields(false);
}

fn computed_receiver(native: bool) {
    run_selected(
        program_from_source(
            r#"
pub fn root() -> i32 {
    let mut count = 0i32;
    image(param.channel = scope { count = count + 1i32; 7i32 }).release();
    count
}
flow main() -> i64 { return 0i64 }
"#,
        ),
        native,
        |value| assert_eq!(value, &RuntimeValue::i32(1)),
    );
}

#[test]
fn source_presentation_computed_handle_evaluates_once_in_native() {
    computed_receiver(true);
}

#[test]
fn source_presentation_computed_handle_evaluates_once_in_decoded_awbc() {
    computed_receiver(false);
}

#[test]
fn source_presentation_retired_handle_cannot_be_reused() {
    let error = super::compile_source(
        r#"
pub fn root() { let handle = image(); handle.release(); handle.show() }
flow main() -> i64 { return 0i64 }
"#,
    )
    .expect_err("retirement transfers the affine handle");
    assert!(
        error.project().diagnostics().iter().any(|diagnostic| {
            diagnostic
                .diagnostic()
                .code()
                .is_some_and(|code| code.as_str() == "sema.final_analysis")
                && diagnostic.diagnostic().message().contains("unavailable")
        }),
        "{error:?}"
    );
}
