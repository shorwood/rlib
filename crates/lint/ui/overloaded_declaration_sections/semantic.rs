#![allow(dead_code)]
#![warn(overloaded_declaration_sections)]

// -----------------------------------------------------------------------------
// Accepted
// -----------------------------------------------------------------------------

struct Accepted;
struct AcceptedBuilder;
struct AcceptedConfig;
struct AcceptedError;
struct AcceptedInput;
struct AcceptedOutput;
struct AcceptedParser;
struct AcceptedPolicy;

// -----------------------------------------------------------------------------
// Overloaded
// -----------------------------------------------------------------------------

struct Overloaded;
struct OverloadedBuilder;
struct OverloadedConfig;
struct OverloadedError;
struct OverloadedInput;
struct OverloadedOutput;
struct OverloadedParser;
struct OverloadedPolicy;
struct OverloadedState;

mod module_namespace {
    // -----------------------------------------------------------------------------
    // ModuleNamespace
    // -----------------------------------------------------------------------------

    fn first() {}
    fn second() {}
    fn third() {}
    fn fourth() {}
    fn fifth() {}
    fn sixth() {}
    fn seventh() {}
    fn eighth() {}
    fn ninth() {}
}

// -----------------------------------------------------------------------------
// Collapsed
// -----------------------------------------------------------------------------

struct Collapsed;

impl Collapsed {
    fn first(&self) {}
}

impl Collapsed {
    fn second(&self) {}
}

fn main() {}
