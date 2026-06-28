# Missing candidate PNG

Expected path from the pinned Windows run:

```text
target/arcweft-native-capture-artifacts/vertical_tutr_golden.candidate.png
```

Status: not generated in this package.

Reason: package-generation environment is not the required Windows native capture
environment and did not run `just native-visual-artifacts`.

Do not substitute the checked-in reference PNG or a locally generated unpinned
image here. A promotion packet must include the real candidate PNG from the same
pinned Windows run as the observe JSON, `imq` JSON, and environment fingerprint.
