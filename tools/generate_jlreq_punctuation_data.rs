use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

const SOURCE_PATH: &str = "crates/arcweft-text-layout/data/jlreq_punctuation_ranges.txt";
const PAIR_SOURCE_PATH: &str = "crates/arcweft-text-layout/data/jlreq_pair_adjustments.txt";
const OUTPUT_PATH: &str = "crates/arcweft-text-layout/src/jlreq_punctuation_data.rs";

#[derive(Clone, Debug, Eq, PartialEq)]
struct Range {
    start: u32,
    end: u32,
    class: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PairAdjustmentRule {
    left: Option<String>,
    right: String,
    keep_together: bool,
    break_penalty: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Check,
    Apply,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mode = parse_mode(env::args().skip(1))?;
    let root = env::current_dir().map_err(|error| error.to_string())?;
    let source_path = root.join(SOURCE_PATH);
    let pair_source_path = root.join(PAIR_SOURCE_PATH);
    let output_path = root.join(OUTPUT_PATH);
    let source = fs::read_to_string(&source_path)
        .map_err(|error| format!("failed to read {}: {error}", source_path.display()))?;
    let pair_source = fs::read_to_string(&pair_source_path)
        .map_err(|error| format!("failed to read {}: {error}", pair_source_path.display()))?;
    let version = parse_version(&source, "source data")?;
    let pair_version = parse_version(&pair_source, "pair source data")?;
    let ranges = parse_ranges(&source)?;
    let pair_rules = parse_pair_rules(&pair_source)?;
    let output = generate(&version, &pair_version, &ranges, &pair_rules);

    match mode {
        Mode::Check => check_output(&output_path, &output),
        Mode::Apply => write_output(&output_path, &output),
    }
}

fn parse_mode(args: impl Iterator<Item = String>) -> Result<Mode, String> {
    let mut mode = Mode::Check;
    for arg in args {
        match arg.as_str() {
            "--check" => mode = Mode::Check,
            "--apply" => mode = Mode::Apply,
            "-h" | "--help" => {
                println!(
                    "Usage: rustc tools/generate_jlreq_punctuation_data.rs -o target/generate_jlreq_punctuation_data && target/generate_jlreq_punctuation_data [--check|--apply]"
                );
                process::exit(0);
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(mode)
}

fn parse_version(source: &str, label: &str) -> Result<String, String> {
    source
        .lines()
        .find_map(|line| line.strip_prefix("# version: "))
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{label} is missing `# version: ...`"))
}

fn parse_ranges(source: &str) -> Result<Vec<Range>, String> {
    let mut ranges = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.splitn(3, ';').map(str::trim);
        let class = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("line {line_number}: missing class"))?;
        validate_class(class).map_err(|error| format!("line {line_number}: {error}"))?;
        let range = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("line {line_number}: missing codepoint range"))?;
        let _notes = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("line {line_number}: missing notes"))?;
        let (start, end) =
            parse_codepoint_range(range).map_err(|error| format!("line {line_number}: {error}"))?;
        ranges.push(Range {
            start,
            end,
            class: class.to_owned(),
        });
    }
    normalize_ranges(ranges)
}

fn validate_class(class: &str) -> Result<(), String> {
    match class {
        "Closing" | "Opening" | "SmallKana" | "Dash" | "Leader" | "MiddleDot" | "RepeatMark" => {
            Ok(())
        }
        _ => Err(format!("unknown class `{class}`")),
    }
}

fn parse_codepoint_range(range: &str) -> Result<(u32, u32), String> {
    if let Some((start, end)) = range.split_once("..") {
        let start = parse_hex(start)?;
        let end = parse_hex(end)?;
        if start > end {
            return Err(format!("range start {start:04X} is after end {end:04X}"));
        }
        Ok((start, end))
    } else {
        let value = parse_hex(range)?;
        Ok((value, value))
    }
}

fn parse_hex(value: &str) -> Result<u32, String> {
    u32::from_str_radix(value, 16).map_err(|error| format!("invalid hex `{value}`: {error}"))
}

fn normalize_ranges(mut ranges: Vec<Range>) -> Result<Vec<Range>, String> {
    ranges.sort_by_key(|range| (range.start, range.end));
    let mut normalized: Vec<Range> = Vec::new();
    for range in ranges {
        if let Some(previous) = normalized.last_mut() {
            if range.start <= previous.end {
                return Err(format!(
                    "overlapping ranges {:#06X}..{:#06X} and {:#06X}..{:#06X}",
                    previous.start, previous.end, range.start, range.end
                ));
            }
            if previous.class == range.class && previous.end.saturating_add(1) == range.start {
                previous.end = range.end;
                continue;
            }
        }
        normalized.push(range);
    }
    Ok(normalized)
}

