# `rlib::axum_ad_hoc_response_wrappers`

## Summary

Finds synchronous Axum response-producing functions and methods whose rendering behavior should be owned by an `IntoResponse` implementation.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::axum` |
| Cargo feature | `axum` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks authored synchronous free functions, inherent methods, and locally declared trait methods
returning Axum's default `Response` or `impl IntoResponse`. Standard `Result` outputs are inspected
through nested `Result` and `Option` branches so adding fallibility does not hide a response helper.
Local declarative macro output remains in scope.

## Why this matters

Named response helpers scatter one response contract across functions and methods that Axum cannot
discover through a bound. An `IntoResponse` implementation assigns rendering to a type, lets handlers
return that type directly, and stores required response context with the value being rendered.

## Examples

### Triggers the lint

```rust,ignore
struct Events(Vec<Event>);

impl Events {
    fn into_sse_response(self, delay: Duration) -> Response {
        render_events(self.0, delay)
    }
}
```

### Use this instead

```rust,ignore
struct Events {
    events: Vec<Event>,
    delay: Duration,
}

impl IntoResponse for Events {
    fn into_response(self) -> Response {
        render_events(self.events, self.delay)
    }
}
```

## What it skips

Asynchronous handlers and middleware, closures, external trait obligations, unsafe, constant, and
foreign-ABI functions, external macro expansions, build output, top-level optional responses,
collections, ordinary concrete responders, and HTTP responses with a non-Axum body are accepted.

## When to turn it off

Turn this lint off when a synchronous response helper intentionally owns orchestration that cannot
be represented by a response value without obscuring its lifecycle or failure semantics.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The concrete response check recognizes Axum's default body type. Functions returning custom HTTP
response bodies remain outside this rule even when another layer later converts them for Axum.

## Related lints

None.
