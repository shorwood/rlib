# leptos_noncanonical_view_formatting

## What it does

Formats authored Leptos `view!` macros with a deterministic leptosfmt policy and reports files whose
markup differs from that canonical rendering.

## Why is this bad?

Inconsistent tag, attribute, and expression layout makes declarative component structure needlessly
hard to scan and creates formatting-only review churn.

## Example

```rust
view! { <main><h1>"Dashboard"</h1><p>{summary}</p></main> }
```

## Use instead

```rust
view! {
    <main>
        <h1>"Dashboard"</h1>
        <p>{summary}</p>
    </main>
}
```

## Configuration

`leptos-view-max-width` sets the formatter width (default `100`).
