# `rlib::unencapsulated_binary_enum_classification`

## Summary

Finds `if` expressions that choose both variants of a local enum with exactly two unit variants when that mapping is written outside the enum's inherent implementation.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `if` expressions that choose both variants of a local enum with exactly two unit
variants when that mapping is written outside the enum's inherent implementation. Both
value-producing branches and branches that explicitly `return` the selected variant are
recognized. Enums with payloads, enums with any other variant count, partial mappings,
macro expansions, and mappings owned by the enum itself remain valid.

## Why this matters

Choosing between all possible variants is a conversion behavior, not incidental call-site
control flow. Repeating that mapping away from the enum makes its polarity easy to reverse
and gives readers no standard place to learn how the source concept becomes domain state.
The compact conditional also encourages later call sites to reproduce the same knowledge.

For example, this helper owns the meaning of `returns_unit` even though the result type
should own that meaning:

## Examples

### Triggers the lint

```rust
enum FunctionReturn {
    Unit,
    Value,
}

# let returns_unit = true;
let function_return = if returns_unit {
    FunctionReturn::Unit
} else {
    FunctionReturn::Value
};
# let _ = function_return;
```

### Use this instead

Put the classification beside the enum and name the source concept. Prefer passing the
source value itself when doing so keeps the predicate private:

```rust
enum FunctionReturn {
    Unit,
    Value,
}

struct FunctionOutput {
    is_unit: bool,
}

impl FunctionOutput {
    fn is_unit(&self) -> bool {
        self.is_unit
    }
}

impl FunctionReturn {
    fn from_output(output: &FunctionOutput) -> Self {
        if output.is_unit() { Self::Unit } else { Self::Value }
    }
}

# let output = FunctionOutput { is_unit: true };
let function_return = FunctionReturn::from_output(&output);
# let _ = function_return;
```

This lint intentionally does not suggest `From<bool>` or apply an automatic fix: choosing
the constructor's behavioral name and deciding whether it should accept a richer source type
require domain judgment.

## What it skips

Finds `if` expressions that choose both variants of a local enum with exactly two unit variants when that mapping is written outside the enum's inherent implementation. Enums with payloads, enums with any other variant count, partial mappings, macro expansions, and mappings owned by the enum itself remain valid.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
