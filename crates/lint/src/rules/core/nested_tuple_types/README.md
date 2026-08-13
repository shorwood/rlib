# nested_tuple_types

## What it does

Finds non-unit tuple types nested inside another explicit type, including references,
slices, arrays, collections, options, results, and associated type bindings.

## Why is this bad?

Nesting an anonymous tuple makes an already structural type harder to understand. The
surrounding container explains storage or ownership, but it cannot explain what each tuple
position represents. Repeating the shape also spreads an unnamed protocol across APIs.

For example, neither component of each affected entry is named:

## Example

```rust
fn affected() -> &'static [(&'static Participant, NameTokens)] {
    todo!()
}
```

## Use instead

Name the element concept and its roles before placing it in the container:

```rust
struct AffectedParticipant<'a> {
    participant: &'a Participant,
    name: NameTokens,
}

fn affected() -> &'static [AffectedParticipant<'static>] {
    todo!()
}
```
