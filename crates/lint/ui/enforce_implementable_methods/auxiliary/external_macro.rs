#![crate_type = "lib"]

#[macro_export]
macro_rules! external_function {
    ($receiver:ty) => {
        fn from_external_macro(value: &$receiver) {
            let _ = value;
        }
    };
}

#[macro_export]
macro_rules! external_struct {
    () => {
        struct ExternalStruct;
    };
}
