# leptos_oversized_view_sections

## What it does

Checks that one named section inside `view!` does not contain too much direct structure.

## Why is this bad?

A heading helps readers find a region, but a very large region is still difficult to scan and
change safely. Smaller sections make the important parts of a view easier to recognize.

## Example

```rust,ignore
view! {
    <main>
        // Account workspace
        <Header/>
        <Navigation/>
        <Show when=ready><Content/></Show>
        <Footer/>
    </main>
}
```

## Use instead

Split the direct children into sections that each describe one part of the interface:

```rust,ignore
view! {
    <main>
        // Account navigation
        <Header/>
        <Navigation/>

        // Account content
        <Show when=ready><Content/></Show>
        <Footer/>
    </main>
}
```

## Configuration

`leptos-view-section-complexity-threshold` sets the maximum named-section complexity (default `4`).
