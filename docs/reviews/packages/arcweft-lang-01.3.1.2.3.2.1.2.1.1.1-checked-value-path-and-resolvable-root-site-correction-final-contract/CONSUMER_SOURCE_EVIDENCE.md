# Concrete path-consumer source evidence

Commit: `36f83f8509417d1110a34f1b32aee6f4a113dcf3`.

## crates/arcweft-dialogue/src/character_dialogue/patch.rs

`crates/arcweft-dialogue/src/character_dialogue/patch.rs` around `pub struct RuntimeFieldPath`:

```text
0025: }
0026: 
0027: /// Schema-ordinal path to one runtime record leaf.
0028: #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
0029: pub struct RuntimeFieldPath(Vec<u16>);
0030: 
0031: /// Field-wise patch for a typed structured value.
0032: #[derive(Clone, Debug, PartialEq)]
0033: pub struct StructuredPatch<T> {
```

`crates/arcweft-dialogue/src/character_dialogue/patch.rs` around `RuntimeNominalRecordValue::new`:

```text
0473:             let type_id = record.type_id().clone();
0474:             let layout = record.layout();
0475:             let mut values = record.clone().into_fields();
0476:             update_fixed_values(&mut values, index, tail, replacement)?;
0477:             *value = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
0478:                 type_id, layout, values,
0479:             ));
0480:             Ok(())
0481:         }
```

`crates/arcweft-dialogue/src/character_dialogue/patch.rs` around `pub(super) fn apply_patch`:

```text
0312:         !matches!(self, Self::Unspecified)
0313:     }
0314: }
0315: 
0316: pub(super) fn apply_patch(
0317:     base: &CharacterDialogueConfig,
0318:     patch: &CharacterDialoguePatch,
0319: ) -> Result<CharacterDialogueConfig, CharacterDialogueValueError> {
0320:     patch.validate()?;
```

## crates/arcweft-runtime-driver/src/session_save.rs

`crates/arcweft-runtime-driver/src/session_save.rs` around `pub enum BundleSessionSaveError`:

```text
0189:     }
0190: }
0191: 
0192: #[derive(Clone, Debug, Error, Eq, PartialEq)]
0193: pub enum BundleSessionSaveError {
0194:     #[error("bundle session save point is not quiescent: {blockers:?}")]
0195:     NonQuiescent {
0196:         blockers: Vec<BundleSessionPendingBlocker>,
0197:     },
```

`crates/arcweft-runtime-driver/src/session_save.rs` around `InvalidRuntimeValue { path: String, message: String }`:

```text
0213:     Fiber { message: String },
0214:     #[error("invalid root-state session snapshot: {message}")]
0215:     Root { message: String },
0216:     #[error("invalid runtime value in session save at {path}: {message}")]
0217:     InvalidRuntimeValue { path: String, message: String },
0218:     #[error("invalid retained View virtualization snapshot: {message}")]
0219:     ViewVirtualization { message: String },
0220:     #[error("invalid executable View runtime snapshot: {message}")]
0221:     ViewRuntime { message: String },
```

`crates/arcweft-runtime-driver/src/session_save.rs` around `program: &AwbcProgram`:

```text
0332: }
0333: 
0334: pub(crate) fn validate_product_awbc_snapshot(
0335:     snapshot: &AwbcProductExecutorSnapshot,
0336:     program: &AwbcProgram,
0337: ) -> Result<(), BundleSessionSaveError> {
0338:     validate_fiber_snapshot("executor.product_awbc.fiber", &snapshot.fiber, program)?;
0339:     for (index, fiber) in snapshot.child_fibers.iter().enumerate() {
0340:         validate_fiber_snapshot(
```

## crates/arcweft-runtime-driver/src/view_runtime.rs

`crates/arcweft-runtime-driver/src/view_runtime.rs` around `runtime_parameters: BTreeMap<String, RuntimeValue>`:

```text
0449:     deterministic_seed: u64,
0450:     state: ViewMountState,
0451:     initialized_parameters: BTreeSet<u16>,
0452:     initialized_state: BTreeSet<u16>,
0453:     runtime_parameters: BTreeMap<String, RuntimeValue>,
0454: }
0455: 
0456: impl MountedView {
0457:     fn view(&self) -> &ViewId {
```

`crates/arcweft-runtime-driver/src/view_runtime.rs` around `root_bindings: BTreeMap<String, RuntimeValue>`:

```text
0474:     text: Option<ViewTextResource>,
0475:     inventory: ViewValueProgramInventory,
0476:     logical_time: FxLogicalTime,
0477:     allocator: ViewMountAllocator,
0478:     root_bindings: BTreeMap<String, RuntimeValue>,
0479:     mounts: BTreeMap<ViewOccurrenceKey, MountedView>,
0480:     axis_seeds: axis_seed::BundleViewAxisSeedRegistry,
0481:     required_dialogue_views: BTreeSet<ViewId>,
0482: }
```

`crates/arcweft-runtime-driver/src/view_runtime.rs` around `pub fn restore(`:

```text
0961:     #[expect(
0962:         clippy::too_many_lines,
0963:         reason = "snapshot restore preflights the complete mount table, allocator, bindings, and axis registry before one atomic commit"
0964:     )]
0965:     pub fn restore(
0966:         &mut self,
0967:         snapshot: &BundleViewRuntimeSnapshot,
0968:         reconciled_root_handles: &[PresentationHandleRecord],
0969:     ) -> Result<(), BundleViewRuntimeError> {
```

## crates/arcweft-core/src/awbc/product_step.rs

Commit-pinned web inspection shows:

```text
0121: /// Stateful canonical AWBC executor exposed through `RuntimeStepResult`.
0123: pub struct AwbcProductStepExecutor {
0124:     pub(super) program: AwbcProgram,
...
0142: impl AwbcProductStepExecutor {
0145:     pub fn replace_program_preserving_state(
0147:         program: AwbcProgram,
0149:         program.verify(...)
...
0164:     pub fn for_entry(
0165:         program: AwbcProgram,
0169:         program.verify(...)
...
0210:     pub fn for_function(
0211:         program: AwbcProgram,
0216:         program.verify(...)
...
0283:     pub const fn program(&self) -> &AwbcProgram
```

The exact commit URL and inspected line ranges are recorded in `WEB_INSPECTION_EVIDENCE.csv`. This is concrete evidence for replacing verify-only raw execution/replacement with the retained `AdmittedAwbcProduct` API in `ADMISSION_AND_PAIR_API.md`.

