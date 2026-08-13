# bare_tuple_types

## What it does

Finds non-unit tuple types used as the root of an explicit type annotation. This includes
fields, parameters, return types, aliases, associated types, locals, and closure
annotations.

## Why is this bad?

A tuple records positions but not their meaning. Readers must recover each component's role
from surrounding code, and later additions or reordered elements are easy to misuse. A
concept-specific record also gives agents a vocabulary to reuse instead of propagating an
anonymous structural shape.

For example, this return type leaves both values unnamed:

## Example

```rust
fn partition() -> (Accepted, Rejected) {
    todo!()
}
```

## Use instead

Introduce a record whose type and fields explain the result:

```rust
struct Partition {
    accepted: Accepted,
    rejected: Rejected,
}

fn partition() -> Partition {
    todo!()
}
```
