# Attribute-bound controlled inputs

## Proposition

Add an `attribute_bound_controlled_inputs` lint for controlled form elements whose current state is
bound through an HTML attribute instead of a DOM property or Leptos `bind:` directive.

```rust
// Bad: `value` sets initial HTML state, while the event handler treats the input as controlled.
view! {
    <input
        value=name
        on:input:target=move |event| name.set(event.target().value())
    />
}
```

```rust
// Better: bind both directions explicitly.
view! {
    <input bind:value=name/>
}

// Also valid.
view! {
    <input
        prop:value=name
        on:input:target=move |event| name.set(event.target().value())
    />
}
```

The `value` and `checked` HTML attributes establish initial state. Their DOM properties represent
current state after user interaction.

Reference: [Forms and Inputs](https://book.leptos.dev/view/05_forms.html).

## Conservative detection

- Inspect `input`, `textarea`, `select`, and configured form controls inside `view!`.
- Recognize a controlled element from an input/change handler writing the same reactive value shown
  by `value` or `checked`.
- Diagnose `value=` where `prop:value=` or `bind:value=` is required.
- Diagnose `checked=` where `prop:checked=` or `bind:checked=` is required.
- Account for `textarea` children, which represent initial server-rendered content rather than the
  live property.
- Point at both the attribute and controlling event handler when possible.

## Uncontrolled inputs

```rust
view! {
    <input value=initial_name node_ref=input_ref/>
}
```

An uncontrolled input legitimately uses `value` for initial state and reads the DOM value through a
`NodeRef` at submission time. The absence of a reactive write-back handler is strong exemption
evidence.

## Diagnostic direction

- Prefer `bind:value`, `bind:checked`, or `bind:group` when they express the complete interaction.
- Suggest explicit property and event syntax when custom validation or event policy exists.
- Do not rewrite event type or timing automatically; `input` and `change` have different semantics.

## Open decisions

- Whether a reactive `value=` without a write-back handler is still suspicious.
- How custom components wrapping native controls declare their controlled properties.
- Whether `selected` options and other DOM property distinctions belong to this rule.
- How spreads and conditionally supplied handlers affect confidence.
- Whether an element using both initial children and `prop:value` for SSR should be explicitly
  recognized as canonical.
