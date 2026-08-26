#![allow(dead_code, unknown_lints)]

#[derive(derive_more::Display)]
enum FactoredNames {
    #[display("userCreated")]
    UserCreated,
    #[display("userDeleted")]
    UserDeleted,
    #[display("legacy")]
    LegacyMode,
}

#[derive(derive_more::Display)]
#[display(rename_all = "camelCase")]
enum RedundantOverrides {
    #[display(rename_all = "camelCase")]
    UserCreated,
    #[display("userDeleted")]
    UserDeleted,
}

#[derive(derive_more::Display)]
enum PayloadFormatsStayLocal {
    #[display("value: {_0}")]
    Value(u64),
    Plain,
}

fn main() {}
