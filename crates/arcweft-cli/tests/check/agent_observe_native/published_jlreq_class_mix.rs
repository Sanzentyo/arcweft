#[test]
fn agent_observe_native_renderer_reports_keycap_strict_class_mix_geometry() {
    assert_native_keycap_strict_class_mix_geometry("vertical_rl");
    assert_native_keycap_strict_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_keycap_strict_class_mix_raw_crops() {
    assert_native_keycap_strict_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_keycap_strict_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_keycap_strict_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_keycap_strict_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_unit_symbol_class_mix_geometry() {
    assert_native_published_jlreq_unit_symbol_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_unit_symbol_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_unit_symbol_class_mix_raw_crops() {
    assert_native_published_jlreq_unit_symbol_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_unit_symbol_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_published_jlreq_unit_symbol_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_unit_symbol_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_numeric_unit_class_mix_geometry() {
    assert_native_published_jlreq_numeric_unit_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_numeric_unit_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_numeric_unit_class_mix_raw_crops() {
    assert_native_published_jlreq_numeric_unit_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_numeric_unit_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_published_jlreq_numeric_unit_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_numeric_unit_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_numeric_separator_class_mix_geometry() {
    assert_native_published_jlreq_numeric_separator_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_numeric_separator_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_numeric_separator_class_mix_raw_crops() {
    assert_native_published_jlreq_numeric_separator_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_numeric_separator_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_published_jlreq_numeric_separator_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_numeric_separator_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_prefixed_abbreviation_class_mix_geometry()
{
    assert_native_published_jlreq_prefixed_abbreviation_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_prefixed_abbreviation_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_prefixed_abbreviation_class_mix_raw_crops()
{
    assert_native_published_jlreq_prefixed_abbreviation_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_prefixed_abbreviation_class_mix_raw_crop(
        "vertical_rl",
        "object-id",
    );
    assert_native_published_jlreq_prefixed_abbreviation_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_prefixed_abbreviation_class_mix_raw_crop(
        "vertical_lr",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_cent_prefixed_abbreviation_class_mix_geometry()
 {
    assert_native_published_jlreq_cent_prefixed_abbreviation_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_cent_prefixed_abbreviation_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_cent_prefixed_abbreviation_class_mix_raw_crops()
 {
    assert_native_published_jlreq_cent_prefixed_abbreviation_class_mix_raw_crop(
        "vertical_rl",
        "mask",
    );
    assert_native_published_jlreq_cent_prefixed_abbreviation_class_mix_raw_crop(
        "vertical_rl",
        "object-id",
    );
    assert_native_published_jlreq_cent_prefixed_abbreviation_class_mix_raw_crop(
        "vertical_lr",
        "mask",
    );
    assert_native_published_jlreq_cent_prefixed_abbreviation_class_mix_raw_crop(
        "vertical_lr",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_yen_prefixed_abbreviation_class_mix_geometry()
 {
    assert_native_published_jlreq_yen_prefixed_abbreviation_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_yen_prefixed_abbreviation_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_yen_prefixed_abbreviation_class_mix_raw_crops()
 {
    for label in ["yen-prefix", "fullwidth-yen-prefix"] {
        assert_native_published_jlreq_yen_prefixed_abbreviation_class_mix_raw_crop(
            "vertical_rl",
            label,
            "mask",
        );
        assert_native_published_jlreq_yen_prefixed_abbreviation_class_mix_raw_crop(
            "vertical_rl",
            label,
            "object-id",
        );
        assert_native_published_jlreq_yen_prefixed_abbreviation_class_mix_raw_crop(
            "vertical_lr",
            label,
            "mask",
        );
        assert_native_published_jlreq_yen_prefixed_abbreviation_class_mix_raw_crop(
            "vertical_lr",
            label,
            "object-id",
        );
    }
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_postfixed_abbreviation_class_mix_geometry()
{
    assert_native_published_jlreq_postfixed_abbreviation_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_postfixed_abbreviation_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_postfixed_abbreviation_class_mix_raw_crops()
{
    assert_native_published_jlreq_postfixed_abbreviation_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_postfixed_abbreviation_class_mix_raw_crop(
        "vertical_rl",
        "object-id",
    );
    assert_native_published_jlreq_postfixed_abbreviation_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_postfixed_abbreviation_class_mix_raw_crop(
        "vertical_lr",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_ideographic_abbreviation_class_mix_geometry()
 {
    assert_native_published_jlreq_ideographic_abbreviation_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_ideographic_abbreviation_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_ideographic_abbreviation_class_mix_raw_crops()
 {
    for label in ["prefix-ideographic", "suffix-ideographic"] {
        assert_native_published_jlreq_ideographic_abbreviation_class_mix_raw_crop(
            "vertical_rl",
            label,
            "mask",
        );
        assert_native_published_jlreq_ideographic_abbreviation_class_mix_raw_crop(
            "vertical_rl",
            label,
            "object-id",
        );
        assert_native_published_jlreq_ideographic_abbreviation_class_mix_raw_crop(
            "vertical_lr",
            label,
            "mask",
        );
        assert_native_published_jlreq_ideographic_abbreviation_class_mix_raw_crop(
            "vertical_lr",
            label,
            "object-id",
        );
    }
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_reference_mark_class_mix_geometry() {
    assert_native_published_jlreq_reference_mark_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_reference_mark_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_reference_mark_class_mix_raw_crops() {
    assert_native_published_jlreq_reference_mark_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_reference_mark_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_published_jlreq_reference_mark_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_reference_mark_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_parenthesized_reference_mark_class_mix_geometry()
 {
    assert_native_published_jlreq_parenthesized_reference_mark_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_parenthesized_reference_mark_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_parenthesized_reference_mark_class_mix_raw_crops()
 {
    assert_native_published_jlreq_parenthesized_reference_mark_class_mix_raw_crop(
        "vertical_rl",
        "mask",
    );
    assert_native_published_jlreq_parenthesized_reference_mark_class_mix_raw_crop(
        "vertical_rl",
        "object-id",
    );
    assert_native_published_jlreq_parenthesized_reference_mark_class_mix_raw_crop(
        "vertical_lr",
        "mask",
    );
    assert_native_published_jlreq_parenthesized_reference_mark_class_mix_raw_crop(
        "vertical_lr",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_temperature_suffix_class_mix_geometry() {
    assert_native_published_jlreq_temperature_suffix_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_temperature_suffix_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_temperature_suffix_class_mix_raw_crops() {
    assert_native_published_jlreq_temperature_suffix_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_temperature_suffix_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_published_jlreq_temperature_suffix_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_temperature_suffix_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_decomposed_temperature_class_mix_geometry()
{
    assert_native_published_jlreq_decomposed_temperature_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_decomposed_temperature_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_decomposed_temperature_class_mix_raw_crops()
{
    assert_native_published_jlreq_decomposed_temperature_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_decomposed_temperature_class_mix_raw_crop(
        "vertical_rl",
        "object-id",
    );
    assert_native_published_jlreq_decomposed_temperature_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_decomposed_temperature_class_mix_raw_crop(
        "vertical_lr",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_subscript_object_class_mix_geometry() {
    assert_native_published_jlreq_subscript_object_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_subscript_object_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_subscript_object_class_mix_raw_crops() {
    assert_native_published_jlreq_subscript_object_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_subscript_object_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_published_jlreq_subscript_object_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_subscript_object_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_greek_subscript_object_class_mix_geometry()
{
    assert_native_published_jlreq_greek_subscript_object_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_greek_subscript_object_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_greek_subscript_object_class_mix_raw_crops()
{
    assert_native_published_jlreq_greek_subscript_object_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_greek_subscript_object_class_mix_raw_crop(
        "vertical_rl",
        "object-id",
    );
    assert_native_published_jlreq_greek_subscript_object_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_greek_subscript_object_class_mix_raw_crop(
        "vertical_lr",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_greek_superscript_object_class_mix_geometry()
 {
    assert_native_published_jlreq_greek_superscript_object_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_greek_superscript_object_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_greek_superscript_object_class_mix_raw_crops()
 {
    assert_native_published_jlreq_greek_superscript_object_class_mix_raw_crop(
        "vertical_rl",
        "mask",
    );
    assert_native_published_jlreq_greek_superscript_object_class_mix_raw_crop(
        "vertical_rl",
        "object-id",
    );
    assert_native_published_jlreq_greek_superscript_object_class_mix_raw_crop(
        "vertical_lr",
        "mask",
    );
    assert_native_published_jlreq_greek_superscript_object_class_mix_raw_crop(
        "vertical_lr",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_strict_jlreq_style_split_geometry() {
    assert_native_strict_jlreq_style_split_geometry("vertical_rl");
    assert_native_strict_jlreq_style_split_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_strict_jlreq_style_split_raw_crops() {
    assert_native_strict_jlreq_style_split_raw_crop("vertical_rl", "mask");
    assert_native_strict_jlreq_style_split_raw_crop("vertical_rl", "object-id");
    assert_native_strict_jlreq_style_split_raw_crop("vertical_lr", "mask");
    assert_native_strict_jlreq_style_split_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_european_numeral_sequence_geometry() {
    assert_native_published_jlreq_european_numeral_sequence_geometry("vertical_rl", false);
    assert_native_published_jlreq_european_numeral_sequence_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_european_numeral_sequence_raw_crops() {
    assert_native_published_jlreq_european_numeral_sequence_raw_crop("vertical_rl", false, "mask");
    assert_native_published_jlreq_european_numeral_sequence_raw_crop(
        "vertical_rl",
        false,
        "object-id",
    );
    assert_native_published_jlreq_european_numeral_sequence_raw_crop("vertical_lr", true, "mask");
    assert_native_published_jlreq_european_numeral_sequence_raw_crop(
        "vertical_lr",
        true,
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_numeric_separator_geometry() {
    assert_native_published_jlreq_numeric_separator_geometry("vertical_rl", false);
    assert_native_published_jlreq_numeric_separator_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_numeric_separator_raw_crops() {
    assert_native_published_jlreq_numeric_separator_raw_crop("vertical_rl", false, "mask");
    assert_native_published_jlreq_numeric_separator_raw_crop("vertical_rl", false, "object-id");
    assert_native_published_jlreq_numeric_separator_raw_crop("vertical_lr", true, "mask");
    assert_native_published_jlreq_numeric_separator_raw_crop("vertical_lr", true, "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_numeric_abbreviation_geometry() {
    assert_native_published_jlreq_numeric_abbreviation_geometry("vertical_rl", false);
    assert_native_published_jlreq_numeric_abbreviation_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_numeric_prefix_abbreviation_raw_crops() {
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_rl",
        false,
        "$",
        "prefix",
        "object.dialogue.0.0.cluster.1.3.4",
        "mask",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_rl",
        false,
        "$",
        "prefix",
        "object.dialogue.0.0.cluster.1.3.4",
        "object-id",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_lr",
        true,
        "$",
        "prefix",
        "object.dialogue.0.0.cluster.1.3.4",
        "mask",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_lr",
        true,
        "$",
        "prefix",
        "object.dialogue.0.0.cluster.1.3.4",
        "object-id",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_rl",
        false,
        "¢",
        "cent-prefix",
        "object.dialogue.0.0.cluster.1.3.5",
        "mask",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_rl",
        false,
        "¢",
        "cent-prefix",
        "object.dialogue.0.0.cluster.1.3.5",
        "object-id",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_lr",
        true,
        "¢",
        "cent-prefix",
        "object.dialogue.0.0.cluster.1.3.5",
        "mask",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_lr",
        true,
        "¢",
        "cent-prefix",
        "object.dialogue.0.0.cluster.1.3.5",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_numeric_suffix_abbreviation_raw_crops() {
    for (mark, label, object_id) in [
        ("%", "suffix", "object.dialogue.0.0.cluster.2.5.6"),
        (
            "℃",
            "temperature-suffix",
            "object.dialogue.0.0.cluster.2.5.8",
        ),
        (
            "°",
            "temperature-suffix-decomposed",
            "object.dialogue.0.0.cluster.2.5.7",
        ),
        (
            "C",
            "temperature-suffix-decomposed",
            "object.dialogue.0.0.cluster.3.7.8",
        ),
    ] {
        for (writing_mode, next_column_moves_right) in
            [("vertical_rl", false), ("vertical_lr", true)]
        {
            for capture_kind in ["mask", "object-id"] {
                assert_native_published_jlreq_numeric_abbreviation_raw_crop(
                    writing_mode,
                    next_column_moves_right,
                    mark,
                    label,
                    object_id,
                    capture_kind,
                );
            }
        }
    }
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_ideographic_numeric_abbreviation_raw_crops()
{
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_rl",
        false,
        "$",
        "prefix-ideographic",
        "object.dialogue.0.0.cluster.1.3.4",
        "mask",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_rl",
        false,
        "$",
        "prefix-ideographic",
        "object.dialogue.0.0.cluster.1.3.4",
        "object-id",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_lr",
        true,
        "$",
        "prefix-ideographic",
        "object.dialogue.0.0.cluster.1.3.4",
        "mask",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_lr",
        true,
        "$",
        "prefix-ideographic",
        "object.dialogue.0.0.cluster.1.3.4",
        "object-id",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_rl",
        false,
        "%",
        "suffix-ideographic",
        "object.dialogue.0.0.cluster.2.6.7",
        "mask",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_rl",
        false,
        "%",
        "suffix-ideographic",
        "object.dialogue.0.0.cluster.2.6.7",
        "object-id",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_lr",
        true,
        "%",
        "suffix-ideographic",
        "object.dialogue.0.0.cluster.2.6.7",
        "mask",
    );
    assert_native_published_jlreq_numeric_abbreviation_raw_crop(
        "vertical_lr",
        true,
        "%",
        "suffix-ideographic",
        "object.dialogue.0.0.cluster.2.6.7",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_reference_mark_geometry() {
    assert_native_published_jlreq_reference_mark_geometry("vertical_rl");
    assert_native_published_jlreq_reference_mark_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_reference_mark_raw_crops() {
    assert_native_published_jlreq_reference_mark_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_reference_mark_raw_crop("vertical_rl", "object-id");
    assert_native_published_jlreq_reference_mark_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_reference_mark_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_parenthesized_reference_mark_geometry() {
    assert_native_published_jlreq_parenthesized_reference_mark_geometry("vertical_rl", false);
    assert_native_published_jlreq_parenthesized_reference_mark_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_parenthesized_reference_mark_raw_crops() {
    assert_native_published_jlreq_parenthesized_reference_mark_raw_crop(
        "vertical_rl",
        false,
        "mask",
    );
    assert_native_published_jlreq_parenthesized_reference_mark_raw_crop(
        "vertical_rl",
        false,
        "object-id",
    );
    assert_native_published_jlreq_parenthesized_reference_mark_raw_crop(
        "vertical_lr",
        true,
        "mask",
    );
    assert_native_published_jlreq_parenthesized_reference_mark_raw_crop(
        "vertical_lr",
        true,
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_latin_unit_geometry() {
    assert_native_published_jlreq_latin_unit_geometry("vertical_rl", false);
    assert_native_published_jlreq_latin_unit_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_latin_unit_raw_crops() {
    assert_native_published_jlreq_latin_unit_raw_crop("vertical_rl", false, "mask");
    assert_native_published_jlreq_latin_unit_raw_crop("vertical_rl", false, "object-id");
    assert_native_published_jlreq_latin_unit_raw_crop("vertical_lr", true, "mask");
    assert_native_published_jlreq_latin_unit_raw_crop("vertical_lr", true, "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_western_word_geometry() {
    assert_native_published_jlreq_western_word_geometry("vertical_rl", false);
    assert_native_published_jlreq_western_word_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_western_word_raw_crops() {
    assert_native_published_jlreq_western_word_raw_crop("vertical_rl", false, "mask");
    assert_native_published_jlreq_western_word_raw_crop("vertical_rl", false, "object-id");
    assert_native_published_jlreq_western_word_raw_crop("vertical_lr", true, "mask");
    assert_native_published_jlreq_western_word_raw_crop("vertical_lr", true, "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_numeric_unit_geometry() {
    assert_native_published_jlreq_numeric_unit_geometry("vertical_rl", false);
    assert_native_published_jlreq_numeric_unit_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_numeric_unit_raw_crops() {
    assert_native_published_jlreq_numeric_unit_raw_crop("vertical_rl", false, "mask");
    assert_native_published_jlreq_numeric_unit_raw_crop("vertical_rl", false, "object-id");
    assert_native_published_jlreq_numeric_unit_raw_crop("vertical_lr", true, "mask");
    assert_native_published_jlreq_numeric_unit_raw_crop("vertical_lr", true, "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_hyphenated_western_word_geometry() {
    assert_native_published_jlreq_hyphenated_western_word_geometry("vertical_rl", false);
    assert_native_published_jlreq_hyphenated_western_word_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_hyphenated_western_word_raw_crops() {
    assert_native_published_jlreq_hyphenated_western_word_raw_crop("vertical_rl", false, "mask");
    assert_native_published_jlreq_hyphenated_western_word_raw_crop(
        "vertical_rl",
        false,
        "object-id",
    );
    assert_native_published_jlreq_hyphenated_western_word_raw_crop("vertical_lr", true, "mask");
    assert_native_published_jlreq_hyphenated_western_word_raw_crop(
        "vertical_lr",
        true,
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_apostrophe_western_word_geometry() {
    assert_native_published_jlreq_apostrophe_western_word_geometry("vertical_rl", false);
    assert_native_published_jlreq_apostrophe_western_word_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_apostrophe_western_word_raw_crops() {
    assert_native_published_jlreq_apostrophe_western_word_raw_crop("vertical_rl", false, "mask");
    assert_native_published_jlreq_apostrophe_western_word_raw_crop(
        "vertical_rl",
        false,
        "object-id",
    );
    assert_native_published_jlreq_apostrophe_western_word_raw_crop("vertical_lr", true, "mask");
    assert_native_published_jlreq_apostrophe_western_word_raw_crop(
        "vertical_lr",
        true,
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_accented_latin_word_geometry() {
    assert_native_published_jlreq_accented_latin_word_geometry("vertical_rl");
    assert_native_published_jlreq_accented_latin_word_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_accented_latin_word_raw_crops() {
    assert_native_published_jlreq_accented_latin_word_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_accented_latin_word_raw_crop("vertical_rl", "object-id");
    assert_native_published_jlreq_accented_latin_word_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_accented_latin_word_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_greek_latin_unit_geometry() {
    assert_native_published_jlreq_greek_latin_unit_geometry("vertical_rl");
    assert_native_published_jlreq_greek_latin_unit_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_greek_latin_unit_raw_crops() {
    assert_native_published_jlreq_greek_latin_unit_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_greek_latin_unit_raw_crop("vertical_rl", "object-id");
    assert_native_published_jlreq_greek_latin_unit_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_greek_latin_unit_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_subscript_object_geometry() {
    assert_native_published_jlreq_subscript_object_geometry("vertical_rl", false);
    assert_native_published_jlreq_subscript_object_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_subscript_object_raw_crops() {
    assert_native_published_jlreq_subscript_object_raw_crop("vertical_rl", false, "mask");
    assert_native_published_jlreq_subscript_object_raw_crop("vertical_rl", false, "object-id");
    assert_native_published_jlreq_subscript_object_raw_crop("vertical_lr", true, "mask");
    assert_native_published_jlreq_subscript_object_raw_crop("vertical_lr", true, "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_strict_jlreq_ruby_text_combine_geometry() {
    assert_native_strict_jlreq_ruby_text_combine_geometry("vertical_rl", true);
    assert_native_strict_jlreq_ruby_text_combine_geometry("vertical_lr", false);
}

#[test]
fn agent_observe_native_renderer_writes_strict_jlreq_ruby_text_combine_raw_crops() {
    assert_native_strict_jlreq_ruby_text_combine_raw_crop("vertical_rl", "mask");
    assert_native_strict_jlreq_ruby_text_combine_raw_crop("vertical_rl", "object-id");
    assert_native_strict_jlreq_ruby_text_combine_raw_crop("vertical_lr", "mask");
    assert_native_strict_jlreq_ruby_text_combine_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_strict_jlreq_hard_break_segment_geometry() {
    assert_native_strict_jlreq_hard_break_segment_geometry("vertical_rl", false);
    assert_native_strict_jlreq_hard_break_segment_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_strict_jlreq_hard_break_segment_raw_crops() {
    assert_native_strict_jlreq_hard_break_segment_raw_crop("vertical_rl", "mask", false);
    assert_native_strict_jlreq_hard_break_segment_raw_crop("vertical_rl", "object-id", false);
    assert_native_strict_jlreq_hard_break_segment_raw_crop("vertical_lr", "mask", true);
    assert_native_strict_jlreq_hard_break_segment_raw_crop("vertical_lr", "object-id", true);
}

fn assert_native_strict_jlreq_paragraph_class_mix_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let loose = observe_native_jlreq_paragraph_class_mix_fixture(writing_mode, "loose");
    let strict = observe_native_jlreq_paragraph_class_mix_fixture(writing_mode, "strict");
    assert_native_rich_text_layer_image_has_content(&loose);
    assert_native_rich_text_layer_image_has_content(&strict);

    assert_eq!(
        first_text_run_presentation_layout(&loose)["jlreq_strictness"],
        "loose"
    );
    assert_eq!(
        first_text_run_presentation_layout(&strict)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&strict)["writing_mode"],
        writing_mode
    );

    let loose_full_stop = find_rich_text_cluster_object(&loose, "。", 36, 39);
    let loose_open = find_rich_text_cluster_object(&loose, "「", 39, 42);
    assert_next_paragraph_column(
        loose_full_stop,
        loose_open,
        next_column_moves_right,
        "loose published-style paragraph may break between closing and opening punctuation",
    );

    let person = find_rich_text_cluster_object(&strict, "人", 33, 36);
    let strict_full_stop = find_rich_text_cluster_object(&strict, "。", 36, 39);
    let strict_open = find_rich_text_cluster_object(&strict, "「", 39, 42);
    let river = find_rich_text_cluster_object(&strict, "川", 42, 45);
    let close = find_rich_text_cluster_object(&strict, "」", 45, 48);
    assert_vertical_cluster_after(
        person,
        strict_full_stop,
        "strict paragraph class mix keeps closing punctuation after its base",
    );
    assert_vertical_cluster_after(
        strict_full_stop,
        strict_open,
        "strict paragraph class mix keeps adjacent closing/opening punctuation together",
    );
    assert_vertical_cluster_after(
        strict_open,
        river,
        "strict paragraph class mix keeps opening punctuation with its base",
    );
    assert_vertical_cluster_after(
        river,
        close,
        "strict paragraph class mix keeps closing bracket with its base",
    );
    assert_eq!(
        strict_open["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    assert_rich_text_object_has_mask_capture(
        strict_open,
        "strict paragraph class-mix opening cluster",
    );
}

fn observe_native_jlreq_paragraph_class_mix_fixture(
    writing_mode: &str,
    strictness: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-jlreq-paragraph-class-mix-{strictness}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq={strictness}]天地春夏秋冬月火、山々人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp JLREQ paragraph class-mix source");
    json
}

fn assert_native_strict_jlreq_paragraph_class_mix_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-strict-jlreq-paragraph-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬月火、山々人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-strict-jlreq-paragraph-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.13.39.42")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native strict JLREQ paragraph class-mix raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} strict JLREQ paragraph class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native strict JLREQ paragraph class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let opening = assert_native_strict_jlreq_paragraph_class_mix_attached_opening(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], opening["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], opening["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], opening["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], opening["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(opening),
            content_pixels,
            &format!("{writing_mode} strict JLREQ paragraph class-mix object-id crop"),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native strict JLREQ paragraph class-mix mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp strict JLREQ paragraph class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp strict JLREQ paragraph class-mix dir");
}

fn assert_native_strict_jlreq_paragraph_class_mix_attached_opening(
    json: &serde_json::Value,
) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "strict"
    );
    let person = find_rich_text_cluster_object(json, "人", 33, 36);
    let full_stop = find_rich_text_cluster_object(json, "。", 36, 39);
    let opening = find_rich_text_cluster_object(json, "「", 39, 42);
    let river = find_rich_text_cluster_object(json, "川", 42, 45);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class-mix raw crop keeps closing punctuation after its base",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class-mix raw crop keeps adjacent closing/opening punctuation together",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class-mix raw crop keeps opening punctuation with its base",
    );
    assert_eq!(opening["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(
        opening["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    opening
}

fn assert_native_published_jlreq_plain_western_word_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_published_jlreq_plain_western_word_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_plain_western_word_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_plain_western_word_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-plain-western-word-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬Web人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ plain Western word class-mix source");
    json
}

fn assert_native_published_jlreq_plain_western_word_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-plain-western-word-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬Web人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-plain-western-word-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.8.20.21")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ plain Western word class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ plain Western word class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ plain Western word class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let word_end =
        assert_native_published_jlreq_plain_western_word_class_mix_objects(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], word_end["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], word_end["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], word_end["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], word_end["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(word_end),
            content_pixels,
            &format!("{writing_mode} published JLREQ plain Western word class-mix object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path)
            .expect("read native published JLREQ plain Western word class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path)
        .expect("remove temp published JLREQ plain Western word class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ plain Western word class-mix dir");
}

fn assert_native_published_jlreq_plain_western_word_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let first = find_rich_text_cluster_object(json, "W", 18, 19);
    let second = find_rich_text_cluster_object(json, "e", 19, 20);
    let last = find_rich_text_cluster_object(json, "b", 20, 21);
    assert_vertical_cluster_after(
        first,
        second,
        "published JLREQ plain Western word class mix keeps leading letters together",
    );
    assert_vertical_cluster_after(
        second,
        last,
        "published JLREQ plain Western word class mix keeps final letter attached",
    );

    let person = find_rich_text_cluster_object(json, "人", 21, 24);
    let full_stop = find_rich_text_cluster_object(json, "。", 24, 27);
    let opening = find_rich_text_cluster_object(json, "「", 27, 30);
    let river = find_rich_text_cluster_object(json, "川", 30, 33);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a plain Western word",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a plain Western word",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a plain Western word",
    );
    assert_rich_text_object_has_mask_capture(
        last,
        &format!("{writing_mode} published JLREQ plain Western word class-mix final letter"),
    );
    assert_eq!(last["rich_text_ref"]["orientation"], "sideways_cw");
    last
}

fn assert_native_published_jlreq_western_word_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_published_jlreq_western_word_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_western_word_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_western_word_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-published-jlreq-western-word-class-mix"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬Web-Test人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp published JLREQ Western word class-mix source");
    json
}

fn assert_native_published_jlreq_western_word_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-western-word-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬Web-Test人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-western-word-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.9.21.22")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ Western word class-mix raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ Western word class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ Western word class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let hyphen = assert_native_published_jlreq_western_word_class_mix_objects(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], hyphen["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], hyphen["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], hyphen["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], hyphen["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(hyphen),
            content_pixels,
            &format!("{writing_mode} published JLREQ Western word class-mix object-id crop"),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native published JLREQ Western word class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ Western word class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ Western word class-mix dir");
}

fn assert_native_published_jlreq_western_word_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let first = find_rich_text_cluster_object(json, "W", 18, 19);
    let second = find_rich_text_cluster_object(json, "e", 19, 20);
    let before_hyphen = find_rich_text_cluster_object(json, "b", 20, 21);
    let hyphen = find_rich_text_cluster_object(json, "-", 21, 22);
    let after_hyphen = find_rich_text_cluster_object(json, "T", 22, 23);
    let after_hyphen_second = find_rich_text_cluster_object(json, "e", 23, 24);
    let after_hyphen_third = find_rich_text_cluster_object(json, "s", 24, 25);
    let last = find_rich_text_cluster_object(json, "t", 25, 26);
    assert_vertical_cluster_after(
        first,
        second,
        "published JLREQ Western word class mix keeps leading letters together",
    );
    assert_vertical_cluster_after(
        second,
        before_hyphen,
        "published JLREQ Western word class mix keeps letters before the hyphen together",
    );
    assert_vertical_cluster_after(
        before_hyphen,
        hyphen,
        "published JLREQ Western word class mix keeps the word-internal hyphen attached",
    );
    assert_vertical_cluster_after(
        hyphen,
        after_hyphen,
        "published JLREQ Western word class mix keeps letters after the hyphen attached",
    );
    assert_vertical_cluster_after(
        after_hyphen,
        after_hyphen_second,
        "published JLREQ Western word class mix keeps letters after the hyphen together",
    );
    assert_vertical_cluster_after(
        after_hyphen_second,
        after_hyphen_third,
        "published JLREQ Western word class mix keeps letters after the hyphen together",
    );
    assert_vertical_cluster_after(
        after_hyphen_third,
        last,
        "published JLREQ Western word class mix keeps the final letter attached",
    );

    let person = find_rich_text_cluster_object(json, "人", 26, 29);
    let full_stop = find_rich_text_cluster_object(json, "。", 29, 32);
    let opening = find_rich_text_cluster_object(json, "「", 32, 35);
    let river = find_rich_text_cluster_object(json, "川", 35, 38);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a Western word",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a Western word",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a Western word",
    );
    assert_rich_text_object_has_mask_capture(
        hyphen,
        &format!("{writing_mode} published JLREQ Western word class-mix hyphen"),
    );
    assert_eq!(hyphen["rich_text_ref"]["orientation"], "sideways_cw");
    hyphen
}

fn assert_native_published_jlreq_apostrophe_western_word_class_mix_geometry(writing_mode: &str) {
    for case in native_published_jlreq_apostrophe_western_word_class_mix_cases() {
        let json = observe_native_published_jlreq_apostrophe_western_word_class_mix_fixture(
            writing_mode,
            case,
        );
        assert_native_rich_text_layer_image_has_content(&json);
        assert_eq!(
            first_text_run_presentation_layout(&json)["jlreq_strictness"],
            "strict"
        );
        assert_eq!(
            first_text_run_presentation_layout(&json)["writing_mode"],
            writing_mode
        );
        assert_native_published_jlreq_apostrophe_western_word_class_mix_objects(
            &json,
            writing_mode,
            case,
        );
    }
}

fn observe_native_published_jlreq_apostrophe_western_word_class_mix_fixture(
    writing_mode: &str,
    case: NativeApostropheWesternWordClassMixCase,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-apostrophe-western-word-class-mix-{}",
            case.label
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{}[/][p]
}}
",
            case.text
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ apostrophe Western word class-mix source");
    json
}

fn assert_native_published_jlreq_apostrophe_western_word_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    for case in native_published_jlreq_apostrophe_western_word_class_mix_cases() {
        assert_native_published_jlreq_apostrophe_western_word_class_mix_case_raw_crop(
            writing_mode,
            capture_kind,
            case,
        );
    }
}

fn assert_native_published_jlreq_apostrophe_western_word_class_mix_case_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
    case: NativeApostropheWesternWordClassMixCase,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-apostrophe-western-word-class-mix-{}-{capture_kind}",
        case.label
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{}[/][p]
}}
",
            case.text
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-apostrophe-western-word-class-mix-{}-{capture_kind}.rgba",
        case.label
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg(case.object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ apostrophe Western word class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ apostrophe Western word class-mix {} {capture_kind} crop should succeed, stderr: {}",
        case.label,
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ apostrophe Western word class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let apostrophe = assert_native_published_jlreq_apostrophe_western_word_class_mix_objects(
        &json,
        writing_mode,
        case,
    );
    assert_native_published_jlreq_apostrophe_western_word_class_mix_crop_pixels(
        &json,
        apostrophe,
        &raw_path,
        writing_mode,
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ apostrophe Western word class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ apostrophe Western word class-mix dir");
}

fn assert_native_published_jlreq_apostrophe_western_word_class_mix_crop_pixels(
    json: &serde_json::Value,
    apostrophe: &serde_json::Value,
    raw_path: &Path,
    writing_mode: &str,
    capture_kind: &str,
) {
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        apostrophe["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        apostrophe["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], apostrophe["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], apostrophe["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            raw_path,
            agent_object_id_color_from_json(apostrophe),
            content_pixels,
            &format!(
                "{writing_mode} published JLREQ apostrophe Western word class-mix object-id crop"
            ),
        );
    } else {
        let bytes = fs::read(raw_path)
            .expect("read native published JLREQ apostrophe Western word class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }
}

fn assert_native_published_jlreq_apostrophe_western_word_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    case: NativeApostropheWesternWordClassMixCase,
) -> &'report serde_json::Value {
    let first = find_rich_text_cluster_object(json, "O", 18, 19);
    let apostrophe = find_rich_text_cluster_object(
        json,
        case.apostrophe,
        case.apostrophe_start,
        case.apostrophe_end,
    );
    let after_apostrophe =
        find_rich_text_cluster_object(json, "K", case.after_start, case.after_end);
    assert_vertical_cluster_after(
        first,
        apostrophe,
        "published JLREQ apostrophe Western word class mix keeps apostrophe with preceding letter",
    );
    assert_vertical_cluster_after(
        apostrophe,
        after_apostrophe,
        "published JLREQ apostrophe Western word class mix keeps following letter attached",
    );

    let person = find_rich_text_cluster_object(json, "人", case.person_start, case.person_end);
    let full_stop = find_rich_text_cluster_object(json, "。", case.person_end, case.full_stop_end);
    let opening = find_rich_text_cluster_object(json, "「", case.full_stop_end, case.opening_end);
    let river = find_rich_text_cluster_object(json, "川", case.opening_end, case.river_end);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after an apostrophe Western word",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after an apostrophe Western word",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after an apostrophe Western word",
    );
    assert_rich_text_object_has_mask_capture(
        apostrophe,
        &format!(
            "{writing_mode} published JLREQ apostrophe Western word class-mix {} apostrophe",
            case.label
        ),
    );
    apostrophe
}

#[derive(Clone, Copy)]
struct NativeApostropheWesternWordClassMixCase {
    label: &'static str,
    text: &'static str,
    apostrophe: &'static str,
    object_id: &'static str,
    apostrophe_start: u64,
    apostrophe_end: u64,
    after_start: u64,
    after_end: u64,
    person_start: u64,
    person_end: u64,
    full_stop_end: u64,
    opening_end: u64,
    river_end: u64,
}

const fn native_published_jlreq_apostrophe_western_word_class_mix_cases()
-> [NativeApostropheWesternWordClassMixCase; 2] {
    [
        NativeApostropheWesternWordClassMixCase {
            label: "ascii",
            text: "天地春夏秋冬O'K人。「川」あっいおーえ―中・外………終",
            apostrophe: "'",
            object_id: "object.dialogue.0.0.cluster.7.19.20",
            apostrophe_start: 19,
            apostrophe_end: 20,
            after_start: 20,
            after_end: 21,
            person_start: 21,
            person_end: 24,
            full_stop_end: 27,
            opening_end: 30,
            river_end: 33,
        },
        NativeApostropheWesternWordClassMixCase {
            label: "typographic",
            text: "天地春夏秋冬O’K人。「川」あっいおーえ―中・外………終",
            apostrophe: "’",
            object_id: "object.dialogue.0.0.cluster.7.19.22",
            apostrophe_start: 19,
            apostrophe_end: 22,
            after_start: 22,
            after_end: 23,
            person_start: 23,
            person_end: 26,
            full_stop_end: 29,
            opening_end: 32,
            river_end: 35,
        },
    ]
}

fn assert_native_published_jlreq_accented_latin_word_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_published_jlreq_accented_latin_word_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_accented_latin_word_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_accented_latin_word_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-accented-latin-word-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬café人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ accented Latin word class-mix source");
    json
}

fn assert_native_published_jlreq_accented_latin_word_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-accented-latin-word-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬café人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-accented-latin-word-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.9.21.23")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ accented Latin word class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ accented Latin word class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ accented Latin word class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let accented =
        assert_native_published_jlreq_accented_latin_word_class_mix_objects(&json, writing_mode);
    assert_native_published_jlreq_accented_latin_word_class_mix_crop_pixels(
        &json,
        accented,
        &raw_path,
        writing_mode,
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ accented Latin word class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ accented Latin word class-mix dir");
}

fn assert_native_published_jlreq_accented_latin_word_class_mix_crop_pixels(
    json: &serde_json::Value,
    accented: &serde_json::Value,
    raw_path: &Path,
    writing_mode: &str,
    capture_kind: &str,
) {
    assert_eq!(json["images"][0]["crop_origin"]["x"], accented["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], accented["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], accented["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], accented["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            raw_path,
            agent_object_id_color_from_json(accented),
            content_pixels,
            &format!("{writing_mode} published JLREQ accented Latin word class-mix object-id crop"),
        );
    } else {
        let bytes =
            fs::read(raw_path).expect("read native published JLREQ accented Latin class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }
}

fn assert_native_published_jlreq_accented_latin_word_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let first = find_rich_text_cluster_object(json, "c", 18, 19);
    let second = find_rich_text_cluster_object(json, "a", 19, 20);
    let before_accent = find_rich_text_cluster_object(json, "f", 20, 21);
    let accented = find_rich_text_cluster_object(json, "é", 21, 23);
    assert_vertical_cluster_after(
        first,
        second,
        "published JLREQ accented Latin word class mix keeps leading letters together",
    );
    assert_vertical_cluster_after(
        second,
        before_accent,
        "published JLREQ accented Latin word class mix keeps letters before accent together",
    );
    assert_vertical_cluster_after(
        before_accent,
        accented,
        "published JLREQ accented Latin word class mix keeps accented grapheme attached",
    );

    let person = find_rich_text_cluster_object(json, "人", 23, 26);
    let full_stop = find_rich_text_cluster_object(json, "。", 26, 29);
    let opening = find_rich_text_cluster_object(json, "「", 29, 32);
    let river = find_rich_text_cluster_object(json, "川", 32, 35);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after an accented Latin word",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after an accented Latin word",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after an accented Latin word",
    );
    assert_rich_text_object_has_mask_capture(
        accented,
        &format!("{writing_mode} published JLREQ accented Latin word class-mix accented grapheme"),
    );
    accented
}

fn assert_native_published_jlreq_decomposed_accented_latin_word_class_mix_geometry(
    writing_mode: &str,
) {
    let json = observe_native_published_jlreq_decomposed_accented_latin_word_class_mix_fixture(
        writing_mode,
    );
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_decomposed_accented_latin_word_class_mix_objects(
        &json,
        writing_mode,
    );
}

fn observe_native_published_jlreq_decomposed_accented_latin_word_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let text = "天地春夏秋冬cafe\u{301}人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-decomposed-accented-latin-word-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ decomposed accented Latin word class-mix source");
    json
}

fn assert_native_published_jlreq_decomposed_accented_latin_word_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-decomposed-accented-latin-word-class-mix-{capture_kind}"
    );
    let text = "天地春夏秋冬cafe\u{301}人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-decomposed-accented-latin-word-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.9.21.24")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ decomposed accented Latin word class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ decomposed accented Latin word class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ decomposed accented Latin word class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let accented = assert_native_published_jlreq_decomposed_accented_latin_word_class_mix_objects(
        &json,
        writing_mode,
    );
    assert_native_published_jlreq_accented_latin_word_class_mix_crop_pixels(
        &json,
        accented,
        &raw_path,
        writing_mode,
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ decomposed accented Latin word class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ decomposed accented Latin word class-mix dir");
}

fn assert_native_published_jlreq_decomposed_accented_latin_word_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let first = find_rich_text_cluster_object(json, "c", 18, 19);
    let second = find_rich_text_cluster_object(json, "a", 19, 20);
    let before_accent = find_rich_text_cluster_object(json, "f", 20, 21);
    let accented = find_rich_text_cluster_object(json, "e\u{301}", 21, 24);
    assert_vertical_cluster_after(
        first,
        second,
        "published JLREQ decomposed accented Latin word class mix keeps leading letters together",
    );
    assert_vertical_cluster_after(
        second,
        before_accent,
        "published JLREQ decomposed accented Latin word class mix keeps letters before accent together",
    );
    assert_vertical_cluster_after(
        before_accent,
        accented,
        "published JLREQ decomposed accented Latin word class mix keeps accented grapheme attached",
    );

    let person = find_rich_text_cluster_object(json, "人", 24, 27);
    let full_stop = find_rich_text_cluster_object(json, "。", 27, 30);
    let opening = find_rich_text_cluster_object(json, "「", 30, 33);
    let river = find_rich_text_cluster_object(json, "川", 33, 36);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a decomposed accented Latin word",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a decomposed accented Latin word",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a decomposed accented Latin word",
    );
    assert_rich_text_object_has_mask_capture(
        accented,
        &format!(
            "{writing_mode} published JLREQ decomposed accented Latin word class-mix accented grapheme"
        ),
    );
    assert_eq!(accented["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(accented["rich_text_ref"]["range"]["start"], 21);
    assert_eq!(accented["rich_text_ref"]["range"]["end"], 24);
    accented
}

fn assert_native_zwj_grapheme_strict_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_zwj_grapheme_strict_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_zwj_grapheme_strict_class_mix_objects(&json, writing_mode);
}

fn observe_native_zwj_grapheme_strict_class_mix_fixture(writing_mode: &str) -> serde_json::Value {
    let text = "天地春夏秋冬👩‍💻人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-zwj-grapheme-strict-class-mix"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp ZWJ grapheme strict class-mix source");
    json
}

fn assert_native_zwj_grapheme_strict_class_mix_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name =
        format!("agent-observe-native-{writing_mode}-zwj-grapheme-strict-class-mix-{capture_kind}");
    let text = "天地春夏秋冬👩‍💻人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-zwj-grapheme-strict-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.6.18.29")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native ZWJ grapheme strict class-mix raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} ZWJ grapheme strict class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native ZWJ grapheme strict class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let cluster = assert_native_zwj_grapheme_strict_class_mix_objects(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], cluster["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], cluster["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], cluster["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], cluster["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(cluster),
            content_pixels,
            &format!("{writing_mode} ZWJ grapheme strict class-mix object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native ZWJ grapheme strict class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp ZWJ grapheme strict class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp ZWJ grapheme strict class-mix dir");
}

fn assert_native_zwj_grapheme_strict_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let cluster = find_rich_text_cluster_object(json, "👩‍💻", 18, 29);
    assert_eq!(cluster["rich_text_ref"]["kind"], "glyph_cluster");
    assert_eq!(cluster["rich_text_ref"]["orientation"], "upright");
    assert_eq!(cluster["rich_text_ref"]["vertical_form"], "none");
    assert_eq!(cluster["rich_text_ref"]["range"]["start"], 18);
    assert_eq!(cluster["rich_text_ref"]["range"]["end"], 29);

    let person = find_rich_text_cluster_object(json, "人", 29, 32);
    let full_stop = find_rich_text_cluster_object(json, "。", 32, 35);
    let opening = find_rich_text_cluster_object(json, "「", 35, 38);
    let river = find_rich_text_cluster_object(json, "川", 38, 41);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a ZWJ grapheme",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a ZWJ grapheme",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a ZWJ grapheme",
    );
    assert_rich_text_object_has_mask_capture(
        cluster,
        &format!("{writing_mode} ZWJ grapheme strict class-mix cluster"),
    );
    cluster
}

fn assert_native_decomposed_kana_strict_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_decomposed_kana_strict_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_decomposed_kana_strict_class_mix_objects(&json, writing_mode);
}

fn observe_native_decomposed_kana_strict_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let text = "天地春夏秋冬か\u{3099}人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-decomposed-kana-strict-class-mix"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp decomposed kana strict class-mix source");
    json
}

fn assert_native_decomposed_kana_strict_class_mix_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-decomposed-kana-strict-class-mix-{capture_kind}"
    );
    let text = "天地春夏秋冬か\u{3099}人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-decomposed-kana-strict-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.6.18.24")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native decomposed kana strict class-mix raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} decomposed kana strict class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native decomposed kana strict class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let kana = assert_native_decomposed_kana_strict_class_mix_objects(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], kana["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], kana["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], kana["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], kana["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(kana),
            content_pixels,
            &format!("{writing_mode} decomposed kana strict class-mix object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native decomposed kana strict class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp decomposed kana strict class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp decomposed kana strict class-mix dir");
}

fn assert_native_decomposed_kana_strict_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let kana = find_rich_text_cluster_object(json, "か\u{3099}", 18, 24);
    assert_eq!(kana["rich_text_ref"]["kind"], "glyph_cluster");
    assert_eq!(kana["rich_text_ref"]["orientation"], "upright");
    assert_eq!(kana["rich_text_ref"]["vertical_form"], "none");
    assert_eq!(kana["rich_text_ref"]["range"]["start"], 18);
    assert_eq!(kana["rich_text_ref"]["range"]["end"], 24);

    let person = find_rich_text_cluster_object(json, "人", 24, 27);
    let full_stop = find_rich_text_cluster_object(json, "。", 27, 30);
    let opening = find_rich_text_cluster_object(json, "「", 30, 33);
    let river = find_rich_text_cluster_object(json, "川", 33, 36);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a decomposed kana grapheme",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a decomposed kana grapheme",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a decomposed kana grapheme",
    );
    assert_rich_text_object_has_mask_capture(
        kana,
        &format!("{writing_mode} decomposed kana strict class-mix cluster"),
    );
    kana
}

fn assert_native_variation_selector_strict_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_variation_selector_strict_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_variation_selector_strict_class_mix_objects(&json, writing_mode);
}

fn observe_native_variation_selector_strict_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let text = "天地春夏秋冬漢\u{fe00}人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-variation-selector-strict-class-mix"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp variation selector strict class-mix source");
    json
}

fn assert_native_variation_selector_strict_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-variation-selector-strict-class-mix-{capture_kind}"
    );
    let text = "天地春夏秋冬漢\u{fe00}人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-variation-selector-strict-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.6.18.24")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native variation selector strict class-mix raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} variation selector strict class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native variation selector strict class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let variant = assert_native_variation_selector_strict_class_mix_objects(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], variant["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], variant["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], variant["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], variant["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(variant),
            content_pixels,
            &format!("{writing_mode} variation selector strict class-mix object-id crop"),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native variation selector strict class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp variation selector strict class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp variation selector strict class-mix dir");
}

fn assert_native_variation_selector_strict_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let variant = find_rich_text_cluster_object(json, "漢\u{fe00}", 18, 24);
    assert_eq!(variant["rich_text_ref"]["kind"], "glyph_cluster");
    assert_eq!(variant["rich_text_ref"]["orientation"], "upright");
    assert_eq!(variant["rich_text_ref"]["vertical_form"], "none");
    assert_eq!(variant["rich_text_ref"]["range"]["start"], 18);
    assert_eq!(variant["rich_text_ref"]["range"]["end"], 24);

    let person = find_rich_text_cluster_object(json, "人", 24, 27);
    let full_stop = find_rich_text_cluster_object(json, "。", 27, 30);
    let opening = find_rich_text_cluster_object(json, "「", 30, 33);
    let river = find_rich_text_cluster_object(json, "川", 33, 36);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a variation-selector grapheme",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a variation-selector grapheme",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a variation-selector grapheme",
    );
    assert_rich_text_object_has_mask_capture(
        variant,
        &format!("{writing_mode} variation selector strict class-mix cluster"),
    );
    variant
}

fn assert_native_emoji_modifier_strict_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_emoji_modifier_strict_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_emoji_modifier_strict_class_mix_objects(&json, writing_mode);
}

fn observe_native_emoji_modifier_strict_class_mix_fixture(writing_mode: &str) -> serde_json::Value {
    let text = "天地春夏秋冬👍🏽人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-emoji-modifier-strict-class-mix"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp emoji modifier strict class-mix source");
    json
}

fn assert_native_emoji_modifier_strict_class_mix_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-emoji-modifier-strict-class-mix-{capture_kind}"
    );
    let text = "天地春夏秋冬👍🏽人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-emoji-modifier-strict-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.6.18.26")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native emoji modifier strict class-mix raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} emoji modifier strict class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native emoji modifier strict class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let emoji = assert_native_emoji_modifier_strict_class_mix_objects(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], emoji["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], emoji["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], emoji["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], emoji["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(emoji),
            content_pixels,
            &format!("{writing_mode} emoji modifier strict class-mix object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native emoji modifier strict class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp emoji modifier strict class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp emoji modifier strict class-mix dir");
}

fn assert_native_emoji_modifier_strict_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let emoji = find_rich_text_cluster_object(json, "👍🏽", 18, 26);
    assert_eq!(emoji["rich_text_ref"]["kind"], "glyph_cluster");
    assert_eq!(emoji["rich_text_ref"]["orientation"], "upright");
    assert_eq!(emoji["rich_text_ref"]["vertical_form"], "none");
    assert_eq!(emoji["rich_text_ref"]["range"]["start"], 18);
    assert_eq!(emoji["rich_text_ref"]["range"]["end"], 26);

    let person = find_rich_text_cluster_object(json, "人", 26, 29);
    let full_stop = find_rich_text_cluster_object(json, "。", 29, 32);
    let opening = find_rich_text_cluster_object(json, "「", 32, 35);
    let river = find_rich_text_cluster_object(json, "川", 35, 38);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after an emoji modifier grapheme",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after an emoji modifier grapheme",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after an emoji modifier grapheme",
    );
    assert_rich_text_object_has_mask_capture(
        emoji,
        &format!("{writing_mode} emoji modifier strict class-mix cluster"),
    );
    emoji
}

fn assert_native_regional_indicator_strict_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_regional_indicator_strict_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_regional_indicator_strict_class_mix_objects(&json, writing_mode);
}

fn observe_native_regional_indicator_strict_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let text = "天地春夏秋冬🇯🇵人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-regional-indicator-strict-class-mix"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp regional indicator strict class-mix source");
    json
}

fn assert_native_regional_indicator_strict_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-regional-indicator-strict-class-mix-{capture_kind}"
    );
    let text = "天地春夏秋冬🇯🇵人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-regional-indicator-strict-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.6.18.26")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native regional indicator strict class-mix raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} regional indicator strict class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native regional indicator strict class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let flag = assert_native_regional_indicator_strict_class_mix_objects(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], flag["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], flag["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], flag["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], flag["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(flag),
            content_pixels,
            &format!("{writing_mode} regional indicator strict class-mix object-id crop"),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native regional indicator strict class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp regional indicator strict class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp regional indicator strict class-mix dir");
}

fn assert_native_regional_indicator_strict_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let flag = find_rich_text_cluster_object(json, "🇯🇵", 18, 26);
    assert_eq!(flag["rich_text_ref"]["kind"], "glyph_cluster");
    assert_eq!(flag["rich_text_ref"]["orientation"], "upright");
    assert_eq!(flag["rich_text_ref"]["vertical_form"], "none");
    assert_eq!(flag["rich_text_ref"]["range"]["start"], 18);
    assert_eq!(flag["rich_text_ref"]["range"]["end"], 26);

    let person = find_rich_text_cluster_object(json, "人", 26, 29);
    let full_stop = find_rich_text_cluster_object(json, "。", 29, 32);
    let opening = find_rich_text_cluster_object(json, "「", 32, 35);
    let river = find_rich_text_cluster_object(json, "川", 35, 38);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a regional-indicator grapheme",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a regional-indicator grapheme",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a regional-indicator grapheme",
    );
    assert_rich_text_object_has_mask_capture(
        flag,
        &format!("{writing_mode} regional indicator strict class-mix cluster"),
    );
    flag
}

fn assert_native_keycap_strict_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_keycap_strict_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_keycap_strict_class_mix_objects(&json, writing_mode);
}

fn observe_native_keycap_strict_class_mix_fixture(writing_mode: &str) -> serde_json::Value {
    let text = "天地春夏秋冬1️⃣人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-keycap-strict-class-mix"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp keycap strict class-mix source");
    json
}

fn assert_native_keycap_strict_class_mix_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name =
        format!("agent-observe-native-{writing_mode}-keycap-strict-class-mix-{capture_kind}");
    let text = "天地春夏秋冬1️⃣人。「川」あっいおーえ―中・外………終";
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-keycap-strict-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.6.18.25")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native keycap strict class-mix raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} keycap strict class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native keycap strict class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let keycap = assert_native_keycap_strict_class_mix_objects(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], keycap["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], keycap["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], keycap["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], keycap["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(keycap),
            content_pixels,
            &format!("{writing_mode} keycap strict class-mix object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native keycap strict class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp keycap strict class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp keycap strict class-mix dir");
}

fn assert_native_keycap_strict_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let keycap = find_rich_text_cluster_object(json, "1️⃣", 18, 25);
    assert_eq!(keycap["rich_text_ref"]["kind"], "glyph_cluster");
    assert_eq!(keycap["rich_text_ref"]["orientation"], "upright");
    assert_eq!(keycap["rich_text_ref"]["vertical_form"], "none");
    assert_eq!(keycap["rich_text_ref"]["range"]["start"], 18);
    assert_eq!(keycap["rich_text_ref"]["range"]["end"], 25);

    let person = find_rich_text_cluster_object(json, "人", 25, 28);
    let full_stop = find_rich_text_cluster_object(json, "。", 28, 31);
    let opening = find_rich_text_cluster_object(json, "「", 31, 34);
    let river = find_rich_text_cluster_object(json, "川", 34, 37);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a keycap grapheme",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a keycap grapheme",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a keycap grapheme",
    );
    assert_rich_text_object_has_mask_capture(
        keycap,
        &format!("{writing_mode} keycap strict class-mix cluster"),
    );
    keycap
}

fn assert_native_published_jlreq_unit_symbol_class_mix_geometry(writing_mode: &str) {
    for case in native_published_jlreq_unit_symbol_class_mix_cases() {
        let json = observe_native_published_jlreq_unit_symbol_class_mix_fixture(writing_mode, case);
        assert_native_rich_text_layer_image_has_content(&json);
        assert_eq!(
            first_text_run_presentation_layout(&json)["jlreq_strictness"],
            "strict"
        );
        assert_eq!(
            first_text_run_presentation_layout(&json)["writing_mode"],
            writing_mode
        );
        assert_native_published_jlreq_unit_symbol_class_mix_objects(&json, writing_mode, case);
    }
}

fn observe_native_published_jlreq_unit_symbol_class_mix_fixture(
    writing_mode: &str,
    case: NativeUnitSymbolClassMixCase,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-unit-symbol-class-mix-{}",
            case.label
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{}[/][p]
}}
",
            case.text
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp published JLREQ unit symbol class-mix source");
    json
}

fn assert_native_published_jlreq_unit_symbol_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    for case in native_published_jlreq_unit_symbol_class_mix_cases() {
        assert_native_published_jlreq_unit_symbol_class_mix_case_raw_crop(
            writing_mode,
            capture_kind,
            case,
        );
    }
}

fn assert_native_published_jlreq_unit_symbol_class_mix_case_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
    case: NativeUnitSymbolClassMixCase,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-unit-symbol-class-mix-{}-{capture_kind}",
        case.label
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{}[/][p]
}}
",
            case.text
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-unit-symbol-class-mix-{}-{capture_kind}.rgba",
        case.label
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg(case.object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ unit symbol class-mix raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ unit symbol class-mix {} {capture_kind} crop should succeed, stderr: {}",
        case.label,
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ unit symbol class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let unit_end =
        assert_native_published_jlreq_unit_symbol_class_mix_objects(&json, writing_mode, case);
    assert_native_published_jlreq_unit_symbol_class_mix_crop_pixels(
        &json,
        unit_end,
        &raw_path,
        writing_mode,
        capture_kind,
        case,
    );

    fs::remove_file(&path).expect("remove temp published JLREQ unit symbol class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ unit symbol class-mix dir");
}

fn assert_native_published_jlreq_unit_symbol_class_mix_crop_pixels(
    json: &serde_json::Value,
    target: &serde_json::Value,
    raw_path: &Path,
    writing_mode: &str,
    capture_kind: &str,
    case: NativeUnitSymbolClassMixCase,
) {
    assert_eq!(json["images"][0]["crop_origin"]["x"], target["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], target["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], target["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], target["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            raw_path,
            agent_object_id_color_from_json(target),
            content_pixels,
            &format!(
                "{writing_mode} published JLREQ unit symbol class-mix {} object-id crop",
                case.label
            ),
        );
    } else {
        let bytes =
            fs::read(raw_path).expect("read native published JLREQ unit symbol class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }
}

fn assert_native_published_jlreq_unit_symbol_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    case: NativeUnitSymbolClassMixCase,
) -> &'report serde_json::Value {
    let first_unit = find_rich_text_cluster_object(json, case.first_unit, 18, case.first_unit_end);
    let second_unit = find_rich_text_cluster_object(
        json,
        case.second_unit,
        case.first_unit_end,
        case.second_unit_end,
    );
    assert_vertical_cluster_after(
        first_unit,
        second_unit,
        "published JLREQ unit symbol class mix keeps unit tail attached",
    );

    let person = find_rich_text_cluster_object(json, "人", case.second_unit_end, case.person_end);
    let full_stop = find_rich_text_cluster_object(json, "。", case.person_end, case.full_stop_end);
    let opening = find_rich_text_cluster_object(json, "「", case.full_stop_end, case.opening_end);
    let river = find_rich_text_cluster_object(json, "川", case.opening_end, case.river_end);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a unit symbol",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a unit symbol",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a unit symbol",
    );
    assert_rich_text_object_has_mask_capture(
        second_unit,
        &format!(
            "{writing_mode} published JLREQ unit symbol class-mix {}",
            case.label
        ),
    );
    second_unit
}

#[derive(Clone, Copy)]
struct NativeUnitSymbolClassMixCase {
    label: &'static str,
    text: &'static str,
    first_unit: &'static str,
    second_unit: &'static str,
    object_id: &'static str,
    first_unit_end: u64,
    second_unit_end: u64,
    person_end: u64,
    full_stop_end: u64,
    opening_end: u64,
    river_end: u64,
}

const fn native_published_jlreq_unit_symbol_class_mix_cases() -> [NativeUnitSymbolClassMixCase; 2] {
    [
        NativeUnitSymbolClassMixCase {
            label: "latin",
            text: "天地春夏秋冬kg人。「川」あっいおーえ―中・外………終",
            first_unit: "k",
            second_unit: "g",
            object_id: "object.dialogue.0.0.cluster.7.19.20",
            first_unit_end: 19,
            second_unit_end: 20,
            person_end: 23,
            full_stop_end: 26,
            opening_end: 29,
            river_end: 32,
        },
        NativeUnitSymbolClassMixCase {
            label: "greek-latin",
            text: "天地春夏秋冬μm人。「川」あっいおーえ―中・外………終",
            first_unit: "μ",
            second_unit: "m",
            object_id: "object.dialogue.0.0.cluster.7.20.21",
            first_unit_end: 20,
            second_unit_end: 21,
            person_end: 24,
            full_stop_end: 27,
            opening_end: 30,
            river_end: 33,
        },
    ]
}

fn assert_native_published_jlreq_numeric_unit_class_mix_geometry(writing_mode: &str) {
    for case in native_published_jlreq_numeric_unit_class_mix_cases() {
        let json =
            observe_native_published_jlreq_numeric_unit_class_mix_fixture(writing_mode, case);
        assert_native_rich_text_layer_image_has_content(&json);
        assert_eq!(
            first_text_run_presentation_layout(&json)["jlreq_strictness"],
            "strict"
        );
        assert_eq!(
            first_text_run_presentation_layout(&json)["writing_mode"],
            writing_mode
        );
        assert_native_published_jlreq_numeric_unit_class_mix_objects(&json, writing_mode, case);
    }
}

fn observe_native_published_jlreq_numeric_unit_class_mix_fixture(
    writing_mode: &str,
    case: NativeNumericUnitClassMixCase,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-numeric-unit-class-mix-{}",
            case.label
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{}[/][p]
}}
",
            case.text
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp published JLREQ numeric unit class-mix source");
    json
}

fn assert_native_published_jlreq_numeric_unit_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    for case in native_published_jlreq_numeric_unit_class_mix_cases() {
        assert_native_published_jlreq_numeric_unit_class_mix_case_raw_crop(
            writing_mode,
            capture_kind,
            case,
        );
    }
}

fn assert_native_published_jlreq_numeric_unit_class_mix_case_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
    case: NativeNumericUnitClassMixCase,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-numeric-unit-class-mix-{}-{capture_kind}",
        case.label
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{}[/][p]
}}
",
            case.text
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-numeric-unit-class-mix-{}-{capture_kind}.rgba",
        case.label
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg(case.object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ numeric unit class-mix raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ numeric unit class-mix {} {capture_kind} crop should succeed, stderr: {}",
        case.label,
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ numeric unit class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let unit_end =
        assert_native_published_jlreq_numeric_unit_class_mix_objects(&json, writing_mode, case);
    assert_native_published_jlreq_numeric_unit_class_mix_crop_pixels(
        &json,
        unit_end,
        &raw_path,
        writing_mode,
        capture_kind,
        case,
    );

    fs::remove_file(&path).expect("remove temp published JLREQ numeric unit class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ numeric unit class-mix dir");
}

fn assert_native_published_jlreq_numeric_unit_class_mix_crop_pixels(
    json: &serde_json::Value,
    target: &serde_json::Value,
    raw_path: &Path,
    writing_mode: &str,
    capture_kind: &str,
    case: NativeNumericUnitClassMixCase,
) {
    assert_eq!(json["images"][0]["crop_origin"]["x"], target["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], target["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], target["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], target["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            raw_path,
            agent_object_id_color_from_json(target),
            content_pixels,
            &format!(
                "{writing_mode} published JLREQ numeric unit class-mix {} object-id crop",
                case.label
            ),
        );
    } else {
        let bytes =
            fs::read(raw_path).expect("read native published JLREQ numeric unit class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }
}

fn assert_native_published_jlreq_numeric_unit_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    case: NativeNumericUnitClassMixCase,
) -> &'report serde_json::Value {
    let digit = find_rich_text_cluster_object(json, "3", 18, 19);
    let first_unit = find_rich_text_cluster_object(json, case.first_unit, 19, case.first_unit_end);
    let second_unit = find_rich_text_cluster_object(
        json,
        case.second_unit,
        case.first_unit_end,
        case.second_unit_end,
    );
    assert_vertical_cluster_after(
        digit,
        first_unit,
        "published JLREQ numeric unit class mix keeps first unit symbol with digit",
    );
    assert_vertical_cluster_after(
        first_unit,
        second_unit,
        "published JLREQ numeric unit class mix keeps unit tail attached",
    );

    let person = find_rich_text_cluster_object(json, "人", case.second_unit_end, case.person_end);
    let full_stop = find_rich_text_cluster_object(json, "。", case.person_end, case.full_stop_end);
    let opening = find_rich_text_cluster_object(json, "「", case.full_stop_end, case.opening_end);
    let river = find_rich_text_cluster_object(json, "川", case.opening_end, case.river_end);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a numeric unit",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a numeric unit",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a numeric unit",
    );
    assert_rich_text_object_has_mask_capture(
        second_unit,
        &format!(
            "{writing_mode} published JLREQ numeric unit class-mix {}",
            case.label
        ),
    );
    second_unit
}

#[derive(Clone, Copy)]
struct NativeNumericUnitClassMixCase {
    label: &'static str,
    text: &'static str,
    first_unit: &'static str,
    second_unit: &'static str,
    object_id: &'static str,
    first_unit_end: u64,
    second_unit_end: u64,
    person_end: u64,
    full_stop_end: u64,
    opening_end: u64,
    river_end: u64,
}

const fn native_published_jlreq_numeric_unit_class_mix_cases() -> [NativeNumericUnitClassMixCase; 2]
{
    [
        NativeNumericUnitClassMixCase {
            label: "latin",
            text: "天地春夏秋冬3kg人。「川」あっいおーえ―中・外………終",
            first_unit: "k",
            second_unit: "g",
            object_id: "object.dialogue.0.0.cluster.8.20.21",
            first_unit_end: 20,
            second_unit_end: 21,
            person_end: 24,
            full_stop_end: 27,
            opening_end: 30,
            river_end: 33,
        },
        NativeNumericUnitClassMixCase {
            label: "greek-latin",
            text: "天地春夏秋冬3μm人。「川」あっいおーえ―中・外………終",
            first_unit: "μ",
            second_unit: "m",
            object_id: "object.dialogue.0.0.cluster.8.21.22",
            first_unit_end: 21,
            second_unit_end: 22,
            person_end: 25,
            full_stop_end: 28,
            opening_end: 31,
            river_end: 34,
        },
    ]
}

fn assert_native_published_jlreq_numeric_separator_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_published_jlreq_numeric_separator_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_numeric_separator_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_numeric_separator_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-published-jlreq-numeric-separator-class-mix"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬1,234.56人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp published JLREQ numeric separator class-mix source");
    json
}

fn assert_native_published_jlreq_numeric_separator_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-numeric-separator-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬1,234.56人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-numeric-separator-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.9.23.24")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ numeric separator class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ numeric separator class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ numeric separator class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let decimal_point =
        assert_native_published_jlreq_numeric_separator_class_mix_objects(&json, writing_mode);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        decimal_point["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        decimal_point["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], decimal_point["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], decimal_point["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(decimal_point),
            content_pixels,
            &format!("{writing_mode} published JLREQ numeric separator class-mix object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path)
            .expect("read native published JLREQ numeric separator class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ numeric separator class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ numeric separator class-mix dir");
}

fn assert_native_published_jlreq_numeric_separator_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let first_digit = find_rich_text_cluster_object(json, "1", 18, 19);
    let comma = find_rich_text_cluster_object(json, ",", 19, 20);
    let middle_digits = find_rich_text_cluster_object(json, "234", 20, 23);
    let decimal_point = find_rich_text_cluster_object(json, ".", 23, 24);
    let final_digits = find_rich_text_cluster_object(json, "56", 24, 26);
    assert_vertical_cluster_after(
        first_digit,
        comma,
        "published JLREQ numeric separator class mix keeps comma with preceding digit",
    );
    assert_vertical_cluster_after(
        comma,
        middle_digits,
        "published JLREQ numeric separator class mix keeps digits after comma attached",
    );
    assert_vertical_cluster_after(
        middle_digits,
        decimal_point,
        "published JLREQ numeric separator class mix keeps decimal point with preceding digits",
    );
    assert_vertical_cluster_after(
        decimal_point,
        final_digits,
        "published JLREQ numeric separator class mix keeps digits after decimal point attached",
    );

    let person = find_rich_text_cluster_object(json, "人", 26, 29);
    let full_stop = find_rich_text_cluster_object(json, "。", 29, 32);
    let opening = find_rich_text_cluster_object(json, "「", 32, 35);
    let river = find_rich_text_cluster_object(json, "川", 35, 38);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a numeric separator sequence",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a numeric separator sequence",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a numeric separator sequence",
    );
    assert_rich_text_object_has_mask_capture(
        comma,
        &format!("{writing_mode} published JLREQ numeric separator class-mix comma"),
    );
    assert_rich_text_object_has_mask_capture(
        decimal_point,
        &format!("{writing_mode} published JLREQ numeric separator class-mix decimal point"),
    );
    assert_eq!(decimal_point["rich_text_ref"]["orientation"], "sideways_cw");
    decimal_point
}

fn assert_native_published_jlreq_prefixed_abbreviation_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_published_jlreq_prefixed_abbreviation_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_prefixed_abbreviation_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_prefixed_abbreviation_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-prefixed-abbreviation-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬$123人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ prefixed abbreviation class-mix source");
    json
}

fn assert_native_published_jlreq_prefixed_abbreviation_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-prefixed-abbreviation-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬$123人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-prefixed-abbreviation-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.6.18.19")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ prefixed abbreviation class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ prefixed abbreviation class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ prefixed abbreviation class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let prefix =
        assert_native_published_jlreq_prefixed_abbreviation_class_mix_objects(&json, writing_mode);
    assert_native_published_jlreq_numeric_abbreviation_crop_pixels(
        &json,
        prefix,
        &raw_path,
        writing_mode,
        "prefixed-abbreviation-class-mix",
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ prefixed abbreviation class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ prefixed abbreviation class-mix dir");
}

fn assert_native_published_jlreq_prefixed_abbreviation_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let prefix = find_rich_text_cluster_object(json, "$", 18, 19);
    let digits = find_rich_text_cluster_object(json, "123", 19, 22);
    assert_vertical_cluster_after(
        prefix,
        digits,
        "published JLREQ prefixed abbreviation class mix keeps digits with prefix",
    );

    let person = find_rich_text_cluster_object(json, "人", 22, 25);
    let full_stop = find_rich_text_cluster_object(json, "。", 25, 28);
    let opening = find_rich_text_cluster_object(json, "「", 28, 31);
    let river = find_rich_text_cluster_object(json, "川", 31, 34);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a prefixed abbreviation",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a prefixed abbreviation",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a prefixed abbreviation",
    );
    assert_rich_text_object_has_mask_capture(
        prefix,
        &format!("{writing_mode} published JLREQ prefixed abbreviation class-mix prefix"),
    );
    assert_eq!(prefix["rich_text_ref"]["orientation"], "sideways_cw");
    prefix
}

fn assert_native_published_jlreq_cent_prefixed_abbreviation_class_mix_geometry(writing_mode: &str) {
    let json =
        observe_native_published_jlreq_cent_prefixed_abbreviation_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_cent_prefixed_abbreviation_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_cent_prefixed_abbreviation_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-cent-prefixed-abbreviation-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬¢123人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ cent-prefixed abbreviation class-mix source");
    json
}

fn assert_native_published_jlreq_cent_prefixed_abbreviation_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-cent-prefixed-abbreviation-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬¢123人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-cent-prefixed-abbreviation-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.6.18.20")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ cent-prefixed abbreviation class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ cent-prefixed abbreviation class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ cent-prefixed abbreviation class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let prefix = assert_native_published_jlreq_cent_prefixed_abbreviation_class_mix_objects(
        &json,
        writing_mode,
    );
    assert_native_published_jlreq_numeric_abbreviation_crop_pixels(
        &json,
        prefix,
        &raw_path,
        writing_mode,
        "cent-prefixed-abbreviation-class-mix",
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ cent-prefixed abbreviation class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ cent-prefixed abbreviation class-mix dir");
}

