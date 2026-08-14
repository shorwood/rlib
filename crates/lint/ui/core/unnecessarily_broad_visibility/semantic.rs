#![warn(unnecessarily_broad_visibility)]
#![allow(dead_code, misordered_module_declarations)]

pub(crate) struct PrivateStruct {
    pub(crate) private_field: usize,
}

pub mod parent {
    pub(crate) struct ParentConstructed {
        pub(crate) value: usize,
    }

    impl ParentConstructed {
        pub(crate) fn parent_method(&self) -> usize {
            self.value
        }
    }

    pub(super) mod inner {
        pub(crate) struct CrateUsed;
    }

    pub(crate) fn parent_only() {}

    pub(super) fn canonical_parent_boundary() {}
}

mod sibling {
    pub(crate) fn use_crate_boundary() {
        let value = super::parent::ParentConstructed { value: 1 };
        let _ = value.parent_method();
        let _ = super::parent::inner::CrateUsed;
        super::parent::canonical_parent_boundary();
    }
}

mod update_owner {
    #[derive(Default)]
    pub(super) struct UpdateValue {
        pub(super) explicit: usize,
        pub(super) retained: usize,
    }

    pub(super) enum Event {
        First,
    }

    impl Event {
        pub(super) fn position(&self) -> usize {
            0
        }
    }
}

mod update_consumer {
    fn exercise() {
        let base = super::update_owner::UpdateValue::default();
        let value = super::update_owner::UpdateValue {
            explicit: 1,
            ..base
        };
        let _ = value;

        let mut events = [super::update_owner::Event::First];
        events.sort_by_key(super::update_owner::Event::position);
    }
}

mod parse_owner {
    use std::str::FromStr;

    pub(crate) struct ParseError;

    pub(crate) struct Parsed;

    impl FromStr for Parsed {
        type Err = ParseError;

        fn from_str(_: &str) -> Result<Self, Self::Err> {
            Ok(Self)
        }
    }
}

mod parse_consumer {
    fn exercise() {
        let _ = "value".parse::<super::parse_owner::Parsed>();
    }
}

pub mod tuple_owner {
    pub(super) struct TupleValue(pub(crate) usize, pub(super) usize);
}

mod tuple_consumer {
    fn exercise() {
        let value = super::tuple_owner::TupleValue(1, 2);
        let super::tuple_owner::TupleValue(first, second) = value;
        let _ = (first, second);
    }
}

pub fn unused_public_function() {}

fn main() {
    parent::parent_only();
}

struct PrecedenceInner;

impl PrecedenceInner {
    fn first(&self) -> u8 {
        1
    }

    fn second(&self) -> u8 {
        2
    }
}

pub(crate) struct DelegatingPrecedence(pub(crate) PrecedenceInner);

impl DelegatingPrecedence {
    pub(crate) fn first(&self) -> u8 {
        self.0.first()
    }

    pub(crate) fn second(&self) -> u8 {
        self.0.second()
    }
}

fn exercise_precedence() {
    let value = DelegatingPrecedence(PrecedenceInner);
    let _ = value.first();
    let _ = value.second();
}
