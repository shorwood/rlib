#![warn(bare_tuple_types)]
#![allow(dead_code, misordered_module_declarations)]

struct Accepted;
struct Rejected;

type PartitionAlias = (Accepted, Rejected);

struct Stored {
    partition: (Accepted, Rejected),
}

type Unit = ();

fn partition(input: (Accepted, Rejected)) -> (Accepted, Rejected) {
    let annotated: (Accepted, Rejected) = input;
    let closure = |value: (Accepted, Rejected)| value;
    closure(annotated)
}

fn main() {}

macro_rules! generated_tuple_alias {
    () => {
        type GeneratedTupleAlias = (Accepted, Rejected);
    };
}

generated_tuple_alias!();
