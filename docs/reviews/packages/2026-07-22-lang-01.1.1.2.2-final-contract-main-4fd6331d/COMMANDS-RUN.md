# Commands and Repository Operations Actually Run

All repository operations were read-only. Times are within the 2026-07-22 Asia/Tokyo work session.

## 1. Repository head checks

Executed twice through the configured GitHub connector:

```text
GitHub.search_commits(
  repository_full_name="Sanzentyo/arcweft",
  query="",
  topn=10,
  sort="committer-date",
  order="desc"
)
```

Both returned:

```text
4fd6331dc342d30a7f4ac7774852b60801866ef7  Implement project nominal type resolution
```

## 2. Repository metadata and source reads

Executed through the GitHub connector:

```text
GitHub.get_repo(repository_full_name="Sanzentyo/arcweft")
GitHub.fetch_commit(repo_full_name="Sanzentyo/arcweft", commit_sha="4fd6331dc342d30a7f4ac7774852b60801866ef7")
GitHub.fetch_file(repository_full_name="Sanzentyo/arcweft", path="AGENTS.md", ref="4fd6331dc342d30a7f4ac7774852b60801866ef7")
GitHub.search(repository_name="Sanzentyo/arcweft", query=...)
GitHub.fetch_file(repository_full_name="Sanzentyo/arcweft", path=<path>, ref="4fd6331dc342d30a7f4ac7774852b60801866ef7")
```

`GitHub.fetch_file` was run for every file listed in `evidence/INSPECTED-FILES.tsv`; large files were read in commit-pinned ranges and scrolled through connector response resources where required.

Representative searches actually run included:

```text
AcceptedNominalWorld
AdapterTypeKind
EnvironmentCallablePublicationRecord
ArcweftRustTypeRef::Named
AdapterTypeKind::Named
source_backed_registration_facts
with_callable_publication
try_callable_publication
GenericTypeOwnerId
nominal_catalog_digest
schema_digest
CompilerObjectKey environment_digest
HoverRequest::METHOD
```

## 3. Local instruction/request read

Executed:

```bash
wc -l   '/mnt/data/Rust Skill.txt'   '/mnt/data/前提(Sanzentyo-arcweft).txt'   '/mnt/data/2026-07-22-lang-01.1.1.2.2-adapter-callable-nominal-publication-projection-correction.md'

cat '/mnt/data/Rust Skill.txt'
cat '/mnt/data/前提(Sanzentyo-arcweft).txt'
cat '/mnt/data/2026-07-22-lang-01.1.1.2.2-adapter-callable-nominal-publication-projection-correction.md'
```

Observed line counts:

```text
56  Rust Skill.txt
0   前提(Sanzentyo-arcweft).txt  (single line without trailing newline)
151 request
```

## 4. Supplemental local checkout probe

A local `git ls-remote`/archive checkout probe was attempted during inspection. The execution environment could not resolve the external Git host, and `gh` was not installed. Repository evidence and latest-main verification were therefore performed through the authenticated GitHub connector, which successfully returned commit-pinned source.

This probe did not create or modify a repository checkout.

## 5. Artifact generation and validation

Executed with Python 3 in the local artifact runtime:

```text
create /mnt/data/2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d/
write contract, evidence, 197-row CSV matrix, validator, and integrity files
compute SHA-256 for every package file
create deterministic ZIP with sorted entries and fixed timestamps
```

Final local verification commands:

```bash
python3   /mnt/data/2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d/validation/validate_contract.py

unzip -t   /mnt/data/2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d.zip

zipinfo -1   /mnt/data/2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d.zip

sha256sum   /mnt/data/2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d.zip
```

The captured results are in `validation/VALIDATION-RESULT.txt` and the sibling `.zip.sha256` file.

## 6. Commands not claimed as run

No production implementation was made, so this package does not claim execution of repository formatting, Clippy, or Cargo tests. The exact commands required for the future implementation commit are listed under “Production acceptance commands” in `IMPLEMENTATION-MAP.md`.
