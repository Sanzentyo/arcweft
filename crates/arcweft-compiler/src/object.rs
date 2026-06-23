use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_project::fingerprint::BuildDigest;

/// Compiler-private module object summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleObject {
    module: CanonicalModulePath,
    interface_digest: BuildDigest,
    body_digest: BuildDigest,
    object_digest: BuildDigest,
    encoded: Vec<u8>,
}

impl ModuleObject {
    /// Creates a deterministic module object envelope.
    pub fn new(
        module: CanonicalModulePath,
        interface_digest: BuildDigest,
        body_digest: BuildDigest,
        encoded: Vec<u8>,
    ) -> Self {
        let object_digest = object_digest(&module, interface_digest, body_digest, &encoded);
        Self {
            module,
            interface_digest,
            body_digest,
            object_digest,
            encoded,
        }
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn interface_digest(&self) -> BuildDigest {
        self.interface_digest
    }

    pub const fn body_digest(&self) -> BuildDigest {
        self.body_digest
    }

    pub const fn object_digest(&self) -> BuildDigest {
        self.object_digest
    }

    pub fn encoded(&self) -> &[u8] {
        &self.encoded
    }
}

fn object_digest(
    module: &CanonicalModulePath,
    interface_digest: BuildDigest,
    body_digest: BuildDigest,
    encoded: &[u8],
) -> BuildDigest {
    let mut bytes = Vec::new();
    put_string(&mut bytes, "arcweft-module-object-v1");
    put_string(&mut bytes, &module.to_string());
    put_digest(&mut bytes, interface_digest);
    put_digest(&mut bytes, body_digest);
    put_digest(&mut bytes, BuildDigest::of(encoded));
    BuildDigest::of(&bytes)
}

fn put_string(out: &mut Vec<u8>, value: &str) {
    let len = u32::try_from(value.len()).expect("module object string length fits u32");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn put_digest(out: &mut Vec<u8>, digest: BuildDigest) {
    out.extend_from_slice(&digest.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::ModuleObject;
    use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModulePath};
    use arcweft_project::fingerprint::BuildDigest;

    fn module(path: &str) -> CanonicalModulePath {
        path.parse::<ModulePath>()
            .expect("module path")
            .resolve_from(&CanonicalModulePath::crate_root())
            .expect("canonical path")
    }

    #[test]
    fn module_object_digest_depends_on_payload() {
        let first = ModuleObject::new(
            module("game"),
            BuildDigest::of(b"interface"),
            BuildDigest::of(b"body"),
            b"one".to_vec(),
        );
        let second = ModuleObject::new(
            module("game"),
            BuildDigest::of(b"interface"),
            BuildDigest::of(b"body"),
            b"two".to_vec(),
        );

        assert_ne!(first.object_digest(), second.object_digest());
    }
}
