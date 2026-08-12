#![feature(decl_macro)]

pub macro external_impl($target:ty) {
    impl $target {}
}
