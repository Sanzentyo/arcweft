use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::{Display, Write as _};

/// Deterministic 256-bit build identity used by project cache keys.
#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BuildDigest([u8; 32]);

/// Named digest entry encoded in canonical name order.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct NamedDigest {
    name: String,
    digest: BuildDigest,
}

/// Project-level fingerprint inputs shared by build snapshots and cache keys.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectFingerprint {
    package: String,
    compiler_build_id: String,
    target_triple: String,
    target_features: Vec<String>,
    profile: String,
    source_root_digest: BuildDigest,
    manifest_digest: BuildDigest,
    adapter_environment_digest: BuildDigest,
    launch_profile_digest: BuildDigest,
    declared_environment_digest: BuildDigest,
}

/// Input fields for a canonical [`ProjectFingerprint`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectFingerprintInput {
    pub package: String,
    pub compiler_build_id: String,
    pub target_triple: String,
    pub target_features: Vec<String>,
    pub profile: String,
    pub source_root_digest: BuildDigest,
    pub manifest_digest: BuildDigest,
    pub adapter_environment_digest: BuildDigest,
    pub launch_profile_digest: BuildDigest,
    pub declared_environment_digest: BuildDigest,
}

impl BuildDigest {
    /// All-zero sentinel for absent optional inputs.
    pub const ZERO: Self = Self([0; 32]);

    /// Hashes canonical bytes with BLAKE3 for the Arcweft build cache.
    pub fn of(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    /// Builds a digest from already validated bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Raw digest bytes.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Lowercase hexadecimal spelling.
    pub fn to_hex(self) -> String {
        self.0
            .iter()
            .fold(String::with_capacity(64), |mut hex, byte| {
                write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
                hex
            })
    }
}

impl std::fmt::Debug for BuildDigest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl Display for BuildDigest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl NamedDigest {
    /// Creates a named digest entry.
    pub fn new(name: impl Into<String>, digest: BuildDigest) -> Self {
        Self {
            name: name.into(),
            digest,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn digest(&self) -> BuildDigest {
        self.digest
    }

    /// Sorts by name and keeps the final entry for duplicate names.
    pub fn canonicalize(entries: impl IntoIterator<Item = Self>) -> Vec<Self> {
        entries
            .into_iter()
            .map(|entry| (entry.name, entry.digest))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|(name, digest)| Self { name, digest })
            .collect()
    }
}

impl ProjectFingerprint {
    /// Creates a canonical project fingerprint.
    pub fn new(input: ProjectFingerprintInput) -> Self {
        let ProjectFingerprintInput {
            package,
            compiler_build_id,
            target_triple,
            mut target_features,
            profile,
            source_root_digest,
            manifest_digest,
            adapter_environment_digest,
            launch_profile_digest,
            declared_environment_digest,
        } = input;
        target_features.sort();
        target_features.dedup();
        Self {
            package,
            compiler_build_id,
            target_triple,
            target_features,
            profile,
            source_root_digest,
            manifest_digest,
            adapter_environment_digest,
            launch_profile_digest,
            declared_environment_digest,
        }
    }

    pub fn package(&self) -> &str {
        &self.package
    }

    pub fn compiler_build_id(&self) -> &str {
        &self.compiler_build_id
    }

    pub fn target_triple(&self) -> &str {
        &self.target_triple
    }

    pub fn target_features(&self) -> &[String] {
        &self.target_features
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }

    pub const fn source_root_digest(&self) -> BuildDigest {
        self.source_root_digest
    }

    pub const fn manifest_digest(&self) -> BuildDigest {
        self.manifest_digest
    }

    pub const fn adapter_environment_digest(&self) -> BuildDigest {
        self.adapter_environment_digest
    }

    pub const fn launch_profile_digest(&self) -> BuildDigest {
        self.launch_profile_digest
    }

    pub const fn declared_environment_digest(&self) -> BuildDigest {
        self.declared_environment_digest
    }

    /// Digest of the canonical project fingerprint fields.
    pub fn digest(&self) -> BuildDigest {
        let mut bytes = Vec::new();
        put_string(&mut bytes, &self.package);
        put_string(&mut bytes, &self.compiler_build_id);
        put_string(&mut bytes, &self.target_triple);
        put_string_vec(&mut bytes, &self.target_features);
        put_string(&mut bytes, &self.profile);
        put_digest(&mut bytes, self.source_root_digest);
        put_digest(&mut bytes, self.manifest_digest);
        put_digest(&mut bytes, self.adapter_environment_digest);
        put_digest(&mut bytes, self.launch_profile_digest);
        put_digest(&mut bytes, self.declared_environment_digest);
        BuildDigest::of(&bytes)
    }
}

pub(crate) fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn put_string(out: &mut Vec<u8>, value: &str) {
    let len = u32::try_from(value.len()).expect("canonical string length fits u32");
    put_u32(out, len);
    out.extend_from_slice(value.as_bytes());
}

pub(crate) fn put_string_vec(out: &mut Vec<u8>, values: &[String]) {
    let len = u32::try_from(values.len()).expect("canonical string vector length fits u32");
    put_u32(out, len);
    for value in values {
        put_string(out, value);
    }
}

pub(crate) fn put_digest(out: &mut Vec<u8>, digest: BuildDigest) {
    out.extend_from_slice(&digest.as_bytes());
}

pub(crate) fn put_named_digests(out: &mut Vec<u8>, values: &[NamedDigest]) {
    let len = u32::try_from(values.len()).expect("canonical digest vector length fits u32");
    put_u32(out, len);
    for value in values {
        put_string(out, value.name());
        put_digest(out, value.digest());
    }
}

#[cfg(test)]
mod tests {
    use super::{BuildDigest, NamedDigest, ProjectFingerprint, ProjectFingerprintInput};

    #[test]
    fn named_digests_are_canonicalized_by_name() {
        let entries = NamedDigest::canonicalize([
            NamedDigest::new("b", BuildDigest::of(b"b")),
            NamedDigest::new("a", BuildDigest::of(b"a")),
        ]);

        assert_eq!(entries[0].name(), "a");
        assert_eq!(entries[1].name(), "b");
    }

    #[test]
    fn project_fingerprint_sorts_target_features() {
        let first = ProjectFingerprint::new(ProjectFingerprintInput {
            package: "pkg".to_owned(),
            compiler_build_id: "compiler".to_owned(),
            target_triple: "target".to_owned(),
            target_features: vec!["simd".to_owned(), "base".to_owned()],
            profile: "dev".to_owned(),
            source_root_digest: BuildDigest::of(b"source"),
            manifest_digest: BuildDigest::of(b"manifest"),
            adapter_environment_digest: BuildDigest::of(b"adapter"),
            launch_profile_digest: BuildDigest::of(b"launch"),
            declared_environment_digest: BuildDigest::of(b"env"),
        });
        let second = ProjectFingerprint::new(ProjectFingerprintInput {
            target_features: vec!["base".to_owned(), "simd".to_owned()],
            ..ProjectFingerprintInput {
                package: "pkg".to_owned(),
                compiler_build_id: "compiler".to_owned(),
                target_triple: "target".to_owned(),
                target_features: Vec::new(),
                profile: "dev".to_owned(),
                source_root_digest: BuildDigest::of(b"source"),
                manifest_digest: BuildDigest::of(b"manifest"),
                adapter_environment_digest: BuildDigest::of(b"adapter"),
                launch_profile_digest: BuildDigest::of(b"launch"),
                declared_environment_digest: BuildDigest::of(b"env"),
            }
        });

        assert_eq!(first.digest(), second.digest());
    }
}
