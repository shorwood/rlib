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
