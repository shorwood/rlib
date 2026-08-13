# oversized_match_arms

## What it does

Finds match arms whose hand-written code exceeds the configured line limit. Blank lines,
comments, and the arm's outer braces are excluded from the count.

## Why is this bad?

A match should reveal the alternatives and the operation selected by each one. Large inline
branches bury that decision table beneath implementation detail, and phase comments cannot
restore the lost overview.

## Example

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

## Use instead

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
