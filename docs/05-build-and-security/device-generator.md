# Device Generator / Profile System

Device profile generation is a deterministic build-time transformation. It
does not define runtime device semantics or grant access. The validated profile,
host capability, and typed stream or capture contract remain authoritative.

Generation accepts authored device declarations and manifests, plus device
descriptors, protocol/parser definitions, capture manifests, and virtual
controller manifests where applicable. The canonical data shapes are owned by
the [Device Profile Manifest](../schemas/device-profile-manifest.md),
[Device I/O Manifest](../schemas/device-io-manifest.md), and
[Capture Device Manifest](../schemas/capture-device-manifest.md).

For supported inputs, generation must validate references, filters, bounds,
parser contracts, permissions, and product policy before producing artifacts.
It may produce validated profile records, typed IDs and port/event bindings,
parser or contract metadata, source-mapped adapter stubs, virtual fixtures,
browser permission descriptors, and Agent/LSP summaries. Every generated
artifact must be traceable to its authored declaration or manifest; generation
must not create a parallel identity, permission, or runtime authority.

Generated outputs must be deterministic for the same validated inputs. The
authored declarations and manifests remain the source of truth. A project may
check selected generated files into source control; untracked generated caches
and outputs are derived data and must be reproducible.

Manual adapter implementations remain valid for protocols that need custom
native code, timing-sensitive behavior, or platform SDK integration. They use
the same validated profile and granted typed ports as generated adapters, and
must not enumerate devices or open them outside the host permission boundary.
Runtime stream and backpressure rules are owned by
[Device Streams](../02-runtime/device-streams.md) and
[Streams, Generators, and Live Device Sources](../02-runtime/streams-generators.md).

For Web targets, generated permission descriptors must reflect the profile's
device filters and browser requirements. Unsupported or unavailable browser
APIs must use a virtual fixture path or leave the capability disabled. Agent
metadata may expose typed summaries and simulated fixtures; raw device payloads
remain private under the [Security and sandbox policy](security.md).
