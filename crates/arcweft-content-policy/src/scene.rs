use crate::raster::{MaskStyle, ObjectIdBuffer, PixelMask, RgbaImage};
use crate::types::{ContentDigest, ContentId, FindingTarget, ObjectId, PolicyError, PolicyFinding};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Why one scene view was rendered.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderSampleKind {
    Canonical,
    Animation,
    Lod,
    MaterialVariant,
    ObjectFocused,
}

impl RenderSampleKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Canonical => "canonical",
            Self::Animation => "animation",
            Self::Lod => "lod",
            Self::MaterialVariant => "material_variant",
            Self::ObjectFocused => "object_focused",
        }
    }
}

/// One trusted-host render with a color image and aligned object-id attachment.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RenderedView {
    pub id: String,
    pub sample_kind: RenderSampleKind,
    pub sample_index: u32,
    pub color: RgbaImage,
    pub object_ids: Option<ObjectIdBuffer>,
}

impl RenderedView {
    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.object_ids.as_ref().is_none_or(|object_ids| {
            object_ids.width() == self.color.width() && object_ids.height() == self.color.height()
        }) {
            Ok(())
        } else {
            Err(PolicyError::InvalidSceneObjectIds {
                view_id: self.id.clone(),
            })
        }
    }
}

/// Explicit coverage contract for a rendered model or scene.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RenderCoverage {
    pub required_canonical_views: u32,
    pub observed_canonical_views: u32,
    pub required_animation_samples: u32,
    pub observed_animation_samples: u32,
    pub required_lod_samples: u32,
    pub observed_lod_samples: u32,
}

impl RenderCoverage {
    pub const fn is_sufficient(&self) -> bool {
        self.observed_canonical_views >= self.required_canonical_views
            && self.observed_animation_samples >= self.required_animation_samples
            && self.observed_lod_samples >= self.required_lod_samples
    }
}

/// Multi-view representation used to classify a 3D model or composed scene.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RenderedScene {
    pub id: ContentId,
    pub views: Vec<RenderedView>,
    pub coverage: RenderCoverage,
    #[serde(default)]
    pub object_names: BTreeMap<ObjectId, String>,
}

impl RenderedScene {
    pub fn new(
        id: ContentId,
        views: Vec<RenderedView>,
        coverage: RenderCoverage,
    ) -> Result<Self, PolicyError> {
        if views.is_empty() {
            return Err(PolicyError::EmptyRenderedScene);
        }
        views.iter().try_for_each(RenderedView::validate)?;
        Ok(Self {
            id,
            views,
            coverage,
            object_names: BTreeMap::new(),
        })
    }

    pub fn content_digest(&self) -> ContentDigest {
        let mut hasher = blake3::Hasher::new();
        hash_field(&mut hasher, b"arcweft.rendered-scene.v1");
        hash_field(&mut hasher, self.id.as_str().as_bytes());
        for value in [
            self.coverage.required_canonical_views,
            self.coverage.observed_canonical_views,
            self.coverage.required_animation_samples,
            self.coverage.observed_animation_samples,
            self.coverage.required_lod_samples,
            self.coverage.observed_lod_samples,
        ] {
            hash_field(&mut hasher, &value.to_le_bytes());
        }
        hash_field(&mut hasher, b"views");
        hash_count(&mut hasher, self.views.len());
        for view in &self.views {
            hash_field(&mut hasher, view.id.as_bytes());
            hash_field(&mut hasher, view.sample_kind.as_str().as_bytes());
            hash_field(&mut hasher, &view.sample_index.to_le_bytes());
            hash_field(&mut hasher, view.color.content_digest().as_str().as_bytes());
            match &view.object_ids {
                Some(object_ids) => {
                    hash_field(&mut hasher, b"object-ids");
                    hash_count(&mut hasher, object_ids.values().len());
                    for id in object_ids.values() {
                        hash_field(&mut hasher, &id.0.to_le_bytes());
                    }
                }
                None => hash_field(&mut hasher, b"no-object-ids"),
            }
        }
        hash_field(&mut hasher, b"object-names");
        hash_count(&mut hasher, self.object_names.len());
        for (id, name) in &self.object_names {
            hash_field(&mut hasher, &id.0.to_le_bytes());
            hash_field(&mut hasher, name.as_bytes());
        }
        ContentDigest::from_hasher(&hasher)
    }

    pub fn sanitized(
        &self,
        findings: &[PolicyFinding],
        style: MaskStyle,
        whole_if_unlocalized: bool,
    ) -> Result<Self, PolicyError> {
        let suppressed_objects = findings
            .iter()
            .filter_map(|finding| match &finding.target {
                FindingTarget::ObjectIds { ids } => Some(ids.iter().copied()),
                _ => None,
            })
            .flatten()
            .collect::<BTreeSet<_>>();
        let views = self
            .views
            .iter()
            .map(|view| {
                let relevant = findings
                    .iter()
                    .filter(|finding| match &finding.target {
                        FindingTarget::SceneViewRect { view_id, .. }
                        | FindingTarget::SceneViewMask { view_id, .. } => view_id == &view.id,
                        FindingTarget::Whole | FindingTarget::ObjectIds { .. } => true,
                        FindingTarget::Text { .. }
                        | FindingTarget::ImageRect { .. }
                        | FindingTarget::ImageMask { .. } => false,
                    })
                    .collect::<Vec<_>>();
                let mut mask = PixelMask::empty(view.color.width(), view.color.height())?;
                for finding in relevant {
                    match &finding.target {
                        FindingTarget::Whole => {
                            mask.fill();
                        }
                        FindingTarget::SceneViewRect { rect, .. } => {
                            mask.set_rect(*rect);
                        }
                        FindingTarget::SceneViewMask {
                            mask: finding_mask, ..
                        } => {
                            mask.union_assign(finding_mask)?;
                        }
                        FindingTarget::ObjectIds { ids } => {
                            if let Some(object_ids) = &view.object_ids {
                                mask.union_assign(&object_ids.mask_for(ids)?)?;
                            } else if whole_if_unlocalized {
                                mask.fill();
                            } else {
                                return Err(PolicyError::ObjectIdDimensionMismatch);
                            }
                        }
                        FindingTarget::Text { .. }
                        | FindingTarget::ImageRect { .. }
                        | FindingTarget::ImageMask { .. } => {}
                    }
                }
                Ok(RenderedView {
                    id: view.id.clone(),
                    sample_kind: view.sample_kind,
                    sample_index: view.sample_index,
                    color: view.color.masked(&mask, style)?,
                    object_ids: view.object_ids.clone(),
                })
            })
            .collect::<Result<Vec<_>, PolicyError>>()?;
        Ok(Self {
            id: self.id.clone(),
            views,
            coverage: self.coverage.clone(),
            object_names: self
                .object_names
                .iter()
                .filter(|(id, _)| !suppressed_objects.contains(id))
                .map(|(id, name)| (*id, name.clone()))
                .collect(),
        })
    }
}

fn hash_count(hasher: &mut blake3::Hasher, value: usize) {
    let value = u64::try_from(value).unwrap_or(u64::MAX);
    hasher.update(&value.to_le_bytes());
}

fn hash_field(hasher: &mut blake3::Hasher, value: &[u8]) {
    hash_count(hasher, value.len());
    hasher.update(value);
}
