use crate::hit::{HitRecord, HitTree};
use crate::input::{InputEvent, InputEventKind, InteractionTarget, KeyPhase, PointerPhase};
use crate::interaction::InteractionState;
use crate::layer::{
    LayerContent, LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree,
    LayerVisibility, RenderPhase,
};
use crate::router::{RouteDecision, RoutedInput};
use crate::text_input::{
    CompositionEndReason, PlatformTextSelection, TextByteOffset, TextCommit, TextCompositionUpdate,
    TextDeleteUnit, TextEditCommand, TextInput, TextInputOperation, TextInputPrivacy, TextRange,
};
use arcweft_id::PublicId;

/// Deterministic hash of the routing-relevant presentation state.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct RoutingHash(pub u64);

/// Replay fingerprint for one routed input decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteFingerprint {
    routing_hash: RoutingHash,
    raw_epoch: crate::input::InputEpoch,
    decision_hash: RoutingHash,
}

/// Hash the parts of presentation state that affect input routing.
pub fn routing_hash(layers: &LayerTree, hits: &HitTree, state: &InteractionState) -> RoutingHash {
    let mut hasher = StableHasher::new("arcweft.presentation.routing.v2");
    hash_layer_tree(&mut hasher, layers);
    hash_hit_tree(&mut hasher, hits);
    hash_interaction_state(&mut hasher, state);
    RoutingHash(hasher.finish())
}

/// Hash a routed input together with the routing state that produced it.
pub fn route_fingerprint(
    layers: &LayerTree,
    hits: &HitTree,
    state: &InteractionState,
    routed: &RoutedInput,
) -> RouteFingerprint {
    let route_hash = routing_hash(layers, hits, state);
    let mut hasher = StableHasher::new("arcweft.presentation.route-decision.v2");
    hasher.u64(routed.raw_epoch().0);
    hash_route_decision(&mut hasher, routed.decision());
    RouteFingerprint {
        routing_hash: route_hash,
        raw_epoch: routed.raw_epoch(),
        decision_hash: RoutingHash(hasher.finish()),
    }
}

impl RouteFingerprint {
    pub const fn routing_hash(self) -> RoutingHash {
        self.routing_hash
    }

    pub const fn raw_epoch(self) -> crate::input::InputEpoch {
        self.raw_epoch
    }

    pub const fn decision_hash(self) -> RoutingHash {
        self.decision_hash
    }
}

struct StableHasher {
    state: u64,
}

impl StableHasher {
    fn new(domain: &str) -> Self {
        let mut hasher = Self {
            state: 0xcbf2_9ce4_8422_2325,
        };
        hasher.str(domain);
        hasher
    }

    const fn finish(self) -> u64 {
        self.state
    }

    fn byte(&mut self, value: u8) {
        self.state ^= u64::from(value);
        self.state = self.state.wrapping_mul(0x0000_0100_0000_01b3);
    }

    fn bytes(&mut self, values: &[u8]) {
        for value in values {
            self.byte(*value);
        }
    }

    fn marker(&mut self, value: &str) {
        self.str(value);
        self.byte(0xff);
    }

