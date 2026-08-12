# leptos_missing_view_section_comments

## What it does

Checks complex groups of direct children in `view!` for explanatory section headings.

## Why is this bad?

A long view can contain several visual or behavioral responsibilities in one anonymous stream.
Readers then need to inspect every child before they can understand the shape of the component.

## Example

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

## Use instead

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
