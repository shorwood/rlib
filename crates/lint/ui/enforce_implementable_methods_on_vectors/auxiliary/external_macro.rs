#![crate_type = "lib"]

#[macro_export]
macro_rules! external_vector_function {
    ($element:ty) => {
        fn from_external_macro(items: Vec<$element>) {
            let _ = items;
        }
    };
}

#[macro_export]
macro_rules! external_struct {
    () => {
        struct ExternalItem;
    };
}