fn assert_native_published_jlreq_cent_prefixed_abbreviation_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let prefix = find_rich_text_cluster_object(json, "¢", 18, 20);
    let digits = find_rich_text_cluster_object(json, "123", 20, 23);
    assert_vertical_cluster_after(
        prefix,
        digits,
        "published JLREQ cent-prefixed abbreviation class mix keeps digits with prefix",
    );

    let person = find_rich_text_cluster_object(json, "人", 23, 26);
    let full_stop = find_rich_text_cluster_object(json, "。", 26, 29);
    let opening = find_rich_text_cluster_object(json, "「", 29, 32);
    let river = find_rich_text_cluster_object(json, "川", 32, 35);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a cent-prefixed abbreviation",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a cent-prefixed abbreviation",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a cent-prefixed abbreviation",
    );
    assert_rich_text_object_has_mask_capture(
        prefix,
        &format!("{writing_mode} published JLREQ cent-prefixed abbreviation class-mix prefix"),
    );
    assert_eq!(prefix["rich_text_ref"]["orientation"], "sideways_cw");
    prefix
}

fn assert_native_published_jlreq_yen_prefixed_abbreviation_class_mix_geometry(writing_mode: &str) {
    for label in ["yen-prefix", "fullwidth-yen-prefix"] {
        let json = observe_native_published_jlreq_yen_prefixed_abbreviation_class_mix_fixture(
            writing_mode,
            label,
        );
        assert_native_rich_text_layer_image_has_content(&json);
        assert_eq!(
            first_text_run_presentation_layout(&json)["jlreq_strictness"],
            "strict"
        );
        assert_eq!(
            first_text_run_presentation_layout(&json)["writing_mode"],
            writing_mode
        );
        assert_native_published_jlreq_yen_prefixed_abbreviation_class_mix_objects(
            &json,
            writing_mode,
            label,
        );
    }
}

