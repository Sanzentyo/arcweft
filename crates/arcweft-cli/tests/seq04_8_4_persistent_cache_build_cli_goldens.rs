use arcweft_bundle::container::{BundleSectionKind, BundleView, ReadBudget};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

const REGENERATE_ENV: &str = "ARCWEFT_REGENERATE_SEQ04_8_4_GOLDENS";
const NORMAL_SINGLE_FIXTURE: &str = "fixtures/persistent-cache-build/seq04-8-4/normal-single";
const NORMAL_CONSERVATIVE_FIXTURE: &str =
    "fixtures/persistent-cache-build/seq04-8-4/normal-conservative-multi";
const GOLDEN_ROOT: &str = "fixtures/persistent-cache-build/seq04-8-4/goldens";

#[test]
fn normal_single_module_build_cli_goldens_cover_actual_reuse_and_corrupt_record() {
    let workspace = workspace_root();
    let fixture = workspace.join(NORMAL_SINGLE_FIXTURE);
    let work = temp_root("normal-single");
    let target_root = work.join("target");

    let first = run_build(&fixture, &target_root);
    let first_awfb = read_artifact_bytes(&first, ".awfb");
    let first_awbc = program_awbc_bytes(&first_awfb);
    let first_snapshot = read_artifact_json(&first, ".snapshot.json");

    let second = run_build(&fixture, &target_root);
    let second_awfb = read_artifact_bytes(&second, ".awfb");
    let second_awbc = program_awbc_bytes(&second_awfb);
    let second_snapshot = read_artifact_json(&second, ".snapshot.json");

    let normal = json!({
        "fixture": "normal-single",
        "command_shape": "arcw build --manifest-path <FIXTURE>/arcw.toml --target-dir <TEMP>/target --json",
        "first_build": {
            "report": normalize_project_report(&first),
            "artifacts": artifact_presence(&first),
            "cache_records": {
                "bytecode_unit": normalize_actual_record(record_for_query(&first, "bytecode_unit")),
                "link_plan": normalize_actual_record(record_for_query(&first, "link_plan"))
            },
            "snapshot_queries": snapshot_query_statuses(
                &first_snapshot,
                &["bytecode_unit", "link_plan"],
            )
        },
        "second_build": {
            "report": normalize_project_report(&second),
            "artifacts": artifact_presence(&second),
            "cache_records": {
                "bytecode_unit": normalize_actual_record(record_for_query(&second, "bytecode_unit")),
                "link_plan": normalize_actual_record(record_for_query(&second, "link_plan"))
            },
            "snapshot_queries": snapshot_query_statuses(
                &second_snapshot,
                &["bytecode_unit", "link_plan"],
            )
        },
        "byte_stability": {
            "content_root_equal": first_snapshot["content_root"] == second_snapshot["content_root"],
            "awfb_bytes_equal": first_awfb == second_awfb,
            "program_awbc_bytes_equal": first_awbc == second_awbc
        }
    });
    assert_or_regenerate(
        &workspace.join(GOLDEN_ROOT).join("normal-build-cli.json"),
        &normal,
    );

    let cache_root = cache_root_from_build(&second);
    let bytecode_logical_item = logical_item(record_for_query(&second, "bytecode_unit"));
    let link_logical_item = logical_item(record_for_query(&second, "link_plan"));
    let bytecode_explain = run_cache_explain(&cache_root, &bytecode_logical_item);
    let link_explain = run_cache_explain(&cache_root, &link_logical_item);

    assert_or_regenerate(
        &workspace
            .join(GOLDEN_ROOT)
            .join("cache-explain-bytecode-unit.json"),
        &normalize_explain_report(&bytecode_explain),
    );
    assert_or_regenerate(
        &workspace
            .join(GOLDEN_ROOT)
            .join("cache-explain-link-plan.json"),
        &normalize_explain_report(&link_explain),
    );

    let bytecode_record_path = cache_explain_record_path(&bytecode_explain);
    fs::write(&bytecode_record_path, b"not-json").expect("corrupt bytecode-unit record");

    let third = run_build(&fixture, &target_root);
    let third_snapshot = read_artifact_json(&third, ".snapshot.json");
    let corrupt = json!({
        "fixture": "normal-single",
        "corrupted_query": "bytecode_unit",
        "corrupted_record": "<CACHE>/records/bytecode-unit/<ARTIFACT-KEY>.awci",
        "third_build": {
            "report": normalize_project_report(&third),
            "cache_record": normalize_actual_record(record_for_query(&third, "bytecode_unit")),
            "snapshot_query": snapshot_status_for_query(&third_snapshot, "bytecode_unit")
        }
    });
    assert_or_regenerate(
        &workspace
            .join(GOLDEN_ROOT)
            .join("corrupt-record-soft-miss.json"),
        &corrupt,
    );

    let _ = fs::remove_dir_all(work);
}

