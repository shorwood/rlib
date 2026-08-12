// compile-flags: --crate-type=proc-macro
// edition:2024

#![warn(invalid_barrel_file_items, foreign_type_method_like_free_functions)]

include!("auxiliary/proc_macro/lib.rs");
