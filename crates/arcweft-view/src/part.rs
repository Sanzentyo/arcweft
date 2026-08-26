//! Canonical private/public View-part identities and static inventory contracts.

use crate::{EventKind, HandlerId, ViewHandlerProgramId};
use arcweft_id::{IdError, IdErrorKind, PublicId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use thiserror::Error;

/// Maximum UTF-8 length of one private or public View-part name.
pub const MAX_VIEW_PART_NAME_BYTES: usize = 255;
/// Maximum dotted segment count of one private or public View-part name.
pub const MAX_VIEW_PART_NAME_SEGMENTS: usize = 32;

/// Private implementation name declared by `.part(name)` inside one View.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPartLocalName(PublicId);

/// Public capability name used by caller-side Style selectors.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPartName(PublicId);

/// Compact owner-local identity allocated in canonical local-name order.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ViewPartId(u32);

/// Checked index of one instruction in an immutable View program.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ViewInstructionIndex(u32);

/// Node-producing instruction family that can own a local part target.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewPartInstructionKind {
    OpenElement,
    EmitText,
    EmitImage,
    EmitCustom,
    CallView,
}

/// Compile-time reachability of one static target.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewPartStaticReachability {
    Reachable,
    StaticallyUnreachable,
}

/// Stable semantic evaluation site, independent of source offsets and instruction ordinals.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewEvaluationSiteId([u8; 32]);

/// One checked private local target in an immutable View program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewStaticPart {
    id: ViewPartId,
    local_name: ViewPartLocalName,
    instruction: ViewInstructionIndex,
    kind: ViewPartInstructionKind,
    reachability: ViewPartStaticReachability,
    site: ViewEvaluationSiteId,
}

/// One public export joined to a checked static target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartExport {
    part: ViewPartId,
    public_name: ViewPartName,
}

/// Invalid private/public View-part name.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewPartNameError {
    #[error("View part name is empty")]
    Empty,
    #[error("View part name has {length} UTF-8 bytes; limit is {limit}")]
    TooLong { length: usize, limit: usize },
    #[error("View part name has {segments} segments; limit is {limit}")]
    TooManySegments { segments: usize, limit: usize },
    #[error("View part name segment {segment} is empty")]
    EmptySegment { segment: usize },
    #[error("View part name contains qualified separator at byte {byte}")]
    QualifiedSeparator { byte: usize },
    #[error("invalid start of View part segment {segment} at byte {byte}")]
    InvalidSegmentStart {
        segment: usize,
        byte: usize,
        found: char,
    },
    #[error("invalid View part character in segment {segment} at byte {byte}")]
    InvalidSegmentCharacter {
        segment: usize,
        byte: usize,
        found: char,
    },
    #[error("reserved View part prefix `{prefix}`")]
    ReservedPrefix { prefix: String },
    #[error(transparent)]
    PublicId(#[from] IdError),
}

/// A host index cannot be represented as a compact part identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("View part index {index} exceeds u32::MAX")]
pub struct ViewPartIdOverflow {
    pub index: usize,
}

/// A host index cannot be represented as a compact instruction identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("View instruction index {index} exceeds u32::MAX")]
pub struct ViewInstructionIndexOverflow {
    pub index: usize,
}

