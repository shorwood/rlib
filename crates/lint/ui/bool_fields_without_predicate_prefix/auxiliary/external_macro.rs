// force-host
// no-prefer-dynamic

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::TokenStream;

// Generate a deliberately invalid field name to verify that external expansions are ignored.
#[proc_macro]
pub fn external_struct(_: TokenStream) -> TokenStream {
    "struct FromExternalMacro { active: bool }"
        .parse()
        .expect("the test fixture should produce valid Rust")
}
