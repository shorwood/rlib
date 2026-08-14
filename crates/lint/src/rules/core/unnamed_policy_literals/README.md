# unnamed_policy_literals

## What it does

Finds numeric literals and literal-derived expressions that control retries, timing,
capacities, concurrency, pagination, truncation, or failure-related thresholds. Evidence
comes from semantically resolved standard and ecosystem APIs, policy fields on explicit
configuration values, and control flow around fallible operations. Immutable local aliases
are followed back to their literal initializer.

Exact `0` and `1`, direct indexing, constant and static initializers, unrelated arithmetic,
infallible loop bounds, ordinary comparisons, success-only `Result` branches, and
macro-generated expressions remain accepted. User-defined APIs that merely share a familiar
method name are not classified as policy sinks.

## Why is this bad?

A behavioral literal exposes a value without explaining its authority. Reviewers and tools
cannot distinguish a convenient test value from a production retry contract, determine
whether two equal values express the same policy, or discover where that policy should be
changed. Generated code commonly repeats such values instead of identifying the domain
decision they encode.

This function contains two invisible delivery policies:

## Example

```rust
# use std::time::Duration;
# async fn sleep(_: Duration) {}
# async fn deliver() -> Result<(), ()> { Ok(()) }
# async fn example() -> Result<(), ()> {
for _attempt in 0..3 {
    sleep(Duration::from_millis(250)).await;
    deliver().await?;
}
# Ok(())
# }
```

## Use instead

Give each decision a domain-specific constant, even when it currently has only one use:

```rust
# use std::time::Duration;
const MAX_DELIVERY_ATTEMPTS: usize = 3;
const DELIVERY_RETRY_DELAY: Duration = Duration::from_millis(250);
# async fn sleep(_: Duration) {}
# async fn deliver() -> Result<(), ()> { Ok(()) }
# async fn example() -> Result<(), ()> {
for _attempt in 0..MAX_DELIVERY_ATTEMPTS {
    sleep(DELIVERY_RETRY_DELAY).await;
    deliver().await?;
}
# Ok(())
# }
```

Configuration syntax does not replace policy ownership. Prefer
`RetryPolicy { max_attempts: MAX_DELIVERY_ATTEMPTS }` over embedding `3` directly in the
field. The diagnostic intentionally provides guidance rather than an automatic fix because
choosing the constant's domain vocabulary and ownership scope requires human judgment.
