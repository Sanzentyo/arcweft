//! Declaration linkage for checked external calls. Authored aliases and runtime
//! display labels are not implementation identities.

use serde::{Deserialize, Serialize};

use crate::entry::{RuntimeCallableId, RuntimeIdentityError};
pub use arcweft_manifest_model::StandardEnvironmentId;

/// Registration authority, independent of a source binding or alias.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeExternalCallableProvider {
    Standard(StandardEnvironmentId),
    Adapter(String),
}

/// Complete declaration lookup key within its registration authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeExternalCallableExport {
    Function(Box<[String]>),
    Method { receiver: [u8; 32], name: String },
    RustFunction(Box<[String]>),
}

/// Selected external declaration, preserving provider, kind and overload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "DeclarationFields")]
pub struct RuntimeExternalCallableDeclaration {
    provider: RuntimeExternalCallableProvider,
    export: RuntimeExternalCallableExport,
    overload: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclarationFields {
    provider: RuntimeExternalCallableProvider,
    export: RuntimeExternalCallableExport,
    overload: u16,
}

impl TryFrom<DeclarationFields> for RuntimeExternalCallableDeclaration {
    type Error = RuntimeIdentityError;

    fn try_from(fields: DeclarationFields) -> Result<Self, Self::Error> {
        Self::try_new(fields.provider, fields.export, fields.overload)
    }
}

impl RuntimeExternalCallableDeclaration {
    pub fn try_new(
        provider: RuntimeExternalCallableProvider,
        export: RuntimeExternalCallableExport,
        overload: u16,
    ) -> Result<Self, RuntimeIdentityError> {
        if let RuntimeExternalCallableProvider::Adapter(package) = &provider {
            validate_component("external callable provider", package)?;
        }
        match &export {
            RuntimeExternalCallableExport::Function(path)
            | RuntimeExternalCallableExport::RustFunction(path) => {
                if path.is_empty() {
                    return Err(RuntimeIdentityError::Empty {
                        kind: "external callable path",
                    });
                }
                for name in path {
                    validate_component("external callable path segment", name)?;
                }
            }
            RuntimeExternalCallableExport::Method { name, .. } => {
                validate_component("external callable method", name)?;
            }
        }
        Ok(Self {
            provider,
            export,
            overload,
        })
    }

    pub const fn provider(&self) -> &RuntimeExternalCallableProvider {
        &self.provider
    }

    pub const fn export(&self) -> &RuntimeExternalCallableExport {
        &self.export
    }

    pub const fn overload(&self) -> u16 {
        self.overload
    }
}

fn validate_component(kind: &'static str, value: &str) -> Result<(), RuntimeIdentityError> {
    if value.is_empty() {
        return Err(RuntimeIdentityError::Empty { kind });
    }
    if let Some((byte, _)) = value
        .char_indices()
        .find(|(_, character)| character.is_control())
    {
        return Err(RuntimeIdentityError::Control { kind, byte });
    }
    Ok(())
}

/// Checked semantic identity and its selected declaration linkage. The checked
/// identity still binds the complete signature and accepted catalog generation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeExternalCallable {
    identity: RuntimeCallableId,
    declaration: RuntimeExternalCallableDeclaration,
}

impl RuntimeExternalCallable {
    pub const fn new(
        identity: RuntimeCallableId,
        declaration: RuntimeExternalCallableDeclaration,
    ) -> Self {
        Self {
            identity,
            declaration,
        }
    }

    pub const fn identity(&self) -> &RuntimeCallableId {
        &self.identity
    }

    pub const fn declaration(&self) -> &RuntimeExternalCallableDeclaration {
        &self.declaration
    }

    pub(crate) fn encode_body_metadata(
        &self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        encoder.string(self.identity.as_str());
        match self.declaration.provider() {
            RuntimeExternalCallableProvider::Standard(provider) => {
                encoder.tag(0);
                encoder.tag(provider.canonical_tag());
            }
            RuntimeExternalCallableProvider::Adapter(package) => {
                encoder.tag(1);
                encoder.string(package);
            }
        }
        match self.declaration.export() {
            RuntimeExternalCallableExport::Function(path)
            | RuntimeExternalCallableExport::RustFunction(path) => {
                encoder.tag(
                    if matches!(
                        self.declaration.export(),
                        RuntimeExternalCallableExport::Function(_)
                    ) {
                        0
                    } else {
                        2
                    },
                );
                encoder.count(path.len());
                for segment in path {
                    encoder.enter_element();
                    encoder.string(segment);
                }
            }
            RuntimeExternalCallableExport::Method { receiver, name } => {
                encoder.tag(1);
                encoder.digest(receiver);
                encoder.string(name);
            }
        }
        encoder.ordinal(u32::from(self.declaration.overload()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

    #[test]
    fn external_callable_semantic_metadata_binds_linkage_as_well_as_checked_identity() {
        let digest = |provider, export, overload| {
            let callable = RuntimeExternalCallable::new(
                RuntimeCallableId::from_checked_digest([0xa1; 32]),
                RuntimeExternalCallableDeclaration::try_new(provider, export, overload).unwrap(),
            );
            let mut meter = TaskSemanticMeter::new(u64::MAX, u64::MAX);
            let mut encoder =
                TaskSemanticEncoder::new(b"arcweft.test.external-call.v1\0", &mut meter);
            callable.encode_body_metadata(&mut encoder);
            encoder.finish().unwrap()
        };
        let provider =
            RuntimeExternalCallableProvider::Standard(StandardEnvironmentId::InferenceTensor);
        let export =
            RuntimeExternalCallableExport::Function(["infer".into(), "add_f32".into()].into());
        let original = digest(provider.clone(), export.clone(), 0);
        assert_ne!(
            original,
            digest(
                RuntimeExternalCallableProvider::Adapter("other".into()),
                export.clone(),
                0
            )
        );
        assert_ne!(original, digest(provider.clone(), export, 1));
        assert_ne!(
            original,
            digest(
                provider.clone(),
                RuntimeExternalCallableExport::Function(["infer.add_f32".into()].into()),
                0
            )
        );
        assert_ne!(
            original,
            digest(
                provider,
                RuntimeExternalCallableExport::RustFunction(
                    ["infer".into(), "add_f32".into()].into()
                ),
                0
            )
        );
    }
}
