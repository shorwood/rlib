# `rlib::leptos_oversized_view_sections`

## Summary

Checks that one named section inside `view!` does not contain too much direct structure.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks that one named section inside `view!` does not contain too much direct structure.

## Why this matters

A heading helps readers find a region, but a very large region is still difficult to scan and
change safely. Smaller sections make the important parts of a view easier to recognize.

## Examples

### Triggers the lint

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

### Use this instead

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

## What it skips

Checks that one named section inside `view!` does not contain too much direct structure.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-view-section-complexity-threshold` | positive integer | `4` | Sets the largest allowed named view section. |

## Known limitations

No known implementation limitations.

## Related lints

None.
