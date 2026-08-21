# Static certification

Delete stale `CheckedViewDynamicReason::DirectAwait`; add:

```rust
LiveNeedSubscription {
    subscription: CheckedViewNeedSubscriptionKey,
}
```

A live subscription is dynamic even when current state is Ready, producer is
memoized, or all arm bodies are constant. It can change selected structure and
contaminates the match and static ancestors.

The parent's deterministic source-order DFS selects the scrutinee before arm
bodies. Without authored `#[static]`, publish Dynamic evidence. With it, after
ordinary semantic/ownership validity, emit
`sema.view.static.required_dynamic`, primary at attribute and related at exact
scrutinee. No Await-specific diagnostic or fallback.

A live-subscription subject has no static certificate/fragment. A tampered
certificate claiming one is static fails strict dependency/certificate validation
before runtime publication.
