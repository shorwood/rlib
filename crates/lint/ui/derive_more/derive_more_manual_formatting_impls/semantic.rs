#![allow(dead_code, rlib::misordered_module_declarations, unknown_lints)]

use std::fmt::{self, Display, Formatter, LowerHex};

struct UserId(u64);

impl Display for UserId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl LowerHex for UserId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        LowerHex::fmt(&self.0, formatter)
    }
}

struct Qualified(u64);

impl Display for Qualified {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        std::write!(formatter, "qualified {}", self.0)
    }
}

struct LayerPath(String);

impl Display for LayerPath {
    /// Render this layer path.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

struct DependencyDetails {
    component: String,
    path: std::path::PathBuf,
}

impl Display for DependencyDetails {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "component `{}` depends on `{}`",
            self.component,
            self.path.display()
        )
    }
}

enum SnapshotNameError {
    MissingSeparator,
    MissingState,
}

impl Display for SnapshotNameError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingSeparator => "snapshot name is missing ` @`",
            Self::MissingState => "snapshot name is missing a state",
        })
    }
}

struct Documented(u64);

/// This formatting implementation is an authored compatibility contract.
impl Display for Documented {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

struct MethodDocumented(u64);

impl Display for MethodDocumented {
    /// This formatting method is an authored compatibility contract.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

mod shadowed_macro {
    use super::{Display, Formatter, fmt};

    macro_rules! write {
        ($formatter:expr, $format:literal, $field:expr) => {{
            let _ = ($formatter, $format, $field);
            Ok(())
        }};
    }

    struct Shadowed(u64);

    impl Display for Shadowed {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
            // False-positive boundary: this is not the standard formatting macro.
            write!(formatter, "{}", self.0)
        }
    }
}

struct Redacted(String);

impl Display for Redacted {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "[redacted:{}]", self.0.len())
    }
}

fn main() {}
