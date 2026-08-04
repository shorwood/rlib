#![warn(bidirectional_module_dependencies)]
#![allow(dead_code, misordered_module_declarations)]

mod parser {
    use super::syntax::Syntax;

    pub struct Parser(pub Syntax);
}

mod syntax {
    use super::parser::Parser;

    pub struct Syntax;

    impl Syntax {
        fn from_parser(_: Parser) -> Self {
            Self
        }
    }
}

mod model {
    pub struct Model;
}

mod service {
    use super::model::Model;

    pub struct Service(pub Model);
}

pub struct Shared;

mod child {
    use super::Shared;

    pub struct Child(pub Shared);
}

pub use child::Child;

fn main() {}
