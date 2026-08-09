#![warn(ad_hoc_string_parsers)]
#![allow(
    dead_code,
    constructor_like_free_functions,
    misordered_module_declarations
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