fn observe_native_published_jlreq_yen_prefixed_abbreviation_class_mix_fixture(
    writing_mode: &str,
    label: &str,
) -> serde_json::Value {
    let text = native_published_jlreq_yen_prefixed_abbreviation_class_mix_text(label);
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-{label}-abbreviation-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ yen-prefixed abbreviation class-mix source");
    json
}

fn assert_native_published_jlreq_yen_prefixed_abbreviation_class_mix_raw_crop(
    writing_mode: &str,
    label: &str,
    capture_kind: &str,
) {
    let text = native_published_jlreq_yen_prefixed_abbreviation_class_mix_text(label);
    let object_id = native_published_jlreq_yen_prefixed_abbreviation_class_mix_object_id(label);
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-{label}-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-{label}-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg(object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ yen-prefixed abbreviation class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ {label} abbreviation class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ yen-prefixed abbreviation class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let prefix = assert_native_published_jlreq_yen_prefixed_abbreviation_class_mix_objects(
        &json,
        writing_mode,
        label,
    );
    assert_native_published_jlreq_numeric_abbreviation_crop_pixels(
        &json,
        prefix,
        &raw_path,
        writing_mode,
        label,
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ yen-prefixed abbreviation class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ yen-prefixed abbreviation class-mix dir");
}

fn native_published_jlreq_yen_prefixed_abbreviation_class_mix_text(label: &str) -> &'static str {
    match label {
        "yen-prefix" => "天地春夏秋冬¥123人。「川」あっいおーえ―中・外………終",
        "fullwidth-yen-prefix" => "天地春夏秋冬￥123人。「川」あっいおーえ―中・外………終",
        _ => panic!("unknown native published JLREQ yen-prefixed abbreviation label {label}"),
    }
}

fn native_published_jlreq_yen_prefixed_abbreviation_class_mix_object_id(
    label: &str,
) -> &'static str {
    match label {
        "yen-prefix" => "object.dialogue.0.0.cluster.6.18.20",
        "fullwidth-yen-prefix" => "object.dialogue.0.0.cluster.6.18.21",
        _ => panic!("unknown native published JLREQ yen-prefixed abbreviation label {label}"),
    }
}

