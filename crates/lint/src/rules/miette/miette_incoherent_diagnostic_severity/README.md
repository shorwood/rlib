# miette_incoherent_diagnostic_severity

## What it does

Finds statically advisory Miette diagnostic types used as the error channel of `Result`-returning
functions and methods, including through standard owning pointer wrappers. A type-level enum
severity is included because it applies to every variant.

## Why is this bad?

Declaring a warning or advice while propagating it as a failure gives reporters and control flow
contradictory classifications.

## Example

```rust,ignore
#[diagnostic(severity(Warning))]
struct DeprecatedInput;

fn validate() -> Result<(), DeprecatedInput> { todo!() }
```

## Use instead

Use error severity for failures, or collect the diagnostic as advisory output while continuing.

```rust,ignore
fn validate() -> Result<Vec<DeprecatedInput>, ValidationError> { todo!() }
```
