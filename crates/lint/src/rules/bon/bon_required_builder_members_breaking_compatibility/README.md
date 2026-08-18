# `rlib::bon_required_builder_members_breaking_compatibility`

## Summary

Compares public Bon struct, free-function, and associated-function builders with the explicitly configured `bon_api_baseline` member snapshot and finds newly added required members.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Compares public Bon struct, free-function, and associated-function builders with the explicitly
configured `bon_api_baseline` member snapshot and finds newly added required members. Associated
builders use `Type::method` baseline keys.

## Why this matters

Adding a required member invalidates every existing external builder call sequence. This is a real
compatibility change that cannot be inferred safely without historical evidence.

## Examples

### Triggers the lint

```toml
[rlib-lint]
bon-api-baseline = [{ builder = "Request", members = ["host"] }]
```

```rust,ignore
#[derive(bon::Builder)]
pub struct Request { host: String, port: u16 }
```

### Use this instead

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

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `bon-api-baseline` | array of tables | `[]` | Records each published builder and the member names that were already required. |

## Known limitations

No known implementation limitations.

## Related lints

None.
