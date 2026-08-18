#![allow(dead_code, rlib::serde_defaults_hiding_missing_data, unknown_lints)]

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Value {
    Detailed {
        id: u64,
        #[serde(default)]
        name: String,
    },
    Minimal {
        id: u64,
    },
}

#[derive(serde::Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum DisjointValue {
    Text { text: String },
    Number { number: u64 },
}

#[derive(serde::Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum NumericOverlap {
    Signed { value: i64 },
    Unsigned { value: u8 },
    Floating { value: f64 },
}

#[derive(serde::Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum TextOverlap {
    Character { value: char },
    Text { value: String },
    Flag { value: bool },
}

#[derive(serde::Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum SkippedVariant {
    Active {
        value: u64,
    },
    #[serde(skip_deserializing)]
    Ignored {
        value: u64,
    },
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum DefaultedOverlap {
    First {
        #[serde(default)]
        first: bool,
    },
    Second {
        #[serde(default)]
        second: String,
    },
}

fn main() {}
