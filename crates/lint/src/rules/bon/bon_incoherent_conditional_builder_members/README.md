# bon_incoherent_conditional_builder_members

## What it does

Finds Bon members on derived structs, free functions, and associated functions whose
requiredness, default, hidden initialization, or positional placement is changed with
`cfg_attr` while the member itself remains present. Policy words in the cfg predicate do not
count unless the `builder(...)` payload changes the contract.

## Why is this bad?

The same domain API then has incompatible builder call contracts across feature sets or targets.
Callers cannot rely on one stable required-member and finishing protocol.

## Example

```rust,ignore
#[bon::builder]
fn connect(#[cfg_attr(feature = "lenient", builder(default))] timeout: u64) {}
```

## Use instead

Keep the construction policy stable, or explicitly gate a distinct capability/API.

```rust,ignore
#[bon::builder]
fn connect(#[builder(default)] timeout: u64) {}
```
