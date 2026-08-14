# ad_hoc_string_parsers

## What it does

Finds the unique hand-written function or receiver-free inherent method that accepts exactly
one immutable `&str`, uses that input, constructs a same-module owned type, and returns it as
`Result<T, E>`. When the target has no borrowed lifetime and no existing `FromStr`
implementation, the lint asks the type to expose the standard parsing contract.

Returned local aliases and explicit returns retain construction provenance. Discarded target
values, uncalled closures, and explicitly discarded input bindings do not establish a parser.

Names are deliberately secondary to structure. Neutral construction words such as
`parse`, `decode`, `try`, and the target's own words describe a canonical parser. Additional
words such as `json`, `lossy`, or `strict` identify a qualified format or policy and remain
valid. Multiple structurally valid parsers for one target are also left alone because
choosing the canonical format requires domain judgment.

## Why is this bad?

An ad hoc canonical parser hides a standard capability from readers and tools. Callers
cannot use `value.parse::<T>()`, generic code cannot state `T: FromStr`, and another helper
may grow beside the first because the type does not visibly own its textual contract.

## Example

```rust
struct UserId(u64);

fn parse_user_id(source: &str) -> Result<UserId, std::num::ParseIntError> {
    Ok(UserId(source.parse()?))
}
```

## Use instead

Prefer making the canonical conversion explicit on the parsed type:

```rust
use std::str::FromStr;

struct UserId(u64);

impl FromStr for UserId {
    type Err = std::num::ParseIntError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        Ok(Self(source.parse()?))
    }
}
```

This lint does not offer an automatic fix because selecting the error type, public API,
imports, and canonical accepted syntax changes a trait contract rather than mere layout.
