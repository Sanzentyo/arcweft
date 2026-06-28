use crate::{
    artifact::{ArtifactKey, ArtifactKind},
    fingerprint::{BuildDigest, NamedDigest, ProjectFingerprint},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Display;

/// Version namespace for persistent Arcweft cache records.
pub const CACHE_SCHEMA_VERSION: u32 = 1;

/// Compiler-owned demand level for a module or compile unit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileDemand {
    Interface,
    Body,
}

/// Incremental query families owned by the project model.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryKind {
    Parse,
    Interface,
    HirBody,
    TypeCheck,
    RuntimePlan,
    BytecodeUnit,
    AssetMetadata,
    AssetPayload,
    LinkPlan,
    BundleSection,
    BundleIndex,
}

/// Dependency evidence required by a query key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryDependencyScope {
    SourceOnly,
    DependencyInterfaces,
    DependencyBodies,
}

/// Reason a previous cache record cannot be reused.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum InvalidationReason {
    Reusable,
    MissingRecord,
    CorruptRecord,
    CorruptObject,
    CompilerChanged,
    CacheSchemaChanged,
    SourceChanged,
    InterfaceChanged,
    BodyChanged,
    DependencyInterfaceChanged { module: String },
    DependencyBodyChanged { module: String },
    EnvironmentChanged,
    OptionsChanged,
    ConservativeInvalidation { policy: String },
}

/// Query record status stored in build snapshots and reports.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CacheRecordStatus {
    Hit,
    HitThenRebuilt { reason: InvalidationReason },
    Miss { reason: InvalidationReason },
    Stored,
    Rebuilt { reason: InvalidationReason },
}

/// Per-module digest evidence emitted by a successful build.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleBuildFingerprint {
    module: String,
    source_digest: BuildDigest,
    interface_digest: BuildDigest,
    body_digest: BuildDigest,
    object_digest: Option<BuildDigest>,
    dependency_interface_digests: Vec<NamedDigest>,
    dependency_body_digests: Vec<NamedDigest>,
}

/// One query result recorded in a build snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QuerySnapshot {
    query: QueryKind,
    key: ArtifactKey,
    artifact_digest: BuildDigest,
    status: CacheRecordStatus,
}

/// Successful build evidence used by watch mode and future patch generation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BuildSnapshot {
    build_id: String,
    project: ProjectFingerprint,
    selected_entries: Vec<String>,
    modules: Vec<ModuleBuildFingerprint>,
    queries: Vec<QuerySnapshot>,
    content_root: Option<BuildDigest>,
}

/// Per-module digest delta between two build snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleInvalidation {
    module: String,
    source_changed: bool,
    interface_changed: bool,
    body_changed: bool,
}

/// Query-level reuse or invalidation decision between two build snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryInvalidation {
    query: QueryKind,
    key: ArtifactKey,
    reason: InvalidationReason,
}

impl CompileDemand {
    /// Joins two demand levels without losing body demand.
    #[must_use]
    pub const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Body, _) | (_, Self::Body) => Self::Body,
            (Self::Interface, Self::Interface) => Self::Interface,
        }
    }

    /// Whether the demand needs the implementation body.
    pub const fn requires_body(self) -> bool {
        matches!(self, Self::Body)
    }
}

impl QueryKind {
    /// Stable cache namespace for this query family.
    pub const fn cache_namespace(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Interface => "interface",
            Self::HirBody => "hir-body",
            Self::TypeCheck => "type-check",
            Self::RuntimePlan => "runtime-plan",
            Self::BytecodeUnit => "bytecode-unit",
            Self::AssetMetadata => "asset-metadata",
            Self::AssetPayload => "asset-payload",
            Self::LinkPlan => "link-plan",
            Self::BundleSection => "bundle-section",
            Self::BundleIndex => "bundle-index",
        }
    }

    /// Artifact family produced by this query.
    pub const fn artifact_kind(self) -> ArtifactKind {
        match self {
            Self::Parse => ArtifactKind::ParsedSyntax,
            Self::Interface => ArtifactKind::InterfaceSummary,
            Self::HirBody => ArtifactKind::HirBody,
            Self::TypeCheck => ArtifactKind::TypeCheckReport,
            Self::RuntimePlan => ArtifactKind::RuntimePlan,
            Self::BytecodeUnit => ArtifactKind::BytecodeUnit,
            Self::AssetMetadata => ArtifactKind::AssetMetadata,
            Self::AssetPayload => ArtifactKind::AssetPayload,
            Self::LinkPlan => ArtifactKind::LinkPlan,
            Self::BundleSection => ArtifactKind::BundleSection,
            Self::BundleIndex => ArtifactKind::BundleIndex,
        }
    }

    /// Dependency evidence encoded into this query's key.
    pub const fn dependency_scope(self) -> QueryDependencyScope {
        match self {
            Self::Parse | Self::AssetPayload => QueryDependencyScope::SourceOnly,
            Self::Interface
            | Self::HirBody
            | Self::TypeCheck
            | Self::AssetMetadata
            | Self::BundleSection
            | Self::BundleIndex => QueryDependencyScope::DependencyInterfaces,
            Self::RuntimePlan | Self::BytecodeUnit | Self::LinkPlan => {
                QueryDependencyScope::DependencyBodies
            }
        }
    }
}

