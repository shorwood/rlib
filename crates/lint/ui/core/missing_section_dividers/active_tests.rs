// compile-flags: --test

#![allow(dead_code)]
#![warn(rlib::missing_section_dividers)]

mod missing_case {
    #[cfg(test)]
    mod tests {
        #[test]
        fn works() {}
    }
}

mod covered_case {
    // -----------------------------------------------------------------------------
    // Tests: In-source tests
    // -----------------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        #[test]
        fn works() {}

        #[test]
        fn covers_an_unrelated_scenario() {}
    }
}
