# bon_required_builder_members_breaking_compatibility

## What it does

Compares public Bon struct, free-function, and associated-function builders with the explicitly
configured `bon_api_baseline` member snapshot and finds newly added required members. Associated
builders use `Type::method` baseline keys.

## Why is this bad?

Adding a required member invalidates every existing external builder call sequence. This is a real
compatibility change that cannot be inferred safely without historical evidence.

## Example

```toml
[rlib-lint]
bon-api-baseline = [{ builder = "Request", members = ["host"] }]
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

## Configuration

`bon-api-baseline` records builders and their previously required members as `{ builder, members }` entries.
