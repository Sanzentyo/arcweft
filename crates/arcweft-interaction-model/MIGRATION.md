# Interaction model migration

## Workspace

Add `crates/arcweft-interaction-model` to workspace members and workspace dependencies.

## Presentation

- rename current `arcweft_presentation::input::InputEvent` to `RoutedInputEvent`
- adapt its variant data to `arcweft_interaction_model::InputEventKind`
- retain platform/raw input types inside presentation or host adapters
- emit `InputEpoch` and `InputSequence` from the router

## Core

Replace:

```rust
pub input_events: Vec<InputEvent>,
pub audio_events: Vec<AudioEvent>,
```

with:

```rust
pub input_events: Vec<arcweft_interaction_model::RoutedInputEvent>,
pub host_events: arcweft_interaction_model::HostEventBatch,
```

Update `RuntimeStepInputRef` in the same change. Delete the old stringly structs; do not retain aliases.

## Runtime host

Translate typed presentation actions and audio callbacks at one explicit adapter boundary. Preserve epoch, sequence, target, and payload in tests.
