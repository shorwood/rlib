// aux-build: module_dependency_macro.rs

#![warn(bidirectional_module_dependencies)]
#![allow(dead_code, misordered_module_declarations)]

extern crate module_dependency_macro;

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

mod domain {
    pub mod request {
        use super::response::Response;

        pub struct Request(pub Response);
    }

    pub mod response {
        use super::request::Request;

        pub struct Response;

        impl Response {
            fn from_request(_: Request) -> Self {
                Self
            }
        }
    }
}

macro_rules! import_local_right {
    () => {
        use super::local_right::LocalRight;
    };
}

mod local_left {
    import_local_right!();

    pub struct LocalLeft(pub LocalRight);
}

mod local_right {
    use super::local_left::LocalLeft;

    pub struct LocalRight;

    impl LocalRight {
        fn from_left(_: LocalLeft) -> Self {
            Self
        }
    }
}

mod external_left {
    module_dependency_macro::import_external_right!();

    pub struct ExternalLeft(pub ExternalRight);
}

mod external_right {
    use super::external_left::ExternalLeft;

    pub struct ExternalRight;

    impl ExternalRight {
        fn from_left(_: ExternalLeft) -> Self {
            Self
        }
    }
}

fn main() {}
