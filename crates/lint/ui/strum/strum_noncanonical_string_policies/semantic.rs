#![allow(dead_code, unknown_lints)]

#[derive(strum::Display)]
enum FactoredNames {
    #[strum(serialize = "userCreated")]
    UserCreated,
    #[strum(serialize = "userDeleted")]
    UserDeleted,
    #[strum(serialize = "legacy")]
    LegacyMode,
}

#[derive(strum::Display)]
#[strum(serialize_all = "camelCase")]
enum RedundantOverride {
    #[strum(serialize = "userCreated")]
    UserCreated,
    UserDeleted,
}

#[derive(strum::Display)]
enum RedundantOutputChoice {
    #[strum(serialize = "ready", to_string = "ready")]
    Ready,
}

#[derive(strum::Display)]
enum AffixedNames {
    #[strum(serialize = "userCreatedApi")]
    UserCreated,
    #[strum(serialize = "userDeletedApi")]
    UserDeleted,
    #[strum(serialize = "userUpdatedApi")]
    UserUpdated,
}

#[derive(strum::EnumString)]
enum PreservedParserAliases {
    #[strum(serialize = "apiUserCreated")]
    UserCreated,
    #[strum(serialize = "apiUserDeleted")]
    UserDeleted,
    #[strum(serialize = "apiUserUpdated")]
    UserUpdated,
}

fn main() {}
