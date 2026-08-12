# leptos_implicit_default_component_props

## What it does

Checks for Leptos component properties that use `#[prop(optional)]` with a concrete value
type. Properties using `Option<T>` are accepted because omission can be meaningful state.

## Why is this bad?

`#[prop(optional)]` fills an omitted property with its type's implementation of `Default`.
That generic default may not describe the component's intended behavior, and a later change to
the type can silently change the component API. Writing the default at the property declaration
makes the behavior visible to readers and callers.

## Example

```rust,ignore
#[component]
fn ProgressBar(
    #[prop(optional)] max: u16,
    progress: Signal<u16>,
) -> impl IntoView {
    // ...
}
```

## Use instead

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
