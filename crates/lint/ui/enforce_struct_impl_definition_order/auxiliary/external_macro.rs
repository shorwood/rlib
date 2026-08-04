// force-host
// no-prefer-dynamic

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::TokenStream;

/// Generates an intentionally disordered struct and impl from code the tested crate cannot edit.
#[proc_macro]
pub fn external_layout(_: TokenStream) -> TokenStream {
    "struct External; fn external_separator() {} impl External {}"
        .parse()
        .expect("the test fixture should produce valid Rust")
}
