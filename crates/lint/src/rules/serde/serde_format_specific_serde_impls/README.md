# serde_format_specific_serde_impls

## What it does

Finds undocumented manual Serde trait implementations whose generic contract contains concrete
format dependencies or switches between distinct data-model shapes with `is_human_readable()`.

## Why is this bad?

`Serialize` and `Deserialize` promise a format-independent data model. Hidden JSON assumptions or
different human/binary shapes make wrappers, migrations, and cross-format round trips surprising.

## Example

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

## Use instead

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
