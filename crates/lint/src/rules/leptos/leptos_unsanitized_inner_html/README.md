# `rlib::leptos_unsanitized_inner_html`

## Summary

Checks for runtime `String` and `&str` values passed directly to Leptos's `inner_html` attribute.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for runtime `String` and `&str` values passed directly to Leptos's `inner_html` attribute.
Reactive attribute closures are inspected at their returned value. String literals and resolved
standard conversions of those literals are accepted because their complete markup is visible at
the call site.

## Why this matters

`inner_html` renders markup without escaping it. An ordinary string does not show whether its
contents came from a user, Markdown document, database, or reviewed sanitizer, making accidental
script injection difficult to spot during review.

## Examples

### Triggers the lint

```rust,ignore
let rendered: String = render_markdown(source);

view! {
    <article inner_html=rendered/>
}
```

### Use this instead

Represent the security decision with a dedicated type and keep raw rendering in one small audited
component:

```rust,ignore
struct TrustedHtml(String);

#[component]
#[allow(rlib::leptos_unsanitized_inner_html)]
fn TrustedMarkup(html: TrustedHtml) -> impl IntoView {
    view! { <article inner_html=html.0/> }
}
```

Callers can then render raw markup only after obtaining `TrustedHtml` through the application's
reviewed sanitization or rendering policy.

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
