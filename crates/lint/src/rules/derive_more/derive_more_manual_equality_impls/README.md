# derive_more_manual_equality_impls

## What it does

Finds structural `PartialEq` implementations, with an optional marker `Eq` implementation, that
compare corresponding fields and nothing else.

## Why is this bad?

Hand-written component equality hides a declarative field-selection policy and can drift when the
type changes.

## Example

```rust,ignore
impl PartialEq for Record {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.revision == other.revision
    }
}
```

## Use instead

Declare structural equality and explicitly skip any representation-only fields.

```rust,ignore
#[derive(derive_more::PartialEq, derive_more::Eq)]
struct Record { id: Id, revision: u64 }
```
