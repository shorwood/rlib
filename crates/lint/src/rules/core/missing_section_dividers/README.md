# missing_section_dividers

## What it does

Requires module-level declaration groups to be covered by a configured section divider.
Nominal types and their direct impls always participate. Free functions, constants, and
statics also participate when they form a group or occur inside an authored section; an
isolated value declaration does not require a divider by itself.

## Why is this bad?

A divider makes the intended naming family explicit. Without one, agents cannot tell
whether neighboring declarations are deliberately related or merely accumulated in the
same file. Ignoring free helpers also lets a nominally valid section conceal inconsistent
vocabulary.

For example, these declarations have no stated family:

## Example

```rust
struct Request;
impl Request {}
```

## Use instead


A divider establishes the naming contract:

```rust
// -----------------------------------------------------------------------------
// Request: Request model and behavior
// -----------------------------------------------------------------------------

struct Request;
impl Request {}
```
