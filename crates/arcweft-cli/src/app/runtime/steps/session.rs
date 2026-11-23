use super::{RuntimeExecutorInstance, RuntimeStepRunError};
use arcweft_runtime_host::native_session::NativeSourceSessionSave;
use arcweft_runtime_host::{BundleAssetAdapter, NativeTaskBridge};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::app) struct NativeSessionFiles<'a> {
    pub(super) load: Option<&'a Path>,
    pub(super) save: Option<&'a Path>,
}

impl NativeSessionFiles<'_> {
    pub(super) fn restore(
        self,
        executor: &mut RuntimeExecutorInstance,
        host: Option<&mut NativeTaskBridge>,
        assets: Option<BundleAssetAdapter>,
    ) -> Result<u64, RuntimeStepRunError> {
        let Some(path) = self.load else {
            return Ok(0);
        };
        let host = host.ok_or(RuntimeStepRunError::SessionHostRequired)?;
        let bytes = read_save(path)?;
        let saved = NativeSourceSessionSave::decode(&bytes)?;
        let assets = assets.map(|adapter| adapter.resolver());
        saved
            .restore(executor.source_executor_mut(), host, move |value| {
                if let Some(assets) = &assets {
                    assets
                        .validate_owned_value(value)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                } else if arcweft_core::value::runtime_bundle_asset_opaque_role(value).is_some() {
                    Err("saved asset value requires the original bundle asset catalog".into())
                } else {
                    Ok(())
                }
            })
            .map_err(Into::into)
    }

    pub(super) fn capture(
        self,
        executor: &RuntimeExecutorInstance,
        host: Option<&NativeTaskBridge>,
        next_step_index: u64,
        task_events: &[arcweft_core::task::TaskEvent],
        host_results: &[arcweft_core::step::RuntimeHostCallResult],
    ) -> Result<(), RuntimeStepRunError> {
        let Some(path) = self.save else {
            return Ok(());
        };
        let host = host.ok_or(RuntimeStepRunError::SessionHostRequired)?;
        let save = NativeSourceSessionSave::capture(
            executor.source_executor(),
            host,
            next_step_index,
            task_events,
            host_results,
        )?;
        write_save(path, &save.encode()?).map_err(|source| RuntimeStepRunError::SessionFile {
            path: path.to_owned(),
            source,
        })
    }
}

fn read_save(path: &Path) -> Result<Vec<u8>, RuntimeStepRunError> {
    let file_error = |source| RuntimeStepRunError::SessionFile {
        path: path.to_owned(),
        source,
    };
    let mut bytes = Vec::new();
    let limit = NativeSourceSessionSave::max_file_bytes();
    File::open(path)
        .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
        .map_err(file_error)?;
    if bytes.len() as u64 > limit {
        return Err(file_error(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "source session save exceeds the envelope byte limit",
        )));
    }
    Ok(bytes)
}

fn write_save(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let directory = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}
