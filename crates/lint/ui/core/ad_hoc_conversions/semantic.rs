#![warn(ad_hoc_conversions)]
#![allow(
    constructor_like_free_functions,
    dead_code,
    method_like_free_functions,
    misordered_module_declarations,
    missing_section_dividers,
    non_adjacent_struct_impls,
    unused_variables
)]

struct Record(String);
struct Account(String);

fn account_from_record(record: Record) -> Account {
    Account(record.0)
}

struct Token(String);
struct Session(String);

impl Token {
    fn into_session(self) -> Session {
        Session(self.0)
    }
}

struct Bytes(Vec<u8>);
struct Packet(Vec<u8>);

fn packet_from_bytes(bytes: Bytes) -> Result<Packet, &'static str> {
    Ok(Packet(bytes.0))
}

struct Source(String);
struct Destination(String);

fn assemble(source: Source) -> Destination {
    let text = source.0;
    Destination(text)
}

mod remote {
    pub struct Input(pub String);
}

struct Local(String);

fn local_from_input(input: remote::Input) -> Local {
    Local(input.0)
}

struct AmbiguousSource(String);
struct AmbiguousTarget(String);

fn alpha_conversion(source: AmbiguousSource) -> AmbiguousTarget {
    AmbiguousTarget(source.0)
}

fn beta_conversion(source: AmbiguousSource) -> AmbiguousTarget {
    AmbiguousTarget(source.0)
}

struct ExistingSource(String);
struct ExistingTarget(String);

impl From<ExistingSource> for ExistingTarget {
    fn from(source: ExistingSource) -> Self {
        Self(source.0)
    }
}

fn existing_from_source(source: ExistingSource) -> ExistingTarget {
    ExistingTarget(source.0)
}

struct PolicySource(i64);
struct PolicyTarget(i64);

fn policy_source_to_target_lossy(source: PolicySource) -> PolicyTarget {
    PolicyTarget(source.0)
}

struct Parser(u64);

fn parser_from_str(source: &str) -> Result<Parser, std::num::ParseIntError> {
    Ok(Parser(source.parse()?))
}

struct Borrowed<'source>(&'source str);

fn borrowed_from_str(source: &str) -> Result<Borrowed<'_>, ()> {
    Ok(Borrowed(source))
}

struct GenericTarget<const LENGTH: usize>([u8; LENGTH]);

fn generic_from_array<const LENGTH: usize>(source: [u8; LENGTH]) -> GenericTarget<LENGTH> {
    GenericTarget(source)
}

fn main() {}
