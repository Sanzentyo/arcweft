use super::*;

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_european_numeral_sequence_unbroken() {
    // W3C JLREQ 3.1.10 treats European numeral sequences as unbreakable
    // because each digit position contributes to the represented value.
    // Arcweft's vertical text-combine policy splits long numeral runs into
    // multiple layout clusters, so the column planner must still treat the
    // whole numeral sequence as one no-break suffix.
    let text = "天202650267人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 90.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let body = nth_laid_out_glyph(&layout, "天", 0);
        let first_digits = nth_laid_out_glyph(&layout, "2026", 0);
        let second_digits = nth_laid_out_glyph(&layout, "5026", 0);
        let final_digit = nth_laid_out_glyph(&layout, "7", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_eq!(
            first_digits.orientation,
            GlyphOrientation::TextCombineUpright
        );
        assert_eq!(
            second_digits.orientation,
            GlyphOrientation::TextCombineUpright
        );
        assert_eq!(final_digit.orientation, GlyphOrientation::SidewaysCw);
        if next_column_moves_right {
            assert!(
                first_digits.origin.x > body.origin.x,
                "overlong European numeral sequence should move right as a unit after body text: {first_digits:?}"
            );
        } else {
            assert!(
                first_digits.origin.x < body.origin.x,
                "overlong European numeral sequence should move left as a unit after body text: {first_digits:?}"
            );
        }
        assert_f32_eq(first_digits.origin.y, config.origin.y);
        assert_vertical_layout_after(
            first_digits,
            second_digits,
            "text-combine chunk should stay with the previous numeral chunk",
        );
        assert_vertical_layout_after(
            second_digits,
            final_digit,
            "remaining digit should stay with the text-combine numeral chunks",
        );
        assert_f32_eq(
            final_digit.bounds.bottom(),
            config.origin.y + config.size.height,
        );
        assert_next_vertical_layout_column(
            final_digit,
            next_body,
            next_column_moves_right,
            "body text after an overhanging European numeral sequence should continue in the next column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_numeric_separators_unbroken() {
    // W3C JLREQ 3.1.10 also keeps decimal points and place separators
    // inside European numerals unbreakable on both sides.
    let text = "天1,234.56人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 126.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let body = nth_laid_out_glyph(&layout, "天", 0);
        let first_digit = nth_laid_out_glyph(&layout, "1", 0);
        let comma = nth_laid_out_glyph(&layout, ",", 0);
        let middle_digits = nth_laid_out_glyph(&layout, "234", 0);
        let decimal_point = nth_laid_out_glyph(&layout, ".", 0);
        let final_digits = nth_laid_out_glyph(&layout, "56", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        if next_column_moves_right {
            assert!(
                first_digit.origin.x > body.origin.x,
                "European numeral with separators should move right as a unit after body text: {first_digit:?}"
            );
        } else {
            assert!(
                first_digit.origin.x < body.origin.x,
                "European numeral with separators should move left as a unit after body text: {first_digit:?}"
            );
        }
        assert_f32_eq(first_digit.origin.y, config.origin.y);
        assert_vertical_layout_after(
            first_digit,
            comma,
            "comma place separator should stay with the preceding digit",
        );
        assert_vertical_layout_after(
            comma,
            middle_digits,
            "digits after comma place separator should stay attached",
        );
        assert_vertical_layout_after(
            middle_digits,
            decimal_point,
            "decimal point should stay with the preceding digit chunk",
        );
        assert_vertical_layout_after(
            decimal_point,
            final_digits,
            "digits after decimal point should stay attached",
        );
        assert_next_vertical_layout_column(
            final_digits,
            next_body,
            next_column_moves_right,
            "body text after an overhanging European numeral with separators should continue in the next column",
        );
    }

    let text = "天12 345人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 126.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let leading_digits = nth_laid_out_glyph(&layout, "12", 0);
        let space = nth_laid_out_glyph(&layout, " ", 0);
        let trailing_digits = nth_laid_out_glyph(&layout, "345", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            leading_digits,
            space,
            "space place separator should stay with the preceding digit chunk",
        );
        assert_vertical_layout_after(
            space,
            trailing_digits,
            "digits after space place separator should stay attached",
        );
        assert_next_vertical_layout_column(
            trailing_digits,
            next_body,
            next_column_moves_right,
            "body text after a European numeral with a space separator should continue in the next column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_numeric_abbreviations_unbroken() {
    // W3C JLREQ 3.1.10 keeps prefixed abbreviations such as "$" with the
    // following numeral, and postfixed abbreviations such as "%" with the
    // preceding numeral.
    let text = "天$1234人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let prefix = nth_laid_out_glyph(&layout, "$", 0);
        let digits = nth_laid_out_glyph(&layout, "1234", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            prefix,
            digits,
            "digits after numeric prefix abbreviation should stay attached",
        );
        assert_next_vertical_layout_column(
            digits,
            next_body,
            next_column_moves_right,
            "body text after a prefixed European numeral should continue in the next column",
        );
    }

    let text = "天¢1234人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let prefix = nth_laid_out_glyph(&layout, "¢", 0);
        let digits = nth_laid_out_glyph(&layout, "1234", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            prefix,
            digits,
            "digits after cent sign prefix abbreviation should stay attached",
        );
        assert_next_vertical_layout_column(
            digits,
            next_body,
            next_column_moves_right,
            "body text after a cent-prefixed European numeral should continue in the next column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_postfixed_abbreviations_unbroken() {
    // W3C JLREQ 3.1.10 keeps postfixed abbreviations such as "%" and
    // temperature units with the preceding numeral.
    let text = "天50%人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let digits = nth_laid_out_glyph(&layout, "50", 0);
        let suffix = nth_laid_out_glyph(&layout, "%", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            digits,
            suffix,
            "numeric suffix abbreviation should stay with the preceding digits",
        );
        assert_next_vertical_layout_column(
            suffix,
            next_body,
            next_column_moves_right,
            "body text after a postfixed European numeral should continue in the next column",
        );
    }

    let text = "天25℃人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let digits = nth_laid_out_glyph(&layout, "25", 0);
        let suffix = nth_laid_out_glyph(&layout, "℃", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            digits,
            suffix,
            "temperature suffix abbreviation should stay with the preceding digits",
        );
        assert_next_vertical_layout_column(
            suffix,
            next_body,
            next_column_moves_right,
            "body text after a temperature-suffixed European numeral should continue in the next column",
        );
    }

    let text = "天25°C人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let digits = nth_laid_out_glyph(&layout, "25", 0);
        let degree = nth_laid_out_glyph(&layout, "°", 0);
        let unit = nth_laid_out_glyph(&layout, "C", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            digits,
            degree,
            "degree suffix abbreviation should stay with the preceding digits",
        );
        assert_vertical_layout_after(
            degree,
            unit,
            "Latin temperature unit tail should stay with the degree suffix",
        );
        assert_vertical_layout_column_restart(
            unit,
            next_body,
            next_column_moves_right,
            "body text after a decomposed temperature unit should continue in the next column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_ideographic_numeric_abbreviations_unbroken() {
    // W3C JLREQ 3.1.10 applies the same prefixed/postfixed abbreviation
    // rule to ideographic numerals.
    let text = "天$五人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let prefix = nth_laid_out_glyph(&layout, "$", 0);
        let ideographic_numeral = nth_laid_out_glyph(&layout, "五", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            prefix,
            ideographic_numeral,
            "ideographic numeral after numeric prefix abbreviation should stay attached",
        );
        assert_next_vertical_layout_column(
            ideographic_numeral,
            next_body,
            next_column_moves_right,
            "body text after a prefixed ideographic numeral should continue in the next column",
        );
    }

    let text = "天五%人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let ideographic_numeral = nth_laid_out_glyph(&layout, "五", 0);
        let suffix = nth_laid_out_glyph(&layout, "%", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            ideographic_numeral,
            suffix,
            "numeric suffix abbreviation should stay with the preceding ideographic numeral",
        );
        assert_next_vertical_layout_column(
            suffix,
            next_body,
            next_column_moves_right,
            "body text after a postfixed ideographic numeral should continue in the next column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_reference_mark_sequence_unbroken() {
    // W3C JLREQ 3.1.10 keeps reference marks with their preceding main
    // text, keeps multi-character reference marks together, and keeps the
    // following full stop attached to that reference mark sequence.
    let text = "本¹²。人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let body = nth_laid_out_glyph(&layout, "本", 0);
        let first_mark = nth_laid_out_glyph(&layout, "¹", 0);
        let second_mark = nth_laid_out_glyph(&layout, "²", 0);
        let full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            body,
            first_mark,
            "reference mark should stay with the preceding main-text cluster",
        );
        assert_vertical_layout_after(
            first_mark,
            second_mark,
            "reference mark digits should stay together",
        );
        assert_f32_eq(second_mark.origin.x, full_stop.origin.x);
        assert!(
            full_stop.bounds.bottom() > second_mark.origin.y,
            "full stop after a reference mark should stay attached to the reference mark column: {full_stop:?}"
        );
        assert_next_vertical_layout_column(
            full_stop,
            next_body,
            next_column_moves_right,
            "body text after the reference mark sequence should continue in the next column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_parenthesized_reference_mark_unbroken() {
    // W3C JLREQ 3.1.10 also treats opening/closing brackets inside a
    // reference mark as part of the same no-break reference sequence.
    let text = "本⁽¹⁾。人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let body = nth_laid_out_glyph(&layout, "本", 0);
        let open = nth_laid_out_glyph(&layout, "⁽", 0);
        let mark = nth_laid_out_glyph(&layout, "¹", 0);
        let close = nth_laid_out_glyph(&layout, "⁾", 0);
        let full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            body,
            open,
            "reference mark opening bracket should stay with the preceding main-text cluster",
        );
        assert_vertical_layout_after(
            open,
            mark,
            "reference mark digit should stay with the opening bracket",
        );
        assert_f32_eq(mark.origin.x, close.origin.x);
        assert!(
            close.bounds.bottom() > mark.origin.y,
            "reference mark closing bracket should stay attached to the reference digit column: {close:?}"
        );
        assert_f32_eq(close.origin.x, full_stop.origin.x);
        assert!(
            full_stop.bounds.bottom() > close.origin.y,
            "full stop after a parenthesized reference mark should stay attached: {full_stop:?}"
        );
        assert_next_vertical_layout_column(
            full_stop,
            next_body,
            next_column_moves_right,
            "body text after the parenthesized reference mark should continue in the next column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_latin_units_and_words_unbroken() {
    // W3C JLREQ 3.1.10 keeps Latin unit symbols and Western words
    // unbroken inside the sequence of letters and word-internal hyphens.
    let text = "天kg人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let unit = nth_laid_out_glyph(&layout, "kg", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_eq!(unit.orientation, GlyphOrientation::SidewaysCw);
        assert_next_vertical_layout_column(
            unit,
            next_body,
            next_column_moves_right,
            "body text after a Latin unit symbol should continue in the next column",
        );
    }

    let text = "天Web人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let word = nth_laid_out_glyph(&layout, "Web", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_eq!(word.orientation, GlyphOrientation::SidewaysCw);
        assert_vertical_layout_column_restart(
            word,
            next_body,
            next_column_moves_right,
            "body text after a Western word should continue in the next column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_hyphenated_western_words_unbroken() {
    let text = "天Web-Test人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 180.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let body = nth_laid_out_glyph(&layout, "天", 0);
        let first = nth_laid_out_glyph(&layout, "Web", 0);
        let hyphen = nth_laid_out_glyph(&layout, "-", 0);
        let after_hyphen = nth_laid_out_glyph(&layout, "Test", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_column_restart(
            body,
            first,
            next_column_moves_right,
            "hyphenated Western word should start as one object after body text",
        );
        assert_vertical_layout_after(
            first,
            hyphen,
            "word-internal hyphen should stay attached to the preceding letters",
        );
        assert_vertical_layout_after(
            hyphen,
            after_hyphen,
            "letters after a word-internal hyphen should stay attached",
        );
        assert_next_vertical_layout_column(
            after_hyphen,
            next_body,
            next_column_moves_right,
            "body text after a hyphenated Western word should continue in the next column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_apostrophe_western_words_unbroken() {
    for (text, joiner) in [("天O'K人", "'"), ("天O’K人", "’")] {
        for (writing_mode, next_column_moves_right) in [
            (RichTextWritingMode::VerticalRl, false),
            (RichTextWritingMode::VerticalLr, true),
        ] {
            let frame = frame_with_run(text, vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(210.0, 90.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");

            let body = nth_laid_out_glyph(&layout, "天", 0);
            let first = nth_laid_out_glyph(&layout, "O", 0);
            let apostrophe = nth_laid_out_glyph(&layout, joiner, 0);
            let after_apostrophe = nth_laid_out_glyph(&layout, "K", 0);
            let next_body = nth_laid_out_glyph(&layout, "人", 0);
            assert_vertical_layout_column_restart(
                body,
                first,
                next_column_moves_right,
                "apostrophe Western word should start as one object after body text",
            );
            assert_vertical_layout_after(
                first,
                apostrophe,
                "word-internal apostrophe should stay attached to the preceding letter",
            );
            assert_vertical_layout_after(
                apostrophe,
                after_apostrophe,
                "letter after a word-internal apostrophe should stay attached",
            );
            assert_next_vertical_layout_column(
                after_apostrophe,
                next_body,
                next_column_moves_right,
                "body text after an apostrophe Western word should continue in the next column",
            );
        }
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_accented_latin_words_unbroken() {
    let text = "天café人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let body = nth_laid_out_glyph(&layout, "天", 0);
        let word = nth_laid_out_glyph(&layout, "café", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_column_restart(
            body,
            word,
            next_column_moves_right,
            "accented Latin word should start as one Western word after body text",
        );
        assert_vertical_layout_column_restart(
            word,
            next_body,
            next_column_moves_right,
            "body text after an accented Latin word should continue in the next column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_greek_latin_unit_symbols_unbroken() {
    // W3C JLREQ 3.9 classifies unit symbols as combinations of Latin and
    // Greek script characters used for SI units.
    let text = "天μm人";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 60.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        let body = nth_laid_out_glyph(&layout, "天", 0);
        let unit = nth_laid_out_glyph(&layout, "μm", 0);
        let next_body = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_column_restart(
            body,
            unit,
            next_column_moves_right,
            "Greek+Latin SI unit symbol should start as one unit after body text",
        );
        assert_vertical_layout_column_restart(
            unit,
            next_body,
            next_column_moves_right,
            "body text after a Greek+Latin SI unit symbol should continue in the next column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_numeric_unit_symbols_unbroken() {
    for (text, unit_text) in [("天3kg人", "kg"), ("天3μm人", "μm")] {
        for (writing_mode, next_column_moves_right) in [
            (RichTextWritingMode::VerticalRl, false),
            (RichTextWritingMode::VerticalLr, true),
        ] {
            let frame = frame_with_run(text, vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(210.0, 90.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");

            let body = nth_laid_out_glyph(&layout, "天", 0);
            let digits = nth_laid_out_glyph(&layout, "3", 0);
            let unit = nth_laid_out_glyph(&layout, unit_text, 0);
            let next_body = nth_laid_out_glyph(&layout, "人", 0);
            assert_vertical_layout_column_restart(
                body,
                digits,
                next_column_moves_right,
                "numeric unit symbol should restart as one unit after body text",
            );
            assert_vertical_layout_after(
                digits,
                unit,
                "unit symbol should stay attached to the preceding digit",
            );
            assert_vertical_layout_column_restart(
                unit,
                next_body,
                next_column_moves_right,
                "body text after a numeric unit symbol should continue in the next column",
            );
        }
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_subscript_object_unbroken() {
    // W3C JLREQ 3.1.10 treats subscripts and superscripts with adjacent
    // base characters as one object, distinct from reference marks.
    for (text, base_text, mark_text, following_base_text) in [
        ("天H₂O人", "H", "₂", "O"),
        ("天α₂β人", "α", "₂", "β"),
        ("天α²β人", "α", "²", "β"),
    ] {
        for (writing_mode, next_column_moves_right) in [
            (RichTextWritingMode::VerticalRl, false),
            (RichTextWritingMode::VerticalLr, true),
        ] {
            let frame = frame_with_run(text, vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(210.0, 84.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");

            let body = nth_laid_out_glyph(&layout, "天", 0);
            let base = nth_laid_out_glyph(&layout, base_text, 0);
            let mark = nth_laid_out_glyph(&layout, mark_text, 0);
            let following_base = nth_laid_out_glyph(&layout, following_base_text, 0);
            let next_body = nth_laid_out_glyph(&layout, "人", 0);
            assert_vertical_layout_column_restart(
                body,
                base,
                next_column_moves_right,
                "published JLREQ sub/superscript object should start after body text",
            );
            assert_vertical_layout_after(
                base,
                mark,
                "sub/superscript should stay attached to the preceding base character",
            );
            assert_vertical_layout_after(
                mark,
                following_base,
                "following base character should stay attached to the sub/superscript object",
            );
            assert_next_vertical_layout_column(
                following_base,
                next_body,
                next_column_moves_right,
                "body text after the sub/superscript object should continue in the next column",
            );
        }
    }
}