    fn bool(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn f32(&mut self, value: f32) {
        self.u32(value.to_bits());
    }

    fn str(&mut self, value: &str) {
        self.u64(value.len() as u64);
        self.bytes(value.as_bytes());
    }

    fn public_id(&mut self, id: &PublicId) {
        self.str(id.as_str());
    }

    fn layer_id(&mut self, id: &LayerId) {
        self.public_id(id.public_id());
    }

    fn target(&mut self, target: &InteractionTarget) {
        self.public_id(target.id());
    }
}

fn hash_layer_tree(hasher: &mut StableHasher, layers: &LayerTree) {
    hasher.marker("layers");
    hasher.layer_id(layers.root());
    hasher.u64(layers.render_order().len() as u64);
    for layer in layers.render_order() {
        hasher.layer_id(layer);
    }
    hasher.u64(layers.input_order().len() as u64);
    for layer in layers.input_order() {
        hasher.layer_id(layer);
        if let Some(node) = layers.get(layer) {
            hash_layer_node(hasher, node);
        }
    }
}

fn hash_layer_node(hasher: &mut StableHasher, node: &LayerNode) {
    hasher.marker("layer-node");
    hasher.layer_id(node.id());
    hasher.bool(node.public_id().is_some());
    if let Some(public_id) = node.public_id() {
        hasher.public_id(public_id);
    }
    hasher.u32(layer_kind_code(node.kind()));
    hash_layer_content(hasher, node.content());
    hasher.bool(node.parent().is_some());
    if let Some(parent) = node.parent() {
        hasher.layer_id(parent);
    }
    hasher.u64(node.children().len() as u64);
    for child in node.children() {
        hasher.layer_id(child);
    }
    hash_layer_order(hasher, node.order());
    hash_layer_transform(hasher, node.transform());
    hasher.u32(layer_visibility_code(node.visibility()));
    hasher.u32(layer_input_policy_code(node.input_policy()));
}

fn hash_layer_content(hasher: &mut StableHasher, content: &LayerContent) {
    match content {
        LayerContent::Empty => hasher.u32(0),
        LayerContent::Dialogue(id) => {
            hasher.u32(1);
            hasher.public_id(id);
        }
        LayerContent::Activity(id) => {
            hasher.u32(2);
            hasher.public_id(id);
        }
        LayerContent::NativeView(id) => {
            hasher.u32(3);
            hasher.public_id(id);
        }
        LayerContent::Html(id) => {
            hasher.u32(4);
            hasher.public_id(id);
        }
        LayerContent::Custom(id) => {
            hasher.u32(5);
            hasher.public_id(id);
        }
        LayerContent::Character(render) => {
            hasher.u32(6);
            hasher.str(render.character().as_str());
            hasher.str(render.look().as_str());
            let canvas = render.canvas();
            hasher.u32(canvas.width());
            hasher.u32(canvas.height());
            let anchor = render.anchor();
            hasher.i32(anchor.x());
            hasher.i32(anchor.y());
            hasher.u64(render.layers().len() as u64);
            for layer in render.layers() {
                hasher.str(layer.part().as_str());
                hasher.str(layer.variant().as_str());
                hasher.public_id(layer.asset_id());
                hasher.str(layer.asset_path().as_str());
                let rect = layer.rect();
                hasher.i32(rect.x());
                hasher.i32(rect.y());
                hasher.u32(rect.width());
                hasher.u32(rect.height());
                hasher.i32(layer.z());
                hasher.u32(u32::from(layer.opacity()));
                hasher.u32(layer.blend().stable_code());
                hasher.bool(layer.clipping());
            }
        }
    }
}

fn hash_layer_order(hasher: &mut StableHasher, order: LayerOrder) {
    hasher.u32(render_phase_code(order.phase));
    hasher.i32(order.z);
    hasher.u32(order.stable_index);
}

fn hash_layer_transform(hasher: &mut StableHasher, transform: crate::layer::LayerTransform) {
    hasher.i32(transform.m11_milli);
    hasher.i32(transform.m12_milli);
    hasher.i32(transform.m21_milli);
    hasher.i32(transform.m22_milli);
    hasher.i32(transform.tx_milli);
    hasher.i32(transform.ty_milli);
}

fn hash_hit_tree(hasher: &mut StableHasher, hits: &HitTree) {
    hasher.marker("hits");
    hasher.u64(hits.as_slice().len() as u64);
    for record in hits.as_slice() {
        hash_hit_record(hasher, record);
    }
}

fn hash_hit_record(hasher: &mut StableHasher, record: &HitRecord) {
    hasher.marker("hit");
    hasher.layer_id(record.layer());
    hasher.target(record.target());
    hasher.u64(record.hover_path().len() as u64);
    for target in record.hover_path() {
        hasher.target(target);
    }
    let bounds = record.bounds();
    hasher.f32(bounds.x);
    hasher.f32(bounds.y);
    hasher.f32(bounds.width);
    hasher.f32(bounds.height);
    hasher.bool(record.enabled());
    hasher.bool(record.visible());
}

fn hash_interaction_state(hasher: &mut StableHasher, state: &InteractionState) {
    hasher.marker("interaction");
    hasher.bool(state.focus().layer().is_some());
    if let Some(layer) = state.focus().layer() {
        hasher.layer_id(layer);
    }
    hasher.bool(state.focus().target().is_some());
    if let Some(target) = state.focus().target() {
        hasher.target(target);
    }
    hasher.u64(state.captures().len() as u64);
    for capture in state.captures() {
        hasher.u64(capture.pointer().0);
        hasher.layer_id(capture.layer());
        hasher.target(capture.target());
    }
    hasher.u64(state.hover_paths().len() as u64);
    for path in state.hover_paths() {
        hasher.u64(path.pointer().0);
        hasher.u64(path.targets().len() as u64);
        for target in path.targets() {
            hasher.target(target);
        }
    }
    hasher.u64(state.pressed_targets().len() as u64);
    for pressed in state.pressed_targets() {
        hasher.u64(pressed.pointer().0);
        hasher.layer_id(pressed.layer());
        hasher.target(pressed.target());
    }
}

fn hash_route_decision(hasher: &mut StableHasher, decision: &RouteDecision) {
    match decision {
        RouteDecision::Routed(event) => {
            hasher.u32(0);
            hash_input_event(hasher, event);
        }
        RouteDecision::BlockedByModal { modal } => {
            hasher.u32(1);
            hasher.layer_id(modal);
        }
        RouteDecision::NoTarget => hasher.u32(2),
        RouteDecision::TargetUnavailable => hasher.u32(3),
        RouteDecision::LayerUnavailable { layer } => {
            hasher.u32(4);
            hasher.layer_id(layer);
        }
        RouteDecision::Ignored => hasher.u32(5),
    }
}

fn hash_input_event(hasher: &mut StableHasher, event: &InputEvent) {
    hasher.u64(event.raw_epoch().0);
    hasher.target(event.target());
    match event.kind() {
        InputEventKind::Activate => hasher.u32(0),
        InputEventKind::Pointer { phase } => {
            hasher.u32(1);
            hasher.u32(pointer_phase_code(*phase));
        }
        InputEventKind::Key { key, phase } => {
            hasher.u32(2);
            hasher.str(key);
            hasher.u32(key_phase_code(*phase));
        }
        InputEventKind::Text(value) => {
            hasher.u32(3);
            hash_text_input(hasher, value);
        }
        InputEventKind::Focus { focused } => {
            hasher.u32(4);
            hasher.bool(*focused);
        }
        InputEventKind::AgentInvoke { action } => {
            hasher.u32(5);
            hasher.public_id(action);
        }
    }
}

fn hash_text_input(hasher: &mut StableHasher, input: &TextInput) {
    hasher.u64(input.session().0);
    hasher.u64(input.serial().0);
    hasher.u32(text_input_privacy_code(input.privacy()));
    hasher.u64(input.operations().len() as u64);
    for operation in input.operations() {
        hash_text_input_operation(hasher, input.privacy(), operation);
    }
}

fn hash_text_input_operation(
    hasher: &mut StableHasher,
    privacy: TextInputPrivacy,
    operation: &TextInputOperation,
) {
    match operation {
        TextInputOperation::StartComposition => hasher.u32(0),
        TextInputOperation::SetComposition(update) => {
            hasher.u32(1);
            hash_composition_update(hasher, privacy, update);
        }
        TextInputOperation::Commit(commit) => {
            hasher.u32(2);
            hash_commit(hasher, privacy, commit);
        }
        TextInputOperation::EndComposition { reason } => {
            hasher.u32(3);
            hasher.u32(composition_end_reason_code(*reason));
        }
        TextInputOperation::DeleteSurrounding {
            before,
            after,
            unit,
        } => {
            hasher.u32(4);
            hasher.u32(*before);
            hasher.u32(*after);
            hasher.u32(text_delete_unit_code(*unit));
        }
        TextInputOperation::SetSelection(selection) => {
            hasher.u32(5);
            hash_platform_selection(hasher, *selection);
        }
        TextInputOperation::Command(command) => {
            hasher.u32(6);
            hasher.u32(text_edit_command_code(*command));
        }
    }
}

fn hash_composition_update(
    hasher: &mut StableHasher,
    privacy: TextInputPrivacy,
    update: &TextCompositionUpdate,
) {
    hasher.bool(update.replacement().is_some());
    if let Some(range) = update.replacement() {
        hash_text_range(hasher, range);
    }
    hash_text_payload(hasher, privacy, update.preedit());
    hash_text_range(hasher, update.selection());
    hasher.u64(update.segments().len() as u64);
    for segment in update.segments() {
        hash_text_range(hasher, segment.range());
        hasher.u32(segment.kind() as u32);
    }
}

fn hash_commit(hasher: &mut StableHasher, privacy: TextInputPrivacy, commit: &TextCommit) {
    hash_text_payload(hasher, privacy, commit.text());
    hasher.bool(commit.replacement().is_some());
    if let Some(range) = commit.replacement() {
        hash_text_range(hasher, range);
    }
}

fn hash_text_payload(hasher: &mut StableHasher, privacy: TextInputPrivacy, value: &str) {
    if privacy.is_sensitive() {
        hasher.marker("redacted-text");
        hasher.u64(value.chars().count() as u64);
        hasher.u64(value.len() as u64);
    } else {
        hasher.str(value);
    }
}

fn hash_text_range(hasher: &mut StableHasher, range: TextRange<TextByteOffset>) {
    hasher.u32(range.start().0);
    hasher.u32(range.end().0);
}

fn hash_platform_selection(hasher: &mut StableHasher, selection: PlatformTextSelection) {
    hash_text_range(hasher, selection.range());
    hasher.u32(selection.affinity() as u32);
}

const fn text_input_privacy_code(privacy: TextInputPrivacy) -> u32 {
    match privacy {
        TextInputPrivacy::Plain => 0,
        TextInputPrivacy::Sensitive => 1,
    }
}

const fn layer_kind_code(kind: LayerKind) -> u32 {
    match kind {
        LayerKind::Root => 0,
        LayerKind::Background => 1,
        LayerKind::World2D => 2,
        LayerKind::Character => 3,
        LayerKind::Effects => 4,
        LayerKind::Dialogue => 5,
        LayerKind::GameView => 6,
        LayerKind::HtmlView => 7,
        LayerKind::Activity => 8,
        LayerKind::Modal => 9,
        LayerKind::Overlay => 10,
        LayerKind::Debug => 11,
        LayerKind::Agent => 12,
        LayerKind::Offscreen => 13,
        LayerKind::Custom => 14,
    }
}

const fn render_phase_code(phase: RenderPhase) -> u32 {
    match phase {
        RenderPhase::Background => 0,
        RenderPhase::World => 1,
        RenderPhase::Characters => 2,
        RenderPhase::Effects => 3,
        RenderPhase::Dialogue => 4,
        RenderPhase::GameView => 5,
        RenderPhase::HtmlView => 6,
        RenderPhase::Modal => 7,
        RenderPhase::Debug => 8,
        RenderPhase::AgentOverlay => 9,
    }
}

const fn layer_visibility_code(visibility: LayerVisibility) -> u32 {
    match visibility {
        LayerVisibility::Visible => 0,
        LayerVisibility::Hidden => 1,
    }
}

const fn layer_input_policy_code(policy: LayerInputPolicy) -> u32 {
    match policy {
        LayerInputPolicy::Ignore => 0,
        LayerInputPolicy::PassThrough => 1,
        LayerInputPolicy::HitTest => 2,
        LayerInputPolicy::Modal => 3,
        LayerInputPolicy::Capture => 4,
    }
}

const fn pointer_phase_code(phase: PointerPhase) -> u32 {
    match phase {
        PointerPhase::Down => 0,
        PointerPhase::Move => 1,
        PointerPhase::Up => 2,
        PointerPhase::Cancel => 3,
    }
}

const fn key_phase_code(phase: KeyPhase) -> u32 {
    match phase {
        KeyPhase::Down => 0,
        KeyPhase::Up => 1,
    }
}

const fn selecting_code(selecting: bool) -> u32 {
    if selecting { 1 } else { 0 }
}

const fn composition_end_reason_code(reason: CompositionEndReason) -> u32 {
    match reason {
        CompositionEndReason::Committed => 0,
        CompositionEndReason::Cancelled => 1,
        CompositionEndReason::FocusChanged => 2,
        CompositionEndReason::SessionInvalidated => 3,
        CompositionEndReason::PlatformDisabled => 4,
    }
}

const fn text_delete_unit_code(unit: TextDeleteUnit) -> u32 {
    match unit {
        TextDeleteUnit::Utf16CodeUnit => 0,
        TextDeleteUnit::UnicodeScalar => 1,
        TextDeleteUnit::GraphemeCluster => 2,
        TextDeleteUnit::Utf8Byte => 3,
    }
}

const fn text_edit_command_code(command: TextEditCommand) -> u32 {
    match command {
        TextEditCommand::MoveLeft { selecting } => selecting_code(selecting),
        TextEditCommand::MoveRight { selecting } => 2 + selecting_code(selecting),
        TextEditCommand::MoveWordLeft { selecting } => 4 + selecting_code(selecting),
        TextEditCommand::MoveWordRight { selecting } => 6 + selecting_code(selecting),
        TextEditCommand::MoveLineStart { selecting } => 8 + selecting_code(selecting),
        TextEditCommand::MoveLineEnd { selecting } => 10 + selecting_code(selecting),
        TextEditCommand::Backspace => 12,
        TextEditCommand::Delete => 13,
        TextEditCommand::SelectAll => 14,
        TextEditCommand::Copy => 15,
        TextEditCommand::Cut => 16,
        TextEditCommand::Paste => 17,
        TextEditCommand::Submit => 18,
        TextEditCommand::Cancel => 19,
        TextEditCommand::MoveUp { selecting } => 20 + selecting_code(selecting),
        TextEditCommand::MoveDown { selecting } => 22 + selecting_code(selecting),
        TextEditCommand::MoveDocumentStart { selecting } => 24 + selecting_code(selecting),
        TextEditCommand::MoveDocumentEnd { selecting } => 26 + selecting_code(selecting),
        TextEditCommand::DeleteWordLeft => 28,
        TextEditCommand::DeleteWordRight => 29,
        TextEditCommand::MovePageUp { selecting } => 30 + selecting_code(selecting),
        TextEditCommand::MovePageDown { selecting } => 32 + selecting_code(selecting),
        TextEditCommand::SelectWord => 34,
        TextEditCommand::SelectLine => 35,
    }
}
