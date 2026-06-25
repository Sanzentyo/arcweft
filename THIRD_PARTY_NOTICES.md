# Third Party Notices

Arcweft Engine's own source code, documentation, tests, fixtures, and generated
demo assets are licensed under `MIT OR Apache-2.0`, unless a file states
otherwise.

Third-party code, dependencies, and assets keep their original licenses. This
file summarizes the materials that are vendored or otherwise especially visible
in this repository; Cargo registry dependencies are checked from `Cargo.lock`
and crate metadata.

## Cargo Dependencies

The current dependency graph was checked with:

```bash
cargo metadata --locked --all-features --format-version 1
cargo deny check licenses
```

The checked graph resolved 687 registry packages plus the patched local
`vendor/glyphon` package, with no missing Cargo `license` or `license_file`
metadata.

Allowed dependency license families are recorded in `deny.toml`. They include
common permissive licenses such as MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0,
Unlicense, CC0-1.0, 0BSD, BSL-1.0, and CDLA-Permissive-2.0. The graph also
contains MPL-2.0 dependencies, including the Symphonia audio crates,
`option-ext`, and `smartstring`; those packages remain under MPL-2.0 terms and
must be handled as third-party code when redistributed.

Some crate license expressions contain GPL or LGPL alternatives, such as
`self_cell` (`Apache-2.0 OR GPL-2.0-only`) and `r-efi`
(`MIT OR Apache-2.0 OR LGPL-2.1-or-later`). The permissive alternative is used
for Arcweft's dependency review.

## Vendored glyphon

`vendor/glyphon` is a vendored fork of glyphon 0.11.0. Its manifest states:

```text
MIT OR Apache-2.0 OR Zlib
```

The corresponding upstream license texts are checked in under:

```text
vendor/glyphon/LICENSE-APACHE
vendor/glyphon/LICENSE-MIT
vendor/glyphon/LICENSE-ZLIB
```

`vendor/glyphon/samples/arabic.txt` and `vendor/glyphon/samples/latin.txt` are
sample text files with Creative Commons Attribution-ShareAlike 4.0 attribution
recorded in `vendor/glyphon/samples/README.md`.

`vendor/glyphon/examples/Inter-Bold.ttf` is from the Inter font family and is
covered by the SIL Open Font License 1.1. Its notice is checked in as
`vendor/glyphon/examples/LICENSE-Inter.txt`.

## Web Demo Assets

`web/assets/arcweft-demo.ttf` is Noto Sans Regular, licensed under the SIL Open
Font License 1.1. The license text is checked in as
`web/assets/LICENSE-NotoSans.txt`.

The generated image fixtures in `web/assets` and `web/.arcweft/asset/generated`
are project-owned generated demo fixtures.
