#![warn(rlib::nested_tuple_types)]
#![allow(dead_code, rlib::misordered_module_declarations)]

struct Participant;

struct NameTokens;

type Affected<'a> = &'a [(&'a Participant, NameTokens)];

type MixedCallable = Result<
    Vec<(Participant, NameTokens)>,
    Box<dyn Fn(Participant, NameTokens) -> Participant>,
>;

type CallableOnly = Box<dyn Fn(Participant, NameTokens) -> Participant>;

type AssociatedTuple = Box<dyn Iterator<Item = (Participant, NameTokens)>>;

struct Stored {
    affected: Vec<(&'static Participant, NameTokens)>,
}

fn affected(
    input: Option<(&Participant, NameTokens)>,
) -> Result<Vec<(&Participant, NameTokens)>, ()> {
    input.map_or(Err(()), |entry| Ok(vec![entry]))
}

fn main() {}

macro_rules! generated_nested_tuple {
    () => {
        type GeneratedNestedTuple = Vec<(Participant, NameTokens)>;
    };
}

generated_nested_tuple!();
