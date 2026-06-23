use arcweft_id::PublicId;
use std::collections::BTreeMap;

/// Stable layer identifier used by render, input, Agent observation, and replay.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LayerId {
    id: PublicId,
}

/// Pure data `LayerTree` shared by render ordering and input routing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayerTree {
    root: LayerId,
    layers: BTreeMap<LayerId, LayerNode>,
    render_order: Vec<LayerId>,
    input_order: Vec<LayerId>,
}

/// One node in the presentation layer tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayerNode {
    id: LayerId,
    public_id: Option<PublicId>,
    kind: LayerKind,
    content: LayerContent,
    parent: Option<LayerId>,
    children: Vec<LayerId>,
    order: LayerOrder,
    transform: LayerTransform,
    visibility: LayerVisibility,
    input: LayerInputPolicy,
}

/// Semantic layer family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayerKind {
    Root,
    Background,
    World2D,
    Character,
    Effects,
    TextBox,
    GameUi,
    HtmlUi,
    Activity,
    Modal,
    Overlay,
    Debug,
    Agent,
    Offscreen,
    Custom,
}

/// Stable ordering key. Render order is ascending; input order is descending.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LayerOrder {
    pub phase: RenderPhase,
    pub z: i32,
    pub stable_index: u32,
}

/// Affine transform from layer-local coordinates to viewport coordinates.
///
/// Matrix fields are fixed-point values in thousandths. This keeps the
/// presentation model deterministic while still allowing hit-testing to map
/// viewport pointer coordinates into layer-local geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayerTransform {
    pub m11_milli: i32,
    pub m12_milli: i32,
    pub m21_milli: i32,
    pub m22_milli: i32,
    pub tx_milli: i32,
    pub ty_milli: i32,
}

/// Point in layer-local logical coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RenderPhase {
    Background,
    World,
    Characters,
    Effects,
    Dialogue,
    GameUi,
    HtmlUi,
    Modal,
    Debug,
    AgentOverlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayerVisibility {
    Visible,
    Hidden,
}

/// Input behavior for a layer after hit-testing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayerInputPolicy {
    Ignore,
    PassThrough,
    HitTest,
    Modal,
    Capture,
}

/// Render content family carried separately from layer policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayerContent {
    Empty,
    Character(crate::character::CharacterRenderSpec),
    TextBox(PublicId),
    Activity(PublicId),
    NativeUi(PublicId),
    Html(PublicId),
    Custom(PublicId),
}

/// Deterministic layer insertion errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayerTreeError {
    DuplicateLayer(LayerId),
    MissingParent(LayerId),
}

impl LayerId {
    pub const fn new(id: PublicId) -> Self {
        Self { id }
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.id
    }
}

impl LayerTree {
    pub fn new(root: LayerNode) -> Self {
        let root_id = root.id.clone();
        let mut layers = BTreeMap::new();
        layers.insert(root_id.clone(), root);
        let mut tree = Self {
            root: root_id,
            layers,
            render_order: Vec::new(),
            input_order: Vec::new(),
        };
        tree.rebuild_orders();
        tree
    }

    pub fn insert(&mut self, node: LayerNode) -> Result<(), LayerTreeError> {
        if self.layers.contains_key(&node.id) {
            return Err(LayerTreeError::DuplicateLayer(node.id));
        }
        if let Some(parent) = &node.parent {
            let Some(parent_node) = self.layers.get_mut(parent) else {
                return Err(LayerTreeError::MissingParent(parent.clone()));
            };
            parent_node.children.push(node.id.clone());
        }
        self.layers.insert(node.id.clone(), node);
        self.rebuild_orders();
        Ok(())
    }

    pub const fn root(&self) -> &LayerId {
        &self.root
    }

    pub fn get(&self, id: &LayerId) -> Option<&LayerNode> {
        self.layers.get(id)
    }

    pub fn render_order(&self) -> &[LayerId] {
        &self.render_order
    }

    pub fn input_order(&self) -> &[LayerId] {
        &self.input_order
    }