impl Display for QueryKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.cache_namespace())
    }
}

impl QueryDependencyScope {
    /// Whether query keys must include dependency body digests.
    pub const fn requires_body_digests(self) -> bool {
        matches!(self, Self::DependencyBodies)
    }
}

impl InvalidationReason {
    /// Whether this reason represents a reusable cache record.
    pub const fn is_reusable(&self) -> bool {
        matches!(self, Self::Reusable)
    }
}

impl CacheRecordStatus {
    /// Stable status label for CLI cache reports.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::HitThenRebuilt { .. } => "hit_then_rebuilt",
            Self::Miss { .. } => "miss",
            Self::Stored => "stored",
            Self::Rebuilt { .. } => "rebuilt",
        }
    }

    /// Whether the artifact was loaded from cache.
    pub const fn is_hit(&self) -> bool {
        matches!(self, Self::Hit)
    }

    /// Whether source rebuild was performed after cache lookup evidence.
    pub const fn is_rebuilt(&self) -> bool {
        matches!(self, Self::HitThenRebuilt { .. } | Self::Rebuilt { .. })
    }

    /// Rebuild reason when this status records a rebuild.
    pub const fn rebuild_reason(&self) -> Option<&InvalidationReason> {
        match self {
            Self::HitThenRebuilt { reason } | Self::Rebuilt { reason } => Some(reason),
            Self::Hit | Self::Miss { .. } | Self::Stored => None,
        }
    }
}

impl ModuleBuildFingerprint {
    /// Creates canonical per-module build evidence.
    pub fn new(
        module: impl Into<String>,
        source_digest: BuildDigest,
        interface_digest: BuildDigest,
        body_digest: BuildDigest,
    ) -> Self {
        Self {
            module: module.into(),
            source_digest,
            interface_digest,
            body_digest,
            object_digest: None,
            dependency_interface_digests: Vec::new(),
            dependency_body_digests: Vec::new(),
        }
    }

    pub fn module(&self) -> &str {
        &self.module
    }

    pub const fn source_digest(&self) -> BuildDigest {
        self.source_digest
    }

    pub const fn interface_digest(&self) -> BuildDigest {
        self.interface_digest
    }

    pub const fn body_digest(&self) -> BuildDigest {
        self.body_digest
    }

    pub const fn object_digest(&self) -> Option<BuildDigest> {
        self.object_digest
    }

    pub fn dependency_interface_digests(&self) -> &[NamedDigest] {
        &self.dependency_interface_digests
    }

    pub fn dependency_body_digests(&self) -> &[NamedDigest] {
        &self.dependency_body_digests
    }

    #[must_use]
    pub fn with_object_digest(mut self, digest: BuildDigest) -> Self {
        self.object_digest = Some(digest);
        self
    }

    #[must_use]
    pub fn with_dependency_interface_digests(
        mut self,
        digests: impl IntoIterator<Item = NamedDigest>,
    ) -> Self {
        self.dependency_interface_digests = NamedDigest::canonicalize(digests);
        self
    }

    #[must_use]
    pub fn with_dependency_body_digests(
        mut self,
        digests: impl IntoIterator<Item = NamedDigest>,
    ) -> Self {
        self.dependency_body_digests = NamedDigest::canonicalize(digests);
        self
    }
}