#[test]
fn multi_module_build_cli_golden_reports_typed_conservative_evidence() {
    let workspace = workspace_root();
    let fixture = workspace.join(NORMAL_CONSERVATIVE_FIXTURE);
    let work = temp_root("normal-conservative");
    let target_root = work.join("target");

    let first = run_build(&fixture, &target_root);
    let second = run_build(&fixture, &target_root);

    let conservative = json!({
        "fixture": "normal-conservative-multi",
        "command_shape": "arcw build --manifest-path <FIXTURE>/arcw.toml --target-dir <TEMP>/target --json",
        "first_build": {
            "report": normalize_project_report(&first),
            "cache_records": {
                "bytecode_unit": normalize_conservative_records(&first, "bytecode_unit"),
                "link_plan": normalize_conservative_records(&first, "link_plan")
            }
        },
        "second_build": {
            "report": normalize_project_report(&second),
            "cache_records": {
                "bytecode_unit": normalize_conservative_records(&second, "bytecode_unit"),
                "link_plan": normalize_conservative_records(&second, "link_plan")
            }
        }
    });
    assert_or_regenerate(
        &workspace
            .join(GOLDEN_ROOT)
            .join("normal-conservative-build-cli.json"),
        &conservative,
    );

    let cache_root = cache_root_from_build(&second);
    let bytecode_logical_item = logical_item(record_for_query(&second, "bytecode_unit"));
    let conservative_explain = run_cache_explain(&cache_root, &bytecode_logical_item);
    assert_or_regenerate(
        &workspace
            .join(GOLDEN_ROOT)
            .join("cache-explain-conservative-bytecode-unit.json"),
        &normalize_explain_report(&conservative_explain),
    );

    let _ = fs::remove_dir_all(work);
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root canonicalizes")
}

fn temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after unix epoch")
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "arcweft-seq04-8-4-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create test target root");
    root
}

fn arcw_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_arcw"))
}

fn run_build(fixture: &Path, target_root: &Path) -> Value {
    output_json(
        "arcw build",
        Command::new(arcw_bin())
            .arg("build")
            .arg("--manifest-path")
            .arg(fixture.join("arcw.toml"))
            .arg("--target-dir")
            .arg(target_root)
            .arg("--json")
            .output()
            .expect("spawn arcw build"),
    )
}

fn run_cache_explain(cache_root: &Path, logical_item: &str) -> Value {
    output_json(
        "arcw cache explain",
        Command::new(arcw_bin())
            .arg("cache")
            .arg("explain")
            .arg(logical_item)
            .arg("--logical")
            .arg("--root")
            .arg(cache_root)
            .arg("--json")
            .output()
            .expect("spawn arcw cache explain"),
    )
}

fn output_json(command: &str, output: Output) -> Value {
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{command} failed with status {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );
    serde_json::from_str(&stdout).unwrap_or_else(|error| {
        panic!("{command} stdout should be JSON: {error}\nstdout:\n{stdout}")
    })
}

fn artifact_path(build: &Value, suffix: &str) -> PathBuf {
    build["artifacts"]
        .as_array()
        .expect("build JSON has artifacts array")
        .iter()
        .filter_map(Value::as_str)
        .find(|path| path_has_suffix(path, suffix))
        .map_or_else(
            || panic!("missing artifact suffix {suffix}: {build}"),
            PathBuf::from,
        )
}

fn read_artifact_bytes(build: &Value, suffix: &str) -> Vec<u8> {
    fs::read(artifact_path(build, suffix)).expect("artifact reads")
}

fn read_artifact_json(build: &Value, suffix: &str) -> Value {
    let bytes = read_artifact_bytes(build, suffix);
    serde_json::from_slice(&bytes).expect("artifact JSON decodes")
}

