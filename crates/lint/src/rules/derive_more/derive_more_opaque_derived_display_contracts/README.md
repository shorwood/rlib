# derive_more_opaque_derived_display_contracts

## What it does

Finds values with derived `Display` whose `to_string()` output is used as a standard map key.

## Why is this bad?

Using presentation output as machine identity silently turns formatting changes into cache misses,
key collisions, or persistence incompatibilities.

## Example

```rust,ignore
#[derive(derive_more::Display)]
#[display("{kind}:{value}")]
struct CacheKey { kind: Kind, value: String }

cache.insert(key.to_string(), record);
```

## Use instead

Define and use a named encoding contract whose compatibility is explicit.

```rust,ignore
impl CacheKey {
    fn encode(&self) -> String { /* stable key grammar */ }
}

cache.insert(key.encode(), record);
```
