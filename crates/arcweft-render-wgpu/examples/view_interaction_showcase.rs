use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::{
    InputEpoch, InputEvent, InteractionTarget, PointerId, PointerInput, PointerPhase,
    RawInputEvent, RawInputKind, ViewportPoint,
};
use arcweft_presentation::interaction::{FocusState, InteractionState, PressedTarget};
use arcweft_presentation::layer::{
    LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase,
};
use arcweft_presentation::router::InputRouter;
use arcweft_presentation::semantic::{SemanticRole, SemanticTree};
use arcweft_render_wgpu::view::ViewPaintPlan;
use arcweft_view::{
    EventBinding, EventKind, FragmentKind, HandlerId, LayoutBox, LayoutLength, LayoutPoint,
    LayoutResults, LayoutSize, LayoutTree, Milli, NodeKey, Rgba8, RichTextSourceId, SemanticSpecId,
    StyleId, UiInteractionSelector, UiLayerOutput, UiPropertyKind, UiPropertyValue,
    UiSemanticFragmentBuilder, UiSemanticNode, UiStyle, UiStyleTable, ViewFragmentBuilder,
};
use num_traits::ToPrimitive;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

type VisualStates = Vec<(&'static str, InteractionState)>;
type Activation = arcweft_view::UiHandlerInvocation;
type ShowcaseFragment = (
    arcweft_view::ViewFragment,
    LayoutResults,
    arcweft_view::UiSemanticFragment,
    Vec<InteractionTarget>,
);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let out = output_directory()?;
    fs::create_dir_all(&out).map_err(|error| error.to_string())?;
    let (output, layer, targets) = showcase_output()?;
    let (states, activation) = routed_states(&output, &layer, &targets)?;

    for (name, interaction) in states {
        let display = output
            .display()
            .resolve_interaction_styles(output.semantics(), output.styles(), &interaction)
            .map_err(|error| error.to_string())?;
        let plan = ViewPaintPlan::from_resolved_display(&display);
        let svg = svg_document(name, plan.rectangles(), output.semantics().as_slice());
        let path = out.join(format!("{name}.svg"));
        fs::write(&path, svg).map_err(|error| error.to_string())?;
        println!("wrote {}", path.display());
    }
    let activation_path = out.join("activation.txt");
    fs::write(
        &activation_path,
        format!(
            "routed activate target={} handler={}\n",
            activation.target().id(),
            activation.handler().0,
        ),
    )
    .map_err(|error| error.to_string())?;
    println!("wrote {}", activation_path.display());
    Ok(())
}

