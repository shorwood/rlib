# thiserror_unpropagated_error_backtraces

## What it does

Finds local thiserror wrappers that recapture a backtrace already provided by their source or fail to
forward that source backtrace.

## Why is this bad?

Recapturing adds cost and points at the wrapper instead of the original failure. A plain
`#[source]` field keeps the error chain, but tools still cannot retrieve the source's backtrace
through the wrapper.

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
