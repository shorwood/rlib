# Leptos i18n lints

## Summary

Rules that keep user-visible Leptos text in the localization catalog.

## How to enable

Enable the `leptos_i18n` Cargo feature to register this family. Enable the whole family with `rlib::leptos_i18n`.

## Lints

| Lint | Summary | Purpose | Fix |
| --- | --- | --- | --- |
| [`rlib::leptos_unlocalized_view_literals`](./leptos_unlocalized_view_literals/README.md) | Checks for directly hand-written user-visible text and translatable literal attributes in Leptos `view!` markup. | Style | Manual |
