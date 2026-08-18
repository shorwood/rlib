#![allow(dead_code)]
#![warn(rlib::incoherent_type_family_names)]

// -----------------------------------------------------------------------------
// MethodLikeFreeFunctions: Family naming fixture
// -----------------------------------------------------------------------------

struct MethodLikeFreeFunctions;

struct MethodLikeFreeFunctionsSourceEdits;

struct MethodLikeFreeFunctionsMigrationBuilder {
    edits: MethodLikeFreeFunctionsSourceEdits,
}

fn main() {}
