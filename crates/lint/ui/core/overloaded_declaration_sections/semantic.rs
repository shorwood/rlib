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

// -----------------------------------------------------------------------------
// Transport
// -----------------------------------------------------------------------------

struct Request;
struct RequestBuilder;
struct RequestHeaders;
struct Response;
struct ResponseBuilder;
struct TransportError;

// -----------------------------------------------------------------------------
// Cohesive
// -----------------------------------------------------------------------------

struct Cohesive;
struct CohesiveBuilder;
struct CohesiveConfig;
struct CohesiveError;
struct CohesiveInput;
struct CohesiveOutput;

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
