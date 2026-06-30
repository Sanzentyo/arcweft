# CSS-style text raster parity fixture evidence

This fixture directory documents the expected evidence files for Seq06.10. The
files are not golden PNGs. They describe the default, compact, and HiDPI evidence
paths generated under `target/css-style-parity/` by `just css-style-parity`.

The text-raster checker consumes native/Web PNGs plus native/Web frame JSON and
writes one `text-raster-<checkpoint>.json` report per checkpoint.
