# `rlib::serde_remote_representations_drifting_from_sources`

## Summary

Finds undocumented Serde remote representations that omit fields from a uniquely resolved local source struct.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds undocumented Serde remote representations that omit fields from a uniquely resolved local
source struct.

## Why this matters

A remote definition is a second schema for another type. When it silently becomes a projection,
new source data can disappear from serialization or be synthesized during deserialization without
an explicit compatibility decision.

## Examples

### Triggers the lint

```rust,ignore
struct Source { id: u64, label: String }

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "Source")]
struct SourceDef { id: u64 }
```

### Use this instead

Keep the mirror complete, or document that it is an intentionally versioned projection and how
omitted fields are reconstructed.

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "Source")]
struct SourceDef { id: u64, label: String }
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
