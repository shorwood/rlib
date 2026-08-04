#![warn(nested_tuple_types)]
#![allow(dead_code, misordered_module_declarations)]

struct Participant;
struct NameTokens;

type Affected<'a> = &'a [(&'a Participant, NameTokens)];

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
