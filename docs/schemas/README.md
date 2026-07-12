# schemas

Schemas describe Arcweft data formats, not I/O APIs. A schema may define JSON,
TOML, binary, or manifest shape, but reading from files, writing files, network
fetch, bundle embedding, and platform storage belong to CLI/build/player
adapters. Schema crates should expose typed data and deterministic validation
over strings or byte slices.

- [Agent Protocol](agent-protocol.md)
- [GraphPatch](graph-patch.md)
- [Adapter Manifest](adapter-manifest.md)
- [Module Manifest](module-manifest.md)
- [Audio Manifest](audio-manifest.md)
- [Layer Manifest](layer-manifest.md)
- [Character Manifest](character-manifest.md)
- [Layer Tree](layer-tree.md)
- [Hook Manifest](hook-manifest.md)
- [Memo Cache](memo-cache.md)
- [Capture Device Manifest](capture-device-manifest.md)
- [USB Device Manifest](usb-device-manifest.md)
- [Device Manifest](device-manifest.md)
- [Device I/O Manifest](device-io-manifest.md)
- [Device Profile Manifest](device-profile-manifest.md)
- [Virtual Controller Manifest](virtual-controller-manifest.md)
- [Dialogue Line Manifest](dialogue-line-manifest.md)
- [Localization Catalog](localization-catalog.md)

- [Dialogue View Manifest](dialogue-view-manifest.md)
