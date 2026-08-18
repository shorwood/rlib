# `rlib::leptos_unlocalized_view_literals`

## Summary

Checks for directly hand-written user-visible text and translatable literal attributes in Leptos `view!` markup.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos_i18n` |
| Cargo feature | `leptos_i18n` |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for directly hand-written user-visible text and translatable literal attributes in Leptos
`view!` markup.

## Why this matters

Even initially fixed interface copy is runtime presentation data. A literal bypasses the locale
catalog, so it cannot react when the user changes language and translators cannot discover it.

## Examples

### Triggers the lint

```rust,ignore
view! {
    <input aria-label="Search" placeholder="Search systems…"/>
    <button>"Save"</button>
}
```

### Use this instead

Move presentation text into the locale catalog and render it through the localization runtime:

```rust,ignore
view! {
    <input
        aria-label=move || t_string!(i18n, common::action.search)
        placeholder=move || t_string!(i18n, systems::search.placeholder)
    />
    <button>{t!(i18n, common::action.save)}</button>
}
```

Stable technical values such as routes, element IDs, ARIA relationships, roles, input types, and
wire values remain valid literals.

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
