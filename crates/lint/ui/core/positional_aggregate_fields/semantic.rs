#![warn(positional_aggregate_fields)]
#![allow(dead_code, misordered_module_declarations)]

struct Span;

struct Rename(Span, String);
struct Coordinates(i32, i32, i32);
struct EmptyTuple();

enum Finding {
    Replacement(Span, String),
    #[allow(positional_aggregate_fields)]
    ExplicitlyAllowed(Span, String),
    Newtype(String),
    Unit,
    EmptyTuple(),
    Record { span: Span, replacement: String },
}

struct UserId(u64);
struct Marker;

fn main() {}

macro_rules! generated_tuple_struct {
    () => {
        struct GeneratedTupleStruct(Span, String);
    };
}

generated_tuple_struct!();
