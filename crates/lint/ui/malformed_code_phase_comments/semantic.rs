#![warn(malformed_code_phase_comments)]
#![allow(dead_code, misordered_module_declarations)]

fn empty_content() {
    //
    let _value = 1;
}

fn loud_content() {
    // THIS IS LOUD
    let _value = 1;
}

fn noncanonical_prefix() {
    //  Normalize prefix spacing
    let _value = 1;
}

fn detached_header() {
    // Prepare the value.

    let _value = 1;
}

fn missing_blank_line() {
    let _first = 1;
    // Prepare the second value.
    let _second = 2;
}

fn canonical_headers() {
    // Prepare the first value using `u8` semantics and HIR context.
    // Continuation lines may wrap naturally without becoming new headers.
    let _first = 1;

    // NOTE: Prepare the second value after the first phase.
    let _second = 2;
}

#[allow(malformed_code_phase_comments)]
fn explicitly_allowed() {
    // THIS IS ALLOWED
    let _value = 1;
}

fn closure_is_independent() {
    let _closure = || {
        // THIS IS INSIDE A CLOSURE
        let _value = 1;
    };
}

macro_rules! generated_phase_comment {
    () => {
        fn generated_phase_comment() {
            // THIS IS GENERATED
            let _value = 1;
        }
    };
}

generated_phase_comment!();

fn main() {}
