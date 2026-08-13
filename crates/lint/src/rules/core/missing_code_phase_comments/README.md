# missing_code_phase_comments

## What it does

Finds oversized phases in named functions and methods once the author has established phase
boundaries. A blank line or an existing phase comment establishes a boundary; concise phases
need no narration, and the lint does not invent phases in uninterrupted code merely because a
line threshold was crossed. Nested hand-written blocks are measured independently, while
declarative struct literals and literal-only match mappings count as one operation regardless
of formatting.

## Why is this bad?

Once an authored phase itself becomes long, an unexplained boundary makes readers infer both
its purpose and where its responsibility should end. Natural prose can name a real transition;
extraction is preferable when a named phase still contains too much work. Concise phases and
purely uninterrupted work need no generated narration.

For a three-line limit, the oversized first phase has no explanation:

## Example

```rust
fn prepare() {
    let input = String::new();
    let trimmed = input.trim();
    let length = trimmed.len();
    let empty = trimmed.is_empty();

    consume(length, empty);
}
```

## Use instead

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