fn assert_native_published_jlreq_yen_prefixed_abbreviation_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    label: &str,
) -> &'report serde_json::Value {
    let (prefix_text, prefix_end, digits_end, person_end, full_stop_end, opening_end, river_end) =
        if label == "yen-prefix" {
            ("¥", 20, 23, 26, 29, 32, 35)
        } else {
            ("￥", 21, 24, 27, 30, 33, 36)
        };
    let prefix = find_rich_text_cluster_object(json, prefix_text, 18, prefix_end);
    let digits = find_rich_text_cluster_object(json, "123", prefix_end, digits_end);
    assert_vertical_cluster_after(
        prefix,
        digits,
        "published JLREQ yen-prefixed abbreviation class mix keeps digits with prefix",
    );

    let person = find_rich_text_cluster_object(json, "人", digits_end, person_end);
    let full_stop = find_rich_text_cluster_object(json, "。", person_end, full_stop_end);
    let opening = find_rich_text_cluster_object(json, "「", full_stop_end, opening_end);
    let river = find_rich_text_cluster_object(json, "川", opening_end, river_end);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a yen-prefixed abbreviation",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a yen-prefixed abbreviation",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a yen-prefixed abbreviation",
    );
    assert_rich_text_object_has_mask_capture(
        prefix,
        &format!("{writing_mode} published JLREQ {label} class-mix prefix"),
    );
    prefix
}

