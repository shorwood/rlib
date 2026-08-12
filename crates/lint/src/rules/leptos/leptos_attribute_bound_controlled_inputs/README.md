# leptos_attribute_bound_controlled_inputs

## What it does

Checks for writable reactive values passed to the plain `value` or `checked` HTML attribute of a
form control.

## Why is this bad?

HTML attributes describe initial state. After a user edits a control, its live value belongs to the
DOM property, so later reactive changes can disagree with what the control displays.

## Example

```rust,ignore
view! {
    <input value=name/>
}
```

## Use instead

Use a two-way binding for an ordinary controlled input:

```rust,ignore
view! {
    <input bind:value=name/>
}
```

Use `prop:value` or `prop:checked` with an explicit event handler when validation or event timing
needs custom behavior. A plain attribute is still suitable for a fixed initial value.
