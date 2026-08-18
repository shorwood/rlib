# `rlib::leptos_noncanonical_view_formatting`

## Summary

Formats hand-written Leptos `view!` macros with a deterministic leptosfmt policy and reports files whose markup differs from that standard rendering.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Style |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Formats hand-written Leptos `view!` macros with a deterministic leptosfmt policy and reports files whose
markup differs from that standard rendering.

## Why this matters

Inconsistent tag, attribute, and expression layout makes declarative component structure needlessly
hard to scan and creates formatting-only review churn.

## Examples

### Triggers the lint

```rust
view! { <main><h1>"Dashboard"</h1><p>{summary}</p></main> }
```

### Use this instead

```rust
view! {
    <main>
        <h1>"Dashboard"</h1>
        <p>{summary}</p>
    </main>
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-view-max-width` | positive integer | `100` | Sets the line width used to format `view!` markup. |

## Known limitations

No known implementation limitations.

## Related lints

None.
