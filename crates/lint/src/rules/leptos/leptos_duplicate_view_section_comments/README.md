# `rlib::leptos_duplicate_view_section_comments`

## Summary

Checks that section headings in the same part of a `view!` have distinct names.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks that section headings in the same part of a `view!` have distinct names.

## Why this matters

Two nearby regions with the same name appear to have the same responsibility. Readers must inspect
their markup to discover the difference, which defeats the purpose of the headings.

## Examples

### Triggers the lint

```rust,ignore
view! {
    <main>
        // Account content
        <Profile/>

        // Account content
        <Activity/>
    </main>
}
```

### Use this instead

Name each region after the role it has in the interface:

```rust,ignore
view! {
    <main>
        // Account profile
        <Profile/>

        // Recent activity
        <Activity/>
    </main>
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
