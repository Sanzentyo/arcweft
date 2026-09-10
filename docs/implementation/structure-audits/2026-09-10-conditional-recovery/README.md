# Generated conditional recovery audit

Generated on 2026-09-10 from existing `main` at
`11bbdff0471881480e731abcf15605234ac3979e`, with the conditional recovery
implementation dirty in the checkout:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-10-conditional-recovery --fail-on-blocking
```

Result: 95 packages, 2,258 Rust files, 310 review triggers, zero blocking
violations. These generated reports are audit evidence, not production source
or an implementation specification. The
[ownership review](../../2026-09-10-conditional-recovery-structure.md)
records dispositions for every touched trigger.

- [File measurements](file_metrics.csv)
- [Package measurements](package_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Review triggers](findings.md)
- [Public type name screening](public_type_duplicates.csv)
