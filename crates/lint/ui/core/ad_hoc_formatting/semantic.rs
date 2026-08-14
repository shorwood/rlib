#![warn(ad_hoc_formatting)]
#![allow(dead_code, misordered_module_declarations, missing_section_dividers)]

use std::borrow::Cow;
use std::fmt::{self, Display, Formatter};

struct UserId(u64);

impl UserId {
    fn to_text(&self) -> String {
        format!("user-{}", self.0)
    }
}

struct Ambiguous(u64);

impl Ambiguous {
    fn display_value(&self) -> String {
        self.0.to_string()
    }

    fn render_text(&self) -> String {
        format!("{}", self.0)
    }
}

struct Existing(u64);

impl Display for Existing {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl Existing {
    fn render_text(&self) -> String {
        self.to_string()
    }
}

struct BorrowedText(String);

impl BorrowedText {
    fn display_value(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.0)
    }
}

struct FreeValue(u64);

fn render_free_value(value: &FreeValue) -> String {
    value.0.to_string()
}

struct Distinct(u64);

impl Display for Distinct {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl Distinct {
    fn render_text(&self) -> String {
        format!("distinct {}", self.0)
    }

    fn render_html(&self) -> String {
        format!("<span>{}</span>", self.0)
    }
}

struct AccessToken(String);

impl AccessToken {
    fn to_text(&self) -> String {
        self.0.clone()
    }
}

struct Borrowed(String);

impl Borrowed {
    fn as_str(&self) -> &str {
        &self.0
    }
}

struct Configured(u64);

impl Configured {
    fn format_with(&self, prefix: &str) -> String {
        format!("{prefix}{}", self.0)
    }
}

struct Generic<T>(T);

impl<T: ToString> Generic<T> {
    fn to_text(&self) -> String {
        self.0.to_string()
    }
}

struct UnrelatedText(u64);

impl UnrelatedText {
    // False-positive boundary: reading the receiver does not make constant output its display.
    fn render_text(&self) -> String {
        let _value = self.0;
        "constant".to_owned()
    }
}

struct AliasedText(u64);

impl AliasedText {
    // False-negative boundary: receiver-derived aliases preserve presentation provenance.
    fn render_text(&self) -> String {
        let value = &self.0;
        value.to_string()
    }
}

struct DormantText(u64);

impl DormantText {
    // False-positive boundary: an uncalled closure does not supply the helper's text result.
    fn render_text(&self) -> String {
        let _render = || self.0.to_string();
        "constant".to_owned()
    }
}

struct EarlyText(u64);

impl EarlyText {
    // False-negative boundary: explicit returns participate in presentation ownership.
    fn render_text(&self) -> String {
        if self.0 == 0 {
            return self.0.to_string();
        }
        "nonzero".to_owned()
    }
}

fn main() {}
