Rule #1: Forbid boolean props

When defining a component, forbid any kind of boolean prop and ensure that the developer uses an enum, even if it has only two variants. This ensures that the prop is more descriptive and can be extended in the future if needed.

Given this example:

```rs
// BAD
#[component]
fn MyComponent(is_active: bool) {...}

// GOOD
enum Status {
    Active,
    Inactive,
}

#[component]
fn MyComponent(status: Status) {...}
```
