#![allow(dead_code)]
#![warn(overloaded_declaration_sections)]

// -----------------------------------------------------------------------------
// Accepted: Boundary-sized family
// -----------------------------------------------------------------------------

struct Accepted;
struct AcceptedBuilder;
struct AcceptedConfig;
struct AcceptedError;
struct AcceptedInput;

// -----------------------------------------------------------------------------
// Transport: Mixed transport concepts
// -----------------------------------------------------------------------------

struct Request;
struct RequestBuilder;
struct RequestHeaders;
struct Response;
struct ResponseBuilder;
struct TransportError;

// -----------------------------------------------------------------------------
// Cohesive: Single naming family
// -----------------------------------------------------------------------------

struct Cohesive;
struct CohesiveBuilder;
struct CohesiveConfig;
struct CohesiveError;
struct CohesiveInput;
struct CohesiveOutput;

mod module_namespace {
    // -----------------------------------------------------------------------------
    // ModuleNamespace: Module-owned operations
    // -----------------------------------------------------------------------------

    fn first() {}
    fn second() {}
    fn third() {}
    fn fourth() {}
    fn fifth() {}
    fn sixth() {}
}

// -----------------------------------------------------------------------------
// Collapsed: One declaration with several implementations
// -----------------------------------------------------------------------------

struct Collapsed;

impl Collapsed {
    fn first(&self) {}
}

impl Collapsed {
    fn second(&self) {}
}

fn main() {}
