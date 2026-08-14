#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("bad")]
#[diagnostic(url("http://localhost/errors/bad"))]
struct Bad;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("good")]
#[diagnostic(url("https://docs.example.com/errors/good"))]
struct Good;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("bad")]
#[diagnostic(url("https://192.168.1.20/errors/bad"))]
struct PrivateNetwork;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("bad")]
#[diagnostic(url("https://docs.example.com"))]
struct MissingStablePath;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("bad")]
#[diagnostic(url("https://.example.com/errors/bad"))]
struct MalformedHost;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum VariantUrls {
    #[error("bad")]
    #[diagnostic(url("docs/errors/relative"))]
    Relative,
    #[error("host word outside authority")]
    #[diagnostic(url("https://docs.example.com/errors/mentions-localhost?target=localhost"))]
    HostWordOutsideAuthority,
    #[error("public IPv6 host")]
    #[diagnostic(url("https://[2001:4860:4860::8888]/errors/network"))]
    PublicIpv6,
}

fn main() {}
