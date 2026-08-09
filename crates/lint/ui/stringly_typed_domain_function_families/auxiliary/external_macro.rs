#![crate_type = "lib"]

#[macro_export]
macro_rules! external_domain_family {
    () => {
        fn validate_external_key(_key: &str) -> Result<(), ()> {
            Ok(())
        }

        fn normalize_external_key(key: &str) -> String {
            key.to_owned()
        }
    };
}
