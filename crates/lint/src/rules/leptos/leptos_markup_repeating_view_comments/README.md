# leptos_markup_repeating_view_comments

## What it does

Checks for one-node view sections whose heading only repeats the node’s name or visible label.

## Why is this bad?

Comments are useful when they add meaning that the code cannot already express. Repeating the
markup makes the view longer without helping readers understand why the region exists.

## Example

```rust,ignore
view! {
    // Navigation
    <Navigation/>

    // Submit button
    <button>"Submit"</button>
}
```

## Use instead

Remove the comment when the markup is already clear, or describe the region’s responsibility:

```rust,ignore
view! {
    // Account shortcuts
    <Navigation/>

    <button>"Submit"</button>
}
```
