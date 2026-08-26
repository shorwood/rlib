# `rlib::leptos_manual_boolean_view_selection`

## Summary

Checks for boolean Rust expressions that select between Leptos views instead of using `<Show>`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks view-producing `if`/`else`, exhaustive two-arm boolean `match`, `bool::then`, and
`bool::then_some` expressions. Method and direct associated-function syntax are handled uniformly.

## Why this matters

`<Show>` keeps the condition, visible children, and fallback together in the declarative view tree.
It also makes the condition's reactive reevaluation boundary explicit.

## Examples

### Triggers the lint

```rust,ignore
if is_configured {
    view! { <ConfiguredState /> }.into_any()
} else {
    view! { <EmptyState /> }.into_any()
}
```

### Use this instead

```rust,ignore
view! {
    <Show when=move || is_configured fallback=|| view! { <EmptyState /> }>
        <ConfiguredState />
    </Show>
}
```

## What it skips

Non-view conditionals, `if let` and let-chain payload binding, guarded or non-boolean matches,
foreign methods named `then`, and conditions already represented by `<Show>` are accepted.

## When to turn it off

Turn this lint off when a one-time Rust branch communicates intentionally nonreactive view
construction more clearly than a declarative condition.

## Settings

This lint has no behavior-specific settings.

## Known limitations

Moving branch setup into `<Show>` children can change when values are captured or constructed, so
the migration requires an ownership and lifecycle review.

## Related lints

- [`rlib::leptos_manual_optional_view_mapping`](../leptos_manual_optional_view_mapping/README.md) —
  Requires `<ShowLet>` for optional view payloads.
