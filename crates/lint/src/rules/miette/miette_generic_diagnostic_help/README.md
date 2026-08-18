# `rlib::miette_generic_diagnostic_help`

## Summary

Finds static Miette help text that consists only of a configured generic phrase.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds static Miette help text that consists only of a configured generic phrase.

## Why this matters

Advice such as “try again” or “contact support” does not explain which concrete action can resolve
the reported condition.

## Examples

### Triggers the lint

```rust,ignore
#[diagnostic(help("try again"))]
```

### Use this instead

```rust,ignore
#[diagnostic(help("set `config_path` to an existing readable file"))]
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `miette-generic-help-phrases` | string list | built-in list | Lists help text that is too vague. Use `".."` to keep the built-in entries. |

## Known limitations

No known implementation limitations.

## Related lints

None.
