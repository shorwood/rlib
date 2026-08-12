// compile-flags: --test

#![allow(dead_code)]
#![warn(missing_section_dividers)]

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
    }
}
