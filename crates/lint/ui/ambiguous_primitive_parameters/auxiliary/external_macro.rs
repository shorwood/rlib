#![crate_type = "lib"]

#[macro_export]
macro_rules! external_ambiguous_signature {
    () => {
        fn external_assign(account: u32, project: u32) {}
    };
}
