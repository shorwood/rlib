pub(crate) struct Inherent;

pub(crate) struct Traited;

pub(crate) struct Aliased;

pub(crate) type Alias = Aliased;

pub(crate) struct Generic<T>(pub(crate) T);

pub(crate) struct Referenced;

pub(crate) enum Choice {
    Value,
}

pub(crate) struct ExternalMacroTarget;
