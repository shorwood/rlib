#![warn(rlib::ad_hoc_string_parsers)]
#![allow(
    dead_code,
    rlib::constructor_like_free_functions,
    rlib::misordered_module_declarations
)]

use std::str::FromStr;

struct UserId(u64);

fn parse_user_id(source: &str) -> Result<UserId, std::num::ParseIntError> {
    Ok(UserId(source.parse()?))
}

struct AccountId(u64);

impl AccountId {
    fn decode(source: &str) -> Result<Self, std::num::ParseIntError> {
        Ok(Self(source.parse()?))
    }
}

struct AliasedId(u64);

fn parse_aliased_id(source: &str) -> Result<AliasedId, std::num::ParseIntError> {
    // False-negative boundary: a returned local preserves construction provenance.
    let parsed = AliasedId(source.parse()?);
    Ok(parsed)
}

struct EarlyId(u64);

fn parse_early_id(source: &str) -> Result<EarlyId, std::num::ParseIntError> {
    return Ok(EarlyId(source.parse()?));
}

struct DiscardedTarget;

fn parse_discarded_target(source: &str) -> Result<DiscardedTarget, ()> {
    // False-positive boundary: a discarded construction cannot supply the success value.
    let _ = source.len();
    let _ = DiscardedTarget;
    Err(())
}

struct DormantTarget;

fn parse_dormant_target(source: &str) -> Result<DormantTarget, ()> {
    // False-positive boundary: closure-local construction and input use are dormant.
    let _later = || {
        let _ = source.len();
        DormantTarget
    };
    Err(())
}

struct DormantSource;

fn parse_dormant_source(source: &str) -> Result<DormantSource, ()> {
    // False-positive boundary: an uncalled closure does not consume parser input.
    let _later = || source.len();
    Ok(DormantSource)
}

struct NoOpSource;

fn parse_no_op_source(source: &str) -> Result<NoOpSource, ()> {
    // False-positive boundary: explicitly discarding the binding is not parser input use.
    let _ = source;
    Ok(NoOpSource)
}

struct Timestamp(u64);

fn parse_timestamp_rfc3339(source: &str) -> Result<Timestamp, ()> {
    let _ = source;
    Ok(Timestamp(0))
}

struct Flexible(u64);

fn parse_flexible(source: &str) -> Result<Flexible, ()> {
    let _ = source;
    Ok(Flexible(0))
}

fn decode_flexible(source: &str) -> Result<Flexible, ()> {
    let _ = source;
    Ok(Flexible(0))
}

struct Standard(u64);

impl FromStr for Standard {
    type Err = std::num::ParseIntError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        Ok(Self(source.parse()?))
    }
}

fn parse_standard(source: &str) -> Result<Standard, std::num::ParseIntError> {
    Ok(Standard(source.parse()?))
}

struct Borrowed<'source>(&'source str);

fn parse_borrowed(source: &str) -> Result<Borrowed<'_>, ()> {
    Ok(Borrowed(source))
}

struct Ignored;

fn parse_ignored(_source: &str) -> Result<Ignored, ()> {
    Ok(Ignored)
}

struct Configured;

fn parse_configured(source: &str, strict: bool) -> Result<Configured, ()> {
    let _ = (source, strict);
    Ok(Configured)
}

fn main() {}