fn assert_native_published_jlreq_postfixed_abbreviation_class_mix_geometry(writing_mode: &str) {
    let json =
        observe_native_published_jlreq_postfixed_abbreviation_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_postfixed_abbreviation_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_postfixed_abbreviation_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-postfixed-abbreviation-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬50%人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ postfixed abbreviation class-mix source");
    json
}

fn assert_native_published_jlreq_postfixed_abbreviation_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-postfixed-abbreviation-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬50%人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-postfixed-abbreviation-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.7.20.21")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ postfixed abbreviation class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ postfixed abbreviation class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ postfixed abbreviation class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let suffix =
        assert_native_published_jlreq_postfixed_abbreviation_class_mix_objects(&json, writing_mode);
    assert_native_published_jlreq_numeric_abbreviation_crop_pixels(
        &json,
        suffix,
        &raw_path,
        writing_mode,
        "postfixed-abbreviation-class-mix",
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ postfixed abbreviation class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ postfixed abbreviation class-mix dir");
}

fn assert_native_published_jlreq_postfixed_abbreviation_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let digits = find_rich_text_cluster_object(json, "50", 18, 20);
    let suffix = find_rich_text_cluster_object(json, "%", 20, 21);
    assert_vertical_cluster_after(
        digits,
        suffix,
        "published JLREQ postfixed abbreviation class mix keeps suffix with preceding digits",
    );

    let person = find_rich_text_cluster_object(json, "人", 21, 24);
    let full_stop = find_rich_text_cluster_object(json, "。", 24, 27);
    let opening = find_rich_text_cluster_object(json, "「", 27, 30);
    let river = find_rich_text_cluster_object(json, "川", 30, 33);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after a postfixed abbreviation",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a postfixed abbreviation",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a postfixed abbreviation",
    );
    assert_rich_text_object_has_mask_capture(
        suffix,
        &format!("{writing_mode} published JLREQ postfixed abbreviation class-mix suffix"),
    );
    assert_eq!(suffix["rich_text_ref"]["orientation"], "sideways_cw");
    suffix
}

