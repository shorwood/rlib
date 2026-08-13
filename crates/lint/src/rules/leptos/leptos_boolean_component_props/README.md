# leptos_boolean_component_props

## What it does

Rejects direct, optional, and reactive boolean properties on Leptos components when their
names do not communicate a standard binary state. The platform states `disabled` and
`invalid` are accepted because HTML and ARIA already define their meaning and conventional
opposite. Booleans nested inside callback signatures or unknown application wrappers are
not treated as component state by this rule.

## Why is this bad?

Component calls read as declarative markup. A boolean value does not name the selected
state at the call site, makes the opposite state implicit, and cannot grow to represent
another state without changing the property type. A domain enum keeps the component's
vocabulary visible in both its declaration and its uses. This does not apply to conventional
platform states such as `disabled`: replacing that established binary contract with an enum
would obscure interoperability rather than clarify domain vocabulary.

For example, presentation flags leave the selected design variant implicit:

## Example

```rust,ignore
#[component]
fn Badge(compact: bool, featured: bool) -> impl IntoView {
    // ...
}
```

## Use instead

Prefer a domain type that names every supported presentation:

```rust,ignore
enum BadgeDensity { Comfortable, Compact }
enum BadgeEmphasis { Standard, Featured }

#[component]
fn Badge(density: BadgeDensity, emphasis: BadgeEmphasis) -> impl IntoView {
    // ...
}
```
