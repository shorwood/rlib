#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error)]
#[error("marker")]
struct Marker;

impl miette::Diagnostic for Marker {}

#[derive(Debug, thiserror::Error)]
#[error("coded")]
struct Coded;

impl miette::Diagnostic for Coded {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        std::option::Option::Some(Box::new("domain::coded"))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("documented")]
struct Documented;

/// This documentation does not make the implementation custom.
impl miette::Diagnostic for Documented {}

#[derive(Debug, thiserror::Error)]
#[error("generic")]
struct Generic<T: std::fmt::Debug + Send + Sync + 'static>(std::marker::PhantomData<T>);

impl<T: std::fmt::Debug + Send + Sync + 'static> miette::Diagnostic for Generic<T> {}

#[derive(Debug, thiserror::Error)]
#[error("warning")]
struct StaticSeverity;

impl miette::Diagnostic for StaticSeverity {
    fn severity(&self) -> Option<miette::Severity> {
        Some(miette::Severity::Warning)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("sourced")]
struct ForwardedSource {
    input: miette::NamedSource<String>,
}

impl miette::Diagnostic for ForwardedSource {
    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.input)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("custom")]
struct Custom;

impl miette::Diagnostic for Custom {
    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        if std::env::var_os("HELP").is_some() {
            Some(Box::new("dynamic"))
        } else {
            None
        }
    }
}

mod shadowed_some {
    #[allow(non_snake_case)]
    fn Some(value: Box<dyn std::fmt::Display>) -> Option<Box<dyn std::fmt::Display>> {
        std::option::Option::Some(value)
    }

    #[derive(Debug, thiserror::Error)]
    #[error("custom constructor")]
    struct CustomConstructor;

    impl miette::Diagnostic for CustomConstructor {
        fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
            Some(Box::new("not the standard option constructor"))
        }
    }
}

mod shadowed_box_new {
    struct Factory;

    impl Factory {
        fn new(_: &'static str) -> Box<dyn std::fmt::Display> {
            Box::new("computed elsewhere")
        }
    }

    #[derive(Debug, thiserror::Error)]
    #[error("custom factory")]
    struct CustomFactory;

    impl miette::Diagnostic for CustomFactory {
        fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
            Some(Factory::new("not Box::new"))
        }
    }
}

mod shadowed_severity {
    #[allow(non_upper_case_globals)]
    const Advice: miette::Severity = miette::Severity::Error;

    #[derive(Debug, thiserror::Error)]
    #[error("custom severity constant")]
    struct CustomSeverity;

    impl miette::Diagnostic for CustomSeverity {
        fn severity(&self) -> Option<miette::Severity> {
            Some(Advice)
        }
    }
}

fn main() {}
