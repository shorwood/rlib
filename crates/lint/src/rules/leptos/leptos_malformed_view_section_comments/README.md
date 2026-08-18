# `rlib::leptos_malformed_view_section_comments`

## Summary

Checks the spelling and placement of section comments inside `view!`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Checks the spelling and placement of section comments inside `view!`.

## Why this matters

Inconsistent headings make readers distinguish formatting styles instead of following the view’s
structure. A predictable form also lets related lints recognize section boundaries reliably.

## Examples

### Triggers the lint

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

### Use this instead

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

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
