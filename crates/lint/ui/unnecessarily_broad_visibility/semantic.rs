#![warn(unnecessarily_broad_visibility)]
#![allow(dead_code)]

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

pub fn unused_public_function() {}

fn main() {
    parent::parent_only();
}
