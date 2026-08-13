# thiserror_unpropagated_error_backtraces

## What it does

Finds local thiserror wrappers that recapture a backtrace already provided by their source or fail to
forward that source backtrace.

## Why is this bad?

Recapture adds cost and points at the wrapping boundary instead of the original failure. A plain
`#[source]` field preserves the error chain but does not forward generic backtrace requests.

## Example

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("outer")]
struct Outer {
    #[source]
    source: Inner,
    backtrace: std::backtrace::Backtrace,
}
```

## Use instead

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("outer")]
struct Outer {
    #[source]
    #[backtrace]
    source: Inner,
}
```
