# leptos_primitive_context_values

## What it does

Finds primitive, generic-container, callback, and unbranded reactive values used as Leptos context
identities.

## Why is this bad?

Leptos resolves context by concrete type. Broad types can collide with unrelated providers and make
a component's dependency impossible to name or review. Raw writable handles can also expose more
authority than descendants require.

## Example

```rust,ignore
let (_, set_theme) = signal(false);
provide_context(set_theme);
let setter = use_context::<WriteSignal<bool>>();
```

## Use instead

Give the context a named domain identity and expose only the required capability:

```rust,ignore
#[derive(Clone, Copy)]
struct ThemeContext(WriteSignal<Theme>);
provide_context(ThemeContext(set_theme));
```
