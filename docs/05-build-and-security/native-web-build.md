# Native and Web build contracts

Product bundles and their executable payload are owned by
[Packaging and product bundles](packaging.md) and
[Product AWFB bytecode](product-awfb-bytecode.md). Source and development
execution use the shared
[RuntimeStep and executor contract](../02-runtime/runtime-step-and-executors.md).

## Native

The native player owns operating-system input, windows, audio, filesystem
access, and native graphics integration. Those operations stay in host
adapters; Core receives typed inputs and returns effects or host requests as
data. An optional native JIT may accelerate pure deterministic functions, but
does not replace the product AWBC execution boundary.

## Web

The browser player owns DOM integration, browser storage and fetch, WebAudio,
WebGPU, and browser permission flows. Browser-specific APIs stay behind typed
adapter contracts. A Web host may advertise only capabilities provided by its
registered adapter manifests.

Threaded Web builds require SharedArrayBuffer support and a cross-origin
isolated context. The host must detect those requirements at runtime and retain
a single-thread execution path when they are unavailable.

WebGPU compute is an asynchronous adapter boundary with typed borrowed
requests and typed responses. The API keeps operation selection typed and
avoids copying dense input buffers before dispatch. Before allocation, the
adapter validates shape, byte length, and dispatch dimensions against the
browser's default WebGPU limits. It returns structured availability and
execution outcomes for missing browser support, adapter/device failures,
capacity limits, validation errors, device loss, and readback failure; the
player selects any CPU retry through an explicit policy. GPU handles stay on
their owning worker. WebGL may provide a separate rendering path, but it is not
a compute fallback. Browser graphics builds must not link native-only graphics
backends.

Web device permission, availability, and fixture behavior follow the
[device profile](device-generator.md) and
[security policy](security.md).
