# `rlib::derive_more_panic_prone_derived_variant_accessors`

## Summary

Finds calls to derive_more-generated `unwrap_*` methods when the receiver expression does not statically construct the required enum variant.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds calls to derive_more-generated `unwrap_*` methods when the receiver expression does not
statically construct the required enum variant.

## Why this matters

Generated unwrap accessors panic for every other variant. Calling one on a parameter or merged value
hides an unchecked variant assumption behind an ordinary-looking method call.

## Examples

### Triggers the lint

```rust,ignore
#[derive(derive_more::Unwrap)]
enum Response { Ok(Value), Error(Problem) }

fn render(response: Response) {
    render_value(response.unwrap_ok());
}
```

### Use this instead

Use pattern matching or derive `TryUnwrap` when the active variant is not established by construction.

```rust,ignore
match response {
    Response::Ok(value) => render_value(value),
    Response::Error(problem) => render_problem(problem),
}
```

## What it skips

Finds calls to derive_more-generated `unwrap_*` methods when the receiver expression does not statically construct the required enum variant.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
