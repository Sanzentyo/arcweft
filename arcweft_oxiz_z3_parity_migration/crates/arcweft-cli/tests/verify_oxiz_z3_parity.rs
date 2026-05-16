use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Case {
    id: String,
    logic: String,
    benchmark: String,
    expected: String,
    awft_path: String,
    smt_path: String,
    sha256: String,
}

fn suite_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("verify")
        .join("oxiz_z3_parity")
}

fn parse_manifest(root: &Path) -> Vec<Case> {
    let text = fs::read_to_string(root.join("MANIFEST.tsv")).expect("read MANIFEST.tsv");
    let mut cases = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        if line_no == 0 || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), 10, "bad manifest column count on line {}", line_no + 1);
        cases.push(Case {
            id: cols[1].to_string(),
            logic: cols[2].to_string(),
            benchmark: cols[3].to_string(),
            expected: cols[4].to_string(),
            awft_path: cols[7].to_string(),
            smt_path: cols[8].to_string(),
            sha256: cols[9].to_string(),
        });
    }
    cases
}

fn extract_emit_smt(awft: &str) -> String {
    let marker = "emit_smt <<SMT";
    let start = awft.find(marker).expect("missing `emit_smt <<SMT` marker") + marker.len();
    let rest = &awft[start..];
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let end = rest.find("\nSMT").expect("missing closing SMT marker");
    rest[..end].trim_end_matches('\n').to_string()
}

fn trim_final_newlines(s: &str) -> String {
    s.trim_end_matches('\n').to_string()
}

#[test]
fn all_oxiz_z3_parity_fixtures_generate_expected_smt() {
    let root = suite_root();
    let cases = parse_manifest(&root);
    assert_eq!(cases.len(), 168, "manifest case count changed");

    let checklist = fs::read_to_string(root.join("CHECKLIST.md")).expect("read CHECKLIST.md");
    let mut seen = HashSet::new();

    for case in &cases {
        assert!(seen.insert(case.id.clone()), "duplicate case id {}", case.id);
        assert!(checklist.contains(&format!("`{}`", case.id)), "checklist missing {}", case.id);

        let awft = fs::read_to_string(root.join(&case.awft_path))
            .unwrap_or_else(|err| panic!("read {}: {err}", case.awft_path));
        let generated = extract_emit_smt(&awft);
        let expected = fs::read_to_string(root.join(&case.smt_path))
            .unwrap_or_else(|err| panic!("read {}: {err}", case.smt_path));
        let expected = trim_final_newlines(&expected);

        assert_eq!(generated, expected, "SMT mismatch for {}", case.id);
        assert!(generated.contains(&format!("(set-logic {}", case.logic.to_ascii_uppercase())), "{} missing set-logic", case.id);
        assert!(generated.contains("(check-sat)"), "{} missing check-sat", case.id);
        assert!(matches!(case.expected.as_str(), "sat" | "unsat" | "unknown"), "bad expected result for {}", case.id);
        assert_eq!(case.benchmark.ends_with(".smt2"), true, "benchmark must be .smt2");
        assert_eq!(case.sha256.len(), 64, "sha256 must be hex encoded for {}", case.id);
    }
}
