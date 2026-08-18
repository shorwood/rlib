# `rlib::oversized_match_arms`

## Summary

Finds match arms whose hand-written code exceeds the configured line limit.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds match arms whose hand-written code exceeds the configured line limit. Blank lines,
comments, and the arm's outer braces are excluded from the count, and a physical line is counted
only once even when it contains several statements. Matches owned by closure bodies are outside
this named-function structure policy.

## Why this matters

A match should reveal the alternatives and the operation selected by each one. Large inline
branches bury that decision table beneath implementation detail, and phase comments cannot
restore the lost overview.

## Examples

### Triggers the lint

```rust
fn render(value: Option<u8>) {
    match value {
        Some(value) => {
            let doubled = value * 2;
            let text = doubled.to_string();
            println!("{text}");
        }
        None => {}
    }
}
```

### Use this instead

Extract branch behavior into a helper whose name explains the selected operation.

```rust
fn render(value: Option<u8>) {
    match value {
        Some(value) => render_value(value),
        None => {}
    }
}

fn render_value(value: u8) {
    println!("{}", value * 2);
}
```

## What it skips

Matches owned by closure bodies are outside this named-function structure policy.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `match-arm-lines-threshold` | positive integer | `7` | Sets the largest allowed match arm, measured in source lines. |

## Known limitations

No known implementation limitations.

## Related lints

None.
