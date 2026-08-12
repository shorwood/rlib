# leptos_duplicate_view_section_comments

## What it does

Checks that section headings in the same part of a `view!` have distinct names.

## Why is this bad?

Two nearby regions with the same name appear to have the same responsibility. Readers must inspect
their markup to discover the difference, which defeats the purpose of the headings.

## Example

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

## Use instead

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
