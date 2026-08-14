# bon_skipped_builder_members_without_policy

## What it does

Finds fields with bare `#[builder(skip)]` policy and no substantive documentation on Bon-derived
structs. Standard `PhantomData` marker fields are excluded.

## Why is this bad?

Bon initializes a bare skipped field with `Default::default()`. For domain state, that implicit value
can conceal an important construction invariant and make future changes surprising.

## Example

```rust,ignore
#[derive(bon::Builder)]
struct Session {
    #[builder(skip)]
    revision: u64,
}
```

## Use instead

State the initialization rule directly or document why the default is intentional:

```rust,ignore
#[derive(bon::Builder)]
struct Session {
    #[builder(skip = initial_revision())]
    revision: u64,
}
```
