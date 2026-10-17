//! Complete borrowed entity fields shared by persistent, executable and source
//! encoders. Text labels never replace stored runtime-path segments.

use super::RuntimeEntityReference;
use super::imported_entity::ImportedProjectTarget;
use crate::plan::FlowRuntimeId;
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError};

/// An encoding context for the entity owner's complete field grammar. Each
/// context retains its own byte format, counting and first-error state.
pub(crate) trait RuntimeEntityIdentityVisitor {
    type Error;

    fn tag(&mut self, tag: u8) -> Result<(), Self::Error>;
    fn text(&mut self, text: &str) -> Result<(), Self::Error>;
    fn digest(&mut self, digest: &[u8; 32]) -> Result<(), Self::Error>;
    /// Encode the original segment count and strings, preserving boundaries.
    fn path(&mut self, family: RuntimeIdFamily, path: &RuntimeIdPath) -> Result<(), Self::Error>;
}

impl RuntimeEntityReference {
    pub(crate) fn visit_identity<V: RuntimeEntityIdentityVisitor>(
        &self,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        match self {
            Self::Project { family, public_id } => {
                visitor.tag(0)?;
                visitor.tag(family.semantic_tag())?;
                visitor.text(public_id.as_str())
            }
            Self::DialogueLine(line) => {
                visitor.tag(1)?;
                visitor.path(RuntimeIdFamily::Line, line.path())
            }
            Self::CharacterLook { character, look } => {
                visitor.tag(2)?;
                visitor.text(character.as_str())?;
                visitor.text(look.as_str())
            }
            Self::ImportedProject(value) => {
                visitor.tag(3)?;
                visitor.tag(value.family().semantic_tag())?;
                visitor.text(value.public_id().as_str())?;
                visitor.digest(value.target_generation())?;
                visitor.digest(value.semantic_identity())?;
                visitor.digest(value.value_type())?;
                match value.source_target() {
                    ImportedProjectTarget::Public => visitor.tag(0),
                    ImportedProjectTarget::Flow(flow) => {
                        visitor.tag(1)?;
                        flow.visit_reference_identity(visitor)
                    }
                    ImportedProjectTarget::Line(line) => {
                        visitor.tag(2)?;
                        visitor.path(RuntimeIdFamily::Line, line.path())
                    }
                }
            }
            Self::StructuralFlow(flow) => {
                visitor.tag(4)?;
                flow.visit_reference_identity(visitor)
            }
        }
    }

    pub(crate) fn encode_body_identity(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
    ) -> Result<(), TaskSemanticEncodingError> {
        self.visit_identity(encoder)
    }
}

impl FlowRuntimeId {
    fn visit_reference_identity<V: RuntimeEntityIdentityVisitor>(
        &self,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        visitor.path(RuntimeIdFamily::Flow, self.path())?;
        visitor.text(self.public_label_ref().as_str())
    }
}

impl RuntimeEntityIdentityVisitor for TaskSemanticEncoder<'_> {
    type Error = TaskSemanticEncodingError;

    fn tag(&mut self, tag: u8) -> Result<(), Self::Error> {
        self.status()?;
        TaskSemanticEncoder::tag(self, tag);
        self.status()
    }

    fn text(&mut self, text: &str) -> Result<(), Self::Error> {
        self.status()?;
        self.string(text);
        self.status()
    }

    fn digest(&mut self, digest: &[u8; 32]) -> Result<(), Self::Error> {
        self.status()?;
        TaskSemanticEncoder::digest(self, digest);
        self.status()
    }

    fn path(&mut self, _family: RuntimeIdFamily, path: &RuntimeIdPath) -> Result<(), Self::Error> {
        self.status()?;
        self.count(path.segments().len());
        self.status()?;
        for segment in path.segments() {
            self.enter_element();
            self.text(segment.as_str())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
