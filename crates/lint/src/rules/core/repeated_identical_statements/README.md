# repeated_identical_statements

## What it does

Finds adjacent statements whose hand-written source is identical. An authored comment between
the statements makes the repetition explicit and suppresses the finding. Statements produced by
macro expansion are ignored because their repetition cannot be repaired at the diagnostic site.

## Why is this bad?

Exact neighboring repetitions are commonly copy-and-paste mistakes. When repetition is
deliberate, spelling the count or iteration policy explicitly communicates intent and makes
future changes apply consistently.

For example, this weight is accidentally counted twice:

## Example

```rust
# struct Evidence;
# impl Evidence { fn add(&mut self, _: &str) {} }
# let mut evidence = Evidence;
evidence.add("contiguous");
evidence.add("contiguous");
```

## Use instead

Remove the duplicate, or expose intentional multiplicity through the relevant abstraction:

```rust
# struct Evidence;
# impl Evidence { fn add_weight(&mut self, _: &str, _: usize) {} }
# let mut evidence = Evidence;
evidence.add_weight("contiguous", 2);
```
