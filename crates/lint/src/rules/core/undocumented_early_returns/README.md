# undocumented_early_returns

## What it does

Requires an authored explicit mid-flow `return` in a named function or method to have an
explanatory code-phase comment. Entry validation and analysis guards remain self-documenting; the
rule starts once an earlier explicit state mutation in the same block and a substantial remaining
phase make the exit's consequence nonlocal. “Substantial” uses the shared function-phase line
limit. The comment belongs immediately above the controlling `if`, `if let`, `let ... else`, or
match arm when possible; a comment above the return itself is accepted as a fallback. Tail-only
returns, closures, macro output, `?`, `break`, and `continue` are excluded.

## Why is this bad?

Early returns keep the main path flat, but each one introduces an exit policy that a reader must
account for. Naming the reason or consequence at the guard preserves information that the condition
alone often cannot communicate. Existing code-phase comment rules continue to own comment syntax
and placement, so malformed comments receive one focused diagnostic.

## Example

```rust
fn render(input: &str, attempts: &mut usize) -> Option<String> {
    *attempts += 1;
    if input.is_empty() {
        return None;
    }
    let parsed = parse(input)?;
    let normalized = normalize(parsed);
    let validated = validate(normalized)?;
    let enriched = enrich(validated);
    let formatted = format_value(enriched);
    let rendered = render_value(formatted);
    Some(rendered)
}
```

## Use instead

```rust
fn render(input: &str, attempts: &mut usize) -> Option<String> {
    *attempts += 1;

    // Empty input has no renderable value.
    if input.is_empty() {
        return None;
    }
    let parsed = parse(input)?;
    let normalized = normalize(parsed);
    let validated = validate(normalized)?;
    let enriched = enrich(validated);
    let formatted = format_value(enriched);
    let rendered = render_value(formatted);
    Some(rendered)
}
```
