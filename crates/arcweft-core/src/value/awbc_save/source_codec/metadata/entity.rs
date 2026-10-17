//! Runtime addresses retain their original segments and independent public label.

use super::super::{Reader, RuntimeLiteralSourceError as Error, Writer};
use crate::plan::{FlowRuntimeId, RuntimeLineId};
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath, RuntimeIdSegment};
use crate::value::imported_entity::ImportedProjectTarget;
use crate::value::{
    RuntimeEntityIdentityVisitor, RuntimeEntityReference, RuntimeImportedProjectEntityReference,
};
use arcweft_character::id::{CharacterId, CharacterLookId};
use arcweft_id::{DeclarationIdentityFamily, ProjectEntityReferenceFamily, PublicId};

impl Writer<'_> {
    fn path(&mut self, family: RuntimeIdFamily, path: &RuntimeIdPath) -> Result<(), Error> {
        if path.segments().is_empty() {
            return Err(Error::RuntimePath {
                offset: self.length,
                source: crate::runtime_id::RuntimeIdError::Empty { family },
            });
        }
        self.counts
            .string_length(path.canonical_len().ok_or(Error::Arithmetic)?)?;
        self.counts
            .array::<RuntimeIdSegment>(path.segments().len())?;
        self.count(path.segments().len())?;
        for (index, segment) in path.segments().iter().enumerate() {
            RuntimeIdPath::preflight_stored_source_segment(
                family,
                index,
                path.segments().len(),
                segment.as_str(),
            )
            .map_err(|source| Error::RuntimePath {
                offset: self.length,
                source,
            })?;
            self.string(segment.as_str())?;
        }
        Ok(())
    }

    pub(in crate::value::awbc_save::source_codec) fn entity(
        &mut self,
        value: &RuntimeEntityReference,
    ) -> Result<(), Error> {
        match value {
            RuntimeEntityReference::Project { family, public_id } => {
                family
                    .validate_public_id(public_id)
                    .map_err(|source| Error::PublicFamily {
                        offset: self.length,
                        source,
                    })?;
            }
            RuntimeEntityReference::CharacterLook { character, .. } => {
                // CharacterId's existing owner temporarily validates its public
                // ID/local spelling through one additional owned string.
                self.counts.scratch(character.as_str().len())?;
            }
            RuntimeEntityReference::StructuralFlow(_)
            | RuntimeEntityReference::DialogueLine(_)
            | RuntimeEntityReference::ImportedProject(_) => {}
        }
        value.visit_identity(self)
    }
}

impl RuntimeEntityIdentityVisitor for Writer<'_> {
    type Error = Error;

    fn tag(&mut self, tag: u8) -> Result<(), Self::Error> {
        Writer::tag(self, tag)
    }

    fn text(&mut self, text: &str) -> Result<(), Self::Error> {
        self.string(text)
    }

    fn digest(&mut self, digest: &[u8; 32]) -> Result<(), Self::Error> {
        self.bytes(digest)
    }

    fn path(&mut self, family: RuntimeIdFamily, path: &RuntimeIdPath) -> Result<(), Self::Error> {
        Writer::path(self, family, path)
    }
}

impl Reader<'_, '_> {
    fn path(
        &mut self,
        materialize: bool,
        family: RuntimeIdFamily,
    ) -> Result<Option<RuntimeIdPath>, Error> {
        let offset = self.offset;
        let count = self.count()?;
        if count == 0 {
            return Err(Error::RuntimePath {
                offset,
                source: crate::runtime_id::RuntimeIdError::Empty { family },
            });
        }
        self.counts.array::<RuntimeIdSegment>(count)?;
        let mut segments = materialize.then(|| Vec::with_capacity(count));
        let mut canonical_length = count - 1;
        for index in 0..count {
            let offset = self.offset;
            let text = self.string()?;
            canonical_length = canonical_length
                .checked_add(text.len())
                .ok_or(Error::Arithmetic)?;
            let source = RuntimeIdPath::preflight_stored_source_segment(family, index, count, text)
                .map_err(|source| Error::RuntimePath { offset, source })?;
            if let Some(segments) = &mut segments {
                segments.push(source.materialize());
            }
        }
        self.counts.string_length(canonical_length)?;
        segments
            .map(|segments| {
                RuntimeIdPath::from_source_segment_storage(family, segments)
                    .map_err(|source| Error::RuntimePath { offset, source })
            })
            .transpose()
    }