fn parse_pair_rules(source: &str) -> Result<Vec<PairAdjustmentRule>, String> {
    let mut rules = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.splitn(5, ';').map(str::trim);
        let left = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("line {line_number}: missing left class"))?;
        let left = if left == "Any" {
            None
        } else {
            validate_class(left).map_err(|error| format!("line {line_number}: {error}"))?;
            Some(left.to_owned())
        };
        let right = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("line {line_number}: missing right class"))?;
        validate_class(right).map_err(|error| format!("line {line_number}: {error}"))?;
        let keep_together = parse_bool(
            parts
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("line {line_number}: missing keep-together flag"))?,
        )
        .map_err(|error| format!("line {line_number}: {error}"))?;
        let break_penalty = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("line {line_number}: missing break penalty"))?
            .parse::<u16>()
            .map_err(|error| format!("line {line_number}: invalid break penalty: {error}"))?;
        let _notes = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("line {line_number}: missing notes"))?;
        rules.push(PairAdjustmentRule {
            left,
            right: right.to_owned(),
            keep_together,
            break_penalty,
        });
    }
    Ok(rules)
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("invalid bool `{value}`")),
    }
}

fn generate(
    version: &str,
    pair_version: &str,
    ranges: &[Range],
    pair_rules: &[PairAdjustmentRule],
) -> String {
    let mut output = String::new();
    output.push_str("//! Checked-in JLREQ punctuation range data.\n");
    output.push_str("//!\n");
    output.push_str("//! This file is generated from `../data/jlreq_punctuation_ranges.txt`\n");
    output.push_str("//! and `../data/jlreq_pair_adjustments.txt`.\n");
    output.push_str(
        "//! Do not edit range data by hand; run `tools/generate_jlreq_punctuation_data.rs`.\n\n",
    );
    output.push_str("/// Checked-in JLREQ punctuation data version.\n");
    output.push_str("pub const JLREQ_PUNCTUATION_DATA_VERSION: &str = \"");
    output.push_str(version);
    output.push_str("\";\n\n");
    output.push_str("/// Checked-in JLREQ pair adjustment data version.\n");
    output.push_str("pub const JLREQ_PAIR_ADJUSTMENT_DATA_VERSION: &str = \"");
    output.push_str(pair_version);
    output.push_str("\";\n\n");
    output.push_str("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n");
    output.push_str("pub(crate) enum JlreqPunctuationClass {\n");
    output.push_str("    Closing,\n");
    output.push_str("    Opening,\n");
    output.push_str("    SmallKana,\n");
    output.push_str("    Dash,\n");
    output.push_str("    Leader,\n");
    output.push_str("    MiddleDot,\n");
    output.push_str("    RepeatMark,\n");
    output.push_str("}\n\n");
    output.push_str("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n");
    output.push_str("pub(crate) struct JlreqPunctuationRange {\n");
    output.push_str("    pub(crate) start: u32,\n");
    output.push_str("    pub(crate) end: u32,\n");
    output.push_str("    pub(crate) class: JlreqPunctuationClass,\n");
    output.push_str("}\n\n");
    output.push_str("pub(crate) const JLREQ_PUNCTUATION_RANGES: &[JlreqPunctuationRange] = &[\n");
    for range in ranges {
        output.push_str("    JlreqPunctuationRange {\n");
        output.push_str(&format!("        start: 0x{:04X},\n", range.start));
        output.push_str(&format!("        end: 0x{:04X},\n", range.end));
        output.push_str("        class: JlreqPunctuationClass::");
        output.push_str(&range.class);
        output.push_str(",\n");
        output.push_str("    },\n");
    }
    output.push_str("];\n");
    output.push('\n');
    output.push_str("#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]\n");
    output.push_str("pub(crate) struct JlreqPairAdjustment {\n");
    output.push_str("    pub(crate) keep_together: bool,\n");
    output.push_str("    pub(crate) break_penalty: u16,\n");
    output.push_str("}\n\n");
    output.push_str("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n");
    output.push_str("pub(crate) struct JlreqPairAdjustmentRule {\n");
    output.push_str("    pub(crate) left: Option<JlreqPunctuationClass>,\n");
    output.push_str("    pub(crate) right: JlreqPunctuationClass,\n");
    output.push_str("    pub(crate) adjustment: JlreqPairAdjustment,\n");
    output.push_str("}\n\n");
    output.push_str("pub(crate) const JLREQ_PAIR_ADJUSTMENTS: &[JlreqPairAdjustmentRule] = &[\n");
    for rule in pair_rules {
        output.push_str("    JlreqPairAdjustmentRule {\n");
        match &rule.left {
            Some(left) => {
                output.push_str("        left: Some(JlreqPunctuationClass::");
                output.push_str(left);
                output.push_str("),\n");
            }
            None => output.push_str("        left: None,\n"),
        }
        output.push_str("        right: JlreqPunctuationClass::");
        output.push_str(&rule.right);
        output.push_str(",\n");
        output.push_str("        adjustment: JlreqPairAdjustment {\n");
        output.push_str(&format!(
            "            keep_together: {},\n",
            rule.keep_together
        ));
        output.push_str(&format!(
            "            break_penalty: {},\n",
            rule.break_penalty
        ));
        output.push_str("        },\n");
        output.push_str("    },\n");
    }
    output.push_str("];\n");
    output
}

fn check_output(path: &Path, expected: &str) -> Result<(), String> {
    let actual = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if actual == expected {
        println!("{} is up to date", path.display());
        Ok(())
    } else {
        Err(format!(
            "{} is out of date; rerun with --apply",
            PathBuf::from(path).display()
        ))
    }
}

fn write_output(path: &Path, output: &str) -> Result<(), String> {
    fs::write(path, output)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}
