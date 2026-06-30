#[test]
fn web_player_source_has_no_dom_overlay_renderer() {
    let app = include_str!("../../arcweft-player-web/src/app.rs");
    let web_host = include_str!("../src/web.rs");

    assert!(app.contains("with_canvas(Some(state.canvas.clone()))"));
    assert!(app.contains("with_append(false)"));
    assert!(web_host.contains("renderer.render_to_view"));

    for forbidden in [
        "document.create_element(\"button\")",
        "document.create_element(\"div\")",
        "CanvasRenderingContext2d",
        "get_context(\"2d\")",
    ] {
        assert!(
            !app.contains(forbidden),
            "web player source must not contain DOM/canvas UI fallback: {forbidden}"
        );
    }
}