fn assert_native_published_jlreq_ideographic_abbreviation_class_mix_geometry(writing_mode: &str) {
    for label in ["prefix-ideographic", "suffix-ideographic"] {
        let json = observe_native_published_jlreq_ideographic_abbreviation_class_mix_fixture(
            writing_mode,
            label,
        );
        assert_native_rich_text_layer_image_has_content(&json);
        assert_eq!(
            first_text_run_presentation_layout(&json)["jlreq_strictness"],
            "strict"
        );
        assert_eq!(
            first_text_run_presentation_layout(&json)["writing_mode"],
            writing_mode
        );
        assert_native_published_jlreq_ideographic_abbreviation_class_mix_objects(
            &json,
            writing_mode,
            label,
        );
    }
}

fn observe_native_published_jlreq_ideographic_abbreviation_class_mix_fixture(
    writing_mode: &str,
    label: &str,
) -> serde_json::Value {
    let text = native_published_jlreq_ideographic_abbreviation_class_mix_text(label);
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-{label}-abbreviation-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ ideographic abbreviation class-mix source");
    json
}

fn assert_native_published_jlreq_ideographic_abbreviation_class_mix_raw_crop(
    writing_mode: &str,
    label: &str,
    capture_kind: &str,
) {
    let text = native_published_jlreq_ideographic_abbreviation_class_mix_text(label);
    let object_id = native_published_jlreq_ideographic_abbreviation_class_mix_object_id(label);
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-{label}-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-{label}-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg(object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ ideographic abbreviation class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ {label} abbreviation class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ ideographic abbreviation class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let target = assert_native_published_jlreq_ideographic_abbreviation_class_mix_objects(
        &json,
        writing_mode,
        label,
    );
    assert_native_published_jlreq_numeric_abbreviation_crop_pixels(
        &json,
        target,
        &raw_path,
        writing_mode,
        label,
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ ideographic abbreviation class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ ideographic abbreviation class-mix dir");
}

fn native_published_jlreq_ideographic_abbreviation_class_mix_text(label: &str) -> &'static str {
    match label {
        "prefix-ideographic" => "天地春夏秋冬$五人。「川」あっいおーえ―中・外………終",
        "suffix-ideographic" => "天地春夏秋冬五%人。「川」あっいおーえ―中・外………終",
        _ => panic!("unknown native published JLREQ ideographic abbreviation label {label}"),
    }
}

fn native_published_jlreq_ideographic_abbreviation_class_mix_object_id(
    label: &str,
) -> &'static str {
    match label {
        "prefix-ideographic" => "object.dialogue.0.0.cluster.6.18.19",
        "suffix-ideographic" => "object.dialogue.0.0.cluster.7.21.22",
        _ => panic!("unknown native published JLREQ ideographic abbreviation label {label}"),
    }
}

