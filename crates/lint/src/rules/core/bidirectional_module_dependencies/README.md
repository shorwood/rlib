# bidirectional_module_dependencies

## What it does

Finds sibling modules that import from each other in both directions. Dependencies between
parents and children are not considered because child modules routinely use names defined by
their parent.

## Why is this bad?

A two-way dependency leaves neither sibling as the lower-level abstraction. Moving shared
concepts into a neutral module or choosing one dependency direction makes ownership clearer
and lets either side evolve without preserving an accidental cycle.

For example, these sibling modules form a cycle:

## Example

```rust
mod parser {
    use super::syntax::Syntax;
    pub struct Parser(pub Syntax);
}
mod syntax {
    use super::parser::Parser;
    pub struct Syntax;
    impl Syntax { fn parse(_: Parser) {} }
}
```

## Use instead


Extract shared concepts or move the coordinating behavior so imports flow one way:

```rust
mod syntax { pub struct Syntax; }
mod parser {
    use super::syntax::Syntax;
    pub struct Parser(pub Syntax);
}
```
