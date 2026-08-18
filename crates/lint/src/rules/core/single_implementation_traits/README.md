# `rlib::single_implementation_traits`

## Summary

Finds hand-written, non-generic local traits with exactly one concrete local implementation and no active polymorphic consumer.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds hand-written, non-generic local traits with exactly one concrete local implementation and
no active polymorphic consumer. A consumer is evidence that code depends on the abstraction:
a generic bound, `impl Trait`, a trait object, an associated-type projection, a dependent
supertrait, or a trait alias. Calling a trait method on the sole concrete implementation,
including with UFCS, is not polymorphic evidence because the caller still knows the type.

Marker, unsafe, auto, and semantically sealed traits are preserved, as are focused extension
traits for foreign types and implementations over generic or blanket targets. Traits with
zero or multiple implementations, generated declarations, and unrestricted public APIs in
publishable libraries are also outside the lint. Binaries, private APIs, and packages marked
`publish = false` are analyzed as closed code. Active test implementations and consumers
count as real evidence; inactive `cfg` branches are not visible to the compiler invocation.

## Why this matters

A trait with only one implementation and no caller that accepts alternatives duplicates the
concrete API while adding another place to navigate and maintain. Possible future implementations
do not make the trait useful today.

## Examples

### Triggers the lint

```rust
trait ReportSink {
    fn write(&self, report: &str);
}

struct FileSink;

impl ReportSink for FileSink {
    fn write(&self, report: &str) {
        println!("{report}");
    }
}

fn emit(sink: &FileSink) {
    sink.write("ready");
}
```

### Use this instead

Until a caller accepts something such as `&impl ReportSink` or `&dyn ReportSink`, put the
behavior on the concrete type:

```rust
struct FileSink;

impl FileSink {
    fn write(&self, report: &str) {
        println!("{report}");
    }
}

fn emit(sink: &FileSink) {
    sink.write("ready");
}
```

The lint only sees code compiled in the current run. Downstream users and disabled configurations
may contain implementations or callers that accept several implementations. It does
not offer an automatic rewrite because removing a trait can require changing bounds, method
resolution, associated items, and public documentation together.

## What it skips

Calling a trait method on the sole concrete implementation, including with UFCS, is not polymorphic evidence because the caller still knows the type. Traits with zero or multiple implementations, generated declarations, and unrestricted public APIs in publishable libraries are also outside the lint. Active test implementations and consumers count as real evidence; inactive `cfg` branches are not visible to the compiler invocation.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::needless_delegating_types`](../needless_delegating_types/README.md) — Covers wrapper types that add no distinct behavior.
- [`rlib::unconsumed_generic_abstractions`](../unconsumed_generic_abstractions/README.md) — Covers generic choices that callers do not use.
