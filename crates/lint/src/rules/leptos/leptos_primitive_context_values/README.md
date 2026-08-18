# `rlib::leptos_primitive_context_values`

## Summary

Finds primitive, generic-container, callback, and unbranded reactive values used as Leptos context identities.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds primitive, generic-container, callback, and unbranded reactive values used as Leptos context
identities.

This includes providing, using, expecting, taking, borrowing, updating, and bidirectionally
searching for context values.

## Why this matters

Leptos resolves context by concrete type. Broad types can collide with unrelated providers and make
a component's dependency impossible to name or review. Raw writable handles can also expose more
authority than descendants require.

## Examples

### Triggers the lint

```rust,ignore
let (_, set_theme) = signal(false);
provide_context(set_theme);
let setter = use_context::<WriteSignal<bool>>();
```

### Use this instead

Give the context a named domain identity and expose only the required capability:

```rust,ignore
#[derive(Clone, Copy)]
struct ThemeContext(WriteSignal<Theme>);
provide_context(ThemeContext(set_theme));
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
