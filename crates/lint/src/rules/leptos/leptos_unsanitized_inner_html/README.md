# leptos_unsanitized_inner_html

## What it does

Checks for runtime `String` and `&str` values passed directly to Leptos's `inner_html` attribute.
String literals are accepted because their complete markup is visible at the call site.

## Why is this bad?

`inner_html` renders markup without escaping it. An ordinary string does not show whether its
contents came from a user, Markdown document, database, or reviewed sanitizer, making accidental
script injection difficult to spot during review.

## Example

```rust,ignore
let rendered: String = render_markdown(source);

view! {
    <article inner_html=rendered/>
}
```

## Use instead

Represent the security decision with a dedicated type and keep raw rendering in one small audited
component:

```rust,ignore
struct TrustedHtml(String);

#[component]
#[allow(leptos_unsanitized_inner_html)]
fn TrustedMarkup(html: TrustedHtml) -> impl IntoView {
    view! { <article inner_html=html.0/> }
}
```

Callers can then render raw markup only after obtaining `TrustedHtml` through the application's
reviewed sanitization or rendering policy.