fn program_awbc_bytes(bundle_bytes: &[u8]) -> Vec<u8> {
    let view = BundleView::parse(bundle_bytes, ReadBudget::default()).expect("AWFB parses");
    let descriptor = view
        .sections()
        .iter()
        .find(|descriptor| descriptor.known_kind() == Some(BundleSectionKind::ProgramBytecode))
        .expect("AWFB has ProgramBytecode section");
    view.decoded_section(descriptor.id())
        .expect("ProgramBytecode section decodes")
        .expect("ProgramBytecode section is present")
}

fn cache_root_from_build(build: &Value) -> PathBuf {
    PathBuf::from(string_field(&build["cache"], "root"))
}

fn cache_records(build: &Value) -> &[Value] {
    build["cache"]["records"]
        .as_array()
        .expect("build JSON has cache records")
}

fn record_for_query<'a>(build: &'a Value, query: &str) -> &'a Value {
    cache_records(build)
        .iter()
        .find(|record| record["query"] == query && record.get("reuse_evidence").is_some())
        .unwrap_or_else(|| panic!("missing persistent {query} record: {build}"))
}

fn logical_item(record: &Value) -> String {
    string_field(record, "logical_item")
}

fn cache_explain_record_path(explain: &Value) -> PathBuf {
    let path = explain["matches"]
        .as_array()
        .expect("explain has matches")
        .iter()
        .find(|item| item["kind"] == "record")
        .and_then(|item| item["path"].as_str())
        .expect("explain includes record path");
    PathBuf::from(path)
}

fn normalize_project_report(build: &Value) -> Value {
    let report = &build["report"];
    json!({
        "status": report["status"],
        "package": report["package"],
        "selected_profile": report["selected_profile"],
        "modules": report["modules"]
            .as_array()
            .expect("report modules")
            .iter()
            .map(|module| module["module"].clone())
            .collect::<Vec<_>>(),
        "compile_units": report["compile_units"]
            .as_array()
            .expect("report compile units")
            .iter()
            .map(|unit| json!({
                "id": unit["id"],
                "modules": unit["modules"],
                "cache": unit["cache"]
            }))
            .collect::<Vec<_>>()
    })
}

fn artifact_presence(build: &Value) -> Value {
    let artifacts = build["artifacts"]
        .as_array()
        .expect("build JSON has artifacts array");
    json!({
        "awfb": artifacts
            .iter()
            .any(|value| path_has_suffix(string_value(value), ".awfb")),
        "project_json": artifacts
            .iter()
            .any(|value| path_has_suffix(string_value(value), ".project.json")),
        "plan": artifacts
            .iter()
            .any(|value| path_has_suffix(string_value(value), ".plan")),
        "snapshot_json": artifacts
            .iter()
            .any(|value| path_has_suffix(string_value(value), ".snapshot.json"))
    })
}

fn normalize_actual_record(record: &Value) -> Value {
    let evidence = &record["reuse_evidence"];
    json!({
        "query": record["query"],
        "artifact_kind": record["artifact_kind"],
        "logical_item": record["logical_item"],
        "status": record["status"],
        "key": "<DIGEST>",
        "object_digest": "<DIGEST>",
        "reuse_evidence": {
            "kind": evidence["kind"],
            "producer_family": evidence["producer_family"],
            "identity_owner": evidence["identity_owner"],
            "identity": "<DIGEST>"
        }
    })
}

fn normalize_conservative_records(build: &Value, query: &str) -> Value {
    let mut records = cache_records(build)
        .iter()
        .filter(|record| record["query"] == query)
        .filter(|record| record["reuse_evidence"]["kind"] == "conservative")
        .map(normalize_conservative_record)
        .collect::<Vec<_>>();
    records.sort_by(|left, right| {
        string_field(left, "logical_item").cmp(&string_field(right, "logical_item"))
    });
    Value::Array(records)
}

fn normalize_conservative_record(record: &Value) -> Value {
    let evidence = &record["reuse_evidence"];
    json!({
        "query": record["query"],
        "artifact_kind": record["artifact_kind"],
        "logical_item": record["logical_item"],
        "status": record["status"],
        "key": "<DIGEST>",
        "object_digest": "<DIGEST>",
        "reuse_evidence": {
            "kind": evidence["kind"],
            "producer_family": evidence["producer_family"],
            "identity_owner": evidence["identity_owner"],
            "reason": evidence["reason"],
            "missing_identity": evidence["missing_identity"],
            "consumer_boundary": evidence["consumer_boundary"],
            "follow_up_sequence": evidence["follow_up_sequence"]
        }
    })
}

