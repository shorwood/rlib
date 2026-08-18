# `rlib::strum_defaulted_payload_enum_construction`

## Summary

Finds `EnumIter`, `EnumString`, or `FromRepr` derives that construct data-bearing variants with default payload values.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `EnumIter`, `EnumString`, or `FromRepr` derives that construct data-bearing variants with
default payload values.

## Why this matters

Identifiers, quantities, paths, timestamps, and other payload state cannot generally be invented
without domain context. A syntactically valid default can therefore create a semantically invalid
enum value.

## Examples

### Triggers the lint

```rust,ignore
#[derive(strum::EnumString)]
enum Limit { Unlimited, Fixed(u32) }
```

### Use this instead

Disable the payload variant for that derive, use a unit discriminant enum, or keep a hand-written
constructor that requires the payload.

```rust,ignore
enum Job { Pending(JobId), Complete }
impl Job { fn pending(id: JobId) -> Self { Self::Pending(id) } }
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
