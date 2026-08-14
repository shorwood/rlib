# leptos_hydration_divergent_views

## What it does

Finds compile-time server/browser branches that author different initial `view!` node shapes,
including element nesting and text-node presence.

## Why is this bad?

Hydration walks browser DOM expecting the node sequence produced during server rendering. Different
initial element shapes can make hydration attach state and listeners to the wrong nodes or fail.

## Example

```rust,ignore
let toolbar = if cfg!(target_arch = "wasm32") {
    view! { <ClientToolbar/> }
} else {
    view! { <ServerPlaceholder/> }
};
```

## Use instead

Render one stable initial shape, then perform browser-only work after hydration:

```rust,ignore
let toolbar = view! { <Toolbar/> };
Effect::new(move |_| initialize_browser_toolbar());
```
