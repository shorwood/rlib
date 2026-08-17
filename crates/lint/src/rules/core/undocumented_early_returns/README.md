# undocumented_early_returns

## What it does

Requires an authored explicit `return` controlled by an `if`, `if let`, `let ... else`, or match
arm to have an explanatory code-phase comment immediately before that condition. Entry guards,
short continuations, and closure-local conditions follow the same policy whenever the return
bypasses later work in their body. A comment inside the returning branch is too late because it
does not introduce the decision boundary. Tail-only conditional exits, direct returns, macro
output, `?`, `break`, and `continue` are excluded.

## Why is this bad?

Early returns keep the main path flat, but each conditional exit introduces a policy that a reader
must account for before entering either branch. Naming the reason or consequence at the condition
preserves that information where the decision is made. Existing code-phase comment rules continue
to own comment syntax and placement, so malformed comments receive one focused diagnostic.

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
