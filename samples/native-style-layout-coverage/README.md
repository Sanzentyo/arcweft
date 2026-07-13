# Native Style layout coverage sample

This sample is separate from `native-style-parity`. It keeps typed View layout
and Style application coverage independent from text-raster parity.

Run the focused source and bundle check from the repository root:

```bash
just native-style-layout-coverage
```

The source uses only native Style sheets, typed tokens, typed selectors, and
native declarations. No external stylesheet or adapter-specific fixture is
part of this route.