impl QuerySnapshot {
    pub const fn new(
        query: QueryKind,
        key: ArtifactKey,
        artifact_digest: BuildDigest,
        status: CacheRecordStatus,
    ) -> Self {
        Self {
            query,
            key,
            artifact_digest,
            status,
        }
    }

    pub const fn query(&self) -> QueryKind {
        self.query
    }

    pub const fn key(&self) -> ArtifactKey {
        self.key
    }

    pub const fn artifact_digest(&self) -> BuildDigest {
        self.artifact_digest
    }

    pub const fn status(&self) -> &CacheRecordStatus {
        &self.status
    }
}

impl BuildSnapshot {
    /// Creates a deterministic build snapshot.
    pub fn new(
        build_id: impl Into<String>,
        project: ProjectFingerprint,
        selected_entries: impl IntoIterator<Item = impl Into<String>>,
        modules: impl IntoIterator<Item = ModuleBuildFingerprint>,
        queries: impl IntoIterator<Item = QuerySnapshot>,
    ) -> Self {
        let mut selected_entries = selected_entries
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        selected_entries.sort();
        selected_entries.dedup();
        let mut modules = modules.into_iter().collect::<Vec<_>>();
        modules.sort_by(|left, right| left.module.cmp(&right.module));
        let mut queries = queries.into_iter().collect::<Vec<_>>();
        sort_query_snapshots(&mut queries);
        Self {
            build_id: build_id.into(),
            project,
            selected_entries,
            modules,
            queries,
            content_root: None,
        }
    }

    pub fn build_id(&self) -> &str {
        &self.build_id
    }

    pub const fn project(&self) -> &ProjectFingerprint {
        &self.project
    }

    pub fn selected_entries(&self) -> &[String] {
        &self.selected_entries
    }

    pub fn modules(&self) -> &[ModuleBuildFingerprint] {
        &self.modules
    }

    pub fn queries(&self) -> &[QuerySnapshot] {
        &self.queries
    }

    pub const fn content_root(&self) -> Option<BuildDigest> {
        self.content_root
    }

    #[must_use]
    pub fn with_additional_queries(
        mut self,
        queries: impl IntoIterator<Item = QuerySnapshot>,
    ) -> Self {
        self.queries.extend(queries);
        sort_query_snapshots(&mut self.queries);
        self
    }

    pub fn module_invalidations_since(&self, previous: &Self) -> Vec<ModuleInvalidation> {
        let previous_modules = previous
            .modules
            .iter()
            .map(|module| (module.module.as_str(), module))
            .collect::<BTreeMap<_, _>>();
        self.modules
            .iter()
            .map(|module| {
                previous_modules.get(module.module.as_str()).map_or_else(
                    || ModuleInvalidation::changed(module.module.clone()),
                    |previous| ModuleInvalidation {
                        module: module.module.clone(),
                        source_changed: module.source_digest != previous.source_digest,
                        interface_changed: module.interface_digest != previous.interface_digest,
                        body_changed: module.body_digest != previous.body_digest,
                    },
                )
            })
            .filter(ModuleInvalidation::changed_any)
            .collect()
    }

    pub fn query_invalidations_since(&self, previous: &Self) -> Vec<QueryInvalidation> {
        let module_invalidations = self.module_invalidations_since(previous);
        let previous_queries = previous
            .queries
            .iter()
            .map(|query| ((query.query, query.key), query))
            .collect::<BTreeMap<_, _>>();
        self.queries
            .iter()
            .map(|query| {
                let reason = previous_queries.get(&(query.query, query.key)).map_or_else(
                    || invalidation_reason_for_query(query.query, &module_invalidations),
                    |previous| {
                        if previous.artifact_digest == query.artifact_digest {
                            InvalidationReason::Reusable
                        } else {
                            InvalidationReason::BodyChanged
                        }
                    },
                );
                QueryInvalidation {
                    query: query.query,
                    key: query.key,
                    reason,
                }
            })
            .collect()
    }

    #[must_use]
    pub const fn with_content_root(mut self, content_root: BuildDigest) -> Self {
        self.content_root = Some(content_root);
        self
    }
}

impl ModuleInvalidation {
    fn changed(module: String) -> Self {
        Self {
            module,
            source_changed: true,
            interface_changed: true,
            body_changed: true,
        }
    }

    pub fn module(&self) -> &str {
        &self.module
    }

    pub const fn source_changed(&self) -> bool {
        self.source_changed
    }

