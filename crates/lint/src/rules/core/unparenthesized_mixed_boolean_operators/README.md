# unparenthesized_mixed_boolean_operators

## What it does

Finds boolean expressions that directly mix `&&` and `||` without parentheses expressing
their intended grouping. Chains containing only one of the operators remain valid.

## Why is this bad?

Rust gives `&&` higher precedence than `||`, but relying on that fact makes subtle
conditions easy to misread and edit incorrectly. Explicit groups communicate whether one
requirement governs every alternative or only the neighboring expression.

For example, this expression can easily be mistaken for requiring `is_final` in both cases:

## Example

```rust
# let (is_final, left_matches, right_matches) = (true, true, true);
let accepted = is_final && left_matches || right_matches;
# let _ = accepted;
```

## Use instead


Parenthesize the alternatives to state that intention directly:

```rust
# let (is_final, left_matches, right_matches) = (true, true, true);
let accepted = is_final && (left_matches || right_matches);
# let _ = accepted;
```
