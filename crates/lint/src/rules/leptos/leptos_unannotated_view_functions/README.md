# `rlib::leptos_unannotated_view_functions`

## Summary

Checks for authored free functions that expose an explicit Leptos view contract without `#[component]`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks ordinary synchronous free functions returning `impl IntoView`, `View`, or `AnyView`,
including aliases and functions that only forward an already-built view.

## Why this matters

`#[component]` gives a view-producing boundary component identity, declarative props, and an
uppercase tag that remains visible in its parent's view tree.

## Examples

### Triggers the lint

```rust,ignore
fn mfa_status_rows(status: MfaStatus) -> AnyView {
    view! { <MfaRows status /> }.into_any()
}
```

### Use this instead

```rust,ignore
#[component]
fn MfaStatusRows(status: MfaStatus) -> impl IntoView {
    view! { <MfaRows status /> }
}
```

## What it skips

Components, islands, methods, closures, asynchronous, constant, unsafe, and foreign-ABI functions,
generated source, and concrete return types that are only renderable through blanket traits are
accepted.

## When to turn it off

Turn this lint off when a view-returning function is deliberately an implementation helper whose
positional Rust call interface should remain hidden from markup.

## Settings

This lint has no behavior-specific settings.

## Known limitations

Adding `#[component]` generates a PascalCase entry point and a props interface, so existing direct
callers and public APIs require a manual compatibility review.

## Related lints

- [`rlib::leptos_overpopulated_component_modules`](../leptos_overpopulated_component_modules/README.md) —
  Limits the number of component boundaries owned by one module.
