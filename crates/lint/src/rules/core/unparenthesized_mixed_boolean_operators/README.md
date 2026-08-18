# `rlib::unparenthesized_mixed_boolean_operators`

## Summary

Finds boolean expressions that directly mix `&&` and `||` without parentheses expressing their intended grouping.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds boolean expressions that directly mix `&&` and `||` without parentheses expressing
their intended grouping. Chains containing only one of the operators remain valid.

## Why this matters

Rust gives `&&` higher precedence than `||`, but relying on that fact makes subtle
conditions easy to misread and edit incorrectly. Explicit groups communicate whether one
requirement governs every alternative or only the neighboring expression.

For example, this expression can easily be mistaken for requiring `is_final` in both cases:

## Examples

### Triggers the lint

```rust
# let (is_final, left_matches, right_matches) = (true, true, true);
let accepted = is_final && left_matches || right_matches;
# let _ = accepted;
```

### Use this instead

Parenthesize the alternatives to state that intention directly:

```rust
# let (is_final, left_matches, right_matches) = (true, true, true);
let accepted = is_final && (left_matches || right_matches);
# let _ = accepted;
```

## What it skips

Chains containing only one of the operators remain valid.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
