# Unsanitized inner HTML

## Proposition

Add an `unsanitized_inner_html` lint that rejects raw textual values passed to Leptos's
`inner_html` attribute unless their type proves that the content is trusted or sanitized.

```rust
// Bad: Article::body can contain attacker-controlled markup.
view! {
    <article inner_html=article.body/>
}
```

```rust
// Better: validity is established before the rendering boundary.
struct SanitizedHtml(String);

view! {
    <article inner_html=article.body.as_str()/>
}
```

Leptos documents that `inner_html` does not escape its input and warns about cross-site scripting.

Reference: [Dynamic Attributes: Injecting Raw HTML](https://book.leptos.dev/view/02_dynamic_attributes.html#dynamic-attributes).

## Conservative detection

- Inspect `inner_html` attributes in authored `view!` invocations.
- Reject `String`, `&str`, `Cow<str>`, formatted strings, and reactive wrappers around those types.
- Permit configured branded types such as `SanitizedHtml` or `TrustedHtml` only through APIs that do
  not expose unrestricted public construction.
- Optionally permit compile-time string literals after deciding whether uniform branding is more
  important than convenience.
- Follow simple accessors so `trusted_html.as_str()` retains the branded origin.
- Avoid treating a function name containing `sanitize` as proof without a trusted return type.

## Validity by construction

```rust
impl SanitizedHtml {
    pub fn sanitize(untrusted: &str) -> Self {
        // Sanitizer with an explicit, reviewed policy.
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

The wrapper should make the safety decision discoverable and prevent unrelated callers from
constructing trusted markup directly. A public tuple field or unchecked `From<String>` impl defeats
the guarantee and may warrant a core companion rule.

## Legitimate raw markup

- audited static application markup;
- output from a sanitizer represented by a branded type;
- trusted compile-time generated content;
- tightly controlled framework internals explicitly configured as trusted.

Server-provided content, Markdown rendering, CMS content, and database HTML are not intrinsically
trusted merely because they originate inside the application.

## Open decisions

- Whether string literals require the branded type.
- How trusted wrapper types and constructors are configured or inferred.
- Whether the lint verifies that wrapper fields are private.
- Whether a separate type should distinguish escaped text from sanitized HTML.
- Whether known sanitizer crates can be recognized without weakening type-based enforcement.
