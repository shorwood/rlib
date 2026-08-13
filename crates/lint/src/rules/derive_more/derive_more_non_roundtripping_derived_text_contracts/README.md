# derive_more_non_roundtripping_derived_text_contracts

## What it does

Finds numeric newtypes whose derived `Display` adds literals or a nontransparent format while their
derived `FromStr` still forwards the complete string to the numeric field parser.

## Why is this bad?

The type appears to own matching text traits, but values formatted through `Display` cannot be read
back through `FromStr`.

## Example

```rust,ignore
#[derive(derive_more::Display, derive_more::FromStr)]
#[display("port:{_0}")]
struct Port(u16);
```

## Use instead

Keep both traits transparent, or author a parser that consumes the same explicit text grammar.

```rust,ignore
#[derive(derive_more::Display, derive_more::FromStr)]
struct Port(u16);
```
