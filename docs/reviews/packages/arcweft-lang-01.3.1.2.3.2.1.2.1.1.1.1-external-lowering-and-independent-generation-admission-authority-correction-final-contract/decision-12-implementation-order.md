# Decision 12 — compile-clean implementation and deletion order

`IMPLEMENTATION_ORDER.csv` defines fourteen coherent phases. Each phase ends
with focused `cargo check`, tests, and Clippy for the changed crates; the final
phase runs the workspace and structural gates. A replacement and the obsolete
surface it supersedes are in the same phase.

The order begins with the public checked cross-crate construction vocabulary,
then migrates the real lowerer before privatizing raw aggregates. The
independent generation projection/issuer and compiler assembly land before any
admission/context API. Operational expression facts and effect-owned audio
coordinates land before final AWBC pair admission. Driver, bundle,
restore/replay, VM, AOT, snapshot, and docs migrate only after admitted wrappers
exist.

No phase contains a placeholder generation, raw self-admission fallback,
temporary public field, disabled validation branch, compatibility alias, dual
reader, or version increment. Exact additions, migrations, deletions, and
commands are normative in the CSV.
