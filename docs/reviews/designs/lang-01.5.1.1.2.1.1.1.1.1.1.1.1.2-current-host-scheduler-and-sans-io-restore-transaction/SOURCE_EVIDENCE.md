# Current source evidence

## 1. Repository state

- Full inspected Git commit:
  `9168c8ac7285c6b44f29018626a0e7c1b0059796`
- Branch: `main`
- Upstream display: `main...origin/main`
- Working tree at design start: dirty only with unrelated untracked review
  intake notes, retained ZIPs, and extracted package mirrors. This design did
  not rewrite or remove them.
- Production files changed by this design: none.

## 2. Current owner evidence

| Current path | Git blob | Evidence used |
|---|---|---|
| `crates/arcweft-core/src/task.rs` | `c0a8ca5fb2fc4f184f3f494281883c1878d5ddc6` | current `GenerationId`, producer/catalog substrate, `TaskEventKind::{Ready,Failed,Cancelled,Progress}`, and narrow but insufficient `TaskHost::{ensure_task,cancel_scope,poll_frame}` owner |
| `crates/arcweft-core/Cargo.toml` | `22091bfdea72b6168576181b2cc62fdd25a75ada` | core has no scheduler, driver, host, save, filesystem, or async I/O dependency |
| `crates/arcweft-runtime-scheduler/src/lib.rs` | `c80e120ef922f3178de3d78501e0ce9c0ccc9fe4` | current Sans-I/O `RuntimeScheduler` owns pending/in-flight/join/cancel maps and deterministic dispatch/event normalization, but no journal, adapter transaction, snapshot, or restore typestate |
| `crates/arcweft-runtime-scheduler/Cargo.toml` | `28068bdb0ac30304abeb57e76661225deaee0a9a` | scheduler depends only on core; final protocol must preserve that edge |
| `crates/arcweft-host-adapter/src/lib.rs` | `5367620b6062d9e1b9beec9f5c6f63ef4190cfb5` | current `HostAdapter` performs immediate submit/completion/boolean cancel through a registry; accepted parent schedules its deletion in favor of prepare/commit/rollback |
| `crates/arcweft-host-adapter/Cargo.toml` | `04ff7503e718035252ba1764af1436024bfda494` | adapter crate depends on core and can implement a core-owned protocol without a cycle |
| `crates/arcweft-runtime-host/src/native_task.rs` | `89a88445188e7f9a348779c4b00f8d73c72c4532` | `NativeTaskBridge` currently owns `HostAdapterRegistry` and `RuntimeScheduler` separately and is the concrete composition migration point |
| `crates/arcweft-runtime-host/src/bundle_runner/session.rs` | `8f4333d7fd6c126255b149939243882f8bc53354` | current headless bundle-runner session already owns one `NativeTaskBridge`; the final model reuses it rather than inventing a second headless scheduler owner |
| `crates/arcweft-runtime-host/Cargo.toml` | `fb6bd1c3b0f4c6eb34a5a8ca7842f2cecd867717` | runtime-host already depends on core, host-adapter, and scheduler |
| `crates/arcweft-runtime-driver/src/task.rs` | `928601cee0dc24cc636db9187159332c85fedd3e` | `RuntimeTaskRegistry` is a second lifecycle/event owner and deletion target |
| `crates/arcweft-runtime-driver/src/session.rs` | `10572c7226948075d03725b6a80841ab94130d64` | `BundleSession` stores registry and task-generation pins, applies events there, creates `HostTaskDispatch`, and returns requested tasks/cancel scopes instead of borrowing `TaskHost` |
| `crates/arcweft-runtime-driver/src/session/persistence.rs` | `7e484089e5b49b8798e0b8f06dd511eb0bf25ade` | current restore validates detached session copies then swaps fields, but resets `RuntimeTaskRegistry`; outer pure after-image pattern is reusable while task authority moves out |
| `crates/arcweft-runtime-driver/Cargo.toml` | `4cac7e66512b8d652e72e2ceef5ae92fd673e3dc` | driver currently depends on core/save and has no scheduler/host-adapter dependency; this direction is retained |
| `crates/arcweft-player-web/src/host.rs` | `fe57dc4f3258fbf059802ff85b2b24d1afdeae6a` | `BrowserTaskBroker` currently owns duplicated cancelled-scope/event queues and imports driver `HostTaskDispatch`; final composition removes that authority leak |
| `crates/arcweft-player-web/Cargo.toml` | `fc4d39a13014df2e5fe20c0b625d363b1d600644` | Web player owns application/session composition and may depend directly on scheduler; scheduler must not depend back on it |

`cargo metadata --no-deps --format-version 1` completed with exit code `0`
during this design audit. It was used only to confirm the current workspace
manifests parse; no production check/test result is claimed.

## 3. Accepted parent authority

The accepted design at
`docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-launch-receipt-keyed-ordinal-and-current-owner/`
is the retained semantic parent.

Its normative evidence establishes:

- `RuntimeGenerationJournal::apply_after_image` owns the atomic core swap;
- `JournalTransaction` and `SealedJournalAfterImage` are the only staging/apply
  path;
- journal revision is not persisted and restore revision starts at zero;
- adapter prepare occurs before apply;
- apply failure rolls adapter tokens back;
- scheduler runtime after-image swap is infallible;
- adapter commit follows both core and scheduler apply and is infallible;
- no fallible step exists after journal apply; and
- handles/accepted receipts are observable only after apply/commit.

The parent schema's method-generic `RuntimeScheduler` coordinator was the exact
constructibility gap addressed by the original coordinator request. This
design changes the owner to `RuntimeTaskScheduler<A>` while retaining its
transaction transcript.

## 4. Rejected package evidence

The rejected returned package is retained under
`docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-task-coordinator-and-two-phase-restore-correction-final-contract/`.
It recorded no inspected SHA and targeted nonexistent
`crates/arcweft-runtime/src/task/*` paths. Its `TaskPersistence` API and durable
PREPARED/COMMITTED sequence are historical review evidence only; no type,
method, state, or compatibility rule from it is an implementation authority.

## 5. Maintained runtime evidence

The accepted parent already selected maintained
`docs/02-runtime/async-scheduler.md`, `need-timeout.md`, and
`executable-runtime-core.md` for scheduler/event/timeout/safe-point rules. This
correction keeps deterministic event ordering, Sans-I/O execution, logical
time, version `1`, and typed failures. It changes no numeric AWBC allocation.
