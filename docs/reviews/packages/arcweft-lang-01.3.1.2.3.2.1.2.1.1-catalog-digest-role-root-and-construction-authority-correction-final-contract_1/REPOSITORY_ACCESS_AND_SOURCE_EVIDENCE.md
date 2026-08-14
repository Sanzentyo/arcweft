# Repository access and source evidence

## Current main

GitHub's current `main` commit inspected for this replacement is `36f83f8509417d1110a34f1b32aee6f4a113dcf3`, commit title `Record invalid catalog authority return`. Its commit tree changes only `docs/implementation` and retained review-package paths; it does not change production crates. The maintained request and invalid intake note were read in full from that head.

## Clone attempt and fallback

Attempted command:

```bash
git clone --depth=1 --branch main https://github.com/Sanzentyo/arcweft.git /mnt/data/arcweft-current-main
```

The container networking layer could not connect to GitHub. `git-clone-current-main.log` is included unchanged. Investigation continued through commit-pinned raw/rendered GitHub files and the source captures listed in `SOURCE_EVIDENCE.csv`, which is the permitted repository-access fallback for this project. This package does not claim a local clean working tree, Cargo execution, or a completed clone.

## Exact maintained inputs

- `SOURCE_REQUEST.md`: SHA-256 `0c570da664999507d1895813d65a707fb13726d48c489e8fd322c238a3361b78`; byte-identical current-main maintained request, including the project-root error clarification.
- `SOURCE_INVALID_INTAKE_NOTE.md`: SHA-256 `6f2d13f40738fb05c806f3283404afc8e1c9617d26aee0a42fad1c5b9f53e7f7`; full invalid-return adjudication.
- parent archive: SHA-256 `aa43429b6ffe5aac6489c94c7ff7a117ca1bbd43c764fed6ff4a1f3b5d540e06`; locally rehashed and safely extracted for retained substrate.

## Evidence rule

Current source, maintained docs, and the retained parent govern. Source snapshot paths used for current production shapes are valid because the current head is a documentation-only intake commit; no production-source change intervenes at that head. No result is based only on conversation summary or filename.
