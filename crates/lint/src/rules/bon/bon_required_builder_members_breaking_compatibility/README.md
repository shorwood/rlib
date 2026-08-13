# bon_required_builder_members_breaking_compatibility

## What it does

Compares public Bon builders with the explicitly configured `bon_api_baseline` member snapshot and
finds newly added required members.

## Why is this bad?

Adding a required member invalidates every existing external builder call sequence. This is a real
compatibility change that cannot be inferred safely without historical evidence.

## Example

```toml
[rlib-lint.bon_api_baseline.builders.Request]
members = ["host"]
```

```rust,ignore
#[derive(bon::Builder)]
pub struct Request { host: String, port: u16 }
```

## Use instead

Provide a compatible default/optional policy or make the break explicit through versioning and a
coordinated baseline update.

```rust,ignore
#[derive(bon::Builder)]
pub struct Request {
    host: String,
    #[builder(default = 443)]
    port: u16,
}
```
