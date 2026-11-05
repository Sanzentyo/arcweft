#[test]
fn agent_observe_native_renderer_resolves_point_and_pixel_font_sizes_in_logical_space() {
    use arcweft_agent_protocol::rich_text::AgentRichTextElementKind;
    use arcweft_agent_protocol::text_context::AgentTextContextCatalog;
    use arcweft_text_model::RichTextRange;

    for mode in ["horizontal_tb", "vertical_rl", "vertical_lr"] {
        let mut expected = None;
        for size in ["18pt", "24px"] {
            let (path, entry) = temp_main_cli_fixture(
                &format!("agent-observe-native-logical-font-{mode}-{size}"),
                &format!(
                    "pub character alice {{ display = \"Alice\" }}\nflow main {{ alice: #layout(.{mode})[#size({size})[天地夢]][p] }}"
                ),
            );
            let json = observe_native_rich_text_layer_report_at_entry(&path, &entry);
            fs::remove_file(&path).expect("remove logical font unit fixture");
            assert_native_rich_text_layer_image_has_content(&json);
            let run = find_rich_text_run_object(&json, "天地夢");
            let catalog: AgentTextContextCatalog =
                serde_json::from_value(json["text_contexts"].clone()).unwrap();
            let (context, element) = checked_observed_text_element(&catalog, run);
            assert_eq!(element.kind, AgentRichTextElementKind::TextRun);
            assert_eq!(element.range, RichTextRange::new(0, 9));
            let owner = catalog.resolve(&context).unwrap();
            let layout = owner.content.layout();
            let styled = &layout.runs[element.index];
            assert_eq!(styled.style.font_size_milli(), 24_000, "{mode} {size}");
            assert_eq!(styled.style.line_height_milli(), 32_400, "{mode} {size}");
            assert_eq!(styled.source_range, RichTextRange::new(0, 9));
            assert_eq!(layout.glyphs.len(), 3);
            let geometry = layout
                .glyphs
                .iter()
                .map(|glyph| {
                    assert_eq!(glyph.shape_key.font_size_bits, 24.0_f32.to_bits());
                    (glyph.source_range, glyph.layout_bounds, glyph.advance)
                })
                .collect::<Vec<_>>();
            if let Some(expected) = &expected {
                assert_eq!(
                    &geometry, expected,
                    "{mode}: point and pixel glyph geometry"
                );
            } else {
                expected = Some(geometry);
            }
        }
    }
}

#[test]
fn agent_observe_native_renderer_keeps_generic_and_quoted_named_font_families() {
    use arcweft_agent_protocol::text_context::AgentTextContextCatalog;
    use arcweft_render_text::TextFontFamily;

    for (source, expected) in [
        ("serif", TextFontFamily::Serif),
        ("sans_serif", TextFontFamily::SansSerif),
        ("monospace", TextFontFamily::Monospace),
        ("cursive", TextFontFamily::Cursive),
        ("fantasy", TextFontFamily::Fantasy),
        (r#""serif""#, TextFontFamily::Named("serif".into())),
        (
            r#""Noto Sans JP""#,
            TextFontFamily::Named("Noto Sans JP".into()),
        ),
    ] {
        let (path, entry) = temp_main_cli_fixture(
            "agent-observe-native-font-family",
            &format!(
                "pub character alice {{ display = \"Alice\" }}\nflow main {{ alice: #font({source})[天地夢][p] }}"
            ),
        );
        let json = observe_native_rich_text_layer_report_at_entry(&path, &entry);
        fs::remove_file(&path).expect("remove font family fixture");
        assert_native_rich_text_layer_image_has_content(&json);
        let run = find_rich_text_run_object(&json, "天地夢");
        let catalog: AgentTextContextCatalog =
            serde_json::from_value(json["text_contexts"].clone()).unwrap();
        let (context, element) = checked_observed_text_element(&catalog, run);
        let owner = catalog.resolve(&context).unwrap();
        let styled = &owner.content.layout().runs[element.index];
        assert_eq!(
            styled.style.font_families().first(),
            Some(&expected),
            "{source}"
        );
        assert_eq!(
            styled.source_range,
            arcweft_text_model::RichTextRange::new(0, 9)
        );
    }
}
