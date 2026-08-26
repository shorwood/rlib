# `rlib::leptos_manual_optional_view_mapping`

## Summary

Checks for standard `Option` mapping operations that construct Leptos views instead of using `<ShowLet>`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks `Option::map`, `Option::map_or`, and `Option::map_or_else` when the mapped value has an
explicit `impl IntoView`, `View`, or `AnyView` contract. Method and direct associated-function
syntax are handled uniformly.

## Why this matters

`<ShowLet>` exposes optional presence, the bound payload, rendered children, and fallback as one
declarative view structure without requiring branch-wide type erasure.

## Examples

### Triggers the lint

```rust,ignore
authenticator.map_or_else(
    || view! { <EmptyMfaState /> }.into_any(),
    |authenticator| view! { <ConfiguredMfaState authenticator /> }.into_any(),
)
```

### Use this instead

```rust,ignore
view! {
    <ShowLet some=authenticator let:authenticator fallback=|| view! { <EmptyMfaState /> }>
        <ConfiguredMfaState authenticator />
    </ShowLet>
}
```

## What it skips

Mappings that produce data rather than explicit views, `Result` mappings, foreign methods with the
same names, `match` and `if let`, and views already represented by `<ShowLet>` are accepted.

## When to turn it off

Turn this lint off when consuming an option exactly once is more important than representing its
presence in the view tree.

## Settings

This lint has no behavior-specific settings.

## Known limitations

Static `<ShowLet>` inputs require their payload to satisfy `Clone + Send + Sync + 'static`. The lint
still reports nonconforming owned payloads because migration may require shared ownership or a
reactive owner rather than a local syntax rewrite.

## Related lints

- [`rlib::leptos_manual_boolean_view_selection`](../leptos_manual_boolean_view_selection/README.md) —
  Requires `<Show>` for boolean view selection.
