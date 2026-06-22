use crate::classifier::{ContentClassifier, PolicyInputRef};
use crate::raster::PixelMask;
use crate::types::{
    ClassificationReport, ClassifierIdentity, ClassifierRun, Completeness, FindingTarget, ObjectId,
    PixelRect, PolicyCategory, PolicyError, PolicyFinding, TextRange,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Input accepted by an embedded text/image model runtime.
#[derive(Clone, Copy, Debug)]
pub enum ModelInput<'a> {
    Text(&'a str),
    Rgba {
        width: u32,
        height: u32,
        pixels: &'a [u8],
    },
    RenderedView {
        view_id: &'a str,
        width: u32,
        height: u32,
        pixels: &'a [u8],
        object_ids: Option<&'a [ObjectId]>,
    },
}

/// Backend-neutral model detection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModelDetection {
    pub category: PolicyCategory,
    pub score_milli: u16,
    pub text_range: Option<TextRange>,
    pub rect: Option<PixelRect>,
    pub mask: Option<PixelMask>,
    #[serde(default)]
    pub object_ids: BTreeSet<ObjectId>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

impl ModelDetection {
    fn into_text_finding(self, text_len: usize) -> PolicyFinding {
        let target = self
            .text_range
            .filter(|range| range.end <= text_len && !range.is_empty())
            .map_or(FindingTarget::Whole, |range| FindingTarget::Text { range });
        PolicyFinding {
            category: self.category,
            score_milli: self.score_milli.min(1000),
            target,
            attributes: self.attributes,
        }
    }

    fn into_image_finding(self) -> PolicyFinding {
        let target = if !self.object_ids.is_empty() {
            FindingTarget::ObjectIds {
                ids: self.object_ids,
            }
        } else if let Some(mask) = self.mask {
            FindingTarget::ImageMask { mask }
        } else {
            self.rect
                .map_or(FindingTarget::Whole, |rect| FindingTarget::ImageRect {
                    rect,
                })
        };
        PolicyFinding {
            category: self.category,
            score_milli: self.score_milli.min(1000),
            target,
            attributes: self.attributes,
        }
    }

    fn into_scene_finding(self, view_id: &str) -> PolicyFinding {
        let target = if !self.object_ids.is_empty() {
            FindingTarget::ObjectIds {
                ids: self.object_ids,
            }
        } else if let Some(mask) = self.mask {
            FindingTarget::SceneViewMask {
                view_id: view_id.to_owned(),
                mask,
            }
        } else {
            self.rect
                .map_or(FindingTarget::Whole, |rect| FindingTarget::SceneViewRect {
                    view_id: view_id.to_owned(),
                    rect,
                })
        };
        PolicyFinding {
            category: self.category,
            score_milli: self.score_milli.min(1000),
            target,
            attributes: self.attributes,
        }
    }
}

/// Typed output returned by an embedded model runtime.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModelOutput {
    pub detections: Vec<ModelDetection>,
    pub completeness: Completeness,
    pub failure_code: Option<String>,
}

impl ModelOutput {
    pub fn complete(detections: Vec<ModelDetection>) -> Self {
        Self {
            detections,
            completeness: Completeness::Complete,
            failure_code: None,
        }
    }
}

/// Real model ABI. Implementations may use an in-process runtime or an isolated
/// local process, but raw content must not cross the configured trust boundary.
pub trait EmbeddedPolicyModel {
    fn identity(&self) -> ClassifierIdentity;

    fn infer(&self, input: ModelInput<'_>) -> Result<ModelOutput, PolicyError>;
}

/// Adapts a real embedded model into the common classifier interface.
#[derive(Clone, Debug)]
pub struct ModelClassifier<M> {
    model: M,
}

impl<M> ModelClassifier<M> {
    pub const fn new(model: M) -> Self {
        Self { model }
    }
}

impl<M> ContentClassifier for ModelClassifier<M>
where
    M: EmbeddedPolicyModel,
{
    fn identity(&self) -> ClassifierIdentity {
        self.model.identity()
    }

    fn classify(&self, input: PolicyInputRef<'_>) -> Result<ClassificationReport, PolicyError> {
        match input {
            PolicyInputRef::Text(text) => {
                let output = self.model.infer(ModelInput::Text(text))?;
                let run = model_run(self.model.identity(), &output);
                Ok(ClassificationReport {
                    findings: output
                        .detections
                        .into_iter()
                        .map(|detection| detection.into_text_finding(text.len()))
                        .collect(),
                    runs: vec![run],
                })
            }
            PolicyInputRef::Image(image) => {
                let output = self.model.infer(ModelInput::Rgba {
                    width: image.width(),
                    height: image.height(),
                    pixels: image.pixels(),
                })?;
                let run = model_run(self.model.identity(), &output);
                Ok(ClassificationReport {
                    findings: output
                        .detections
                        .into_iter()
                        .map(ModelDetection::into_image_finding)
                        .collect(),
                    runs: vec![run],
                })
            }
            PolicyInputRef::RenderedScene(scene) => {
                scene
                    .views
                    .iter()
                    .try_fold(ClassificationReport::default(), |report, view| {
                        let output = self.model.infer(ModelInput::RenderedView {
                            view_id: &view.id,
                            width: view.color.width(),
                            height: view.color.height(),
                            pixels: view.color.pixels(),
                            object_ids: view
                                .object_ids
                                .as_ref()
                                .map(super::raster::ObjectIdBuffer::values),
                        })?;
                        let run = model_run(self.model.identity(), &output);
                        let view_report = ClassificationReport {
                            findings: output
                                .detections
                                .into_iter()
                                .map(|detection| detection.into_scene_finding(&view.id))
                                .collect(),
                            runs: vec![run],
                        };
                        Ok(report.merge(view_report))
                    })
            }
        }
    }
}

fn model_run(identity: ClassifierIdentity, output: &ModelOutput) -> ClassifierRun {
    match output.completeness {
        Completeness::Complete => ClassifierRun::complete(identity),
        Completeness::NotApplicable => ClassifierRun::not_applicable(identity),
        completeness => ClassifierRun::incomplete(
            identity,
            completeness,
            output
                .failure_code
                .clone()
                .unwrap_or_else(|| "model_incomplete".to_owned()),
        ),
    }
}
