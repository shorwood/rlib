// compile-flags: --test

#![warn(visibility_required_only_by_tests)]
#![allow(dead_code)]

mod parser {
    pub(crate) fn normalize(input: &str) -> String {
        input.trim().to_owned()
    }

    pub(crate) fn production_api() {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn reaches_an_internal_detail() {
        assert_eq!(super::parser::normalize(" value "), "value");
    }
}

fn production_caller() {
    parser::production_api();
}
