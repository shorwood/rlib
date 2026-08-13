# serde_remote_representations_drifting_from_sources

## What it does

Finds undocumented Serde remote representations that omit fields from a uniquely resolved local
source struct.

## Why is this bad?

A remote definition is a second schema for another type. When it silently becomes a projection,
new source data can disappear from serialization or be synthesized during deserialization without
an explicit compatibility decision.

## Example

```rust,ignore
struct Source { id: u64, label: String }

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "Source")]
struct SourceDef { id: u64 }
```

## Use instead

Keep the mirror complete, or document that it is an intentionally versioned projection and how
omitted fields are reconstructed.

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "Source")]
struct SourceDef { id: u64, label: String }
```
