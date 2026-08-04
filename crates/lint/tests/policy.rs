use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Attribute, ExprCall, ItemMod, Meta, Token};

#[test]
fn production_sources_do_not_allow_project_lints() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let project_lints = declared_lints(&source_root);
    for path in rust_sources(&source_root) {
        let source = fs::read_to_string(&path).expect("production Rust source should be readable");
        let syntax = syn::parse_file(&source).expect("production Rust source should parse");
        let mut allows = AllowCollector::default();
        allows.visit_file(&syntax);
        for lint in allows.lints {
            assert!(
                !project_lints.contains(&lint),
                "{} broadly allows project lint `{lint}`; fix the violation or use a narrow, reasoned expectation",
                path.display()
            );
        }
    }
}

#[test]
fn every_declared_lint_is_registered_once() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let declared = declared_lints(&source_root);
    let source = fs::read_to_string(source_root.join("registration.rs"))
        .expect("lint registration source should be readable");
    let syntax = syn::parse_file(&source).expect("lint registration source should parse");
    let mut registrations = RegistrationCollector::default();
    registrations.visit_file(&syntax);
    assert_eq!(registrations.lints, declared);
}

fn declared_lints(source_root: &Path) -> BTreeSet<String> {
    let source = fs::read_to_string(source_root.join("rules/core/mod.rs"))
        .expect("core rule module should be readable");
    let syntax = syn::parse_file(&source).expect("core rule module should parse");
    syntax
        .items
        .into_iter()
        .filter_map(|item| match item {
            syn::Item::Mod(ItemMod { ident, .. }) => Some(ident.to_string()),
            _ => None,
        })
        .collect()
}

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_owned()];
    let mut sources = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).expect("source directory should be readable") {
            let path = entry.expect("source entry should be readable").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
    }
    sources
}

#[derive(Default)]
struct AllowCollector {
    lints: Vec<String>,
}

impl AllowCollector {
    fn collect_meta(&mut self, meta: &Meta) {
        let Meta::List(list) = meta else { return };
        if list.path.is_ident("allow") {
            let arguments = list
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .expect("allow arguments should be lint metadata");
            self.lints.extend(arguments.into_iter().filter_map(|meta| {
                match meta {
                    Meta::Path(path) => path
                        .segments
                        .last()
                        .map(|segment| segment.ident.to_string()),
                    Meta::List(_) | Meta::NameValue(_) => None,
                }
            }));
        } else if list.path.is_ident("cfg_attr") {
            let nested = list
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .expect("cfg_attr arguments should be metadata");
            for meta in nested.into_iter().skip(1) {
                self.collect_meta(&meta);
            }
        }
    }
}

impl<'syntax> Visit<'syntax> for AllowCollector {
    fn visit_attribute(&mut self, attribute: &'syntax Attribute) {
        self.collect_meta(&attribute.meta);
        visit::visit_attribute(self, attribute);
    }
}

#[derive(Default)]
struct RegistrationCollector {
    lints: BTreeSet<String>,
}

impl<'syntax> Visit<'syntax> for RegistrationCollector {
    fn visit_expr_call(&mut self, call: &'syntax ExprCall) {
        if let syn::Expr::Path(path) = &*call.func {
            let segments = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            if let [rules, core, lint, register] = segments.as_slice()
                && rules == "rules"
                && core == "core"
                && register == "register_lints"
            {
                assert!(
                    self.lints.insert(lint.clone()),
                    "lint `{lint}` is registered more than once"
                );
            }
        }
        visit::visit_expr_call(self, call);
    }
}

#[cfg(test)]
mod tests {
    use super::{AllowCollector, Visit};

    #[test]
    fn finds_spaced_multiline_namespaced_and_conditional_allows() {
        let syntax = syn::parse_file(
            r#"
                #[allow (bare_tuple_types, reason = "fixture")]
                struct Spaced;

                #[allow(
                    rlib_lint::nested_tuple_types,
                )]
                struct Namespaced;

                #[cfg_attr(test, allow(positional_aggregate_fields))]
                struct Conditional;
            "#,
        )
        .expect("fixture should parse");
        let mut collector = AllowCollector::default();
        collector.visit_file(&syntax);
        assert_eq!(
            collector.lints,
            [
                "bare_tuple_types",
                "nested_tuple_types",
                "positional_aggregate_fields"
            ]
        );
    }
}