/// Failure to construct one internally consistent immutable View program.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewProgramBuildError {
    #[error("View instruction count {length} cannot be represented")]
    InstructionIndexOverflow { length: usize },
    #[error("View part count {count} cannot be represented")]
    PartIdOverflow { count: usize },
    #[error("unknown View instruction {instruction:?}")]
    UnknownInstruction { instruction: ViewInstructionIndex },
    #[error("instruction {instruction:?} cannot own a View part")]
    UnsupportedInstruction { instruction: ViewInstructionIndex },
    #[error("instruction {instruction:?} has {actual:?}, expected {expected:?}")]
    InstructionKindMismatch {
        instruction: ViewInstructionIndex,
        expected: ViewPartInstructionKind,
        actual: ViewPartInstructionKind,
    },
    #[error("local View part names are not canonical")]
    NonCanonicalLocalOrder {
        previous: ViewPartLocalName,
        next: ViewPartLocalName,
    },
    #[error("exported View part names are not canonical")]
    NonCanonicalExportOrder {
        previous: ViewPartName,
        next: ViewPartName,
    },
    #[error("duplicate local View part `{name}`")]
    DuplicateLocalName {
        name: ViewPartLocalName,
        first: ViewPartId,
        duplicate_instruction: ViewInstructionIndex,
    },
    #[error("instruction {instruction:?} already owns View part {first:?}")]
    DuplicateInstructionTarget {
        instruction: ViewInstructionIndex,
        first: ViewPartId,
    },
    #[error("unknown View part {part:?}")]
    UnknownPart { part: ViewPartId },
    #[error("View part {part:?} already exports `{existing}`")]
    TargetAlreadyExported {
        part: ViewPartId,
        existing: ViewPartName,
    },
    #[error("duplicate public View part `{name}`")]
    DuplicatePublicName {
        name: ViewPartName,
        first: ViewPartId,
        duplicate: ViewPartId,
    },
    #[error("CallView part {part:?} at {instruction:?} cannot be exported")]
    UnsupportedCallViewExport {
        part: ViewPartId,
        instruction: ViewInstructionIndex,
    },
    #[error("View part {part:?} targets stale instruction {instruction:?}")]
    StalePartTarget {
        part: ViewPartId,
        instruction: ViewInstructionIndex,
    },
    #[error("duplicate View evaluation site {site:?}")]
    DuplicateEvaluationSite {
        site: ViewEvaluationSiteId,
        first: ViewInstructionIndex,
        duplicate: ViewInstructionIndex,
    },
    #[error("View handler count {count} cannot be represented")]
    HandlerIdOverflow { count: usize },
    #[error("View handler row {actual:?} is not the expected dense identity {expected:?}")]
    NonCanonicalHandlerId {
        expected: HandlerId,
        actual: HandlerId,
    },
    #[error("duplicate View handler program {program:?}")]
    DuplicateHandlerProgram { program: ViewHandlerProgramId },
    #[error("View handler programs are not in canonical identity order")]
    NonCanonicalHandlerOrder {
        previous: ViewHandlerProgramId,
        next: ViewHandlerProgramId,
    },
    #[error("View handler {handler:?} captures are not in canonical coordinate order")]
    NonCanonicalHandlerCaptures { handler: HandlerId },
    #[error("View event at {instruction:?} references unknown handler {handler:?}")]
    UnknownHandler {
        instruction: ViewInstructionIndex,
        handler: HandlerId,
    },
    #[error("View handler {handler:?} is bound more than once")]
    DuplicateHandlerBinding { handler: HandlerId },
    #[error("View handler {handler:?} is never bound to an event")]
    UnboundHandler { handler: HandlerId },
    #[error("View event at {instruction:?} has no preceding node target")]
    HandlerWithoutTarget { instruction: ViewInstructionIndex },
    #[error("View node {target:?} binds duplicate {event:?} events")]
    DuplicateTargetEvent {
        target: ViewInstructionIndex,
        event: EventKind,
    },
}

impl ViewPartLocalName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ViewPartNameError> {
        let value = value.into();
        validate_name(&value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub const fn as_public_id(&self) -> &PublicId {
        &self.0
    }
}

impl fmt::Display for ViewPartLocalName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ViewPartName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ViewPartNameError> {
        let value = value.into();
        validate_name(&value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub const fn as_public_id(&self) -> &PublicId {
        &self.0
    }
}

impl fmt::Display for ViewPartName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ViewPartId {
    pub fn try_from_index(index: usize) -> Result<Self, ViewPartIdOverflow> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| ViewPartIdOverflow { index })
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ViewInstructionIndex {
    pub fn try_from_index(index: usize) -> Result<Self, ViewInstructionIndexOverflow> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| ViewInstructionIndexOverflow { index })
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ViewPartInstructionKind {
    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::OpenElement => 0,
            Self::EmitText => 1,
            Self::EmitImage => 2,
            Self::EmitCustom => 3,
            Self::CallView => 4,
        }
    }

    pub const fn is_exportable(self) -> bool {
        !matches!(self, Self::CallView)
    }
}

impl ViewEvaluationSiteId {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Derives a stable semantic site from its owner, local name, and instruction family.
    pub fn from_part(
        view: &crate::ViewId,
        local: &ViewPartLocalName,
        kind: ViewPartInstructionKind,
    ) -> Self {
        let mut transcript = Vec::with_capacity(view.as_str().len() + local.as_str().len() + 32);
        transcript.extend_from_slice(b"arcweft.view-part-site.v1\0");
        append_site_part(&mut transcript, view.as_str().as_bytes());
        append_site_part(&mut transcript, local.as_str().as_bytes());
        transcript.push(kind.wire_tag());
        Self(*blake3::hash(&transcript).as_bytes())
    }
}

fn append_site_part(transcript: &mut Vec<u8>, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("slice lengths fit the u64 hash transcript");
    transcript.extend_from_slice(&length.to_le_bytes());
    transcript.extend_from_slice(value);
}

impl ViewStaticPart {
    pub(crate) const fn new(
        id: ViewPartId,
        local_name: ViewPartLocalName,
        instruction: ViewInstructionIndex,
        kind: ViewPartInstructionKind,
        reachability: ViewPartStaticReachability,
        site: ViewEvaluationSiteId,
    ) -> Self {
        Self {
            id,
            local_name,
            instruction,
            kind,
            reachability,
            site,
        }
    }

