# `rlib::missing_code_phase_comments`

## Summary

Finds oversized phases in named functions and methods once the author has established phase boundaries.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds oversized phases in named functions and methods once the author has established phase
boundaries. A blank line or an existing phase comment establishes a boundary; concise phases
need no narration, and the lint does not invent phases in uninterrupted code merely because a
line threshold was crossed. Nested hand-written blocks are measured independently, while
declarative struct literals and literal-only match mappings count as one operation regardless
of formatting.

## Why this matters

Once a hand-written phase itself becomes long, an unexplained boundary makes readers infer both
its purpose and where its responsibility should end. Natural prose can name a real transition;
extraction is preferable when a named phase still contains too much work. Concise phases and
purely uninterrupted work need no generated narration.

For a three-line limit, the oversized first phase has no explanation:

## Examples

### Triggers the lint

```rust
fn prepare() {
    let input = String::new();
    let trimmed = input.trim();
    let length = trimmed.len();
    let empty = trimmed.is_empty();

    consume(length, empty);
}
```

### Use this instead

Name the real transition and keep each phase below the configured limit:

```rust
fn prepare() {
    // Read and normalize the input.
    let input = String::new();
    let trimmed = input.trim();

    // Derive the properties consumed by the caller.
    let length = trimmed.len();
    let empty = trimmed.is_empty();

    consume(length, empty);
}
```

## What it skips

A blank line or an existing phase comment establishes a boundary; concise phases need no narration, and the lint does not invent phases in uninterrupted code merely because a line threshold was crossed.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `function-phase-lines-threshold` | positive integer | `7` | Sets how many lines a function phase may contain before it needs a short heading. |

## Known limitations

No known implementation limitations.

## Related lints

None.
