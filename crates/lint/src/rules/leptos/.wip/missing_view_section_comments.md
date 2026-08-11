# Missing view section comments

## Proposition

Add a `missing_view_section_comments` lint for complex runs of direct sibling nodes in Leptos
`view!` trees that do not name their visual or behavioral regions.

```rust
// Bad: several responsibilities form one anonymous stream.
view! {
    <main>
        <AccountNavigation/>
        <Breadcrumbs/>

        <ResourceBoundary resource=account>
            <AccountSummary/>
            <SubscriptionStatus/>
            <RecentActivity/>
        </ResourceBoundary>

        <DeleteAccountButton/>
        <SignOutButton/>
    </main>
}
```

```rust
// Better: stable regions are visible without opening every component.
view! {
    <main>
        // Account navigation
        <AccountNavigation/>
        <Breadcrumbs/>

        // Account overview
        <ResourceBoundary resource=account>
            <AccountSummary/>
            <SubscriptionStatus/>
            <RecentActivity/>
        </ResourceBoundary>

        // Destructive account actions
        <DeleteAccountButton/>
        <SignOutButton/>
    </main>
}
```

## Rationale

A long declarative view is difficult for a reader to scan for the same reason as a long imperative
function: distinct responsibilities are adjacent but unnamed. In a view, those responsibilities are
visual regions, interaction zones, loading boundaries, navigation areas, or user-visible states.

Comments are not substitutes for semantic HTML or component names. They provide a source-level map
when several direct children collectively express one responsibility that no individual tag or
component name can carry.

## Direct-scope analysis

Each sibling list should be analyzed independently:

- a parent counts only its direct children and direct reactive constructs;
- nested element children are analyzed as their own scope;
- children inside `For`, `Show`, `Suspense`, `ErrorBoundary`, and component-like control flow are not
  charged again to the parent;
- blank lines affect placement but never create section boundaries;
- once direct complexity crosses the threshold, the first section also requires a heading;
- a valid section extends until the next valid heading or the end of the sibling scope.

This mirrors direct code-phase analysis and avoids duplicate parent/child findings.

## Complexity model

Physical source lines are a poor measure because formatting can expand one element across many
lines. A configurable direct complexity score is more stable:

| Direct construct | Suggested weight |
|---|---:|
| Native element | 1 |
| Component child | 1 |
| Reactive child expression | 1 |
| Child with event behavior | 1 additional |
| `For`, `Show`, `Suspense`, or `ErrorBoundary` | 2 |
| Conditional or matched view expression | 2 |
| Dense opening tag | 1 additional |

The score should measure navigation burden, not runtime cost.

## Comment semantics

Headings should identify responsibility:

```rust
// Weak
// Components
// Buttons
// Content

// Strong
// Account navigation
// Subscription status and recent activity
// Destructive account actions
```

A one-word structural heading such as `Navigation` may be acceptable when the surrounding component
already supplies the missing context. Configuration or future inference can decide how much context
must be repeated.

## Diagnostic direction

> This view contains several unnamed visual responsibilities. Name its stable regions or extract a
> component whose name carries that responsibility; do not divide arbitrary adjacent nodes merely
> to satisfy the lint.

The diagnostic must not generate headings automatically because deciding the region name is the
design work the lint exists to demand.

## Open decisions

- The default complexity threshold and weights.
- Whether every complex scope requires a heading before its first child.
- Whether one-child sections are allowed when the child is itself a complex control-flow boundary.
- Whether generic headings are rejected here or by `malformed_view_section_comments`.
- How fragments returned from helper macros preserve direct-scope ownership.
