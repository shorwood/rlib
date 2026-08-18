# `rlib::serde_format_specific_serde_impls`

## Summary

Finds undocumented manual Serde implementations that depend on a specific format or use different data shapes when `is_human_readable()` changes.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds undocumented manual Serde implementations that depend on a specific format or use different
data shapes when `is_human_readable()` changes.

## Why this matters

`Serialize` and `Deserialize` promise a format-independent data model. Hidden JSON assumptions or
different human/binary shapes make wrappers, migrations, and cross-format round trips surprising.

## Examples

### Triggers the lint

```rust,ignore
impl serde::Serialize for Identifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        if serializer.is_human_readable() {
            serializer.serialize_str("42")
        } else {
            serializer.serialize_u64(42)
        }
    }
}
```

### Use this instead

Keep one portable Serde shape, or expose a format-specific wrapper and document a deliberately dual
representation.

```rust,ignore
impl serde::Serialize for Identifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_u64(self.0)
    }
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