    pub const fn id(&self) -> ViewPartId {
        self.id
    }

    pub const fn local_name(&self) -> &ViewPartLocalName {
        &self.local_name
    }

    pub const fn instruction(&self) -> ViewInstructionIndex {
        self.instruction
    }

    pub const fn kind(&self) -> ViewPartInstructionKind {
        self.kind
    }

    pub const fn reachability(&self) -> ViewPartStaticReachability {
        self.reachability
    }

    pub const fn site(&self) -> ViewEvaluationSiteId {
        self.site
    }
}

impl ViewPartExport {
    pub(crate) const fn new(part: ViewPartId, public_name: ViewPartName) -> Self {
        Self { part, public_name }
    }

    pub const fn part(&self) -> ViewPartId {
        self.part
    }

    pub const fn public_name(&self) -> &ViewPartName {
        &self.public_name
    }
}

fn validate_name(value: &str) -> Result<PublicId, ViewPartNameError> {
    if value.is_empty() {
        return Err(ViewPartNameError::Empty);
    }
    if value.len() > MAX_VIEW_PART_NAME_BYTES {
        return Err(ViewPartNameError::TooLong {
            length: value.len(),
            limit: MAX_VIEW_PART_NAME_BYTES,
        });
    }
    if let Some(byte) = value.find("::") {
        return Err(ViewPartNameError::QualifiedSeparator { byte });
    }
    let segments = value.split('.').collect::<Vec<_>>();
    if segments.len() > MAX_VIEW_PART_NAME_SEGMENTS {
        return Err(ViewPartNameError::TooManySegments {
            segments: segments.len(),
            limit: MAX_VIEW_PART_NAME_SEGMENTS,
        });
    }

    let mut segment_start = 0;
    for (segment, text) in segments.iter().enumerate() {
        if text.is_empty() {
            return Err(ViewPartNameError::EmptySegment { segment });
        }
        let mut characters = text.char_indices();
        let (first_offset, first) = characters.next().expect("nonempty segment");
        if first != '_' && !first.is_alphabetic() {
            return Err(ViewPartNameError::InvalidSegmentStart {
                segment,
                byte: segment_start + first_offset,
                found: first,
            });
        }
        if let Some((offset, found)) = characters.find(|(_, character)| {
            *character != '_' && *character != '-' && !character.is_alphanumeric()
        }) {
            return Err(ViewPartNameError::InvalidSegmentCharacter {
                segment,
                byte: segment_start + offset,
                found,
            });
        }
        segment_start += text.len() + 1;
    }

    PublicId::try_new(value.to_owned()).map_err(|error| {
        if error.kind() == IdErrorKind::ReservedPrefix {
            ViewPartNameError::ReservedPrefix {
                prefix: value.split('.').next().unwrap_or(value).to_owned(),
            }
        } else {
            ViewPartNameError::PublicId(error)
        }
    })
}

impl Serialize for ViewPartLocalName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ViewPartLocalName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ViewPartName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ViewPartName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_VIEW_PART_NAME_BYTES, MAX_VIEW_PART_NAME_SEGMENTS, ViewPartLocalName, ViewPartName,
        ViewPartNameError,
    };

    #[test]
    fn local_and_public_names_use_the_same_closed_grammar_but_distinct_types() {
        let local = ViewPartLocalName::try_new("panel.title-row").unwrap();
        let public = ViewPartName::try_new("panel.title-row").unwrap();
        assert_eq!(local.as_str(), public.as_str());
    }

    #[test]
    fn exact_name_limits_are_accepted_and_one_over_is_rejected() {
        let exact_bytes = format!("a{}", "x".repeat(MAX_VIEW_PART_NAME_BYTES - 1));
        assert!(ViewPartLocalName::try_new(exact_bytes).is_ok());
        assert!(matches!(
            ViewPartLocalName::try_new(format!("a{}", "x".repeat(MAX_VIEW_PART_NAME_BYTES))),
            Err(ViewPartNameError::TooLong { .. })
        ));

        let exact_segments = std::iter::repeat_n("a", MAX_VIEW_PART_NAME_SEGMENTS)
            .collect::<Vec<_>>()
            .join(".");
        assert!(ViewPartName::try_new(exact_segments).is_ok());
        let too_many = std::iter::repeat_n("a", MAX_VIEW_PART_NAME_SEGMENTS + 1)
            .collect::<Vec<_>>()
            .join(".");
        assert!(matches!(
            ViewPartName::try_new(too_many),
            Err(ViewPartNameError::TooManySegments { .. })
        ));
    }
}