    pub const fn interface_changed(&self) -> bool {
        self.interface_changed
    }

    pub const fn body_changed(&self) -> bool {
        self.body_changed
    }

    const fn changed_any(&self) -> bool {
        self.source_changed || self.interface_changed || self.body_changed
    }
}

impl QueryInvalidation {
    pub const fn query(&self) -> QueryKind {
        self.query
    }

    pub const fn key(&self) -> ArtifactKey {
        self.key
    }

    pub const fn reason(&self) -> &InvalidationReason {
        &self.reason
    }
}

fn sort_query_snapshots(queries: &mut [QuerySnapshot]) {
    queries.sort_by(|left, right| {
        left.query
            .cmp(&right.query)
            .then_with(|| left.key.cmp(&right.key))
            .then_with(|| left.artifact_digest.cmp(&right.artifact_digest))
    });
}

fn invalidation_reason_for_query(
    query: QueryKind,
    module_invalidations: &[ModuleInvalidation],
) -> InvalidationReason {
    match query.dependency_scope() {
        QueryDependencyScope::SourceOnly => module_invalidations
            .iter()
            .find(|module| module.source_changed())
            .map_or(InvalidationReason::MissingRecord, |_| {
                InvalidationReason::SourceChanged
            }),
        QueryDependencyScope::DependencyInterfaces => module_invalidations
            .iter()
            .find(|module| module.interface_changed())
            .map_or(InvalidationReason::MissingRecord, |module| {
                InvalidationReason::DependencyInterfaceChanged {
                    module: module.module().to_owned(),
                }
            }),
        QueryDependencyScope::DependencyBodies => module_invalidations
            .iter()
            .find(|module| module.body_changed())
            .map_or(InvalidationReason::MissingRecord, |module| {
                InvalidationReason::DependencyBodyChanged {
                    module: module.module().to_owned(),
                }
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BuildSnapshot, CacheRecordStatus, CompileDemand, InvalidationReason,
        ModuleBuildFingerprint, QueryKind, QuerySnapshot,
    };
    use crate::artifact::{ArtifactKey, ArtifactKeyInput};
    use crate::fingerprint::{BuildDigest, ProjectFingerprint, ProjectFingerprintInput};

    fn digest(label: &str) -> BuildDigest {
        BuildDigest::of(label.as_bytes())
    }

    fn project() -> ProjectFingerprint {
        ProjectFingerprint::new(ProjectFingerprintInput {
            package: "pkg".to_owned(),
            compiler_build_id: "compiler".to_owned(),
            target_triple: "target".to_owned(),
            target_features: Vec::new(),
            profile: "dev".to_owned(),
            source_root_digest: digest("source-root"),
            manifest_digest: digest("manifest"),
            adapter_environment_digest: digest("adapter"),
            launch_profile_digest: digest("launch"),
            declared_environment_digest: digest("env"),
        })
    }

    #[test]
    fn compile_demand_join_is_monotone() {
        assert_eq!(
            CompileDemand::Interface.join(CompileDemand::Body),
            CompileDemand::Body
        );
        assert!(CompileDemand::Body.requires_body());
        assert!(!CompileDemand::Interface.requires_body());
    }

    #[test]
    fn query_kind_owns_namespace_artifact_and_dependency_scope() {
        assert_eq!(QueryKind::Parse.cache_namespace(), "parse");
        assert!(
            !QueryKind::Interface
                .dependency_scope()
                .requires_body_digests()
        );
        assert!(
            QueryKind::RuntimePlan
                .dependency_scope()
                .requires_body_digests()
        );
    }

    #[test]
    fn cache_record_status_reports_hits_and_rebuild_reasons() {
        assert!(CacheRecordStatus::Hit.is_hit());
        assert!(!CacheRecordStatus::Stored.is_hit());
        let rebuilt = CacheRecordStatus::Rebuilt {
            reason: InvalidationReason::MissingRecord,
        };
        assert_eq!(rebuilt.as_str(), "rebuilt");
        assert!(!rebuilt.is_hit());
        assert!(rebuilt.is_rebuilt());
        assert_eq!(
            rebuilt.rebuild_reason(),
            Some(&InvalidationReason::MissingRecord)
        );
    }

    #[test]
    fn build_snapshot_orders_entries_modules_and_queries() {
        let snapshot = BuildSnapshot::new(
            "build",
            project(),
            ["game.release", "game.dev", "game.dev"],
            [
                ModuleBuildFingerprint::new(
                    "crate::b",
                    digest("b-src"),
                    digest("b-if"),
                    digest("b-body"),
                ),
                ModuleBuildFingerprint::new(
                    "crate::a",
                    digest("a-src"),
                    digest("a-if"),
                    digest("a-body"),
                ),
            ],
            [],
        );

        assert_eq!(snapshot.selected_entries(), &["game.dev", "game.release"]);
        assert_eq!(snapshot.modules()[0].module(), "crate::a");
        assert_eq!(snapshot.modules()[1].module(), "crate::b");
    }

    #[test]
    fn build_snapshot_appends_query_evidence_deterministically() {
        let snapshot = BuildSnapshot::new(
            "build",
            project(),
            ["game.dev"],
            [],
            [query_snapshot(QueryKind::HirBody, "b", "b-out")],
        )
        .with_additional_queries([
            QuerySnapshot::new(
                QueryKind::Parse,
                artifact_key(QueryKind::Parse, "parse"),
                digest("parse-out"),
                CacheRecordStatus::Rebuilt {
                    reason: InvalidationReason::CorruptObject,
                },
            ),
            query_snapshot(QueryKind::HirBody, "a", "a-out"),
        ]);

        assert_eq!(snapshot.queries()[0].query(), QueryKind::Parse);
        assert_eq!(snapshot.queries()[1].query(), QueryKind::HirBody);
        assert_eq!(snapshot.queries()[2].query(), QueryKind::HirBody);
        assert!(matches!(
            snapshot.queries()[0].status(),
            CacheRecordStatus::Rebuilt {
                reason: InvalidationReason::CorruptObject
            }
        ));
    }

    #[test]
    fn build_snapshot_reports_module_and_query_invalidations() {
        let previous = BuildSnapshot::new(
            "previous",
            project(),
            ["game.dev"],
            [ModuleBuildFingerprint::new(
                "crate::a",
                digest("a-src"),
                digest("a-if"),
                digest("a-body"),
            )],
            [
                query_snapshot(QueryKind::Parse, "parse", "parse-out"),
                query_snapshot(QueryKind::RuntimePlan, "plan", "plan-out"),
            ],
        );
        let current = BuildSnapshot::new(
            "current",
            project(),
            ["game.dev"],
            [ModuleBuildFingerprint::new(
                "crate::a",
                digest("a-src-2"),
                digest("a-if"),
                digest("a-body-2"),
            )],
            [
                query_snapshot(QueryKind::Parse, "parse-2", "parse-out-2"),
                query_snapshot(QueryKind::RuntimePlan, "plan-2", "plan-out-2"),
            ],
        );

        let modules = current.module_invalidations_since(&previous);
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].module(), "crate::a");
        assert!(modules[0].source_changed());
        assert!(!modules[0].interface_changed());
        assert!(modules[0].body_changed());

        let invalidations = current.query_invalidations_since(&previous);
        assert_eq!(
            invalidations[0].reason(),
            &InvalidationReason::SourceChanged
        );
        assert_eq!(
            invalidations[1].reason(),
            &InvalidationReason::DependencyBodyChanged {
                module: "crate::a".to_owned()
            }
        );
    }

    fn query_snapshot(query: QueryKind, key_label: &str, artifact_label: &str) -> QuerySnapshot {
        QuerySnapshot::new(
            query,
            artifact_key(query, key_label),
            digest(artifact_label),
            CacheRecordStatus::Stored,
        )
    }

    fn artifact_key(query: QueryKind, label: &str) -> ArtifactKey {
        ArtifactKey::derive(&ArtifactKeyInput {
            compiler_build_id: "compiler".to_owned(),
            query,
            artifact_kind: query.artifact_kind(),
            target_triple: "target".to_owned(),
            target_features: Vec::new(),
            profile: "dev".to_owned(),
            package: "pkg".to_owned(),
            logical_item: label.to_owned(),
            source_digest: digest(label),
            dependency_interface_digests: Vec::new(),
            dependency_body_digests: Vec::new(),
            adapter_environment_digest: BuildDigest::ZERO,
            launch_profile_digest: BuildDigest::ZERO,
            declared_environment_digest: BuildDigest::ZERO,
            format_options_digest: BuildDigest::ZERO,
        })
    }
}
