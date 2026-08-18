# `rlib::ad_hoc_string_parsers`

## Summary

Finds the unique hand-written function or receiver-free inherent method that accepts exactly one immutable `&str`, uses that input, constructs a same-module owned type, and returns it as `Result<T, E>`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds the unique hand-written function or receiver-free inherent method that accepts exactly
one immutable `&str`, uses that input, constructs a same-module owned type, and returns it as
`Result<T, E>`. When the target has no borrowed lifetime and no existing `FromStr`
implementation, the lint asks the type to expose the standard parsing behavior.

Returned local aliases and explicit returns retain their link to the parsed input. Discarded target
values, uncalled closures, and explicitly discarded input bindings do not establish a parser.

Names are deliberately secondary to structure. Neutral construction words such as
`parse`, `decode`, `try`, and the target's own words describe a standard parser. Additional
words such as `json`, `lossy`, or `strict` identify a qualified format or policy and remain
valid. Multiple structurally valid parsers for one target are also left alone because
choosing the standard format requires domain judgment.

## Why this matters

An ad hoc standard parser hides a standard capability from readers and tools. Callers
cannot use `value.parse::<T>()`, generic code cannot state `T: FromStr`, and another helper
may grow beside the first because the type does not provide one standard way to parse text.

## Examples

### Triggers the lint

```rust
struct UserId(u64);

fn parse_user_id(source: &str) -> Result<UserId, std::num::ParseIntError> {
    Ok(UserId(source.parse()?))
}
```

### Use this instead

Prefer making the standard conversion explicit on the parsed type:

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
imports, and standard accepted syntax changes a trait behavior rather than mere layout.

## What it skips

Discarded target values, uncalled closures, and explicitly discarded input bindings do not establish a parser. Additional words such as `json`, `lossy`, or `strict` identify a qualified format or policy and remain valid. Multiple structurally valid parsers for one target are also left alone because choosing the standard format requires domain judgment.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
