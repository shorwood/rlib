#![warn(constructor_like_free_functions)]
#![allow(
    ad_hoc_string_parsers,
    dead_code,
    method_like_free_functions,
    misordered_module_declarations
)]

struct Session {
    token: String,
}

fn create_session_with_token(token: String) -> Session {
    Session { token }
}

enum MaybeSession {
    Present(Session),
}

fn maybe_session(token: Option<String>) -> Result<Option<MaybeSession>, ()> {
    Ok(token.map(|token| MaybeSession::Present(Session { token })))
}

struct Snapshot;

fn snapshot_of(snapshot: &Snapshot) -> Snapshot {
    Snapshot
}

struct JsonValue;

fn parse_json_value(source: &str) -> Result<JsonValue, ()> {
    let _ = source;
    Ok(JsonValue)
}

struct DeclaredOnly;

fn declared_only() -> DeclaredOnly {
    panic!()
}

mod remote {
    pub struct Remote;
}

fn create_remote() -> remote::Remote {
    remote::Remote
}

fn main() {}
