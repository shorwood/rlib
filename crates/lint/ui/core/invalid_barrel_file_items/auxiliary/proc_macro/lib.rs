extern crate proc_macro;

mod expansion;

/// Compiler-mandated proc-macro entry point retained in the crate root.
#[proc_macro]
pub fn pencil(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    input
}
