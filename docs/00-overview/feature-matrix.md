# 機能マトリクス

| 機能 | MVP | Advanced | Product Flag |
|---|---:|---:|---:|
| DSL parser / CST | ✅ |  | always |
| EntityId / Ref / LSP inlay | ✅ |  | dev |
| Option / Result / Need | ✅ |  | always |
| flow / reducer / view | ✅ |  | always |
| contracts runtime check | ✅ |  | dev/test/product selective |
| Z3/OxiZ formal verification |  | ✅ | dev/ci |
| typed cursor / codec APIs | ✅ |  | always |
| macro/template |  | ✅ | build |
| precompile |  | ✅ | build |
| wgpu headless screenshot | ✅ |  | dev/test/product observe |
| object-id mask | ✅ |  | dev/test/product observe |
| Game Native View | ✅ |  | always |
| Servo HTML View |  | ✅ | native feature |
| DOM HTML View |  | ✅ | web feature |
| Vector IR | ✅ |  | always |
| SVG import |  | ✅ | build |
| RichText | ✅ |  | always |
| Typst bridge |  | ✅ | optional |
| WGSL custom shader | ✅ |  | feature |
| shader hot reload |  | ✅ | dev/debug |
| Audio basic playback | ✅ |  | always |
| BGM stem/adaptive |  | ✅ | feature |
| Spatial audio |  | ✅ | feature |
| TTS |  | ✅ | provider/capability |
| BGM authoring/precompose |  | ✅ | build/dev |
| Device streams / Source<T,E> | ✅ |  | always |
| USB raw device access |  | ✅ | capability + auth |
| HID device access |  | ✅ | capability + auth |
| Gamepad input | ✅ |  | always |
| Touch virtual controller | ✅ |  | touch devices |
| Agent Debug Bus | ✅ |  | product flag |
| MCP server |  | ✅ | product flag + auth |
| Jujutsu history |  | ✅ | dev/tooling |
| GraphRAG |  | ✅ | dev/tooling |
| WASM plugin |  | ✅ | feature |
| Rust Activity static | ✅ |  | feature |
| Rust dylib plugin |  | ✅ | native dev |
| out-of-process IPC |  | ✅ | native feature |
| Cranelift JIT |  | ✅ | native feature |


## Microphone / Camera Capture

| Feature | Native | Web | Notes |
|---|---|---|---|
| Audio output | CPAL | CPAL wasm/WebAudio where viable | AudioWorklet requires atomics/SAB deployment. |
| Microphone input | CPAL | web-sys MediaDevices bootstrap + CPAL/custom bridge | Permission-gated. |
| Camera input | shiguredo_video_device preferred; nokhwa optional | web-sys MediaDevices | Product mode requires indicator and capability. |
| Virtual capture | yes | yes | CI/headless deterministic tests. |

## Device / controller additions

| Feature | Native | Web | Headless | Notes |
|---|---:|---:|---:|---|
| USB/HID device profiles | yes | WebUSB-limited | fixture | `nusb` preferred, `rusb` compatibility, `hidapi` for HID |
| Device profile generator | yes | yes | yes | Generates typed parsers, writers, signals, tests, Agent metadata |
| Virtual touch controller | yes | yes | yes | Native View on `@layer.controls` |
| Physical/virtual controller map | yes | yes | yes | Merges touch, keyboard, gamepad, USB/HID |

## Device I/O / USB / Touch Controller

| Feature | Native | Web | Notes |
|---|---|---|---|
| Raw USB | `nusb` default, `rusb` optional | WebUSB via `web-sys` | Permissioned, product-disabled by default. |
| HID | `hidapi` optional | WebHID via `web-sys` | Use narrow filters and typed parsers. |
| Standard gamepad | `gilrs` | Gamepad API via `web-sys` | Maps into `InputAction` / axes. |
| Touch virtual controller | Game Native View | Game Native View / browser touch | Emits same `InputAction`s as physical controllers. |
| Stream/generator sugar | yes | yes | Generators transform granted streams; they do not open devices. |

## Device I/O / Touch Controllers

| Feature | Native | Web | Notes |
|---|---|---|---|
| Raw USB | nusb preferred, rusb fallback | WebUSB | Permission-gated and allowlisted in product. |
| HID | hidapi | WebHID | For special controllers and input reports. |
| Serial | native serial backend | Web Serial | USB CDC and microcontroller-style devices. |
| Gamepad | native gamepad abstraction | Gamepad API | Prefer high-level controller route when possible. |
| Touch virtual controller | Game Native View | Game Native View / DOM pointer input | Agent-visible and replayable as logical events. |
| generator `fn` / source block | yes | yes | An own-scope-`yield` `fn -> Stream<T,E>` transforms; `source @id: Source<T,E>` declares live policy-backed inputs. |


## Device and controller features

| Feature | Native | Web | Headless/Test | Notes |
|---|---:|---:|---:|---|
| USB raw devices | nusb / rusb | WebUSB via web-sys | virtual fixtures | permissioned, declared profiles only |
| HID devices | hidapi / optional async-hid | WebHID via web-sys | virtual fixtures | game controllers, pedals, custom panels |
| Serial / USB serial | serialport | Web Serial via web-sys | fixture streams | typed decoder function required |
| Virtual touch controller | Game Native View | Game Native View / DOM bridge optional | semantic action | attached to input layers |
| Device generator | yes | emits web descriptors | emits fixtures | deterministic precompile output |


## USB / HID / Virtual Controller

| Feature | Native | Web | Notes |
|---|---|---|---|
| Raw USB | `nusb` preferred, `rusb` optional | WebUSB through `web-sys` | Custom devices only; product mode is capability-gated. |
| HID | `hidapi`/native HID backend optional | WebHID through `web-sys` | Prefer `gilrs`/Gamepad API for standard controllers. |
| Gamepad | `gilrs` | browser Gamepad API / `web-sys` | Normalized to `InputAction`. |
| Virtual touch controller | Game Native View | Game Native View / DOM overlay metadata | Emits same `InputAction` as physical input. |
| Source streams | yes | yes | Explicit backpressure and replay policy. |
