# serde_manual_serialize_impls

## What it does

Finds `Serialize` implementations on single-field newtypes that forward the sole field unchanged to
the provided serializer.

## Why is this bad?

Transparent forwarding boilerplate obscures the manual serializers that actually define a custom
wire contract, validate values, or branch by format capability.

## Example

```rust,ignore
impl serde::Serialize for UserId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        self.0.serialize(serializer)
    }
}
```

## Use instead

```rust,ignore
#[derive(serde::Serialize)]
#[serde(transparent)]
struct UserId(u64);
```