fn assert_native_published_jlreq_ideographic_abbreviation_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    label: &str,
) -> &'report serde_json::Value {
    let (leading, trailing, target) = if label == "prefix-ideographic" {
        let prefix = find_rich_text_cluster_object(json, "$", 18, 19);
        let ideographic_numeral = find_rich_text_cluster_object(json, "五", 19, 22);
        (prefix, ideographic_numeral, prefix)
    } else {
        let ideographic_numeral = find_rich_text_cluster_object(json, "五", 18, 21);
        let suffix = find_rich_text_cluster_object(json, "%", 21, 22);
        (ideographic_numeral, suffix, suffix)
    };
    assert_vertical_cluster_after(
        leading,
        trailing,
        "published JLREQ ideographic abbreviation class mix keeps mark and ideographic numeral attached",
    );

    let person = find_rich_text_cluster_object(json, "人", 22, 25);
    let full_stop = find_rich_text_cluster_object(json, "。", 25, 28);
    let opening = find_rich_text_cluster_object(json, "「", 28, 31);
    let river = find_rich_text_cluster_object(json, "川", 31, 34);
    assert_vertical_cluster_after(
        person,
        full_stop,
        "strict paragraph class mix still keeps closing punctuation after an ideographic abbreviation",
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after an ideographic abbreviation",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after an ideographic abbreviation",
    );
    assert_rich_text_object_has_mask_capture(
        target,
        &format!("{writing_mode} published JLREQ {label} class-mix target"),
    );
    target
}

fn assert_native_published_jlreq_reference_mark_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_published_jlreq_reference_mark_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_reference_mark_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_reference_mark_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-published-jlreq-reference-mark-class-mix"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬本¹²。人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp published JLREQ reference mark class-mix source");
    json
}

fn assert_native_published_jlreq_reference_mark_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-reference-mark-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬本¹²。人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-reference-mark-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.9.25.28")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ reference mark class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ reference mark class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ reference mark class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let reference_full_stop =
        assert_native_published_jlreq_reference_mark_class_mix_objects(&json, writing_mode);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        reference_full_stop["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        reference_full_stop["bbox"]["y"]
    );
    assert_eq!(
        json["images"][0]["width"],
        reference_full_stop["bbox"]["width"]
    );
    assert_eq!(
        json["images"][0]["height"],
        reference_full_stop["bbox"]["height"]
    );

    assert_native_published_jlreq_reference_mark_class_mix_raw_image(
        &json,
        &raw_path,
        reference_full_stop,
        writing_mode,
        capture_kind,
    );

    fs::remove_file(&path).expect("remove temp published JLREQ reference mark class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ reference mark class-mix dir");
}

fn assert_native_published_jlreq_reference_mark_class_mix_raw_image(
    json: &serde_json::Value,
    raw_path: &Path,
    reference_full_stop: &serde_json::Value,
    writing_mode: &str,
    capture_kind: &str,
) {
    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            raw_path,
            agent_object_id_color_from_json(reference_full_stop),
            content_pixels,
            &format!("{writing_mode} published JLREQ reference mark class-mix object-id crop"),
        );
    } else {
        let bytes =
            fs::read(raw_path).expect("read native published JLREQ reference mark class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }
}

fn assert_native_published_jlreq_reference_mark_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let body = find_rich_text_cluster_object(json, "本", 18, 21);
    let first_mark = find_rich_text_cluster_object(json, "¹", 21, 23);
    let second_mark = find_rich_text_cluster_object(json, "²", 23, 25);
    let reference_full_stop = find_rich_text_cluster_object(json, "。", 25, 28);
    assert_vertical_cluster_after(
        body,
        first_mark,
        "published JLREQ reference mark class mix keeps mark with preceding main text",
    );
    assert_vertical_cluster_after(
        first_mark,
        second_mark,
        "published JLREQ reference mark class mix keeps reference digits together",
    );
    assert_eq!(
        agent_json_bbox_x(&second_mark["bbox"]),
        agent_json_bbox_x(&reference_full_stop["bbox"]),
        "published JLREQ reference mark class mix keeps following full stop in the reference column"
    );
    assert!(
        agent_json_bbox_bottom(&reference_full_stop["bbox"])
            > agent_json_bbox_y(&second_mark["bbox"]),
        "published JLREQ reference mark class mix keeps following full stop attached"
    );

    let person = find_rich_text_cluster_object(json, "人", 28, 31);
    let strict_full_stop = find_rich_text_cluster_object(json, "。", 31, 34);
    let opening = find_rich_text_cluster_object(json, "「", 34, 37);
    let river = find_rich_text_cluster_object(json, "川", 37, 40);
    assert_vertical_cluster_after(
        person,
        strict_full_stop,
        "strict paragraph class mix still keeps closing punctuation after a reference mark sequence",
    );
    assert_vertical_cluster_after(
        strict_full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a reference mark sequence",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a reference mark sequence",
    );
    assert_rich_text_object_has_mask_capture(
        second_mark,
        &format!("{writing_mode} published JLREQ reference mark class-mix second mark"),
    );
    assert_rich_text_object_has_mask_capture(
        reference_full_stop,
        &format!("{writing_mode} published JLREQ reference mark class-mix full stop"),
    );
    reference_full_stop
}

fn assert_native_published_jlreq_parenthesized_reference_mark_class_mix_geometry(
    writing_mode: &str,
) {
    let json =
        observe_native_published_jlreq_parenthesized_reference_mark_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_parenthesized_reference_mark_class_mix_objects(
        &json,
        writing_mode,
    );
}

fn observe_native_published_jlreq_parenthesized_reference_mark_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-parenthesized-reference-mark-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬本⁽¹⁾。人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ parenthesized reference mark class-mix source");
    json
}

fn assert_native_published_jlreq_parenthesized_reference_mark_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-parenthesized-reference-mark-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬本⁽¹⁾。人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-parenthesized-reference-mark-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.10.29.32")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ parenthesized reference mark class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ parenthesized reference mark class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ parenthesized reference mark class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let reference_full_stop =
        assert_native_published_jlreq_parenthesized_reference_mark_class_mix_objects(
            &json,
            writing_mode,
        );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        reference_full_stop["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        reference_full_stop["bbox"]["y"]
    );
    assert_eq!(
        json["images"][0]["width"],
        reference_full_stop["bbox"]["width"]
    );
    assert_eq!(
        json["images"][0]["height"],
        reference_full_stop["bbox"]["height"]
    );

    assert_native_published_jlreq_reference_mark_class_mix_raw_image(
        &json,
        &raw_path,
        reference_full_stop,
        writing_mode,
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ parenthesized reference mark class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ parenthesized reference mark class-mix dir");
}

fn assert_native_published_jlreq_parenthesized_reference_mark_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let body = find_rich_text_cluster_object(json, "本", 18, 21);
    let open = find_rich_text_cluster_object(json, "⁽", 21, 24);
    let mark = find_rich_text_cluster_object(json, "¹", 24, 26);
    let close = find_rich_text_cluster_object(json, "⁾", 26, 29);
    let reference_full_stop = find_rich_text_cluster_object(json, "。", 29, 32);
    assert_vertical_cluster_after(
        body,
        open,
        "published JLREQ parenthesized reference mark class mix keeps opening with main text",
    );
    assert_vertical_cluster_after(
        open,
        mark,
        "published JLREQ parenthesized reference mark class mix keeps digit with opening",
    );
    assert_vertical_cluster_after(
        mark,
        close,
        "published JLREQ parenthesized reference mark class mix keeps closing with digit",
    );
    assert_eq!(
        agent_json_bbox_x(&close["bbox"]),
        agent_json_bbox_x(&reference_full_stop["bbox"]),
        "published JLREQ parenthesized reference mark class mix keeps following full stop in the reference column"
    );
    assert!(
        agent_json_bboxes_intersect(&close["bbox"], &reference_full_stop["bbox"]),
        "published JLREQ parenthesized reference mark class mix keeps following full stop attached"
    );

    let person = find_rich_text_cluster_object(json, "人", 32, 35);
    let strict_full_stop = find_rich_text_cluster_object(json, "。", 35, 38);
    let opening = find_rich_text_cluster_object(json, "「", 38, 41);
    let river = find_rich_text_cluster_object(json, "川", 41, 44);
    assert_vertical_cluster_after(
        person,
        strict_full_stop,
        "strict paragraph class mix still keeps closing punctuation after a parenthesized reference mark sequence",
    );
    assert_vertical_cluster_after(
        strict_full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a parenthesized reference mark sequence",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a parenthesized reference mark sequence",
    );
    assert_rich_text_object_has_mask_capture(
        close,
        &format!("{writing_mode} published JLREQ parenthesized reference mark class-mix closing"),
    );
    assert_rich_text_object_has_mask_capture(
        reference_full_stop,
        &format!("{writing_mode} published JLREQ parenthesized reference mark class-mix full stop"),
    );
    reference_full_stop
}

