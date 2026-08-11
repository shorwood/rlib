# Long-lived mutable state

## Proposition

Add a `long_lived_mutable_state` lint for local variables mutated across unrelated semantic phases.

```rust
// Bad: `entries` changes meaning while retaining one identity.
let mut entries = load_entries()?;
entries.retain(is_valid);
entries.sort_by_key(entry_key);
entries = merge_duplicates(entries);
emit(entries)?;

// Better: names expose the dataflow.
let loaded_entries = load_entries()?;
let valid_entries = loaded_entries.into_iter().filter(is_valid);
let sorted_entries = sort_entries(valid_entries);
let merged_entries = merge_duplicates(sorted_entries);
emit(merged_entries)?;
```

## Conservative detection

- Measure the source distance between a mutable binding and its final mutation.
- Increase confidence when mutations cross validated code-phase comments.
- Increase confidence when assignment changes the value's apparent semantic role or occurs in
  unrelated control-flow branches.
- Recommend a newly named value, a focused helper, or a state type according to the dataflow.

Mutation contained inside one small, named phase can remain acceptable. The target is shared
informal state, not mutation itself.

## Open decisions

- Whether crossing any phase boundary should be forbidden.
- How builder-style mutation and deliberate accumulators should be recognized.
- Whether repeated mutation count or live-range length should be the primary threshold.
