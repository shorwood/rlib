# `rlib::malformed_code_phase_comments`

## Summary

Validates explanatory comment blocks inside named functions and methods.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Validates explanatory comment blocks inside named functions and methods. The first line must
use the configured prefix and sentence-style prose, continuation lines may wrap naturally,
and the block must immediately precede the code it explains. Subsequent blocks require a
separating blank line.

## Why this matters

Decorative, empty, or inconsistently placed comments do not explain the code they divide.
Natural prose makes a boundary useful to a reader instead of turning it into a branded or
mechanical line-count escape hatch.

This header has no purpose and is separated from its code:

## Examples

### Triggers the lint

```rust
fn prepare() {
    //

    let input = String::new();
}
```

### Use this instead

Put concise sentence-style prose directly before the phase:

```rust
fn prepare() {
    // Read and normalize the input state.
    let input = String::new();
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
