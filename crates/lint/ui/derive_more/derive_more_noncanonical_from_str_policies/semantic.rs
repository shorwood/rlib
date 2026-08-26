#![allow(dead_code, unknown_lints)]

#[derive(derive_more::FromStr)]
enum FactoredNames {
    #[from_str(rename_all = "camelCase")]
    UserCreated,
    #[from_str(rename_all = "camelCase")]
    UserDeleted,
    #[from_str(rename_all = "UPPERCASE")]
    LegacyMode,
}

#[derive(derive_more::FromStr)]
#[from_str(rename_all = "camelCase")]
enum RedundantOverride {
    #[from_str(rename_all = "camelCase")]
    UserCreated,
    UserDeleted,
}

#[derive(derive_more::FromStr)]
enum ImplicitCaseInsensitiveMustRemain {
    #[from_str(rename_all = "camelCase")]
    UserCreated,
    UserDeleted,
}

fn main() {}
