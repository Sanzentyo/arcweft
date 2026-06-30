# seq04.8.2 persistent-cache build fixture

This fixture directory defines the evidence expected from a covered ordinary `arcw build` route after applying the seq04.8.2 patch.

## Covered shape

- one project;
- one source module;
- one compile unit;
- no extra source modules or SCC units;
- AWFB product `ProgramBytecode` section is the same canonical AWBC bytes used as the reusable bytecode unit.

## Commands

```bash
# from a full repository checkout after applying the patch
rm -rf target/seq04-8-2
arcw build --target-dir target/seq04-8-2 --json > first-build.json
arcw build --target-dir target/seq04-8-2 --json > second-build.json
```

Then inspect the cache records under `target/cache/v1` with the repository's `cache explain` command.

## Expected result

- First build stores actual reusable bytecode/link records.
- Second build performs read-through validation and reports `hit_then_rebuilt`, not `hit`, because this cut still rebuilds before write-through.
- `cache explain` reports the underlying actual hit separately from conservative gates.
- Extracted AWFB bytes and AWBC `ProgramBytecode` bytes are identical across rebuilt/cached outputs.

See the JSON snippets in this directory for the exact fields to assert.
