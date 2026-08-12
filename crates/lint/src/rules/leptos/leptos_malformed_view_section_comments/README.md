# leptos_malformed_view_section_comments

## What it does

Checks the spelling and placement of section comments inside `view!`.

## Why is this bad?

Inconsistent headings make readers distinguish formatting styles instead of following the view’s
structure. A predictable form also lets related lints recognize section boundaries reliably.

## Example

```rust,ignore
view! {
    <main>
        // ACCOUNT NAVIGATION:
        <Navigation/>
        // Account Content
        <Content/>
    </main>
}
```

## Use instead

Use an ordinary line comment with sentence-style prose directly before its region. Separate later
headings from the preceding region with a blank line:

```rust,ignore
view! {
    <main>
        // Account navigation
        <Navigation/>

        // Account content
        <Content/>
    </main>
}
```
