# `rlib::implicit_first_wins_deduplication`

## Summary

Finds iterator filters that deduplicate values by returning the result of `HashSet::insert` directly.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds iterator filters that deduplicate values by returning the result of
`HashSet::insert` directly. Both operations are identified by their resolved standard
definitions, so same-named custom methods, other uses of `HashSet::insert`, and ordinary
predicates remain valid.

## Why this matters

This compact idiom silently selects the first value for every key. That policy is often
harmless for identical values, but it loses information when later records contain richer
state. An explicit merge makes representative selection reviewable.

For example, this always retains the first participant encountered:

## Examples

### Triggers the lint

```rust
# use std::collections::HashSet;
# struct Participant { id: u32 }
# let participants = Vec::<Participant>::new();
let mut seen = HashSet::new();
let unique = participants.iter().filter(|item| seen.insert(item.id));
# let _ = unique;
```

### Use this instead

Collect by key and state how collisions are resolved instead:

```rust
# use std::collections::HashMap;
# struct Participant { id: u32 }
# let participants = Vec::<Participant>::new();
let mut unique = HashMap::new();
for item in participants {
    unique.entry(item.id).or_insert(item);
}
```

## What it skips

Both operations are identified by their resolved standard definitions, so same-named custom methods, other uses of `HashSet::insert`, and ordinary predicates remain valid.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
