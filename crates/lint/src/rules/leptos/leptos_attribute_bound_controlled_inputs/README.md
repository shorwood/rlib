# `rlib::leptos_attribute_bound_controlled_inputs`

## Summary

Checks for writable reactive values passed to the plain `value` or `checked` HTML attribute of a form control.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for writable reactive values passed to the plain `value` or `checked` HTML attribute of a
form control.

## Why this matters

HTML attributes describe initial state. After a user edits a control, its live value belongs to the
DOM property, so later reactive changes can disagree with what the control displays.

## Examples

### Triggers the lint

```rust,ignore
view! {
    <input value=name/>
}
```

### Use this instead

Use a two-way binding for an ordinary controlled input:

```rust,ignore
view! {
    <input bind:value=name/>
}
```

Use `prop:value` or `prop:checked` with an explicit event handler when validation or event timing
needs custom behavior. A plain attribute is still suitable for a fixed initial value.

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
