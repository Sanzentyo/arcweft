//! Sans I/O bundle data model and deterministic JSON codec.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const ARCWEFT_BUNDLE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArcweftBundle {
    pub schema_version: u32,
    pub manifest: BundleManifest,
    pub source: BundleSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub virtual_files: Vec<BundleVirtualFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleManifest {
    pub source_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_kind: Option<BundleLaunchKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapter_manifest_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_host_calls: Vec<String>,
    pub runtime: BundleRuntimeSummary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleSource {
    pub label: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleRuntimeSummary {
    pub entry_flow: Option<String>,
    pub flows: usize,
    pub bytecode_instructions: usize,
    pub line_task_groups: usize,
    pub stream_plans: usize,
    pub source_plans: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleVirtualFile {
    pub space: BundleVirtualFileSpace,
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleVirtualFileSpace {
    Asset,
    Save,
    Temp,
    Export,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleLaunchKind {
    Game,
    Cli,
    Server,
    Test,
    Bench,
}

#[derive(Debug, Error)]
pub enum BundleCodecError {
    #[error("unsupported Arcweft bundle schema version {actual}; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("failed to encode Arcweft bundle JSON: {0}")]
    Encode(#[source] serde_json::Error),
    #[error("failed to decode Arcweft bundle JSON: {0}")]
    Decode(#[source] serde_json::Error),
}

impl ArcweftBundle {
    pub fn new(manifest: BundleManifest, source: BundleSource) -> Self {
        Self {
            schema_version: ARCWEFT_BUNDLE_SCHEMA_VERSION,
            manifest,
            source,
            virtual_files: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_virtual_files(
        mut self,
        files: impl IntoIterator<Item = BundleVirtualFile>,
    ) -> Self {
        self.virtual_files.extend(files);
        self.virtual_files.sort_by(|left, right| {
            left.space
                .as_str()
                .cmp(right.space.as_str())
                .then_with(|| left.path.cmp(&right.path))
        });
        self
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, BundleCodecError> {
        serde_json::to_vec_pretty(self).map_err(BundleCodecError::Encode)
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, BundleCodecError> {
        let bundle: Self = serde_json::from_slice(bytes).map_err(BundleCodecError::Decode)?;
        if bundle.schema_version != ARCWEFT_BUNDLE_SCHEMA_VERSION {
            return Err(BundleCodecError::UnsupportedSchema {
                actual: bundle.schema_version,
                expected: ARCWEFT_BUNDLE_SCHEMA_VERSION,
            });
        }
        Ok(bundle)
    }
}

impl BundleVirtualFileSpace {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asset => "asset",
            Self::Save => "save",
            Self::Temp => "temp",
            Self::Export => "export",
        }
    }
}

impl std::fmt::Display for BundleVirtualFileSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_json_round_trips_without_paths() {
        let bundle = ArcweftBundle::new(
            BundleManifest {
                source_label: "main.arcw".to_owned(),
                profile_id: None,
                profile_kind: None,
                entry: Some("main".to_owned()),
                adapter: Some("native-file".to_owned()),
                adapter_manifest_ids: vec!["native-file".to_owned()],
                required_host_calls: vec!["fs.read_text".to_owned()],
                runtime: BundleRuntimeSummary {
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 3,
                    line_task_groups: 0,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            BundleSource {
                label: "main.arcw".to_owned(),
                text: "flow @flow.main main { return \"ok\" }".to_owned(),
            },
        )
        .with_virtual_files([BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "dialogue/opening.txt".to_owned(),
            bytes: b"hello".to_vec(),
        }]);

        let bytes = bundle.to_json_bytes().expect("bundle encodes");
        let json = String::from_utf8(bytes.clone()).expect("json is utf8");
        assert!(!json.contains("D:\\"));
        assert!(!json.contains("/tmp/"));
        assert_eq!(
            ArcweftBundle::from_json_slice(&bytes).expect("bundle decodes"),
            bundle
        );
    }
}
