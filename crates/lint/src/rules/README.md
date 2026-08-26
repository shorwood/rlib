# Lint catalog

This catalog lists every lint provided by rlib. Each lint page explains what causes the warning,
why it matters, what to use instead, and when the rule may not fit a project.

## Reading a lint page

The public lint name is written as `rlib::<name>`. Use Rust lint attributes to change its level:

```rust
#![warn(rlib::all)]
#![allow(rlib::one_lint_name)]
```

A lint's Cargo feature controls whether it is available. Its Rust lint level still controls whether
it is allowed, shown as a warning, or treated as an error.

## Lint purposes

| Purpose | Meaning |
| --- | --- |
| Correctness | The code is likely to behave differently from what its author intended. |
| Safety | The code can create a security, privacy, panic, resource, concurrency, or lifecycle risk. |
| API design | A public type or function is difficult to use or change safely. |
| Code clarity | The code is difficult to find, understand, test, or modify. |
| Style | The name, formatting, or layout differs without changing behavior. |

## Fixes

| Fix | Meaning |
| --- | --- |
| Automatic | Every reported case has a machine-applicable edit. |
| Partial | Some reported cases have a machine-applicable edit; coordinated refactors remain manual. |
| Needs review | The compiler can suggest an edit, but some cases need a person to check it. |
| Manual | The compiler explains the problem but does not edit the code. |

## Lint families

| Family | Cargo feature | What it covers |
| --- | --- | --- |
| [Core](./core/README.md) | always | General Rust rules for clear modules, APIs, control flow, names, documentation, and error handling. |
| [Bon](./bon/README.md) | `bon` | Rules for Bon builders, including construction safety, public API shape, compatibility, and generated member behavior. |
| [Derive More](./derive_more/README.md) | `derive_more` | Rules that compare hand-written trait code with Derive More and protect invariants when derives create new operations. |
| [Framework](./framework/README.md) | `framework` | Rules that ask projects to choose which derive crate owns an overlapping generated interface. |
| [Leptos](./leptos/README.md) | `leptos` | Rules for Leptos components, views, reactivity, event handlers, resources, and server functions. |
| [Leptos i18n](./leptos_i18n/README.md) | `leptos_i18n` | Rules that keep user-visible Leptos text in the localization catalog. |
| [Leptos Styling](./leptos_styling/README.md) | `leptos_styling` | Rules for colocated, scoped, typed, and consistently formatted Leptos component styles. |
| [Miette](./miette/README.md) | `miette` | Rules for useful, safe, and consistent Miette diagnostics. |
| [Serde](./serde/README.md) | `serde` | Rules for stable Serde data formats and round-trip behavior. |
| [Strum](./strum/README.md) | `strum` | Rules that replace repeated enum helpers with Strum derives while preserving names, values, and iteration behavior. |
| [thiserror](./thiserror/README.md) | `thiserror` | Rules for clear thiserror messages, complete source chains, safe public errors, and replaceable hand-written code. |