fn assert_native_published_jlreq_temperature_suffix_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_published_jlreq_temperature_suffix_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_temperature_suffix_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_temperature_suffix_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-temperature-suffix-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬25℃人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ temperature suffix class-mix source");
    json
}

fn assert_native_published_jlreq_temperature_suffix_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-temperature-suffix-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬25℃人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-temperature-suffix-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.7.20.23")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ temperature suffix class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ temperature suffix class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ temperature suffix class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let suffix =
        assert_native_published_jlreq_temperature_suffix_class_mix_objects(&json, writing_mode);
    assert_native_published_jlreq_numeric_abbreviation_crop_pixels(
        &json,
        suffix,
        &raw_path,
        writing_mode,
        "temperature-suffix-class-mix",
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ temperature suffix class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ temperature suffix class-mix dir");
}

fn assert_native_published_jlreq_temperature_suffix_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let digits = find_rich_text_cluster_object(json, "25", 18, 20);
    let suffix = find_rich_text_cluster_object(json, "℃", 20, 23);
    assert_vertical_cluster_after(
        digits,
        suffix,
        "published JLREQ temperature suffix class mix keeps suffix with preceding digits",
    );

    let person = find_rich_text_cluster_object(json, "人", 23, 26);
    let strict_full_stop = find_rich_text_cluster_object(json, "。", 26, 29);
    let opening = find_rich_text_cluster_object(json, "「", 29, 32);
    let river = find_rich_text_cluster_object(json, "川", 32, 35);
    assert_vertical_cluster_after(
        person,
        strict_full_stop,
        "strict paragraph class mix still keeps closing punctuation after a temperature suffix",
    );
    assert_vertical_cluster_after(
        strict_full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a temperature suffix",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a temperature suffix",
    );
    assert_rich_text_object_has_mask_capture(
        suffix,
        &format!("{writing_mode} published JLREQ temperature suffix class-mix suffix"),
    );
    suffix
}

fn assert_native_published_jlreq_decomposed_temperature_class_mix_geometry(writing_mode: &str) {
    let json =
        observe_native_published_jlreq_decomposed_temperature_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_decomposed_temperature_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_decomposed_temperature_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-decomposed-temperature-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬25°C人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ decomposed temperature class-mix source");
    json
}

fn assert_native_published_jlreq_decomposed_temperature_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-decomposed-temperature-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬25°C人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-decomposed-temperature-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.8.22.23")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ decomposed temperature class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ decomposed temperature class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ decomposed temperature class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let unit =
        assert_native_published_jlreq_decomposed_temperature_class_mix_objects(&json, writing_mode);
    assert_native_published_jlreq_numeric_abbreviation_crop_pixels(
        &json,
        unit,
        &raw_path,
        writing_mode,
        "decomposed-temperature-class-mix",
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ decomposed temperature class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ decomposed temperature class-mix dir");
}

fn assert_native_published_jlreq_decomposed_temperature_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let digits = find_rich_text_cluster_object(json, "25", 18, 20);
    let degree = find_rich_text_cluster_object(json, "°", 20, 22);
    let unit = find_rich_text_cluster_object(json, "C", 22, 23);
    assert_vertical_cluster_after(
        digits,
        degree,
        "published JLREQ decomposed temperature class mix keeps degree with preceding digits",
    );
    assert_vertical_cluster_after(
        degree,
        unit,
        "published JLREQ decomposed temperature class mix keeps unit tail with degree",
    );

    let person = find_rich_text_cluster_object(json, "人", 23, 26);
    let strict_full_stop = find_rich_text_cluster_object(json, "。", 26, 29);
    let opening = find_rich_text_cluster_object(json, "「", 29, 32);
    let river = find_rich_text_cluster_object(json, "川", 32, 35);
    assert_vertical_cluster_after(
        person,
        strict_full_stop,
        "strict paragraph class mix still keeps closing punctuation after a decomposed temperature unit",
    );
    assert_vertical_cluster_after(
        strict_full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a decomposed temperature unit",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a decomposed temperature unit",
    );
    assert_rich_text_object_has_mask_capture(
        degree,
        &format!("{writing_mode} published JLREQ decomposed temperature class-mix degree"),
    );
    assert_rich_text_object_has_mask_capture(
        unit,
        &format!("{writing_mode} published JLREQ decomposed temperature class-mix unit tail"),
    );
    unit
}

fn assert_native_published_jlreq_subscript_object_class_mix_geometry(writing_mode: &str) {
    let json = observe_native_published_jlreq_subscript_object_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_subscript_object_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_subscript_object_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-published-jlreq-subscript-object-class-mix"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬H₂O人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp published JLREQ subscript object class-mix source");
    json
}

fn assert_native_published_jlreq_subscript_object_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-subscript-object-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬H₂O人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-subscript-object-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.8.22.23")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ subscript object class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ subscript object class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ subscript object class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let following_base =
        assert_native_published_jlreq_subscript_object_class_mix_objects(&json, writing_mode);
    assert_native_published_jlreq_subscript_object_class_mix_crop_pixels(
        &json,
        following_base,
        &raw_path,
        writing_mode,
        capture_kind,
    );

    fs::remove_file(&path).expect("remove temp published JLREQ subscript object class-mix source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ subscript object class-mix dir");
}

fn assert_native_published_jlreq_subscript_object_class_mix_crop_pixels(
    json: &serde_json::Value,
    following_base: &serde_json::Value,
    raw_path: &Path,
    writing_mode: &str,
    capture_kind: &str,
) {
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        following_base["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        following_base["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], following_base["bbox"]["width"]);
    assert_eq!(
        json["images"][0]["height"],
        following_base["bbox"]["height"]
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            raw_path,
            agent_object_id_color_from_json(following_base),
            content_pixels,
            &format!("{writing_mode} published JLREQ subscript object class-mix object-id crop"),
        );
    } else {
        let bytes = fs::read(raw_path)
            .expect("read native published JLREQ subscript object class-mix crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }
}

fn assert_native_published_jlreq_subscript_object_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let base = find_rich_text_cluster_object(json, "H", 18, 19);
    let mark = find_rich_text_cluster_object(json, "₂", 19, 22);
    let following_base = find_rich_text_cluster_object(json, "O", 22, 23);
    assert_vertical_cluster_after(
        base,
        mark,
        "published JLREQ subscript object class mix keeps mark with preceding base",
    );
    assert_vertical_cluster_after(
        mark,
        following_base,
        "published JLREQ subscript object class mix keeps following base attached",
    );

    let person = find_rich_text_cluster_object(json, "人", 23, 26);
    let strict_full_stop = find_rich_text_cluster_object(json, "。", 26, 29);
    let opening = find_rich_text_cluster_object(json, "「", 29, 32);
    let river = find_rich_text_cluster_object(json, "川", 32, 35);
    assert_vertical_cluster_after(
        person,
        strict_full_stop,
        "strict paragraph class mix still keeps closing punctuation after a subscript object",
    );
    assert_vertical_cluster_after(
        strict_full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a subscript object",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a subscript object",
    );
    assert_rich_text_object_has_mask_capture(
        following_base,
        &format!("{writing_mode} published JLREQ subscript object class-mix following base"),
    );
    following_base
}

fn assert_native_published_jlreq_greek_subscript_object_class_mix_geometry(writing_mode: &str) {
    let json =
        observe_native_published_jlreq_greek_subscript_object_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_greek_subscript_object_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_greek_subscript_object_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-greek-subscript-object-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬α₂β人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ Greek subscript object class-mix source");
    json
}

fn assert_native_published_jlreq_greek_subscript_object_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-greek-subscript-object-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬α₂β人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-greek-subscript-object-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.8.23.25")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ Greek subscript object class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ Greek subscript object class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ Greek subscript object class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let following_base =
        assert_native_published_jlreq_greek_subscript_object_class_mix_objects(&json, writing_mode);
    assert_native_published_jlreq_subscript_object_class_mix_crop_pixels(
        &json,
        following_base,
        &raw_path,
        writing_mode,
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ Greek subscript object class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ Greek subscript object class-mix dir");
}

fn assert_native_published_jlreq_greek_subscript_object_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let base = find_rich_text_cluster_object(json, "α", 18, 20);
    let mark = find_rich_text_cluster_object(json, "₂", 20, 23);
    let following_base = find_rich_text_cluster_object(json, "β", 23, 25);
    assert_vertical_cluster_after(
        base,
        mark,
        "published JLREQ Greek subscript object class mix keeps mark with preceding base",
    );
    assert_vertical_cluster_after(
        mark,
        following_base,
        "published JLREQ Greek subscript object class mix keeps following base attached",
    );

    let person = find_rich_text_cluster_object(json, "人", 25, 28);
    let strict_full_stop = find_rich_text_cluster_object(json, "。", 28, 31);
    let opening = find_rich_text_cluster_object(json, "「", 31, 34);
    let river = find_rich_text_cluster_object(json, "川", 34, 37);
    assert_vertical_cluster_after(
        person,
        strict_full_stop,
        "strict paragraph class mix still keeps closing punctuation after a Greek subscript object",
    );
    assert_vertical_cluster_after(
        strict_full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a Greek subscript object",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a Greek subscript object",
    );
    assert_rich_text_object_has_mask_capture(
        following_base,
        &format!("{writing_mode} published JLREQ Greek subscript object class-mix following base"),
    );
    following_base
}

fn assert_native_published_jlreq_greek_superscript_object_class_mix_geometry(writing_mode: &str) {
    let json =
        observe_native_published_jlreq_greek_superscript_object_class_mix_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_greek_superscript_object_class_mix_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_greek_superscript_object_class_mix_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-greek-superscript-object-class-mix"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬α²β人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path)
        .expect("remove temp published JLREQ Greek superscript object class-mix source");
    json
}

fn assert_native_published_jlreq_greek_superscript_object_class_mix_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-greek-superscript-object-class-mix-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬α²β人。「川」あっいおーえ―中・外………終[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-greek-superscript-object-class-mix-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--viewport-width")
        .arg("1280")
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.8.22.24")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ Greek superscript object class-mix raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ Greek superscript object class-mix {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ Greek superscript object class-mix report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let following_base = assert_native_published_jlreq_greek_superscript_object_class_mix_objects(
        &json,
        writing_mode,
    );
    assert_native_published_jlreq_subscript_object_class_mix_crop_pixels(
        &json,
        following_base,
        &raw_path,
        writing_mode,
        capture_kind,
    );

    fs::remove_file(&path)
        .expect("remove temp published JLREQ Greek superscript object class-mix source");
    fs::remove_dir_all(&dir)
        .expect("remove temp published JLREQ Greek superscript object class-mix dir");
}

fn assert_native_published_jlreq_greek_superscript_object_class_mix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let base = find_rich_text_cluster_object(json, "α", 18, 20);
    let mark = find_rich_text_cluster_object(json, "²", 20, 22);
    let following_base = find_rich_text_cluster_object(json, "β", 22, 24);
    assert_vertical_cluster_after(
        base,
        mark,
        "published JLREQ Greek superscript object class mix keeps mark with preceding base",
    );
    assert_vertical_cluster_after(
        mark,
        following_base,
        "published JLREQ Greek superscript object class mix keeps following base attached",
    );

    let person = find_rich_text_cluster_object(json, "人", 24, 27);
    let strict_full_stop = find_rich_text_cluster_object(json, "。", 27, 30);
    let opening = find_rich_text_cluster_object(json, "「", 30, 33);
    let river = find_rich_text_cluster_object(json, "川", 33, 36);
    assert_vertical_cluster_after(
        person,
        strict_full_stop,
        "strict paragraph class mix still keeps closing punctuation after a Greek superscript object",
    );
    assert_vertical_cluster_after(
        strict_full_stop,
        opening,
        "strict paragraph class mix still keeps adjacent closing/opening punctuation after a Greek superscript object",
    );
    assert_vertical_cluster_after(
        opening,
        river,
        "strict paragraph class mix still keeps opening punctuation with its base after a Greek superscript object",
    );
    assert_rich_text_object_has_mask_capture(
        following_base,
        &format!(
            "{writing_mode} published JLREQ Greek superscript object class-mix following base"
        ),
    );
    following_base
}

