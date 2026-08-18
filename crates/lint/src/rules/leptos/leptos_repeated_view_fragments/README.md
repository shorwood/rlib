# leptos_repeated_view_fragments

## What it does

Finds crate-wide normalized RSX subtrees repeated often enough to represent a missing component.

## Why is this bad?

Copied markup drifts independently and leaves a shared visual concept implicit.

## Example

```rust,ignore
view! { <label class=style::FIELD><span>"Name"</span><input prop:value=name /></label> }
view! { <label class=style::FIELD><span>"Email"</span><input prop:value=email /></label> }
```

## Use instead

```rust,ignore
view! { <FormField label="Name" value=name /><FormField label="Email" value=email /> }
```

## Configuration

`leptos-repeated-view-fragment-nodes-threshold` (default `6`) and `leptos-repeated-view-fragment-occurrences-threshold` (default `2`) define a repeated fragment.
