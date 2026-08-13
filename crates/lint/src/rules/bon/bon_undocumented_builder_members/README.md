# bon_undocumented_builder_members

## What it does

Finds non-obvious optionality, default, conversion, or hidden-initialization behavior on undocumented
members of public Bon builders.

## Why is this bad?

Generated signatures show how to call a setter, but they cannot explain what omission, conversion,
validation, or hidden initialization means in the domain contract.

## Example

```rust,ignore
#[bon::builder]
pub fn connect(#[builder(default = 30)] timeout_seconds: u64) {}
```

## Use instead

Document the behavior on the hand-written field or parameter so Bon can carry it to generated APIs:

```rust,ignore
#[bon::builder]
pub fn connect(
    /// Connection timeout in seconds; defaults to 30.
    #[builder(default = 30)]
    timeout_seconds: u64,
) {}
```
