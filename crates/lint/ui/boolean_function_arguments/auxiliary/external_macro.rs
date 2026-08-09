#![crate_type = "lib"]

#[macro_export]
macro_rules! external_boolean_signature {
    () => {
        fn external_render(pretty: bool) {}
    };
}
