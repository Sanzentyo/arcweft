# Applicable instructions and precedence

## Sources read completely

1. `SOURCE_REQUEST.md`, SHA-256 `5f1bf2335fb0c68f8aef66a3e7e63628bcaffdda80a29d131ee0930b24b3fda4`.
2. The supplied Rust skill, SHA-256 `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`.
3. The project precondition, SHA-256 `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1`.
4. Current root `AGENTS.md`, locally retained SHA-256 `90bae8bface6d390246538c60842da7d71d1ebd576ae3fa403019caa35a91498`.
5. Current scoped `crates/AGENTS.md`, `docs/reviews/AGENTS.md`, and
   `docs/implementation/AGENTS.md` through the repository source.
6. The accepted Lang-01.5.1.1.1 owner/admission result and current production
   consumers named by the request.

## Applied rules

- Current production, maintained documents, and accepted typed contracts outrank
  filename implications and conversation summaries.
- Preserve `syntax -> HIR -> sema -> compiler/runtime-plan -> bundle/runtime`
  direction and keep lower/data crates Sans I/O.
- Select one typed authority; no source reconstruction, old AST/flattened-HIR
  reader, fallback resolver, copied endpoint catalog, compatibility alias, or
  dual reader.
- Replace unreleased internal shapes directly and use deletion-driven migration.
- Put missing domain behavior on Arcweft-owned enums/types or a legitimate named
  context; do not add an extension trait or one-off conversion helper.
- No `unsafe`, unstable feature, or new macro is required by this design.
- Validation records must distinguish passed, failed, blocked, and not run.

## Request precedence retained

The accepted dialogue-profile owner chain, typed `SyntaxNodeId`/final-HIR
identity, ordinary-function/direct-suspension roles, typed RichText authority,
typed resource registry, CharacterDialogue runtime, and current persistent
identity remain authoritative. This package only closes the View execution and
static-certificate gap.
