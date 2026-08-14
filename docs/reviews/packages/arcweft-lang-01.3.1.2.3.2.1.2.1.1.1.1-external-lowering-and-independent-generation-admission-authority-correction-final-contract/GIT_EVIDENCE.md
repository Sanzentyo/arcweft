# Git evidence

- Branch ref inspected: `main`.
- Full commit: `80348beed0efa72db07f712122217b4e679e0a97`.
- Parent: `eb450570acff118ccc3e2a75751144f037af170f`.
- Commit subject: `Record checked root-site return blocker`.
- Commit patch SHA-256: `cb4bbf123e49549aef931e735f802d316ea5de73eb001ed8481200986c4e09c0`.
- Request in patch/raw current main: SHA-256 `2498106d805515f2fba326ef55685a8699aec2ab1abb986e22bc2f0a1f984cc6`.
- Direct container `git clone` result: **failed** before checkout because the
  container could not resolve `github.com`; no clean/dirty working-tree claim is
  made for a checkout that did not exist.
- Fallback actually performed: exact commit patch retrieval, patch application
  to reconstruct all added files, and full-SHA raw retrieval of relevant
  current source/AGENTS files.
- Production modifications performed: **none**.

This is Git evidence, not a Jujutsu identity. The package does not invent a
working-tree cleanliness result.
