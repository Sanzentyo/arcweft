# Rust skill evidence

- Input: `/mnt/data/Rust Skill.txt`
- SHA-256: `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`
- Read boundary: complete file, first through final line.

Applied rules relevant to this no-implementation contract:

- owner-local/newtype APIs rather than stringly values;
- narrow public API and responsibility modules;
- no unsafe/Box::leak/forget;
- no unstable feature requirement;
- careful dependency additions; the `png` version already exists in the workspace;
- Clippy and fmt at implementation cut points;
- iterator/canonical collection use where it does not hide errors or sacrifice efficiency.

Repository `AGENTS.md` supersedes the skill where it is more specific, especially by preferring inherent methods over local extension traits and by prohibiting source gates and compatibility layers.
