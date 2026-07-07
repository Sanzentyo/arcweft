use arcweft_id::PublicId;
use arcweft_view::{
    EventBinding, FragmentKind, FragmentNode, HandlerId, NodeId, NodeKey, SemanticSpecId, StyleId,
    ViewId, ViewPartId, ViewProgramId,
};
use std::collections::BTreeMap;

const ATTR_NODE: &str = "data-aw-node";
const ATTR_KEY: &str = "data-aw-key";
const ATTR_KIND: &str = "data-aw-kind";
const ATTR_STYLE: &str = "data-aw-style";
const ATTR_VIEW: &str = "data-aw-view";
const ATTR_PROGRAM: &str = "data-aw-program";
const ATTR_PART: &str = "data-aw-part";
const ATTR_SEMANTIC: &str = "data-aw-semantic";
const ATTR_HANDLERS: &str = "data-aw-handlers";
const ATTR_AGENT: &str = "data-aw-agent";
const ATTR_PATH: &str = "data-aw-takumi-path";

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TakumiPath(Vec<usize>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcweftNodeMetadata {
    node: NodeId,
    key: NodeKey,
    kind: FragmentKind,
    style: StyleId,
    view: Option<ViewId>,
    program: Option<ViewProgramId>,
    part: Option<ViewPartId>,
    semantic: Option<SemanticSpecId>,
    handlers: Vec<HandlerId>,
    agent: Option<PublicId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TakumiMetadataEntry {
    path: TakumiPath,
    metadata: ArcweftNodeMetadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TakumiMetadataMap {
    entries: Vec<TakumiMetadataEntry>,
}

impl TakumiPath {
    pub fn root() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub fn child(&self, index: usize) -> Self {
        let mut path = self.0.clone();
        path.push(index);
        Self(path)
    }

    pub fn as_slice(&self) -> &[usize] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<usize> {
        self.0
    }

    pub fn from_attribute(value: &str) -> Option<Self> {
        if value.is_empty() {
            return Some(Self::root());
        }
        value
            .split('.')
            .map(str::parse::<usize>)
            .collect::<Result<Vec<_>, _>>()
            .ok()
            .map(Self)
    }

    pub fn to_attribute(&self) -> String {
        self.0
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl From<Vec<usize>> for TakumiPath {
    fn from(value: Vec<usize>) -> Self {
        Self(value)
    }
}

impl ArcweftNodeMetadata {
    pub fn new(
        node: NodeId,
        key: NodeKey,
        kind: FragmentKind,
        style: StyleId,
        handlers: impl Into<Vec<HandlerId>>,
        semantic: Option<SemanticSpecId>,
    ) -> Self {
        Self {
            node,
            key,
            kind,
            style,
            view: None,
            program: None,
            part: None,
            semantic,
            handlers: handlers.into(),
            agent: None,
        }
    }

    pub fn from_fragment_node(
        node: NodeId,
        fragment_node: &FragmentNode,
        events: &[EventBinding],
    ) -> Self {
        Self::new(
            node,
            fragment_node.key(),
            fragment_node.kind(),
            fragment_node.style(),
            events
                .iter()
                .map(|event| event.handler())
                .collect::<Vec<_>>(),
            fragment_node.semantics(),
        )
    }

    pub fn node(&self) -> NodeId {
        self.node
    }

    pub fn key(&self) -> NodeKey {
        self.key
    }

    pub fn kind(&self) -> FragmentKind {
        self.kind
    }

    pub fn style(&self) -> StyleId {
        self.style
    }

    pub fn view(&self) -> Option<ViewId> {
        self.view
    }

    pub fn program(&self) -> Option<ViewProgramId> {
        self.program
    }

    pub fn part(&self) -> Option<ViewPartId> {
        self.part
    }

    pub fn semantic(&self) -> Option<SemanticSpecId> {
        self.semantic
    }

    pub fn handlers(&self) -> &[HandlerId] {
        &self.handlers
    }

    pub fn agent(&self) -> Option<&PublicId> {
        self.agent.as_ref()
    }

    #[must_use]
    pub fn with_view(mut self, view: ViewId) -> Self {
        self.view = Some(view);
        self
    }

    #[must_use]
    pub fn with_program(mut self, program: ViewProgramId) -> Self {
        self.program = Some(program);
        self
    }

    #[must_use]
    pub fn with_part(mut self, part: ViewPartId) -> Self {
        self.part = Some(part);
        self
    }

    #[must_use]
    pub fn with_agent(mut self, agent: PublicId) -> Self {
        self.agent = Some(agent);
        self
    }

    pub fn attributes(&self, path: &TakumiPath) -> BTreeMap<Box<str>, Box<str>> {
        let mut attributes = BTreeMap::new();
        insert_attr(&mut attributes, ATTR_NODE, self.node.0.to_string());
        insert_attr(&mut attributes, ATTR_KEY, self.key.0.to_string());
        insert_attr(&mut attributes, ATTR_KIND, self.kind_attribute());
        insert_attr(&mut attributes, ATTR_STYLE, self.style.0.to_string());
        insert_attr(&mut attributes, ATTR_PATH, path.to_attribute());
        if let Some(view) = self.view {
            insert_attr(&mut attributes, ATTR_VIEW, view.0.to_string());
        }
        if let Some(program) = self.program {
            insert_attr(&mut attributes, ATTR_PROGRAM, program.0.to_string());
        }
        if let Some(part) = self.part {
            insert_attr(&mut attributes, ATTR_PART, part.0.to_string());
        }
        if let Some(semantic) = self.semantic {
            insert_attr(&mut attributes, ATTR_SEMANTIC, semantic.0.to_string());
        }
        if !self.handlers.is_empty() {
            insert_attr(
                &mut attributes,
                ATTR_HANDLERS,
                self.handlers
                    .iter()
                    .map(|handler| handler.0.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        if let Some(agent) = &self.agent {
            insert_attr(&mut attributes, ATTR_AGENT, agent.as_str());
        }
        attributes
    }

    fn kind_attribute(&self) -> String {
        match self.kind {
            FragmentKind::Container(kind) => format!("container.{kind:?}"),
            FragmentKind::Text(id) => format!("text.{}", id.0),
            FragmentKind::RichText(id) => format!("rich_text.{}", id.0),
            FragmentKind::Image(id) => format!("image.{}", id.0),
            FragmentKind::View(entity) => format!("view.{entity:?}"),
            FragmentKind::Custom(id) => format!("custom.{}", id.0),
        }
    }
}

impl TakumiMetadataEntry {
    pub fn new(path: TakumiPath, metadata: ArcweftNodeMetadata) -> Self {
        Self { path, metadata }
    }

    pub fn path(&self) -> &TakumiPath {
        &self.path
    }

    pub fn metadata(&self) -> &ArcweftNodeMetadata {
        &self.metadata
    }
}

impl TakumiMetadataMap {
    pub fn push(&mut self, path: TakumiPath, metadata: ArcweftNodeMetadata) {
        self.entries.push(TakumiMetadataEntry::new(path, metadata));
    }

    pub fn entries(&self) -> &[TakumiMetadataEntry] {
        &self.entries
    }

    pub fn get_by_path(&self, path: &TakumiPath) -> Option<&ArcweftNodeMetadata> {
        self.entries
            .iter()
            .find(|entry| entry.path() == path)
            .map(TakumiMetadataEntry::metadata)
    }

    pub fn get_by_node(&self, node: NodeId) -> Option<&ArcweftNodeMetadata> {
        self.entries
            .iter()
            .find(|entry| entry.metadata().node() == node)
            .map(TakumiMetadataEntry::metadata)
    }
}

fn insert_attr(attributes: &mut BTreeMap<Box<str>, Box<str>>, name: &str, value: impl AsRef<str>) {
    attributes.insert(name.into(), value.as_ref().into());
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::PublicId;
    use arcweft_view::{ContainerKind, FragmentKind, HandlerId, NodeKey, StyleId};

    #[test]
    fn metadata_attributes_preserve_arcweft_identity() {
        let metadata = ArcweftNodeMetadata::new(
            NodeId(2),
            NodeKey(7),
            FragmentKind::Container(ContainerKind::Stack),
            StyleId(3),
            [HandlerId(11)],
            Some(SemanticSpecId(13)),
        )
        .with_view(ViewId(17))
        .with_program(ViewProgramId(19))
        .with_part(ViewPartId(23))
        .with_agent(PublicId::try_new("agent.dialogue").expect("valid agent id"));

        let path = TakumiPath::root().child(4).child(1);
        let attrs = metadata.attributes(&path);

        assert_eq!(attrs.get(ATTR_NODE).map(Box::as_ref), Some("2"));
        assert_eq!(attrs.get(ATTR_KEY).map(Box::as_ref), Some("7"));
        assert_eq!(attrs.get(ATTR_VIEW).map(Box::as_ref), Some("17"));
        assert_eq!(attrs.get(ATTR_PROGRAM).map(Box::as_ref), Some("19"));
        assert_eq!(attrs.get(ATTR_PART).map(Box::as_ref), Some("23"));
        assert_eq!(attrs.get(ATTR_SEMANTIC).map(Box::as_ref), Some("13"));
        assert_eq!(attrs.get(ATTR_HANDLERS).map(Box::as_ref), Some("11"));
        assert_eq!(
            attrs.get(ATTR_AGENT).map(Box::as_ref),
            Some("agent.dialogue")
        );
        assert_eq!(attrs.get(ATTR_PATH).map(Box::as_ref), Some("4.1"));
        assert_eq!(TakumiPath::from_attribute("4.1"), Some(path));
    }
}
