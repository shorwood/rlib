#![allow(dead_code)]
#![warn(rlib::incoherent_type_family_names)]

macro_rules! generated_collision {
    () => {
        struct Tarjan;
    };
}

generated_collision!();

// -----------------------------------------------------------------------------
// DeclarationNode: Family naming fixture
// -----------------------------------------------------------------------------

struct DeclarationNode;

struct DeclarationNodeTarjan;

fn main() {}
