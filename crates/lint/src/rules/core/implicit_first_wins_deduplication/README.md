# implicit_first_wins_deduplication

## What it does

Finds iterator filters that deduplicate values by returning the result of
`HashSet::insert` directly. Other uses of `HashSet::insert` and ordinary predicates remain
valid.

## Why is this bad?

This compact idiom silently selects the first value for every key. That policy is often
harmless for identical values, but it loses information when later records contain richer
state. An explicit merge makes representative selection reviewable.

For example, this always retains the first participant encountered:

## Example

```rust
# use std::collections::HashSet;
# struct Participant { id: u32 }
# let participants = Vec::<Participant>::new();
let mut seen = HashSet::new();
let unique = participants.iter().filter(|item| seen.insert(item.id));
# let _ = unique;
```

## Use instead


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
