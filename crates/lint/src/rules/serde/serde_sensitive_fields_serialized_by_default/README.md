# serde_sensitive_fields_serialized_by_default

## What it does

Finds public derived Serde serializers that include strongly named credential fields stored in raw
string or byte carrier types without an explicit serialization policy.

## Why is this bad?

Derived serialization is frequently reused by logs, responses, caches, and diagnostics. Including
raw secrets by default can disclose credentials through a boundary far from the type declaration.

## Example

```rust,ignore
#[derive(serde::Serialize)]
pub struct Session { pub access_token: String }
```

## Use instead

Skip the field, use a redacting/encrypting serializer, or store it in a secret-aware wrapper with an
intentional contract.

```rust,ignore
#[derive(serde::Serialize)]
pub struct Session {
    #[serde(skip_serializing)]
    pub access_token: String,
}
```
