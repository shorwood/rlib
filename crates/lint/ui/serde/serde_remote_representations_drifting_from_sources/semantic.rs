#![allow(dead_code, unknown_lints)]

struct Source {
    id: u64,
    label: String,
}

#[derive(serde::Serialize)]
#[serde(remote = "Source")]
struct SourceDef {
    id: u64,
}

struct StableSource {
    id: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "StableSource")]
struct StableSourceDef {
    id: u64,
}

mod left {
    pub struct Source {
        pub id: u64,
        pub label: String,
    }
}

mod right {
    pub struct Source {
        pub id: u64,
    }
}

#[derive(serde::Serialize)]
#[serde(remote = "left::Source")]
struct QualifiedSourceDef {
    id: u64,
}

#[derive(serde::Serialize)]
#[serde(remote = "left::Source")]
/// A source representation.
struct UnrelatedDocumentationDef {
    id: u64,
}

#[derive(serde::Serialize)]
#[serde(remote = "left::Source")]
/// This versioned projection omits label and reconstructs it outside Serde.
struct DocumentedProjectionDef {
    id: u64,
}

struct SkippedSource {
    id: u64,
    label: String,
}

#[derive(serde::Serialize)]
#[serde(remote = "SkippedSource")]
struct SkippedSourceDef {
    id: u64,
    #[serde(skip)]
    label: String,
}

fn main() {}
