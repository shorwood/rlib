# single_implementation_traits

## What it does

Finds authored, non-generic local traits with exactly one concrete local implementation and
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

## Why is this bad?

A trait without an observed substitution or abstract boundary duplicates the concrete API
while adding navigation, indirection, and maintenance cost. Concrete calls do not by
themselves demonstrate polymorphism, and prose about possible future implementations does
not make the current contract substitutable.

## Example

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

## Use instead


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

The lint is deliberately compilation-local. Downstream users and disabled configurations
may contain implementations or polymorphic consumers that are not observable here. It does
not offer an automatic rewrite because removing a trait can require changing bounds, method
resolution, associated items, and public documentation together.
