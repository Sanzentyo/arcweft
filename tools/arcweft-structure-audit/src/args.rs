use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Args {
    pub root: PathBuf,
    pub write_dir: Option<PathBuf>,
    pub fail_on_violations: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseOutcome {
    Run(Args),
    Help,
}

pub fn parse() -> Result<ParseOutcome, String> {
    parse_from(std::env::args_os().skip(1))
}

fn parse_from<I>(arguments: I) -> Result<ParseOutcome, String>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let mut root = PathBuf::from(".");
    let mut write_dir = None;
    let mut fail_on_violations = false;
    let mut arguments = arguments.into_iter();

    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--root") => {
                root = arguments
                    .next()
                    .map(PathBuf::from)
                    .ok_or_else(|| "--root requires a path".to_owned())?;
            }
            Some("--write") => {
                write_dir = Some(
                    arguments
                        .next()
                        .map(PathBuf::from)
                        .ok_or_else(|| "--write requires a directory".to_owned())?,
                );
            }
            Some("--fail-on-violations") => fail_on_violations = true,
            Some("--help" | "-h") => return Ok(ParseOutcome::Help),
            Some(value) => return Err(format!("unknown argument: {value}")),
            None => return Err("arguments must be valid UTF-8".to_owned()),
        }
    }

    Ok(ParseOutcome::Run(Args {
        root,
        write_dir,
        fail_on_violations,
    }))
}

pub fn help() -> &'static str {
    concat!(
        "arcweft-structure-audit\n\n",
        "Usage:\n",
        "  arcweft-structure-audit [--root PATH] [--write DIR] ",
        "[--fail-on-violations]\n\n",
        "Options:\n",
        "  --root PATH             Repository root (default: .)\n",
        "  --write DIR             Write CSV/Markdown reports; omitted means dry-run\n",
        "  --fail-on-violations    Exit with status 2 when warnings or errors are found\n",
        "  -h, --help              Show this help\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn defaults_to_dry_run() {
        let outcome = parse_from(Vec::<OsString>::new()).expect("parse arguments");
        assert_eq!(
            outcome,
            ParseOutcome::Run(Args {
                root: PathBuf::from("."),
                write_dir: None,
                fail_on_violations: false,
            })
        );
    }

    #[test]
    fn parses_explicit_report_directory() {
        let outcome = parse_from([
            OsString::from("--root"),
            OsString::from("repo"),
            OsString::from("--write"),
            OsString::from("target/audit"),
            OsString::from("--fail-on-violations"),
        ])
        .expect("parse arguments");
        let ParseOutcome::Run(arguments) = outcome else {
            panic!("expected run arguments");
        };
        assert_eq!(arguments.root, PathBuf::from("repo"));
        assert_eq!(arguments.write_dir, Some(PathBuf::from("target/audit")));
        assert!(arguments.fail_on_violations);
    }
}
