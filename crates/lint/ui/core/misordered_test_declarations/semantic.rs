#![warn(rlib::misordered_test_declarations)]
#![allow(dead_code)]
// compile-flags: --test

mod nonterminal {
    #[cfg(test)]
    mod tests {
        #[test]
        fn works() {}
    }

    fn production_declaration() {}
}

mod duplicated {
    #[cfg(test)]
    mod test {
        #[test]
        fn first() {}
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn second() {}
    }
}

mod terminal {
    fn production_declaration() {}

    #[cfg(test)]
    mod tests {
        #[test]
        fn works() {}
    }
}

mod production_module_named_tests {
    mod tests {}

    fn production_declaration() {}
}

macro_rules! generated_tests {
    () => {
        #[cfg(test)]
        mod tests {}
    };
}

mod generated {
    generated_tests!();

    fn production_declaration() {}
}

#[allow(rlib::misordered_test_declarations)]
mod explicitly_allowed {
    #[cfg(test)]
    mod tests {}

    fn production_declaration() {}
}
