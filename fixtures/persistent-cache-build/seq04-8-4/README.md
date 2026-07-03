# seq04.8.4 persistent-cache normal build CLI fixture

This fixture directory owns the checked-in, normalized CLI goldens for normal
`arcw build` persistent bytecode/link cache evidence.

## Fixture shape

- `normal-single/`: one package, one root source module, one compile unit. This is
the minimal ordinary build shape that can prove actual reusable bytecode/link
identity after seq04.8.2 and seq04.8.3.
- `normal-conservative-multi/`: one package with a second authored module. This
keeps the same user-facing `arcw build` route but exercises the remaining typed
conservative producer path for full-build multi-module product AWBC.

The tests run the real binary command shape:

```text
arcw build --manifest-path <fixture>/arcw.toml --target-dir <temp>/target --json
arcw cache explain <logical-item> --logical --root <temp>/target/cache/v1 --json
```

Checked-in goldens intentionally contain normalized digests and no host absolute
paths, cache roots, usernames, timestamps, or target directories.
