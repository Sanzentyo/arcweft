# seq06.9c compositing capture evidence

`expected-evidence.json` is the canonical serialization of a typed
`TakumiCompositingCaptureRecord`. The integration test compares the complete
generated packet with this file, including stable ids, bounds, masks, blend
mode, and effect outsets.

This fixture does not claim pixel coverage. Exact image capture belongs to a
pinned renderer/GPU lane with a real capture implementation, not to a test that
only inspects repository text.
