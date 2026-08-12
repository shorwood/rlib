# strum_non_roundtripping_enum_strings

## What it does

Finds unit variants whose Strum-generated output spelling is not parsed back into the same variant.

## Why is this bad?

Types exposing both input and output text contracts usually imply a round trip. Prefixes, suffixes,
aliases, and case-insensitive collisions can silently break it.

## Example

```rust,ignore
#[derive(strum::Display, strum::EnumString)]
#[strum(prefix = "status-")]
enum Status { Ready }
```

## Use instead

Align the parser and output spellings, or expose only the intentionally one-way contract.

```rust,ignore
#[strum(serialize = "pending", to_string = "pending")]
Pending
```
