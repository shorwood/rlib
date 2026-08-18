# `rlib::bon_skipped_builder_members_without_policy`

## Summary

Finds fields with bare `#[builder(skip)]` policy and no substantive documentation on Bon-derived structs.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds fields with bare `#[builder(skip)]` policy and no substantive documentation on Bon-derived
structs. Standard `PhantomData` marker fields are excluded.

## Why this matters

Bon initializes a bare skipped field with `Default::default()`. For domain state, that implicit value
can conceal an important construction invariant and make future changes surprising.

## Examples

### Triggers the lint

```rust,ignore
#[derive(bon::Builder)]
struct Session {
    #[builder(skip)]
    revision: u64,
}
```

### Use this instead

State the initialization rule directly or document why the default is intentional:

```rust,ignore
#[derive(bon::Builder)]
struct Session {
    #[builder(skip = initial_revision())]
    revision: u64,
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
