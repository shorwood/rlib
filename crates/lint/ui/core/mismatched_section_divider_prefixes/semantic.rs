#![allow(dead_code)]
#![warn(mismatched_section_divider_prefixes)]

fn main() {}

// -----------------------------------------------------------------------------
// Req: Request construction
// -----------------------------------------------------------------------------

struct Request;
struct RequestBuilder;

// -----------------------------------------------------------------------------
// Payload: Response payload model
// -----------------------------------------------------------------------------

struct Payload;
struct Response;

// -----------------------------------------------------------------------------
// Standalone: Independent model
// -----------------------------------------------------------------------------

struct Standalone;

// -----------------------------------------------------------------------------
// Violation: Diagnostic evidence
// -----------------------------------------------------------------------------

struct Violation;
struct ConflictEvidence;

mod serde {
    mod contracts {
        // -----------------------------------------------------------------------------
        // SerdeAttributes: Authored wire policy recovery
        // -----------------------------------------------------------------------------

        struct SerdeDirection;
        struct SerdeAttributes;
        struct SerdeFlag;
    }
}

// -----------------------------------------------------------------------------
// Handler: Request and response operations
// -----------------------------------------------------------------------------

fn handler_parse() {}
fn render_response() {}

mod identifier_case {
    // -----------------------------------------------------------------------------
    // IdentifierCase: Identifier spelling operations
    // -----------------------------------------------------------------------------

    pub fn words() {
        super::render_response();
    }
    pub fn is_pascal() {}
}
