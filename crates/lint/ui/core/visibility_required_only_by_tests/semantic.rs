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
mod test {
    #[test]
    fn reaches_an_internal_detail() {
        assert_eq!(super::parser::normalize(" value "), "value");
    }
}

mod production_parser {
    pub(crate) fn normalize(input: &str) -> String {
        input.trim().to_owned()
    }
}

mod tests {
    fn production_consumer() {
        let _ = super::production_parser::normalize(" production ");
    }
}

fn production_caller() {
    parser::production_api();
}
