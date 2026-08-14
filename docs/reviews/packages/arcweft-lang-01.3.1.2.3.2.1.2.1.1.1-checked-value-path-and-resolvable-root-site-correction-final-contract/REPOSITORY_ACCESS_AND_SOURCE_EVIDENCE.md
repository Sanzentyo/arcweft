# Repository access and source-evidence boundary

## Current main

GitHub `main` was verified as `36f83f8509417d1110a34f1b32aee6f4a113dcf3`, title `Record invalid catalog authority return`. The request itself names the same implementation-audit commit. Source URLs in `SOURCE_EVIDENCE.csv` are pinned to that full SHA, not a moving branch.

## Clone attempt

Attempted exactly:

```bash
git clone --depth=1 --branch main https://github.com/Sanzentyo/arcweft.git /mnt/data/arcweft-current-main
```

The execution container returned `Could not resolve host: github.com`; no `.git` directory was created. The exact output is included as `git-clone-current-main.log`. Investigation then used commit-pinned raw GitHub source, which the project instructions permit when cloning is unavailable. The source captures were hashed and are inventoried in `SOURCE_EVIDENCE.csv`; the content-type-blocked product-step source is separately recorded with exact commit URL and line ranges in `WEB_INSPECTION_EVIDENCE.csv`.

## Inputs

- current request SHA-256: `034eb287c315d699d1cf110babaffbd80650d2b8c1eb340bb6e8d6b6efc6c32e`; `SOURCE_REQUEST.md` is byte-identical;
- retained retry ZIP SHA-256: `e0aa31dfefa5bc0d9fab213d19fef6fd74a142cef6dd7d4e6922d05c077bc998`; locally rehashed before use;
- current-main source evidence: 43 hashed files, one commit-pinned web-inspected source row, plus selected line-numbered excerpts;
- applicable policy evidence: 6 hashed root/scoped files.

This design archive does not claim a local clean Git working tree, production compilation, or implementation test execution.
