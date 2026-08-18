# `rlib::derive_more_non_roundtripping_derived_text_contracts`

## Summary

Finds numeric newtypes whose derived `Display` adds literals or a nontransparent format while their derived `FromStr` still forwards the complete string to the numeric field parser.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds numeric newtypes whose derived `Display` adds literals or a nontransparent format while their
derived `FromStr` still forwards the complete string to the numeric field parser.

## Why this matters

The type appears to own matching text traits, but values formatted through `Display` cannot be read
back through `FromStr`.

## Examples

### Triggers the lint

```rust,ignore
#[derive(derive_more::Display, derive_more::FromStr)]
#[display("port:{_0}")]
struct Port(u16);
```

### Use this instead

Keep both traits transparent, or author a parser that consumes the same explicit text grammar.

```rust,ignore
#[derive(derive_more::Display, derive_more::FromStr)]
struct Port(u16);
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
