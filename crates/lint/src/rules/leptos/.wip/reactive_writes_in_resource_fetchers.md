# Reactive writes in resource fetchers

## Proposition

Add a `reactive_writes_in_resource_fetchers` lint for asynchronous resource bodies that mutate
signals, stores, or other local reactive state as a secondary effect of loading data.

```rust
// Bad: loading User also mutates an unrelated reactive node.
let user = Resource::new(
    move || user_id.get(),
    |user_id| async move {
        let user = load_user(user_id).await?;
        selected_team.set(user.default_team);
        Ok(user)
    },
);
```

```rust
// Better: return loaded data and derive state from that result.
let user = Resource::new(move || user_id.get(), load_user);
```

Resources and suspended futures execute differently during SSR, hydration, and CSR. In particular,
the async body may not rerun during hydration because its serialized result is reused. A signal
write hidden inside that body therefore cannot be assumed to occur in every execution stage.

Reference: [Asynchronous Closures and Futures](https://book.leptos.dev/server/28_async_quick_reference.html).

## Conservative detection

- Identify closures or async blocks used as `Resource`, `LocalResource`, or configured resource
  fetchers.
- Resolve writes through Leptos signal and store APIs.
- Follow captures through simple helper closures and async blocks.
- Diagnose writes that are not part of constructing the returned resource value.
- Increase severity for values later used to construct the hydrated view tree.
- Report one finding per resource with all affected reactive targets listed.

## Side effects and data loading

The network or database operation that produces the resource value is expected. The problematic
behavior is secondary mutation whose correctness depends on the fetcher executing in a particular
stage or order.

Logging, metrics, cache warming, and idempotent external actions still have execution-stage
semantics and should not be assumed safe merely because they do not write a signal. This lint can
start with reactive writes because they are reliably identifiable and directly threaten view
consistency.

## Diagnostic direction

- Return all loaded values as one domain object.
- Derive presentation state through a closure or memo.
- Move event-driven mutation to the event that initiated the resource change.
- Use an explicitly client-only effect when synchronization with a browser system is intended.
- Avoid suggesting another effect merely to reproduce reactive-to-reactive synchronization.

## Open decisions

- Whether `LocalResource` receives the same rule despite being client-only.
- Whether writes to non-view cache signals are configurable exemptions.
- How a project declares idempotent resource instrumentation.
- Whether mutation through context is treated identically to direct signal writes.
- Whether resource bodies with any side effect deserve a broader companion lint.
