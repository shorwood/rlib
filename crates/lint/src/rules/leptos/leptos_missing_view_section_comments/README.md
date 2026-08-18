# `rlib::leptos_missing_view_section_comments`

## Summary

Checks complex groups of direct children in `view!` for explanatory section headings.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks complex groups of direct children in `view!` for explanatory section headings.

## Why this matters

A long view can contain several visual or behavioral responsibilities in one anonymous stream.
Readers then need to inspect every child before they can understand the shape of the component.

## Examples

### Triggers the lint

```rust,ignore
view! {
    <main>
        <AccountNavigation/>
        <Breadcrumbs/>
        <AccountSummary/>
        <RecentActivity/>
        <DeleteAccountButton/>
    </main>
}
```

### Use this instead

Name stable regions, or extract them into components with meaningful names:

```rust,ignore
view! {
    <main>
        // Account navigation
        <AccountNavigation/>
        <Breadcrumbs/>

        // Account overview
        <AccountSummary/>
        <RecentActivity/>

        // Destructive account actions
        <DeleteAccountButton/>
    </main>
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-unnamed-view-complexity-threshold` | positive integer | `4` | Sets when a view needs short section headings. |

## Known limitations

No known implementation limitations.

## Related lints

None.
