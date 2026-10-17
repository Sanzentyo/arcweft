//! One runtime projection of accepted callable identity and declaration linkage.

use arcweft_core::{
    entry::{RuntimeCallableId, RuntimeIdentityError},
    value::{
        RuntimeCallTarget, RuntimeExternalCallable, RuntimeExternalCallableDeclaration,
        RuntimeExternalCallableExport, RuntimeExternalCallableProvider,
        RuntimeStandardCallableProvider,
    },
};

use super::{
    CallableLookupKey, CheckedCallableDeclaration, CheckedCallableId, EnvironmentCallableId,
    EnvironmentCallableKind, EnvironmentCallableOwner, StandardEnvironmentId,
};

#[derive(Debug, thiserror::Error)]
pub enum RuntimeCallableProjectionError {
    #[error(transparent)]
    GenericScope(#[from] crate::types::GenericScopeError),
    #[error(transparent)]
    Identity(#[from] RuntimeIdentityError),
    #[error("the accepted external callable kind and lookup key differ")]
    KindMismatch,
}

impl CheckedCallableId {
    /// Projects the exact checked identity without replacing it with a source
    /// spelling. Environment calls also retain their selected declaration.
    pub fn runtime_target(&self) -> Result<RuntimeCallTarget, RuntimeCallableProjectionError> {
        let identity = RuntimeCallableId::from_checked_digest(self.semantic_digest().into_bytes());
        match self.declaration() {
            CheckedCallableDeclaration::Environment(declaration) => {
                Ok(RuntimeCallTarget::External(RuntimeExternalCallable::new(
                    identity,
                    declaration.runtime_declaration()?,
                )))
            }
            _ => Ok(RuntimeCallTarget::callable(identity)),
        }
    }
}

impl EnvironmentCallableId {
    fn runtime_declaration(
        &self,
    ) -> Result<RuntimeExternalCallableDeclaration, RuntimeCallableProjectionError> {
        let provider = match self.owner() {
            EnvironmentCallableOwner::Adapter(package) => {
                RuntimeExternalCallableProvider::Adapter(package.as_str().to_owned())
            }
            EnvironmentCallableOwner::Standard(environment) => {
                RuntimeExternalCallableProvider::Standard(match environment {
                    StandardEnvironmentId::Core => RuntimeStandardCallableProvider::Core,
                    StandardEnvironmentId::SansIo => RuntimeStandardCallableProvider::SansIo,
                    StandardEnvironmentId::NativeHttp => {
                        RuntimeStandardCallableProvider::NativeHttp
                    }
                    StandardEnvironmentId::InferenceTensor => {
                        RuntimeStandardCallableProvider::InferenceTensor
                    }
                    StandardEnvironmentId::SystemInfo => {
                        RuntimeStandardCallableProvider::SystemInfo
                    }
                    StandardEnvironmentId::NativeFile => {
                        RuntimeStandardCallableProvider::NativeFile
                    }
                    StandardEnvironmentId::Math => RuntimeStandardCallableProvider::Math,
                })
            }
        };
        let export = match (self.kind(), self.key()) {
            (EnvironmentCallableKind::Function, CallableLookupKey::Free(path))
            | (EnvironmentCallableKind::RustFunction, CallableLookupKey::Free(path)) => {
                let path = path
                    .segments()
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect();
                match self.kind() {
                    EnvironmentCallableKind::RustFunction => {
                        RuntimeExternalCallableExport::RustFunction(path)
                    }
                    _ => RuntimeExternalCallableExport::Function(path),
                }
            }
            (EnvironmentCallableKind::Method, CallableLookupKey::Method(key)) => {
                RuntimeExternalCallableExport::Method {
                    receiver: *key.receiver().semantic_identity_digest()?.as_bytes(),
                    name: key.method().as_str().to_owned(),
                }
            }
            _ => return Err(RuntimeCallableProjectionError::KindMismatch),
        };
        Ok(RuntimeExternalCallableDeclaration::try_new(
            provider,
            export,
            u16::try_from(self.overload().get()).expect("validated u16 overload"),
        )?)
    }
}
