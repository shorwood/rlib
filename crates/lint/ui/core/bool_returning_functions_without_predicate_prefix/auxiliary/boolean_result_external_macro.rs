// force-host
// no-prefer-dynamic

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::TokenStream;

// Generate an invalid name to verify that external expansions remain ignored.
#[proc_macro]
pub fn external_function(_: TokenStream) -> TokenStream {
    "fn generated_ready() -> bool { true }"
        .parse()
        .expect("the test fixture should produce valid Rust")
}
