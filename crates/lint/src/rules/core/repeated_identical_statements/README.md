# `rlib::repeated_identical_statements`

## Summary

Finds adjacent statements whose hand-written source is identical.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds adjacent statements whose hand-written source is identical. A hand-written comment between
the statements makes the repetition explicit and suppresses the finding. Statements produced by
macro expansion are ignored because their repetition cannot be repaired at the diagnostic site.

## Why this matters

Exact neighboring repetitions are commonly copy-and-paste mistakes. When repetition is
deliberate, spelling the count or iteration policy explicitly communicates intent and makes
future changes apply consistently.

For example, this weight is accidentally counted twice:

## Examples

### Triggers the lint

```rust
# struct Evidence;
# impl Evidence { fn add(&mut self, _: &str) {} }
# let mut evidence = Evidence;
evidence.add("contiguous");
evidence.add("contiguous");
```

### Use this instead

Remove the duplicate, or expose intentional multiplicity through the relevant abstraction:

```rust
# struct Evidence;
# impl Evidence { fn add_weight(&mut self, _: &str, _: usize) {} }
# let mut evidence = Evidence;
evidence.add_weight("contiguous", 2);
```

## What it skips

Statements produced by macro expansion are ignored because their repetition cannot be repaired at the diagnostic site.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