    fn rebuild_orders(&mut self) {
        let mut ordered = self
            .layers
            .values()
            .filter(|node| node.visibility == LayerVisibility::Visible)
            .collect::<Vec<_>>();
        ordered.sort_by_key(|node| (node.order, node.id.clone()));
        self.render_order = ordered.iter().map(|node| node.id.clone()).collect();
        self.input_order = self.render_order.iter().rev().cloned().collect();
    }
}

impl LayerNode {
    pub fn new(id: LayerId, kind: LayerKind, order: LayerOrder) -> Self {
        Self {
            id,
            public_id: None,
            kind,
            content: LayerContent::Empty,
            parent: None,
            children: Vec::new(),
            order,
            transform: LayerTransform::identity(),
            visibility: LayerVisibility::Visible,
            input: LayerInputPolicy::HitTest,
        }
    }

    #[must_use]
    pub fn with_public_id(mut self, public_id: PublicId) -> Self {
        self.public_id = Some(public_id);
        self
    }

    #[must_use]
    pub fn with_parent(mut self, parent: LayerId) -> Self {
        self.parent = Some(parent);
        self
    }

    #[must_use]
    pub fn with_content(mut self, content: LayerContent) -> Self {
        self.content = content;
        self
    }

    #[must_use]
    pub fn with_visibility(mut self, visibility: LayerVisibility) -> Self {
        self.visibility = visibility;
        self
    }

    #[must_use]
    pub const fn with_transform(mut self, transform: LayerTransform) -> Self {
        self.transform = transform;
        self
    }

    #[must_use]
    pub fn with_input_policy(mut self, input: LayerInputPolicy) -> Self {
        self.input = input;
        self
    }

    pub const fn id(&self) -> &LayerId {
        &self.id
    }

    pub const fn public_id(&self) -> Option<&PublicId> {
        self.public_id.as_ref()
    }

    pub const fn kind(&self) -> LayerKind {
        self.kind
    }

    pub const fn content(&self) -> &LayerContent {
        &self.content
    }

    pub const fn parent(&self) -> Option<&LayerId> {
        self.parent.as_ref()
    }

    pub fn children(&self) -> &[LayerId] {
        &self.children
    }

    pub const fn order(&self) -> LayerOrder {
        self.order
    }

    pub const fn transform(&self) -> LayerTransform {
        self.transform
    }

    pub const fn visibility(&self) -> LayerVisibility {
        self.visibility
    }

    pub const fn input_policy(&self) -> LayerInputPolicy {
        self.input
    }
}

impl LayerTransform {
    pub const fn identity() -> Self {
        Self {
            m11_milli: 1_000,
            m12_milli: 0,
            m21_milli: 0,
            m22_milli: 1_000,
            tx_milli: 0,
            ty_milli: 0,
        }
    }

    pub const fn translation_milli(
        horizontal_offset_milli: i32,
        vertical_offset_milli: i32,
    ) -> Self {
        Self {
            tx_milli: horizontal_offset_milli,
            ty_milli: vertical_offset_milli,
            ..Self::identity()
        }
    }

    pub const fn scale_milli(horizontal_scale_milli: i32, vertical_scale_milli: i32) -> Self {
        Self {
            m11_milli: horizontal_scale_milli,
            m12_milli: 0,
            m21_milli: 0,
            m22_milli: vertical_scale_milli,
            tx_milli: 0,
            ty_milli: 0,
        }
    }

    pub fn viewport_to_local(self, x: f32, y: f32) -> Option<LayerPoint> {
        let m11 = milli_to_f64(self.m11_milli);
        let m12 = milli_to_f64(self.m12_milli);
        let m21 = milli_to_f64(self.m21_milli);
        let m22 = milli_to_f64(self.m22_milli);
        let det = (m11 * m22) - (m12 * m21);
        if det.abs() <= f64::EPSILON {
            return None;
        }

        let dx = f64::from(x) - milli_to_f64(self.tx_milli);
        let dy = f64::from(y) - milli_to_f64(self.ty_milli);
        Some(LayerPoint {
            x: ((m22 * dx) - (m12 * dy)) / det,
            y: ((-m21 * dx) + (m11 * dy)) / det,
        })
    }
}

fn milli_to_f64(value: i32) -> f64 {
    f64::from(value) / 1_000.0
}
