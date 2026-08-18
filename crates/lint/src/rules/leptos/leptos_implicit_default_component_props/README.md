# `rlib::leptos_implicit_default_component_props`

## Summary

Checks for Leptos component properties that use `#[prop(optional)]` with a concrete value type.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for Leptos component properties that use `#[prop(optional)]` with a concrete value
type. Properties using `Option<T>` are accepted because omission can be meaningful state.

## Why this matters

`#[prop(optional)]` fills an omitted property with its type's implementation of `Default`.
That generic default may not describe the component's intended behavior, and a later change to
the type can silently change the component API. Writing the default at the property declaration
makes the behavior visible to readers and callers.

## Examples

### Triggers the lint

```rust,ignore
#[component]
fn ProgressBar(
    #[prop(optional)] max: u16,
    progress: Signal<u16>,
) -> impl IntoView {
    // ...
}
```

### Use this instead

State the component default explicitly:

```rust,ignore
#[component]
fn ProgressBar(
    #[prop(default = 100)] max: u16,
    progress: Signal<u16>,
) -> impl IntoView {
    // ...
}
```

Use `Option<T>` instead when the component needs to distinguish absence from a present value.

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
