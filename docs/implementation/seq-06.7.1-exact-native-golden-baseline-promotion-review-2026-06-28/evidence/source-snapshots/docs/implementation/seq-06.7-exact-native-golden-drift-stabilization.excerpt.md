# Seq06.7 source note excerpt

Seq06.7 deliberately did not update checked-in native golden PNGs. It introduced a reviewed workflow for exact native golden drift: capture a candidate PNG, retain observe JSON, retain `imq` JSON, retain an environment fingerprint, and classify the result before accepting or rejecting a baseline.

Known seq06.6 `vertical_tutr_golden` drift:

- dimensions: 1280x720
- mse: 0.0030918550895167305
- mae: 0.004233718228315644
- gates: mse <= 0.002, mae <= 0.003

Seq06.7 classified this as `baseline_drift`, not as a malformed artifact or hard capture failure, but it did not prove that the checked-in reference was stale because seq06.6 lacked OS version, backend, font probe, commit/source hash, or `imq` version evidence.
