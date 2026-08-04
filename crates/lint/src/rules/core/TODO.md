Rule #2: Enforce Implementable Methods on vectors
When defining a struct, every method that can be implemented for that struct should be implemented in the impl block for that struct. This rule also applies to methods that can be implemented on vectors of that struct.

Given this example:

```rs
fn do_something(mystructs: Vec<MyStruct>, ...) {...}
fn do_something(mystructs: &[MyStruct], ...) {...}
```

The rule should forbid it and ensures that the developer creates a new struct that wraps the vector and implements the method in the impl block for that struct instead:

```rs
struct MyStructs {
    mystructs: Vec<MyStruct>,
}

impl MyStructs {
    fn do_something(&self) {...}
}
```

---

Rule #2: Struct Impl Colocation
When defining a struct, every impl block for that struct should be colocated with the struct definition.

---

Rule #3: Struct Impl Definition Order
When defining a struct, the impl block for that struct should be defined immediately after the struct definition.

---

Rule #5: Ensure blocks of continuous code with more than N lines are separated with a comment line
When defining a function, any block of continuous code that exceeds N lines should be separated with a newline and comment line. This helps to improve code readability and maintainability.

This rule can be customized with 2 parameters:
- N: The maximum number of lines allowed in a block of continuous code before a comment line
- Prefix: The prefix to use for the comment line that separates the block of code

For example, if N is set to 3 and the prefix is set to `// ---`, then the following code would be considered valid:
```rs
fn example_function() {
    // --- This is a comment line separating the block of code
    let x = 10;
    let y = 20;
    let z = x + y;

    // --- This is another comment line separating the block of code
    let a = 30;
    let b = 40;
    let c = a + b;
}
```

---

Rule #6: Type dependency ordering.
When defining a struct or a type, any types that are used as fields in the struct or type should be defined before the struct or type definition. This ensures that the code is organized and easy to read.

```rs
// BAD

#[derive(Clone, Debug)]
pub(crate) struct SlotArgument {
    pub name: String,
    pub default: bool,
    pub content: SlotContent,
}

#[derive(Clone, Debug)]
pub(crate) enum SlotContent {
    Elements(Vec<Element>),
    Forwarded(String),
}
```

```rs
// Good

#[derive(Clone, Debug)]
pub(crate) enum SlotContent {
    Elements(Vec<Element>),
    Forwarded(String),
}

#[derive(Clone, Debug)]
pub(crate) struct SlotArgument {
    pub name: String,
    pub default: bool,
    pub content: SlotContent,
}
```

---

Rule #7: Match file name with fn, struct, const or trait names.

Given a file name `my_thing.rs`, every function, struct, const or trait defined in that file should have a name that matches the file name. This ensures that the code is organized and easy to read.

```rs
// BAD
// File name: my_thing.rs
struct MyStruct {
    ...
}

type MyType {
    ...
}

trait MyTrait {
    ...
}

const MY_CONSTANT: &str = "my_constant";
```

```rs

// Good
// File name: my_thing.rs
struct MyThing {
    ...
}

struct MyThingAndOtherStuff {
    ...
}

trait MyThingTrait {
    ...
}

const MY_THING_CONSTANT: &str = "my_thing";
```

---

Rule: Ensure `mod.rs` is only a barrel file and does not contain any other code.

```rs
// BAD
mod my_module1;
mod my_module2;

fn my_function() {
    ...
}
```

```rs
// Good
mod my_module1;
mod my_module2;
```
