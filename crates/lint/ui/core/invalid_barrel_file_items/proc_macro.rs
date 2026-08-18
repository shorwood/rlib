// compile-flags: --crate-type=proc-macro

#![warn(rlib::invalid_barrel_file_items, rlib::foreign_type_method_like_free_functions)]

include!("auxiliary/proc_macro/lib.rs");
