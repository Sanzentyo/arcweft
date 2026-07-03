# CSS layout/cascade coverage sample

This sample is separate from `css-style-parity`. It exists to keep the seq06.12
coverage fixture focused on retained UI layout/cascade decisions rather than text
raster parity.

The paired CSS and evidence fixtures are under:

```text
fixtures/css-layout-cascade-coverage/
```

The fixture intentionally includes both supported and unsupported CSS so the
coverage path proves that unsupported features are diagnostic-driven.
