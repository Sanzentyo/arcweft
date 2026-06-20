use arcweft_debug_model::chunk::ChunkId;
use arcweft_debug_model::embedding::{
    EmbeddingError, EmbeddingInput, EmbeddingModelDescriptor, EmbeddingProvider, StoredEmbedding,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub(in crate::app) enum RemoteCommandEmbeddingError {
    #[error("failed to serialize remote embedding request: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to spawn remote embedding command `{command}`: {source}")]
    Spawn {
        command: String,
        source: std::io::Error,
    },
    #[error("failed to write remote embedding request to `{command}`: {source}")]
    WriteRequest {
        command: String,
        source: std::io::Error,
    },
    #[error("failed to wait for remote embedding command `{command}`: {source}")]
    Wait {
        command: String,
        source: std::io::Error,
    },
    #[error("remote embedding command `{command}` exited with {status}: {stderr}")]
    Failed {
        command: String,
        status: String,
        stderr: String,
    },
    #[error("failed to parse remote embedding response from `{command}`: {source}")]
    ParseResponse {
        command: String,
        source: serde_json::Error,
    },
    #[error("remote embedding response contains duplicate chunk id `{0:?}`")]
    DuplicateChunk(ChunkId),
    #[error("remote embedding response contains unknown chunk id `{0:?}`")]
    UnknownChunk(ChunkId),
    #[error("remote embedding response omitted chunk id `{0:?}`")]
    MissingChunk(ChunkId),
    #[error("remote embedding response for `{chunk_id:?}` is invalid: {source}")]
    InvalidEmbedding {
        chunk_id: ChunkId,
        source: EmbeddingError,
    },
}

#[derive(Debug)]
pub(in crate::app) struct RemoteCommandEmbeddingProvider {
    descriptor: EmbeddingModelDescriptor,
    command: String,
    args: Vec<String>,
}

impl RemoteCommandEmbeddingProvider {
    pub(in crate::app) fn new(
        descriptor: EmbeddingModelDescriptor,
        command: String,
        args: Vec<String>,
    ) -> Self {
        Self {
            descriptor,
            command,
            args,
        }
    }
}

impl EmbeddingProvider for RemoteCommandEmbeddingProvider {
    type Error = RemoteCommandEmbeddingError;

    fn descriptor(&self) -> EmbeddingModelDescriptor {
        self.descriptor.clone()
    }

    fn embed(&mut self, inputs: &[EmbeddingInput]) -> Result<Vec<StoredEmbedding>, Self::Error> {
        let request = RemoteEmbeddingRequest {
            schema_version: 1,
            model: &self.descriptor,
            inputs,
        };
        let request_bytes =
            serde_json::to_vec(&request).map_err(RemoteCommandEmbeddingError::Serialize)?;
        let output = run_remote_embedding_command(&self.command, &self.args, &request_bytes)?;
        let response: RemoteEmbeddingResponse =
            serde_json::from_slice(&output.stdout).map_err(|source| {
                RemoteCommandEmbeddingError::ParseResponse {
                    command: self.command.clone(),
                    source,
                }
            })?;
        normalize_remote_embeddings(&self.descriptor, inputs, response.embeddings)
    }
}

struct RemoteEmbeddingCommandOutput {
    stdout: Vec<u8>,
}

fn run_remote_embedding_command(
    command: &str,
    args: &[String],
    request_bytes: &[u8],
) -> Result<RemoteEmbeddingCommandOutput, RemoteCommandEmbeddingError> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| RemoteCommandEmbeddingError::Spawn {
            command: command.to_owned(),
            source,
        })?;
    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(request_bytes)
        .map_err(|source| RemoteCommandEmbeddingError::WriteRequest {
            command: command.to_owned(),
            source,
        })?;
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|source| RemoteCommandEmbeddingError::Wait {
            command: command.to_owned(),
            source,
        })?;
    if !output.status.success() {
        return Err(RemoteCommandEmbeddingError::Failed {
            command: command.to_owned(),
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(RemoteEmbeddingCommandOutput {
        stdout: output.stdout,
    })
}

fn normalize_remote_embeddings(
    model: &EmbeddingModelDescriptor,
    inputs: &[EmbeddingInput],
    vectors: Vec<RemoteEmbeddingVector>,
) -> Result<Vec<StoredEmbedding>, RemoteCommandEmbeddingError> {
    let input_by_chunk = inputs
        .iter()
        .map(|input| (input.chunk_id.clone(), input))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let created_unix_ms = current_unix_millis();
    let mut embeddings = Vec::with_capacity(vectors.len());
    for vector in vectors {
        if !seen.insert(vector.chunk_id.clone()) {
            return Err(RemoteCommandEmbeddingError::DuplicateChunk(vector.chunk_id));
        }
        let input = input_by_chunk
            .get(&vector.chunk_id)
            .ok_or_else(|| RemoteCommandEmbeddingError::UnknownChunk(vector.chunk_id.clone()))?;
        let chunk_id = vector.chunk_id;
        let embedding = StoredEmbedding::normalized(
            chunk_id.clone(),
            model.clone(),
            vector.values,
            input.content_hash.clone(),
            created_unix_ms,
        )
        .map_err(|source| RemoteCommandEmbeddingError::InvalidEmbedding { chunk_id, source })?;
        embeddings.push(embedding);
    }
    if let Some(missing) = input_by_chunk
        .keys()
        .find(|chunk_id| !seen.contains(*chunk_id))
        .cloned()
    {
        return Err(RemoteCommandEmbeddingError::MissingChunk(missing));
    }
    Ok(embeddings)
}

fn current_unix_millis() -> i64 {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

#[derive(serde::Serialize)]
struct RemoteEmbeddingRequest<'a> {
    schema_version: u32,
    model: &'a EmbeddingModelDescriptor,
    inputs: &'a [EmbeddingInput],
}

#[derive(serde::Deserialize)]
struct RemoteEmbeddingResponse {
    embeddings: Vec<RemoteEmbeddingVector>,
}

#[derive(serde::Deserialize)]
struct RemoteEmbeddingVector {
    chunk_id: ChunkId,
    values: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_debug_model::chunk::PrivacyClass;
    use std::fs;

    #[test]
    fn remote_command_provider_normalizes_fixture_vectors() {
        let script = write_fixture_provider_script(
            "remote-command-provider-normalizes",
            r#"{"embeddings":[{"chunk_id":"chunk:remote","values":[3.0,4.0]}]}"#,
        );
        let (command, args) = fixture_provider_command(&script);
        let model = EmbeddingModelDescriptor {
            model_id: "fixture-remote".to_owned(),
            model_revision: "1".to_owned(),
            dimensions: 2,
        };
        let mut provider = RemoteCommandEmbeddingProvider::new(model, command, args);
        let embeddings = provider
            .embed(&[EmbeddingInput {
                chunk_id: ChunkId::new("chunk:remote"),
                content_hash: "blake3:remote".to_owned(),
                privacy: PrivacyClass::Project,
                text: "remote body".to_owned(),
            }])
            .expect("remote provider embeds fixture chunk");

        assert_eq!(embeddings.len(), 1);
        assert_eq!(embeddings[0].chunk_id.as_str(), "chunk:remote");
        assert_eq!(embeddings[0].values, vec![0.6, 0.8]);
        assert_eq!(embeddings[0].content_hash, "blake3:remote");
    }

    #[test]
    fn remote_command_provider_rejects_missing_chunk_vectors() {
        let script = write_fixture_provider_script(
            "remote-command-provider-missing",
            r#"{"embeddings":[{"chunk_id":"chunk:other","values":[1.0,0.0]}]}"#,
        );
        let (command, args) = fixture_provider_command(&script);
        let model = EmbeddingModelDescriptor {
            model_id: "fixture-remote".to_owned(),
            model_revision: "1".to_owned(),
            dimensions: 2,
        };
        let mut provider = RemoteCommandEmbeddingProvider::new(model, command, args);
        let error = provider
            .embed(&[EmbeddingInput {
                chunk_id: ChunkId::new("chunk:remote"),
                content_hash: "blake3:remote".to_owned(),
                privacy: PrivacyClass::Project,
                text: "remote body".to_owned(),
            }])
            .expect_err("unknown response chunk is rejected");

        assert!(matches!(
            error,
            RemoteCommandEmbeddingError::UnknownChunk(chunk_id)
                if chunk_id.as_str() == "chunk:other"
        ));
    }

    fn write_fixture_provider_script(name: &str, response: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("arcweft-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create provider fixture dir");
        if cfg!(windows) {
            let path = dir.join("provider.cmd");
            fs::write(
                &path,
                format!("@echo off\r\nmore >NUL\r\necho {response}\r\n"),
            )
            .expect("write provider cmd fixture");
            path
        } else {
            let path = dir.join("provider.sh");
            fs::write(
                &path,
                format!("cat >/dev/null\nprintf '%s\\n' '{response}'\n"),
            )
            .expect("write provider shell fixture");
            path
        }
    }

    fn fixture_provider_command(script: &std::path::Path) -> (String, Vec<String>) {
        if cfg!(windows) {
            (
                "cmd".to_owned(),
                vec!["/C".to_owned(), script.display().to_string()],
            )
        } else {
            ("sh".to_owned(), vec![script.display().to_string()])
        }
    }
}
