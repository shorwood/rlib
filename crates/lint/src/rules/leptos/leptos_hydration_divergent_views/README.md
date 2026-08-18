# `rlib::leptos_hydration_divergent_views`

## Summary

Finds compile-time server/browser branches that author different initial `view!` node shapes, including element nesting and text-node presence.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds compile-time server/browser branches that author different initial `view!` node shapes,
including element nesting and text-node presence.

## Why this matters

Hydration walks browser DOM expecting the node sequence produced during server rendering. Different
initial element shapes can make hydration attach state and listeners to the wrong nodes or fail.

## Examples

### Triggers the lint

```rust,ignore
let toolbar = if cfg!(target_arch = "wasm32") {
    view! { <ClientToolbar/> }
} else {
    view! { <ServerPlaceholder/> }
};
```

### Use this instead

Render one stable initial shape, then perform browser-only work after hydration:

```rust,ignore
let toolbar = view! { <Toolbar/> };
Effect::new(move |_| initialize_browser_toolbar());
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
