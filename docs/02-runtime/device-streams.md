# Device Stream Runtime Boundary

Device and capture input enters the language through an admitted external
capability that returns `Stream<T, E>`. The capability host owns device
permission, lifecycle, timestamp normalization, privacy, replay, and buffering
policy. Source generators may transform a granted stream, but cannot enumerate
or open hardware themselves; the language-level generator rules live in
[Streams, Generators, and External Capabilities](streams-generators.md).

Core receives typed stream values and events through the Sans-I/O
[`RuntimeStep` contract](runtime-step-and-executors.md). It does not own device
handles, backend callbacks, or permission prompts.

Device-specific policy is owned by the relevant [Device Profile
Manifest](../schemas/device-profile-manifest.md), [Device I/O
Manifest](../schemas/device-io-manifest.md), [USB Device
Manifest](../schemas/usb-device-manifest.md), [Virtual Controller
Manifest](../schemas/virtual-controller-manifest.md), and [Capture Device
Manifest](../schemas/capture-device-manifest.md), together with the
[layered input](layered-input.md), [Microphone / Camera Capture
Devices](../03-presentation/capture-devices.md), and [Security and sandbox
policy](../05-build-and-security/security.md).