    fn flow(&mut self, materialize: bool) -> Result<Option<FlowRuntimeId>, Error> {
        let path = self.path(materialize, RuntimeIdFamily::Flow)?;
        let label = self.string()?;
        Ok(path.map(|path| FlowRuntimeId::from_source_path(path, label.to_owned())))
    }

    pub(in crate::value::awbc_save::source_codec) fn entity(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeEntityReference>, Error> {
        let offset = self.offset;
        match self.tag()? {
            0 => {
                let offset = self.offset;
                let tag = self.tag()?;
                let family =
                    DeclarationIdentityFamily::from_semantic_tag(tag).ok_or(Error::Tag {
                        kind: "entity family",
                        tag,
                        offset,
                    })?;
                let offset = self.offset;
                let text = self.string()?;
                if materialize {
                    let id = PublicId::try_new_engine_owned(text)
                        .map_err(|source| Error::PublicId { offset, source })?;
                    RuntimeEntityReference::try_project(family, id)
                        .map(Some)
                        .map_err(|source| Error::PublicFamily { offset, source })
                } else {
                    Ok(None)
                }
            }
            1 => Ok(self.path(materialize, RuntimeIdFamily::Line)?.map(|path| {
                RuntimeEntityReference::DialogueLine(RuntimeLineId::from_source_path(path))
            })),
            2 => {
                let offset = self.offset;
                let character = self.string()?;
                self.counts.scratch(character.len())?;
                let look_offset = self.offset;
                let look = self.string()?;
                if materialize {
                    let character = CharacterId::try_new(character)
                        .map_err(|source| Error::Character { offset, source })?;
                    let look =
                        CharacterLookId::try_new(look).map_err(|source| Error::Character {
                            offset: look_offset,
                            source,
                        })?;
                    Ok(Some(RuntimeEntityReference::CharacterLook {
                        character,
                        look,
                    }))
                } else {
                    Ok(None)
                }
            }
            3 => {
                let family_offset = self.offset;
                let tag = self.tag()?;
                let family =
                    ProjectEntityReferenceFamily::from_semantic_tag(tag).ok_or(Error::Tag {
                        kind: "imported entity family",
                        tag,
                        offset: family_offset,
                    })?;
                let public_offset = self.offset;
                let text = self.string()?;
                let generation = self.digest()?;
                let semantic = self.digest()?;
                let value_type = self.digest()?;
                let target_offset = self.offset;
                let target = match self.tag()? {
                    0 => materialize.then_some(ImportedProjectTarget::Public),
                    1 => self.flow(materialize)?.map(ImportedProjectTarget::Flow),
                    2 => self.path(materialize, RuntimeIdFamily::Line)?.map(|path| {
                        ImportedProjectTarget::Line(RuntimeLineId::from_source_path(path))
                    }),
                    tag => {
                        return Err(Error::Tag {
                            kind: "imported entity target",
                            tag,
                            offset: target_offset,
                        });
                    }
                };
                if materialize {
                    let public_id =
                        PublicId::try_new_engine_owned(text).map_err(|source| Error::PublicId {
                            offset: public_offset,
                            source,
                        })?;
                    RuntimeImportedProjectEntityReference::from_source_parts(
                        family,
                        public_id,
                        generation,
                        semantic,
                        value_type,
                        target.expect("retained exact imported target"),
                    )
                    .map(RuntimeEntityReference::ImportedProject)
                    .map(Some)
                    .map_err(|source| Error::Imported { offset, source })
                } else {
                    Ok(None)
                }
            }
            4 => Ok(self
                .flow(materialize)?
                .map(RuntimeEntityReference::StructuralFlow)),
            tag => Err(Error::Tag {
                kind: "entity reference",
                tag,
                offset,
            }),
        }
    }
}
