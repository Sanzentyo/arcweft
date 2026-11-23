use arcweft_audio_core::graph::{
    AudioAsset, AudioBusDef, AudioDecodeStrategy, AudioFormat, AudioGraph,
};
use arcweft_bundle::BundleVirtualFile;
use arcweft_id::{AssetVirtualPath, VoiceResourceId};
use arcweft_interaction_model::audio::{AudioBusId, AudioLoopMode, AudioResourceId, GainDbMilli};
use std::collections::{BTreeMap, btree_map::Entry};
use std::process::ExitCode;

/// Builds the audio graph for recordings in the authored voice resource root.
pub(super) fn collect<'a>(
    files: impl IntoIterator<Item = &'a BundleVirtualFile>,
) -> Result<Option<AudioGraph>, ExitCode> {
    let mut assets = BTreeMap::new();
    for file in files
        .into_iter()
        .filter(|file| file.path.starts_with(VoiceResourceId::VIRTUAL_PATH_PREFIX))
    {
        let Some(format) = file
            .path
            .rsplit('/')
            .next()
            .and_then(|filename| filename.rsplit_once('.'))
            .and_then(|(_, extension)| AudioFormat::from_extension(extension))
        else {
            continue;
        };
        let path = AssetVirtualPath::try_new(file.path.clone()).map_err(|error| {
            eprintln!(
                "error: invalid bundled voice asset path {}: {error}",
                file.path
            );
            ExitCode::FAILURE
        })?;
        let Some(id) = VoiceResourceId::from_virtual_path(&path).map_err(|error| {
            eprintln!("error: invalid bundled voice asset identity for {path}: {error}");
            ExitCode::FAILURE
        })?
        else {
            continue;
        };
        match assets.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert((file.path.as_str(), format));
            }
            Entry::Occupied(entry) => {
                eprintln!(
                    "error: bundled voice assets {} and {} derive the same identity {}",
                    entry.get().0,
                    file.path,
                    entry.key().as_str()
                );
                return Err(ExitCode::FAILURE);
            }
        }
    }
    if assets.is_empty() {
        return Ok(None);
    }
    let assets = assets
        .into_iter()
        .map(|(id, (path, format))| {
            let asset_id = id.bundle_asset_id();
            let id = AudioResourceId::new(asset_id.as_str()).map_err(|error| {
                eprintln!("error: invalid audio resource ID for {path}: {error}");
                ExitCode::FAILURE
            })?;
            Ok(AudioAsset {
                id,
                path: path.to_owned(),
                format,
                strategy: AudioDecodeStrategy::Preload,
                default_loop: AudioLoopMode::None,
            })
        })
        .collect::<Result<_, _>>()?;
    let master_bus = AudioBusId::new("bus.master").expect("standard master bus identity");
    Ok(Some(AudioGraph {
        master_bus: master_bus.clone(),
        assets,
        buses: vec![AudioBusDef {
            id: master_bus,
            parent: None,
            gain: GainDbMilli::UNITY,
            muted: false,
            effects: Vec::new(),
        }],
        snapshots: Vec::new(),
    }))
}
