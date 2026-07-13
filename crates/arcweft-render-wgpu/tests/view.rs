use arcweft_id::PublicId;
use arcweft_presentation::appearance::{
    PresentationColor, PresentationEnvironment, SystemPaletteSet,
};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::interaction::{FocusState, InteractionState};
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_render_wgpu::view::ViewPaintPlan;
use arcweft_view::{
    FragmentKind, LayoutBox, LayoutLength, LayoutPoint, LayoutResults, LayoutSize, LayoutTree,
    NodeKey, RichTextSourceId, SemanticSpecId, ViewColorValue, ViewElementKind,
    ViewFragmentBuilder, ViewInteractionSelector, ViewLengthMilli, ViewPropertyKind,
    ViewSemanticFragmentBuilder, ViewSemanticNode, ViewSpecifiedValue, ViewStyleApplicationTarget,
    ViewStyleAssignOp, ViewStyleDeclaration, ViewStylePredicate, ViewStyleProgram,
    ViewStyleResolver, ViewStyleRevisionSet, ViewStyleRule, ViewStyleSelector,
    ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId,
};

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).unwrap()
}

fn color(red: u8, green: u8, blue: u8, alpha: u8) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Color {
        value: ViewColorValue::Literal {
            color: PresentationColor::rgba(red, green, blue, alpha),
        },
    }
}

fn selector(state: Option<ViewInteractionSelector>) -> ViewStyleSelector {
    ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(
            None,
            Some(ViewElementKind::Button),
            None,
            state
                .map(ViewStylePredicate::Interaction)
                .into_iter()
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn declaration(
    property: ViewPropertyKind,
    value: ViewSpecifiedValue,
    source: u32,
) -> ViewStyleDeclaration {
    ViewStyleDeclaration::new(
        property,
        value,
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(source),
    )
    .unwrap()
}

fn focused_button_style_program(sheet_id: ViewStyleSheetId) -> ViewStyleProgram {
    let rules = vec![
        ViewStyleRule::new(
            selector(None),
            vec![declaration(
                ViewPropertyKind::BackgroundColor,
                color(30, 60, 90, 255),
                0,
            )],
            0,
            ViewStyleSourceId::new(0),
        )
        .unwrap(),
        ViewStyleRule::new(
            selector(Some(ViewInteractionSelector::Focused)),
            vec![
                declaration(
                    ViewPropertyKind::OutlineColor,
                    color(120, 210, 255, 255),
                    10,
                ),
                declaration(
                    ViewPropertyKind::OutlineWidth,
                    ViewSpecifiedValue::Length {
                        value: ViewLengthMilli::new(3_000),
                    },
                    11,
                ),
            ],
            1,
            ViewStyleSourceId::new(1),
        )
        .unwrap(),
    ];
    let sheet = ViewStyleSheet::new(sheet_id, Vec::new(), rules).unwrap();
    ViewStyleProgram::try_new(vec![sheet], Vec::new()).unwrap()
}

#[test]
fn resolved_focus_style_lowers_to_background_and_outline_rectangles() {
    let layer = LayerId::new(public_id("layer.view"));
    let target = InteractionTarget::new(public_id("target.button.confirm"));
    let mut fragment = ViewFragmentBuilder::default();
    let sheet_id = ViewStyleSheetId::try_new("style.focused-button").unwrap();
    let style_applications = [ViewStyleApplicationTarget::named(sheet_id.clone())];
    let node = fragment
        .push_node(
            NodeKey(1),
            FragmentKind::RichText(RichTextSourceId(1)),
            &style_applications,
            &[],
            &[],
            Some(SemanticSpecId(0)),
        )
        .unwrap();
    let fragment = fragment.finish();
    let tree = LayoutTree::from_fragment(&fragment).unwrap();
    let mut layouts = LayoutResults::new(&tree);
    layouts
        .set(
            node,
            LayoutBox::new(
                LayoutPoint::new(LayoutLength::px(10), LayoutLength::px(20)),
                LayoutSize::new(LayoutLength::px(100), LayoutLength::px(40)),
            ),
        )
        .unwrap();
    let display = arcweft_view::DisplayList::from_fragment(&fragment, &layouts).unwrap();
    let mut semantics = ViewSemanticFragmentBuilder::default();
    semantics
        .push(ViewSemanticNode::new(
            NodeKey(1),
            layer.clone(),
            target.clone(),
            SemanticRole::Button,
            HitRect::new(10.0, 20.0, 100.0, 40.0),
        ))
        .unwrap();
    let semantics = semantics.finish();

    let program = focused_button_style_program(sheet_id);
    let mut interaction = InteractionState::default();
    interaction.set_focus(FocusState::new(layer, target));
    let resolved = display
        .resolve_styles(
            &semantics,
            &program,
            &interaction,
            &PresentationEnvironment::ENGINE_DEFAULT,
            ViewStyleRevisionSet::default(),
            &mut ViewStyleResolver::default(),
        )
        .unwrap();
    let plan = ViewPaintPlan::from_resolved_display(
        &resolved,
        &PresentationEnvironment::ENGINE_DEFAULT,
        &SystemPaletteSet::ENGINE_DEFAULT,
    );

    assert_eq!(plan.rectangles().len(), 5);
    assert_eq!(
        plan.rectangles()[0].bounds,
        HitRect::new(10.0, 20.0, 100.0, 40.0)
    );
    assert!((plan.rectangles()[1].bounds.height - 3.0).abs() < f32::EPSILON);
}