fn output_directory() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    let mut out = PathBuf::from("target/ui-interaction-showcase");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = PathBuf::from(args.next().ok_or("--out requires a path")?),
            "--help" | "-h" => {
                println!(
                    "usage: cargo run -p arcweft-render-wgpu --example ui_interaction_showcase -- [--out DIR]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(out)
}

fn showcase_output() -> Result<(UiLayerOutput, LayerId, Vec<InteractionTarget>), String> {
    let layer = LayerId::new(public_id("layer.ui.showcase")?);
    let (fragment, layouts, semantics, targets) = showcase_fragment(&layer)?;
    let styles = showcase_styles()?;

    UiLayerOutput::from_fragment_with_styles(&fragment, &layouts, semantics, styles)
        .map(|output| (output, layer, targets))
        .map_err(|error| error.to_string())
}

fn showcase_fragment(layer: &LayerId) -> Result<ShowcaseFragment, String> {
    let labels = ["Settings", "Inventory", "Close"];
    let mut fragment = ViewFragmentBuilder::default();
    let mut nodes = Vec::new();
    let mut targets = Vec::new();
    let mut semantics = UiSemanticFragmentBuilder::default();

    for (index, label) in labels.iter().enumerate() {
        let index_u32 = u32::try_from(index).map_err(|error| error.to_string())?;
        let key = NodeKey(u64::try_from(index + 1).map_err(|error| error.to_string())?);
        let target = InteractionTarget::new(public_id(&format!("target.showcase.{index}"))?);
        let semantic = SemanticSpecId(index_u32);
        let node = fragment
            .push_node(
                key,
                FragmentKind::RichText(RichTextSourceId(index_u32)),
                StyleId(1),
                &[],
                &[EventBinding::new(EventKind::Activate, HandlerId(index_u32))],
                Some(semantic),
            )
            .map_err(|error| error.to_string())?;
        let y = 56 + i32::try_from(index).map_err(|error| error.to_string())? * 72;
        semantics
            .push(
                UiSemanticNode::new(
                    key,
                    layer.clone(),
                    target.clone(),
                    SemanticRole::Button,
                    HitRect::new(44.0, y.to_f32().unwrap_or_default(), 280.0, 52.0),
                )
                .with_label(*label)
                .with_action(public_id("action.showcase.activate")?),
            )
            .map_err(|error| error.to_string())?;
        nodes.push((node, y));
        targets.push(target);
    }

    let fragment = fragment.finish();
    let tree = LayoutTree::from_fragment(&fragment).map_err(|error| error.to_string())?;
    let mut layouts = LayoutResults::new(&tree);
    for (node, y) in nodes {
        layouts
            .set(
                node,
                LayoutBox::new(
                    LayoutPoint::new(LayoutLength::px(44), LayoutLength::px(y)),
                    LayoutSize::new(LayoutLength::px(280), LayoutLength::px(52)),
                ),
            )
            .map_err(|error| error.to_string())?;
    }
    Ok((fragment, layouts, semantics.finish(), targets))
}

fn showcase_styles() -> Result<UiStyleTable, String> {
    let mut style = UiStyle::default();
    style
        .set_base(
            UiPropertyKind::BackgroundColor,
            UiPropertyValue::Color(Rgba8::new(30, 48, 78, 255)),
        )
        .map_err(|error| error.to_string())?;
    style
        .set_rule(
            UiInteractionSelector::Hovered,
            UiPropertyKind::BackgroundColor,
            UiPropertyValue::Color(Rgba8::new(50, 88, 142, 255)),
        )
        .map_err(|error| error.to_string())?;
    style
        .set_rule(
            UiInteractionSelector::Focused,
            UiPropertyKind::OutlineColor,
            UiPropertyValue::Color(Rgba8::new(118, 205, 255, 255)),
        )
        .map_err(|error| error.to_string())?;
    style
        .set_rule(
            UiInteractionSelector::Focused,
            UiPropertyKind::OutlineWidth,
            UiPropertyValue::Milli(Milli::new(3_000)),
        )
        .map_err(|error| error.to_string())?;
    style
        .set_rule(
            UiInteractionSelector::Pressed,
            UiPropertyKind::BackgroundColor,
            UiPropertyValue::Color(Rgba8::new(24, 68, 112, 255)),
        )
        .map_err(|error| error.to_string())?;
    style
        .set_rule(
            UiInteractionSelector::Pressed,
            UiPropertyKind::Scale,
            UiPropertyValue::Milli(Milli::new(970)),
        )
        .map_err(|error| error.to_string())?;
    let mut styles = UiStyleTable::default();
    styles
        .insert(StyleId(1), style)
        .map_err(|error| error.to_string())?;
    Ok(styles)
}

fn routed_states(
    output: &UiLayerOutput,
    layer: &LayerId,
    targets: &[InteractionTarget],
) -> Result<(VisualStates, Activation), String> {
    let layers = showcase_layers(layer)?;
    let semantics = output.semantics().to_semantic_tree();
    let hits = semantics.to_hit_tree();
    let pointer = PointerId(0);
    let neutral = InteractionState::default();
    let hovered = hover_state(pointer, &semantics, &layers, &hits, &targets[1])?;
    let focused = focus_state(pointer, &semantics, &layers, &hits, &targets[0], 1)?;
    let pressed = pressed_state(pointer, &semantics, &layers, &hits, &targets[2])?;
    let activation = activation_for(output, InputEpoch(3), pressed.primary_pressed_target())?;

    Ok((
        vec![
            ("neutral", neutral),
            ("hovered", hovered),
            ("focused", focused),
            ("pressed", pressed),
        ],
        activation,
    ))
}

fn showcase_layers(layer: &LayerId) -> Result<LayerTree, String> {
    let root = LayerId::new(public_id("layer.root")?);
    let mut layers = LayerTree::new(
        LayerNode::new(
            root.clone(),
            LayerKind::Root,
            LayerOrder {
                phase: RenderPhase::Background,
                z: 0,
                stable_index: 0,
            },
        )
        .with_input_policy(LayerInputPolicy::Ignore),
    );
    layers
        .insert(
            LayerNode::new(
                layer.clone(),
                LayerKind::GameUi,
                LayerOrder {
                    phase: RenderPhase::GameUi,
                    z: 0,
                    stable_index: 0,
                },
            )
            .with_parent(root)
            .with_input_policy(LayerInputPolicy::HitTest),
        )
        .map_err(|error| format!("failed to build showcase layer tree: {error:?}"))?;
    Ok(layers)
}

fn hover_state(
    pointer: PointerId,
    semantics: &SemanticTree,
    layers: &LayerTree,
    hits: &arcweft_presentation::hit::HitTree,
    target: &InteractionTarget,
) -> Result<InteractionState, String> {
    let hover_bounds = semantics
        .find(target)
        .ok_or("missing hover semantic target")?
        .bounds();
    let hover_position = center(hover_bounds);
    let mut hovered = InteractionState::default();
    let hover_path = InputRouter::hover_path(pointer, hover_position, layers, hits)
        .ok_or("hover routing did not find the second button")?;
    let _ = hovered.set_hover_path(hover_path);
    Ok(hovered)
}

fn focus_state(
    pointer: PointerId,
    semantics: &SemanticTree,
    layers: &LayerTree,
    hits: &arcweft_presentation::hit::HitTree,
    target: &InteractionTarget,
    epoch: u64,
) -> Result<InteractionState, String> {
    let focus_bounds = semantics
        .find(target)
        .ok_or("missing focus semantic target")?
        .bounds();
    let down_event = routed_pointer_down(
        pointer,
        center(focus_bounds),
        InputEpoch(epoch),
        layers,
        hits,
    )?;
    let focus_node = semantics
        .find(down_event.target())
        .ok_or("routed pointer target has no semantic node")?;
    let mut focused = InteractionState::default();
    focused.set_focus(FocusState::new(
        focus_node.layer().clone(),
        down_event.target().clone(),
    ));
    Ok(focused)
}

fn pressed_state(
    pointer: PointerId,
    semantics: &SemanticTree,
    layers: &LayerTree,
    hits: &arcweft_presentation::hit::HitTree,
    target: &InteractionTarget,
) -> Result<InteractionState, String> {
    let pressed_bounds = semantics
        .find(target)
        .ok_or("missing pressed semantic target")?
        .bounds();
    let pressed_position = center(pressed_bounds);
    let down_event = routed_pointer_down(pointer, pressed_position, InputEpoch(2), layers, hits)?;
    let pressed_node = semantics
        .find(down_event.target())
        .ok_or("pressed target has no semantic node")?;
    let mut pressed = InteractionState::default();
    if let Some(path) = InputRouter::hover_path(pointer, pressed_position, layers, hits) {
        let _ = pressed.set_hover_path(path);
    }
    pressed.set_focus(FocusState::new(
        pressed_node.layer().clone(),
        down_event.target().clone(),
    ));
    pressed.press_pointer(PressedTarget::new(
        pointer,
        pressed_node.layer().clone(),
        down_event.target().clone(),
    ));
    Ok(pressed)
}

fn routed_pointer_down(
    pointer: PointerId,
    position: ViewportPoint,
    epoch: InputEpoch,
    layers: &LayerTree,
    hits: &arcweft_presentation::hit::HitTree,
) -> Result<InputEvent, String> {
    let down = RawInputEvent::new(
        epoch,
        RawInputKind::Pointer(PointerInput {
            pointer,
            position,
            phase: PointerPhase::Down,
        }),
    );
    InputRouter::route(&down, layers, hits, &InteractionState::default())
        .event()
        .cloned()
        .ok_or("pointer down was not routed".to_string())
}

fn activation_for(
    output: &UiLayerOutput,
    epoch: InputEpoch,
    target: Option<&InteractionTarget>,
) -> Result<Activation, String> {
    let target = target.ok_or("pressed state did not retain a target")?;
    let activate = InputEvent::activate(epoch, target.clone());
    let mut invocations = output.handlers().dispatch_input(&activate);
    if invocations.len() != 1 {
        return Err(format!(
            "expected one activation handler, found {}",
            invocations.len()
        ));
    }
    Ok(invocations.remove(0))
}

fn center(bounds: HitRect) -> ViewportPoint {
    ViewportPoint::new(
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    )
}

fn svg_document(
    state: &str,
    rectangles: &[arcweft_render_wgpu::geometry::PaintRect],
    semantics: &[UiSemanticNode],
) -> String {
    let mut svg = String::from(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="368" height="300" viewBox="0 0 368 300">"#,
    );
    svg.push_str(r##"<rect width="368" height="300" fill="#0b1020"/>"##);
    writeln!(
        svg,
        r##"<text x="44" y="32" fill="#d8e8ff" font-family="sans-serif" font-size="18">state: {state}</text>"##
    )
    .expect("writing to String cannot fail");
    for rectangle in rectangles {
        let [red, green, blue, alpha] = rectangle.rgba;
        writeln!(
            svg,
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="rgb({},{},{})" fill-opacity="{}"/>"#,
            rectangle.bounds.x,
            rectangle.bounds.y,
            rectangle.bounds.width,
            rectangle.bounds.height,
            channel(red),
            channel(green),
            channel(blue),
            alpha.clamp(0.0, 1.0),
        )
        .expect("writing to String cannot fail");
    }
    for semantic in semantics {
        if let Some(label) = semantic.label() {
            let bounds = semantic.bounds();
            writeln!(
                svg,
                r##"<text x="{}" y="{}" fill="#f6f9ff" font-family="sans-serif" font-size="17">{}</text>"##,
                bounds.x + 18.0,
                bounds.y + 32.0,
                escape_xml(label),
            )
            .expect("writing to String cannot fail");
        }
    }
    svg.push_str("</svg>\n");
    svg
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0)
        .round()
        .to_u8()
        .unwrap_or_default()
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn public_id(value: &str) -> Result<PublicId, String> {
    PublicId::try_new(value).map_err(|error| error.to_string())
}
