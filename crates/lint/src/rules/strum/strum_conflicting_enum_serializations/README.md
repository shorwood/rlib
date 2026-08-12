# strum_conflicting_enum_serializations

## What it does

Finds Strum enum spellings whose parser languages overlap or whose output alias is selected only by
the longest-alias fallback.

## Why is this bad?

Overlapping names make variants unreachable or order-dependent, while implicit output selection can
change when an alias is added.

## Example

```rust,ignore
#[derive(strum::EnumString)]
enum Color {
    #[strum(serialize = "gray", ascii_case_insensitive)]
    Gray,
    #[strum(serialize = "GRAY")]
    LegacyGray,
}
```

## Use instead

Give every parser spelling one owner and set an explicit `to_string` value when aliases coexist.

```rust,ignore
#[strum(serialize = "start", to_string = "start")]
Start
```