fn snapshot_query_statuses(snapshot: &Value, queries: &[&str]) -> Value {
    let mut output = Map::new();
    for query in queries {
        output.insert(
            (*query).to_owned(),
            snapshot_status_for_query(snapshot, query),
        );
    }
    Value::Object(output)
}

fn snapshot_status_for_query(snapshot: &Value, query: &str) -> Value {
    snapshot["queries"]
        .as_array()
        .expect("snapshot queries")
        .iter()
        .find(|entry| entry["query"] == query)
        .map_or_else(
            || panic!("snapshot query {query} missing: {snapshot}"),
            |entry| normalize_cache_record_status(&entry["status"]),
        )
}

fn path_has_suffix(path: &str, suffix: &str) -> bool {
    path.to_ascii_lowercase()
        .ends_with(&suffix.to_ascii_lowercase())
}

fn normalize_explain_report(report: &Value) -> Value {
    json!({
        "status": report["status"],
        "query": report["query"],
        "matches": report["matches"]
            .as_array()
            .expect("cache explain matches")
            .iter()
            .map(normalize_explain_match)
            .collect::<Vec<_>>(),
        "issues": report["issues"]
            .as_array()
            .expect("cache explain issues")
            .iter()
            .map(normalize_cache_issue)
            .collect::<Vec<_>>()
    })
}

fn normalize_explain_match(item: &Value) -> Value {
    json!({
        "kind": item["kind"],
        "artifact_kind": item["artifact_kind"],
        "logical_item": item["logical_item"],
        "object_status": item["object_status"],
        "persistent_query": normalize_persistent_query(&item["persistent_query"])
    })
}

fn normalize_persistent_query(query: &Value) -> Value {
    let mut output = Map::new();
    insert_if_present(&mut output, query, "query");
    insert_if_present(&mut output, query, "object_kind");
    insert_if_present(&mut output, query, "producer_family");
    insert_if_present(&mut output, query, "producer_classification");
    insert_if_present(&mut output, query, "conservative_reason");
    insert_if_present(&mut output, query, "payload_kind");
    insert_if_present(&mut output, query, "status");
    if let Some(status) = query.get("cache_record_status") {
        output.insert(
            "cache_record_status".to_owned(),
            normalize_cache_record_status(status),
        );
    }
    insert_if_present(&mut output, query, "soft_miss_reason");
    insert_if_present(&mut output, query, "typecheck_gate_reuse_policy");
    insert_if_present(&mut output, query, "bytecode_unit_reuse_policy");
    insert_if_present(&mut output, query, "link_plan_reuse_policy");
    insert_if_present(&mut output, query, "actual_reuse_policy");
    insert_if_present(&mut output, query, "conservative_reuse_policy");
    insert_if_present(&mut output, query, "recovery_action");
    Value::Object(output)
}

fn normalize_cache_record_status(status: &Value) -> Value {
    let kind = string_field(status, "kind");
    match status.get("reason") {
        Some(reason) => json!({
            "kind": kind,
            "reason": normalize_invalidation_reason(reason)
        }),
        None => json!({ "kind": kind }),
    }
}

fn normalize_invalidation_reason(reason: &Value) -> Value {
    let kind = string_field(reason, "kind");
    match reason.get("policy") {
        Some(policy) => json!({ "kind": kind, "policy": policy }),
        None => json!({ "kind": kind }),
    }
}

fn normalize_cache_issue(issue: &Value) -> Value {
    json!({
        "kind": issue["kind"],
        "message": issue["message"]
    })
}

fn insert_if_present(output: &mut Map<String, Value>, source: &Value, key: &str) {
    if let Some(value) = source.get(key) {
        output.insert(key.to_owned(), value.clone());
    }
}

fn string_field(value: &Value, key: &str) -> String {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("expected string field {key}: {value}"))
        .to_owned()
}

fn string_value(value: &Value) -> &str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("expected JSON string: {value}"))
}

fn assert_or_regenerate(path: &Path, value: &Value) {
    let stable = stable_json(value);
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&stable).expect("golden JSON renders")
    );
    if env::var_os(REGENERATE_ENV).is_some() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create golden directory");
        }
        fs::write(path, rendered).expect("write regenerated golden");
    } else {
        let expected = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let expected: Value = serde_json::from_str(&expected)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
        assert_eq!(expected, stable, "golden drift in {}", path.display());
    }
}

fn stable_json(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(stable_json).collect()),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), stable_json(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        _ => value.clone(),
    }
}
