#![warn(bare_tuple_types)]
#![allow(dead_code, misordered_module_declarations)]

struct Accepted;

struct Rejected;

type PartitionAlias = (Accepted, Rejected);

struct Stored {
    partition: (Accepted, Rejected),
}

type Unit = ();

type Singleton = (Accepted,);

type NestedOnly = Option<(Accepted, Rejected)>;

type Callable = dyn Fn(Accepted, Rejected) -> Accepted;

trait PartitionContract {
    type Output;

    fn partition() -> (Accepted, Rejected);
}

struct Contract;

impl PartitionContract for Contract {
    type Output = (Accepted, Rejected);

    fn partition() -> (Accepted, Rejected) {
        (Accepted, Rejected)
    }
}

static STORED_PARTITION: (Accepted, Rejected) = (Accepted, Rejected);

fn partition(input: (Accepted, Rejected)) -> (Accepted, Rejected) {
    let annotated: (Accepted, Rejected) = input;
    let closure = |value: (Accepted, Rejected)| value;
    let producing = || -> (Accepted, Rejected) { (Accepted, Rejected) };
    let _ = producing();
    closure(annotated)
}

fn main() {}

macro_rules! generated_tuple_alias {
    () => {
        type GeneratedTupleAlias = (Accepted, Rejected);
    };
}

generated_tuple_alias!();
