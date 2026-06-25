# Audio CPAL v3 Implementation Note

Source package: `D:/sanze/Downloads/arcweft-audio-cpal-final-v3.zip`

Baseline revision when this slice was integrated: Git `a4bfc340c`, Jujutsu
change `yvwnuprmwxto` / commit `47b5bf51172c`.

## Implemented

- Added typed audio crates:
  - `arcweft-audio-core` for audio graph validation, prepared commands, decoded
    PCM, and graph-owned errors.
  - `arcweft-audio-codec` for Symphonia-backed bundle audio decoding and cubic
    resampling.
  - `arcweft-audio-mixer` for deterministic stereo voice, bus, snapshot, and
    effect processing.
  - `arcweft-audio-device-cpal` for native CPAL output and microphone capture
    device boundaries.
- Added typed runtime audio command flow:
  - `RuntimeAudioCommand` lives in `arcweft-core`.
  - runtime-plan lowers `audio.*` calls into `LineEffectRequest::Audio`.
  - engine evaluation emits `AudioCommandEnvelope` host requests.
  - runtime-host and runtime-driver expose audio commands as typed step output.
- Added bundle support:
  - bundle schema version is now 3.
  - `ArcweftBundle` can carry an `AudioGraph`.
  - bundle audio asset bytes resolve through typed audio resource ids.
- Added native player CPAL integration:
  - windowed native bundle execution prepares the bundle audio graph.
  - declared audio assets are decoded, resampled to the output device rate, and
    installed into the realtime mixer.
  - runtime `AudioCommandEnvelope` values are prepared and submitted to CPAL.
  - CPAL/mixer `AudioEvent` values are routed back into the next runtime step.
- Added Web-facing groundwork:
  - `web/arcweft-microphone-worklet.js` contains an AudioWorklet processor for
    microphone frames.
  - `arcweft-player-web` reports emitted audio command counts.

## API Placement

Enum-owned behavior was preferred where the enum is part of the Arcweft
workspace. Examples in this slice include:

- `RuntimeAudioCommand::operation_name`.
- `AudioPlaybackEndReason::as_str`.
- audio value newtypes and parameter kind validation in
  `arcweft-interaction-model`.

No compatibility JSON fallback or stringly request shim was added for the audio
path. Runtime, host, bundle, mixer, and device boundaries use typed structs and
enums.

Small helpers that only wrapped `Err(...)` were not introduced. Existing
localized conversion points either return structured errors directly or map
device/setup failures to typed `AudioFailure` events at the host boundary.

## Validation

Commands run from `D:/git/arcweft`:

```bash
cargo check -p arcweft-player-native -p arcweft-runtime-host
cargo clippy -p arcweft-interaction-model -p arcweft-audio-core -p arcweft-audio-codec -p arcweft-audio-mixer -p arcweft-audio-device-cpal -p arcweft-runtime-plan -p arcweft-core -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-runtime-host -p arcweft-player-web -p arcweft-player-native --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-player-native -p arcweft-audio-device-cpal --all-targets --all-features -- -D warnings
cargo test -p arcweft-audio-core -p arcweft-audio-mixer -p arcweft-bundle -p arcweft-runtime-plan -p arcweft-player-native audio -- --nocapture
cargo test -p arcweft-audio-core -p arcweft-audio-codec -p arcweft-audio-mixer -p arcweft-audio-device-cpal
cargo test -p arcweft-player-native
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audit-audio-cpal-v3
```

Results:

- Targeted check: passed.
- Targeted clippy with `-D warnings`: passed.
- Audio-filtered tests: passed; 2 matching tests ran
  (`bundle_audio_graph_round_trips_and_resolves_asset_bytes`,
  `runtime_plan_lowers_audio_call_to_typed_audio_effect`).
- Unfiltered audio crate tests and doc-tests: passed.
- Native player tests: passed.
- Structural audit: 0 errors, 87 warnings.

The structural audit output is checked in under
`docs/implementation/structure-audit-audio-cpal-v3/`.

## Boundaries And Known Follow-Up

- Native CPAL output is connected for windowed `.awfb` execution when a bundle
  contains an audio graph and the host has a default output device.
- Native CPAL microphone request/stop commands are connected in the windowed
  player, and captured sample levels are returned as typed `CaptureLevel`
  events.
- Headless bundle execution exposes audio command counts and typed requests but
  does not open a CPAL device.
- Browser WebAudio output coordination is not part of this CPAL device slice.
  The web worklet and command reporting are present, but a full browser
  `AudioContext` coordinator should be implemented in the web player when the
  browser audio device target is scheduled.
- Capture monitor routing to an audio bus still returns a typed
  `CommandFailed` event because the mixer does not yet expose a live input bus
  endpoint.
